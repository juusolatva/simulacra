//! The portable fixtures: committed scenes with the SHA-256 of their render.
//!
//! Each `fixtures/<style>.json` holds a scene at 48 x 36, seed 3, phase 0.37
//! and the SHA-256 of the packed RGB a `portable` build renders from it. The
//! committed scenes were written before the Python cutover by the Python
//! scene builders (the Rust builder for `attractor`), whose reference is in
//! git history. These tests load each scene through serde and render it; with
//! `portable` the hash must match on every platform, and with the platform
//! libm the fixtures only have to load and render a full frame.
//!
//! A deliberate change to a shader or scene builder updates them with the
//! ignored `bless_fixtures` test, run on its own so that no other test reads
//! the fixtures while it writes them, and then the normal tests:
//!
//! ```text
//! cargo test --manifest-path core/Cargo.toml -p simulacra_engine --features json,portable \
//!     --test fixtures -- --ignored --exact bless_fixtures
//! cargo test --manifest-path core/Cargo.toml -p simulacra_engine --features json,portable --test fixtures
//! ```
//!
//! It rewrites only a fixture that is missing, does not parse, holds a scene
//! that no longer deserializes or validates, or whose Rust-built scene renders
//! another hash; other files stay byte for byte as they are. A rewritten file
//! lists its keys alphabetically and formats numbers as `serde_json` does
//! (`1e-5` rather than Python's `1e-05`), so the first bless of a
//! Python-written fixture changes every line of that file; compare the
//! `sha256` and the scene values rather than the line diff.
#![cfg(feature = "json")]

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};
use simulacra_engine::{
    PALETTES, PORTABLE, Palette, SCENE_BUILDERS, STYLES, build_scene, build_scene_json, render,
    scene_from_json, seed32,
};

#[derive(Deserialize)]
struct Fixture {
    style: String,
    width: usize,
    height: usize,
    seed: i64,
    phase: f64,
    core_seed: u32,
    stops: Vec<(i64, i64, i64)>,
    sha256: String,
    scene: Box<serde_json::value::RawValue>,
}

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn load(style: &str) -> Fixture {
    let path = fixture_dir().join(format!("{style}.json"));
    let text = fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

#[test]
fn every_style_has_exactly_one_fixture() {
    let mut files: Vec<String> = fs::read_dir(fixture_dir())
        .expect("fixtures directory")
        .filter_map(|entry| {
            let name = entry.ok()?.file_name().into_string().ok()?;
            name.strip_suffix(".json").map(str::to_owned)
        })
        .collect();
    files.sort_unstable();
    let mut styles: Vec<&str> = STYLES.to_vec();
    styles.sort_unstable();
    assert_eq!(
        files, styles,
        "run the bless_fixtures test (see the module docs)"
    );
}

#[test]
fn fixtures_render_their_recorded_hash() {
    let mut mismatched = Vec::new();
    for style in STYLES {
        let fixture = load(style);
        assert_eq!(fixture.style, style);
        let scene = scene_from_json(style, fixture.scene.get())
            .unwrap_or_else(|err| panic!("{style}: {err}"));
        let palette = Palette::new(fixture.stops).expect("palette");
        // The checked render: every committed scene must pass validation.
        let rgb = render(
            scene.as_ref(),
            fixture.width,
            fixture.height,
            fixture.core_seed,
            &palette,
        )
        .unwrap_or_else(|err| panic!("{style}: {err}"));
        assert_eq!(rgb.len(), fixture.width * fixture.height * 3, "{style}");
        let digest = hex(&Sha256::digest(&rgb));
        if digest != fixture.sha256 {
            mismatched.push(style);
        }
    }
    if cfg!(feature = "portable") {
        assert!(
            mismatched.is_empty(),
            "portable renders differ from the fixtures for {mismatched:?}; if the change is \
             intended, run the bless_fixtures test (see the module docs)"
        );
    }
}

/// Scenes built by the Rust ports of the Python builders must render the
/// fixture's hash too: the same scene, so the same image, without Python.
#[test]
fn ported_scene_builders_render_the_fixture_hash() {
    for style in SCENE_BUILDERS {
        let fixture = load(style);
        assert_eq!(seed32(fixture.seed), fixture.core_seed);
        let palette = Palette::new(fixture.stops).expect("palette");
        let scene = build_scene(
            style,
            fixture.width,
            fixture.height,
            fixture.seed,
            &palette,
            fixture.phase,
        )
        .expect("ported builder");
        let rgb = scene.render(fixture.width, fixture.height, fixture.core_seed, &palette);
        let digest = hex(&Sha256::digest(&rgb));
        if cfg!(feature = "portable") {
            assert_eq!(
                digest, fixture.sha256,
                "{style}: the Rust-built scene renders differently"
            );
        }
    }
}

#[cfg(feature = "portable")]
/// A fresh copy of the committed fixtures in its own directory, for tests
/// that bless without touching the real ones.
fn fixture_copy(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("fixtures")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    for entry in fs::read_dir(fixture_dir()).unwrap() {
        let path = entry.unwrap().path();
        fs::copy(&path, dir.join(path.file_name().unwrap())).unwrap();
    }
    dir
}

/// Equal JSON values, floats compared bit for bit.
fn same_json(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    use serde_json::Value;
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => match (x.as_f64(), y.as_f64()) {
            _ if x.is_f64() || y.is_f64() => {
                x.is_f64() == y.is_f64()
                    && x.as_f64().map(f64::to_bits) == y.as_f64().map(f64::to_bits)
            }
            _ => x == y,
        },
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same_json(p, q))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, v)| y.get(k).is_some_and(|w| same_json(v, w)))
        }
        _ => a == b,
    }
}

