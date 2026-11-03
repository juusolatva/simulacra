//! Noise- and wave-based styles that compute each pixel independently.

use std::f64::consts::{PI, TAU};

use pyo3::FromPyObject;
use rayon::prelude::*;

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

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub struct Rings {
    /// (radius, phase)
    rings: Vec<(f64, f64)>,
}

impl Style for Rings {
    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        let (cx, cy) = center(width, height);
        shade(width, height, palette, |x, y| {
            let (dx, dy) = (x as f64 - cx, y as f64 - cy);
            let r = (dx.powi(2) + dy.powi(2)).sqrt();
            // The last matching ring wins; no ring leaves the first stop (t = 0).
            let mut t = 0.0;
            for &(radius, phase) in &self.rings {
                if radius * 0.9 < r && r < radius * 1.1 {
                    let angle = dy.atan2(dx);
                    t = 0.3 + 0.35 * (1.0 + (angle + phase).cos());
                }
            }
            t
        })
    }
}

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub struct Starburst {
    rays: f64,
    phase: f64,
}

impl Style for Starburst {
    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        let (cx, cy) = center(width, height);
        let scale = (width.min(height) as f64 / 2.0).max(1.0);
        shade(width, height, palette, |x, y| {
            let (dx, dy) = ((x as f64 - cx) / scale, (y as f64 - cy) / scale);
            let r = dx.hypot(dy);
            let theta = dy.atan2(dx);
            let ray = (0.5 + 0.5 * (theta * self.rays + self.phase).cos()).powf(12.0);
            let q = r / 0.18;
            let glow = (-(q * q)).exp();
            glow + ray * (-r * 1.3).exp() * (1.0 - (-r * 12.0).exp())
        })
    }
}

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub struct Spiral {
    /// (turns, phase, amp, width)
    arms: Vec<(f64, f64, f64, f64)>,
}

impl Style for Spiral {
    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        let (cx, cy) = center(width, height);
        let max_r = cx.hypot(cy).max(1.0);
        shade(width, height, palette, |x, y| {
            let (dx, dy) = (x as f64 - cx, y as f64 - cy);
            let r = (dx * dx + dy * dy).sqrt() / max_r;
            let theta = dy.atan2(dx);
            let mut v = 0.0;
            for &(turns, phase, amp, arm_width) in &self.arms {
                let d = py_mod(r * turns * TAU - theta - phase, TAU);
                let dist = (d - PI).abs();
                v += amp * gaussian(dist, arm_width);
            }
            v *= 1.0 - 0.5 * r;
            1.0 - (-1.6 * v).exp()
        })
    }
}

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub struct Petals {
    count: f64,
    length: f64,
    phase: f64,
}

impl Style for Petals {
    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        let (cx, cy) = center(width, height);
        let scale = (width.min(height) as f64 / 2.0).max(1.0);
        shade(width, height, palette, |x, y| {
            let (dx, dy) = ((x as f64 - cx) / scale, (y as f64 - cy) / scale);
            let r = dx.hypot(dy);
            let theta = dy.atan2(dx);
            let angular = (0.5 + 0.5 * (self.count * theta + self.phase).cos()).powf(3.0);
            let q = (r - self.length * 0.52) / (self.length * 0.3);
            let radial = (-(q * q)).exp();
            let q = r / 0.16;
            angular * radial + (-(q * q)).exp()
        })
    }
}

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub struct Aurora {
    /// (y0, amp, freq, phase, bw)
    ribbons: Vec<(f64, f64, f64, f64, f64)>,
}

impl Style for Aurora {
    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        shade(width, height, palette, |x, y| {
            let (x, y) = (x as f64, y as f64);
            let mut v = 0.0;
            for &(y0, amp, freq, phase, bw) in &self.ribbons {
                let yy = y0 + amp * (x * freq + phase).sin();
                let d = (y - yy) / amp;
                let w = (-d * d).exp();
                let s = (x * freq * 2.0 + phase).sin();
                v += w * (0.5 + 0.5 * s) * bw;
            }
            0.05 + 0.95 * (1.0 - (-1.5 * v).exp())
        })
    }
}

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub struct Gems {
    /// (cx, cy, radius, rotation)
    gems: Vec<(f64, f64, f64, f64)>,
}

