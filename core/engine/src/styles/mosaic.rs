//! Mosaic: Python computes one palette position per tile (all patterns, regions
//! and randomness live in `_mosaic_scene()`); Rust only expands tiles to pixels.

use rayon::prelude::*;

use super::Style;
use crate::palette::Palette;

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
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
    fn render(&self, width: usize, height: usize, _seed: u32, palette: &Palette) -> Vec<u8> {
        let colors: Vec<[u8; 3]> = self.values.iter().map(|&t| palette.color(t)).collect();
        let grout = palette.color(0.0);
        let xs = axis(width, self.tile, self.cols, self.grout);
        let ys = axis(height, self.tile, self.rows, self.grout);
        let mut rgb = vec![0u8; width * height * 3];
        rgb.par_chunks_mut(width * 3).zip(ys.par_iter()).for_each(|(row, cell_y)| {
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
