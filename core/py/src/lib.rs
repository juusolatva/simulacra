//! The `simulacra_core` Python extension: a thin adapter over `simulacra_engine`.
//!
//! Python validates arguments and generates each scene with `random.Random`,
//! so seeds stay compatible; this crate extracts the scene dict into the
//! engine's scene struct, renders it with the GIL released and returns packed
//! RGB bytes (row-major, 3 bytes per pixel).

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use simulacra_engine::{Palette, Style, for_each_style};

/// Extracts `scene` into the scene struct registered for `style`.
macro_rules! style_dispatch {
    ($($name:literal => $scene:path),* $(,)?) => {
        fn extract_style(style: &str, scene: &Bound<'_, PyAny>) -> PyResult<Box<dyn Style>> {
            Ok(match style {
                $($name => Box::new(scene.extract::<$scene>()?),)*
                _ => {
                    return Err(PyValueError::new_err(format!(
                        "no native renderer for style {style:?}"
                    )));
                }
            })
        }
    };
}

for_each_style!(style_dispatch);

type Stops = Vec<(i64, i64, i64)>;

fn palette(stops: Stops) -> PyResult<Palette> {
    Palette::new(stops).map_err(|err| PyValueError::new_err(err.to_string()))
}

#[pyo3::pymodule]
mod simulacra_core {
    use pyo3::prelude::*;
    use pyo3::types::PyBytes;

    use super::{Stops, extract_style, palette};

    /// True when built with the `portable` feature (the pure-Rust `libm` crate),
    /// whose output can differ from Python's; see `PORTABLE_DRIFT_STYLES` in Python.
    #[pymodule_export]
    const PORTABLE: bool = simulacra_engine::PORTABLE;

    /// Names of the styles this module can render.
    #[pymodule_export]
    const STYLES: [&str; simulacra_engine::STYLES.len()] = simulacra_engine::STYLES;

    /// The engine's built-in palettes as `(name, stops)` pairs; a Python test
    /// keeps them equal to `simulacra.PALETTES`.
    #[pyfunction]
    fn palettes() -> Vec<(&'static str, Vec<simulacra_engine::palettes::Stop>)> {
        simulacra_engine::PALETTES
            .iter()
            .map(|&(name, stops)| (name, stops.to_vec()))
            .collect()
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

    /// fBm noise in [0, 1]; same results as `simulacra.fbm`. A `phase`
    /// (1.0 repeats 0.0) makes it evolve over a loop.
    #[pyfunction]
    #[pyo3(signature = (x, y, seed, octaves, phase=0.0))]
    fn fbm(x: f64, y: f64, seed: u32, octaves: u32, phase: f64) -> f64 {
        simulacra_engine::noise::fbm_loop(x, y, seed, octaves, phase)
    }

    /// Palette lookup; same results as `simulacra.palette_color` (clamped to 0..=255).
    #[pyfunction]
    fn palette_color(stops: Stops, t: f64) -> PyResult<(u8, u8, u8)> {
        let [r, g, b] = palette(stops)?.color(t);
        Ok((r, g, b))
    }
}
