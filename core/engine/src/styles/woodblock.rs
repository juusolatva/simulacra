//! Carved landscapes: animated silhouettes sampled per column (the port of
//! `_woodblock_scene()`), shaded as paper, overlapping layers and contour
//! grooves in Python's operation order.

use crate::math::Libm;
use rayon::prelude::*;

use std::f64::consts::TAU;

use super::{Style, loop_angle};
use crate::geometry::py_mod;
use crate::palette::Palette;
use crate::random::PyRandom;
use crate::scenes::{BuildScene, SceneParams};
use crate::validate::{Canvas, Numbers, SceneError, check_numbers};

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
struct Layer {
    surface: Vec<f64>,
    spacing: f64,
    color: f64,
    carve: f64,
    wave: bool,
}

impl Numbers for Layer {
    fn check(&self, field: &'static str) -> Result<(), SceneError> {
        self.surface.check(field)?;
        (self.spacing, self.color, self.carve).check(field)
    }
}

impl Layer {
    fn ink(&self, depth: f64, x: usize, scale: f64) -> f64 {
        let mut groove = depth / self.spacing;
        groove += 0.16 * (x as f64 / scale * 18.0 + depth / scale * 12.0 + self.carve).m_sin();
        if depth < 0.007 * scale {
            return 0.04;
        }
        if py_mod(groove, 1.0) < 0.12 {
            return if self.wave { 0.94 } else { 0.86 };
        }
        self.color
    }
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Woodblock {
    layers: Vec<Layer>,
    scale: f64,
}

impl Woodblock {
    fn value(&self, x: usize, y: usize) -> f64 {
        for layer in self.layers.iter().rev() {
            let depth = y as f64 - layer.surface[x];
            if depth >= 0.0 {
                return layer.ink(depth, x, self.scale);
            }
        }
        1.0
    }
}

impl Style for Woodblock {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(self, layers, scale);
        for layer in &self.layers {
            if layer.surface.len() != canvas.width {
                return Err(SceneError::Length {
                    field: "layers (surface, one height per column)",
                    expected: canvas.width,
                    found: layer.surface.len(),
                });
            }
        }
        Ok(canvas.per_pixel(self.layers.len() as f64))
    }

    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        let mut rgb = vec![0u8; width * height * 3];
        rgb.par_chunks_mut(width * 3)
            .enumerate()
            .for_each(|(y, row)| {
                for (x, pixel) in row.as_chunks_mut::<3>().0.iter_mut().enumerate() {
                    *pixel = palette.color(self.value(x, y));
                }
            });
        rgb
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Profile {
    Hill,
    Cliff,
    Wave,
}

impl Profile {
    /// Python's `_woodblock_profile(kind, angle, bend)`: rounded hills, steep
    /// cliffs and leaning wave crests.
    fn height(self, angle: f64, bend: f64) -> f64 {
        match self {
            Self::Cliff => {
                0.72 * (3.5 * angle.m_sin()).m_tanh() + 0.18 * (3.0 * angle + bend).m_sin()
            }
            Self::Wave => (angle + 0.72 * (angle + bend).m_sin()).m_sin(),
            Self::Hill => 0.76 * angle.m_sin() + 0.24 * (2.0 * angle + bend).m_sin(),
        }
    }
}

/// Python's `_woodblock_layer(rng, index, count, width, height, kind, top, phase)`.
fn build_layer(
    rng: &mut PyRandom,
    (index, count): (usize, usize),
    (width, height): (usize, usize),
    kind: Profile,
    top: f64,
    phase: f64,
) -> Layer {
    let scale = width.min(height) as f64;
    let frequency = rng.uniform(1.1, 2.4) * TAU;
    let offset = rng.uniform(0.0, TAU);
    let bend = rng.uniform(-0.7, 0.7) + 0.45 * loop_angle(phase, 1).m_sin();
    let travel = loop_angle(phase, if index % 2 == 1 { -1 } else { 1 });
    let amplitude = rng.uniform(0.07, 0.16) * scale;
    let base = height as f64 * (top + index as f64 * (0.92 - top) / count as f64);
    let surface = (0..width)
        .map(|x| {
            base + amplitude * kind.height(x as f64 / scale * frequency + offset + travel, bend)
        })
        .collect();
    let spacing = rng.uniform(0.028, 0.045) * scale;
    let color = 0.14 + 0.62 * (1.0 - index as f64 / (count - 1).max(1) as f64);
    let carve = rng.uniform(0.0, TAU);
    Layer {
        surface,
        spacing,
        color,
        carve,
        wave: kind == Profile::Wave,
    }
}

impl BuildScene for Woodblock {
    /// `_woodblock_scene`: 4 to 7 layers of one kind (or mixed), each sliding
    /// sideways once per loop, alternating in direction.
    fn build(params: &SceneParams<'_>) -> Self {
        const KINDS: [Profile; 3] = [Profile::Hill, Profile::Cliff, Profile::Wave];
        let (width, height) = (params.width, params.height);
        let mut rng = PyRandom::new(params.seed);
        // None stands for "mixed": a kind is drawn per layer.
        let mode = *rng.choice(&[
            Some(Profile::Hill),
            Some(Profile::Cliff),
            Some(Profile::Wave),
            None,
        ]);
        let count = rng.randint(4, 7) as usize;
        // Some prints are cropped into their first ridge; others leave open paper.
        let top = *rng.choice(&[-0.22, 0.20, 0.34]);
        let kinds: Vec<Profile> = (0..count)
            .map(|_| mode.unwrap_or_else(|| *rng.choice(&KINDS)))
            .collect();
        let layers = kinds
            .into_iter()
            .enumerate()
            .map(|(index, kind)| {
                build_layer(
                    &mut rng,
                    (index, count),
                    (width, height),
                    kind,
                    top,
                    params.phase,
                )
            })
            .collect();
        Self {
            layers,
            scale: width.min(height) as f64,
        }
    }
}
