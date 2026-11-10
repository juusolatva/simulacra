//! `render_drift`: noise sampled through a current-warped coordinate grid.

use crate::noise::{contrast, fbm_loop};
use crate::palette::Palette;
use crate::raster::shade;
use crate::scenes::{BuildScene, SceneParams};
use crate::validate::{Canvas, SceneError, check_loop_phase};

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Drift {
    phase: f64,
}

impl BuildScene for Drift {
    /// `_drift_scene`: the noise only needs the phase.
    fn build(params: &SceneParams<'_>) -> Self {
        Self {
            phase: params.phase,
        }
    }
}

impl super::Style for Drift {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_loop_phase(self.phase)?;
        Ok(canvas.per_pixel(0.0))
    }

    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        // Noise coordinates scale with the canvas so the composition is size-independent.
        let k = 32.0 / width.max(height) as f64;
        shade(width, height, palette, |x, y| {
            let (u, w) = (x as f64 * k, y as f64 * k);
            // current offsets from low-frequency turbulence
            let ox = (fbm_loop(u * 0.55, w * 0.55, seed, 3, self.phase) - 0.5) * 24.0;
            let oy = (fbm_loop(u * 0.5 + 2.1, w * 0.5 + 5.7, seed, 2, self.phase) - 0.5) * 18.0;
            // sample the value field displaced along the current
            contrast(fbm_loop(u + ox, w + oy, seed, 4, self.phase), 2.6)
        })
    }
}
