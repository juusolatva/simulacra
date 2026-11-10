//! Brush-, trail- and curve-based styles.
//!
//! These accumulate contributions in a fixed order, and floating-point
//! addition is not associative, so stamping runs sequentially in exactly the
//! order `simulacra.py` uses. Only the final per-pixel shading is parallel.

use crate::math::{Libm, py_hypot};
use std::f64::consts::{PI, TAU};

use super::fields::diagonal;
use super::{ROUGH_TAU, Style, circle_offset, drift_stars, gaussian, last_index, loop_angle, unit};
use crate::geometry::{Point, py_sum};
use crate::noise::{contrast, fbm};
use crate::palette::Palette;
use crate::random::PyRandom;
use crate::raster::shade;
use crate::scenes::{BuildScene, SceneParams};
use crate::validate::{
    Canvas, SEQUENTIAL_WEIGHT, SceneError, check_index, check_numbers, clipped_span,
};

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
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
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

impl BuildScene for Flowfield {
    /// `_flowfield_scene`: 40 to 80 trail starts, each circling a small radius.
    fn build(params: &SceneParams<'_>) -> Self {
        let (width, height) = (params.width, params.height);
        let mut rng = PyRandom::new(params.seed);
        let reach = 0.04 * width.min(height) as f64;
        let (last_x, last_y) = (last_index(width), last_index(height));
        let count = rng.randint(40, 80);
        let starts = (0..count as usize)
            .map(|index| {
                let (x, y) = (rng.uniform(0.0, last_x), rng.uniform(0.0, last_y));
                let (dx, dy) = circle_offset(index, reach, params.phase);
                (x + dx, y + dy)
            })
            .collect();
        Self { starts }
    }
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Comet {
    /// (x, y, angle, speed, brush radius)
    comets: Vec<(f64, f64, f64, f64, f64)>,
}

impl Style for Comet {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, comets);
        let mut work = canvas.pixels();
        for &(_, _, _, speed, radius) in &self.comets {
            // `steps` stamps, each a square of side 2 * reach + 1 (wrapping, not
            // clipped). A step costs its libm calls even when the square is
            // empty (a negative radius), so it counts on its own as well.
            let steps = (speed * 2.0).ceil().max(1.0);
            let side = (2.0 * (radius * 2.0).ceil() + 1.0).max(0.0);
            work += SEQUENTIAL_WEIGHT * steps * (1.0 + side * side);
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

impl BuildScene for Comet {
    /// `_comet_scene`: 3 to 6 comets whose heads circle a point.
    fn build(params: &SceneParams<'_>) -> Self {
        let (width, height) = (params.width, params.height);
        let mut rng = PyRandom::new(params.seed);
        let (w, h) = (width as f64, height as f64);
        let reach = 0.12 * width.min(height) as f64;
        let count = rng.randint(3, 6);
        let comets = (0..count as usize)
            .map(|index| {
                let (x, y) = (rng.uniform(0.0, w - 1.0), rng.uniform(0.0, h - 1.0));
                let angle = rng.uniform(0.0, 2.0 * PI);
                let speed = rng.uniform(0.2, 0.6) * w;
                let radius = rng.uniform(0.8, 3.0);
                let (dx, dy) = circle_offset(index, reach, params.phase);
                (x + dx, y + dy, angle, speed, radius)
            })
            .collect();
        Self { comets }
    }
}

/// (x0, y0, x1, y1, control x, control y, brush, alpha, steps). `steps` is a
/// whole number held as a float: Python's `max(8, ceil(length * 2))` is an
/// unbounded int, which outgrows `i64` on sides near 2**64 but is always a
/// float's exact value.
type Stroke = (f64, f64, f64, f64, f64, f64, f64, f64, f64);

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Ink {
    strokes: Vec<Stroke>,
}

impl Style for Ink {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, strokes);
        if self.strokes.iter().any(|s| s.8 < 1.0 || s.8.fract() != 0.0) {
            return Err(SceneError::Range {
                field: "strokes",
                rule: "has a stroke whose steps are not a whole number of at least 1",
            });
        }
        let mut work = canvas.pixels();
        for &(_, _, _, _, _, _, brush, _, steps) in &self.strokes {
            // `steps + 1` stamps; the radius never exceeds max(brush, 0.5), and each
            // stamp's square is clipped to the canvas.
            let stamps = steps + 1.0;
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
            // A whole number of at most 2**40 (validated), so the cast is exact.
            for i in 0..=steps as i64 {
                let t = i as f64 / steps;
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

impl BuildScene for Ink {
    /// `_ink_scene`: 5 to 9 strokes whose control points circle, so the curves
    /// flex while their ends stay put.
    fn build(params: &SceneParams<'_>) -> Self {
        let (width, height) = (params.width, params.height);
        let mut rng = PyRandom::new(params.seed);
        let small = width.min(height) as f64;
        let (last_x, last_y) = (last_index(width), last_index(height));
        let count = rng.randint(5, 9);
        let strokes = (0..count as usize)
            .map(|index| {
                let (x0, y0) = (rng.uniform(0.0, last_x), rng.uniform(0.0, last_y));
                let (x1, y1) = (rng.uniform(0.0, last_x), rng.uniform(0.0, last_y));
                let (cx, cy) = (rng.uniform(0.0, last_x), rng.uniform(0.0, last_y));
                let brush = (small * rng.uniform(0.008, 0.025)).max(0.8);
                let alpha = rng.uniform(0.35, 0.9);
                let (dx, dy) = circle_offset(index, 0.15 * small, params.phase);
                let (cx, cy) = (cx + dx, cy + dy);
                let length = py_hypot(cx - x0, cy - y0) + py_hypot(x1 - cx, y1 - cy);
                let steps = (length * 2.0).ceil().max(8.0);
                (x0, y0, x1, y1, cx, cy, brush, alpha, steps)
            })
            .collect();
        Self { strokes }
    }
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
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

impl BuildScene for Ember {
    /// `_ember_scene`: 60 to 110 embers that sway on small circles while their
    /// flicker runs one cycle per loop.
    fn build(params: &SceneParams<'_>) -> Self {
        let (width, height, phase) = (params.width, params.height, params.phase);
        let mut rng = PyRandom::new(params.seed);
        let (w, h) = (width as f64, height as f64);
        let sway = 0.03 * width.min(height) as f64;
        let count = rng.randint(60, 110);
        let embers = (0..count as usize)
            .map(|index| {
                let x = rng.uniform(0.0, w - 1.0);
                let y = rng.uniform(0.0, h - 1.0);
                let rise = rng.uniform(0.15, 0.55) * h;
                let size = rng.uniform(0.6, 2.4);
                let flicker_phase = rng.uniform(0.0, ROUGH_TAU);
                let flicker = rng.uniform(0.5, 1.0);
                let (dx, dy) = circle_offset(index, sway, phase);
                (
                    x + dx,
                    y + dy,
                    rise,
                    size,
                    flicker_phase + loop_angle(phase, 1),
                    flicker,
                )
            })
            .collect();
        Self { embers }
    }
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
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

/// A sampled curve point whose first two values are screen x and y.
trait ScreenPoint: Copy {
    fn xy(self) -> Point;
}

impl ScreenPoint for Point {
    fn xy(self) -> Point {
        self
    }
}

impl ScreenPoint for (f64, f64, f64) {
    fn xy(self) -> Point {
        (self.0, self.1)
    }
}

/// Python's `_sample_by_length(point_at, spacing, closed)`: samples `point_at(u)`
/// for `u` in [0, 1] about `spacing` pixels apart, measuring the length on 512
/// coarse segments with Python's compensated `sum()`. A closed curve repeats
/// its first point at the end; an open one ends at `point_at(1.0)`.
fn sample_by_length<P: ScreenPoint>(
    point_at: impl Fn(f64) -> P,
    spacing: f64,
    closed: bool,
) -> Vec<P> {
    let coarse: Vec<Point> = (0..=512)
        .map(|i| point_at(f64::from(i) / 512.0).xy())
        .collect();
    let length = py_sum(
        coarse
            .windows(2)
            .map(|pair| py_hypot(pair[1].0 - pair[0].0, pair[1].1 - pair[0].1)),
    );
    // A float-to-int cast saturates, and the count is a whole number of at most
    // a few times the canvas side.
    let count = ((length / spacing).ceil() as usize).max(64);
    let mut points: Vec<P> = (0..count)
        .map(|i| point_at(i as f64 / count as f64))
        .collect();
    points.push(if closed { points[0] } else { point_at(1.0) });
    points
}

/// Python's `min(values)`: the first of the smallest values.
fn py_min(values: impl IntoIterator<Item = f64>) -> f64 {
    values
        .into_iter()
        .reduce(|best, v| if v < best { v } else { best })
        .expect("values")
}

/// Python's `max(values)`: the first of the largest values.
fn py_max(values: impl IntoIterator<Item = f64>) -> f64 {
    values
        .into_iter()
        .reduce(|best, v| if v > best { v } else { best })
        .expect("values")
}

/// Python's `_torus_knot_point(tube, a, b, u)`: the (a, b) torus knot in model
/// space at `u` in [0, 1).
fn torus_knot_point(tube: f64, (a, b): (f64, f64), u: f64) -> (f64, f64, f64) {
    let t = TAU * u;
    let ct = (b * t).m_cos();
    (
        (2.0 + tube * ct) * (a * t).m_cos(),
        (2.0 + tube * ct) * (a * t).m_sin(),
        tube * (b * t).m_sin(),
    )
}

/// Python's `_turned(point, cosine, sine)`: a turn about the vertical axis.
fn turned((x, y, z): (f64, f64, f64), cosine: f64, sine: f64) -> (f64, f64, f64) {
    (x * cosine + z * sine, y, z * cosine - x * sine)
}

impl BuildScene for Torus {
    /// `_torus_scene`: one of four knots, turned once about the vertical axis
    /// per loop; the centre and scale come from the unturned knot and the depth
    /// range from the turned one.
    fn build(params: &SceneParams<'_>) -> Self {
        let (width, height) = (params.width, params.height);
        let mut rng = PyRandom::new(params.seed);
        let tube = rng.uniform(0.6, 1.2);
        let knot = *rng.choice(&[(2.0, 3.0), (3.0, 2.0), (3.0, 5.0), (5.0, 2.0)]);
        let angle = loop_angle(params.phase, 1);
        let (cosine, sine) = (angle.m_cos(), angle.m_sin());
        let model: Vec<(f64, f64, f64)> = (0..2200)
            .map(|i| torus_knot_point(tube, knot, f64::from(i) / 2200.0))
            .collect();
        let (x_lo, x_hi) = (
            py_min(model.iter().map(|p| p.0)),
            py_max(model.iter().map(|p| p.0)),
        );
        let (y_lo, y_hi) = (
            py_min(model.iter().map(|p| p.1)),
            py_max(model.iter().map(|p| p.1)),
        );
        let (cx, cy) = ((x_lo + x_hi) / 2.0, (y_lo + y_hi) / 2.0);
        let zs: Vec<f64> = model.iter().map(|&p| turned(p, cosine, sine).2).collect();
        let (z_lo, z_hi) = (py_min(zs.iter().copied()), py_max(zs.iter().copied()));
        let span = py_max([x_hi - x_lo, y_hi - y_lo, 1e-9]);
        let small = width.min(height) as f64;
        let s = small * 0.85 / span;
        let r_glow = (small * 0.016).max(2.0);
        let (half_x, half_y) = (last_index(width) / 2.0, last_index(height) / 2.0);
        let depth_span = (z_hi - z_lo).max(1e-9);
        let screen = |u: f64| {
            let (x, y, z) = turned(torus_knot_point(tube, knot, u), cosine, sine);
            (
                (x - cx) * s + half_x,
                (cy - y) * s + half_y,
                0.45 + 0.55 * (z - z_lo) / depth_span,
            )
        };
        let points = sample_by_length(screen, (r_glow * 0.5).max(1.0), true);
        Self { points, r_glow }
    }
}

/// The most curves a `lissajous` scene may have; the Python builder draws 2 or 3.
const LISSAJOUS_MAX_CURVES: usize = 8;

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
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

impl BuildScene for Lissajous {
    /// `_lissajous_scene`: 2 or 3 curves whose two phase offsets turn in
    /// opposite directions over a loop, each sampled as an open polyline.
    fn build(params: &SceneParams<'_>) -> Self {
        let (width, height, phase) = (params.width, params.height, params.phase);
        let mut rng = PyRandom::new(params.seed);
        let small = width.min(height) as f64;
        let n_curves = rng.randint(2, 3);
        // (ax, ay, fx, fy, px, py, amp, sigma), drawn in that order.
        let specs: Vec<[f64; 8]> = (0..n_curves)
            .map(|_| {
                [
                    rng.uniform(0.25, 0.8),
                    rng.uniform(0.25, 0.8),
                    rng.uniform(1.0, 2.5),
                    rng.uniform(2.0, 4.5),
                    rng.uniform(0.0, ROUGH_TAU) + loop_angle(phase, -1),
                    rng.uniform(0.0, ROUGH_TAU) + loop_angle(phase, 1),
                    rng.uniform(0.5, 1.0),
                    (small * rng.uniform(0.008, 0.02)).max(0.8),
                ]
            })
            .collect();
        let (cx, cy) = (last_index(width) / 2.0, last_index(height) / 2.0);
        let curves = specs
            .into_iter()
            .map(|[ax, ay, fx, fy, px, py, amp, sigma]| {
                let point_at = |u: f64| {
                    let t = TAU * u;
                    (
                        cx + cx * ax * (fx * t + px).m_sin(),
                        cy - cy * ay * (fy * t + py).m_sin(),
                    )
                };
                let reach = (sigma * 3.0).ceil().max(2.0);
                // Non-integer frequencies leave the curve open over one period.
                let points = sample_by_length(point_at, (reach / 2.0).max(1.0), false);
                (points, reach, sigma, amp)
            })
            .collect();
        Self {
            curves,
            n_curves: n_curves as f64,
        }
    }
}

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
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
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
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
            (x, y) = (
                (a * y).m_sin() + c * (a * x).m_cos(),
                (b * x).m_sin() + d * (b * y).m_cos(),
            );
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

/// Python's `_ATTRACTOR_MAX_POINTS`: the most orbit points a render plots.
const ATTRACTOR_MAX_POINTS: u128 = 24_000_000;

/// Python's `_ATTRACTOR_REACH`: the radius the parameters circle with.
const ATTRACTOR_REACH: f64 = 0.06;

/// Python's `_ATTRACTOR_TABLE`, entry for entry (see its comment and
/// "Attractor parameter table" in PLAN.md for how they were chosen; add
/// entries only with the same offline checks, in both places).
const ATTRACTOR_TABLE: [(f64, f64, f64, f64); 256] = [
    (-1.763, 1.233, -1.576, 1.14),
    (-1.687, -1.998, 0.524, 1.355),
    (-1.806, -1.916, 1.093, 0.348),
    (1.713, -1.582, 1.49, -0.901),
    (1.764, -1.84, -1.299, 1.251),
    (-1.528, 1.821, 1.252, -1.19),
    (-1.748, -1.762, -0.378, 1.005),
    (-1.675, -1.755, 1.138, -1.458),
    (-1.776, -1.653, 1.447, -1.578),
    (-1.755, 1.501, -1.227, 1.425),
    (-1.652, -1.579, -1.41, -1.311),
    (-1.708, 1.487, -0.854, 1.208),
    (-1.93, 1.806, -0.659, 1.004),
    (1.889, -1.406, -1.595, 0.745),
    (-1.954, 1.97, -0.982, 1.565),
    (1.816, -1.931, -0.572, -1.532),
    (1.858, 1.959, -1.485, 1.275),
    (-1.546, -1.775, -1.022, -1.548),
    (1.518, -1.499, -1.067, -0.867),
    (1.395, -1.937, 0.659, -1.559),
    (-1.977, -1.933, -1.11, 1.597),
    (-1.983, -1.88, -0.34, -0.977),
    (1.973, -1.63, -0.962, -1.449),
    (1.618, 1.864, -1.596, -1.502),
    (1.638, 1.843, 1.4, -1.398),
    (-1.894, 1.564, 1.419, 1.365),
    (-1.828, -1.377, -0.78, -1.071),
    (-1.598, 1.876, 1.239, 0.566),
    (1.885, -1.656, -1.478, -1.067),
    (-1.201, -1.914, 1.561, 1.448),
    (-1.707, 1.498, -1.569, -1.252),
    (-1.949, -1.956, -0.504, -1.574),
    (-1.569, 1.533, 1.042, 1.165),
    (-1.724, 1.917, -1.099, -0.531),
    (1.714, -1.976, -1.028, 1.501),
    (1.84, -1.593, -1.486, 1.575),
    (1.822, -1.735, 1.108, 1.321),
    (-1.329, 1.444, -1.033, 1.099),
    (1.927, -1.507, 1.495, 1.484),
    (1.914, -1.993, -1.386, -1.063),
    (-1.9, -2.0, 1.18, 1.216),
    (1.39, -1.311, -0.929, -1.554),
    (1.799, -1.505, 1.039, 0.842),
    (1.961, -1.695, -1.21, 0.183),
    (-1.873, -1.941, -1.144, 1.262),
    (1.493, -1.465, 1.133, -1.315),
    (-1.822, 1.761, -1.258, 1.15),
    (-1.313, -1.68, -1.311, -1.591),
    (1.334, -1.447, 1.556, -1.579),
    (-1.746, 1.978, 0.993, -1.582),
    (-1.601, -1.19, 0.922, -1.448),
    (-1.58, 1.97, -0.975, 0.881),
    (-1.367, 1.978, -1.213, 1.448),
    (1.528, 1.934, -1.493, -1.235),
    (1.584, -1.645, -1.573, 1.302),
    (-1.802, 1.947, 0.994, 1.125),
    (1.342, -1.196, 1.425, -1.087),
    (-1.673, 1.996, 1.056, 1.551),
    (1.69, -1.822, 0.726, -1.215),
    (-1.93, 1.966, 1.002, 0.105),
    (1.532, 1.554, 1.207, 1.118),
    (1.821, 1.871, 1.138, 0.436),
    (-1.467, 1.419, 0.919, -1.466),
    (-1.998, 1.75, 1.026, 0.873),
    (-1.245, -1.889, -1.583, 1.586),
    (-1.53, -1.867, -1.595, -1.558),
    (-1.858, -1.346, -1.555, 1.467),
    (-1.994, 1.759, -0.984, -0.986),
    (1.592, -1.955, 1.195, -1.521),
    (1.879, 1.978, -1.346, -1.347),
    (-1.99, -1.674, -1.161, -1.453),
    (1.782, -1.548, 1.45, -1.533),
    (1.865, -1.48, -1.294, -1.434),
    (-1.979, 1.962, -1.428, -0.85),
    (1.997, 1.233, 1.582, 1.357),
    (-1.959, -1.069, 1.321, -1.408),
    (-1.934, 1.799, 1.498, 0.729),
    (1.514, -1.997, 0.766, 1.455),
    (1.584, -1.605, -1.065, 0.903),
    (-1.92, -1.983, -1.404, 1.295),
    (-1.704, 1.482, 1.596, -1.518),
    (1.95, 1.938, -1.517, -0.713),
    (1.804, -1.828, -1.592, -1.388),
    (-1.965, 1.846, -1.132, -0.004),
    (-1.962, -1.992, 0.739, 1.434),
    (1.595, -1.735, -1.187, -1.127),
    (-1.323, 1.507, 1.365, 0.928),
    (-1.926, -1.863, -1.515, 0.644),
    (-1.457, -1.507, 0.436, -1.404),
    (-1.798, 1.995, -1.572, 1.458),
    (-1.424, 1.975, -0.408, 1.511),
    (1.726, -1.643, 0.288, -1.262),
    (1.407, 1.757, 1.153, 0.829),
    (1.758, -1.951, -1.123, -1.527),
    (-1.971, 1.68, -1.527, 0.77),
    (1.777, 1.953, 1.592, 1.067),
    (-1.633, -1.939, 1.576, 1.386),
    (-1.966, -1.862, -1.369, -1.068),
    (1.949, 1.635, -1.385, 0.947),
    (-1.996, -1.897, -1.441, -1.423),
    (-1.637, 1.99, -0.72, 1.548),
    (-1.272, 1.484, 1.461, 1.525),
    (1.452, -1.455, -0.501, -1.524),
    (1.863, 1.73, 1.325, 1.262),
    (1.923, 1.967, -1.011, 0.383),
    (1.252, -1.982, -0.802, -1.5),
    (-1.997, -1.986, 0.955, -1.155),
    (1.944, -1.462, 1.175, -1.003),
    (-1.753, 1.607, 0.812, -1.597),
    (-1.724, 1.482, 1.512, 0.867),
    (1.351, -1.428, -1.188, 1.571),
    (-1.999, -1.872, 1.044, -1.516),
    (1.792, -1.526, 0.932, 1.586),
    (1.106, 1.382, 1.416, -1.489),
    (1.668, 1.648, -0.333, 1.393),
    (-1.87, -1.831, 1.593, 1.13),
    (-1.526, -1.907, 0.827, -1.494),
    (1.691, -1.283, -1.591, -1.531),
    (-1.558, 1.745, 1.227, 0.313),
    (-1.954, -1.624, -1.165, 0.535),
    (-1.958, 1.364, -1.517, -1.104),
    (-1.904, -1.61, -1.284, -0.779),
    (1.697, 1.127, 0.813, 1.597),
    (-1.996, 1.745, 1.142, -1.552),
    (1.899, -1.999, 1.078, -0.262),
    (1.758, -1.703, 1.27, -0.322),
    (1.892, 1.795, 1.423, -1.176),
    (-1.958, -1.679, 1.519, -1.084),
    (1.458, -1.724, 1.586, -1.493),
    (1.95, -1.932, -0.911, 1.091),
    (-1.933, 1.93, 0.733, -1.505),
    (-1.979, -1.373, 0.908, -0.899),
    (1.298, -1.995, 1.414, -1.515),
    (-1.566, -1.235, -1.145, 1.497),
    (-1.915, -1.449, 1.09, -1.406),
    (-1.906, -1.739, 1.505, -0.77),
    (-1.655, 1.875, -1.212, 1.444),
    (-1.712, 1.951, 0.539, 0.897),
    (1.753, 1.969, 0.505, 1.423),
    (1.843, -1.97, 0.467, -1.307),
    (1.876, -1.199, 1.54, 0.895),
    (1.925, 1.302, -1.495, -1.554),
    (1.712, 1.908, 1.485, 1.592),
    (-1.39, 1.475, -0.578, 1.538),
    (-1.31, 1.976, -0.867, 1.508),
    (-1.635, 1.967, 0.878, -1.29),
    (1.958, 1.52, 0.676, 0.838),
    (1.428, 1.038, 1.572, 1.508),
    (-1.948, 1.679, -0.989, 0.277),
    (-1.523, -1.605, 1.435, -1.554),
    (-1.561, -1.835, 0.43, -1.552),
    (-1.912, -1.786, -0.876, 0.409),
    (-1.497, 1.519, -1.586, 1.283),
    (1.686, 1.837, 0.888, 1.588),
    (1.411, -1.993, -0.763, 1.555),
    (-1.975, 1.995, -0.991, 0.781),
    (-1.962, 1.511, -1.439, -0.539),
    (-1.891, 1.587, -0.916, 0.918),
    (1.544, 1.763, 1.564, 1.325),
    (1.477, 1.859, -1.038, 1.569),
    (-1.4, 1.489, -1.131, -0.994),
    (-1.201, -1.493, -1.515, 1.201),
    (1.85, 1.91, -0.654, -1.157),
    (-1.459, -1.89, 1.57, -1.202),
    (1.704, 1.06, 1.499, 1.571),
    (1.727, 1.25, 1.581, -1.488),
    (1.861, -1.443, -1.559, 1.089),
    (-1.372, 1.524, -1.317, -1.269),
    (-1.258, 1.911, -0.863, -1.535),
    (1.292, -1.318, -1.155, 1.285),
    (1.911, -1.731, 1.138, -0.928),
    (1.777, -1.488, -0.998, -0.86),
    (1.433, 1.698, -1.218, -0.877),
    (-1.654, -1.682, 1.524, 1.506),
    (1.987, 1.849, -1.254, -1.046),
    (-1.452, -1.488, -1.107, 1.296),
    (-1.604, 1.761, 0.167, 1.342),
    (1.773, -1.195, 1.581, -1.478),
    (1.552, -1.763, -0.789, -1.549),
    (-1.607, -1.651, 1.326, -0.761),
    (-1.936, 1.962, -0.645, 1.548),
    (1.968, 1.921, 0.909, -1.363),
    (-1.968, 1.944, 1.506, 1.583),
    (1.729, 1.277, -1.332, 1.355),
    (1.683, -1.48, -0.869, -1.577),
    (1.933, 1.929, 0.961, 0.093),
    (1.927, -1.39, 1.371, 0.62),
    (-1.4, -1.45, -1.101, -0.428),
    (-1.678, -1.768, 0.056, -1.295),
    (-1.993, -1.878, -0.894, -1.088),
    (1.761, -1.988, -0.885, 0.624),
    (1.358, -1.364, 0.898, 1.1),
    (-1.315, -1.732, 1.459, -0.632),
    (1.521, -1.98, -1.301, 1.554),
    (-1.489, 1.766, -1.4, -0.556),
    (-1.989, 1.773, 1.42, 1.018),
    (1.651, 1.861, 0.742, 1.096),
    (1.963, -1.583, -1.232, 1.589),
    (-1.399, 1.818, -1.124, -1.59),
    (-1.679, -1.589, 1.37, 0.206),
    (-1.931, 1.946, 1.107, -0.635),
    (-1.968, -1.878, 0.917, 1.069),
    (1.348, -1.4, 1.56, -0.83),
    (-1.41, -1.75, -0.939, -0.277),
    (-1.569, 1.828, -1.353, 1.127),
    (-1.648, -1.214, 0.661, -1.448),
    (1.913, 1.952, 1.001, -1.089),
    (1.974, 1.405, -0.901, -1.091),
    (1.782, 1.445, 1.027, 1.512),
    (1.691, 1.281, 1.35, 1.215),
    (-1.399, -1.053, 1.51, 1.374),
    (-1.844, -1.685, 0.984, -0.704),
    (1.874, 1.896, 1.034, -0.243),
    (-1.987, -1.953, 1.427, 1.561),
    (-1.363, -1.412, -0.984, -1.026),
    (-1.974, -1.988, 1.544, 0.44),
    (-1.658, -1.992, -1.361, -0.664),
    (-1.725, 1.536, -1.357, -1.573),
    (1.623, 1.959, 1.223, 1.589),
    (-1.748, 1.468, -0.599, 1.383),
    (1.712, -1.542, 1.238, 1.578),
    (1.495, -1.747, 1.439, 1.163),
    (1.608, -1.988, 1.556, -1.543),
    (-1.483, 1.384, 1.594, 1.226),
    (1.93, 1.68, 1.592, -1.542),
    (1.999, -1.484, -1.492, -0.549),
    (1.901, 1.519, -1.587, -1.245),
    (-1.738, 1.986, -1.117, -1.121),
    (1.737, 1.971, 0.48, -1.386),
    (-1.065, -1.976, -1.578, -1.425),
    (-1.334, -0.95, -1.317, -1.437),
    (-1.924, -1.996, -0.81, -1.396),
    (-1.806, -1.4, -1.063, -1.104),
    (1.554, 1.738, 1.307, 0.333),
    (-1.416, 1.232, -1.464, 1.27),
    (1.53, 1.142, 1.129, 1.596),
    (1.909, -1.977, 0.515, -0.717),
    (1.941, 1.722, 1.45, -0.499),
    (-1.827, -1.917, 1.552, -1.422),
    (-1.603, 1.789, -1.178, -0.22),
    (1.754, 1.995, -0.394, -1.006),
    (-1.735, 1.45, -0.175, 1.499),
    (1.786, 1.992, -0.851, 1.495),
    (1.869, -1.989, 1.294, -0.861),
    (-1.867, -1.618, 1.575, 0.835),
    (1.178, 1.958, 1.326, 1.282),
    (1.576, 1.677, -1.577, 1.494),
    (1.702, 1.768, 0.182, -1.295),
    (-1.213, 1.314, 0.972, -1.463),
    (-1.86, 1.92, 0.45, 1.217),
    (1.956, -1.27, -1.269, -0.934),
    (1.898, -1.562, -1.547, -1.445),
    (-1.858, 1.986, -1.564, 1.18),
    (1.653, -1.743, 0.761, 0.953),
    (-1.28, 1.541, 1.101, 1.172),
    (1.603, 1.484, 1.591, 1.472),
];

/// Python's `_clifford_step`: one step of the Clifford map.
fn clifford_step((x, y): Point, (a, b, c, d): (f64, f64, f64, f64)) -> Point {
    (
        (a * y).m_sin() + c * (a * x).m_cos(),
        (b * x).m_sin() + d * (b * y).m_cos(),
    )
}

/// Python's `_clifford_bounds`: `(min_x, max_x, min_y, max_y)` of a 3,000-point
/// orbit after 100 warm-up steps.
fn clifford_bounds(params: (f64, f64, f64, f64)) -> (f64, f64, f64, f64) {
    let mut point = (0.1, 0.1);
    let mut orbit = Vec::with_capacity(3000);
    for i in 0..3100 {
        point = clifford_step(point, params);
        if i >= 100 {
            orbit.push(point);
        }
    }
    (
        py_min(orbit.iter().map(|p| p.0)),
        py_max(orbit.iter().map(|p| p.0)),
        py_min(orbit.iter().map(|p| p.1)),
        py_max(orbit.iter().map(|p| p.1)),
    )
}

impl BuildScene for Attractor {
    /// `_attractor_scene`: a table entry (mirrored for half the seeds) whose
    /// parameters circle over a loop, framed by the bounds of a short orbit.
    fn build(params: &SceneParams<'_>) -> Self {
        let (width, height) = (params.width, params.height);
        // `_attractor_base`: only integer draws, so no libm changes the choice.
        let mut rng = PyRandom::new(params.seed);
        let (a, b, c, d) = *rng.choice(&ATTRACTOR_TABLE);
        let base = if rng.getrandbits(1) == 1 {
            (b, a, d, c)
        } else {
            (a, b, c, d)
        };
        // `_attractor_params`
        let a_off = circle_offset(0, ATTRACTOR_REACH, params.phase);
        let c_off = circle_offset(1, ATTRACTOR_REACH, params.phase);
        let moved = (
            base.0 + a_off.0,
            base.1 + a_off.1,
            base.2 + c_off.0,
            base.3 + c_off.1,
        );
        let (min_x, max_x, min_y, max_y) = clifford_bounds(moved);
        let span = py_max([max_x - min_x, max_y - min_y, 1e-6]);
        // Python's exact `min(6 * width * height, _ATTRACTOR_MAX_POINTS)`; the
        // product saturates, which only matters far above the cap.
        let points = (6 * width as u128)
            .saturating_mul(height as u128)
            .min(ATTRACTOR_MAX_POINTS) as usize;
        Self {
            params: moved,
            mid: ((min_x + max_x) / 2.0, (min_y + max_y) / 2.0),
            scale: 0.9 * width.min(height) as f64 / span,
            points,
        }
    }
}

impl BuildScene for Constellations {
    /// `_constellation_scene`: 28 to 48 stars, pairs chosen from the starting
    /// positions, then the stars drift and each line is measured where its
    /// stars now are.
    fn build(params: &SceneParams<'_>) -> Self {
        let (width, height) = (params.width, params.height);
        let mut rng = PyRandom::new(params.seed);
        let count = rng.randint(28, 48);
        let (last_x, last_y) = (last_index(width), last_index(height));
        let stars: Vec<(f64, f64, f64, f64)> = (0..count)
            .map(|_| {
                (
                    rng.uniform(0.0, last_x),
                    rng.uniform(0.0, last_y),
                    rng.uniform(0.7, 2.2),
                    rng.uniform(0.45, 1.0),
                )
            })
            .collect();
        let max_dist = diagonal(width, height) * 0.32;
        let mut pairs = Vec::new();
        for (i, &(xi, yi, _, _)) in stars.iter().enumerate() {
            for (j, star) in stars.iter().enumerate().skip(i + 1) {
                if py_hypot(star.0 - xi, star.1 - yi) <= max_dist {
                    pairs.push((i, j));
                }
            }
        }
        let stars = drift_stars(stars, 0.025 * width.min(height) as f64, params.phase);
        let pairs = pairs
            .into_iter()
            .map(|(i, j)| {
                (
                    i,
                    j,
                    py_hypot(stars[j].0 - stars[i].0, stars[j].1 - stars[i].1),
                )
            })
            .collect();
        Self { stars, pairs }
    }
}
