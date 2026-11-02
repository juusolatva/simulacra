//! Parallel per-pixel shading into a packed RGB buffer.

use rayon::prelude::*;

use crate::palette::Palette;

/// Evaluate `value(x, y)` for every pixel, map it through `palette`, and return
/// row-major RGB bytes (3 per pixel). Rows are shaded in parallel.
pub fn shade<F>(width: usize, height: usize, palette: &Palette, value: F) -> Vec<u8>
where
    F: Fn(usize, usize) -> f64 + Sync,
{
    let stride = width * 3;
    let mut rgb = vec![0u8; stride * height];
    if stride == 0 {
        return rgb;
    }
    rgb.par_chunks_mut(stride).enumerate().for_each(|(y, row)| {
        for (x, pixel) in row.as_chunks_mut::<3>().0.iter_mut().enumerate() {
            *pixel = palette.color(value(x, y));
        }
    });
    rgb
}
