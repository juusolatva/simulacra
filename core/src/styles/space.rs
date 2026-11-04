//! Lunar, asteroid, black hole and supernova styles: per-pixel shaders that mirror
//! `_lunar_value`, `_asteroids_value`, `_blackhole_value` and `_supernova_value`.
//! Distances use `sqrt(dx * dx + dy * dy)` on both sides so results stay bit-identical.

use pyo3::FromPyObject;

use super::{Style, center, unit};
use crate::noise::{contrast, fbm, fbm_loop, polar_noise};
use crate::palette::Palette;
use crate::raster::shade;

/// `(x, y, sigma, brightness)`
type Star = (f64, f64, f64, f64);

/// Canvas units for pixel `(x, y)`: the short side spans -1 to 1 (Python's `_view`).
#[inline]
fn view(x: usize, y: usize, width: usize, height: usize) -> (f64, f64) {
    let (cx, cy) = center(width, height);
    let scale = (width.min(height) as f64 / 2.0).max(1.0);
    ((x as f64 - cx) / scale, (y as f64 - cy) / scale)
}

fn star_glow(u: f64, v: f64, stars: &[Star]) -> f64 {
    let mut total = 0.0;
    for &(sx, sy, sigma, bright) in stars {
        let dx = u - sx;
        let dy = v - sy;
        let reach = 4.0 * sigma;
        if dx.abs() > reach || dy.abs() > reach {
            continue;
        }
        let q = (dx * dx + dy * dy).sqrt() / sigma;
        total += bright * (-(q * q) / 2.0).exp();
    }
    total
}

#[inline]
fn smoothstep(t: f64) -> f64 {
    let t = unit(t);
    t * t * (3.0 - 2.0 * t)
}

const LUNAR_REACH: f64 = 2.2;
const LUNAR_STEPS: i64 = 18;
const LUNAR_STEP: f64 = 0.04;
const LUNAR_EPS: f64 = 0.004;

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub struct Lunar {
    /// (x, y, radius, depth, peak)
    craters: Vec<(f64, f64, f64, f64, f64)>,
    /// (dx, dy, slope, lx, ly, lz)
    light: (f64, f64, f64, f64, f64, f64),
}

impl Lunar {
    fn height(&self, u: f64, v: f64) -> f64 {
        let mut height = 0.0;
        for &(cx, cy, radius, depth, peak) in &self.craters {
            let reach = radius * LUNAR_REACH;
            let dx = u - cx;
            let dy = v - cy;
            if dx.abs() >= reach || dy.abs() >= reach {
                continue;
            }
            let d = (dx * dx + dy * dy).sqrt() / radius;
            if d >= LUNAR_REACH {
                continue;
            }
            let bowl = if d < 1.0 { d * d - 1.0 } else { 0.0 };
            let rim = (d - 1.0) / 0.18;
            let hump = d / 0.15;
            height += radius
                * depth
                * (bowl + 0.35 * (-(rim * rim)).exp() + peak * 0.3 * (-(hump * hump)).exp());
        }
        height
    }

    fn shadow(&self, u: f64, v: f64, ground: f64) -> f64 {
        let slope = self.light.2;
        let mut clear = slope;
        for k in 1..=LUNAR_STEPS {
            let t = k as f64 * LUNAR_STEP;
            let blocked = self.height(u + self.light.0 * t, v + self.light.1 * t) - ground;
            clear = clear.min((slope * t - blocked) / t);
            if clear < -0.05 {
                return 0.0;
            }
        }
        unit(0.5 + clear / 0.1)
    }
}

