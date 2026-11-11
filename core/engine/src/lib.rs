//! Simulacra's renderer.
//!
//! Each style is a scene struct (in [`styles`]) that implements [`Style`]: given
//! a size, a seed and a [`Palette`], it shades the pixels in parallel and
//! returns packed RGB bytes (row-major, 3 bytes per pixel). [`build_scene`]
//! builds a style's scene (all its random draws, through [`PyRandom`]).
//!
//! The engine was ported from a Python reference, `simulacra.py`, which was
//! retired when Simulacra became a Rust-only project; it can be read in git
//! history before that change. Comments that name a Python function (such as
//! `_mosaic_scene` or `render_waves`) or Python semantics refer to that
//! reference, which the shaders and scene builders reproduce exactly.

mod geometry;
mod math;
pub mod noise;
pub mod palette;
pub mod palettes;
pub mod random;
mod raster;
pub mod scenes;
pub mod styles;
pub mod validate;

/// Python's float `%`, for callers that compute phases as `simulacra.py` does.
pub use geometry::py_mod;
pub use palette::{Palette, PaletteError};
pub use palettes::{PALETTES, palette_stops};
pub use random::PyRandom;
#[cfg(feature = "json")]
pub use scenes::build_scene_json;
pub use scenes::{BuildScene, SCENE_BUILDERS, SceneParams, build_scene};
pub use styles::Style;
pub use validate::{SceneError, scene_work, validate_scene, work_limit};

/// True when built with the `portable` feature (the pure-Rust `libm` crate),
/// whose output can differ from Python's; see `PORTABLE_DRIFT_STYLES` in Python.
pub const PORTABLE: bool = cfg!(feature = "portable");

/// Why [`render`] refused a canvas or scene.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RenderError {
    /// Width or height is zero.
    EmptyCanvas { width: usize, height: usize },
    /// `width * height * 3` bytes exceed the largest possible allocation
    /// (`isize::MAX` bytes).
    TooLarge { width: usize, height: usize },
    /// The scene does not fit the canvas or palette, or would take too long
    /// to render; see [`validate_scene`].
    Scene(SceneError),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::EmptyCanvas { width, height } => {
                write!(
                    f,
                    "width and height must be positive, not {width} x {height}"
                )
            }
            Self::TooLarge { width, height } => {
                write!(f, "{width} x {height} pixels do not fit in memory")
            }
            Self::Scene(err) => err.fmt(f),
        }
    }
}

/// `Display` includes the scene error's message, so `source` is not set.
impl std::error::Error for RenderError {}

impl From<SceneError> for RenderError {
    fn from(err: SceneError) -> Self {
        Self::Scene(err)
    }
}

/// Renders `scene` as packed RGB (`width * height * 3` bytes, row-major) after
/// checking the canvas (both sides positive, the buffer no larger than Rust
/// can allocate, `isize::MAX` bytes) and the scene ([`validate_scene`]). A
/// buffer below that can still exhaust memory; callers that take sizes from
/// users should also apply a limit, as the CLI does.
/// `seed` is the shaders' 32-bit seed; see [`seed32`].
///
/// # Errors
///
/// [`RenderError`] if the canvas is empty or too large to address, or the
/// scene is invalid for it.
pub fn render(
    scene: &dyn Style,
    width: usize,
    height: usize,
    seed: u32,
    palette: &Palette,
) -> Result<Vec<u8>, RenderError> {
    if width == 0 || height == 0 {
        return Err(RenderError::EmptyCanvas { width, height });
    }
    let bytes = width.checked_mul(height).and_then(|n| n.checked_mul(3));
    if bytes.is_none_or(|n| n > isize::MAX as usize) {
        return Err(RenderError::TooLarge { width, height });
    }
    validate_scene(scene, width, height, palette)?;
    Ok(scene.render(width, height, seed, palette))
}

/// Python's `_seed32(seed)`, `seed & 0xFFFFFFFF`: the noise hash sees a seed
/// modulo 2**32. For a negative `seed` that is its two's complement low word,
/// which is what the truncating cast gives.
#[must_use]
pub fn seed32(seed: i64) -> u32 {
    seed as u32
}

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
        /// Only the types are checked here, not that the data is consistent:
        /// render the scene with [`render`], which validates it against the
        /// canvas and palette first ([`validate_scene`]). Calling
        /// [`Style::render`] directly on a scene from an untrusted source can
        /// panic (an index out of range) or run for a very long time (a huge
        /// iteration count).
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

/// Reads the scene for `style` from JSON text; see [`deserialize_scene`],
/// including why it must be rendered with [`render`]. Floats are
/// parsed exactly (`serde_json`'s `float_roundtrip`).
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

    #[test]
    fn render_checks_the_canvas() {
        use super::{RenderError, render, scenes::build_scene};
        let palette = super::Palette::new(vec![(0, 0, 0), (255, 255, 255)]).unwrap();
        let scene = build_scene("blobs", 4, 3, 1, &palette, 0.0).unwrap();
        assert_eq!(render(scene.as_ref(), 4, 3, 1, &palette).unwrap().len(), 36);
        let empty = render(scene.as_ref(), 0, 3, 1, &palette);
        assert_eq!(
            empty,
            Err(RenderError::EmptyCanvas {
                width: 0,
                height: 3
            })
        );
        let huge = render(scene.as_ref(), usize::MAX, 2, 1, &palette);
        assert_eq!(
            huge,
            Err(RenderError::TooLarge {
                width: usize::MAX,
                height: 2
            })
        );
        // Fits in a usize but not in an allocation (above isize::MAX bytes).
        let width = isize::MAX as usize / 3 + 1;
        let unallocatable = render(scene.as_ref(), width, 1, 1, &palette);
        assert_eq!(
            unallocatable,
            Err(RenderError::TooLarge { width, height: 1 })
        );
    }

    #[test]
    fn seed32_matches_python() {
        // seed & 0xFFFFFFFF in Python
        assert_eq!(super::seed32(3), 3);
        assert_eq!(super::seed32(-1), 0xffff_ffff);
        assert_eq!(super::seed32(1 << 32), 0);
        assert_eq!(super::seed32(-7), 4_294_967_289);
        assert_eq!(super::seed32(i64::MIN), 0);
    }

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
