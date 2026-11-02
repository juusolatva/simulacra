//! Native renderers for `abstract_art.py`.
//!
//! Python validates arguments and generates each scene with `random.Random`,
//! so seeds stay compatible; this crate shades the pixels in parallel and
//! returns packed RGB bytes (row-major, 3 bytes per pixel).

mod geometry;
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
    use crate::styles::{Style, drift, fields, quasar, shapes};

    type Stops = Vec<(i64, i64, i64)>;

    fn palette(stops: Stops) -> PyResult<Palette> {
        Palette::new(stops).map_err(PyValueError::new_err)
    }

    /// Names of the styles this module can render.
    #[pymodule_export]
    const STYLES: [&str; 18] = [
        "blobs",
        "bloom",
        "circuit",
        "cosmic",
        "drift",
        "galaxy",
        "geo",
        "interference",
        "kaleidoscope",
        "mandala",
        "nebula",
        "orbits",
        "pinwheel",
        "plasma",
        "prism",
        "quasar",
        "shard",
        "waves",
    ];

    fn extract_style(style: &str, scene: &Bound<'_, PyAny>) -> PyResult<Box<dyn Style>> {
        Ok(match style {
            "blobs" => Box::new(fields::Blobs),
            "bloom" => Box::new(scene.extract::<fields::Bloom>()?),
            "circuit" => Box::new(scene.extract::<shapes::Circuit>()?),
            "cosmic" => Box::new(scene.extract::<fields::Cosmic>()?),
            "drift" => Box::new(drift::Drift),
            "galaxy" => Box::new(scene.extract::<fields::Galaxy>()?),
            "geo" => Box::new(scene.extract::<shapes::Geo>()?),
            "interference" => Box::new(scene.extract::<fields::Interference>()?),
            "kaleidoscope" => Box::new(scene.extract::<fields::Kaleidoscope>()?),
            "mandala" => Box::new(scene.extract::<shapes::Mandala>()?),
            "nebula" => Box::new(scene.extract::<fields::Nebula>()?),
            "orbits" => Box::new(scene.extract::<fields::Orbits>()?),
            "pinwheel" => Box::new(scene.extract::<fields::Pinwheel>()?),
            "plasma" => Box::new(scene.extract::<fields::Plasma>()?),
            "prism" => Box::new(scene.extract::<shapes::Prism>()?),
            "quasar" => Box::new(scene.extract::<quasar::Quasar>()?),
            "shard" => Box::new(scene.extract::<shapes::Shard>()?),
            "waves" => Box::new(scene.extract::<fields::Waves>()?),
            _ => {
                return Err(PyValueError::new_err(format!(
                    "no native renderer for style {style:?}"
                )));
            }
        })
    }

    /// Render `style` from a scene dict built by the matching Python
    /// `_<style>_scene()`; returns packed RGB bytes.
    #[pyfunction]
    fn render<'py>(
        py: Python<'py>,
        style: &str,
        width: usize,
        height: usize,
        seed: u32,
        stops: Stops,
        scene: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let palette = palette(stops)?;
        let renderer = extract_style(style, scene)?;
        let rgb = py.detach(|| renderer.render(width, height, seed, &palette));
        Ok(PyBytes::new(py, &rgb))
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
}
