//! Lunar, asteroid, black hole and supernova styles: per-pixel shaders that mirror
//! `_lunar_value`, `_asteroids_value`, `_blackhole_value` and `_supernova_value`.
//! Distances use `sqrt(dx * dx + dy * dy)` on both sides so results stay bit-identical.

use crate::math::Libm;
use std::f64::consts::{PI, TAU};

use super::{Style, center, loop_angle, small_side, unit};
use crate::geometry::py_mod;
use crate::noise::{contrast, fbm, fbm_loop, polar_noise};
use crate::palette::Palette;
use crate::random::PyRandom;
use crate::raster::shade;
use crate::scenes::{BuildScene, SceneParams};
use crate::validate::{Canvas, SceneError, check_loop_phase, check_numbers};

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
        total += bright * (-(q * q) / 2.0).m_exp();
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

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
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
                * (bowl + 0.35 * (-(rim * rim)).m_exp() + peak * 0.3 * (-(hump * hump)).m_exp());
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
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, craters, light);
        // Height, two gradient samples and up to LUNAR_STEPS shadow samples per pixel.
        let samples = 3.0 + LUNAR_STEPS as f64;
        Ok(canvas.per_pixel(self.craters.len() as f64 * samples))
    }

    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        let light = self.light;
        shade(width, height, palette, |x, y| {
            let (u, v) = view(x, y, width, height);
            let ground = self.height(u, v);
            let gx = (self.height(u + LUNAR_EPS, v) - ground) / LUNAR_EPS;
            let gy = (self.height(u, v + LUNAR_EPS) - ground) / LUNAR_EPS;
            let diffuse =
                (light.5 - gx * light.3 - gy * light.4) / (gx * gx + gy * gy + 1.0).sqrt();
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

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Asteroids {
    rocks: Vec<Rock>,
    stars: Vec<Star>,
    belt: (f64, f64),
    light: (f64, f64, f64),
}

impl Asteroids {
    fn shade_rock(&self, dx: f64, dy: f64, rock: &Rock) -> Option<f64> {
        let &(_, _, radius, spin, e2, q2, e3, q3, e4, q4, fade, tex) = rock;
        let theta = dy.m_atan2(dx) - spin;
        let edge = radius
            * (1.0
                + e2 * (2.0 * theta + q2).m_cos()
                + e3 * (3.0 * theta + q3).m_cos()
                + e4 * (5.0 * theta + q4).m_cos());
        if (dx * dx + dy * dy).sqrt() >= edge {
            return None;
        }
        let nx = dx / edge;
        let ny = dy / edge;
        let nz = (1.0 - (nx * nx + ny * ny)).max(0.0).sqrt();
        let (sn, cs) = (spin.m_sin(), spin.m_cos());
        let lu = (dx * cs + dy * sn) / edge;
        let lv = (dy * cs - dx * sn) / edge;
        let bump = 1.0 + 0.3 * (6.0 * lu + tex).m_sin() * (6.0 * lv + 1.7 * tex).m_sin();
        let lit = (nx * self.light.0 + ny * self.light.1 + nz * self.light.2).max(0.0);
        Some(fade * (0.1 + 0.8 * lit * bump))
    }

    fn sky(&self, u: f64, v: f64, seed: u32) -> f64 {
        let (cos_t, sin_t) = self.belt;
        let q = (v * cos_t - u * sin_t) / 0.7;
        let haze = (-(q * q)).m_exp();
        let dust = 0.03 + 0.2 * haze * contrast(fbm(u * 14.0, v * 14.0, seed, 3), 2.2).max(0.0);
        (dust + 0.8 * star_glow(u, v, &self.stars)).min(1.0)
    }
}

impl Style for Asteroids {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, rocks, stars, belt, light);
        Ok(canvas.per_pixel((self.rocks.len() + self.stars.len()) as f64))
    }

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

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
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
        let streak = 0.6 + 0.4 * (self.arms * y.m_atan2(x) + 4.0 * big + self.spin).m_sin();
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
        let shimmer = 0.8 + 0.2 * (self.arms * v.m_atan2(u) + 3.0 * r + self.spin).m_sin();
        let beam = 1.0 + 0.5 * u / r;
        edge * (0.9 * (-(ring * ring)).m_exp()
            + (0.4 + 0.9 * up) * (-(arc * arc)).m_exp() * shimmer)
            * (beam * beam)
    }

    fn sky(&self, u: f64, v: f64, seed: u32) -> f64 {
        let dust = 0.03 + 0.14 * contrast(fbm(u * 12.0, v * 12.0, seed, 4), 2.2).max(0.0);
        (dust + 0.8 * star_glow(u, v, &self.stars)).min(1.0)
    }
}

