use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use minifb::{Key, Scale, ScaleMode, Window, WindowOptions};

use triple_buffer::Output;

use tokio::task::{self, JoinHandle};

use tracing::error;

use crate::audio::AudioConfig;
use crate::audio::backend::{AudioBackend, AudioPacket};
#[cfg(target_os = "windows")]
use crate::audio::wasapi::WasapiBackend as Backend;

use crate::graphics::renderer::Renderer;

use crate::visualisation::visualiser::Visualiser;

const DEFAULT_WIDTH: usize = 1400;
const DEFAULT_HEIGHT: usize = 600;

pub struct App {
  window: Window,
  renderer: Renderer,
  visualiser: Visualiser,
  audio_rx: Output<AudioPacket>,
  audio_handle: Option<JoinHandle<()>>,
  stop: Arc<AtomicBool>,
}

impl App {
  pub fn new(config: AudioConfig) -> Result<Self, anyhow::Error> {
    // create window
    let window_options = WindowOptions {
      resize: true,
      scale: Scale::X1,
      scale_mode: ScaleMode::Stretch,
      ..Default::default()
    };
    let window = Window::new("a field", DEFAULT_WIDTH, DEFAULT_HEIGHT, window_options)?;

    #[cfg(not(target_os = "windows"))]
    compile_error!("windows only for now...");

    // create audio backend...
    let stop = Arc::new(AtomicBool::new(false));
    let audio_backend = Backend::new(config.clone(), Arc::clone(&stop));

    // create channel for audio packets
    let (audio_tx, audio_rx) = triple_buffer::triple_buffer(&AudioPacket::default());

    // spawn audio capture task
    let audio_handle = tokio::spawn(async move {
      if let Err(e) = audio_backend.run(audio_tx).await {
        error!("audio capture error - {}", e);
      }
    });

    let renderer = Renderer::new(DEFAULT_WIDTH, DEFAULT_HEIGHT);
    let visualiser = Visualiser::new(config, DEFAULT_WIDTH);

    Ok(Self {
      window,
      renderer,
      visualiser,
      audio_rx,
      audio_handle: Some(audio_handle),
      stop,
    })
  }

  pub async fn run(&mut self) -> Result<(), anyhow::Error> {
    self.window.set_target_fps(60);

    while self.window.is_open() && !self.window.is_key_down(Key::Escape) {
      // observe current window size...
      let (width, height) = self.window.get_size();
      // process user inputs...
      self.handle_input();
      // process audio packet living in buffer...
      let packet = self.audio_rx.read();
      self.visualiser.update(packet);
      // live resize if the dimensions changed
      self.resize(width, height);
      // render a frame...
      self.renderer.clear();
      self.visualiser.render(&mut self.renderer);
      self
        .window
        .update_with_buffer(self.renderer.buffer(), width, height)?;
      // yield to other tasks
      task::yield_now().await;
    }

    Ok(())
  }

  fn resize(&mut self, width: usize, height: usize) {
    self.renderer.resize(width, height);
    self.visualiser.resize(width);
  }

  fn handle_input(&mut self) {
    // stub
  }
}

impl Drop for App {
  fn drop(&mut self) {
    self.stop.store(true, Ordering::Relaxed);
    if let Some(handle) = self.audio_handle.take() {
      handle.abort();
    }
  }
}
