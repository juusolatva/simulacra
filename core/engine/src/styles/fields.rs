//! Noise- and wave-based styles that compute each pixel independently.

use crate::math::{Libm, py_hypot};
use std::f64::consts::{PI, TAU};

use rayon::prelude::*;

use super::{
    ROUGH_TAU, Style, center, circle_offset, drift_stars, gaussian, golden_start, loop_angle,
    small_side, unit,
};
use crate::geometry::py_mod;
use crate::geometry::py_sum;
use crate::noise::{contrast, fbm, fbm_loop};
use crate::palette::Palette;
use crate::random::PyRandom;
use crate::raster::{shade, shade_rgb};
use crate::scenes::{BuildScene, SceneParams};
use crate::validate::{Canvas, SceneError, check_index, check_loop_phase, check_numbers};

/// Python's `math.sqrt(width * width + height * height)`: the integers are
/// squared and added exactly and rounded to a float once, as Python converts
/// its unbounded int (round half to even).
pub(crate) fn diagonal(width: usize, height: usize) -> f64 {
    let (w, h) = (width as u128, height as u128);
    // Each square fits in u128 (a usize has at most 64 bits); their sum can
    // need 129 bits.
    let (w2, h2) = (w * w, h * h);
    let sum = match w2.checked_add(h2) {
        Some(sum) => sum as f64,
        None => {
            // sum = 2 * half + low. Halve without losing anything that affects
            // rounding: `half` has at least 127 significant bits, so OR-ing the
            // dropped bit into its lowest bit keeps it as a sticky bit far below
            // the 53 that survive, and doubling the float is exact.
            let half = (w2 >> 1) + (h2 >> 1) + ((w2 & 1) & (h2 & 1));
            let low = (w2 ^ h2) & 1;
            (half | low) as f64 * 2.0
        }
    };
    sum.sqrt()
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Blobs {
    phase: f64,
}

impl BuildScene for Blobs {
    /// `_blobs_scene`: the noise only needs the phase.
    fn build(params: &SceneParams<'_>) -> Self {
        Self {
            phase: params.phase,
        }
    }
}

impl Style for Blobs {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_loop_phase(self.phase)?;
        Ok(canvas.per_pixel(0.0))
    }

    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        let k = 40.0 / width.max(height) as f64;
        shade(width, height, palette, |x, y| {
            contrast(
                fbm_loop(x as f64 * k, y as f64 * k, seed, 5, self.phase),
                2.2,
            )
        })
    }
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Waves {
    layers: Vec<(f64, f64, f64)>,
}

impl BuildScene for Waves {
    /// `_waves_scene`: four layers of (amplitude, frequency, phase), the phases
    /// drawn from `random.Random(seed)` and turned by the animation phase.
    fn build(params: &SceneParams<'_>) -> Self {
        let phase = params.phase;
        let mut rng = PyRandom::new(params.seed);
        let layers = vec![
            (
                30.0,
                2.0,
                rng.uniform(0.0, ROUGH_TAU) + loop_angle(phase, 1),
            ),
            (
                18.0,
                3.5,
                rng.uniform(0.0, ROUGH_TAU) + loop_angle(phase, -1),
            ),
            (
                12.0,
                5.0,
                rng.uniform(0.0, ROUGH_TAU) + loop_angle(phase, 2),
            ),
            (
                7.0,
                7.0,
                rng.uniform(0.0, ROUGH_TAU) + loop_angle(phase, -2),
            ),
        ];
        Self { layers }
    }
}

impl Style for Waves {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, layers);
        Ok(canvas.per_pixel(self.layers.len() as f64))
    }

    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        let size = width.max(height) as f64;
        shade(width, height, palette, |x, y| {
            let (x, y) = (x as f64, y as f64);
            let n = fbm(x * 0.015, y * 0.015, seed, 3);
            let mut v = 0.1 + n * 0.5;
            for &(amp, freq, phase) in &self.layers {
                let angle = (x + y * 0.9) * freq * TAU / size + phase;
                v += amp / 67.0 * angle.m_sin() * (0.3 + n);
            }
            unit(v * 1.4)
        })
    }
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Plasma {
    layers: Vec<(f64, f64, f64, f64)>,
}

impl BuildScene for Plasma {
    /// `_plasma_scene`: four plane waves in random directions, well below Nyquist.
    fn build(params: &SceneParams<'_>) -> Self {
        let mut rng = PyRandom::new(params.seed);
        let size = params.width.max(params.height) as f64;
        // (amplitude, cycles across the canvas, turns per loop)
        let layers = [
            (1.0, 1.5, 1),
            (0.8, 2.5, -1),
            (0.6, 4.0, 2),
            (0.45, 6.5, -1),
        ]
        .into_iter()
        .map(|(amp, cycles, turns)| {
            let direction = rng.uniform(0.0, TAU);
            let k = cycles * TAU / size;
            let phase = rng.uniform(0.0, ROUGH_TAU) + loop_angle(params.phase, turns);
            (amp, k * direction.m_cos(), k * direction.m_sin(), phase)
        })
        .collect();
        Self { layers }
    }
}

impl Style for Plasma {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, layers);
        Ok(canvas.per_pixel(self.layers.len() as f64))
    }

    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        shade(width, height, palette, |x, y| {
            let (x, y) = (x as f64, y as f64);
            let mut v = 0.0;
            let n = fbm(x * 0.01, y * 0.01, seed, 4);
            for &(amp, kx, ky, phase) in &self.layers {
                v += amp * (x * kx + y * ky + phase).m_sin();
            }
            0.5 + 0.5 * (v * 1.5 + n * 3.0).m_sin()
        })
    }
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Interference {
    layers: Vec<(f64, f64, f64, f64)>,
}

impl BuildScene for Interference {
    /// `_interference_scene`: three plane waves, each in a random direction.
    fn build(params: &SceneParams<'_>) -> Self {
        let mut rng = PyRandom::new(params.seed);
        let size = params.width.max(params.height) as f64;
        let layers = [1, -1, 2]
            .into_iter()
            .map(|turns| {
                let direction = rng.uniform(0.0, PI);
                // Spatial frequency in cycles across the canvas, far below Nyquist.
                let k = rng.uniform(4.0, 18.0) * TAU / size;
                let phase = rng.uniform(0.0, ROUGH_TAU) + loop_angle(params.phase, turns);
                let amp = rng.uniform(1.0, 4.0);
                (phase, k * direction.m_cos(), k * direction.m_sin(), amp)
            })
            .collect();
        Self { layers }
    }
}

impl Style for Interference {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, layers);
        Ok(canvas.per_pixel(self.layers.len() as f64))
    }

    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        shade(width, height, palette, |x, y| {
            let (x, y) = (x as f64, y as f64);
            let mut v = 0.0;
            for &(phase, kx, ky, amp) in &self.layers {
                v += (x * kx + y * ky + phase).m_sin() * amp;
            }
            let n = fbm(x * 0.006, y * 0.006, seed, 2);
            v += n * 2.0;
            (v * 0.35).m_sin() * 0.5 + 0.5 + (n - 0.5) * 0.3
        })
    }
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Kaleidoscope {
    folds: i64,
    phase: f64,
}

