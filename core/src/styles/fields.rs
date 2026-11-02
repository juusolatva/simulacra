//! Noise- and wave-based styles that compute each pixel independently.

use std::f64::consts::{PI, TAU};

use pyo3::FromPyObject;

use super::{Style, center, gaussian, small_side, unit};
use crate::geometry::py_mod;
use crate::noise::{contrast, fbm};
use crate::palette::Palette;
use crate::raster::{shade, shade_rgb};

/// `render_blobs` (no random scene).
pub struct Blobs;

impl Style for Blobs {
    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        let k = 40.0 / width.max(height) as f64;
        shade(width, height, palette, |x, y| {
            contrast(fbm(x as f64 * k, y as f64 * k, seed, 5), 2.2)
        })
    }
}

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub struct Waves {
    layers: Vec<(f64, f64, f64)>,
}

impl Style for Waves {
    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        let size = width.max(height) as f64;
        shade(width, height, palette, |x, y| {
            let (x, y) = (x as f64, y as f64);
            let n = fbm(x * 0.015, y * 0.015, seed, 3);
            let mut v = 0.1 + n * 0.5;
            for &(amp, freq, phase) in &self.layers {
                let angle = (x + y * 0.9) * freq * TAU / size + phase;
                v += amp / 67.0 * angle.sin() * (0.3 + n);
            }
            unit(v * 1.4)
        })
    }
}

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub struct Plasma {
    layers: Vec<(f64, f64, f64, f64)>,
}

impl Style for Plasma {
    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        shade(width, height, palette, |x, y| {
            let (x, y) = (x as f64, y as f64);
            let mut v = 0.0;
            let n = fbm(x * 0.01, y * 0.01, seed, 4);
            for &(amp, kx, ky, phase) in &self.layers {
                v += amp * (x * kx + y * ky + phase).sin();
            }
            0.5 + 0.5 * (v * 1.5 + n * 3.0).sin()
        })
    }
}

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub struct Interference {
    layers: Vec<(f64, f64, f64, f64)>,
}

impl Style for Interference {
    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        shade(width, height, palette, |x, y| {
            let (x, y) = (x as f64, y as f64);
            let mut v = 0.0;
            for &(phase, kx, ky, amp) in &self.layers {
                v += (x * kx + y * ky + phase).sin() * amp;
            }
            let n = fbm(x * 0.006, y * 0.006, seed, 2);
            v += n * 2.0;
            (v * 0.35).sin() * 0.5 + 0.5 + (n - 0.5) * 0.3
        })
    }
}

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub struct Kaleidoscope {
    folds: i64,
    phase: f64,
}

impl Style for Kaleidoscope {
    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        let (cx, cy) = center(width, height);
        let scale = (width.min(height) as f64 / 2.0).max(1.0);
        let folds = self.folds as f64;
        let sector = TAU / folds;
        shade(width, height, palette, |x, y| {
            let dx = (x as f64 - cx) / scale;
            let dy = (y as f64 - cy) / scale;
            let r = dx.hypot(dy);
            let angle = (py_mod(dy.atan2(dx) + sector / 2.0, sector) - sector / 2.0).abs();
            let (u, v) = (r * angle.cos(), r * angle.sin());
            let n = fbm(u * 32.0, v * 32.0, seed, 3);
            let t = 0.5 + 0.5 * (r * 18.0 + angle * folds * 2.0 + n * 4.0 + self.phase).sin();
            t * (-0.35 * r * r).exp()
        })
    }
}

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub struct Pinwheel {
    n_arms: i64,
    twist: f64,
}

impl Style for Pinwheel {
    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        let (half_w, half_h) = (width as f64 / 2.0, height as f64 / 2.0);
        let max_r = (half_w * half_w + half_h * half_h).sqrt() * 1.05;
        let arms = self.n_arms as f64;
        shade(width, height, palette, |x, y| {
            let dx = x as f64 - half_w;
            let dy = y as f64 - half_h;
            let r = (dx * dx + dy * dy).sqrt();
            let theta = if dx != 0.0 || dy != 0.0 {
                dy.atan2(dx)
            } else {
                0.0
            };
            let t = unit(r / max_r);
            let arm_phase = theta * arms + t * self.twist;
            let d = (py_mod(arm_phase + PI, 2.0 * PI) - PI).abs();
            let w = gaussian(d, 0.35 * (1.0 - t));
            let n = fbm(x as f64 * 0.006, y as f64 * 0.006, seed, 2);
            unit((w * (1.0 - 0.35 * t) + n * 0.15) / 1.2)
        })
    }
}

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub struct Galaxy {
    bulge_r: f64,
    bulge_amp: f64,
    /// (r0, width, phase, pitch, amp)
    arms: Vec<(f64, f64, f64, f64, f64)>,
}

