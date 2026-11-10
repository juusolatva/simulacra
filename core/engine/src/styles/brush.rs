//! Brush-, trail- and curve-based styles.
//!
//! These accumulate contributions in a fixed order, and floating-point
//! addition is not associative, so stamping runs sequentially in exactly the
//! order `simulacra.py` uses. Only the final per-pixel shading is parallel.

use crate::math::{Libm, py_hypot};
use std::f64::consts::TAU;


use super::{Style, gaussian, unit};
use crate::geometry::Point;
use crate::noise::{contrast, fbm};
use crate::palette::Palette;
use crate::validate::{
    Canvas, SEQUENTIAL_WEIGHT, SceneError, check_index, check_numbers, clipped_span,
};
use crate::raster::shade;

/// Maps an intensity buffer through `1 - exp(-max(0, v))`, as `_glow_pixels` does.
fn glow_pixels(width: usize, height: usize, palette: &Palette, glow: &[f64]) -> Vec<u8> {
    shade(width, height, palette, |x, y| {
        1.0 - (-glow[y * width + x].max(0.0)).m_exp()
    })
}

/// Visit every pixel in the segment's bounding box grown by `reach`, like
/// `_segment_box`: `visit(row, col, distance, t)` gets the distance to the
/// closest point on the segment, which lies at fraction `t` along it.
#[allow(
    clippy::manual_clamp,
    reason = "clamp() keeps NaN; Python's min(1.0, max(0.0, t)) does not"
)]
fn segment_box(
    p0: Point,
    p1: Point,
    reach: f64,
    (width, height): (usize, usize),
    mut visit: impl FnMut(usize, usize, f64, f64),
) {
    let (ax, ay) = p0;
    let (dx, dy) = (p1.0 - ax, p1.1 - ay);
    let len2 = dx * dx + dy * dy;
    // Python's int() truncates toward zero, as `as i64` does.
    let x0 = ((ax.min(p1.0) - reach) as i64).max(0);
    let x1 = (((ax.max(p1.0) + reach) as i64) + 1).min(width as i64 - 1);
    let y0 = ((ay.min(p1.1) - reach) as i64).max(0);
    let y1 = (((ay.max(p1.1) + reach) as i64) + 1).min(height as i64 - 1);
    for yy in y0..=y1 {
        for xx in x0..=x1 {
            let (fx, fy) = (xx as f64, yy as f64);
            let t = if len2 < 1e-12 {
                0.0
            } else {
                (((fx - ax) * dx + (fy - ay) * dy) / len2).max(0.0).min(1.0)
            };
            let d = py_hypot(fx - (ax + t * dx), fy - (ay + t * dy));
            visit(yy as usize, xx as usize, d, t);
        }
    }
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Flowfield {
    starts: Vec<Point>,
}

/// Stamps per flow step: enough that consecutive pixels along a step touch.
fn flow_samples(width: usize, height: usize) -> i64 {
    let scale = (width.min(height) as f64).max(1.0);
    let step = (scale / 200.0).max(0.35);
    ((step * 2.0).ceil() as i64).max(1)
}

impl Style for Flowfield {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, starts);
        // Each start walks at most 120 steps of `samples + 1` stamps.
        let samples = flow_samples(canvas.width, canvas.height) as f64;
        let stamps = self.starts.len() as f64 * 120.0 * (samples + 1.0);
        Ok(canvas.pixels() + SEQUENTIAL_WEIGHT * stamps)
    }

    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        let mut glow = vec![0.0; width * height];
        let scale = (width.min(height) as f64).max(1.0);
        let step = (scale / 200.0).max(0.35);
        let samples = flow_samples(width, height);
        let inside =
            |px: i64, py: i64| (0..width as i64).contains(&px) && (0..height as i64).contains(&py);
        for &(mut x, mut y) in &self.starts {
            for _ in 0..120 {
                let angle = fbm(x / scale * 40.0, y / scale * 40.0, seed, 3) * TAU * 2.0;
                let (ox, oy) = (x, y);
                x += angle.m_cos() * step;
                y += angle.m_sin() * step;
                let mut previous = None;
                for i in 0..=samples {
                    let fraction = i as f64 / samples as f64;
                    let px = (ox + (x - ox) * fraction).floor() as i64;
                    let py = (oy + (y - oy) * fraction).floor() as i64;
                    if inside(px, py) && previous != Some((px, py)) {
                        glow[py as usize * width + px as usize] += 0.15;
                        previous = Some((px, py));
                    }
                }
                if !(0.0 <= x && x < width as f64 && 0.0 <= y && y < height as f64) {
                    break;
                }
            }
        }
        glow_pixels(width, height, palette, &glow)
    }
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Comet {
    /// (x, y, angle, speed, brush radius)
    comets: Vec<(f64, f64, f64, f64, f64)>,
}

