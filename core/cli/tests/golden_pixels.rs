//! Golden pixel hashes: the SHA-256 of every decoded frame of every golden
//! workload, kept in `golden/PIXELS` apart from the file hashes in
//! `golden/SHA256SUMS`, so that a change to the encoders' bytes (a `flate2`
//! update, say) can be told apart from a change to the images.
//!
//! Render the workloads first and keep them:
//!
//! ```text
//! sh core/cli/tests/golden.sh check BINARY --keep DIR
//! SIMULACRA_GOLDEN_OUT=DIR cargo test --release --manifest-path core/Cargo.toml \
//!     -p simulacra_cli --features portable --test golden_pixels -- --ignored
//! ```
//!
//! With `SIMULACRA_BLESS=1` the test rewrites `PIXELS` instead of checking it.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

/// What differs between the names in a manifest and the names found:
/// `no hash for` a found name the manifest lacks, `hash without a file` for
/// a manifest name not found, and `duplicate` for a name listed twice.
fn compare_inventories(manifest: &[&str], found: &[&str]) -> Vec<String> {
    let mut problems = Vec::new();
    for (list, what) in [(manifest, "in PIXELS"), (found, "found")] {
        let mut seen = BTreeSet::new();
        for name in list {
            if !seen.insert(name) {
                problems.push(format!("duplicate {what}: {name}"));
            }
        }
    }
    let manifest: BTreeSet<_> = manifest.iter().collect();
    let found: BTreeSet<_> = found.iter().collect();
    problems.extend(
        found
            .difference(&manifest)
            .map(|n| format!("no hash for {n}")),
    );
    problems.extend(
        manifest
            .difference(&found)
            .map(|n| format!("hash without a file: {n}")),
    );
    problems
}

/// What keeps a `--keep` directory from being blessed: the files found must
/// be exactly the workloads named in `SHA256SUMS` (whose text is `sums`).
fn bless_problems(files: &[&str], sums: &str) -> Vec<String> {
    let workloads: Vec<&str> = sums
        .lines()
        .filter_map(|line| line.trim_end_matches('\r').split_once("  "))
        .map(|(_, name)| name)
        .collect();
    compare_inventories(&workloads, files)
        .into_iter()
        .map(|problem| problem.replace("in PIXELS", "in SHA256SUMS"))
        .collect()
}

/// `<hex>  <name>#<frame>` for every frame of every file in `dir`, ordered
/// by file name and then frame.
fn frame_lines(dir: &Path) -> Vec<(String, String)> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("{}: {err}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .collect();
    files.sort();
    assert!(!files.is_empty(), "{} holds no outputs", dir.display());
    let mut lines = Vec::new();
    for file in files {
        let name = file.file_name().unwrap().to_str().unwrap().to_owned();
        let decoded = common::decode(&file).unwrap_or_else(|err| panic!("{err}"));
        for (i, frame) in decoded.frames.iter().enumerate() {
            lines.push((format!("{name}#{i}"), common::sha256_hex(frame)));
        }
    }
    lines
}

/// The `sha256` of a portable fixture, found by its JSON key.
fn fixture_hash(style: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../engine/tests/fixtures")
        .join(format!("{style}.json"));
    let text = fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
    let start = text.find("\"sha256\": \"").expect("a sha256 field") + 11;
    text[start..start + 64].to_owned()
}

/// The fixture workloads render the portable fixtures' settings, so their
/// frames must hash as the fixtures do.
fn fixture_mismatches(lines: &[(String, String)]) -> Vec<String> {
    let mut mismatches = Vec::new();
    for (name, hash) in lines {
        if let Some(style) = name
            .strip_prefix("fixture-")
            .and_then(|n| n.strip_suffix(".png#0"))
        {
            let fixture = fixture_hash(style);
            if *hash != fixture {
                mismatches.push(format!("{name}: fixture {style} has {fixture}, got {hash}"));
            }
        }
    }
    mismatches
}

#[test]
#[ignore = "run on a golden.sh --keep directory: SIMULACRA_GOLDEN_OUT=DIR cargo test --manifest-path core/Cargo.toml -p simulacra_cli --features portable --test golden_pixels -- --ignored"]
fn every_frame_matches_its_pixel_hash() {
    let Some(dir) = std::env::var_os("SIMULACRA_GOLDEN_OUT") else {
        panic!("set SIMULACRA_GOLDEN_OUT to a golden.sh --keep directory");
    };
    let lines = frame_lines(Path::new(&dir));
    let pixels = golden_dir().join("PIXELS");
    let fixtures = fixture_mismatches(&lines);
    if std::env::var_os("SIMULACRA_BLESS").is_some_and(|v| v == "1") {
        // Bless only a complete, correct render: every workload's file, and
        // fixture frames that match the portable fixtures.
        let sums = fs::read_to_string(golden_dir().join("SHA256SUMS")).unwrap();
        let files: BTreeSet<&str> = lines
            .iter()
            .map(|(name, _)| name.rsplit_once('#').unwrap().0)
            .collect();
        let mut problems = bless_problems(&files.into_iter().collect::<Vec<_>>(), &sums);
        problems.extend(fixtures);
        assert!(
            problems.is_empty(),
            "not blessing:\n{}",
            problems.join("\n")
        );
        let text: String = lines.iter().map(|(n, h)| format!("{h}  {n}\n")).collect();
        fs::write(&pixels, text).unwrap();
        return;
    }
    let text =
        fs::read_to_string(&pixels).unwrap_or_else(|err| panic!("{}: {err}", pixels.display()));
    let expected: Vec<(&str, &str)> = text
        .lines()
        .map(|line| {
            let line = line.trim_end_matches('\r');
            let (hash, name) = line
                .split_once("  ")
                .unwrap_or_else(|| panic!("bad PIXELS line {line:?}"));
            (name, hash)
        })
        .collect();
    assert!(!expected.is_empty(), "{} is empty", pixels.display());
    let manifest: Vec<&str> = expected.iter().map(|(n, _)| *n).collect();
    let found: Vec<&str> = lines.iter().map(|(n, _)| n.as_str()).collect();
    let problems = compare_inventories(&manifest, &found);
    assert!(problems.is_empty(), "{}", problems.join("\n"));

    let want: BTreeMap<&str, &str> = expected.into_iter().collect();
    let mut mismatches: Vec<String> = lines
        .iter()
        .filter(|(name, hash)| want[name.as_str()] != hash)
        .map(|(name, hash)| format!("{name}: expected {}, got {hash}", want[name.as_str()]))
        .collect();
    mismatches.extend(fixtures);
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}

#[test]
fn blessing_requires_a_file_for_every_workload() {
    let sums = "aa  a.png\nbb  b.gif\r\n";
    assert_eq!(
        bless_problems(&["a.png", "b.gif"], sums),
        Vec::<String>::new()
    );
    assert_eq!(
        bless_problems(&["a.png", "c.png"], sums),
        ["no hash for c.png", "hash without a file: b.gif"]
    );
}

#[test]
fn inventory_mismatch_names_each_difference() {
    let problems = compare_inventories(&["a#0", "b#0"], &["b#0", "c#0", "c#0"]);
    assert_eq!(
        problems,
        [
            "duplicate found: c#0",
            "no hash for c#0",
            "hash without a file: a#0"
        ]
    );
}