#[test]
fn fixture_writer_round_trips_every_number() {
    for style in STYLES {
        let text = fs::read_to_string(fixture_dir().join(format!("{style}.json"))).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        let again: serde_json::Value = serde_json::from_str(&write_fixture_json(&value)).unwrap();
        assert!(
            same_json(&value, &again),
            "{style} changes when written back"
        );
    }
}

#[test]
fn fixture_writer_keeps_the_layout() {
    let value = serde_json::json!({"a": [1, 2.5], "b": {"c": [[1, 2], [3, 4]]}});
    assert_eq!(
        write_fixture_json(&value),
        "{\n \"a\": [1, 2.5],\n \"b\": {\n  \"c\": [\n   [1, 2],\n   [3, 4]\n  ]\n }\n}\n"
    );
}

#[cfg(feature = "portable")]
fn read_all(dir: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    fs::read_dir(dir)
        .unwrap()
        .map(|e| {
            let path = e.unwrap().path();
            (
                path.file_name().unwrap().to_string_lossy().into_owned(),
                fs::read(&path).unwrap(),
            )
        })
        .collect()
}

#[cfg(feature = "portable")]
#[test]
fn blessing_leaves_matching_fixtures_untouched() {
    let copy = fixture_copy("untouched");
    assert_eq!(bless_into(&copy), Vec::<String>::new());
    assert_eq!(read_all(&copy), read_all(&fixture_dir()));
}

#[cfg(feature = "portable")]
/// The recorded hash of `style` in `dir`, and whether its stored scene renders it.
fn renders_its_hash(dir: &Path, style: &str) -> (String, bool) {
    let text = fs::read_to_string(dir.join(format!("{style}.json"))).unwrap();
    let fixture: Fixture = serde_json::from_str(&text).unwrap();
    let scene = scene_from_json(style, fixture.scene.get()).unwrap();
    let palette = Palette::new(fixture.stops).unwrap();
    let rgb = render(
        scene.as_ref(),
        fixture.width,
        fixture.height,
        fixture.core_seed,
        &palette,
    )
    .unwrap();
    let ok = hex(&Sha256::digest(&rgb)) == fixture.sha256;
    (fixture.sha256, ok)
}

#[cfg(feature = "portable")]
#[test]
fn blessing_rewrites_a_changed_or_missing_fixture() {
    let copy = fixture_copy("changed");
    let waves = copy.join("waves.json");
    let text = fs::read_to_string(&waves).unwrap();
    let (original, _) = renders_its_hash(&fixture_dir(), "waves");
    fs::write(&waves, text.replace(&original, &"0".repeat(64))).unwrap();
    fs::remove_file(copy.join("blobs.json")).unwrap();
    assert_eq!(bless_into(&copy), ["blobs", "waves"]);
    for style in ["blobs", "waves"] {
        let (hash, ok) = renders_its_hash(&copy, style);
        assert!(ok, "{style} does not render its new hash");
        assert_eq!(hash, renders_its_hash(&fixture_dir(), style).0, "{style}");
    }
    let (before, after) = (read_all(&fixture_dir()), read_all(&copy));
    for (name, bytes) in &before {
        if name != "blobs.json" && name != "waves.json" {
            assert_eq!(after.get(name), Some(bytes), "{name} changed");
        }
    }
}

#[cfg(feature = "portable")]
#[test]
fn blessing_repairs_a_scene_that_no_longer_loads() {
    let copy = fixture_copy("schema");
    let waves = copy.join("waves.json");
    let text = fs::read_to_string(&waves).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
    let scene = value["scene"].as_object_mut().unwrap();
    let key = scene.keys().next().unwrap().clone();
    let field = scene.remove(&key).unwrap();
    scene.insert(format!("{key}_old"), field);
    fs::write(&waves, write_fixture_json(&value)).unwrap();
    assert_eq!(bless_into(&copy), ["waves"]);
    let (hash, ok) = renders_its_hash(&copy, "waves");
    assert!(ok);
    assert_eq!(hash, renders_its_hash(&fixture_dir(), "waves").0);
    let (before, after) = (read_all(&fixture_dir()), read_all(&copy));
    for (name, bytes) in &before {
        if name != "waves.json" {
            assert_eq!(after.get(name), Some(bytes), "{name} changed");
        }
    }
}

// Blessing.

/// The fixtures' render settings.
const WIDTH: usize = 48;
const HEIGHT: usize = 36;
const SEED: i64 = 3;
const PHASE: f64 = 0.37;

