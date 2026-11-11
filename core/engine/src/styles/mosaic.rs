//! Mosaic: the builder (the port of `_mosaic_scene()`) computes one palette
//! position per tile from its regions and patterns; the shader only expands
//! tiles to pixels.

use rayon::prelude::*;

use super::Style;
use crate::geometry::py_mod;
use crate::noise::{contrast, fbm, fbm_loop, hash};
use crate::palette::Palette;
use crate::random::PyRandom;
use crate::scenes::{BuildScene, SceneParams};
use crate::seed32;
use crate::validate::{Canvas, SceneError, check_numbers};

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Mosaic {
    cols: usize,
    rows: usize,
    tile: f64,
    grout: f64,
    values: Vec<f64>,
}

/// Port of `_mosaic_axis`: the tile index of each pixel, or `None` for grout.
fn axis(length: usize, tile: f64, count: usize, grout: f64) -> Vec<Option<usize>> {
    (0..length)
        .map(|p| {
            let k = (count - 1).min((p as f64 / tile) as usize);
            if p as f64 - k as f64 * tile < grout {
                None
            } else {
                Some(k)
            }
        })
        .collect()
}

impl Style for Mosaic {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, tile, grout, values);
        if self.cols == 0 || self.rows == 0 {
            return Err(SceneError::Range {
                field: "cols and rows",
                rule: "must both be at least 1",
            });
        }
        let tiles = self.cols.checked_mul(self.rows).ok_or(SceneError::Range {
            field: "cols and rows",
            rule: "multiply to more tiles than fit in memory",
        })?;
        if self.values.len() != tiles {
            return Err(SceneError::Length {
                field: "values",
                expected: tiles,
                found: self.values.len(),
            });
        }
        Ok(canvas.pixels() + tiles as f64)
    }

    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        let colors: Vec<[u8; 3]> = self.values.iter().map(|&t| palette.color(t)).collect();
        let grout = palette.color(0.0);
        let xs = axis(width, self.tile, self.cols, self.grout);
        let ys = axis(height, self.tile, self.rows, self.grout);
        let mut rgb = vec![0u8; width * height * 3];
        rgb.par_chunks_mut(width * 3)
            .zip(ys.par_iter())
            .for_each(|(row, cell_y)| {
                for (pixel, cell_x) in row.as_chunks_mut::<3>().0.iter_mut().zip(&xs) {
                    *pixel = match (cell_y, cell_x) {
                        (Some(j), Some(i)) => colors[j * self.cols + i],
                        _ => grout,
                    };
                }
            });
        rgb
    }
}

/// Python's `_MOSAIC_TILES`: tiles across the short side.
const MOSAIC_TILES: usize = 64;

/// Python's `_MOSAIC_PATTERNS`, in its order (the builder draws one by index).
#[derive(Clone, Copy, PartialEq)]
enum Pattern {
    Checker,
    Stripes,
    Chevron,
    Diamonds,
    Squares,
    Rainbow,
    Lattice,
    Rosette,
    Smear,
    Maze,
}

const PATTERNS: [Pattern; 10] = [
    Pattern::Checker,
    Pattern::Stripes,
    Pattern::Chevron,
    Pattern::Diamonds,
    Pattern::Squares,
    Pattern::Rainbow,
    Pattern::Lattice,
    Pattern::Rosette,
    Pattern::Smear,
    Pattern::Maze,
];

/// One region of the patchwork (Python's `_mosaic_region` dict).
struct Region {
    kind: Pattern,
    ci: i64,
    cj: i64,
    q: i64,
    amp: i64,
    flip: i64,
    levels: Vec<f64>,
    /// `seed + 101 * (index + 1)` modulo 2**32, all the noise and hash see.
    seed: u32,
}

/// Python's `_mosaic_shift(phase, period)`: whole tiles a pattern has moved.
fn shift(phase: f64, period: i64) -> i64 {
    (py_mod(phase, 1.0) * period as f64) as i64
}

impl Region {
    /// Python's `_mosaic_region(rng, index, seed, cols, rows)`.
    fn draw(rng: &mut PyRandom, index: usize, seed: i64, cols: i64, rows: i64) -> Self {
        let kind = *rng.choice(&PATTERNS);
        let ci = rng.randrange(cols);
        let cj = rng.randrange(rows);
        let q = rng.randint(1, 3);
        let amp = rng.randint(2, 5);
        let flip = *rng.choice(&[-1, 1]);
        let levels = Self::levels(rng, kind);
        // Python adds exact ints; the noise and hash only see the low 32 bits.
        let seed = seed32(seed).wrapping_add((101 * (index + 1)) as u32);
        Self {
            kind,
            ci,
            cj,
            q,
            amp,
            flip,
            levels,
            seed,
        }
    }

