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

#[cfg(feature = "serde")]
macro_rules! style_deserialize {
    ($($name:literal => $scene:path),* $(,)?) => {
        /// Reads the scene for `style` from any serde format. The fields are
        /// those of the dict the Python `_<style>_scene()` builder returns;
        /// unknown fields are ignored, as the Python adapter ignores them.
        ///
        /// # Errors
        ///
        /// Fails if `style` is not in [`STYLES`] or the data does not match its
        /// scene.
        pub fn deserialize_scene<'de, D>(style: &str, deserializer: D) -> Result<Box<dyn Style>, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            use serde::Deserialize as _;
            Ok(match style {
                $($name => Box::new(<$scene>::deserialize(deserializer)?),)*
                _ => {
                    return Err(serde::de::Error::custom(format_args!(
                        "no native renderer for style {style:?}"
                    )));
                }
            })
        }
    };
}

#[cfg(feature = "serde")]
for_each_style!(style_deserialize);

/// Reads the scene for `style` from JSON text; see [`deserialize_scene`].
/// Floats are parsed exactly (`serde_json`'s `float_roundtrip`).
///
/// # Errors
///
/// Fails if the JSON is malformed, `style` is unknown or the data does not
/// match its scene.
#[cfg(feature = "json")]
pub fn scene_from_json(style: &str, json: &str) -> Result<Box<dyn Style>, serde_json::Error> {
    let mut deserializer = serde_json::Deserializer::from_str(json);
    let scene = deserialize_scene(style, &mut deserializer)?;
    deserializer.end()?;
    Ok(scene)
}

#[cfg(test)]
mod tests {
    use super::STYLES;

    #[cfg(feature = "json")]
    #[test]
    fn json_scenes_load_and_unknown_styles_fail() {
        let scene = super::scene_from_json("drift", r#"{"phase": 0.25, "extra": [1, 2]}"#);
        let palette = super::Palette::new(vec![(0, 0, 0), (255, 255, 255)]).unwrap();
        let rgb = scene.expect("drift scene").render(4, 3, 7, &palette);
        assert_eq!(rgb.len(), 4 * 3 * 3);
        let err = super::scene_from_json("nosuchstyle", "{}")
            .err()
            .expect("unknown style");
        assert!(err.to_string().contains("nosuchstyle"), "{err}");
        let missing = super::scene_from_json("drift", "{}");
        assert!(missing.is_err(), "missing field");
        let trailing = super::scene_from_json("drift", r#"{"phase": 0.0} 1"#);
        assert!(trailing.is_err(), "trailing data");
    }

    /// serde_json without `float_roundtrip` parses `significand / 10**k` with a
    /// float division, which misrounds these shortest-repr floats (found by
    /// simulating that path); scenes would then not load exactly.
    #[test]
    fn json_floats_parse_exactly() {
        for (text, bits) in FLOAT_CASES {
            let value: f64 = serde_json::from_str(text).expect("float");
            assert_eq!(value.to_bits(), *bits, "{text}");
        }
    }

    /// `(Python repr, bits)`; the comment gives the misrounded naive result.
    const FLOAT_CASES: &[(&str, u64)] = &[
        ("-1.5471761413874225", 0xbff8_c13b_c506_9136), // naive: -1.5471761413874223
        ("-2042.3747258517706", 0xc09f_e97f_b822_3948), // naive: -2042.3747258517703
        ("-1.4449303264043873", 0xbff7_1e6f_430e_7cb8), // naive: -1.4449303264043871
        ("-1743.2617050292927", 0xc09b_3d0b_fc67_380b), // naive: -1743.261705029293
    ];

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
