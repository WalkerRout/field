use std::sync::Arc;

use apodize::hanning_iter;

use realfft::{RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;

use crate::audio::AudioConfig;

struct BandInfo {
  bin_low: usize,
  bin_high: usize,
  compensation: f32,
}

pub struct AudioProcessor {
  config: AudioConfig,
  fft: Arc<dyn RealToComplex<f32>>,
  window_function: Vec<f32>,
  // real input buffer for fft
  fft_real_input: Vec<f32>,
  // complex output of fft (length = fft_size/2+1)
  fft_complex: Vec<Complex<f32>>,
  // scratch buffer used by the fft
  fft_scratch: Vec<Complex<f32>>,
  // processed magnitudes (length = fft_size/2)
  fft_output: Vec<f32>,
  smoothed_fft: Vec<f32>,
  band_mapping: Vec<BandInfo>,
  // last sampled rate, used to detect changes and trigger band recalculation
  sample_rate: f32,
  // precomputed normalization factor (1/sqrt(N))
  norm_factor: f32,
  // precomputed gain and gamma combined
  gain_gamma: f32,
}

impl AudioProcessor {
  /// alpha and beta for magnitude approximation...
  /// α = 2*cos(π/8)/(1+cos(π/8)) = 0.96043387
  /// β = 2*sin(π/8)/(1+cos(π/8)) = 0.39782473
  const MAG_ALPHA: f32 = 0.96043387;
  const MAG_BETA: f32 = 0.39782473;

  pub fn new(config: AudioConfig) -> Self {
    let mut planner = RealFftPlanner::<f32>::new();
    let r2c = planner.plan_fft_forward(config.fft_size);

    let window_function: Vec<f32> = hanning_iter(config.fft_size).map(|v| v as f32).collect();

    // allocate fft buffers once
    let fft_real_input = r2c.make_input_vec();
    let fft_complex = r2c.make_output_vec();
    let fft_scratch = r2c.make_scratch_vec();

    // one-time precompute constants
    let norm_factor = 1.0 / (config.fft_size as f32).sqrt();
    let gain_gamma = 8.0f32.powf(0.6);

    let fft_size = config.fft_size;
    let bar_count = config.bar_count;
    AudioProcessor {
      config,
      fft: r2c,
      window_function,
      fft_real_input,
      fft_complex,
      fft_scratch,
      fft_output: vec![0.0; fft_size / 2],
      smoothed_fft: vec![0.0; bar_count],
      band_mapping: Vec::with_capacity(bar_count),
      sample_rate: 0.0,
      norm_factor,
      gain_gamma,
    }
  }

  /// Process a block of samples at the given sample rate
  pub fn process(&mut self, samples: &[f32], sample_rate: f32) {
    // If the rate changed, rebuild our band map
    if (sample_rate - self.sample_rate).abs() > f32::EPSILON {
      self.sample_rate = sample_rate;
      self.precalculate_bands(sample_rate);
    }

    // nothing to do, just decay and finish up...
    if samples.is_empty() {
      self.decay();
      return;
    }

    let fft_size = self.config.fft_size;
    let half = fft_size / 2;
    let count = fft_size.min(samples.len());

    // zero-padded, windowed and dc-removed input
    // zero fill real buffer...
    self.fft_real_input.fill(0.0);

    // compute dc mean
    let mut sum = 0.0f32;
    for &s in &samples[..count] {
      sum += s;
    }
    let mean = sum / (count as f32);

    // apply window and dc removal
    self
      .fft_real_input
      .iter_mut()
      .zip(self.window_function.iter())
      .zip(samples.iter().take(count))
      .for_each(|((out, w), s)| {
        *out = (s - mean) * w;
      });

    // fft, use scratch for performance...
    self
      .fft
      .process_with_scratch(
        &mut self.fft_real_input,
        &mut self.fft_complex,
        &mut self.fft_scratch,
      )
      .expect("fft forward failed");

    // magnitude and scaling
    for i in 0..half {
      let c = &self.fft_complex[i];
      let re = c.re.abs();
      let im = c.im.abs();
      // https://en.wikipedia.org/wiki/Alpha_max_plus_beta_min_algorithm
      let mag_approx = Self::MAG_ALPHA * re.max(im) + Self::MAG_BETA * re.min(im);
      // apply normalization and gain/gamma in one go
      let scaled = (mag_approx * self.norm_factor * self.gain_gamma).min(1.0);
      self.fft_output[i] = scaled;
    }

    // update groupings
    self.update_bands();
  }

  fn precalculate_bands(&mut self, sample_rate: f32) {
    self.band_mapping.clear();

    const F_MIN: f32 = 20.0;
    const F_MAX: f32 = 20_000.0;

    let fft_size = self.config.fft_size;

    for i in 0..self.config.bar_count {
      let frac = i as f32 / (self.config.bar_count - 1) as f32;
      let freq_center = F_MIN * (F_MAX / F_MIN).powf(frac);
      let bandwidth_factor = 0.3 + 0.7 * frac;
      let freq_low = freq_center / (1.0 + bandwidth_factor);
      let freq_high = freq_center * (1.0 + bandwidth_factor);

      let bin_low = ((freq_low * fft_size as f32) / sample_rate).round() as usize;
      let bin_high = ((freq_high * fft_size as f32) / sample_rate).round() as usize;
      let bin_low = bin_low.min(fft_size / 2 - 2);
      let bin_high = bin_high.max(bin_low + 1).min(fft_size / 2 - 1);

      let compensation = match i {
        0..=7 => 0.2 + 0.3 * (i as f32 / 8.0),
        8..=31 => 0.5 + 0.5 * ((i - 8) as f32 / 24.0),
        _ => 1.0 + 1.0 * ((i - 32) as f32 / 32.0),
      };

      self.band_mapping.push(BandInfo {
        bin_low,
        bin_high,
        compensation,
      });
    }
  }

  fn update_bands(&mut self) {
    const SMOOTH_FACTOR: f32 = 0.85;
    const ATTACK_FACTOR: f32 = 0.15;

    for (i, band) in self.band_mapping.iter().enumerate() {
      let avg = if band.bin_high > band.bin_low {
        let sum: f32 = self.fft_output[band.bin_low..=band.bin_high].iter().sum();
        sum / (band.bin_high - band.bin_low + 1) as f32
      } else {
        0.0
      };
      let compensated = (avg * band.compensation).min(1.0);
      self.smoothed_fft[i] = self.smoothed_fft[i] * SMOOTH_FACTOR + compensated * ATTACK_FACTOR;
    }
  }

  fn decay(&mut self) {
    const DECAY_FACTOR: f32 = 0.95;
    for val in &mut self.smoothed_fft {
      *val *= DECAY_FACTOR;
    }
  }

  pub fn spectrum(&self) -> &[f32] {
    &self.smoothed_fft
  }

  pub fn fft_output(&self) -> &[f32] {
    &self.fft_output
  }
}
