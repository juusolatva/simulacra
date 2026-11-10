//! The portable fixtures: committed scenes with the SHA-256 of their render.
//!
//! `make_fixtures.py` wrote each `fixtures/<style>.json` from the Python scene
//! builder and hashed the packed RGB rendered by a core built with the
//! `portable` feature. This test loads the scene through serde and renders it
//! without Python. With `portable` the hash must match on every platform; with
//! the platform libm it may differ, so the test then only checks that every
//! fixture loads and renders a full frame.
#![cfg(feature = "json")]

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};
use simulacra_engine::{
    Palette, SCENE_BUILDERS, STYLES, build_scene, render, scene_from_json, seed32,
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
    assert_eq!(files, styles, "rerun core/engine/tests/make_fixtures.py");
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
             intended, rerun core/engine/tests/make_fixtures.py with a portable core"
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
