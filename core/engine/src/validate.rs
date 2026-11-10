//! Scene validation: what the engine checks before it renders a scene.
//!
//! The shaders trust their scene. A scene read from a file or passed through
//! the Python adapter could otherwise index past the end of a list, describe
//! a grid that does not match its size or ask for an effectively endless loop.
//! [`validate_scene`] (called by [`render`](crate::render)) rejects such scenes:
//!
//! - every float is finite and at most [`MAX_MAGNITUDE`] in size, so no
//!   float-to-integer conversion in a shader saturates and then overflows
//!   (a looping-noise `phase` only has to be finite: the noise reduces it
//!   modulo 1, and Python passes the caller's phase through unchanged);
//! - indices, lengths and counts fit the canvas, the palette and each other
//!   (each style's [`Style::validate`] lists its rules);
//! - the work a scene asks for, estimated by its [`Style::validate`] in inner
//!   loop iterations, stays within [`work_limit`] for the canvas, so a scene
//!   cannot stall rendering or allocate buffers far larger than the frame.
//!
//! Scenes from the Python builders stay far below the limit at every size;
//! `test_native_scenes_stay_well_within_the_work_limit` in `test_simulacra.py`
//! holds them to an eighth of it.

use std::fmt;

use crate::palette::Palette;
use crate::styles::Style;

/// The largest magnitude a scene number may have: 2**40, 256 times the
/// largest side a PNG can store, so canvas coordinates always fit and the
/// shaders' integer conversions cannot overflow.
pub const MAX_MAGNITUDE: f64 = 1_099_511_627_776.0;

/// Work allowed per pixel. Work is counted in inner loop iterations of the
/// parallel per-pixel shaders, and sequential steps are weighted
/// ([`SEQUENTIAL_WEIGHT`]) so that units take roughly the same time in every
/// style. The heaviest built-in scenes need about 760 per pixel (`lunar`) and
/// 1370 (`ink` at 3840 x 2160), so the limit is about 12 times the most a
/// built-in scene asks for.
pub const WORK_PER_PIXEL: f64 = 16_384.0;

/// Work allowed on top of the per-pixel share (2**27), for scenes whose work
/// does not shrink with the canvas, such as a `comet` crossing a 4000 x 1
/// strip or fixed lists of stars on a tiny canvas.
pub const WORK_BASE: f64 = 134_217_728.0;

/// Weight of one sequential step (a brush stamp pixel, a point along a line,
/// a pixel of a segment's box). The brush styles stamp on one thread in
/// Python's order, while shading runs on every core; measured on 12 threads,
/// a stamp took about as long as 8 to 40 shading iterations.
pub const SEQUENTIAL_WEIGHT: f64 = 8.0;

/// The most work [`validate_scene`] accepts for a canvas:
/// `WORK_BASE + WORK_PER_PIXEL * width * height`.
#[must_use]
pub fn work_limit(width: usize, height: usize) -> f64 {
    WORK_BASE + WORK_PER_PIXEL * width as f64 * height as f64
}

/// Why a scene was rejected.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SceneError {
    /// A number is NaN, infinite or larger in magnitude than [`MAX_MAGNITUDE`].
    Number { field: &'static str, value: f64 },
    /// An index refers past the end of the list (or palette) it indexes.
    Index {
        field: &'static str,
        index: usize,
        len: usize,
    },
    /// A list does not have the length the canvas or the scene requires.
    Length {
        field: &'static str,
        expected: usize,
        found: usize,
    },
    /// A value is outside the range the style supports.
    Range {
        field: &'static str,
        rule: &'static str,
    },
    /// Rendering would take more than [`work_limit`] inner loop iterations.
    Work { work: f64, limit: f64 },
}

