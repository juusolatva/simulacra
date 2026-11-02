//! Per-style pixel shaders. Each mirrors the matching `render_*` function in
//! `abstract_art.py` expression for expression; Python generates the scene
//! (all random draws) and passes it in as a dict, extracted into these structs.

pub mod drift;
pub mod fields;
pub mod quasar;
pub mod shapes;

use crate::palette::Palette;

/// A style whose scene has been extracted from Python and can be shaded.
pub trait Style: Send + Sync {
    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8>;
}

/// `(width - 1) / 2.0`, the pixel-center origin most styles use.
#[inline]
pub fn center(width: usize, height: usize) -> (f64, f64) {
    ((width as f64 - 1.0) / 2.0, (height as f64 - 1.0) / 2.0)
}

/// `max(1.0, min(width, height))` as Python computes it.
#[inline]
pub fn small_side(width: usize, height: usize) -> f64 {
    (width.min(height) as f64).max(1.0)
}

/// Gaussian falloff `exp(-((d / sigma) ** 2) / 2.0)` in Python's operation order.
#[inline]
pub fn gaussian(d: f64, sigma: f64) -> f64 {
    let q = d / sigma;
    (-(q * q) / 2.0).exp()
}

/// Python's `min(1.0, max(0.0, t))`.
#[inline]
#[allow(
    clippy::manual_clamp,
    reason = "clamp() keeps NaN; Python's max(0.0, nan) is 0.0"
)]
pub fn unit(t: f64) -> f64 {
    t.max(0.0).min(1.0)
}
