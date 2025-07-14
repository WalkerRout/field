#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tracing::info;

use tracing_subscriber::filter::LevelFilter;

mod app;
mod audio;
mod graphics;
mod visualisation;

use app::App;
use audio::AudioConfig;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), anyhow::Error> {
  tracing_subscriber::fmt()
    .with_max_level(LevelFilter::INFO)
    .with_target(false)
    .init();

  // default config...
  let config = AudioConfig {
    fft_size: 1024,
    buffer_size: 2048,
    bar_count: 64,
  };

  info!("audio visualizer spinning up...");

  let mut app = App::new(config)?;
  app.run().await?;

  info!("audio visualizer spinning down...");
  Ok(())
}
