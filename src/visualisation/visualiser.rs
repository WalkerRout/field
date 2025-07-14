use std::cell::Cell;

use chrono::{Local, Timelike};

use rand::Rng;

use crate::audio::AudioConfig;
use crate::audio::backend::AudioPacket;
use crate::audio::processor::AudioProcessor;

use crate::graphics::renderer::Renderer;

use crate::visualisation::spectrum::SpectrumAnalyzer;
use crate::visualisation::waveform::WaveformDisplay;

pub struct Visualiser {
  processor: AudioProcessor,
  spectrum: SpectrumAnalyzer,
  waveform: WaveformDisplay,
  config: AudioConfig,
  // width, height
  window_dims: Cell<(usize, usize)>,
}

impl Visualiser {
  pub fn new(config: AudioConfig, initial_width: usize) -> Self {
    Self {
      processor: AudioProcessor::new(config.clone()),
      spectrum: SpectrumAnalyzer::new(config.bar_count),
      waveform: WaveformDisplay::new(initial_width),
      config,
      window_dims: Cell::from((initial_width, 0)),
    }
  }

  pub fn update(&mut self, packet: &AudioPacket) {
    if packet.is_silent {
      self.processor.process(&[], packet.sample_rate);
      self.waveform.decay();
    } else {
      let mono_samples = self.mix_to_mono(&packet.samples, packet.channels);
      // process fft...
      self.processor.process(&mono_samples, packet.sample_rate);
      self
        .waveform
        .update(&mono_samples, self.processor.fft_output());
    }
    // update spectrum with processed...
    self.spectrum.update(self.processor.spectrum(), self.window_dims.get().1);
  }

  pub fn resize(&mut self, width: usize) {
    self.waveform.resize(width);
  }

  pub fn render(&self, renderer: &mut Renderer) {
    let (width, height) = renderer.dimensions();
    self.window_dims.set((width, height));

    self.waveform.render(renderer);
    self.spectrum.render(renderer);
    self.render_particles(renderer);

    // draw current 24hour time...
    let now = Local::now();
    let hour = now.hour();
    let minute = now.minute();
    let mut time_buffer = [0u8; 5];
    time_buffer[0] = b'0' + (hour / 10) as u8;
    time_buffer[1] = b'0' + (hour % 10) as u8;
    time_buffer[2] = b':';
    time_buffer[3] = b'0' + (minute / 10) as u8;
    time_buffer[4] = b'0' + (minute % 10) as u8;

    let time_str = std::str::from_utf8(&time_buffer).unwrap();
    renderer.draw_text(time_str, 10, 10, 0x00FFFFFF);
  }

  fn mix_to_mono(&self, samples: &[f32], channels: u16) -> Vec<f32> {
    let frame_count = samples.len() / channels as usize;
    let mut mono = vec![0.0; frame_count.min(self.config.buffer_size)];

    for (i, m) in mono.iter_mut().enumerate() {
      let mut sum = 0.0;
      for ch in 0..channels as usize {
        let idx = i * channels as usize + ch;
        if idx < samples.len() {
          sum += samples[idx];
        }
      }
      *m = sum / channels as f32;
    }

    mono
  }

  fn render_particles(&self, renderer: &mut Renderer) {
    let total_energy: f32 = self.processor.spectrum().iter().sum();
    let particle_count = (total_energy * 100.0) as usize;
    let (width, height) = renderer.dimensions();

    let mut rng = rand::rng();
    for _ in 0..particle_count.min(50) {
      let x = rng.random_range(0..width);
      let y = rng.random_range(0..height);
      let r = rng.random_range(180..255);
      let g = rng.random_range(100..200);
      let b = rng.random_range(200..255);
      let color = (r << 16) | (g << 8) | b;
      renderer.set_pixel(x, y, color);
    }
  }
}
