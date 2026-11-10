//! Noise- and wave-based styles that compute each pixel independently.

use crate::math::{Libm, py_hypot};
use std::f64::consts::{PI, TAU};

use rayon::prelude::*;

use super::{Style, center, gaussian, loop_angle, small_side, unit};
use crate::geometry::py_mod;
use crate::noise::{contrast, fbm, fbm_loop};
use crate::palette::Palette;
use crate::validate::{Canvas, SceneError, check_index, check_loop_phase, check_numbers};
use crate::random::PyRandom;
use crate::raster::{shade, shade_rgb};

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Blobs {
    phase: f64,
}

impl Blobs {
    /// `_blobs_scene`: the noise only needs the phase.
    #[must_use]
    pub fn build(phase: f64) -> Self {
        Self { phase }
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
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Waves {
    layers: Vec<(f64, f64, f64)>,
}

impl Waves {
    /// `_waves_scene`: four layers of (amplitude, frequency, phase), the phases
    /// drawn from `random.Random(seed)` and turned by the animation phase.
    #[must_use]
    #[allow(
        clippy::approx_constant,
        reason = "Python draws uniform(0, 6.28), not tau; the scene must match it"
    )]
    pub fn build(seed: i64, phase: f64) -> Self {
        let mut rng = PyRandom::new(seed);
        let layers = vec![
            (30.0, 2.0, rng.uniform(0.0, 6.28) + loop_angle(phase, 1)),
            (18.0, 3.5, rng.uniform(0.0, 6.28) + loop_angle(phase, -1)),
            (12.0, 5.0, rng.uniform(0.0, 6.28) + loop_angle(phase, 2)),
            (7.0, 7.0, rng.uniform(0.0, 6.28) + loop_angle(phase, -2)),
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
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Plasma {
    layers: Vec<(f64, f64, f64, f64)>,
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
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Interference {
    layers: Vec<(f64, f64, f64, f64)>,
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
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Kaleidoscope {
    folds: i64,
    phase: f64,
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
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Pinwheel {
    n_arms: i64,
    twist: f64,
    spin: f64,
    phase: f64,
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
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Galaxy {
    bulge_r: f64,
    bulge_amp: f64,
    /// (r0, width, phase, pitch, amp)
    arms: Vec<(f64, f64, f64, f64, f64)>,
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
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Bloom {
    /// (x, y, radius, amp)
    blooms: Vec<(f64, f64, f64, f64)>,
    phase: f64,
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
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Nebula {
    /// (x, y, radius, amp)
    clouds: Vec<(f64, f64, f64, f64)>,
    /// (x, y, radius)
    stars: Vec<(f64, f64, f64)>,
    phase: f64,
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
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Orbits {
    /// (x, y, radius)
    stars: Vec<(f64, f64, f64)>,
    /// (radius, palette stop index)
    rings: Vec<(f64, usize)>,
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
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
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
        let (arm_cos, arm_sin, arm_tan) =
            (self.arm_ang.m_cos(), self.arm_ang.m_sin(), self.arm_w.m_tan());
        shade(width, height, palette, |x, y| {
            let (x, y) = (x as f64, y as f64);
            let rad = py_hypot(x - ox, y - oy);
            let mut v = gaussian(rad, glow_sigma) * 0.6;
            let ang = (y - oy).m_atan2(x - ox);
            let ang_diff = py_mod(ang - self.arm_ang + PI, 2.0 * PI) - PI;
            let dist_along = rad * arm_tan + self.arm_r * (ang_diff / self.arm_w).m_cos();
            let d2 = py_hypot(x - (ox + dist_along * arm_cos), y - (oy + dist_along * arm_sin));
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
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Rings {
    /// (radius, phase)
    rings: Vec<(f64, f64)>,
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
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Starburst {
    rays: f64,
    phase: f64,
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
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Spiral {
    /// (turns, phase, amp, width)
    arms: Vec<(f64, f64, f64, f64)>,
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
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Petals {
    count: f64,
    length: f64,
    phase: f64,
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
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Aurora {
    /// (y0, amp, freq, phase, bw)
    ribbons: Vec<(f64, f64, f64, f64, f64)>,
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
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Gems {
    /// (cx, cy, radius, rotation)
    gems: Vec<(f64, f64, f64, f64)>,
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
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Cosmos {
    /// (x, y, max_r, rings, phase, amp)
    centers: Vec<(f64, f64, f64, i64, f64, f64)>,
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
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Mandelbrot {
    scale: f64,
    center: (f64, f64),
    iterations: u32,
}

/// `_MANDELBROT_HOME`, `_MANDELBROT_TARGET` and `_MANDELBROT_DEPTH` in Python.
const MANDELBROT_HOME: (f64, f64) = (-0.6, 0.0);
const MANDELBROT_TARGET: (f64, f64) = (-0.743_643_887_037_151, 0.131_825_904_205_330);
const MANDELBROT_DEPTH: f64 = 7.0;

impl Mandelbrot {
    /// `_mandelbrot_scene`: a zoom toward `MANDELBROT_TARGET` and back over a
    /// loop, eased at both ends; phase 0 is the whole set.
    #[must_use]
    pub fn build(width: usize, height: usize, phase: f64) -> Self {
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
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Horizon {
    /// (ridge height fraction, noise offset, palette position) per layer
    layers: Vec<(f64, f64, f64)>,
    phase: f64,
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
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Voronoi {
    /// (x, y, brightness)
    sites: Vec<(f64, f64, f64)>,
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
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Crystal {
    light: (f64, f64, f64),
    half: (f64, f64, f64),
    phase: f64,
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
            let tilt = ((py_hypot(gx, gy) * relief).m_atan() / tilt_step).round_ties_even() * tilt_step;
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
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
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

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
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

const CONTOUR_LEVELS: f64 = 14.0;

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
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

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
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

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
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
