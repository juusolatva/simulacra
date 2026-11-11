//! Polygon-, segment- and ring-based styles.

use crate::math::{Libm, py_hypot};
use std::f64::consts::{PI, TAU};

use super::{ROUGH_TAU, Style, center, circle_offset, gaussian, loop_angle, small_side, unit};
use crate::geometry::{Point, Segment, point_in_poly, poly_min_dist, py_sum, seg_hit};
use crate::random::PyRandom;
use crate::scenes::{BuildScene, SceneParams};

/// (polygon, ridge width, gain)
type PrismLayer = (Vec<Point>, f64, f64);
use crate::noise::polar_noise;
use crate::palette::Palette;
use crate::raster::shade;
use crate::validate::{Canvas, SceneError, check_numbers, check_polygons};

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Circuit {
    /// (node pads, segments) per trace
    traces: Vec<(Vec<Point>, Vec<Segment>)>,
}

impl Style for Circuit {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, traces);
        // Every trace costs work per pixel, even one without nodes or segments.
        let per_pixel: usize = self
            .traces
            .iter()
            .map(|(nodes, segments)| 1 + nodes.len() + segments.len())
            .sum();
        Ok(canvas.per_pixel(per_pixel as f64))
    }

    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        let small = small_side(width, height);
        let core = (small * 0.008).max(0.7);
        let halo = (small * 0.04).max(2.0);
        let pad = (small * 0.018).max(1.5);
        shade(width, height, palette, |x, y| {
            let (x, y) = (x as f64, y as f64);
            let mut v = 0.0;
            for (nodes, segments) in &self.traces {
                let d = segments
                    .iter()
                    .map(|&(p1, p2)| seg_hit(x, y, p1, p2))
                    .fold(f64::INFINITY, |best, d| if d < best { d } else { best });
                v += gaussian(d, core) + 0.3 * gaussian(d, halo);
                for &(px, py) in nodes {
                    let dn = py_hypot(x - px, y - py);
                    if dn < pad * 2.5 {
                        v += 1.5 * gaussian(dn, pad);
                    }
                }
            }
            1.0 - (-1.2 * v).m_exp()
        })
    }
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Geo {
    polygons: Vec<Vec<Point>>,
}

impl Style for Geo {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, polygons);
        check_polygons("polygons", &self.polygons)?;
        let per_pixel: usize = self.polygons.iter().map(Vec::len).sum();
        Ok(canvas.per_pixel(per_pixel as f64))
    }

    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        let sigma = ((width.min(height) as f64 / 2.0).max(1.0) * 0.025).max(0.6);
        shade(width, height, palette, |x, y| {
            let (x, y) = (x as f64, y as f64);
            // Python sums with sum(), which is compensated since 3.12.
            let v = py_sum(self.polygons.iter().map(|poly| {
                let q = poly_min_dist(x, y, poly) / sigma;
                (-0.5 * (q * q)).m_exp()
            }));
            1.0 - (-v).m_exp()
        })
    }
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Shard {
    /// (radius, polygon, cracks)
    shards: Vec<(f64, Vec<Point>, Vec<Segment>)>,
}

impl Style for Shard {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, shards);
        check_polygons("shards", self.shards.iter().map(|shard| &shard.1))?;
        // The polygon is walked twice per pixel (distance and inside test).
        let per_pixel: usize = self
            .shards
            .iter()
            .map(|(_, poly, cracks)| 2 * poly.len() + cracks.len())
            .sum();
        Ok(canvas.per_pixel(per_pixel as f64))
    }

    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        let norm = self.shards.len() as f64 * 2.2;
        shade(width, height, palette, |x, y| {
            let (x, y) = (x as f64, y as f64);
            let mut v = 0.0;
            for (r, poly, cracks) in &self.shards {
                let sigma = (r * 0.9).max(1.0);
                let halo = gaussian(poly_min_dist(x, y, poly), sigma);
                v += halo * (if point_in_poly(x, y, poly) { 1.95 } else { 1.4 });
                let crack_reach = (r * 0.12).max(2.0);
                for &(p1, p2) in cracks {
                    let d = seg_hit(x, y, p1, p2);
                    if d < crack_reach {
                        v += 0.9 * gaussian(d, sigma);
                    }
                }
            }
            unit(v / norm)
        })
    }
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Prism {
    /// (cx, cy, radius, glow, layers)
    gems: Vec<(f64, f64, f64, f64, Vec<PrismLayer>)>,
}

