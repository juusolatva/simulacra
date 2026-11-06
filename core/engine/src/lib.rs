//! Simulacra's renderer, independent of Python.
//!
//! Each style is a scene struct (in [`styles`]) that implements [`Style`]: given
//! a size, a seed and a [`Palette`], it shades the pixels in parallel and
//! returns packed RGB bytes (row-major, 3 bytes per pixel). Scenes are built by
//! the Python scene builders in `simulacra.py` and converted by the `py` adapter
//! crate, which turns on the `python` feature to derive `pyo3::FromPyObject` on
//! them. Without that feature this crate does not depend on Python or PyO3.

mod geometry;
mod math;
pub mod noise;
pub mod palette;
mod raster;
pub mod styles;

pub use palette::{Palette, PaletteError};
pub use styles::Style;

/// True when built with the `portable` feature (the pure-Rust `libm` crate),
/// whose output can differ from Python's; see `PORTABLE_DRIFT_STYLES` in Python.
pub const PORTABLE: bool = cfg!(feature = "portable");

/// The style registry: calls `$callback! { "name" => SceneType, ... }` with
/// every style the engine renders, in the order of [`STYLES`]. This is the only
/// list of styles on the Rust side; [`STYLES`] and the adapter's dispatch are
/// both generated from it.
#[macro_export]
macro_rules! for_each_style {
    ($callback:ident) => {
        $callback! {
            "aurora" => $crate::styles::fields::Aurora,
            "blobs" => $crate::styles::fields::Blobs,
            "bloom" => $crate::styles::fields::Bloom,
            "circuit" => $crate::styles::shapes::Circuit,
            "cosmic" => $crate::styles::fields::Cosmic,
            "cosmos" => $crate::styles::fields::Cosmos,
            "crystal" => $crate::styles::fields::Crystal,
            "cymatics" => $crate::styles::fields::Cymatics,
            "drift" => $crate::styles::drift::Drift,
            "galaxy" => $crate::styles::fields::Galaxy,
            "gems" => $crate::styles::fields::Gems,
            "geo" => $crate::styles::shapes::Geo,
            "horizon" => $crate::styles::fields::Horizon,
            "interference" => $crate::styles::fields::Interference,
            "kaleidoscope" => $crate::styles::fields::Kaleidoscope,
            "mandala" => $crate::styles::shapes::Mandala,
            "mandelbrot" => $crate::styles::fields::Mandelbrot,
            "nebula" => $crate::styles::fields::Nebula,
            "optics" => $crate::styles::fields::Optics,
            "orbits" => $crate::styles::fields::Orbits,
            "petals" => $crate::styles::fields::Petals,
            "pinwheel" => $crate::styles::fields::Pinwheel,
            "plasma" => $crate::styles::fields::Plasma,
            "prism" => $crate::styles::shapes::Prism,
            "quasar" => $crate::styles::quasar::Quasar,
            "rings" => $crate::styles::fields::Rings,
            "shard" => $crate::styles::shapes::Shard,
            "spiral" => $crate::styles::fields::Spiral,
            "starburst" => $crate::styles::fields::Starburst,
            "voronoi" => $crate::styles::fields::Voronoi,
            "waves" => $crate::styles::fields::Waves,
            "torus" => $crate::styles::brush::Torus,
            "lissajous" => $crate::styles::brush::Lissajous,
            "constellations" => $crate::styles::brush::Constellations,
            "ink" => $crate::styles::brush::Ink,
            "flowfield" => $crate::styles::brush::Flowfield,
            "comet" => $crate::styles::brush::Comet,
            "ember" => $crate::styles::brush::Ember,
            "mosaic" => $crate::styles::mosaic::Mosaic,
            "woodblock" => $crate::styles::woodblock::Woodblock,
            "contour" => $crate::styles::fields::Contour,
            "marbling" => $crate::styles::fields::Marbling,
            "attractor" => $crate::styles::brush::Attractor,
            "opart" => $crate::styles::fields::Opart,
            "lunar" => $crate::styles::space::Lunar,
            "asteroids" => $crate::styles::space::Asteroids,
            "blackhole" => $crate::styles::space::Blackhole,
            "supernova" => $crate::styles::space::Supernova,
        }
    };
}

macro_rules! style_names {
    ($($name:literal => $scene:path),* $(,)?) => {
        /// Names of the styles the engine renders, in registry order.
        pub const STYLES: [&str; [$($name),*].len()] = [$($name),*];
    };
}

for_each_style!(style_names);

#[cfg(test)]
mod tests {
    use super::STYLES;

    #[test]
    fn style_names_are_unique_and_lowercase() {
        let mut names = STYLES.to_vec();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), STYLES.len(), "duplicate style name");
        for name in STYLES {
            assert!(name.bytes().all(|b| b.is_ascii_lowercase()), "{name}");
        }
    }
}
