use crate::graphics::renderer::Renderer;

pub struct SpectrumAnalyzer {
  bar_count: usize,
  peak_levels: Vec<f32>,
  peak_velocities: Vec<f32>,
  window_height: usize,
  // precomputed
  colour_lut: Vec<u32>,
}

impl SpectrumAnalyzer {
  pub fn new(bar_count: usize) -> Self {
    Self {
      bar_count,
      peak_levels: vec![0.0; bar_count],
      peak_velocities: vec![0.0; bar_count],
      window_height: 0,
      colour_lut: Vec::new(),
    }
  }

  pub fn update(&mut self, spectrum: &[f32], max_height: usize) {
    const GRAVITY: f32 = 0.001;
    const DAMPING: f32 = 0.98;
    const ATTACK: f32 = 0.15;
    // colour values
    const TOP: (u8, u8, u8) = (0x8F, 0x4E, 0x8B);
    const BOTTOM: (u8, u8, u8) = (0x45, 0x3A, 0x62);

    if max_height != self.window_height || self.colour_lut.is_empty() {
      self.window_height = max_height;
      self.colour_lut.clear();
      // lut entries from 0..=max_height
      for h in 0..=max_height {
        let t = h as f32 / max_height as f32;
        let r = ((1.0 - t) * TOP.0 as f32 + t * BOTTOM.0 as f32) as u32;
        let g = ((1.0 - t) * TOP.1 as f32 + t * BOTTOM.1 as f32) as u32;
        let b = ((1.0 - t) * TOP.2 as f32 + t * BOTTOM.2 as f32) as u32;
        self.colour_lut.push((r << 16) | (g << 8) | b);
      }
    }

    for ((level, velocity), spec) in self
      .peak_levels
      .iter_mut()
      .zip(self.peak_velocities.iter_mut())
      .zip(spectrum)
      .take(self.bar_count)
    {
      if spec > level {
        // instant attack for new peaks
        *level = *level * (1.0 - ATTACK) + spec * ATTACK;
        *velocity = 0.0;
      } else {
        // physics-based decay...
        *velocity += GRAVITY;
        *velocity *= DAMPING;
        *level = (*level - *velocity).max(0.0);
      }
    }
  }

  pub fn render(&self, renderer: &mut Renderer) {
    let (width, _) = renderer.dimensions();
    let bar_width = 5;
    let spacing = 2;
    let bottom_offset = 20;
    let total_width = self.bar_count * (bar_width + spacing);
    let start_x = width.saturating_sub(total_width) / 2;
    let max_height = (self.window_height - 20).min(200);

    for i in 0..self.bar_count {
      let height = (self.peak_levels[i] * max_height as f32) as usize;
      if height == 0 {
        continue;
      }

      let x = start_x + i * (bar_width + spacing);
      let y = self.window_height - bottom_offset - height;

      // single rect call with gradient precomputed...
      for h in 0..height {
        let color_idx = (h * self.colour_lut.len() / max_height).min(self.colour_lut.len() - 1);
        let color = self.colour_lut[color_idx];
        renderer.draw_rect(x, y + h, bar_width, 1, color);
      }
    }
  }
}