impl Style for Prism {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, gems);
        let layers = || self.gems.iter().flat_map(|gem| &gem.4);
        check_polygons("gems", layers().map(|layer| &layer.0))?;
        // Assume every gem's box covers every pixel.
        let per_pixel = self.gems.len() + layers().map(|layer| 2 * layer.0.len()).sum::<usize>();
        Ok(canvas.per_pixel(per_pixel as f64))
    }

    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        shade(width, height, palette, |x, y| {
            let (x, y) = (x as f64, y as f64);
            let mut v = 0.0;
            for (gx, gy, r, glow, layers) in &self.gems {
                if (x - gx).abs() > *r || (y - gy).abs() > *r {
                    continue;
                }
                for (poly, ridge, gain) in layers {
                    if point_in_poly(x, y, poly) {
                        let d = poly_min_dist(x, y, poly);
                        v += glow * (0.25 * gain + gaussian(d, *ridge));
                    }
                }
            }
            1.0 - (-1.1 * v).m_exp()
        })
    }
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Mandala {
    center: Point,
    medallion: f64,
    /// (radius, size, wobble fraction)
    rings: Vec<(f64, f64, f64)>,
    /// (inner radius, outer radius, angle)
    spokes: Vec<(f64, f64, f64)>,
    /// (x, y, size)
    glyphs: Vec<(f64, f64, f64)>,
}

impl Style for Mandala {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, center, medallion, rings, spokes, glyphs);
        let per_pixel = self.rings.len() + self.spokes.len() + self.glyphs.len();
        Ok(canvas.per_pixel(per_pixel as f64))
    }

    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        let (cx, cy) = self.center;
        let medallion = self.medallion;
        shade(width, height, palette, |x, y| {
            let (x, y) = (x as f64, y as f64);
            let (dx, dy) = (x - cx, y - cy);
            let r = py_hypot(dx, dy);
            let theta = dy.m_atan2(dx);
            let mut v = (-(r * r) / (2.0 * medallion * medallion)).m_exp() * 3.5;
            let noise = polar_noise(r * 0.05, theta, seed, 3.0, 2);
            for &(radius, size, wob) in &self.rings {
                let d = r - radius * (1.0 + wob * (noise - 0.5));
                v += (-(d * d) / (2.0 * (size * size))).m_exp() * 1.6;
            }
            for &(r0, r1, phase) in &self.spokes {
                let dang = (theta - phase).m_sin().m_atan2((theta - phase).m_cos());
                if r0 <= r && r <= r1 && dang.abs() < 0.06 {
                    v += 0.9;
                }
            }
            for &(gx, gy, size) in &self.glyphs {
                let d = py_hypot(x - gx, y - gy);
                if d < size * 0.9 {
                    v += (-(d * d) / (2.0 * size * size)).m_exp() * 2.2;
                }
            }
            1.0 - (-0.7 * v).m_exp()
        })
    }
}

impl BuildScene for Geo {
    /// `_geo_scene`: 4 to 6 regular polygons, alternate ones turning either
    /// way once per loop.
    fn build(params: &SceneParams<'_>) -> Self {
        let mut rng = PyRandom::new(params.seed);
        let (cx, cy) = center(params.width, params.height);
        let scale = (params.width.min(params.height) as f64 / 2.0).max(1.0);
        let count = rng.randint(4, 6);
        let polygons = (0..count)
            .map(|n| {
                let sides = rng.randint(3, 8);
                let radius = scale * rng.uniform(0.45, 0.95);
                let turns = if n % 2 == 0 { 1 } else { -1 };
                let angle = rng.uniform(0.0, TAU) + loop_angle(params.phase, turns);
                (0..sides)
                    .map(|i| {
                        let a = i as f64 * TAU / sides as f64 + angle;
                        (cx + radius * a.m_cos(), cy + radius * a.m_sin())
                    })
                    .collect()
            })
            .collect();
        Self { polygons }
    }
}

/// Python's `_prism_layers(rng, g, small, turn)` for a gem at `(cx, cy)` with
/// radius `r`.
fn prism_layers(
    rng: &mut PyRandom,
    gem: (f64, f64, f64),
    small: f64,
    turn: f64,
) -> Vec<PrismLayer> {
    let (cx, cy, r) = gem;
    let n_layers = rng.randint(2, 4);
    let shrink = rng.uniform(0.55, 0.8);
    let mut layers = Vec::new();
    for k in 0..n_layers {
        let radius = r * (1.0 - shrink * k as f64 / (n_layers - 1).max(1) as f64);
        let sides = rng.randint(3, 6);
        let turns = if k % 2 == 0 { 1 } else { -1 };
        let phase = rng.uniform(0.0, ROUGH_TAU) + loop_angle(turn, turns);
        let gain = rng.uniform(0.35, 0.9);
        if radius < 1.0 {
            continue;
        }
        let poly = (0..sides)
            .map(|i| {
                let a = TAU * i as f64 / sides as f64 + phase;
                (cx + radius * a.m_cos(), cy - radius * a.m_sin())
            })
            .collect();
        layers.push((poly, (gain * small * 0.025).max(2.0), gain));
    }
    layers
}

