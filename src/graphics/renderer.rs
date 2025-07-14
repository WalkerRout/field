use crate::graphics::primitives;

pub struct Renderer {
  width: usize,
  height: usize,
  buffer: Vec<u32>,
}

impl Renderer {
  pub fn new(width: usize, height: usize) -> Self {
    Self {
      width,
      height,
      buffer: vec![0; width * height],
    }
  }

  pub fn resize(&mut self, width: usize, height: usize) {
    if self.width != width || self.height != height {
      self.width = width;
      self.height = height;
      self.buffer.resize(width * height, 0);
    }
  }

  pub fn clear(&mut self) {
    self.buffer.fill(0x001A1A1A);
  }

  pub fn draw_line(&mut self, x1: usize, y1: isize, x2: usize, y2: isize, color: u32) {
    primitives::draw_line(
      &mut self.buffer,
      self.width,
      self.height,
      (x1, y1),
      (x2, y2),
      color,
    );
  }

  pub fn draw_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
    primitives::draw_rect(
      &mut self.buffer,
      self.width,
      self.height,
      (x, y),
      w,
      h,
      color,
    );
  }

  pub fn draw_text(&mut self, text: &str, x: usize, y: usize, color: u32) {
    primitives::draw_text(
      &mut self.buffer,
      self.width,
      self.height,
      text,
      (x, y),
      color,
    );
  }

  pub fn set_pixel(&mut self, x: usize, y: usize, color: u32) {
    if x < self.width && y < self.height {
      self.buffer[y * self.width + x] = color;
    }
  }

  pub fn buffer(&self) -> &[u32] {
    &self.buffer
  }

  pub fn dimensions(&self) -> (usize, usize) {
    (self.width, self.height)
  }
}
