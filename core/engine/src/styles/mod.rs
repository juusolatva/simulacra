//! Per-style pixel shaders. Each mirrors the matching `render_*` function in
//! `simulacra.py` expression for expression. Python generates the scene (all
//! random draws) as a dict; with the `python` feature the `py` adapter extracts
//! it into these structs.

pub mod brush;
pub mod drift;
pub mod fields;
pub mod mosaic;
pub mod quasar;
pub mod shapes;
pub mod space;
pub mod woodblock;

use crate::geometry::py_mod;
use crate::math::Libm;
use crate::palette::Palette;
use crate::validate::{Canvas, SceneError};

/// A style's scene, ready to shade.
pub trait Style: Send + Sync {
    /// Shades the scene without checking it: an inconsistent scene can panic
    /// or run for a very long time. [`render`](crate::render) validates the
    /// scene first; call this directly only on a scene that passed
    /// [`validate`](Self::validate) for the same canvas and palette.
    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8>;

    /// Checks the scene against the canvas and palette (see
    /// [`validate`](crate::validate)) and estimates the work rendering it
    /// takes, in inner loop iterations; [`validate_scene`](crate::validate_scene)
    /// compares that with [`work_limit`](crate::work_limit).
    ///
    /// # Errors
    ///
    /// [`SceneError`] for a number that is not finite or too large, an index
    /// or length that does not fit, or a value outside the style's range.
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError>;
}

/// Python's `_loop(phase, cycles)`: `tau * cycles * (phase % 1.0)`, an angle that
/// makes `cycles` whole turns as `phase` runs from 0 to 1 (exactly 0.0 at phase 0).
#[inline]
pub(crate) fn loop_angle(phase: f64, cycles: i32) -> f64 {
    std::f64::consts::TAU * f64::from(cycles) * py_mod(phase, 1.0)
}

/// Python's `6.28`, the upper bound many builders draw a phase from (it is not
/// tau, and the scenes must match it).
#[allow(
    clippy::approx_constant,
    reason = "Python draws uniform(0, 6.28), not tau"
)]
pub(crate) const ROUGH_TAU: f64 = 6.28;

/// The golden angle in radians, as Python's `_circle_offset` writes it.
const GOLDEN_ANGLE: f64 = 2.399_963_229_728_653;

/// Python's `index * 2.399963229728653`: where item `index` starts on its
/// circle (in `_circle_offset`, `_drift_stars` and the ring breathing of
/// `orbits`).
#[inline]
pub(crate) fn golden_start(index: usize) -> f64 {
    index as f64 * GOLDEN_ANGLE
}

/// Python's `_circle_offset(index, reach, phase)`: where item `index` sits on a
/// circle of radius `reach` as `phase` runs from 0 to 1, measured from the
/// circle's starting point, so phase 0 gives exactly `(0.0, 0.0)`.
#[inline]
pub(crate) fn circle_offset(index: usize, reach: f64, phase: f64) -> (f64, f64) {
    let start = golden_start(index);
    let angle = start + loop_angle(phase, 1);
    (
        reach * (angle.m_cos() - start.m_cos()),
        reach * (angle.m_sin() - start.m_sin()),
    )
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
    (-(q * q) / 2.0).m_exp()
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

/// Python's `_drift_stars(stars, reach, phase)`: each `(x, y, radius,
/// brightness)` star circles with [`circle_offset`] and twinkles once per
/// loop; phase 0 changes nothing.
pub(crate) fn drift_stars(
    stars: Vec<(f64, f64, f64, f64)>,
    reach: f64,
    phase: f64,
) -> Vec<(f64, f64, f64, f64)> {
    stars
        .into_iter()
        .enumerate()
        .map(|(index, (x, y, radius, brightness))| {
            let (dx, dy) = circle_offset(index, reach, phase);
            let start = golden_start(index);
            let twinkle = 1.0 + 0.25 * ((start + loop_angle(phase, 1)).m_sin() - start.m_sin());
            (x + dx, y + dy, radius, brightness * twinkle)
        })
        .collect()
}

/// Python's `(width - 1)` converted to a float: the subtraction is exact
/// before the single rounding, as with Python's unbounded ints.
#[inline]
pub(crate) fn last_index(side: usize) -> f64 {
    (side as i128 - 1) as f64
}