impl BuildScene for Prism {
    /// `_prism_scene`: 2 to 5 gems, all placed before any gets its layers.
    fn build(params: &SceneParams<'_>) -> Self {
        let (w, h) = (params.width as f64, params.height as f64);
        let mut rng = PyRandom::new(params.seed);
        let n_gems = rng.randint(2, 5);
        let small = params.width.min(params.height) as f64;
        let gems: Vec<(f64, f64, f64, f64)> = (0..n_gems)
            .map(|_| {
                let cx = rng.uniform(w * 0.15, w * 0.85);
                let cy = rng.uniform(h * 0.15, h * 0.85);
                let r = rng.uniform(small * 0.08, small * 0.28);
                (cx, cy, r, rng.uniform(0.5, 1.0))
            })
            .collect();
        let gems = gems
            .into_iter()
            .map(|(cx, cy, r, glow)| {
                (
                    cx,
                    cy,
                    r,
                    glow,
                    prism_layers(&mut rng, (cx, cy, r), small, params.phase),
                )
            })
            .collect();
        Self { gems }
    }
}

/// Python's `_turned_about(point, center, cosine, sine)`: the offset is added
/// to the point, so angle 0 returns it exactly.
#[inline]
fn turned_about(point: Point, center: Point, cosine: f64, sine: f64) -> Point {
    let (dx, dy) = (point.0 - center.0, point.1 - center.1);
    (
        point.0 + (dx * (cosine - 1.0) - dy * sine),
        point.1 + (dx * sine + dy * (cosine - 1.0)),
    )
}

/// Python's `_make_shard(rng, width, height)`: `(r, poly, cracks, center)`.
fn make_shard(
    rng: &mut PyRandom,
    width: usize,
    height: usize,
) -> (f64, Vec<Point>, Vec<Segment>, Point) {
    let (w, h) = (width as f64, height as f64);
    let small = width.min(height) as f64;
    let cx = rng.uniform(w * 0.2, w * 0.8);
    let cy = rng.uniform(h * 0.2, h * 0.8);
    let r = rng.uniform(small * 0.15, small * 0.42);
    let m = rng.randint(5, 8);
    let poly: Vec<Point> = (0..m)
        .map(|i| {
            let ang = 2.0 * PI * i as f64 / m as f64 + rng.uniform(-0.12, 0.12);
            let rad = r * (0.55 + rng.uniform(0.45, 1.05));
            (cx + rad * ang.m_cos(), cy - rad * ang.m_sin())
        })
        .collect();
    let n_cracks = rng.randint(1, 3);
    let cracks = (0..n_cracks)
        .map(|_| {
            let i = rng.randrange(m);
            // A non-adjacent vertex; both indices are below m.
            let j = (i + rng.randint(2, m - 2)) % m;
            (poly[i as usize], poly[j as usize])
        })
        .collect();
    (r, poly, cracks, (cx, cy))
}

impl BuildScene for Shard {
    /// `_shard_scene`: 2 to 4 cracked shards, neighbours turning about their
    /// centres in opposite directions once per loop.
    fn build(params: &SceneParams<'_>) -> Self {
        let mut rng = PyRandom::new(params.seed);
        let count = rng.randint(2, 4);
        let made: Vec<_> = (0..count)
            .map(|_| make_shard(&mut rng, params.width, params.height))
            .collect();
        let shards = made
            .into_iter()
            .enumerate()
            .map(|(index, (r, poly, cracks, center))| {
                let angle = loop_angle(params.phase, if index % 2 == 0 { 1 } else { -1 });
                let (cosine, sine) = (angle.m_cos(), angle.m_sin());
                let turn = |p| turned_about(p, center, cosine, sine);
                let poly = poly.into_iter().map(turn).collect();
                let cracks = cracks
                    .into_iter()
                    .map(|(a, b)| (turn(a), turn(b)))
                    .collect();
                (r, poly, cracks)
            })
            .collect();
        Self { shards }
    }
}