impl BuildScene for Kaleidoscope {
    /// `_kaleidoscope_scene`: the fold count and a turning phase.
    fn build(params: &SceneParams<'_>) -> Self {
        let mut rng = PyRandom::new(params.seed);
        let folds = rng.randint(4, 8);
        let phase = rng.uniform(0.0, TAU) + loop_angle(params.phase, 1);
        Self { folds, phase }
    }
}

impl Style for Kaleidoscope {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, phase);
        Ok(canvas.per_pixel(0.0))
    }

    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        let (cx, cy) = center(width, height);
        let scale = (width.min(height) as f64 / 2.0).max(1.0);
        let folds = self.folds as f64;
        let sector = TAU / folds;
        shade(width, height, palette, |x, y| {
            let dx = (x as f64 - cx) / scale;
            let dy = (y as f64 - cy) / scale;
            let r = py_hypot(dx, dy);
            let angle = (py_mod(dy.m_atan2(dx) + sector / 2.0, sector) - sector / 2.0).abs();
            let (u, v) = (r * angle.m_cos(), r * angle.m_sin());
            let n = fbm(u * 32.0, v * 32.0, seed, 3);
            let t = 0.5 + 0.5 * (r * 18.0 + angle * folds * 2.0 + n * 4.0 + self.phase).m_sin();
            t * (-0.35 * r * r).m_exp()
        })
    }
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Pinwheel {
    n_arms: i64,
    twist: f64,
    spin: f64,
    phase: f64,
}

impl BuildScene for Pinwheel {
    /// `_pinwheel_scene`: arm count and twist; the arms spin once per loop.
    fn build(params: &SceneParams<'_>) -> Self {
        let mut rng = PyRandom::new(params.seed);
        let n_arms = rng.randint(3, 5);
        let twist = rng.uniform(2.0, 7.0);
        Self {
            n_arms,
            twist,
            spin: loop_angle(params.phase, 1),
            phase: params.phase,
        }
    }
}

impl Style for Pinwheel {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, twist, spin);
        check_loop_phase(self.phase)?;
        Ok(canvas.per_pixel(0.0))
    }

    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        let (half_w, half_h) = (width as f64 / 2.0, height as f64 / 2.0);
        let max_r = (half_w * half_w + half_h * half_h).sqrt() * 1.05;
        let arms = self.n_arms as f64;
        shade(width, height, palette, |x, y| {
            let dx = x as f64 - half_w;
            let dy = y as f64 - half_h;
            let r = (dx * dx + dy * dy).sqrt();
            let theta = if dx != 0.0 || dy != 0.0 {
                dy.m_atan2(dx)
            } else {
                0.0
            };
            let t = unit(r / max_r);
            let arm_phase = theta * arms + t * self.twist + self.spin;
            let d = (py_mod(arm_phase + PI, 2.0 * PI) - PI).abs();
            let w = gaussian(d, 0.35 * (1.0 - t));
            let n = fbm_loop(x as f64 * 0.006, y as f64 * 0.006, seed, 2, self.phase);
            unit((w * (1.0 - 0.35 * t) + n * 0.15) / 1.2)
        })
    }
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Galaxy {
    bulge_r: f64,
    bulge_amp: f64,
    /// (r0, width, phase, pitch, amp)
    arms: Vec<(f64, f64, f64, f64, f64)>,
}

impl BuildScene for Galaxy {
    /// `_galaxy_scene`: a bulge and two or three log-spiral arms that turn.
    fn build(params: &SceneParams<'_>) -> Self {
        let mut rng = PyRandom::new(params.seed);
        let (cx, cy) = center(params.width, params.height);
        let max_r = py_hypot(cx, cy).max(1.0);
        let bulge_r = max_r * rng.uniform(0.10, 0.26);
        let bulge_amp = rng.uniform(0.9, 1.5);
        let count = rng.randint(2, 3);
        let arms = (0..count)
            .map(|_| {
                let r0 = max_r * rng.uniform(0.015, 0.05);
                let arm_width = rng.uniform(0.08, 0.18);
                let arm_phase = rng.uniform(0.0, ROUGH_TAU) + loop_angle(params.phase, 1);
                let pitch = rng.uniform(1.5, 3.5);
                let amp = rng.uniform(0.5, 1.0);
                (r0, arm_width, arm_phase, pitch, amp)
            })
            .collect();
        Self {
            bulge_r,
            bulge_amp,
            arms,
        }
    }
}

impl Style for Galaxy {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, bulge_r, bulge_amp, arms);
        Ok(canvas.per_pixel(self.arms.len() as f64))
    }

    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        let (cx, cy) = center(width, height);
        let max_r = py_hypot(cx, cy).max(1.0);
        shade(width, height, palette, |x, y| {
            let dx = x as f64 - cx;
            let dy = y as f64 - cy;
            let r = (dx * dx + dy * dy).sqrt();
            let theta = dy.m_atan2(dx);
            let n = fbm(x as f64 * 0.0035, y as f64 * 0.0035, seed, 2);
            let q = r / self.bulge_r;
            let mut v = (-0.5 * (q * q)).m_exp() * self.bulge_amp * (0.8 + 0.4 * n);
            for &(r0, arm_width, phase, pitch, amp) in &self.arms {
                let arm_theta = phase + (r / r0).m_ln_1p() * pitch;
                let d = (py_mod(arm_theta - theta + PI, TAU) - PI).abs();
                let q = d / arm_width;
                v += (-0.5 * (q * q) - r / max_r).m_exp() * amp;
            }
            (1.0 - (-v).m_exp()) * 0.95
        })
    }
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Bloom {
    /// (x, y, radius, amp)
    blooms: Vec<(f64, f64, f64, f64)>,
    phase: f64,
}

impl BuildScene for Bloom {
    /// `_bloom_scene`: 4 to 9 glows, each circling its home position.
    fn build(params: &SceneParams<'_>) -> Self {
        let (width, height, phase) = (params.width, params.height, params.phase);
        let mut rng = PyRandom::new(params.seed);
        let scale = small_side(width, height);
        let count = rng.randint(4, 9);
        let blooms = (0..count as usize)
            .map(|i| {
                let bx = rng.uniform(0.0, width as f64 - 1.0);
                let by = rng.uniform(0.0, height as f64 - 1.0);
                let br = scale * rng.uniform(0.08, 0.27);
                let amp = rng.uniform(0.4, 1.0);
                let (dx, dy) = circle_offset(i, scale * 0.05, phase);
                (bx + dx, by + dy, br, amp)
            })
            .collect();
        Self { blooms, phase }
    }
}

impl Style for Bloom {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, blooms);
        check_loop_phase(self.phase)?;
        Ok(canvas.per_pixel(self.blooms.len() as f64))
    }

    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        shade(width, height, palette, |x, y| {
            let (x, y) = (x as f64, y as f64);
            let mut v = 0.0;
            let n = fbm_loop(x * 0.006, y * 0.006, seed, 3, self.phase);
            for &(bx, by, br, amp) in &self.blooms {
                v += gaussian(py_hypot(x - bx, y - by), br) * amp;
            }
            v *= 0.4 + 1.2 * n;
            1.0 - (-1.4 * v).m_exp()
        })
    }
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Nebula {
    /// (x, y, radius, amp)
    clouds: Vec<(f64, f64, f64, f64)>,
    /// (x, y, radius)
    stars: Vec<(f64, f64, f64)>,
    phase: f64,
}

