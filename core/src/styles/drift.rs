//! `render_drift`: noise sampled through a current-warped coordinate grid.

use crate::noise::{contrast, fbm};
use crate::palette::Palette;
use crate::raster::shade;

/// `render_drift` (no random scene).
pub struct Drift;

impl super::Style for Drift {
    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        // Noise coordinates scale with the canvas so the composition is size-independent.
        let k = 32.0 / width.max(height) as f64;
        shade(width, height, palette, |x, y| {
            let (u, w) = (x as f64 * k, y as f64 * k);
            // current offsets from low-frequency turbulence
            let ox = (fbm(u * 0.55, w * 0.55, seed, 3) - 0.5) * 24.0;
            let oy = (fbm(u * 0.5 + 2.1, w * 0.5 + 5.7, seed, 2) - 0.5) * 18.0;
            // sample the value field displaced along the current
            contrast(fbm(u + ox, w + oy, seed, 4), 2.6)
        })
    }
}