/// Python's `_make_trace(rng, width, height, phase, first_pad)`: node pads
/// circling small orbits, joined by right-angled legs.
fn make_trace(
    rng: &mut PyRandom,
    width: usize,
    height: usize,
    phase: f64,
    first_pad: usize,
) -> (Vec<Point>, Vec<Segment>) {
    let (w, h) = (width as f64, height as f64);
    let mut nodes = vec![(
        rng.uniform(w * 0.08, w * 0.92),
        rng.uniform(h * 0.08, h * 0.92),
    )];
    for _ in 0..rng.randint(2, 5) {
        let x = rng.uniform(w * 0.04, w * 0.96);
        nodes.push((x, rng.uniform(h * 0.04, h * 0.96)));
    }
    let reach = 0.03 * width.min(height) as f64;
    let nodes: Vec<Point> = nodes
        .into_iter()
        .enumerate()
        .map(|(i, (x, y))| {
            let (dx, dy) = circle_offset(first_pad + i, reach, phase);
            (x + dx, y + dy)
        })
        .collect();
    let segments = nodes
        .windows(2)
        .flat_map(|pair| {
            let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
            let corner = if rng.random() < 0.5 {
                (x1, y0)
            } else {
                (x0, y1)
            };
            [((x0, y0), corner), (corner, (x1, y1))]
        })
        .collect();
    (nodes, segments)
}

impl BuildScene for Circuit {
    /// `_circuit_scene`: 3 to 6 traces, pads numbered 8 apart per trace.
    fn build(params: &SceneParams<'_>) -> Self {
        let mut rng = PyRandom::new(params.seed);
        let count = rng.randint(3, 6) as usize;
        let traces = (0..count)
            .map(|index| {
                make_trace(
                    &mut rng,
                    params.width,
                    params.height,
                    params.phase,
                    index * 8,
                )
            })
            .collect();
        Self { traces }
    }
}

/// A mandala ring while the scene is built: radius, glyph count, glyph size,
/// phase and wobble, in Python's draw order.
struct MandalaRing {
    radius: f64,
    glyphs: i64,
    size: f64,
    phase: f64,
    wob: f64,
}

impl BuildScene for Mandala {
    /// `_mandala_scene`: rings sorted by radius, spokes between neighbours and
    /// glyphs around each ring, neighbouring rings turning opposite ways.
    fn build(params: &SceneParams<'_>) -> Self {
        let phase = params.phase;
        let mut rng = PyRandom::new(params.seed);
        let (cx, cy) = center(params.width, params.height);
        let max_r = (cx * cx + cy * cy).sqrt().max(1.0);
        let count = rng.randint(6, 12);
        let mut rings: Vec<MandalaRing> = (0..count)
            .map(|_| {
                let radius = max_r * rng.uniform(0.2, 0.9);
                let glyphs = rng.randint(2, 10);
                let size = max_r * rng.uniform(0.014, 0.044);
                let phase = rng.uniform(0.0, ROUGH_TAU);
                let wob = rng.uniform(0.05, 0.18);
                MandalaRing {
                    radius,
                    glyphs,
                    size,
                    phase,
                    wob,
                }
            })
            .collect();
        // Python's stable sort by radius; radii are finite.
        rings.sort_by(|a, b| a.radius.total_cmp(&b.radius));
        let spokes = rings
            .windows(2)
            .enumerate()
            .map(|(index, pair)| {
                let turns = if index % 2 == 1 { 1 } else { -1 };
                let angle = rng.uniform(0.0, ROUGH_TAU) + loop_angle(phase, turns);
                (pair[0].radius, pair[1].radius, angle)
            })
            .collect();
        for (index, ring) in rings.iter_mut().enumerate() {
            ring.phase += loop_angle(phase, if index % 2 == 1 { -1 } else { 1 });
        }
        let glyphs = rings
            .iter()
            .flat_map(|ring| {
                (0..ring.glyphs).map(move |gi| {
                    let ga = ring.phase + (gi as f64 / ring.glyphs as f64) * 2.0 * PI;
                    (
                        cx + ga.m_cos() * ring.radius,
                        cy + ga.m_sin() * ring.radius,
                        ring.size,
                    )
                })
            })
            .collect();
        Self {
            center: (cx, cy),
            medallion: (max_r * 0.06).max(3.0),
            rings: rings
                .iter()
                .map(|ring| (ring.radius, ring.size, ring.wob))
                .collect(),
            spokes,
            glyphs,
        }
    }
}