impl Style for Blackhole {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, center, radius, tilt, roll, arms, spin, stars);
        Ok(canvas.per_pixel(self.stars.len() as f64))
    }

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
            let glow = 1.0 - (-1.6 * energy).m_exp();
            unit(sky * (1.0 - glow) + glow)
        })
    }
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
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
        (-(across_b * across_b)).m_exp() * (-a.abs() / 0.35).m_exp()
            + (-(across_a * across_a)).m_exp() * (-b.abs() / 0.35).m_exp()
    }
}

impl Style for Supernova {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, center, radius, spike, pulse, stars);
        check_loop_phase(self.phase)?;
        Ok(canvas.per_pixel(self.stars.len() as f64))
    }

    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        let radius = self.radius;
        shade(width, height, palette, |x, y| {
            let (u, v) = view(x, y, width, height);
            let u = u - self.center.0;
            let v = v - self.center.1;
            let r = (u * u + v * v).sqrt();
            let wobble = polar_noise(0.0, v.m_atan2(u), seed, 6.0, 3);
            let shell_radius = radius * (1.0 + 0.5 * (wobble - 0.5));
            let band = (r - shell_radius) / (0.12 * radius);
            let shell = (-(band * band)).m_exp();
            let n = fbm_loop(u * 40.0 + 31.0, v * 40.0, seed, 5, self.phase);
            let ridge = (1.0 - (2.0 * contrast(n, 2.5) - 1.0).abs()).max(0.0);
            let ridge2 = ridge * ridge;
            let inner = r / (0.9 * radius);
            let blast = (r - 1.14 * shell_radius) / (0.04 * radius);
            let core = r / 0.05;
            let energy = 1.6 * shell * (0.2 + ridge2 * ridge2 * 1.6)
                + 0.7 * (-(inner * inner)).m_exp() * n * n
                + 0.3 * (-(blast * blast)).m_exp()
                + self.pulse * (2.0 * (1.0 / (1.0 + core * core)) + 0.9 * self.spikes(u, v))
                + 0.8 * star_glow(u, v, &self.stars);
            unit(1.0 - (-1.3 * energy).m_exp())
        })
    }
}

/// Python's `width / max(1.0, min(width, height))` and the same for the
/// height: half the canvas in view units, whose short side spans 2.
#[inline]
fn half_extents(width: usize, height: usize) -> (f64, f64) {
    let small = small_side(width, height);
    (width as f64 / small, height as f64 / small)
}

/// Python's `_sky_stars(rng, count, half_x, half_y)`.
fn sky_stars(rng: &mut PyRandom, count: usize, half_x: f64, half_y: f64) -> Vec<Star> {
    (0..count)
        .map(|_| {
            (
                rng.uniform(-half_x, half_x),
                rng.uniform(-half_y, half_y),
                rng.uniform(0.004, 0.011),
                rng.uniform(0.35, 1.0),
            )
        })
        .collect()
}

const LUNAR_CRATERS: usize = 36;

impl BuildScene for Lunar {
    /// `_lunar_scene`: 36 craters and a sun whose azimuth swings over the loop.
    fn build(params: &SceneParams<'_>) -> Self {
        let mut rng = PyRandom::new(params.seed);
        let (half_x, half_y) = half_extents(params.width, params.height);
        let craters = (0..LUNAR_CRATERS)
            .map(|_| {
                // Python's `x ** 3` on a float is libm pow.
                let radius = 0.025 + 0.3 * rng.random().m_powf(3.0);
                let x = rng.uniform(-half_x, half_x);
                let y = rng.uniform(-half_y, half_y);
                let depth = rng.uniform(0.1, 0.22);
                (x, y, radius, depth, if radius > 0.15 { 1.0 } else { 0.0 })
            })
            .collect();
        let azimuth = rng.uniform(0.0, TAU) + 0.9 * loop_angle(params.phase, 1).m_sin();
        let elevation = rng.uniform(0.2, 0.38);
        let flat = elevation.m_cos();
        Self {
            craters,
            light: (
                azimuth.m_cos(),
                azimuth.m_sin(),
                elevation.m_tan(),
                flat * azimuth.m_cos(),
                flat * azimuth.m_sin(),
                elevation.m_sin(),
            ),
        }
    }
}

