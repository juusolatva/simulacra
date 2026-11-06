//! Polygon-, segment- and ring-based styles.

use crate::math::{Libm, py_hypot};

use super::{Style, gaussian, small_side, unit};
use crate::geometry::{Point, Segment, point_in_poly, poly_min_dist, py_sum, seg_hit};

/// (polygon, ridge width, gain)
type PrismLayer = (Vec<Point>, f64, f64);
use crate::noise::polar_noise;
use crate::palette::Palette;
use crate::raster::shade;

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Circuit {
    /// (node pads, segments) per trace
    traces: Vec<(Vec<Point>, Vec<Segment>)>,
}

impl Style for Circuit {
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

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Geo {
    polygons: Vec<Vec<Point>>,
}

impl Style for Geo {
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

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Shard {
    /// (radius, polygon, cracks)
    shards: Vec<(f64, Vec<Point>, Vec<Segment>)>,
}

impl Style for Shard {
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

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Prism {
    /// (cx, cy, radius, glow, layers)
    gems: Vec<(f64, f64, f64, f64, Vec<PrismLayer>)>,
}

impl Style for Prism {
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

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
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