impl Style for Comet {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, comets);
        let mut work = canvas.pixels();
        for &(_, _, _, speed, radius) in &self.comets {
            // `steps` stamps, each a square of side 2 * reach + 1 (wrapping, not clipped).
            let steps = (speed * 2.0).ceil().max(1.0);
            let side = (2.0 * (radius * 2.0).ceil() + 1.0).max(0.0);
            work += SEQUENTIAL_WEIGHT * steps * side * side;
        }
        Ok(work)
    }

    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        let mut glow = vec![0.0; width * height];
        let (w, h) = (width as i64, height as i64);
        for &(x0, y0, angle, speed, radius) in &self.comets {
            let steps = ((speed * 2.0).ceil() as i64).max(1);
            let reach = (radius * 2.0).ceil() as i64;
            let limit = (radius * 2.0) * (radius * 2.0);
            for s in 0..steps {
                let t = (s as f64 + 0.5) / steps as f64;
                let px = x0 + angle.m_cos() * speed * t;
                let py = y0 + angle.m_sin() * speed * t;
                let (ix, iy) = (px.floor() as i64, py.floor() as i64);
                let fall = (1.0 - t * 1.6).max(0.0);
                let g = fall.m_powf(3.0) * (0.4 + 0.6 * (1.0 - t));
                for ddx in -reach..=reach {
                    for ddy in -reach..=reach {
                        let dx = (ix + ddx) as f64 - px;
                        let dy = (iy + ddy) as f64 - py;
                        let d2 = dx * dx + dy * dy;
                        if d2 <= limit {
                            let (xx, yy) = ((ix + ddx).rem_euclid(w), (iy + ddy).rem_euclid(h));
                            glow[yy as usize * width + xx as usize] +=
                                g * (-2.0 * d2 / (radius * radius)).m_exp();
                        }
                    }
                }
            }
        }
        glow_pixels(width, height, palette, &glow)
    }
}

/// (x0, y0, x1, y1, control x, control y, brush, alpha, steps)
type Stroke = (f64, f64, f64, f64, f64, f64, f64, f64, i64);

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Ink {
    strokes: Vec<Stroke>,
}

impl Style for Ink {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, strokes);
        let mut work = canvas.pixels();
        for &(_, _, _, _, _, _, brush, _, steps) in &self.strokes {
            // `steps + 1` stamps; the radius never exceeds max(brush, 0.5), and each
            // stamp's square is clipped to the canvas.
            let stamps = (steps as f64 + 1.0).max(0.0);
            let side = 2.0 * (brush.max(0.5) * 2.0).ceil() + 1.0;
            let area = side.min(canvas.width as f64) * side.min(canvas.height as f64);
            work += SEQUENTIAL_WEIGHT * stamps * area;
        }
        Ok(work)
    }

    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        let mut glow = vec![0.0; width * height];
        let (w, h) = (width as i64, height as i64);
        for &(x0, y0, x1, y1, cx, cy, brush, alpha, steps) in &self.strokes {
            for i in 0..=steps {
                let t = i as f64 / steps as f64;
                let u = 1.0 - t;
                let px = u * u * x0 + 2.0 * u * t * cx + t * t * x1;
                let py = u * u * y0 + 2.0 * u * t * cy + t * t * y1;
                let radius = (brush * (1.0 - t).m_powf(0.7)).max(0.5);
                let reach = (radius * 2.0).ceil() as i64;
                let limit = (radius * 2.0) * (radius * 2.0);
                for yy in ((py as i64) - reach).max(0)..((py as i64) + reach + 1).min(h) {
                    for xx in ((px as i64) - reach).max(0)..((px as i64) + reach + 1).min(w) {
                        let (ex, ey) = (xx as f64 - px, yy as f64 - py);
                        let d2 = ex * ex + ey * ey;
                        if d2 <= limit {
                            glow[yy as usize * width + xx as usize] +=
                                alpha * (-2.0 * d2 / (radius * radius)).m_exp() * 0.3;
                        }
                    }
                }
            }
        }
        glow_pixels(width, height, palette, &glow)
    }
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Ember {
    /// (x, y, rise, size, phase, flicker)
    embers: Vec<(f64, f64, f64, f64, f64, f64)>,
}

