//! Transcendental functions the shaders take from libm, behind one switch.
//!
//! By default these call the platform libm through `f64`'s methods, as CPython's
//! `math` module does, so native output matches Python on the same machine. With
//! the `portable` feature they call the pure-Rust `libm` crate instead, which
//! gives the same result on every target (used to measure drift; see PLAN.md).
//! `sqrt` and `floor` are not routed here, because they are exact everywhere.
//! `hypot` is not libm in Python: [`py_hypot`] ports CPython's own algorithm,
//! and both backends use it.

/// Libm-backed methods on `f64`, named with an `m_` prefix so call sites read
/// like the `f64` methods they replace.
pub trait Libm {
    #[must_use]
    fn m_exp(self) -> f64;
    #[must_use]
    fn m_sin(self) -> f64;
    #[must_use]
    fn m_cos(self) -> f64;
    #[must_use]
    fn m_tan(self) -> f64;
    #[must_use]
    fn m_atan(self) -> f64;
    #[must_use]
    fn m_atan2(self, other: f64) -> f64;
    #[must_use]
    fn m_powf(self, n: f64) -> f64;
    #[must_use]
    fn m_ln_1p(self) -> f64;
}

// The one place allowed to call the platform libm (see clippy.toml).
#[cfg(not(feature = "portable"))]
#[allow(clippy::disallowed_methods)]
impl Libm for f64 {
    #[inline]
    fn m_exp(self) -> f64 {
        self.exp()
    }
    #[inline]
    fn m_sin(self) -> f64 {
        self.sin()
    }
    #[inline]
    fn m_cos(self) -> f64 {
        self.cos()
    }
    #[inline]
    fn m_tan(self) -> f64 {
        self.tan()
    }
    #[inline]
    fn m_atan(self) -> f64 {
        self.atan()
    }
    #[inline]
    fn m_atan2(self, other: f64) -> f64 {
        self.atan2(other)
    }
    #[inline]
    fn m_powf(self, n: f64) -> f64 {
        self.powf(n)
    }
    #[inline]
    fn m_ln_1p(self) -> f64 {
        self.ln_1p()
    }
}

#[cfg(feature = "portable")]
impl Libm for f64 {
    #[inline]
    fn m_exp(self) -> f64 {
        libm::exp(self)
    }
    #[inline]
    fn m_sin(self) -> f64 {
        libm::sin(self)
    }
    #[inline]
    fn m_cos(self) -> f64 {
        libm::cos(self)
    }
    #[inline]
    fn m_tan(self) -> f64 {
        libm::tan(self)
    }
    #[inline]
    fn m_atan(self) -> f64 {
        libm::atan(self)
    }
    #[inline]
    fn m_atan2(self, other: f64) -> f64 {
        libm::atan2(self, other)
    }
    #[inline]
    fn m_powf(self, n: f64) -> f64 {
        libm::pow(self, n)
    }
    #[inline]
    fn m_ln_1p(self) -> f64 {
        libm::log1p(self)
    }
}

/// Python's `math.hypot(x, y)`: CPython's `math_hypot_impl` and `vector_norm`
/// (identical in 3.12 and 3.14) for two arguments, not libm's `hypot`, which
/// can differ in the last bit. Every step is IEEE arithmetic with `sqrt` and a
/// fused multiply-add, both correctly rounded, so it gives the same result on
/// every platform and under either backend.
#[must_use]
#[inline]
pub fn py_hypot(x: f64, y: f64) -> f64 {
    let (x, y) = (x.abs(), y.abs());
    // `x > max` is false for NaN, so NaN never becomes the maximum and an
    // infinity wins over NaN, as in CPython.
    let mut max = 0.0;
    if x > max {
        max = x;
    }
    if y > max {
        max = y;
    }
    vector_norm(x, y, max, x.is_nan() || y.is_nan())
}

/// `x * y` as an unevaluated sum `hi + lo` (CPython `dl_mul`). The fused
/// product is exact here, unlike the expression rewrites the porting rules
/// forbid elsewhere.
#[inline]
fn dl_mul(x: f64, y: f64) -> (f64, f64) {
    let z = x * y;
    (z, x.mul_add(y, -z))
}

/// `a + b` as `hi + lo` for `|a| >= |b|` (CPython `dl_fast_sum`).
#[inline]
fn dl_fast_sum(a: f64, b: f64) -> (f64, f64) {
    let x = a + b;
    (x, (a - x) + b)
}