impl BuildScene for Nebula {
    /// `_nebula_scene`: 3 to 6 drifting clouds and 40 fixed stars.
    fn build(params: &SceneParams<'_>) -> Self {
        let (width, height, phase) = (params.width, params.height, params.phase);
        let mut rng = PyRandom::new(params.seed);
        let (w, h) = (width as f64, height as f64);
        let diag = diagonal(width, height);
        let count = rng.randint(3, 6);
        let clouds = (0..count as usize)
            .map(|i| {
                let ccx = rng.uniform(w * 0.1, w * 0.9);
                let ccy = rng.uniform(h * 0.1, h * 0.9);
                let cr = rng.uniform(0.08, 0.25) * diag;
                let amp = rng.uniform(0.5, 1.5);
                let (dx, dy) = circle_offset(i, diag * 0.03, phase);
                (ccx + dx, ccy + dy, cr, amp)
            })
            .collect();
        let stars = (0..40)
            .map(|_| {
                let x = rng.uniform(0.0, w - 1.0);
                let y = rng.uniform(0.0, h - 1.0);
                (x, y, rng.uniform(0.5, 2.0))
            })
            .collect();
        Self {
            clouds,
            stars,
            phase,
        }
    }
}

impl Style for Nebula {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, clouds, stars);
        check_loop_phase(self.phase)?;
        Ok(canvas.per_pixel(self.clouds.len() as f64 + self.stars.len() as f64))
    }

    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        let noise_k = 24.0 / width.max(height) as f64;
        shade(width, height, palette, |x, y| {
            let (x, y) = (x as f64, y as f64);
            let mut v = 0.0;
            for &(ccx, ccy, cr, amp) in &self.clouds {
                let d2 = (x - ccx) * (x - ccx) + (y - ccy) * (y - ccy);
                v += (-d2 / (2.0 * (cr * cr))).m_exp() * amp;
            }
            let n = fbm_loop(x * noise_k, y * noise_k, seed, 4, self.phase);
            v *= (0.2 + 1.6 * (contrast(n, 2.0) - 0.2)).max(0.0);
            if self
                .stars
                .iter()
                .any(|&(sx, sy, sr)| py_hypot(x - sx, y - sy) < sr)
            {
                v += 1.2;
            }
            1.0 - (-1.3 * v).m_exp()
        })
    }
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Orbits {
    /// (x, y, radius)
    stars: Vec<(f64, f64, f64)>,
    /// (radius, palette stop index)
    rings: Vec<(f64, usize)>,
}

impl BuildScene for Orbits {
    /// `_orbits_scene`: 60 circling stars and 6 breathing ring bands. The ring
    /// colour draw, `randrange(n_stops)`, depends on the palette size.
    fn build(params: &SceneParams<'_>) -> Self {
        let (width, height, phase) = (params.width, params.height, params.phase);
        let mut rng = PyRandom::new(params.seed);
        let (w, h) = (width as f64, height as f64);
        let reach = 0.03 * width.min(height) as f64;
        let stars = (0..60)
            .map(|index| {
                let x = rng.uniform(0.0, w - 1.0);
                let y = rng.uniform(0.0, h - 1.0);
                let (dx, dy) = circle_offset(index, reach, phase);
                (x + dx, y + dy, rng.uniform(0.5, 3.0))
            })
            .collect();
        // Python's `x ** 2` on a float is libm pow.
        let max_r = ((w / 2.0).m_powf(2.0) + (h / 2.0).m_powf(2.0)).sqrt();
        let n_stops = params.palette.stop_count() as i64;
        let rings = (0..6)
            .map(|index| {
                let radius = max_r * rng.uniform(0.15, 0.9);
                let start = golden_start(index);
                let breath = 1.0 + 0.05 * ((start + loop_angle(phase, 1)).m_sin() - start.m_sin());
                // randrange(n) is below n, which is a usize.
                (radius * breath, rng.randrange(n_stops) as usize)
            })
            .collect();
        Self { stars, rings }
    }
}

impl Style for Orbits {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, stars, rings);
        for &(_, stop) in &self.rings {
            check_index("rings (palette stop)", stop, canvas.stops)?;
        }
        Ok(canvas.per_pixel((self.stars.len() + self.rings.len()) as f64))
    }

    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        let (cx, cy) = (width as f64 / 2.0, height as f64 / 2.0);
        shade_rgb(width, height, |x, y| {
            let (x, y) = (x as f64, y as f64);
            let n = fbm(x * 0.008, y * 0.008, seed, 3);
            let mut color = palette.color(contrast(n, 1.6));
            let r = ((x - cx) * (x - cx) + (y - cy) * (y - cy)).sqrt();
            for &(radius, stop) in &self.rings {
                if radius * 0.9 < r && r < radius * 1.1 {
                    color = palette.stop(stop);
                }
            }
            if self
                .stars
                .iter()
                .any(|&(sx, sy, sr)| py_hypot(x - sx, y - sy) < sr * 1.2)
            {
                color = [255, 255, 255];
            }
            color
        })
    }
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Cosmic {
    /// (x, y, size, brightness)
    stars: Vec<(f64, f64, f64, f64)>,
    arm_r: f64,
    arm_ang: f64,
    arm_w: f64,
}

impl Style for Cosmic {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, stars, arm_r, arm_ang, arm_w);
        Ok(canvas.per_pixel(self.stars.len() as f64))
    }

    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        let small = small_side(width, height);
        let (ox, oy) = (width as f64 / 2.0, height as f64 / 2.0);
        let glow_sigma = small * 0.55;
        let arm_sigma = (small * 0.08).max(1.0);
        let star_sigma = (small * 0.02).max(1.0);
        let (arm_cos, arm_sin, arm_tan) = (
            self.arm_ang.m_cos(),
            self.arm_ang.m_sin(),
            self.arm_w.m_tan(),
        );
        shade(width, height, palette, |x, y| {
            let (x, y) = (x as f64, y as f64);
            let rad = py_hypot(x - ox, y - oy);
            let mut v = gaussian(rad, glow_sigma) * 0.6;
            let ang = (y - oy).m_atan2(x - ox);
            let ang_diff = py_mod(ang - self.arm_ang + PI, 2.0 * PI) - PI;
            let dist_along = rad * arm_tan + self.arm_r * (ang_diff / self.arm_w).m_cos();
            let d2 = py_hypot(
                x - (ox + dist_along * arm_cos),
                y - (oy + dist_along * arm_sin),
            );
            v += gaussian(d2, arm_sigma) * 1.2;
            for &(sx, sy, size, brightness) in &self.stars {
                let ds = py_hypot(x - sx, y - sy);
                if ds < star_sigma * 3.0 {
                    v += size * brightness * gaussian(ds, star_sigma);
                }
            }
            1.0 - (-1.2 * v).m_exp()
        })
    }
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Rings {
    /// (radius, phase)
    rings: Vec<(f64, f64)>,
}

