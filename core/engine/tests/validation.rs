//! Scene validation against malformed scenes, built by editing the portable
//! fixtures: every number a style reads is range-checked, and every structural
//! rule and work limit rejects the scene before rendering starts.
#![cfg(feature = "json")]

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use serde_json::{Value, json};
use simulacra_engine::{
    Palette, RenderError, STYLES, SceneError, render, scene_from_json, validate_scene,
};

struct Fixture {
    style: &'static str,
    width: usize,
    height: usize,
    palette: Palette,
    scene: Value,
}

fn load(style: &'static str) -> Fixture {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/{style}.json"));
    let text = fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
    let mut data: Value = serde_json::from_str(&text).expect("fixture JSON");
    let stops: Vec<(i64, i64, i64)> =
        serde_json::from_value(data["stops"].take()).expect("fixture stops");
    let size = |key: &str| usize::try_from(data[key].as_u64().expect("size")).expect("size");
    Fixture {
        style,
        width: size("width"),
        height: size("height"),
        palette: Palette::new(stops).expect("palette"),
        scene: data["scene"].take(),
    }
}

impl Fixture {
    /// Validates `scene`, which must still load as this style's scene.
    fn validate(&self, scene: &Value) -> Result<(), SceneError> {
        let loaded = scene_from_json(self.style, &scene.to_string())
            .unwrap_or_else(|err| panic!("{}: {err}", self.style));
        validate_scene(loaded.as_ref(), self.width, self.height, &self.palette)
    }

    /// Validates the fixture's scene after `edit`.
    fn validate_edited(&self, edit: impl FnOnce(&mut Value)) -> Result<(), SceneError> {
        let mut scene = self.scene.clone();
        edit(&mut scene);
        self.validate(&scene)
    }
}

/// JSON pointers to the first number at each distinct path, with list
/// indices collapsed (`/stars/0/1` stands for every star's second field).
fn first_numbers(
    value: &Value,
    pointer: &str,
    pattern: &str,
    out: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    match value {
        Value::Number(_) => {
            if seen.insert(pattern.to_owned()) {
                out.push(pointer.to_owned());
            }
        }
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                first_numbers(
                    item,
                    &format!("{pointer}/{i}"),
                    &format!("{pattern}/[]"),
                    out,
                    seen,
                );
            }
        }
        Value::Object(fields) => {
            for (key, item) in fields {
                first_numbers(
                    item,
                    &format!("{pointer}/{key}"),
                    &format!("{pattern}/{key}"),
                    out,
                    seen,
                );
            }
        }
        _ => {}
    }
}

/// Styles whose `phase` drives looping noise, which reduces it modulo 1: any
/// finite phase is valid there (Python passes the caller's phase through).
const LOOP_PHASE_STYLES: [&str; 9] = [
    "blobs",
    "bloom",
    "contour",
    "crystal",
    "drift",
    "horizon",
    "nebula",
    "pinwheel",
    "supernova",
];

/// Every float a scene holds must be range-checked: setting any number to
/// 1e300 either fails to load (an integer field) or fails validation with
/// `SceneError::Number`, except a looping-noise phase, which stays valid. This
/// catches a field missing from a style's `check_numbers!`.
#[test]
fn every_scene_number_is_range_checked() {
    for style in STYLES {
        let fixture = load(style);
        fixture
            .validate(&fixture.scene)
            .unwrap_or_else(|err| panic!("{style}: {err}"));
        let mut pointers = Vec::new();
        first_numbers(&fixture.scene, "", "", &mut pointers, &mut HashSet::new());
        let mut floats = 0;
        for pointer in pointers {
            let mut scene = fixture.scene.clone();
            *scene.pointer_mut(&pointer).expect("pointer") = json!(1e300);
            let Ok(loaded) = scene_from_json(style, &scene.to_string()) else {
                continue; // an integer field: 1e300 does not load
            };
            floats += 1;
            let result = validate_scene(
                loaded.as_ref(),
                fixture.width,
                fixture.height,
                &fixture.palette,
            );
            if pointer == "/phase" && LOOP_PHASE_STYLES.contains(&style) {
                assert_eq!(result, Ok(()), "{style}{pointer}");
            } else {
                assert!(
                    matches!(result, Err(SceneError::Number { value, .. }) if value == 1e300),
                    "{style}{pointer}: {result:?}"
                );
            }
        }
        assert!(floats > 0, "{style}: no float fields found");
    }
}

