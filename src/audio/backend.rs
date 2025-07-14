use triple_buffer::Input;

#[derive(Clone)]
pub struct AudioPacket {
  pub samples: Vec<f32>,
  pub sample_rate: f32,
  pub channels: u16,
  pub is_silent: bool,
}

impl Default for AudioPacket {
  fn default() -> Self {
    Self {
      samples: Vec::new(),
      sample_rate: 0.0,
      channels: 0,
      is_silent: true,
    }
  }
}

pub trait AudioBackend: Send {
  type Error;

  async fn run(self, tx: Input<AudioPacket>) -> Result<(), Self::Error>;
}