impl Style for Lunar {
    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        let light = self.light;
        shade(width, height, palette, |x, y| {
            let (u, v) = view(x, y, width, height);
            let ground = self.height(u, v);
            let gx = (self.height(u + LUNAR_EPS, v) - ground) / LUNAR_EPS;
            let gy = (self.height(u, v + LUNAR_EPS) - ground) / LUNAR_EPS;
            let diffuse = (light.5 - gx * light.3 - gy * light.4) / (gx * gx + gy * gy + 1.0).sqrt();
            let lit = diffuse.max(0.0) / light.5;
            let sun = if lit > 0.0 {
                self.shadow(u, v, ground)
            } else {
                0.0
            };
            let albedo = 0.85 + 0.3 * contrast(fbm(u * 90.0, v * 90.0, seed, 4), 1.5);
            unit(0.04 + 0.6 * lit.min(1.5) * sun * albedo)
        })
    }
}

/// (x, y, radius, spin, e2, q2, e3, q3, e4, q4, fade, texture)
type Rock = (f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64);

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub struct Asteroids {
    rocks: Vec<Rock>,
    stars: Vec<Star>,
    belt: (f64, f64),
    light: (f64, f64, f64),
}

impl Asteroids {
    fn shade_rock(&self, dx: f64, dy: f64, rock: &Rock) -> Option<f64> {
        let &(_, _, radius, spin, e2, q2, e3, q3, e4, q4, fade, tex) = rock;
        let theta = dy.atan2(dx) - spin;
        let edge = radius
            * (1.0
                + e2 * (2.0 * theta + q2).cos()
                + e3 * (3.0 * theta + q3).cos()
                + e4 * (5.0 * theta + q4).cos());
        if (dx * dx + dy * dy).sqrt() >= edge {
            return None;
        }
        let nx = dx / edge;
        let ny = dy / edge;
        let nz = (1.0 - (nx * nx + ny * ny)).max(0.0).sqrt();
        let (sn, cs) = (spin.sin(), spin.cos());
        let lu = (dx * cs + dy * sn) / edge;
        let lv = (dy * cs - dx * sn) / edge;
        let bump = 1.0 + 0.3 * (6.0 * lu + tex).sin() * (6.0 * lv + 1.7 * tex).sin();
        let lit = (nx * self.light.0 + ny * self.light.1 + nz * self.light.2).max(0.0);
        Some(fade * (0.1 + 0.8 * lit * bump))
    }

    fn sky(&self, u: f64, v: f64, seed: u32) -> f64 {
        let (cos_t, sin_t) = self.belt;
        let q = (v * cos_t - u * sin_t) / 0.7;
        let haze = (-(q * q)).exp();
        let dust = 0.03 + 0.2 * haze * contrast(fbm(u * 14.0, v * 14.0, seed, 3), 2.2).max(0.0);
        (dust + 0.8 * star_glow(u, v, &self.stars)).min(1.0)
    }
}

impl Style for Asteroids {
    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        shade(width, height, palette, |x, y| {
            let (u, v) = view(x, y, width, height);
            for rock in &self.rocks {
                let dx = u - rock.0;
                let dy = v - rock.1;
                let reach = 1.5 * rock.2;
                if dx.abs() >= reach || dy.abs() >= reach {
                    continue;
                }
                if let Some(brightness) = self.shade_rock(dx, dy, rock) {
                    return unit(brightness);
                }
            }
            self.sky(u, v, seed)
        })
    }
}

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub struct Blackhole {
    center: (f64, f64),
    radius: f64,
    tilt: f64,
    roll: (f64, f64),
    arms: f64,
    spin: f64,
    stars: Vec<Star>,
}

impl Blackhole {
    fn disk(&self, u: f64, v: f64, edge: f64) -> f64 {
        let x = u;
        let y = v / self.tilt;
        let big = (x * x + y * y).sqrt().max(1e-6);
        let streak = 0.6 + 0.4 * (self.arms * y.atan2(x) + 4.0 * big + self.spin).sin();
        let beam = 1.0 + 0.55 * x / big;
        let seen = if y > 0.0 { 1.0 } else { edge };
        4.0 / (big * big)
            * smoothstep((big - 2.0) / 0.35)
            * smoothstep((6.5 - big) / 2.5)
            * streak
            * (beam * beam)
            * seen
    }

