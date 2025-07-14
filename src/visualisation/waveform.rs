use crate::graphics::renderer::Renderer;

struct BandParams {
  factor: f32, // energy-based scaling
  colour: u32,
}

// audio bands... (start, end, colour)
const BANDS: &[(usize, usize, u32)] = &[
  (0, 4, 0x00FF0000), // sub-bass - red
  (4, 12, 0x00FF7F00), // bass - orange
  (12, 24, 0x00FFFF00), // low-mid - yellow
  (24, 40, 0x0000FF00), // mid - green
  (40, 56, 0x000000FF), // high-mid - blue
  (56, 80, 0x004B0082), // treble - indigo
];

const BANDS_LEN: usize = BANDS.len();
const BANDS_HALF_LEN: usize = BANDS_LEN / 2;

pub struct WaveformDisplay {
  samples: Vec<f32>,
  width: usize,
  bands: Vec<BandParams>,
}

impl WaveformDisplay {
  pub fn new(width: usize) -> Self {
    let bands: Vec<_> = BANDS
      .iter()
      .map(|(_, _, col)| BandParams {
        factor: 0.0,
        colour: *col,
      })
      .collect();
    Self {
      samples: vec![0.0; width],
      width,
      bands,
    }
  }

  pub fn resize(&mut self, new_width: usize) {
    if new_width != self.width {
      self.samples.resize(new_width, 0.0);
      self.width = new_width;
    }
  }

  pub fn update(&mut self, new_samples: &[f32], fft_output: &[f32]) {
    // update samples...
    if !new_samples.is_empty() {
      for x in 0..self.width {
        let idx = (x as f32 / self.width as f32 * new_samples.len() as f32) as usize;
        if idx < new_samples.len() {
          self.samples[x] = self.samples[x] * 0.5 + new_samples[idx] * 0.5;
        }
      }
    }

    // update bands given current fft...
    self
      .bands
      .iter_mut()
      .zip(BANDS)
      .enumerate()
      .for_each(|(i, (band, (start, end, _)))| {
        let energy = calculate_band_energy(fft_output, *start, *end);
        let gain = (i as f32 + 1.0) * 0.4;
        // we apply waveform height in render... more fitting...
        band.factor = gain * energy;
      });
  }

  pub fn decay(&mut self) {
    for sample in &mut self.samples {
      // decrease sample strength just a little bit...
      *sample *= 0.95;
    }
  }

  pub fn render(&self, renderer: &mut Renderer) {
    let (width, height) = renderer.dimensions();
    let center_y = (height / 2) as isize;
    let waveform_h = height / 5;

    // draw a center line...
    for x in 0..width {
      renderer.set_pixel(x, center_y as usize, 0x00333333);
    }

    // clamp samples to width
    let samples = &self.samples[..width.min(self.samples.len())];

    // for each band, calculate offset and render
    for (i, band) in self.bands.iter().enumerate() {
      let offset_y = center_y + (i as isize - BANDS_HALF_LEN as isize) * 60;
      let factor = band.factor * waveform_h as f32;

      // zip pairs (s0,s1) for drawing line segments
      for (x, (s0, s1)) in samples.iter().zip(samples.iter().skip(1)).enumerate() {
        let y0 = offset_y + (s0 * factor) as isize;
        let y1 = offset_y + (s1 * factor) as isize;
        renderer.draw_line(x, y0, x + 1, y1, band.colour);
      }
    }
  }
}

#[inline]
fn calculate_band_energy(fft_output: &[f32], start: usize, end: usize) -> f32 {
  let energy: f32 = fft_output
    .iter()
    .take(end.min(fft_output.len()))
    .skip(start)
    .sum();
  energy / (end - start).max(1) as f32
}