/// CPython's `vector_norm` for `n == 2`, with `x` and `y` already absolute.
#[inline]
fn vector_norm(x: f64, y: f64, max: f64, found_nan: bool) -> f64 {
    if max.is_infinite() {
        return max;
    }
    if found_nan {
        return f64::NAN;
    }
    if max == 0.0 {
        return max;
    }
    let max_e = frexp_exponent(max);
    if max_e < -1023 {
        return vector_norm_tiny(x, y, max, found_nan);
    }
    let scale = pow2(-max_e);
    let (mut csum, mut frac1, mut frac2) = (1.0, 0.0, 0.0);
    for v in [x, y] {
        let v = v * scale; // lossless scaling
        let (pr_hi, pr_lo) = dl_mul(v, v); // lossless squaring
        let (sm_hi, sm_lo) = dl_fast_sum(csum, pr_hi); // lossless addition
        csum = sm_hi;
        frac1 += pr_lo;
        frac2 += sm_lo;
    }
    let mut h = (csum - 1.0 + (frac1 + frac2)).sqrt();
    let (pr_hi, pr_lo) = dl_mul(-h, h);
    let (sm_hi, sm_lo) = dl_fast_sum(csum, pr_hi);
    csum = sm_hi;
    frac1 += pr_lo;
    frac2 += sm_lo;
    let correction = csum - 1.0 + (frac1 + frac2); // CPython's `x`
    h += correction / (2.0 * h); // differential correction
    h / scale
}

/// The `max_e < -1023` branch of `vector_norm`, where `ldexp(1.0, -max_e)`
/// would overflow: rescale the subnormals into the normal range first. Kept out
/// of line so the common path inlines.
#[cold]
#[inline(never)]
fn vector_norm_tiny(x: f64, y: f64, max: f64, found_nan: bool) -> f64 {
    f64::MIN_POSITIVE
        * vector_norm(
            x / f64::MIN_POSITIVE,
            y / f64::MIN_POSITIVE,
            max / f64::MIN_POSITIVE,
            found_nan,
        )
}

/// The exponent C's `frexp` reports for a finite `v >= 0`: `v = m * 2**e`
/// with `0.5 <= m < 1`, and 0 for zero.
#[inline]
fn frexp_exponent(v: f64) -> i32 {
    let biased = i32::try_from((v.to_bits() >> 52) & 0x7ff).expect("11 bits");
    if biased == 0 {
        return subnormal_frexp_exponent(v);
    }
    biased - 1022
}

/// `frexp_exponent` for zero or a subnormal (biased exponent 0). Zero gives 0,
/// as in C; scaling it would recurse forever. A subnormal scales into the
/// normal range by 2**54, which is exact.
#[cold]
#[inline(never)]
fn subnormal_frexp_exponent(v: f64) -> i32 {
    if v == 0.0 {
        return 0;
    }
    frexp_exponent(v * pow2(54)) - 54
}

/// `2**k` exactly (C's `ldexp(1.0, k)`) for `-1074 <= k <= 1023`.
#[inline]
fn pow2(k: i32) -> f64 {
    debug_assert!((-1074..=1023).contains(&k));
    if k >= -1022 {
        f64::from_bits(u64::try_from(k + 1023).expect("normal exponent") << 52)
    } else {
        f64::from_bits(1_u64 << (k + 1074))
    }
}

#[cfg(test)]
mod tests {
    use super::py_hypot;

    /// `(x, y, math.hypot(x, y))` as bits, from CPython 3.14.4 (3.12 agrees).
    fn check(cases: &[(u64, u64, u64)]) {
        for &(x, y, want) in cases {
            let got = py_hypot(f64::from_bits(x), f64::from_bits(y));
            assert_eq!(got.to_bits(), want, "hypot({x:#x}, {y:#x})");
            let swapped = py_hypot(f64::from_bits(y), f64::from_bits(x));
            assert_eq!(swapped.to_bits(), want, "hypot({y:#x}, {x:#x})");
        }
    }

