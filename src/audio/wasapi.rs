use std::ptr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use triple_buffer::Input;

use tokio::task;

use tracing::info;

use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::Media::Audio::{
  AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
  AUDCLNT_STREAMFLAGS_LOOPBACK, IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator,
  MMDeviceEnumerator, eConsole, eRender,
};
use windows::Win32::System::Com::{
  CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize,
};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};

use crate::audio::AudioConfig;
use crate::audio::backend::{AudioBackend, AudioPacket};

pub struct WasapiBackend {
  config: AudioConfig,
  stop: Arc<AtomicBool>,
}

impl WasapiBackend {
  pub fn new(config: AudioConfig, stop: Arc<AtomicBool>) -> Self {
    Self { config, stop }
  }
}

impl AudioBackend for WasapiBackend {
  type Error = anyhow::Error;

  async fn run(self, tx: Input<AudioPacket>) -> Result<(), Self::Error> {
    task::spawn_blocking(move || capture_loop(self.config, self.stop, tx)).await??;
    Ok(())
  }
}

fn capture_loop(
  _config: AudioConfig,
  stop: Arc<AtomicBool>,
  tx: Input<AudioPacket>,
) -> Result<(), anyhow::Error> {
  unsafe {
    // init com
    CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
    // create event for audio notifications
    let event_handle = CreateEventW(None, false, false, None)?;
    if event_handle.is_invalid() {
      return Err(anyhow::anyhow!("Failed to create event"));
    }
    // get default loopback device
    let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
    let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
    // activate client
    let audio_client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;

    // get mix format
    let pwfx_ptr = audio_client.GetMixFormat()?;
    let pwfx = &*pwfx_ptr;
    let sample_rate = pwfx.nSamplesPerSec;
    let channels = pwfx.nChannels;

    // 20ms for low latency while maintaining stability
    let hns_buffer = 200_000i64;
    // init with event callback
    audio_client.Initialize(
      AUDCLNT_SHAREMODE_SHARED,
      AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
      hns_buffer,
      0,
      pwfx_ptr,
      None,
    )?;
    CoTaskMemFree(Some(pwfx_ptr.cast()));
    // set event handle
    audio_client.SetEventHandle(event_handle)?;
    // get capture client
    let capture_client: IAudioCaptureClient = audio_client.GetService()?;

    // run event loop...
    let result = capture_loop_inner(
      event_handle,
      &audio_client,
      &capture_client,
      sample_rate,
      channels,
      stop,
      tx,
    );

    // clean up
    audio_client.Stop()?;
    let _ = CloseHandle(event_handle);
    CoUninitialize();

    info!("wasapi capture stopped...");
    result
  }
}

fn capture_loop_inner(
  event_handle: HANDLE,
  audio_client: &IAudioClient,
  capture_client: &IAudioCaptureClient,
  sample_rate: u32,
  channels: u16,
  stop: Arc<AtomicBool>,
  mut tx: Input<AudioPacket>,
) -> Result<(), anyhow::Error> {
  unsafe {
    // pre alloc buffers
    let max_frames = audio_client.GetBufferSize()?;
    let max_len = (max_frames as usize).saturating_mul(channels as usize);
    let mut samples_buf: Vec<f32> = Vec::with_capacity(max_len);
    let mut batch_buffer: Vec<AudioPacket> = Vec::with_capacity(8);

    // start streaming
    audio_client.Start()?;
    info!("event driven wasapi capture started...");

    let mut last_process = Instant::now();
    let throttle_duration = Duration::from_micros(4000); // 4ms
    let timeout_ms = 100; // 100ms timeout to check stop flag

    while !stop.load(Ordering::Relaxed) {
      // wait for audio event or timeout after 100ms
      let wait_result = WaitForSingleObject(event_handle, timeout_ms);

      match wait_result {
        WAIT_OBJECT_0 => {
          // event signaled - check throttle
          let elapsed = last_process.elapsed();
          if elapsed < throttle_duration {
            // too soon, wait remaining time
            std::hint::spin_loop();
            continue;
          }
          last_process = Instant::now();

          // process all available buffers
          batch_buffer.clear();

          loop {
            let mut data_ptr: *mut u8 = ptr::null_mut();
            let mut frames_avail = 0u32;
            let mut flags = 0u32;

            match capture_client.GetBuffer(&mut data_ptr, &mut frames_avail, &mut flags, None, None)
            {
              Ok(()) => {
                if frames_avail == 0 {
                  break;
                }

                let len = (frames_avail as usize).saturating_mul(channels as usize);
                let is_silent = (flags & (AUDCLNT_BUFFERFLAGS_SILENT.0 as u32)) != 0;

                samples_buf.clear();
                if is_silent {
                  samples_buf.resize(len, 0.0);
                } else {
                  let slice = std::slice::from_raw_parts(data_ptr as *const f32, len);
                  samples_buf.extend_from_slice(slice);
                }

                capture_client.ReleaseBuffer(frames_avail)?;

                batch_buffer.push(AudioPacket {
                  samples: std::mem::take(&mut samples_buf),
                  sample_rate: sample_rate as f32,
                  channels,
                  is_silent,
                });
              }
              Err(_) => break,
            }
          }

          // send batched packets
          if let Some(packet) = batch_buffer.drain(..).next_back() {
            tx.write(packet);
          }
        }
        _ => continue, // timeout - check stop flag on next iteration
      }
    }

    Ok(())
  }
}