fn assert_index(result: Result<(), SceneError>, field: &str) {
    assert!(
        matches!(result, Err(SceneError::Index { field: f, .. }) if f == field),
        "{result:?}"
    );
}

fn assert_range(result: Result<(), SceneError>, field: &str) {
    assert!(
        matches!(result, Err(SceneError::Range { field: f, .. }) if f == field),
        "{result:?}"
    );
}

fn assert_too_much_work(result: Result<(), SceneError>) {
    assert!(matches!(result, Err(SceneError::Work { .. })), "{result:?}");
}

#[test]
fn indices_must_refer_to_existing_entries() {
    let fixture = load("constellations");
    let stars = fixture.scene["stars"].as_array().expect("stars").len();
    assert_index(
        fixture.validate_edited(|s| s["pairs"][0][0] = json!(stars)),
        "pairs",
    );
    assert_index(
        fixture.validate_edited(|s| s["pairs"][0][1] = json!(stars)),
        "pairs",
    );
    assert!(
        fixture
            .validate_edited(|s| s["pairs"][0][1] = json!(stars - 1))
            .is_ok()
    );

    let fixture = load("orbits");
    let stops = fixture.palette.stop_count();
    assert_index(
        fixture.validate_edited(|s| s["rings"][0][1] = json!(stops)),
        "rings (palette stop)",
    );
    assert!(
        fixture
            .validate_edited(|s| s["rings"][0][1] = json!(stops - 1))
            .is_ok()
    );
}

#[test]
fn grids_and_surfaces_must_match_their_sizes() {
    let fixture = load("mosaic");
    assert_range(
        fixture.validate_edited(|s| s["cols"] = json!(0)),
        "cols and rows",
    );
    assert_range(
        fixture.validate_edited(|s| s["rows"] = json!(0)),
        "cols and rows",
    );
    assert_range(
        fixture.validate_edited(|s| {
            s["cols"] = json!(usize::MAX);
            s["rows"] = json!(2);
        }),
        "cols and rows",
    );
    let result = fixture.validate_edited(|s| {
        s["values"].as_array_mut().expect("values").pop();
    });
    assert!(
        matches!(
            result,
            Err(SceneError::Length {
                field: "values",
                ..
            })
        ),
        "{result:?}"
    );

    let fixture = load("woodblock");
    let result = fixture.validate_edited(|s| {
        s["layers"][0]["surface"]
            .as_array_mut()
            .expect("surface")
            .pop();
    });
    assert_eq!(
        result,
        Err(SceneError::Length {
            field: "layers (surface, one height per column)",
            expected: fixture.width,
            found: fixture.width - 1,
        })
    );
}

#[test]
fn polygons_need_points_and_counts_need_a_range() {
    let fixture = load("geo");
    assert_range(
        fixture.validate_edited(|s| s["polygons"][0] = json!([])),
        "polygons",
    );
    let fixture = load("shard");
    assert_range(
        fixture.validate_edited(|s| s["shards"][0][1] = json!([])),
        "shards",
    );
    let fixture = load("prism");
    let gem = fixture.scene["gems"]
        .as_array()
        .expect("gems")
        .iter()
        .position(|gem| !gem[4].as_array().expect("layers").is_empty())
        .expect("a gem with layers");
    assert_range(
        fixture.validate_edited(|s| s["gems"][gem][4][0][0] = json!([])),
        "gems",
    );

    let fixture = load("cosmos");
    assert_range(
        fixture.validate_edited(|s| s["centers"][0][3] = json!(0)),
        "centers",
    );
    assert!(
        fixture
            .validate_edited(|s| s["centers"][0][3] = json!(1))
            .is_ok()
    );

    let fixture = load("horizon");
    let layers = |n| {
        move |s: &mut Value| {
            let layer = s["layers"][0].clone();
            s["layers"] = Value::Array(vec![layer; n]);
        }
    };
    assert_range(fixture.validate_edited(layers(65)), "layers");
    assert!(fixture.validate_edited(layers(64)).is_ok());

    let fixture = load("lissajous");
    let result = fixture.validate_edited(|s| {
        let curve = s["curves"][0].clone();
        s["curves"] = Value::Array(vec![curve; 9]);
    });
    assert_range(result, "curves");
    assert!(
        fixture
            .validate_edited(|s| {
                let curve = s["curves"][0].clone();
                s["curves"] = Value::Array(vec![curve; 8]);
            })
            .is_ok()
    );
}