impl BuildScene for Rings {
    /// `_rings_scene`: 4 to 9 rings; neighbouring rings turn in opposite directions.
    fn build(params: &SceneParams<'_>) -> Self {
        let mut rng = PyRandom::new(params.seed);
        let scale = (params.width.min(params.height) as f64 / 2.0).max(1.0);
        let count = rng.randint(4, 9);
        let rings = (0..count)
            .map(|i| {
                let radius = scale * rng.uniform(0.15, 0.95);
                let turns = if i % 2 == 0 { 1 } else { -1 };
                (
                    radius,
                    rng.uniform(0.0, TAU) + loop_angle(params.phase, turns),
                )
            })
            .collect();
        Self { rings }
    }
}

impl Style for Rings {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, rings);
        Ok(canvas.per_pixel(self.rings.len() as f64))
    }

    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        let (cx, cy) = center(width, height);
        shade(width, height, palette, |x, y| {
            let (dx, dy) = (x as f64 - cx, y as f64 - cy);
            let r = (dx.powi(2) + dy.powi(2)).sqrt();
            // The last matching ring wins; no ring leaves the first stop (t = 0).
            let mut t = 0.0;
            for &(radius, phase) in &self.rings {
                if radius * 0.9 < r && r < radius * 1.1 {
                    let angle = dy.m_atan2(dx);
                    t = 0.3 + 0.35 * (1.0 + (angle + phase).m_cos());
                }
            }
            t
        })
    }
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Starburst {
    rays: f64,
    phase: f64,
}

impl BuildScene for Starburst {
    /// `_starburst_scene`: the ray count and a turning phase.
    fn build(params: &SceneParams<'_>) -> Self {
        let mut rng = PyRandom::new(params.seed);
        let rays = rng.randint(8, 14) as f64;
        let phase = rng.uniform(0.0, TAU) + loop_angle(params.phase, 1);
        Self { rays, phase }
    }
}

impl Style for Starburst {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, rays, phase);
        Ok(canvas.per_pixel(0.0))
    }

    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        let (cx, cy) = center(width, height);
        let scale = (width.min(height) as f64 / 2.0).max(1.0);
        shade(width, height, palette, |x, y| {
            let (dx, dy) = ((x as f64 - cx) / scale, (y as f64 - cy) / scale);
            let r = py_hypot(dx, dy);
            let theta = dy.m_atan2(dx);
            let ray = (0.5 + 0.5 * (theta * self.rays + self.phase).m_cos()).m_powf(12.0);
            let q = r / 0.18;
            let glow = (-(q * q)).m_exp();
            glow + ray * (-r * 1.3).m_exp() * (1.0 - (-r * 12.0).m_exp())
        })
    }
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Spiral {
    /// (turns, phase, amp, width)
    arms: Vec<(f64, f64, f64, f64)>,
}

impl BuildScene for Spiral {
    /// `_spiral_scene`: 2 to 5 arms of (turns, phase, amplitude, width).
    fn build(params: &SceneParams<'_>) -> Self {
        let mut rng = PyRandom::new(params.seed);
        let count = rng.randint(2, 5);
        let arms = (0..count)
            .map(|_| {
                let turns = rng.uniform(1.0, 4.0);
                let phase = rng.uniform(0.0, ROUGH_TAU) + loop_angle(params.phase, 1);
                let amp = rng.uniform(0.4, 1.0);
                (turns, phase, amp, rng.uniform(0.08, 0.35))
            })
            .collect();
        Self { arms }
    }
}

impl Style for Spiral {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, arms);
        Ok(canvas.per_pixel(self.arms.len() as f64))
    }

    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        let (cx, cy) = center(width, height);
        let max_r = py_hypot(cx, cy).max(1.0);
        shade(width, height, palette, |x, y| {
            let (dx, dy) = (x as f64 - cx, y as f64 - cy);
            let r = (dx * dx + dy * dy).sqrt() / max_r;
            let theta = dy.m_atan2(dx);
            let mut v = 0.0;
            for &(turns, phase, amp, arm_width) in &self.arms {
                let d = py_mod(r * turns * TAU - theta - phase, TAU);
                let dist = (d - PI).abs();
                v += amp * gaussian(dist, arm_width);
            }
            v *= 1.0 - 0.5 * r;
            1.0 - (-1.6 * v).m_exp()
        })
    }
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Petals {
    count: f64,
    length: f64,
    phase: f64,
}

impl BuildScene for Petals {
    /// `_petals_scene`: petal count and length, and a turning phase.
    fn build(params: &SceneParams<'_>) -> Self {
        let mut rng = PyRandom::new(params.seed);
        let count = rng.randint(5, 8) as f64;
        let length = rng.uniform(0.65, 0.95);
        let phase = rng.uniform(0.0, TAU) + loop_angle(params.phase, 1);
        Self {
            count,
            length,
            phase,
        }
    }
}

impl Style for Petals {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, count, length, phase);
        Ok(canvas.per_pixel(0.0))
    }

    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        let (cx, cy) = center(width, height);
        let scale = (width.min(height) as f64 / 2.0).max(1.0);
        shade(width, height, palette, |x, y| {
            let (dx, dy) = ((x as f64 - cx) / scale, (y as f64 - cy) / scale);
            let r = py_hypot(dx, dy);
            let theta = dy.m_atan2(dx);
            let angular = (0.5 + 0.5 * (self.count * theta + self.phase).m_cos()).m_powf(3.0);
            let q = (r - self.length * 0.52) / (self.length * 0.3);
            let radial = (-(q * q)).m_exp();
            let q = r / 0.16;
            angular * radial + (-(q * q)).m_exp()
        })
    }
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Aurora {
    /// (y0, amp, freq, phase, bw)
    ribbons: Vec<(f64, f64, f64, f64, f64)>,
}

impl BuildScene for Aurora {
    /// `_aurora_scene`: 3 to 7 ribbons; neighbouring ones turn in opposite directions.
    fn build(params: &SceneParams<'_>) -> Self {
        let (width, height) = (params.width, params.height);
        let mut rng = PyRandom::new(params.seed);
        let count = rng.randint(3, 7);
        let ribbons = (0..count)
            .map(|i| {
                let y0 = rng.uniform(0.0, height as f64 - 1.0);
                let amp = (height as f64 * rng.uniform(0.06, 0.27)).max(1.0);
                let freq = rng.uniform(1.0, 5.0) / width.max(1) as f64;
                let turns = if i % 2 == 0 { 1 } else { -1 };
                let phase = rng.uniform(0.0, ROUGH_TAU) + loop_angle(params.phase, turns);
                let bw = rng.uniform(0.6, 1.4);
                (y0, amp, freq, phase, bw)
            })
            .collect();
        Self { ribbons }
    }
}

