pub mod backend;
pub mod processor;

#[cfg(target_os = "windows")]
pub mod wasapi;

#[derive(Clone)]
pub struct AudioConfig {
  pub fft_size: usize,
  pub buffer_size: usize,
  pub bar_count: usize,
}
