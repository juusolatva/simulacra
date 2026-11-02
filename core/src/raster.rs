//! Parallel per-pixel shading into a packed RGB buffer.

use rayon::prelude::*;

use crate::palette::Palette;

/// Evaluate `color(x, y)` for every pixel and return row-major RGB bytes
/// (3 per pixel). Rows are shaded in parallel.
pub fn shade_rgb<F>(width: usize, height: usize, color: F) -> Vec<u8>
where
    F: Fn(usize, usize) -> [u8; 3] + Sync,
{
    let stride = width * 3;
    let mut rgb = vec![0u8; stride * height];
    if stride == 0 {
        return rgb;
    }
    rgb.par_chunks_mut(stride).enumerate().for_each(|(y, row)| {
        for (x, pixel) in row.as_chunks_mut::<3>().0.iter_mut().enumerate() {
            *pixel = color(x, y);
        }
    });
    rgb
}

/// Like `shade_rgb`, mapping a scalar `value(x, y)` through `palette`.
pub fn shade<F>(width: usize, height: usize, palette: &Palette, value: F) -> Vec<u8>
where
    F: Fn(usize, usize) -> f64 + Sync,
{
    shade_rgb(width, height, |x, y| palette.color(value(x, y)))
}