/// One level per line with a one-space indent per level, a list of plain
/// scalars on one line, and a final newline: the line layout of the
/// Python-era `make_fixtures.py`, but with keys in the map's (alphabetical)
/// order and numbers as `serde_json` writes them.
fn write_fixture_json(value: &serde_json::Value) -> String {
    fn dump(value: &serde_json::Value, indent: usize, out: &mut String) {
        use serde_json::Value;
        let pad = " ".repeat(indent);
        match value {
            Value::Object(map) => {
                out.push_str("{\n");
                for (i, (key, item)) in map.iter().enumerate() {
                    if i > 0 {
                        out.push_str(",\n");
                    }
                    let _ = write!(out, "{pad} {}: ", serde_json::to_string(key).unwrap());
                    dump(item, indent + 1, out);
                }
                let _ = write!(out, "\n{pad}}}");
            }
            Value::Array(items) if items.iter().all(|v| !v.is_object() && !v.is_array()) => {
                let parts: Vec<String> = items.iter().map(|v| v.to_string()).collect();
                let _ = write!(out, "[{}]", parts.join(", "));
            }
            Value::Array(items) => {
                out.push_str("[\n");
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push_str(",\n");
                    }
                    let _ = write!(out, "{pad} ");
                    dump(item, indent + 1, out);
                }
                let _ = write!(out, "\n{pad}]");
            }
            scalar => out.push_str(&scalar.to_string()),
        }
    }
    let mut out = String::new();
    dump(value, 0, &mut out);
    out.push('\n');
    out
}

/// The SHA-256 of `scene` rendered through the checked `render`, if it loads
/// and validates.
fn stored_render(fixture: &Fixture) -> Option<String> {
    let scene = scene_from_json(&fixture.style, fixture.scene.get()).ok()?;
    let palette = Palette::new(fixture.stops.clone()).ok()?;
    let rgb = render(
        scene.as_ref(),
        fixture.width,
        fixture.height,
        fixture.core_seed,
        &palette,
    )
    .ok()?;
    Some(hex(&Sha256::digest(&rgb)))
}

/// Whether the fixture text needs rewriting: see the module docs.
fn needs_bless(text: Option<&str>, style: &str, rust_hash: &str) -> bool {
    let Some(fixture) = text.and_then(|t| serde_json::from_str::<Fixture>(t).ok()) else {
        return true;
    };
    fixture.style != style || stored_render(&fixture).is_none() || fixture.sha256 != rust_hash
}

/// Blesses the fixtures in `dir` from the Rust scene builders; returns the
/// styles it rewrote, sorted. Also deletes fixtures of styles that no longer
/// exist.
fn bless_into(dir: &Path) -> Vec<String> {
    if !PORTABLE {
        panic!("blessing needs --features portable");
    }
    let mut styles: Vec<&str> = STYLES.to_vec();
    styles.sort_unstable();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        if path.extension().is_some_and(|e| e == "json") && !styles.contains(&stem.as_str()) {
            fs::remove_file(&path).unwrap();
        }
    }
    let mut rewritten = Vec::new();
    for (index, style) in styles.iter().enumerate() {
        let (palette_name, stops) = PALETTES[index % PALETTES.len()];
        let palette = Palette::new(stops.to_vec()).unwrap();
        let built =
            build_scene(style, WIDTH, HEIGHT, SEED, &palette, PHASE).expect("ported builder");
        let core_seed = seed32(SEED);
        let rgb = render(built.as_ref(), WIDTH, HEIGHT, core_seed, &palette)
            .unwrap_or_else(|err| panic!("{style}: {err}"));
        let hash = hex(&Sha256::digest(&rgb));
        let path = dir.join(format!("{style}.json"));
        let text = fs::read_to_string(&path).ok();
        if !needs_bless(text.as_deref(), style, &hash) {
            continue;
        }
        let scene_json = build_scene_json(style, WIDTH, HEIGHT, SEED, &palette, PHASE)
            .unwrap()
            .expect("ported builder");
        let value = serde_json::json!({
            "style": style,
            "width": WIDTH,
            "height": HEIGHT,
            "seed": SEED,
            "core_seed": core_seed,
            "phase": PHASE,
            "palette": palette_name,
            "stops": stops.iter().map(|&(r, g, b)| [r, g, b]).collect::<Vec<_>>(),
            "sha256": hash,
            "scene": serde_json::from_str::<serde_json::Value>(&scene_json).unwrap(),
        });
        let temporary = dir.join(format!(".{style}.json.tmp"));
        fs::write(&temporary, write_fixture_json(&value)).unwrap();
        fs::rename(&temporary, &path).unwrap();
        rewritten.push((*style).to_owned());
    }
    rewritten
}

#[test]
#[ignore = "rewrites the fixtures; run alone: cargo test --manifest-path core/Cargo.toml -p simulacra_engine --features json,portable --test fixtures -- --ignored --exact bless_fixtures"]
fn bless_fixtures() {
    let rewritten = bless_into(&fixture_dir());
    println!("blessed {} fixture(s): {rewritten:?}", rewritten.len());
}