impl Style for Aurora {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, ribbons);
        Ok(canvas.per_pixel(self.ribbons.len() as f64))
    }

    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        shade(width, height, palette, |x, y| {
            let (x, y) = (x as f64, y as f64);
            let mut v = 0.0;
            for &(y0, amp, freq, phase, bw) in &self.ribbons {
                let yy = y0 + amp * (x * freq + phase).m_sin();
                let d = (y - yy) / amp;
                let w = (-d * d).m_exp();
                let s = (x * freq * 2.0 + phase).m_sin();
                v += w * (0.5 + 0.5 * s) * bw;
            }
            0.05 + 0.95 * (1.0 - (-1.5 * v).m_exp())
        })
    }
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Gems {
    /// (cx, cy, radius, rotation)
    gems: Vec<(f64, f64, f64, f64)>,
}

impl BuildScene for Gems {
    /// `_gems_scene`: three gems whose facets turn.
    fn build(params: &SceneParams<'_>) -> Self {
        let (width, height) = (params.width, params.height);
        let mut rng = PyRandom::new(params.seed);
        let small = small_side(width, height);
        let gems = (0..3)
            .map(|_| {
                let cx = rng.uniform(0.0, width as f64 - 1.0);
                let cy = rng.uniform(0.0, height as f64 - 1.0);
                let cr = small * rng.uniform(0.1, 0.25);
                let rotation = rng.uniform(0.0, ROUGH_TAU) + loop_angle(params.phase, 1);
                (cx, cy, cr, rotation)
            })
            .collect();
        Self { gems }
    }
}

impl Style for Gems {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, gems);
        Ok(canvas.per_pixel(self.gems.len() as f64))
    }

    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        shade(width, height, palette, |x, y| {
            let (x, y) = (x as f64, y as f64);
            // The last gem covering the pixel wins; none leaves the first stop (t = 0).
            let mut t = 0.0;
            for &(cx, cy, cr, rot) in &self.gems {
                let (dx, dy) = (x - cx, y - cy);
                let d2 = dx * dx + dy * dy;
                if d2 < cr * cr {
                    let depth = d2.sqrt() / cr;
                    let local = ((rot + depth * 3.0).m_cos() + 1.0) / 2.0;
                    t = unit((1.0 - depth) * local);
                }
            }
            t
        })
    }
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Cosmos {
    /// (x, y, max_r, rings, phase, amp)
    centers: Vec<(f64, f64, f64, i64, f64, f64)>,
}

impl BuildScene for Cosmos {
    /// `_cosmos_scene`: 2 to 5 ring centres whose bands move outward.
    fn build(params: &SceneParams<'_>) -> Self {
        let (width, height) = (params.width, params.height);
        let mut rng = PyRandom::new(params.seed);
        let (w, h) = (width as f64, height as f64);
        let diag = diagonal(width, height);
        let count = rng.randint(2, 5);
        let centers = (0..count)
            .map(|_| {
                let x = rng.uniform(w * 0.2, w * 0.8);
                let y = rng.uniform(h * 0.2, h * 0.8);
                let max_r = rng.uniform(0.25, 0.6) * diag;
                let rings = rng.randint(3, 8);
                let phase = rng.uniform(0.0, ROUGH_TAU) - loop_angle(params.phase, 1);
                let amp = rng.uniform(0.6, 1.4);
                (x, y, max_r, rings, phase, amp)
            })
            .collect();
        Self { centers }
    }
}

impl Style for Cosmos {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, centers);
        if self.centers.iter().any(|center| center.3 < 1) {
            return Err(SceneError::Range {
                field: "centers",
                rule: "has a centre with fewer than one ring",
            });
        }
        Ok(canvas.per_pixel(self.centers.len() as f64))
    }

    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        shade(width, height, palette, |x, y| {
            let (x, y) = (x as f64, y as f64);
            let mut v = 0.0;
            let mut weight = 0.0;
            for &(cx, cy, max_r, rings, phase, amp) in &self.centers {
                let (dx, dy) = (x - cx, y - cy);
                let d = (dx * dx + dy * dy).sqrt();
                if d < max_r {
                    // d >= 0, so truncation matches Python's int().
                    let k = ((d / max_r) * (rings - 1) as f64 + 0.5) as i64;
                    let w = gaussian(d, max_r / (rings - 1).max(1) as f64);
                    let s = (k as f64 * 2.0 + phase).m_sin();
                    v += w * amp * (0.5 + 0.5 * s);
                    weight += w;
                }
            }
            unit(v / (weight + 0.6)) * 0.95
        })
    }
}

/// `render_mandelbrot`: Python gives the view (scale, centre) and iteration limit.
#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Mandelbrot {
    scale: f64,
    center: (f64, f64),
    iterations: u32,
}

/// `_MANDELBROT_HOME`, `_MANDELBROT_TARGET` and `_MANDELBROT_DEPTH` in Python.
const MANDELBROT_HOME: (f64, f64) = (-0.6, 0.0);
const MANDELBROT_TARGET: (f64, f64) = (-0.743_643_887_037_151, 0.131_825_904_205_330);
const MANDELBROT_DEPTH: f64 = 7.0;

impl BuildScene for Mandelbrot {
    /// `_mandelbrot_scene`: a zoom toward `MANDELBROT_TARGET` and back over a
    /// loop, eased at both ends; phase 0 is the whole set.
    fn build(params: &SceneParams<'_>) -> Self {
        let (width, height, phase) = (params.width, params.height, params.phase);
        let ease = 0.5 - 0.5 * loop_angle(phase, 1).m_cos();
        let closing = (-MANDELBROT_DEPTH * ease).m_exp();
        let (home_x, home_y) = MANDELBROT_HOME;
        let (target_x, target_y) = MANDELBROT_TARGET;
        Self {
            scale: 3.2 / width.min(height) as f64 * closing,
            center: (
                home_x + (target_x - home_x) * (1.0 - closing),
                home_y + (target_y - home_y) * (1.0 - closing),
            ),
            // Python's int() truncates; 150 * ease is in [0, 150].
            iterations: 100 + (150.0 * ease) as u32,
        }
    }
}

impl Style for Mandelbrot {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, scale, center);
        Ok(canvas.per_pixel(f64::from(self.iterations)))
    }

    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        let scale = self.scale;
        let (center_x, center_y) = self.center;
        let max_iter = self.iterations;
        let (mid_x, mid_y) = center(width, height);
        shade(width, height, palette, |x, y| {
            let cy = (mid_y - y as f64) * scale + center_y;
            let cx = (x as f64 - mid_x) * scale + center_x;
            let (mut zx, mut zy) = (0.0_f64, 0.0_f64);
            let mut n = 0;
            while zx * zx + zy * zy <= 4.0 && n < max_iter {
                (zx, zy) = (zx * zx - zy * zy + cx, 2.0 * zx * zy + cy);
                n += 1;
            }
            if n == max_iter {
                1.0
            } else {
                (n as f64 / max_iter as f64).sqrt()
            }
        })
    }
}

/// The most layers a `horizon` scene may have; the Python builder draws 4.
const HORIZON_MAX_LAYERS: usize = 64;

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Horizon {
    /// (ridge height fraction, noise offset, palette position) per layer
    layers: Vec<(f64, f64, f64)>,
    phase: f64,
}

