//! `render_quasar`: accretion disk, lensing ring and Doppler-beamed jets.
//!
//! Mirrors `_quasar_value()` in `simulacra.py`; the scene comes from
//! `_quasar_scene()` so both implementations share the same random layout.

use crate::math::{Libm, py_hypot};

use super::Style;
use crate::noise::{fbm, polar_noise};
use crate::palette::Palette;
use crate::validate::{Canvas, SceneError, check_numbers};
use crate::raster::shade;

#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Quasar {
    pub center: (f64, f64),
    pub beta: f64,
    pub r_shadow: f64,
    pub r_outer: f64,
    pub jet_axis: (f64, f64),
    pub jet_half_angle: f64,
    pub jet_len: f64,
    /// (orbit radius, turbulence seed) per accretion ring.
    pub orbits: Vec<(f64, u32)>,
}

/// Bipolar cone emission along the visible axis, fading toward the tips.
fn jet_glow(
    along: f64,
    perpendicular: f64,
    length: f64,
    opening: f64,
    shadow: f64,
    beta: f64,
) -> f64 {
    let distance = along.abs();
    if distance <= shadow || distance >= length {
        return 0.0;
    }
    let cone_width = (opening * (distance - shadow)).max(0.75);
    let q = perpendicular / cone_width;
    let cross_section = (-0.5 * (q * q)).m_exp();
    let launch = ((distance - shadow) / (shadow * 0.5).max(1.0)).min(1.0);
    let beaming = 0.35 + 0.65 * (if along > 0.0 { beta } else { 1.0 - beta });
    3.0 * cross_section * launch * (-2.0 * distance / length).m_exp() * beaming
}

impl Quasar {
    fn value(&self, x: usize, y: usize, seed: u32) -> f64 {
        let dx = x as f64 - self.center.0;
        let dy = y as f64 - self.center.1;
        let (r_shadow, r_outer) = (self.r_shadow, self.r_outer);
        let r = py_hypot(dx, dy);
        let theta = dy.m_atan2(dx);
        let mut v = 0.0;
        // gravitational lensing glow ring around the shadow
        let lens_r = r_shadow * 2.4;
        let lens_wob = 0.7 + 0.6 * polar_noise(r * 0.35, theta, seed, 2.0, 2);
        let lens_w = ((r_shadow * 0.9).max(1.0) * lens_wob).max(0.4);
        let d = r - lens_r;
        v += (-(d * d) / (2.0 * lens_w * lens_w)).m_exp() * 2.2;
        // black hole shadow with a hot inner edge
        if r < r_shadow {
            let q = (r - r_shadow) / (r_shadow * 0.15).max(1.0);
            v += (-(q * q) / 2.0).m_exp() * 2.5;
        }
        // relativistic accretion disk: glowing orbit rings, hotter toward the hole
        let doppler = 0.35 + 0.65 * (0.5 - 0.5 * theta.m_cos());
        let disk_t = (r - r_shadow) / (r_outer - r_shadow).max(1.0);
        let disk_level = 1.0 - 0.45 * (r / r_outer.max(1.0));
        for &(ro, orbit_seed) in &self.orbits {
            let w = ((ro - r_shadow * 1.2) * 0.06).max(0.4);
            let wob = 0.35 + 0.65 * polar_noise(r * 0.5, theta, orbit_seed, 3.0, 3);
            let d = r - (ro + w * (wob - 0.5));
            let g = (-(d * d) / (2.0 * w * w)).m_exp() * doppler * disk_level;
            v += g * (1.5 - 0.6 * disk_t);
        }
        // relativistic jets along the spin axis, with cone and Doppler beaming
        let (ux, uy) = self.jet_axis;
        let along = dx * ux + dy * uy;
        let perp = -dx * uy + dy * ux;
        v += jet_glow(
            along,
            perp,
            self.jet_len,
            self.jet_half_angle,
            r_shadow,
            self.beta,
        );
        // faint starfield backdrop
        let n = fbm(x as f64 * 0.004, y as f64 * 0.004, seed, 2);
        v += (n - 0.5).max(0.0) * 0.6;
        1.0 - (-0.75 * v).m_exp()
    }
}

impl Style for Quasar {
    fn validate(&self, canvas: &Canvas) -> Result<f64, SceneError> {
        check_numbers!(
            self,
            center,
            beta,
            r_shadow,
            r_outer,
            jet_axis,
            jet_half_angle,
            jet_len,
            orbits
        );
        Ok(canvas.per_pixel(self.orbits.len() as f64))
    }

    fn render(&self, width: usize, height: usize, seed: u32, palette: &Palette) -> Vec<u8> {
        shade(width, height, palette, |x, y| self.value(x, y, seed))
    }
}
