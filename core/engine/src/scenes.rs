//! Scene builders ported from Python, so the engine can render a style on its
//! own. Each matches the Python `_<style>_scene()` draw for draw (through
//! [`PyRandom`](crate::PyRandom)); styles not ported yet return `None` and still
//! need their scene from Python (through the adapter or a scene file).
//!
//! A ported style implements [`BuildScene`] on its scene struct and gets one
//! line in `scene_builders!` below, which generates [`SCENE_BUILDERS`],
//! [`build_scene`] and (with the `json` feature) [`build_scene_json`].

use crate::palette::Palette;
use crate::styles::{Style, drift, fields};
use crate::validate::{SceneError, check_loop_phase};

/// What a scene builder receives: the arguments of
/// `simulacra.NATIVE_STYLES[style](width, height, seed, stops, phase)`.
#[derive(Clone, Copy)]
pub struct SceneParams<'a> {
    pub width: usize,
    pub height: usize,
    /// The full seed: the random generator sees all of it, while the shaders
    /// take [`seed32`](crate::seed32) of it.
    pub seed: i64,
    /// Passed because some builders depend on the number of stops.
    pub palette: &'a Palette,
    pub phase: f64,
}

/// A scene struct whose Python builder is ported to Rust.
pub trait BuildScene: Style + Sized + 'static {
    /// Builds the scene exactly as the Python `_<style>_scene()` does.
    fn build(params: &SceneParams<'_>) -> Self;
}

/// The one list of ported builders, in registry (alphabetical) order.
macro_rules! scene_builders {
    ($($name:literal => $scene:ty),* $(,)?) => {
        /// Styles whose scene the engine can build, in registry order.
        pub const SCENE_BUILDERS: [&str; [$($name),*].len()] = [$($name),*];

        /// Builds the scene for `style` as `simulacra.NATIVE_STYLES[style](width,
        /// height, seed, stops, phase)` does, or `None` if its builder is still
        /// Python only.
        #[must_use]
        pub fn build_scene(
            style: &str,
            width: usize,
            height: usize,
            seed: i64,
            palette: &Palette,
            phase: f64,
        ) -> Option<Box<dyn Style>> {
            let params = SceneParams { width, height, seed, palette, phase };
            Some(match style {
                $($name => Box::new(<$scene as BuildScene>::build(&params)),)*
                _ => return None,
            })
        }

        /// [`build_scene`] serialized as JSON, field names as in the Python
        /// scene dict and every float in its shortest exact form, so a test
        /// can compare the scene with the Python builder's value by value.
        ///
        /// # Errors
        ///
        /// [`SceneError::Number`] for a phase that is not finite: JSON has no
        /// NaN or infinity (`serde_json` would write `null`), and rendering
        /// rejects such a phase anyway.
        #[cfg(feature = "json")]
        pub fn build_scene_json(
            style: &str,
            width: usize,
            height: usize,
            seed: i64,
            palette: &Palette,
            phase: f64,
        ) -> Result<Option<String>, SceneError> {
            check_loop_phase(phase)?;
            let params = SceneParams { width, height, seed, palette, phase };
            Ok(Some(match style {
                $($name => to_json(&<$scene as BuildScene>::build(&params)),)*
                _ => return Ok(None),
            }))
        }
    };
}

scene_builders! {
    "aurora" => fields::Aurora,
    "blobs" => fields::Blobs,
    "bloom" => fields::Bloom,
    "cosmos" => fields::Cosmos,
    "crystal" => fields::Crystal,
    "drift" => drift::Drift,
    "galaxy" => fields::Galaxy,
    "gems" => fields::Gems,
    "horizon" => fields::Horizon,
    "interference" => fields::Interference,
    "kaleidoscope" => fields::Kaleidoscope,
    "mandelbrot" => fields::Mandelbrot,
    "nebula" => fields::Nebula,
    "orbits" => fields::Orbits,
    "petals" => fields::Petals,
    "pinwheel" => fields::Pinwheel,
    "plasma" => fields::Plasma,
    "rings" => fields::Rings,
    "spiral" => fields::Spiral,
    "starburst" => fields::Starburst,
    "voronoi" => fields::Voronoi,
    "waves" => fields::Waves,
}

#[cfg(feature = "json")]
fn to_json(scene: &impl serde::Serialize) -> String {
    // Scene structs hold only numbers, tuples and vectors, and every float a
    // builder makes is finite, so serialization cannot fail.
    serde_json::to_string(scene).expect("a built scene serializes")
}

#[cfg(test)]
mod tests {
    use super::{SCENE_BUILDERS, build_scene};
    use crate::{Palette, STYLES};

    #[test]
    fn builders_cover_exactly_the_listed_styles() {
        let palette = Palette::new(vec![(0, 0, 0), (255, 255, 255)]).unwrap();
        for style in STYLES {
            let built = build_scene(style, 8, 6, 1, &palette, 0.0).is_some();
            assert_eq!(built, SCENE_BUILDERS.contains(&style), "{style}");
        }
    }

    #[test]
    fn builders_are_listed_in_registry_order() {
        let mut sorted = SCENE_BUILDERS;
        sorted.sort_unstable();
        assert_eq!(sorted, SCENE_BUILDERS);
    }

    /// Every built scene passes validation, at sizes from a single pixel up,
    /// with negative and large phases.
    #[test]
    fn built_scenes_are_valid() {
        let palette = Palette::new(vec![(0, 0, 0), (90, 20, 160), (255, 255, 255)]).unwrap();
        for style in SCENE_BUILDERS {
            for (width, height) in [(1, 1), (8, 6), (1, 40), (300, 2)] {
                for phase in [0.0, 0.37, -2.25, 1e12] {
                    let scene = build_scene(style, width, height, -5, &palette, phase).unwrap();
                    crate::render(scene.as_ref(), width, height, 7, &palette)
                        .unwrap_or_else(|err| panic!("{style} {width}x{height} {phase}: {err}"));
                }
            }
        }
    }

    #[cfg(feature = "json")]
    #[test]
    fn json_scenes_load_back_into_the_same_style() {
        let palette = Palette::new(vec![(0, 0, 0), (255, 255, 255)]).unwrap();
        for style in SCENE_BUILDERS {
            let json = super::build_scene_json(style, 30, 20, 11, &palette, 0.4)
                .unwrap()
                .unwrap();
            let scene = crate::scene_from_json(style, &json).unwrap();
            let direct = build_scene(style, 30, 20, 11, &palette, 0.4).unwrap();
            assert_eq!(
                scene.render(30, 20, 11, &palette),
                direct.render(30, 20, 11, &palette),
                "{style}"
            );
        }
        assert!(
            super::build_scene_json("ink", 30, 20, 11, &palette, 0.4)
                .unwrap()
                .is_none()
        );
        for phase in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(super::build_scene_json("blobs", 30, 20, 11, &palette, phase).is_err());
        }
    }

    /// The diagonal-based builders square the sides exactly, so even sides far
    /// beyond any renderable canvas neither overflow nor wrap.
    #[test]
    fn huge_sides_do_not_overflow() {
        let palette = Palette::new(vec![(0, 0, 0), (255, 255, 255)]).unwrap();
        for style in SCENE_BUILDERS {
            for (width, height) in [(usize::MAX, usize::MAX), (1 << 33, 3), (1, usize::MAX)] {
                assert!(
                    build_scene(style, width, height, 2, &palette, 0.3).is_some(),
                    "{style}"
                );
            }
        }
    }
}