impl fmt::Display for SceneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Number { field, value } => write!(
                f,
                "invalid scene: {field} holds {value:e}, but scene numbers must be finite and at most 2**40 in size"
            ),
            Self::Index { field, index, len } => write!(
                f,
                "invalid scene: {field} refers to entry {index}, but there are only {len}"
            ),
            Self::Length {
                field,
                expected,
                found,
            } => write!(
                f,
                "invalid scene: {field} has {found} entries, but {expected} are needed"
            ),
            Self::Range { field, rule } => write!(f, "invalid scene: {field} {rule}"),
            Self::Work { work, limit } => write!(
                f,
                "invalid scene: rendering it would take about {work:.3e} steps, more than the {limit:.3e} allowed for this canvas"
            ),
        }
    }
}

impl std::error::Error for SceneError {}

/// What a scene is checked against.
#[derive(Debug, Clone, Copy)]
pub struct Canvas {
    pub width: usize,
    pub height: usize,
    /// Number of palette stops.
    pub stops: usize,
}

impl Canvas {
    #[must_use]
    pub fn pixels(&self) -> f64 {
        self.width as f64 * self.height as f64
    }

    /// Work for shading every pixel once plus `per_pixel` loop iterations.
    #[must_use]
    pub fn per_pixel(&self, per_pixel: f64) -> f64 {
        self.pixels() * (1.0 + per_pixel)
    }
}

/// Checks `scene` against the canvas and palette; see the module docs. The
/// canvas itself (positive sides, addressable size) is checked by
/// [`render`](crate::render).
///
/// # Errors
///
/// [`SceneError`] describing the first problem found.
pub fn validate_scene(
    scene: &dyn Style,
    width: usize,
    height: usize,
    palette: &Palette,
) -> Result<(), SceneError> {
    let canvas = Canvas {
        width,
        height,
        stops: palette.stop_count(),
    };
    let work = scene.validate(&canvas)?;
    let limit = work_limit(width, height);
    // Written so that a NaN estimate is rejected too.
    if work.is_nan() || work > limit {
        return Err(SceneError::Work { work, limit });
    }
    Ok(())
}

/// Estimated work of `scene` on the canvas and the limit it must stay within,
/// after the structural checks; for tests that keep built-in scenes well below
/// the limit.
///
/// # Errors
///
/// [`SceneError`] if a structural check fails.
pub fn scene_work(
    scene: &dyn Style,
    width: usize,
    height: usize,
    palette: &Palette,
) -> Result<(f64, f64), SceneError> {
    let canvas = Canvas {
        width,
        height,
        stops: palette.stop_count(),
    };
    Ok((scene.validate(&canvas)?, work_limit(width, height)))
}