impl BuildScene for Horizon {
    /// `_horizon_scene`: four ridges, far to near, each with its own noise offset.
    fn build(params: &SceneParams<'_>) -> Self {
        let mut rng = PyRandom::new(params.seed);
        let layers = [0.35, 0.5, 0.65, 0.8]
            .into_iter()
            .enumerate()
            .map(|(i, fraction)| (fraction, rng.uniform(0.0, 100.0), 0.55 - i as f64 * 0.13))
            .collect();
        Self {
            layers,
            phase: params.phase,
        }
    }
}

impl Style for Horizon {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, layers);
        check_loop_phase(self.phase)?;
        // Each layer holds a row of ridge heights as wide as the canvas.
        if self.layers.len() > HORIZON_MAX_LAYERS {
            return Err(SceneError::Range {
                field: "layers",
                rule: "has more than 64 layers (each needs a canvas-wide row)",
            });
        }
        // One ridge per layer, then every layer per pixel.
        Ok(canvas.per_pixel(2.0 * self.layers.len() as f64))
    }

    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        let h = height as f64;
        let span = (width as f64 - 1.0).max(1.0);
        let ridges: Vec<(Vec<f64>, [u8; 3])> = self
            .layers
            .iter()
            .map(|&(fraction, offset, color)| {
                let ridge = (0..width)
                    .map(|x| {
                        h * (fraction
                            + 0.18
                                * (fbm_loop(x as f64 / span * 40.0, offset, seed, 3, self.phase)
                                    - 0.5))
                    })
                    .collect();
                (ridge, palette.color(color))
            })
            .collect();
        let rows = (height as f64 - 1.0).max(1.0);
        shade_rgb(width, height, |x, y| {
            let mut color = palette.color(0.85 - 0.35 * y as f64 / rows);
            for (ridge, mountain) in &ridges {
                if y as f64 >= ridge[x] {
                    color = *mountain;
                }
            }
            color
        })
    }
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Voronoi {
    /// (x, y, brightness)
    sites: Vec<(f64, f64, f64)>,
}

impl BuildScene for Voronoi {
    /// `_voronoi_scene`: 10 to 16 sites, all drawn first, then each set
    /// circling its home position.
    fn build(params: &SceneParams<'_>) -> Self {
        let (width, height) = (params.width, params.height);
        let mut rng = PyRandom::new(params.seed);
        let count = rng.randint(10, 16);
        let sites: Vec<(f64, f64, f64)> = (0..count)
            .map(|_| {
                let x = rng.uniform(0.0, width as f64 - 1.0);
                let y = rng.uniform(0.0, height as f64 - 1.0);
                (x, y, rng.uniform(0.1, 0.7))
            })
            .collect();
        let reach = small_side(width, height) * 0.06;
        let sites = sites
            .into_iter()
            .enumerate()
            .map(|(i, (x, y, color))| {
                let (dx, dy) = circle_offset(i, reach, params.phase);
                (x + dx, y + dy, color)
            })
            .collect();
        Self { sites }
    }
}

impl Style for Voronoi {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, sites);
        Ok(canvas.per_pixel(self.sites.len() as f64))
    }

    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        let scale = (width.min(height) as f64).max(1.0);
        shade(width, height, palette, |x, y| {
            let (x, y) = (x as f64, y as f64);
            // Python sorts (distance, brightness) tuples and reads the first two
            // distances: track the lexicographic minimum and the second distance.
            let mut nearest = (f64::INFINITY, f64::INFINITY);
            let mut second = f64::INFINITY;
            for &(sx, sy, color) in &self.sites {
                let d = py_hypot(x - sx, y - sy);
                if d < nearest.0 || (d == nearest.0 && color < nearest.1) {
                    second = nearest.0;
                    nearest = (d, color);
                } else if d < second {
                    second = d;
                }
            }
            let gap = second - nearest.0;
            let edge = (-gap / (scale * 0.012).max(0.5)).m_exp();
            nearest.1 * (-nearest.0 / (scale * 0.3)).m_exp() + edge * 0.5
        })
    }
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Crystal {
    light: (f64, f64, f64),
    half: (f64, f64, f64),
    phase: f64,
}

impl BuildScene for Crystal {
    /// `_crystal_scene`: the fixed light and its Blinn-Phong half vector.
    fn build(params: &SceneParams<'_>) -> Self {
        let (lx, ly, lz) = (0.45_f64, -0.55_f64, 0.70_f64);
        // Python's sum() of the squares, which is compensated since 3.12.
        let ln = py_sum([lx * lx, ly * ly, lz * lz]).sqrt();
        let light = (lx / ln, ly / ln, lz / ln);
        let (hx, hy, hz) = (light.0, light.1, light.2 + 1.0); // half vector, viewer at +z
        let hn = (hx * hx + hy * hy + hz * hz).sqrt();
        Self {
            light,
            half: (hx / hn, hy / hn, hz / hn),
            phase: params.phase,
        }
    }
}

impl Style for Crystal {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, light, half);
        check_loop_phase(self.phase)?;
        Ok(canvas.per_pixel(1.0))
    }

    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        let k = 28.0 / width.max(height) as f64;
        let relief = 14.0 / k;
        let tilt_step = PI / 10.0;
        let az_step = PI / 4.0;
        let (light, half) = (self.light, self.half);

        // Height field first: every pixel's gradient reads its neighbours.
        let field: Vec<f64> = (0..width * height)
            .into_par_iter()
            .map(|i| {
                fbm_loop(
                    (i % width) as f64 * k,
                    (i / width) as f64 * k,
                    seed,
                    3,
                    self.phase,
                )
            })
            .collect();
        let at = |x: usize, y: usize| field[y * width + x];

        shade(width, height, palette, |x, y| {
            let (ym, yp) = (y.saturating_sub(1), (y + 1).min(height - 1));
            let (xm, xp) = (x.saturating_sub(1), (x + 1).min(width - 1));
            let gx = if xp != xm {
                (at(xp, y) - at(xm, y)) / (xp - xm) as f64
            } else {
                0.0
            };
            let gy = if yp != ym {
                (at(x, yp) - at(x, ym)) / (yp - ym) as f64
            } else {
                0.0
            };
            // Python's round() is half-to-even.
            let tilt =
                ((py_hypot(gx, gy) * relief).m_atan() / tilt_step).round_ties_even() * tilt_step;
            let azimuth = (gy.m_atan2(gx) / az_step).round_ties_even() * az_step;
            let n = (
                -tilt.m_sin() * azimuth.m_cos(),
                -tilt.m_sin() * azimuth.m_sin(),
                tilt.m_cos(),
            );
            let diffuse = (n.0 * light.0 + n.1 * light.1 + n.2 * light.2).max(0.0);
            let spec = (n.0 * half.0 + n.1 * half.1 + n.2 * half.2)
                .max(0.0)
                .m_powf(24.0);
            0.75 * diffuse * (0.5 + at(x, y)) + 0.5 * spec - 0.2
        })
    }
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Cymatics {
    circular: bool,
    /// (n, m, weight, phase_a, phase_b)
    modes: Vec<(f64, f64, f64, f64, f64)>,
}