impl Style for Galaxy {
    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        let (cx, cy) = center(width, height);
        let max_r = cx.hypot(cy).max(1.0);
        shade(width, height, palette, |x, y| {
            let dx = x as f64 - cx;
            let dy = y as f64 - cy;
            let r = (dx * dx + dy * dy).sqrt();
            let theta = dy.atan2(dx);
            let n = fbm(x as f64 * 0.0035, y as f64 * 0.0035, seed, 2);
            let q = r / self.bulge_r;
            let mut v = (-0.5 * (q * q)).exp() * self.bulge_amp * (0.8 + 0.4 * n);
            for &(r0, arm_width, phase, pitch, amp) in &self.arms {
                let arm_theta = phase + (r / r0).ln_1p() * pitch;
                let d = (py_mod(arm_theta - theta + PI, TAU) - PI).abs();
                let q = d / arm_width;
                v += (-0.5 * (q * q) - r / max_r).exp() * amp;
            }
            (1.0 - (-v).exp()) * 0.95
        })
    }
}

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub struct Bloom {
    /// (x, y, radius, amp)
    blooms: Vec<(f64, f64, f64, f64)>,
}

impl Style for Bloom {
    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        shade(width, height, palette, |x, y| {
            let (x, y) = (x as f64, y as f64);
            let mut v = 0.0;
            let n = fbm(x * 0.006, y * 0.006, seed, 3);
            for &(bx, by, br, amp) in &self.blooms {
                v += gaussian((x - bx).hypot(y - by), br) * amp;
            }
            v *= 0.4 + 1.2 * n;
            1.0 - (-1.4 * v).exp()
        })
    }
}

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub struct Nebula {
    /// (x, y, radius, amp)
    clouds: Vec<(f64, f64, f64, f64)>,
    /// (x, y, radius)
    stars: Vec<(f64, f64, f64)>,
}

impl Style for Nebula {
    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        let noise_k = 24.0 / width.max(height) as f64;
        shade(width, height, palette, |x, y| {
            let (x, y) = (x as f64, y as f64);
            let mut v = 0.0;
            for &(ccx, ccy, cr, amp) in &self.clouds {
                let d2 = (x - ccx) * (x - ccx) + (y - ccy) * (y - ccy);
                v += (-d2 / (2.0 * (cr * cr))).exp() * amp;
            }
            let n = fbm(x * noise_k, y * noise_k, seed, 4);
            v *= (0.2 + 1.6 * (contrast(n, 2.0) - 0.2)).max(0.0);
            if self
                .stars
                .iter()
                .any(|&(sx, sy, sr)| (x - sx).hypot(y - sy) < sr)
            {
                v += 1.2;
            }
            1.0 - (-1.3 * v).exp()
        })
    }
}

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub struct Orbits {
    /// (x, y, radius)
    stars: Vec<(f64, f64, f64)>,
    /// (radius, palette stop index)
    rings: Vec<(f64, usize)>,
}

impl Style for Orbits {
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
                .any(|&(sx, sy, sr)| (x - sx).hypot(y - sy) < sr * 1.2)
            {
                color = [255, 255, 255];
            }
            color
        })
    }
}

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub struct Cosmic {
    /// (x, y, size, brightness)
    stars: Vec<(f64, f64, f64, f64)>,
    arm_r: f64,
    arm_ang: f64,
    arm_w: f64,
}

impl Style for Cosmic {
    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        let small = small_side(width, height);
        let (ox, oy) = (width as f64 / 2.0, height as f64 / 2.0);
        let glow_sigma = small * 0.55;
        let arm_sigma = (small * 0.08).max(1.0);
        let star_sigma = (small * 0.02).max(1.0);
        let (arm_cos, arm_sin, arm_tan) =
            (self.arm_ang.cos(), self.arm_ang.sin(), self.arm_w.tan());
        shade(width, height, palette, |x, y| {
            let (x, y) = (x as f64, y as f64);
            let rad = (x - ox).hypot(y - oy);
            let mut v = gaussian(rad, glow_sigma) * 0.6;
            let ang = (y - oy).atan2(x - ox);
            let ang_diff = py_mod(ang - self.arm_ang + PI, 2.0 * PI) - PI;
            let dist_along = rad * arm_tan + self.arm_r * (ang_diff / self.arm_w).cos();
            let d2 = (x - (ox + dist_along * arm_cos)).hypot(y - (oy + dist_along * arm_sin));
            v += gaussian(d2, arm_sigma) * 1.2;
            for &(sx, sy, size, brightness) in &self.stars {
                let ds = (x - sx).hypot(y - sy);
                if ds < star_sigma * 3.0 {
                    v += size * brightness * gaussian(ds, star_sigma);
                }
            }
            1.0 - (-1.2 * v).exp()
        })
    }
}