    #[test]
    fn matches_cpython_where_glibc_hypot_does_not() {
        // A sample of the 12,715 disagreements among 3.5 M inputs on glibc
        // 2.43; the port matched CPython on all 3.5 M under both backends.
        check(&[
            (
                0x40b9_6eb1_1bf6_c31f,
                0x4087_2f51_4ec3_9139,
                0x40b9_98d3_c9a6_833f,
            ), // 6510.69.., 741.91.. (glibc 6552.82729569153)
            (
                0xc098_b169_bb9f_4998,
                0x4074_5dab_2b94_7859,
                0x4099_3664_0293_5dcf,
            ), // -1580.35.., 325.85.. (glibc 1613.5976660753258)
            (
                0x4061_45f2_5495_d020,
                0x4048_f189_f0d8_54dd,
                0x4062_5d48_4938_54b6,
            ), // 138.18.., 49.88.. (glibc 146.91507397653706)
            (
                0x4092_2f99_74e9_80c0,
                0xc082_6bc1_0cfb_c0fc,
                0x4094_62a3_84ba_87ee,
            ), // 1163.89.., -589.46.. (glibc 1304.659686007067)
            (
                0xbff5_3295_8938_30c2,
                0x3fe8_fd9c_bcb0_e81c,
                0x3ff8_9b38_6311_a04d,
            ), // -1.32.., 0.78.. (glibc 1.5378955716969527)
            (
                0x3fed_2c15_e4ab_9c54,
                0xbff7_896b_ed4d_396c,
                0x3ffb_b0a3_2edd_9b11,
            ), // 0.91.., -1.47.. (glibc 1.730624373501716)
            (
                0x3fda_2d5f_844b_c73e,
                0x3fe4_b603_b956_3305,
                0x3fe8_8009_afce_b7f9,
            ), // 0.40.., 0.64.. (glibc 0.7656296190014009)
            (
                0xbffd_c48e_95ef_6bd4,
                0xbfff_4410_4ed9_8302,
                0x4005_95cd_2872_191c,
            ), // -1.86.., -1.95.. (glibc 2.698145214063642)
            (
                0xe875_7adb_7312_1f9c,
                0xe82d_323b_91f6_39a7,
                0x6875_7fd0_c7ec_1838,
            ), // -1.56e195, -6.66e193
            (
                0xaa90_48f5_3b17_223a,
                0x2aa7_229a_15d9_bf57,
                0x2aa8_86b7_1db1_f34c,
            ), // -1.13e-103, 3.22e-103
            (
                0x8164_8b5d_488b_60e9,
                0x0164_91d3_e40c_76bd,
                0x016d_126f_a451_4c86,
            ), // -5.99e-302, 5.99e-302
            (
                0x0157_7dea_306a_72b6,
                0x016f_2d80_149b_a022,
                0x0170_a891_4070_6dc8,
            ), // 3.42e-302, 9.09e-302
            (
                0x7fa6_95e0_7c5c_c74e,
                0x7fa1_e91b_3714_d09a,
                0x7fac_d33f_79bb_c7c5,
            ), // 7.93e306, 6.28e306
            (
                0xffdf_8947_1744_837b,
                0x7fc5_0f65_94d0_7aea,
                0x7fe0_9fb8_4221_4fca,
            ), // -8.85e307, 2.95e307
        ]);
    }

    #[test]
    fn matches_cpython_at_the_edges_of_the_range() {
        check(&[
            (
                0x4008_0000_0000_0000,
                0xc010_0000_0000_0000,
                0x4014_0000_0000_0000,
            ), // 3, -4 -> 5
            (
                0x0000_0000_0000_0001,
                0x0000_0000_0000_0001,
                0x0000_0000_0000_0001,
            ), // smallest subnormals
            (
                0x0000_0000_0000_0001,
                0x0000_0000_0000_0003,
                0x0000_0000_0000_0003,
            ), // rescaled recursion
            (
                0x000c_0000_0000_0000,
                0x0002_0000_0000_0000,
                0x000c_2a5f_d9b1_ee1c,
            ), // frexp exponent -1022
            (
                0x0008_0000_0000_0000,
                0x0006_0000_0000_0000,
                0x000a_0000_0000_0000,
            ), // frexp exponent -1022
            (
                0x0010_0000_0000_0000,
                0x0000_1268_8b70_e62b,
                0x0010_000a_96ee_eaa3,
            ), // smallest normal
            (
                0x7fef_ffff_ffff_ffff,
                0x7fef_ffff_ffff_ffff,
                0x7ff0_0000_0000_0000,
            ), // overflows to inf
            (
                0x7fef_ffff_ffff_ffff,
                0x7e37_e43c_8800_759c,
                0x7fef_ffff_ffff_ffff,
            ), // f64::MAX, 1e300
            (
                0x7e37_e43c_8800_759c,
                0x01a5_6e1f_c2f8_f359,
                0x7e37_e43c_8800_759c,
            ), // 1e300, 1e-300
            (
                0x7fe0_0000_0000_0000,
                0x0000_0000_0000_0000,
                0x7fe0_0000_0000_0000,
            ), // 2**1023, 0
            (
                0x8000_0000_0000_0000,
                0x8000_0000_0000_0000,
                0x0000_0000_0000_0000,
            ), // -0, -0 -> +0
        ]);
    }

    #[test]
    fn frexp_exponent_matches_c() {
        use super::{frexp_exponent, pow2};
        assert_eq!(frexp_exponent(0.0), 0);
        assert_eq!(frexp_exponent(-0.0), 0);
        assert_eq!(frexp_exponent(1.0), 1);
        assert_eq!(frexp_exponent(0.75), 0);
        assert_eq!(frexp_exponent(f64::MIN_POSITIVE), -1021);
        assert_eq!(frexp_exponent(pow2(-1074)), -1073);
        assert_eq!(frexp_exponent(f64::MAX), 1024);
    }

    #[test]
    fn infinity_wins_over_nan_as_in_cpython() {
        assert_eq!(py_hypot(f64::INFINITY, f64::NAN), f64::INFINITY);
        assert_eq!(py_hypot(f64::NAN, f64::NEG_INFINITY), f64::INFINITY);
        assert!(py_hypot(f64::NAN, 1.0).is_nan());
        assert!(py_hypot(0.0, f64::NAN).is_nan());
    }
}