    /// Python's `_mosaic_levels(rng, kind)`: the palette positions a region's
    /// pattern cycles through.
    fn levels(rng: &mut PyRandom, kind: Pattern) -> Vec<f64> {
        if kind == Pattern::Rainbow {
            let n = rng.randint(5, 8);
            let start = rng.uniform(0.15, 0.35);
            return (0..n)
                .map(|k| start + (1.0 - start) * k as f64 / (n - 1) as f64)
                .collect();
        }
        let set_in_black = matches!(
            kind,
            Pattern::Checker | Pattern::Lattice | Pattern::Rosette | Pattern::Maze
        );
        let n = match kind {
            Pattern::Checker | Pattern::Maze => 2,
            Pattern::Lattice | Pattern::Rosette => 3,
            _ => rng.randint(2, 4),
        };
        let mut levels: Vec<f64> = (0..n).map(|_| rng.uniform(0.3, 1.0)).collect();
        if set_in_black || rng.random() < 0.4 {
            // The black tiles the patterns are set in.
            levels[0] = rng.uniform(0.0, 0.08);
        }
        levels
    }

    /// Python's `_MOSAIC_BANDS[kind](r, i, j, di, dj, phase)`: which level the
    /// tile at folded column `i`, row `j` takes (before wrapping).
    fn band(&self, i: i64, j: i64, phase: f64) -> i64 {
        let (q, di, dj) = (self.q, i - self.ci, j - self.cj);
        let cycle = q * self.levels.len() as i64;
        match self.kind {
            Pattern::Checker => (i + shift(phase, 2 * q)).div_euclid(q) + j.div_euclid(q),
            Pattern::Stripes => (i + self.flip * j + shift(phase, cycle)).div_euclid(q),
            Pattern::Chevron => {
                let (i, j) = if self.flip < 0 { (j, i) } else { (i, j) };
                let a = self.amp;
                (j + (i.rem_euclid(2 * a) - a).abs() + shift(phase, cycle)).div_euclid(q)
            }
            Pattern::Diamonds => (di.abs() + dj.abs() - shift(phase, cycle)).div_euclid(q),
            Pattern::Squares => (di.abs().max(dj.abs()) - shift(phase, cycle)).div_euclid(q),
            Pattern::Rainbow => {
                let along = if self.flip > 0 { i } else { j };
                (along + shift(phase, cycle)).div_euclid(q)
            }
            Pattern::Lattice => {
                let p = q + 3;
                let a = (i + j + shift(phase, p)).rem_euclid(p);
                let b = (i - j).rem_euclid(p);
                if a == 0 || b == 0 {
                    1
                } else if a == p / 2 && b == p / 2 {
                    2
                } else {
                    0
                }
            }
            Pattern::Rosette => {
                let p = 2 * q + 7;
                let half = p / 2;
                let u = (di + half).rem_euclid(p) - half;
                let v = (dj + half).rem_euclid(p) - half;
                if u == 0 && v == 0 {
                    2
                } else if u == 0 || v == 0 || u.abs() == v.abs() {
                    1
                } else {
                    0
                }
            }
            Pattern::Smear => {
                let n = fbm_loop(i as f64 * 1.1, j as f64 * 3.2, self.seed, 3, phase);
                #[allow(
                    clippy::manual_clamp,
                    reason = "Python's max(0.0, min(0.999, n)), as written"
                )]
                let t = contrast(n, 2.5).min(0.999).max(0.0);
                (t * self.levels.len() as f64) as i64
            }
            Pattern::Maze => {
                let (lx, ly) = (i.rem_euclid(2), j.rem_euclid(2));
                if lx == 1 && ly == 1 {
                    return 0;
                }
                if lx == 0 && ly == 0 {
                    return 1;
                }
                let across = hash(i.div_euclid(2), j.div_euclid(2), self.seed) < 0.5;
                i64::from(across == (ly == 0))
            }
        }
    }
}

/// The layout `_mosaic_tile` reads (Python's intermediate `scene` dict).
struct Layout {
    seed: u32,
    cols: i64,
    rows: i64,
    mirror_x: bool,
    mirror_y: bool,
    silhouette: bool,
    cx: f64,
    cy: f64,
    rx: f64,
    ry: f64,
    warp: f64,
    sparkle: f64,
    regions: Vec<Region>,
}

impl Layout {
    /// Python's `_mosaic_region_at(regions, wi, wj)`: the nearest region centre
    /// under an octagonal distance (the first one on a tie).
    fn region_at(&self, wi: f64, wj: f64) -> &Region {
        let mut best = &self.regions[0];
        let mut best_d = f64::INFINITY;
        for (k, r) in self.regions.iter().enumerate() {
            let dx = (wi - r.ci as f64).abs();
            let dy = (wj - r.cj as f64).abs();
            let d = py_max2(dx, dy) + 0.4 * py_min2(dx, dy);
            if k == 0 || d < best_d {
                best = r;
                best_d = d;
            }
        }
        best
    }

