//! Python-exact numeric helpers and 2D geometry shared by the styles.

use crate::math::py_hypot;

pub type Point = (f64, f64);
pub type Segment = (Point, Point);

/// Python's float `%`: the result takes the sign of the divisor (CPython `float_rem`).
#[inline]
pub fn py_mod(a: f64, b: f64) -> f64 {
    let m = a % b; // C fmod
    if m != 0.0 {
        if (b < 0.0) != (m < 0.0) { m + b } else { m }
    } else {
        0.0_f64.copysign(b)
    }
}

/// Python 3.12+ `sum()` of floats starting from the int 0: Neumaier compensated
/// summation, exactly as CPython's `builtin_sum_impl` does it.
pub fn py_sum(values: impl IntoIterator<Item = f64>) -> f64 {
    let mut values = values.into_iter();
    let Some(first) = values.next() else {
        return 0.0;
    };
    let mut total = 0.0 + first; // int 0 + float
    let mut compensation = 0.0;
    for x in values {
        let t = total + x;
        if total.abs() >= x.abs() {
            compensation += (total - t) + x;
        } else {
            compensation += (x - t) + total;
        }
        total = t;
    }
    if compensation != 0.0 && compensation.is_finite() {
        total += compensation;
    }
    total
}

/// Ray-casting point-in-polygon test (`_point_in_poly`).
pub fn point_in_poly(x: f64, y: f64, poly: &[Point]) -> bool {
    let mut inside = false;
    let mut j = poly.len() - 1;
    for (i, &(xi, yi)) in poly.iter().enumerate() {
        let (xj, yj) = poly[j];
        if ((yi > y) != (yj > y)) && (x < (xj - xi) * (y - yi) / (yj - yi) + xi) {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Distance from a point to a segment, clamped to its ends (`_seg_hit`).
#[inline]
#[allow(
    clippy::manual_clamp,
    reason = "matches Python's max(0.0, min(1.0, t)) for NaN"
)]
pub fn seg_hit(x: f64, y: f64, p1: Point, p2: Point) -> f64 {
    let (x1, y1) = p1;
    let (dx, dy) = (p2.0 - x1, p2.1 - y1);
    let seg_len2 = dx * dx + dy * dy;
    if seg_len2 < 1e-12 {
        return py_hypot(x - x1, y - y1);
    }
    let t = (((x - x1) * dx + (y - y1) * dy) / seg_len2)
        .min(1.0)
        .max(0.0);
    py_hypot(x - (x1 + t * dx), y - (y1 + t * dy))
}

/// Minimum distance from a point to a closed polygon's edges (`_poly_min_dist`).
pub fn poly_min_dist(x: f64, y: f64, poly: &[Point]) -> f64 {
    let mut best = 1e9;
    for (i, &p1) in poly.iter().enumerate() {
        let d = seg_hit(x, y, p1, poly[(i + 1) % poly.len()]);
        if d < best {
            best = d;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn py_mod_matches_python() {
        // Python: -1.5 % 4.0 == 2.5, 7.0 % -3.0 == -2.0, -0.0 % 2.0 == 0.0
        assert_eq!(py_mod(-1.5, 4.0), 2.5);
        assert_eq!(py_mod(7.0, -3.0), -2.0);
        assert!(py_mod(-0.0, 2.0).is_sign_positive());
    }

    #[test]
    fn py_sum_is_compensated_like_python() {
        // Python 3.12+: sum([0.1] * 10) == 1.0 (naive summation gives 0.9999999999999999)
        assert_eq!(py_sum([0.1; 10]), 1.0);
        assert_eq!(py_sum([1e100, 1.0, -1e100]), 1.0);
    }
}
