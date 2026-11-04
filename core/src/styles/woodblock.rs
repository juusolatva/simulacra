//! Carved landscapes. Python builds the animated silhouettes; Rust shades the
//! paper, overlapping layers and contour grooves in Python's operation order.

use rayon::prelude::*;

use super::Style;
use crate::geometry::py_mod;
use crate::palette::Palette;

#[derive(pyo3::FromPyObject)]
#[pyo3(from_item_all)]
struct Layer {
    surface: Vec<f64>,
    spacing: f64,
    color: f64,
    carve: f64,
    wave: bool,
}

impl Layer {
    fn ink(&self, depth: f64, x: usize, scale: f64) -> f64 {
        let mut groove = depth / self.spacing;
        groove += 0.16 * (x as f64 / scale * 18.0 + depth / scale * 12.0 + self.carve).sin();
        if depth < 0.007 * scale {
            return 0.04;
        }
        if py_mod(groove, 1.0) < 0.12 {
            return if self.wave { 0.94 } else { 0.86 };
        }
        self.color
    }
}

#[derive(pyo3::FromPyObject)]
#[pyo3(from_item_all)]
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
