//! The functions Python's `math` module takes from the platform libm.
//!
//! Shaders call these as methods (`x.m_exp()`), so swapping the backend never
//! changes the order of operations in an expression. The default `system`
//! backend calls the platform libm through `f64`, as Python does, which is what
//! the byte-identity tests against Python need. The `portable` feature uses the
//! pure-Rust `libm` crate instead, which gives the same result on every target
//! (built without its `arch` feature, which would pick per-target routines).
//!
//! Not routed here: `sqrt` and `floor`, which are exact everywhere; `powi`,
//! which is multiplication; and `hypot`, which CPython implements itself, so
//! it needs a port of CPython's algorithm rather than a libm backend.

pub trait Libm {
    fn m_exp(self) -> Self;
    fn m_sin(self) -> Self;
    fn m_cos(self) -> Self;
    fn m_tan(self) -> Self;
    fn m_atan(self) -> Self;
    fn m_atan2(self, x: Self) -> Self;
    fn m_powf(self, n: Self) -> Self;
    fn m_ln_1p(self) -> Self;
}

#[cfg(not(feature = "portable"))]
#[allow(clippy::disallowed_methods)] // The one place that calls them directly.
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
    fn m_atan2(self, x: f64) -> f64 {
        self.atan2(x)
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
    fn m_atan2(self, x: f64) -> f64 {
        libm::atan2(self, x)
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

#[cfg(test)]
mod tests {
    use super::Libm;

    #[test]
    fn methods_compute_the_named_functions() {
        let x = 0.3_f64;
        assert!((x.m_exp() - 1.349_858_807_576_003).abs() < 1e-15);
        assert!((x.m_sin() - 0.295_520_206_661_339_6).abs() < 1e-15);
        assert!((x.m_cos() - 0.955_336_489_125_606).abs() < 1e-15);
        assert!((x.m_tan() - 0.309_336_249_609_623_3).abs() < 1e-15);
        assert!((x.m_atan() - 0.291_456_794_477_867_1).abs() < 1e-15);
        assert!((x.m_atan2(-1.0) - 2.850_135_859_111_926).abs() < 1e-15);
        assert!((x.m_powf(2.5) - 0.049_295_030_175_464_95).abs() < 1e-15);
        assert!((x.m_ln_1p() - 0.262_364_264_467_491).abs() < 1e-15);
    }
}