impl Style for Gems {
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
                    let local = ((rot + depth * 3.0).cos() + 1.0) / 2.0;
                    t = unit((1.0 - depth) * local);
                }
            }
            t
        })
    }
}

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub struct Cosmos {
    /// (x, y, max_r, rings, phase, amp)
    centers: Vec<(f64, f64, f64, i64, f64, f64)>,
}

impl Style for Cosmos {
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
                    let s = (k as f64 * 2.0 + phase).sin();
                    v += w * amp * (0.5 + 0.5 * s);
                    weight += w;
                }
            }
            unit(v / (weight + 0.6)) * 0.95
        })
    }
}

/// `render_mandelbrot` (no random scene).
pub struct Mandelbrot;

impl Style for Mandelbrot {
    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        const MAX_ITER: u32 = 100;
        let scale = 3.2 / width.min(height) as f64;
        let (mid_x, mid_y) = center(width, height);
        shade(width, height, palette, |x, y| {
            let cy = (mid_y - y as f64) * scale;
            let cx = (x as f64 - mid_x) * scale - 0.6;
            let (mut zx, mut zy) = (0.0_f64, 0.0_f64);
            let mut n = 0;
            while zx * zx + zy * zy <= 4.0 && n < MAX_ITER {
                (zx, zy) = (zx * zx - zy * zy + cx, 2.0 * zx * zy + cy);
                n += 1;
            }
            if n == MAX_ITER {
                1.0
            } else {
                (n as f64 / MAX_ITER as f64).sqrt()
            }
        })
    }
}

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub struct Horizon {
    /// (ridge height fraction, noise offset, palette position) per layer
    layers: Vec<(f64, f64, f64)>,
}

impl Style for Horizon {
    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        let h = height as f64;
        let span = (width as f64 - 1.0).max(1.0);
        let ridges: Vec<(Vec<f64>, [u8; 3])> = self
            .layers
            .iter()
            .map(|&(fraction, offset, color)| {
                let ridge = (0..width)
                    .map(|x| {
                        h * (fraction + 0.18 * (fbm(x as f64 / span * 40.0, offset, seed, 3) - 0.5))
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

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub struct Voronoi {
    /// (x, y, brightness)
    sites: Vec<(f64, f64, f64)>,
}

impl Style for Voronoi {
    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        let scale = (width.min(height) as f64).max(1.0);
        shade(width, height, palette, |x, y| {
            let (x, y) = (x as f64, y as f64);
            // Python sorts (distance, brightness) tuples and reads the first two
            // distances: track the lexicographic minimum and the second distance.
            let mut nearest = (f64::INFINITY, f64::INFINITY);
            let mut second = f64::INFINITY;
            for &(sx, sy, color) in &self.sites {
                let d = (x - sx).hypot(y - sy);
                if d < nearest.0 || (d == nearest.0 && color < nearest.1) {
                    second = nearest.0;
                    nearest = (d, color);
                } else if d < second {
                    second = d;
                }
            }
            let gap = second - nearest.0;
            let edge = (-gap / (scale * 0.012).max(0.5)).exp();
            nearest.1 * (-nearest.0 / (scale * 0.3)).exp() + edge * 0.5
        })
    }
}

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub struct Crystal {
    light: (f64, f64, f64),
    half: (f64, f64, f64),
}

impl Style for Crystal {
    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        let k = 28.0 / width.max(height) as f64;
        let relief = 14.0 / k;
        let tilt_step = PI / 10.0;
        let az_step = PI / 4.0;
        let (light, half) = (self.light, self.half);

        // Height field first: every pixel's gradient reads its neighbours.
        let field: Vec<f64> = (0..width * height)
            .into_par_iter()
            .map(|i| fbm((i % width) as f64 * k, (i / width) as f64 * k, seed, 3))
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
            let tilt = ((gx.hypot(gy) * relief).atan() / tilt_step).round_ties_even() * tilt_step;
            let azimuth = (gy.atan2(gx) / az_step).round_ties_even() * az_step;
            let n = (
                -tilt.sin() * azimuth.cos(),
                -tilt.sin() * azimuth.sin(),
                tilt.cos(),
            );
            let diffuse = (n.0 * light.0 + n.1 * light.1 + n.2 * light.2).max(0.0);
            let spec = (n.0 * half.0 + n.1 * half.1 + n.2 * half.2)
                .max(0.0)
                .powf(24.0);
            0.75 * diffuse * (0.5 + at(x, y)) + 0.5 * spec - 0.2
        })
    }
}