impl Style for Ember {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, embers);
        let mut work = canvas.pixels();
        for &(_, _, _, size, _, _) in &self.embers {
            // Every row is visited; the columns are clipped to the canvas.
            let columns = (2.0 * (size * 2.0 + 0.01 + 1.0) + 3.0).clamp(0.0, canvas.width as f64);
            work += SEQUENTIAL_WEIGHT * canvas.height as f64 * (1.0 + columns);
        }
        Ok(work)
    }

    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        let mut glow_sum = vec![0.0; width * height];
        for &(cx, y0, rise, size, phase, base_flicker) in &self.embers {
            let flicker = base_flicker * (0.55 + 0.45 * (phase + 2.0 * phase.m_sin()).m_sin());
            let cy_top = y0 - rise;
            // The horizontal falloff is exactly 0.0 once |dx| >= size * 2 + 0.01.
            let reach = size * 2.0 + 0.01 + 1.0;
            let x_lo = ((cx - reach).floor() as i64).max(0);
            let x_hi = (((cx + reach).ceil() as i64) + 1).min(width as i64);
            for yy in 0..height {
                let dy = yy as f64 - cy_top;
                if dy.abs() > rise * 3.0 {
                    continue;
                }
                let fall = (1.0 - dy.abs() / (rise * 2.5 + 0.01)).max(0.0);
                if fall < 0.02 {
                    continue;
                }
                let glow = (fall * fall) * (0.35 + 0.65 * flicker);
                for xx in x_lo..x_hi {
                    let q = (xx as f64 - cx) / ((size * 2.0) + 0.01);
                    let m = (1.0 - q * q).max(0.0);
                    glow_sum[yy * width + xx as usize] += glow * (m * m);
                }
            }
        }
        glow_pixels(width, height, palette, &glow_sum)
    }
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Torus {
    /// Polyline of the knot as (x, y, depth)
    points: Vec<(f64, f64, f64)>,
    r_glow: f64,
}

impl Style for Torus {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, points, r_glow);
        let mut work = 2.0 * canvas.pixels();
        for pair in self.points.windows(2) {
            let (p0, p1) = (pair[0], pair[1]);
            work += SEQUENTIAL_WEIGHT
                * clipped_span(p0.0, p1.0, self.r_glow, canvas.width)
                * clipped_span(p0.1, p1.1, self.r_glow, canvas.height);
        }
        Ok(work)
    }

    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        let mut glow = vec![0.0_f64; width * height];
        for pair in self.points.windows(2) {
            let (p0, p1) = (pair[0], pair[1]);
            segment_box(
                (p0.0, p0.1),
                (p1.0, p1.1),
                self.r_glow,
                (width, height),
                |yy, xx, d, t| {
                    if d < self.r_glow {
                        let depth = p0.2 + (p1.2 - p0.2) * t;
                        let value = (1.0 - d / self.r_glow) * depth;
                        let cell = &mut glow[yy * width + xx];
                        if value > *cell {
                            *cell = value;
                        }
                    }
                },
            );
        }
        shade(width, height, palette, |x, y| {
            let background = contrast(fbm(x as f64 * 0.012, y as f64 * 0.012, seed, 3), 1.6) * 0.4;
            let g = glow[y * width + x];
            if g > background { g } else { background }
        })
    }
}

/// The most curves a `lissajous` scene may have; the Python builder draws 2 or 3.
const LISSAJOUS_MAX_CURVES: usize = 8;

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Lissajous {
    /// (polyline, reach, sigma, amp) per curve
    curves: Vec<(Vec<Point>, f64, f64, f64)>,
    n_curves: f64,
}

impl Style for Lissajous {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, curves, n_curves);
        // Each curve holds a distance buffer as large as the canvas.
        if self.curves.len() > LISSAJOUS_MAX_CURVES {
            return Err(SceneError::Range {
                field: "curves",
                rule: "has more than 8 curves (each needs a canvas-sized buffer)",
            });
        }
        let mut work = canvas.per_pixel(self.curves.len() as f64);
        for (points, reach, _, _) in &self.curves {
            for pair in points.windows(2) {
                work += SEQUENTIAL_WEIGHT
                    * clipped_span(pair[0].0, pair[1].0, *reach, canvas.width)
                    * clipped_span(pair[0].1, pair[1].1, *reach, canvas.height);
            }
        }
        Ok(work)
    }

    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        let grids: Vec<Vec<f64>> = self
            .curves
            .iter()
            .map(|(points, reach, _, _)| {
                let mut min_d = vec![1e9_f64; width * height];
                for pair in points.windows(2) {
                    segment_box(pair[0], pair[1], *reach, (width, height), |yy, xx, d, _| {
                        let cell = &mut min_d[yy * width + xx];
                        if d < *cell {
                            *cell = d;
                        }
                    });
                }
                min_d
            })
            .collect();
        shade(width, height, palette, |x, y| {
            let mut v = 0.0;
            for (grid, (_, _, sigma, amp)) in grids.iter().zip(&self.curves) {
                let d = grid[y * width + x];
                if d < 1e8 {
                    v += gaussian(d, *sigma) * amp;
                }
            }
            unit(v / (self.n_curves * 0.9))
        })
    }
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Constellations {
    /// (x, y, radius, brightness)
    stars: Vec<(f64, f64, f64, f64)>,
    /// (first star, second star, distance) for stars joined by a line
    pairs: Vec<(usize, usize, f64)>,
}