    fn halo(&self, u: f64, v: f64, r: f64, edge: f64) -> f64 {
        let up = (-v / r).max(0.0);
        let ring = (r - 1.5) / 0.04;
        let arc = (r - (1.5 + 0.55 * up)) / (0.05 + 0.4 * up * up);
        let shimmer = 0.8 + 0.2 * (self.arms * v.atan2(u) + 3.0 * r + self.spin).sin();
        let beam = 1.0 + 0.5 * u / r;
        edge * (0.9 * (-(ring * ring)).exp()
            + (0.4 + 0.9 * up) * (-(arc * arc)).exp() * shimmer)
            * (beam * beam)
    }

    fn sky(&self, u: f64, v: f64, seed: u32) -> f64 {
        let dust = 0.03 + 0.14 * contrast(fbm(u * 12.0, v * 12.0, seed, 4), 2.2).max(0.0);
        (dust + 0.8 * star_glow(u, v, &self.stars)).min(1.0)
    }
}

impl Style for Blackhole {
    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        let (cos_r, sin_r) = self.roll;
        shade(width, height, palette, |x, y| {
            let (u, v) = view(x, y, width, height);
            let du = u - self.center.0;
            let dv = v - self.center.1;
            let a = (du * cos_r + dv * sin_r) / self.radius;
            let b = (dv * cos_r - du * sin_r) / self.radius;
            let r = (a * a + b * b).sqrt().max(1e-6);
            let edge = unit((r - 1.0) / 0.03);
            let lens = 1.0 - 1.6 / (r * r);
            let sky = self.sky(du * lens, dv * lens, seed) * edge;
            let energy = self.disk(a, b, edge) + self.halo(a, b, r, edge);
            let glow = 1.0 - (-1.6 * energy).exp();
            unit(sky * (1.0 - glow) + glow)
        })
    }
}

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub struct Supernova {
    center: (f64, f64),
    radius: f64,
    spike: (f64, f64),
    pulse: f64,
    phase: f64,
    stars: Vec<Star>,
}

impl Supernova {
    fn spikes(&self, u: f64, v: f64) -> f64 {
        let a = u * self.spike.0 + v * self.spike.1;
        let b = v * self.spike.0 - u * self.spike.1;
        let across_b = b / 0.008;
        let across_a = a / 0.008;
        (-(across_b * across_b)).exp() * (-a.abs() / 0.35).exp()
            + (-(across_a * across_a)).exp() * (-b.abs() / 0.35).exp()
    }
}

impl Style for Supernova {
    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        let radius = self.radius;
        shade(width, height, palette, |x, y| {
            let (u, v) = view(x, y, width, height);
            let u = u - self.center.0;
            let v = v - self.center.1;
            let r = (u * u + v * v).sqrt();
            let wobble = polar_noise(0.0, v.atan2(u), seed, 6.0, 3);
            let shell_radius = radius * (1.0 + 0.5 * (wobble - 0.5));
            let band = (r - shell_radius) / (0.12 * radius);
            let shell = (-(band * band)).exp();
            let n = fbm_loop(u * 40.0 + 31.0, v * 40.0, seed, 5, self.phase);
            let ridge = (1.0 - (2.0 * contrast(n, 2.5) - 1.0).abs()).max(0.0);
            let ridge2 = ridge * ridge;
            let inner = r / (0.9 * radius);
            let blast = (r - 1.14 * shell_radius) / (0.04 * radius);
            let core = r / 0.05;
            let energy = 1.6 * shell * (0.2 + ridge2 * ridge2 * 1.6)
                + 0.7 * (-(inner * inner)).exp() * n * n
                + 0.3 * (-(blast * blast)).exp()
                + self.pulse * (2.0 * (1.0 / (1.0 + core * core)) + 0.9 * self.spikes(u, v))
                + 0.8 * star_glow(u, v, &self.stars);
            unit(1.0 - (-1.3 * energy).exp())
        })
    }
}
