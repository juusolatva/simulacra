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
    use crate::styles::{Style, brush, drift, fields, mosaic, quasar, shapes, woodblock};

    type Stops = Vec<(i64, i64, i64)>;

    fn palette(stops: Stops) -> PyResult<Palette> {
        Palette::new(stops).map_err(PyValueError::new_err)
    }

    /// Names of the styles this module can render.
    #[pymodule_export]
    const STYLES: [&str; 38] = [
        "aurora",
        "blobs",
        "bloom",
        "circuit",
        "cosmic",
        "cosmos",
        "crystal",
        "drift",
        "galaxy",
        "gems",
        "geo",
        "horizon",
        "interference",
        "kaleidoscope",
        "mandala",
        "mandelbrot",
        "nebula",
        "orbits",
        "petals",
        "pinwheel",
        "plasma",
        "prism",
        "quasar",
        "rings",
        "shard",
        "spiral",
        "starburst",
        "voronoi",
        "waves",
        "torus",
        "lissajous",
        "constellations",
        "ink",
        "flowfield",
        "comet",
        "ember",
        "mosaic",
        "woodblock",
    ];

    fn extract_style(style: &str, scene: &Bound<'_, PyAny>) -> PyResult<Box<dyn Style>> {
        Ok(match style {
            "aurora" => Box::new(scene.extract::<fields::Aurora>()?),
            "blobs" => Box::new(scene.extract::<fields::Blobs>()?),
            "bloom" => Box::new(scene.extract::<fields::Bloom>()?),
            "circuit" => Box::new(scene.extract::<shapes::Circuit>()?),
            "cosmic" => Box::new(scene.extract::<fields::Cosmic>()?),
            "cosmos" => Box::new(scene.extract::<fields::Cosmos>()?),
            "crystal" => Box::new(scene.extract::<fields::Crystal>()?),
            "drift" => Box::new(scene.extract::<drift::Drift>()?),
            "galaxy" => Box::new(scene.extract::<fields::Galaxy>()?),
            "gems" => Box::new(scene.extract::<fields::Gems>()?),
            "geo" => Box::new(scene.extract::<shapes::Geo>()?),
            "horizon" => Box::new(scene.extract::<fields::Horizon>()?),
            "interference" => Box::new(scene.extract::<fields::Interference>()?),
            "kaleidoscope" => Box::new(scene.extract::<fields::Kaleidoscope>()?),
            "mandala" => Box::new(scene.extract::<shapes::Mandala>()?),
            "mandelbrot" => Box::new(scene.extract::<fields::Mandelbrot>()?),
            "nebula" => Box::new(scene.extract::<fields::Nebula>()?),
            "orbits" => Box::new(scene.extract::<fields::Orbits>()?),
            "petals" => Box::new(scene.extract::<fields::Petals>()?),
            "pinwheel" => Box::new(scene.extract::<fields::Pinwheel>()?),
            "plasma" => Box::new(scene.extract::<fields::Plasma>()?),
            "prism" => Box::new(scene.extract::<shapes::Prism>()?),
            "quasar" => Box::new(scene.extract::<quasar::Quasar>()?),
            "rings" => Box::new(scene.extract::<fields::Rings>()?),
            "shard" => Box::new(scene.extract::<shapes::Shard>()?),
            "spiral" => Box::new(scene.extract::<fields::Spiral>()?),
            "starburst" => Box::new(scene.extract::<fields::Starburst>()?),
            "voronoi" => Box::new(scene.extract::<fields::Voronoi>()?),
            "waves" => Box::new(scene.extract::<fields::Waves>()?),
            "torus" => Box::new(scene.extract::<brush::Torus>()?),
            "lissajous" => Box::new(scene.extract::<brush::Lissajous>()?),
            "constellations" => Box::new(scene.extract::<brush::Constellations>()?),
            "ink" => Box::new(scene.extract::<brush::Ink>()?),
            "flowfield" => Box::new(scene.extract::<brush::Flowfield>()?),
            "comet" => Box::new(scene.extract::<brush::Comet>()?),
            "ember" => Box::new(scene.extract::<brush::Ember>()?),
            "mosaic" => Box::new(scene.extract::<mosaic::Mosaic>()?),
            "woodblock" => Box::new(scene.extract::<woodblock::Woodblock>()?),
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

    /// fBm noise in [0, 1]; same results as `abstract_art.fbm`. A `phase`
    /// (1.0 repeats 0.0) makes it evolve over a loop.
    #[pyfunction]
    #[pyo3(signature = (x, y, seed, octaves, phase=0.0))]
    fn fbm(x: f64, y: f64, seed: u32, octaves: u32, phase: f64) -> f64 {
        crate::noise::fbm_loop(x, y, seed, octaves, phase)
    }

    /// Palette lookup; same results as `abstract_art.palette_color` (clamped to 0..=255).
    #[pyfunction]
    fn palette_color(stops: Stops, t: f64) -> PyResult<(u8, u8, u8)> {
        let [r, g, b] = palette(stops)?.color(t);
        Ok((r, g, b))
    }
}