    /// Python's `_mosaic_inside(scene, i, j)`: whether folded tile (i, j)
    /// belongs to the ragged silhouette.
    fn inside(&self, i: i64, j: i64) -> bool {
        if !self.silhouette {
            return true;
        }
        let n = fbm(i as f64 * 0.6, j as f64 * 0.6, self.seed.wrapping_add(5), 4);
        let du = (i as f64 - self.cx) / self.rx;
        let dv = (j as f64 - self.cy) / self.ry;
        contrast(n, 2.2) - 0.75 * (du * du + dv * dv).sqrt() > -0.05
    }

    /// Python's `_mosaic_tile(scene, i, j, phase)`: the palette position of the
    /// tile at column `i`, row `j`.
    fn tile(&self, i: i64, j: i64, phase: f64) -> f64 {
        let fi = if self.mirror_x {
            i.min(self.cols - 1 - i)
        } else {
            i
        };
        let fj = if self.mirror_y {
            j.min(self.rows - 1 - j)
        } else {
            j
        };
        if !self.inside(fi, fj) {
            // Python's seed + 977 * shift, modulo 2**32 for the hash.
            let twinkle = self.seed.wrapping_add((977 * shift(phase, 6)) as u32);
            return if hash(i, j, twinkle) < 0.012 {
                self.sparkle
            } else {
                0.0
            };
        }
        let (x, y) = (fi as f64 * 0.8, fj as f64 * 0.8);
        let wi = fi as f64 + (fbm(x, y, self.seed.wrapping_add(11), 2) - 0.5) * self.warp;
        let wj = fj as f64 + (fbm(x, y, self.seed.wrapping_add(23), 2) - 0.5) * self.warp;
        let r = self.region_at(wi, wj);
        let band = r.band(fi, fj, phase);
        r.levels[band.rem_euclid(r.levels.len() as i64) as usize]
    }
}

/// Python's `max(a, b)`: `a` unless `b` is larger.
fn py_max2(a: f64, b: f64) -> f64 {
    if b > a { b } else { a }
}

/// Python's `min(a, b)`: `a` unless `b` is smaller.
fn py_min2(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

impl BuildScene for Mosaic {
    /// `_mosaic_scene`: 14 to 22 patterned regions on a grid of about 64 tiles
    /// across the short side, often mirrored and sometimes cut to a ragged
    /// silhouette with sparkles outside it.
    fn build(params: &SceneParams<'_>) -> Self {
        let (width, height, seed) = (params.width, params.height, params.seed);
        let mut rng = PyRandom::new(seed);
        let small = width.min(height);
        let tiles = MOSAIC_TILES.min(small);
        let tile = small as f64 / tiles as f64;
        let cols = ((width as f64 / tile).ceil() as usize).max(1);
        let rows = ((height as f64 / tile).ceil() as usize).max(1);
        // "none", "x", "x", "xy" as (mirror_x, mirror_y).
        let (mirror_x, mirror_y) =
            *rng.choice(&[(false, false), (true, false), (true, false), (true, true)]);
        let (c, r) = (cols as i64, rows as i64);
        let span_i = if mirror_x { (c + 1) / 2 } else { c };
        let span_j = if mirror_y { (r + 1) / 2 } else { r };
        let silhouette = rng.random() < 0.55;
        let cx = if mirror_x {
            (c - 1) as f64 / 2.0
        } else {
            rng.uniform(0.35, 0.65) * c as f64
        };
        let cy = if mirror_y {
            (r - 1) as f64 / 2.0
        } else {
            rng.uniform(0.4, 0.6) * r as f64
        };
        let warp = rng.uniform(4.0, 9.0) * tiles as f64 / MOSAIC_TILES as f64;
        let sparkle = rng.uniform(0.75, 1.0);
        let count = rng.randint(14, 22) as usize;
        let regions = (0..count)
            .map(|k| Region::draw(&mut rng, k, seed, span_i, span_j))
            .collect();
        let layout = Layout {
            seed: seed32(seed),
            cols: c,
            rows: r,
            mirror_x,
            mirror_y,
            silhouette,
            cx,
            cy,
            rx: c as f64 * 0.5,
            ry: r as f64 * 0.5,
            warp,
            sparkle,
            regions,
        };
        let values = (0..r)
            .flat_map(|j| (0..c).map(move |i| (i, j)))
            .map(|(i, j)| layout.tile(i, j, params.phase))
            .collect();
        let grout = if tile >= 4.0 {
            (0.12 * tile).max(1.0)
        } else {
            0.0
        };
        Self {
            cols,
            rows,
            tile,
            grout,
            values,
        }
    }
}