/// Where a Gaussian of `GAUSSIAN_ZERO` standard deviations underflows to exactly 0.0.
fn gaussian_zero() -> f64 {
    (2.0_f64 * 746.0).sqrt()
}

impl Style for Constellations {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, stars, pairs);
        let mut work = canvas.per_pixel(self.stars.len() as f64);
        for &(i, j, dist) in &self.pairs {
            check_index("pairs", i, self.stars.len())?;
            check_index("pairs", j, self.stars.len())?;
            // `steps + 1` points along the line.
            work += SEQUENTIAL_WEIGHT * ((dist / 1.2).trunc().max(1.0) + 1.0);
        }
        Ok(work)
    }

    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        let mut lines = vec![0.0_f64; width * height];
        for &(i, j, dist) in &self.pairs {
            let ((sx, sy, _, _), (ex, ey, _, _)) = (self.stars[i], self.stars[j]);
            let steps = ((dist / 1.2) as i64).max(1);
            for k in 0..=steps {
                let t = k as f64 / steps as f64;
                let ix = (sx + (ex - sx) * t) as i64;
                let iy = (sy + (ey - sy) * t) as i64;
                if (0..width as i64).contains(&ix) && (0..height as i64).contains(&iy) {
                    let frac = k as f64 / steps as f64;
                    let falloff = (-(frac * frac) / 2.0).m_exp();
                    lines[iy as usize * width + ix as usize] += falloff * 0.9;
                }
            }
        }
        let zero = gaussian_zero();
        shade(width, height, palette, |x, y| {
            let (fx, fy) = (x as f64, y as f64);
            let mut v = 0.0;
            for &(sx, sy, radius, brightness) in &self.stars {
                // Beyond this vertical reach the contribution is exactly 0.0.
                if (fy - sy).abs() > radius * zero + 1.0 {
                    continue;
                }
                v += gaussian(py_hypot(fx - sx, fy - sy), radius) * brightness;
            }
            v += lines[y * width + x];
            unit(1.0 - (-v).m_exp())
        })
    }
}

/// Work of one orbit step: four libm calls in a chain no thread can share,
/// measured at about 16 brush stamps.
const ATTRACTOR_STEP_WORK: f64 = 16.0 * SEQUENTIAL_WEIGHT;

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Attractor {
    params: (f64, f64, f64, f64),
    mid: (f64, f64),
    scale: f64,
    points: usize,
}

impl Style for Attractor {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, params, mid, scale);
        Ok(canvas.pixels() + ATTRACTOR_STEP_WORK * (self.points as f64 + 100.0))
    }

    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        let (a, b, c, d) = self.params;
        let (cx, cy) = ((width as f64 - 1.0) / 2.0, (height as f64 - 1.0) / 2.0);
        let mut counts = vec![0u32; width * height];
        let (mut x, mut y) = (0.1_f64, 0.1_f64);
        for i in 0..self.points.saturating_add(100) {
            (x, y) = ((a * y).m_sin() + c * (a * x).m_cos(), (b * x).m_sin() + d * (b * y).m_cos());
            if i >= 100 {
                let ix = (cx + (x - self.mid.0) * self.scale + 0.5) as i64;
                let iy = (cy + (y - self.mid.1) * self.scale + 0.5) as i64;
                if (0..width as i64).contains(&ix) && (0..height as i64).contains(&iy) {
                    counts[iy as usize * width + ix as usize] += 1;
                }
            }
        }
        let top = f64::from(counts.iter().copied().max().unwrap_or(0));
        let norm = top.m_ln_1p().max(1e-9);
        shade(width, height, palette, |px, py| {
            f64::from(counts[py * width + px]).m_ln_1p() / norm
        })
    }
}