const ASTEROID_COUNT: usize = 80;

impl BuildScene for Asteroids {
    /// `_asteroids_scene`: a tilted belt of 80 rocks drifting a whole number of
    /// laps per loop, sorted nearest (largest `fade`) first, over 90 stars.
    fn build(params: &SceneParams<'_>) -> Self {
        let phase = params.phase;
        let mut rng = PyRandom::new(params.seed);
        let (half_x, half_y) = half_extents(params.width, params.height);
        let tilt = rng.uniform(-0.6, 0.6);
        let (cos_t, sin_t) = (tilt.m_cos(), tilt.m_sin());
        let span = 2.0 * ((half_x * half_x + half_y * half_y).sqrt() + 0.3);
        let sun = rng.uniform(0.0, TAU);
        let stars = sky_stars(&mut rng, 90, half_x, half_y);
        let mut rocks: Vec<Rock> = (0..ASTEROID_COUNT)
            .map(|_| {
                let depth = rng.random();
                let lap = rng.random();
                let across = (rng.random() + rng.random() + rng.random() - 1.5) * 0.7;
                let radius = 0.03 + 0.15 * depth * depth * (0.4 + 0.6 * rng.random());
                let laps = if depth > 0.6 { 2.0 } else { 1.0 };
                let along = (py_mod(lap + laps * py_mod(phase, 1.0), 1.0) - 0.5) * span;
                let turns = if lap > 0.5 { 1 } else { -1 };
                (
                    along * cos_t - across * sin_t,
                    along * sin_t + across * cos_t,
                    radius,
                    rng.uniform(0.0, TAU) + loop_angle(phase, turns),
                    rng.uniform(0.04, 0.16),
                    rng.uniform(0.0, TAU),
                    rng.uniform(0.03, 0.12),
                    rng.uniform(0.0, TAU),
                    rng.uniform(0.02, 0.08),
                    rng.uniform(0.0, TAU),
                    0.45 + 0.55 * depth,
                    rng.uniform(0.0, TAU),
                )
            })
            .collect();
        // Python's stable `sort(key=lambda rock: -rock[10])`; fade is finite.
        rocks.sort_by(|a, b| (-a.10).total_cmp(&-b.10));
        Self {
            rocks,
            stars,
            belt: (cos_t, sin_t),
            light: (0.8 * sun.m_cos(), 0.8 * sun.m_sin(), 0.6),
        }
    }
}

impl BuildScene for Blackhole {
    /// `_blackhole_scene`: horizon, tilted and rolled disk turning two laps per
    /// loop, and 140 stars.
    fn build(params: &SceneParams<'_>) -> Self {
        let mut rng = PyRandom::new(params.seed);
        let (half_x, half_y) = half_extents(params.width, params.height);
        let roll = rng.uniform(-0.45, 0.45);
        let center = (rng.uniform(-0.12, 0.12), rng.uniform(-0.12, 0.12));
        let radius = rng.uniform(0.17, 0.24);
        let tilt = rng.uniform(0.16, 0.3);
        let arms = rng.randint(2, 5) as f64;
        let spin = rng.uniform(0.0, TAU) - loop_angle(params.phase, 2);
        Self {
            center,
            radius,
            tilt,
            roll: (roll.m_cos(), roll.m_sin()),
            arms,
            spin,
            stars: sky_stars(&mut rng, 140, half_x, half_y),
        }
    }
}

impl BuildScene for Supernova {
    /// `_supernova_scene`: a breathing shell, a pulsing core with turned
    /// diffraction spikes, looping filament noise and 110 stars.
    fn build(params: &SceneParams<'_>) -> Self {
        let phase = params.phase;
        let mut rng = PyRandom::new(params.seed);
        let (half_x, half_y) = half_extents(params.width, params.height);
        let center = (rng.uniform(-0.12, 0.12), rng.uniform(-0.12, 0.12));
        let radius = rng.uniform(0.5, 0.68) * (1.0 + 0.06 * loop_angle(phase, 1).m_sin());
        let spike = rng.uniform(0.0, PI / 2.0);
        Self {
            center,
            radius,
            spike: (spike.m_cos(), spike.m_sin()),
            pulse: 1.0 + 0.3 * loop_angle(phase, 1).m_cos(),
            phase,
            stars: sky_stars(&mut rng, 110, half_x, half_y),
        }
    }
}