impl Style for Cymatics {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, modes);
        Ok(canvas.per_pixel(self.modes.len() as f64))
    }

    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        let (cx, cy) = center(width, height);
        let scale = (width.min(height) as f64 / 2.0).max(1.0);
        shade(width, height, palette, |x, y| {
            let u = (x as f64 - cx) / scale;
            let v = (y as f64 - cy) / scale;
            let mut w = 0.0;
            if self.circular {
                let r = py_hypot(u, v);
                let theta = v.m_atan2(u);
                for &(n, m, weight, phase_a, phase_b) in &self.modes {
                    let radial = (n * PI * r + phase_a).m_cos() / (1.0 + 0.4 * n * r).sqrt();
                    w += weight * radial * (m * theta + phase_b).m_cos();
                }
            } else {
                for &(n, m, weight, phase_a, phase_b) in &self.modes {
                    let a = (n * PI * u + phase_a).m_cos() * (m * PI * v + phase_b).m_cos();
                    let b = (m * PI * u + phase_b).m_cos() * (n * PI * v + phase_a).m_cos();
                    w += weight * (a - b);
                }
            }
            1.0 / (1.0 + (-3.2 * w).m_exp())
        })
    }
}

impl BuildScene for Cymatics {
    /// `_cymatics_scene`: a circular or square plate and three modes, each with
    /// two phases turning at different rates.
    fn build(params: &SceneParams<'_>) -> Self {
        let phase = params.phase;
        let mut rng = PyRandom::new(params.seed);
        let circular = rng.random() < 0.5;
        let modes = (0..3)
            .map(|i| {
                let n = rng.randint(2, 5) as f64;
                let mut m = rng.randint(1, 6) as f64;
                if !circular && n == m {
                    m = n + 1.0;
                }
                let weight = rng.uniform(0.45, 0.75);
                let turns_a = if i % 2 == 0 { 1 } else { -1 };
                let phase_a = rng.uniform(0.0, TAU) + loop_angle(phase, turns_a);
                let turns_b = if i == 1 { -2 } else { 1 };
                let phase_b = rng.uniform(0.0, TAU) + loop_angle(phase, turns_b);
                (n, m, weight, phase_a, phase_b)
            })
            .collect();
        Self { circular, modes }
    }
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Optics {
    /// (kx, ky, wave_phase)
    waves: Vec<(f64, f64, f64)>,
    /// (fkx, fky, ffreq, fphase)
    film: (f64, f64, f64, f64),
    blades: f64,
    spin: f64,
    /// (fx, fy, radius, amp)
    flares: Vec<(f64, f64, f64, f64)>,
}

impl Style for Optics {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, waves, film, blades, spin, flares);
        Ok(canvas.per_pixel(self.waves.len() as f64 + self.flares.len() as f64))
    }

    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        let (cx, cy) = center(width, height);
        let scale = (width.min(height) as f64 / 2.0).max(1.0);
        let (fkx, fky, ffreq, fphase) = self.film;
        shade(width, height, palette, |x, y| {
            let u = (x as f64 - cx) / scale;
            let v = (y as f64 - cy) / scale;
            let mut caustic = 0.0;
            for &(kx, ky, wave_phase) in &self.waves {
                let ridge = 1.0 - (u * kx + v * ky + wave_phase).m_sin().abs();
                caustic += ridge * ridge * ridge;
            }
            caustic *= 0.25;
            let film = 0.5 + 0.5 * ((u * fkx + v * fky + caustic * ffreq) * TAU + fphase).m_cos();
            let mut diffraction = 0.0;
            for &(fx, fy, radius, amp) in &self.flares {
                let dx = u - fx;
                let dy = v - fy;
                let d = py_hypot(dx, dy);
                let spike = 0.5 + 0.5 * (self.blades * dy.m_atan2(dx) + self.spin).m_cos();
                let spike2 = spike * spike;
                let airy = 0.5 + 0.5 * (d * 26.0 - fphase).m_cos();
                let core = gaussian(d, radius * 0.28);
                let halo = gaussian(d, radius) * (0.65 * (spike2 * spike2) + 0.35 * airy);
                diffraction += amp * (core + halo);
            }
            let energy = 0.85 * caustic + 0.32 * film * (0.25 + caustic) + 0.65 * diffraction;
            1.0 - (-1.5 * energy).m_exp()
        })
    }
}

impl BuildScene for Optics {
    /// `_optics_scene`: four caustic waves, a thin film, an aperture's blades
    /// and three circling flares.
    fn build(params: &SceneParams<'_>) -> Self {
        let phase = params.phase;
        let mut rng = PyRandom::new(params.seed);
        let waves = (0..4_i32)
            .map(|i| {
                let angle = f64::from(i) * (PI / 4.0) + rng.uniform(-0.25, 0.25);
                let freq = rng.uniform(2.2, 3.8) * PI;
                let turns = if i % 2 == 0 { 1 } else { -1 };
                let wave_phase = rng.uniform(0.0, TAU) + loop_angle(phase, turns);
                (freq * angle.m_cos(), freq * angle.m_sin(), wave_phase)
            })
            .collect();
        let film = (
            rng.uniform(-2.4, 2.4),
            rng.uniform(-2.4, 2.4),
            rng.uniform(2.6, 4.2),
            rng.uniform(0.0, TAU) + loop_angle(phase, 1),
        );
        let blades = *rng.choice(&[4.0, 6.0, 8.0]);
        let spin = rng.uniform(0.0, TAU) + loop_angle(phase, -1);
        let flares = (0..3)
            .map(|i| {
                let fx = rng.uniform(-0.55, 0.55);
                let fy = rng.uniform(-0.55, 0.55);
                let (dx, dy) = circle_offset(i, 0.14, phase);
                (
                    fx + dx,
                    fy + dy,
                    rng.uniform(0.22, 0.36),
                    rng.uniform(0.55, 0.85),
                )
            })
            .collect();
        Self {
            waves,
            film,
            blades,
            spin,
            flares,
        }
    }
}

const CONTOUR_LEVELS: f64 = 14.0;

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Contour {
    phase: f64,
}

impl Style for Contour {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_loop_phase(self.phase)?;
        Ok(canvas.per_pixel(2.0))
    }

    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        let k = 24.0 / width.max(height) as f64;
        let terrain = |x: f64, y: f64| contrast(fbm_loop(x * k, y * k, seed, 5, self.phase), 2.0);
        let half_base = (0.0015 * width.min(height) as f64).max(0.75);
        shade(width, height, palette, |x, y| {
            let (fx, fy) = (x as f64, y as f64);
            let h0 = terrain(fx, fy);
            let hx = terrain((x + 1) as f64, fy);
            let hy = terrain(fx, (y + 1) as f64);
            let s = h0 * CONTOUR_LEVELS;
            let step = py_hypot(hx * CONTOUR_LEVELS - s, hy * CONTOUR_LEVELS - s) + 1e-9;
            let level = s.floor();
            let frac = s - level;
            let nearest = if frac < 0.5 { level } else { level + 1.0 };
            let major = (nearest as i64).rem_euclid(5) == 0;
            let half = half_base * if major { 2.0 } else { 1.0 };
            let line = unit(1.0 - frac.min(1.0 - frac) / step / half);
            let base = unit(level / CONTOUR_LEVELS);
            let ink = if major { 0.95 } else { 0.78 };
            base * (1.0 - line) + ink * line
        })
    }
}