/// Numbers a scene holds, checked to be finite and at most [`MAX_MAGNITUDE`].
/// Integers need no check here; styles bound them where it matters.
pub(crate) trait Numbers {
    fn check(&self, field: &'static str) -> Result<(), SceneError>;
}

impl Numbers for f64 {
    fn check(&self, field: &'static str) -> Result<(), SceneError> {
        if self.is_nan() || self.abs() > MAX_MAGNITUDE {
            return Err(SceneError::Number {
                field,
                value: *self,
            });
        }
        Ok(())
    }
}

macro_rules! no_check {
    ($($t:ty),*) => {
        $(impl Numbers for $t {
            fn check(&self, _field: &'static str) -> Result<(), SceneError> {
                Ok(())
            }
        })*
    };
}

no_check!(i64, u32, usize, bool);

impl<T: Numbers> Numbers for Vec<T> {
    fn check(&self, field: &'static str) -> Result<(), SceneError> {
        self.iter().try_for_each(|item| item.check(field))
    }
}

macro_rules! tuple_numbers {
    ($($name:ident),+) => {
        impl<$($name: Numbers),+> Numbers for ($($name,)+) {
            #[allow(non_snake_case, reason = "the type parameters name the fields")]
            fn check(&self, field: &'static str) -> Result<(), SceneError> {
                let ($($name,)+) = self;
                $($name.check(field)?;)+
                Ok(())
            }
        }
    };
}

tuple_numbers!(A, B);
tuple_numbers!(A, B, C);
tuple_numbers!(A, B, C, D);
tuple_numbers!(A, B, C, D, E);
tuple_numbers!(A, B, C, D, E, F);
tuple_numbers!(A, B, C, D, E, F, G);
tuple_numbers!(A, B, C, D, E, F, G, H);
tuple_numbers!(A, B, C, D, E, F, G, H, I);
tuple_numbers!(A, B, C, D, E, F, G, H, I, J, K, L);

/// Checks the numbers of the named fields of a scene struct, using each
/// field's name in the error.
macro_rules! check_numbers {
    ($scene:expr, $($field:ident),+ $(,)?) => {
        $($crate::validate::Numbers::check(&$scene.$field, stringify!($field))?;)+
    };
}

pub(crate) use check_numbers;

/// Checks a looping-noise phase (`fbm_loop`'s `phase`), which the noise
/// reduces modulo 1: any finite value works, so `phase=1e300` stays valid.
pub(crate) fn check_loop_phase(phase: f64) -> Result<(), SceneError> {
    if phase.is_finite() {
        Ok(())
    } else {
        Err(SceneError::Number {
            field: "phase",
            value: phase,
        })
    }
}

/// Fails unless `index < len`.
pub(crate) fn check_index(field: &'static str, index: usize, len: usize) -> Result<(), SceneError> {
    if index < len {
        Ok(())
    } else {
        Err(SceneError::Index { field, index, len })
    }
}

/// Fails unless every polygon has at least one point (the point-in-polygon
/// test reads the last point).
pub(crate) fn check_polygons<'a>(
    field: &'static str,
    polygons: impl IntoIterator<Item = &'a Vec<crate::geometry::Point>>,
) -> Result<(), SceneError> {
    for polygon in polygons {
        if polygon.is_empty() {
            return Err(SceneError::Range {
                field,
                rule: "contains a polygon without points",
            });
        }
    }
    Ok(())
}

/// Pixels the bounding box from `lo` to `hi`, grown by `reach` and clipped to
/// `size`, covers along one axis: an upper bound on a loop over it.
pub(crate) fn clipped_span(lo: f64, hi: f64, reach: f64, size: usize) -> f64 {
    let span = (hi - lo).abs() + 2.0 * reach.max(0.0) + 3.0;
    span.min(size as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_must_be_finite_and_moderate() {
        assert!(1.0_f64.check("x").is_ok());
        assert!(MAX_MAGNITUDE.check("x").is_ok());
        assert!((-MAX_MAGNITUDE).check("x").is_ok());
        for bad in [f64::NAN, f64::INFINITY, -f64::INFINITY, MAX_MAGNITUDE * 2.0] {
            assert!(
                matches!(bad.check("x"), Err(SceneError::Number { field: "x", .. })),
                "{bad}"
            );
        }
        let nested = vec![(1.0, vec![(2.0, 3_usize)]), (4.0, vec![(f64::NAN, 5)])];
        assert!(nested.check("nested").is_err());
    }

    #[test]
    fn loop_phases_only_need_to_be_finite() {
        assert!(check_loop_phase(1e300).is_ok());
        assert!(check_loop_phase(-1e300).is_ok());
        assert!(check_loop_phase(f64::NAN).is_err());
        assert!(check_loop_phase(f64::INFINITY).is_err());
    }

    #[test]
    fn indices_and_polygons() {
        assert!(check_index("pairs", 2, 3).is_ok());
        assert_eq!(
            check_index("pairs", 3, 3),
            Err(SceneError::Index {
                field: "pairs",
                index: 3,
                len: 3
            })
        );
        let polygons = [vec![(0.0, 0.0)], vec![]];
        assert!(check_polygons("polygons", &polygons[..1]).is_ok());
        assert!(check_polygons("polygons", &polygons).is_err());
    }

    #[test]
    fn errors_read_well() {
        let err = SceneError::Index {
            field: "pairs",
            index: 7,
            len: 5,
        };
        assert_eq!(
            err.to_string(),
            "invalid scene: pairs refers to entry 7, but there are only 5"
        );
        let err = SceneError::Work {
            work: 1e15,
            limit: 2e9,
        };
        assert!(err.to_string().contains("1.000e15"), "{err}");
    }
}
