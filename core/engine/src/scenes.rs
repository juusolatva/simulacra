//! Scene builders ported from Python, so the engine can render a style on its
//! own. Each matches the Python `_<style>_scene()` draw for draw (through
//! [`PyRandom`](crate::PyRandom)); styles not ported yet return `None` and still
//! need their scene from Python (through the adapter or a scene file).

use crate::palette::Palette;
use crate::styles::{Style, fields};

/// Styles whose scene the engine can build, in registry order.
pub const SCENE_BUILDERS: [&str; 3] = ["blobs", "mandelbrot", "waves"];

/// Builds the scene for `style` as `simulacra.NATIVE_STYLES[style](width,
/// height, seed, stops, phase)` does, or `None` if its builder is still Python
/// only. `seed` is the full seed (the random generator sees all of it; the
/// shaders take [`seed32`](crate::seed32) of it). `palette` is passed because
/// some builders depend on the number of stops.
#[must_use]
pub fn build_scene(
    style: &str,
    width: usize,
    height: usize,
    seed: i64,
    palette: &Palette,
    phase: f64,
) -> Option<Box<dyn Style>> {
    let _ = palette; // no ported builder needs the stops yet
    Some(match style {
        "blobs" => Box::new(fields::Blobs::build(phase)),
        "mandelbrot" => Box::new(fields::Mandelbrot::build(width, height, phase)),
        "waves" => Box::new(fields::Waves::build(seed, phase)),
        _ => return None,
    })
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
}