impl BuildScene for Contour {
    /// `_contour_scene`: only the phase, which the looping terrain noise takes.
    fn build(params: &SceneParams<'_>) -> Self {
        Self {
            phase: params.phase,
        }
    }
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Opart {
    pattern: i64,
    angle: f64,
    freq: f64,
    shift: f64,
    /// (x, y, radius, strength)
    bulges: Vec<(f64, f64, f64, f64)>,
}

impl Style for Opart {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, angle, freq, shift, bulges);
        Ok(canvas.per_pixel(self.bulges.len() as f64))
    }

    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        let (cx, cy) = center(width, height);
        let scale = (width.min(height) as f64 / 2.0).max(1.0);
        shade(width, height, palette, |x, y| {
            let mut u = (x as f64 - cx) / scale;
            let mut v = (y as f64 - cy) / scale;
            let mut glow = 0.0;
            for &(bx, by, radius, strength) in &self.bulges {
                let q = py_hypot(u - bx, v - by) / radius;
                let f = strength * (-(q * q)).m_exp();
                u = bx + (u - bx) * (1.0 - f);
                v = by + (v - by) * (1.0 - f);
                glow += f;
            }
            let along = u * self.angle.m_cos() + v * self.angle.m_sin();
            let across = v * self.angle.m_cos() - u * self.angle.m_sin();
            let wave = match self.pattern {
                0 => (TAU * self.freq * along + self.shift).m_sin(),
                1 => {
                    (TAU * self.freq * 0.5 * along + self.shift).m_sin()
                        * (TAU * self.freq * 0.5 * across - self.shift).m_sin()
                }
                _ => (TAU * self.freq * py_hypot(u, v) + self.shift).m_sin(),
            };
            let steep = if self.pattern == 1 { 8.0 } else { 5.0 };
            let sharp = 1.0 / (1.0 + ((-steep) * wave).m_exp());
            0.04 + 0.8 * sharp + 0.16 * glow.min(1.0)
        })
    }
}

/// Python's `_OPART_PATTERNS`, whose length `_opart_scene` draws below.
const OPART_PATTERNS: i64 = 3;

impl BuildScene for Opart {
    /// `_opart_scene`: one of the stripe, checker and ring patterns, warped by
    /// two or three circling bulges.
    fn build(params: &SceneParams<'_>) -> Self {
        let phase = params.phase;
        let mut rng = PyRandom::new(params.seed);
        let pattern = rng.randrange(OPART_PATTERNS);
        let angle = rng.uniform(0.0, PI);
        let freq = rng.uniform(3.0, 5.0);
        let shift = rng.uniform(0.0, TAU) + loop_angle(phase, 1);
        let count = rng.randint(2, 3);
        let bulges = (0..count as usize)
            .map(|i| {
                let bx = rng.uniform(-0.8, 0.8);
                let by = rng.uniform(-0.8, 0.8);
                let radius = rng.uniform(0.32, 0.6);
                let strength = rng.uniform(0.5, 0.85);
                let (dx, dy) = circle_offset(i, 0.18, phase);
                (bx + dx, by + dy, radius, strength)
            })
            .collect();
        Self {
            pattern,
            angle,
            freq,
            shift,
            bulges,
        }
    }
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Marbling {
    /// (kind, a, b, c, d, e, f): kind 0 is a drop, kind 1 a comb.
    ops: Vec<(f64, f64, f64, f64, f64, f64, f64)>,
    bands: f64,
    angle: f64,
    slide: f64,
}

impl Marbling {
    /// Undo the ops from last to first; `Some(ink)` when the point lies inside a drop.
    fn trace(&self, mut u: f64, mut v: f64) -> (f64, f64, Option<f64>) {
        for &(kind, a, b, c, d, _, _) in self.ops.iter().rev() {
            if kind < 0.5 {
                let (du, dv) = (u - a, v - b);
                let dist2 = du * du + dv * dv;
                if dist2 < c * c {
                    return (u, v, Some(d));
                }
                let keep = (1.0 - c * c / dist2).sqrt();
                u = a + du * keep;
                v = b + dv * keep;
            } else {
                let w = u * -a.m_sin() + v * a.m_cos() - c;
                let near = (w - b * (w / b + 0.5).floor()).abs();
                let shift = d * (-near / (0.25 * b)).m_exp();
                u -= shift * a.m_cos();
                v -= shift * a.m_sin();
            }
        }
        (u, v, None)
    }
}

impl Style for Marbling {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, ops, bands, angle, slide);
        Ok(canvas.per_pixel(self.ops.len() as f64))
    }

    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        let (cx, cy) = center(width, height);
        let scale = (width.min(height) as f64 / 2.0).max(1.0);
        shade(width, height, palette, |x, y| {
            let (u, v, ink) = self.trace((x as f64 - cx) / scale, (y as f64 - cy) / scale);
            if let Some(ink) = ink {
                return ink;
            }
            let band = ((v * self.angle.m_cos() - u * self.angle.m_sin()) * self.bands + self.slide)
                .floor() as i64;
            // (2 * band) mod 5 without overflowing for a band near the i64 limits.
            0.1 + 0.8 * ((band.rem_euclid(5) * 2).rem_euclid(5) as f64) / 4.0
        })
    }
}

impl BuildScene for Cosmic {
    /// `_cosmic_scene`: 40 to 90 drifting, twinkling stars and an arm that
    /// turns once per loop.
    fn build(params: &SceneParams<'_>) -> Self {
        let (w, h) = (params.width as f64, params.height as f64);
        let mut rng = PyRandom::new(params.seed);
        let count = rng.randint(40, 90);
        let stars = (0..count)
            .map(|_| {
                (
                    rng.uniform(0.0, w),
                    rng.uniform(0.0, h),
                    rng.uniform(0.2, 1.2),
                    rng.uniform(0.4, 1.0),
                )
            })
            .collect();
        let small = small_side(params.width, params.height);
        let arm_r = rng.uniform(small * 0.5, small * 0.8);
        let arm_ang = rng.uniform(0.0, 2.0 * PI);
        Self {
            stars: drift_stars(stars, 0.02 * small, params.phase),
            arm_r,
            arm_ang: arm_ang + loop_angle(params.phase, 1),
            arm_w: rng.uniform(0.4, 1.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::diagonal;

    /// Python's `math.sqrt(w * w + h * h)` on exact ints, including sums that
    /// need more than 128 bits.
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn diagonal_matches_python_exact_ints() {
        let cases: [(usize, usize, f64); 8] = [
            (usize::MAX, usize::MAX, 2.6087635650665566e+19),
            (usize::MAX, 1, 1.8446744073709552e+19),
            (usize::MAX, 1 << 63, 2.0624086856177975e+19),
            ((1 << 63) + 1, (1 << 63) + 1, 1.3043817825332783e+19),
            (1 << 33, (1 << 33) + 1, 12148002000.611305),
            (640, 480, 800.0),
            (0, 0, 0.0),
            (1, 0, 1.0),
        ];
        for (width, height, expected) in cases {
            assert_eq!(
                diagonal(width, height).to_bits(),
                expected.to_bits(),
                "{width} x {height}"
            );
        }
    }
}
