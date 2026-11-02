//! Value noise and fBm, ported operation-for-operation from `abstract_art.py`.
//!
//! The arithmetic order matches the Python reference exactly (Rust does not
//! fuse multiply-adds by default), so results agree bit-for-bit apart from
//! any differences in the platform math library.

/// Integer lattice hash in [0, 1]. Python ints are unbounded, but every step
/// is masked to 32 bits, so wrapping 64-bit arithmetic gives the same result.
#[inline]
fn hash(x: i64, y: i64, seed: u32) -> f64 {
    let n = (x as u64)
        .wrapping_mul(3_747_612)
        .wrapping_add((y as u64).wrapping_mul(6_682_691))
        .wrapping_add(u64::from(seed).wrapping_mul(974_631))
        & 0xFFFF_FFFF;
    let n = (n ^ (n >> 13)).wrapping_mul(0x5BD1_E995) & 0xFFFF_FFFF;
    let n = (n ^ (n >> 16)) & 0xFFFF_FFFF;
    n as f64 / 4_294_967_295.0
}

/// Smoothly interpolated lattice noise with the Python default cell size of 8.
#[inline]
pub fn value_noise(x: f64, y: f64, seed: u32) -> f64 {
    const CELL: f64 = 8.0;
    let gx = (x / CELL).floor();
    let gy = (y / CELL).floor();
    let mut fx = x / CELL - gx;
    let mut fy = y / CELL - gy;
    fx = fx * fx * (3.0 - 2.0 * fx);
    fy = fy * fy * (3.0 - 2.0 * fy);
    let (ix, iy) = (gx as i64, gy as i64);
    let n00 = hash(ix, iy, seed);
    let n10 = hash(ix + 1, iy, seed);
    let n01 = hash(ix, iy + 1, seed);
    let n11 = hash(ix + 1, iy + 1, seed);
    n00 * (1.0 - fx) * (1.0 - fy) + n10 * fx * (1.0 - fy) + n01 * (1.0 - fx) * fy + n11 * fx * fy
}

/// Fractal Brownian motion with persistence 0.5 and lacunarity 2.0; returns [0, 1].
#[inline]
pub fn fbm(x: f64, y: f64, seed: u32, octaves: u32) -> f64 {
    let mut total = 0.0;
    let mut norm = 0.0;
    let mut freq = 1.0;
    let mut amp = 1.0;
    for _ in 0..octaves {
        total += value_noise(x * freq, y * freq, seed) * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    total / norm
}

/// fBm sampled on a circle in noise space; continuous across theta = +/-pi.
#[inline]
pub fn polar_noise(r_coord: f64, theta: f64, seed: u32, around: f64, octaves: u32) -> f64 {
    fbm(r_coord + around * theta.cos(), around * theta.sin(), seed, octaves)
}

/// Stretch fbm output around 0.5.
#[inline]
pub fn contrast(n: f64, gain: f64) -> f64 {
    0.5 + (n - 0.5) * gain
}

#[cfg(test)]
mod tests {
    use super::*;

    // Expected values were produced by the Python reference implementation.
    #[test]
    fn hash_matches_python() {
        assert_eq!(hash(0, 0, 0), 0.0);
        assert_eq!(hash(-1, -2, 5), 0.8618520523565477);
        assert_eq!(hash(123, 456, 4_294_967_295), 0.8277825722069905);
    }

    #[test]
    fn fbm_and_polar_noise_match_python() {
        assert_eq!(fbm(-3.7, 12.25, 77, 4), 0.6149435913811198);
        assert_eq!(polar_noise(5.5, -2.0, 9, 3.0, 3), 0.6977435125610463);
    }

    #[test]
    fn fbm_is_unit_range() {
        for i in -50..50 {
            let v = fbm(f64::from(i) * 1.7, f64::from(i) * -2.3, 9, 4);
            assert!((0.0..=1.0).contains(&v));
        }
    }
}