#[test]
fn scenes_that_would_stall_are_rejected() {
    let fixture = load("ink");
    assert_too_much_work(fixture.validate_edited(|s| s["strokes"][0][8] = json!(i64::MAX)));
    let fixture = load("comet");
    assert_too_much_work(fixture.validate_edited(|s| s["comets"][0][3] = json!(1e12)));
    assert_too_much_work(fixture.validate_edited(|s| s["comets"][0][4] = json!(1e9)));
    // A negative radius empties the stamps, but every step still runs.
    assert_too_much_work(fixture.validate_edited(|s| {
        s["comets"][0][3] = json!(1e12);
        s["comets"][0][4] = json!(-1.0);
    }));
    let fixture = load("circuit");
    assert_too_much_work(fixture.validate_edited(|s| {
        s["traces"] = Value::Array(vec![json!([[], []]); 200_000]);
    }));
    let fixture = load("attractor");
    assert_too_much_work(fixture.validate_edited(|s| s["points"] = json!(1_u64 << 40)));
    let fixture = load("mandelbrot");
    assert_too_much_work(fixture.validate_edited(|s| s["iterations"] = json!(u32::MAX)));
    let fixture = load("constellations");
    assert_too_much_work(fixture.validate_edited(|s| s["pairs"][0][2] = json!(1e12)));
    let fixture = load("lunar");
    assert_too_much_work(fixture.validate_edited(|s| {
        let crater = s["craters"][0].clone();
        s["craters"] = Value::Array(vec![crater; 20_000]);
    }));
    // A long list multiplies the per-pixel work.
    let fixture = load("cosmic");
    assert_too_much_work(fixture.validate_edited(|s| {
        let star = s["stars"][0].clone();
        s["stars"] = Value::Array(vec![star; 200_000]);
    }));
}

/// A valid scene can still lead a shader to huge derived coordinates:
/// blackhole's lensing multiplies sky coordinates by `1 - 1.6 / r**2`. The
/// noise must not overflow (a panic in overflow-checked builds). Found by
/// Codex on PR #37.
#[test]
fn huge_derived_coordinates_render() {
    let fixture = load("blackhole");
    let mut scene = fixture.scene.clone();
    scene["center"] = json!([1_000_000.0, 0.0]);
    scene["radius"] = json!(1e12);
    scene["roll"] = json!([1.0, 0.0]);
    scene["tilt"] = json!(0.5);
    scene["stars"] = json!([]);
    let loaded = scene_from_json("blackhole", &scene.to_string()).expect("loads");
    let rgb = render(loaded.as_ref(), 1, 1, 3, &fixture.palette).expect("valid scene");
    assert_eq!(rgb.len(), 3);
}

/// `render` reports an invalid scene as an error instead of panicking.
#[test]
fn render_returns_the_scene_error() {
    let fixture = load("constellations");
    let mut scene = fixture.scene.clone();
    scene["pairs"][0][0] = json!(10_000);
    let loaded = scene_from_json("constellations", &scene.to_string()).expect("loads");
    let result = render(
        loaded.as_ref(),
        fixture.width,
        fixture.height,
        3,
        &fixture.palette,
    );
    let Err(RenderError::Scene(err)) = result else {
        panic!(
            "expected a scene error, got {:?}",
            result.map(|rgb| rgb.len())
        );
    };
    assert!(
        err.to_string().contains("pairs refers to entry 10000"),
        "{err}"
    );
}
