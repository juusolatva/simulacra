//! Native renderers for `abstract_art.py`.
//!
//! Python validates arguments and generates each scene with `random.Random`,
//! so seeds stay compatible; this crate shades the pixels in parallel and
//! returns packed RGB bytes (row-major, 3 bytes per pixel).

mod noise;
mod palette;
mod raster;
mod styles;

#[pyo3::pymodule]
mod abstract_art_core {
    use pyo3::exceptions::PyValueError;
    use pyo3::prelude::*;
    use pyo3::types::PyBytes;

    use crate::palette::Palette;
    use crate::styles::{drift, quasar};

    type Stops = Vec<(i64, i64, i64)>;

    fn palette(stops: Stops) -> PyResult<Palette> {
        Palette::new(stops).map_err(PyValueError::new_err)
    }

    /// fBm noise in [0, 1]; same results as `abstract_art.fbm`.
    #[pyfunction]
    fn fbm(x: f64, y: f64, seed: u32, octaves: u32) -> f64 {
        crate::noise::fbm(x, y, seed, octaves)
    }

    /// Palette lookup; same results as `abstract_art.palette_color` (clamped to 0..=255).
    #[pyfunction]
    fn palette_color(stops: Stops, t: f64) -> PyResult<(u8, u8, u8)> {
        let [r, g, b] = palette(stops)?.color(t);
        Ok((r, g, b))
    }

    #[pyfunction]
    fn render_drift<'py>(
        py: Python<'py>,
        width: usize,
        height: usize,
        seed: u32,
        stops: Stops,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let palette = palette(stops)?;
        let rgb = py.detach(|| drift::render(width, height, seed, &palette));
        Ok(PyBytes::new(py, &rgb))
    }

    #[pyfunction]
    #[allow(clippy::too_many_arguments)]
    fn render_quasar<'py>(
        py: Python<'py>,
        width: usize,
        height: usize,
        seed: u32,
        stops: Stops,
        center: (f64, f64),
        beta: f64,
        r_shadow: f64,
        r_outer: f64,
        jet_axis: (f64, f64),
        jet_half_angle: f64,
        jet_len: f64,
        orbits: Vec<(f64, u32)>,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let palette = palette(stops)?;
        let scene = quasar::Quasar {
            seed,
            center,
            beta,
            r_shadow,
            r_outer,
            jet_axis,
            jet_half_angle,
            jet_len,
            orbits,
        };
        let rgb = py.detach(|| scene.render(width, height, &palette));
        Ok(PyBytes::new(py, &rgb))
    }
}
