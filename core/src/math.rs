//! Transcendental functions the shaders take from libm, behind one switch.
//!
//! By default these call the platform libm through `f64`'s methods, as CPython's
//! `math` module does, so native output matches Python on the same machine. With
//! the `portable` feature they call the pure-Rust `libm` crate instead, which
//! gives the same result on every target (used to measure drift; see PLAN.md).
//! `hypot`, `sqrt` and `floor` are not routed here: `sqrt` and `floor` are exact
//! everywhere, and `hypot` will become a port of CPython's own algorithm.

/// Libm-backed methods on `f64`, named with an `m_` prefix so call sites read
/// like the `f64` methods they replace.
pub trait Libm {
    fn m_exp(self) -> f64;
    fn m_sin(self) -> f64;
    fn m_cos(self) -> f64;
    fn m_tan(self) -> f64;
    fn m_atan(self) -> f64;
    fn m_atan2(self, other: f64) -> f64;
    fn m_powf(self, n: f64) -> f64;
    fn m_ln_1p(self) -> f64;
}

#[cfg(not(feature = "portable"))]
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
