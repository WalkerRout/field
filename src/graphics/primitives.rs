pub fn draw_line(
  buffer: &mut [u32],
  width: usize,
  height: usize,
  (x1, y1): (usize, isize),
  (x2, y2): (usize, isize),
  color: u32,
) {
  let dx = (x2 as isize - x1 as isize).abs();
  let dy = (y2 - y1).abs();
  let sx = if x1 < x2 { 1isize } else { -1isize };
  let sy = if y1 < y2 { 1isize } else { -1isize };
  let mut err = dx - dy;

  let mut x = x1 as isize;
  let mut y = y1;

  loop {
    if x >= 0 && x < width as isize && y >= 0 && y < height as isize {
      buffer[y as usize * width + x as usize] = color;
    }

    if x == x2 as isize && y == y2 {
      break;
    }

    let e2 = 2 * err;
    if e2 > -dy {
      err -= dy;
      x += sx;
    }
    if e2 < dx {
      err += dx;
      y += sy;
    }
  }
}

pub fn draw_rect(
  buffer: &mut [u32],
  width: usize,
  height: usize,
  (x, y): (usize, usize),
  w: usize,
  h: usize,
  color: u32,
) {
  for dy in 0..h {
    for dx in 0..w {
      let px = x + dx;
      let py = y + dy;
      if px < width && py < height {
        buffer[py * width + px] = color;
      }
    }
  }
}

pub fn draw_text(
  buffer: &mut [u32],
  width: usize,
  height: usize,
  text: &str,
  (x, y): (usize, usize),
  color: u32,
) {
  let mut pos_x = x;
  for c in text.chars() {
    if pos_x + 8 < width && y + 10 < height {
      match c {
        '0'..='9' => draw_digit(
          buffer,
          width,
          height,
          c as usize - '0' as usize,
          (pos_x, y),
          color,
        ),
        ':' => {
          buffer[(y + 3) * width + pos_x + 2] = color;
          buffer[(y + 7) * width + pos_x + 2] = color;
        }
        _ => {}
      }
      pos_x += 8;
    }
  }
}

fn draw_digit(
  buffer: &mut [u32],
  width: usize,
  height: usize,
  digit: usize,
  (x, y): (usize, usize),
  color: u32,
) {
  // typical segmented display layout from 0..=9...
  const SEGMENTS: [[bool; 7]; 10] = [
    [true, true, true, false, true, true, true],     // 0
    [false, false, true, false, false, true, false], // 1
    [true, false, true, true, true, false, true],    // 2
    [true, false, true, true, false, true, true],    // 3
    [false, true, true, true, false, true, false],   // 4
    [true, true, false, true, false, true, true],    // 5
    [true, true, false, true, true, true, true],     // 6
    [true, false, true, false, false, true, false],  // 7
    [true, true, true, true, true, true, true],      // 8
    [true, true, true, true, false, true, true],     // 9
  ];

  if digit >= 10 {
    return;
  }

  let segments = SEGMENTS[digit];

  // draw each segment...
  if segments[0] {
    // top
    draw_rect(buffer, width, height, (x, y), 5, 1, color);
  }
  if segments[1] {
    // top left
    draw_rect(buffer, width, height, (x, y), 1, 4, color);
  }
  if segments[2] {
    // top right
    draw_rect(buffer, width, height, (x + 4, y), 1, 4, color);
  }
  if segments[3] {
    // middle
    draw_rect(buffer, width, height, (x, y + 4), 5, 1, color);
  }
  if segments[4] {
    // bottom left
    draw_rect(buffer, width, height, (x, y + 4), 1, 4, color);
  }
  if segments[5] {
    // bottom right
    draw_rect(buffer, width, height, (x + 4, y + 4), 1, 4, color);
  }
  if segments[6] {
    // bottom
    draw_rect(buffer, width, height, (x, y + 8), 5, 1, color);
  }
}
