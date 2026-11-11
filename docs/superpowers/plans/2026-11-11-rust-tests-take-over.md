# Rust Tests Take Over (Python cutover, PR 1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Committed golden hashes and Rust integration tests take over everything the Python suite checks that is to survive the cutover, while Python still passes on the same commit.

**Architecture:** A POSIX shell checker (`golden.sh`) renders a workload list with any `simulacra` binary and compares whole-file SHA-256s against `SHA256SUMS`. An ignored Rust test decodes the same outputs and checks each frame's RGB against `PIXELS`. Rust integration tests in `simulacra_cli` (CLI behavior, with test-only PNG/APNG/GIF decoders) and `simulacra_engine` (invariants through `build_scene` and the checked `render`) replace the Python tests the spec keeps. CI runs the golden check on every PR and on every release target.

**Tech Stack:** Rust (workspace in `core/`, `flate2`, `crc32fast`, `sha2` as a dev-dependency), POSIX sh plus coreutils, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-11-11-python-cutover-design.md` (section "PR 1: Rust tests take over"). This plan covers PR 1 only; PR 2 (remove Python) and PR 3 (math backends) get their own plans after PR 1 merges, since PR 3 depends on a measurement.

## Global Constraints

- Python stays in this PR. `python3 -m unittest -q`, `.venv/bin/python -m unittest -q` and `SIMULACRA_CLI=core/target/release/simulacra .venv/bin/python -m unittest -q -k StandaloneCli test_simulacra` must pass on the commit that blesses the manifests.
- Golden hashes hold only for `portable` builds; bless only from `cargo build --release -p simulacra_cli --features portable`.
- `golden.sh`: POSIX shell and coreutils only (`sh`, `sed`, `sort`, `sha256sum`, `mktemp`, `cmp`, `tr`). It must run under Git Bash on Windows and, for rendering, inside `debian:trixie-slim` (no Python, no Rust).
- Clippy clean with `-D warnings` for every CI combination: the workspace with and without `portable`, the engine alone with `portable`, and the engine with `json,portable`.
- New crate versions must be at least two weeks old (AGENTS.md). `sha2` is already a workspace dependency; reuse it.
- Commit messages follow the repository style: an imperative sentence, no `feat:` prefix, ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Do not commit generated images. The `--keep` directory and blessing scratch output go in a temporary directory.
- Branch: `phase6/rust-tests` (already holds the spec commits). Rebase onto `origin/main` before opening the PR.

## Review Focus

1. **A Windows checkout with CRLF line endings** in `workloads.txt`, `SHA256SUMS` or `PIXELS` must not change names or hashes. Fix: `.gitattributes` forces `eol=lf` for `core/cli/tests/golden/*` and `*.sh`, and `golden.sh` strips `\r` when it reads files. Test: a self-test case with a CRLF workloads file (Task 4).
2. **A workload whose render fails** (nonzero exit, missing output) must fail the check, naming the workload and showing the binary's stderr, not hash a stale file. Test: the stub binary fails for one name (Task 4).
3. **A non-empty `--keep` directory** left over from an earlier run would let the pixel test see stale files. `golden.sh` must refuse it. Test: a self-test case (Task 4).
4. **`--list-styles` printing nothing, or a binary path without `.exe` on Windows:** an empty inventory must fail, and `BINARY` resolves to `BINARY.exe` when only that exists. Test: the empty-styles case (Task 4).
5. **The pixel test run against a directory missing a file, or holding an extra one,** must fail naming it rather than check a subset. Test: `inventory_mismatch_names_each_difference` (Task 5).

---

## File Structure

- Create `core/cli/tests/common/mod.rs`: test-only decoders. `decode_png` (PNG and APNG: chunks, CRC, inflate, unfilter, `acTL`/`fcTL`) and `decode_gif` (with an LZW decoder written separately from `src/gif.rs`), plus `sha256_hex`.
- Create `core/cli/tests/cli.rs`: CLI behavior through `env!("CARGO_BIN_EXE_simulacra")`.
- Create `core/engine/tests/invariants.rs`: engine invariants over every style.
- Modify `core/engine/src/noise.rs` (unit test) and `core/engine/src/palettes.rs` (extend a unit test).
- Create `core/cli/tests/golden/workloads.txt` (hand-edited), `core/cli/tests/golden/SHA256SUMS` and `core/cli/tests/golden/PIXELS` (generated).
- Create `core/cli/tests/golden.sh`: check and bless.
- Create `core/cli/tests/golden_selftest.sh`: runs `golden.sh` against a stub binary in a temporary golden directory.
- Create `core/cli/tests/golden_pixels.rs`: the ignored pixel test.
- Create or modify `.gitattributes`.
- Modify `core/cli/Cargo.toml` (`[dev-dependencies] sha2`), `.github/workflows/tests.yml`, `.github/workflows/release-binaries.yml`, `AGENTS.md`, `README.md`, `PLAN.md`.
- `core/cli/tests/check_release.py` stays until PR 2 (the spec deletes it there), but no workflow calls it after Task 6.

---

### Task 1: Test decoders

**Files:**

- Create: `core/cli/tests/common/mod.rs`
- Modify: `core/cli/Cargo.toml` (add `sha2 = { workspace = true }` under `[dev-dependencies]`)
- Test: `core/cli/tests/cli.rs` (created here with the decoder tests; Task 2 adds the CLI tests)

**Interfaces:**

- Produces (all `pub`, in `mod common`; each test crate declares `mod common;` and may `#[allow(dead_code)]` the module):
  - `pub struct Decoded { pub width: u32, pub height: u32, pub frames: Vec<Vec<u8>>, pub loops: bool, pub delays: Vec<(u16, u16)>, pub table: Option<Vec<[u8; 3]>> }`. `frames` holds packed RGB per frame. `delays` holds each frame's (numerator, denominator): APNG `fcTL` values as stored, with denominator 0 read as 100; GIF graphics-control delays as `(hundredths, 100)`. `loops` is APNG `acTL` plays == 0, or a GIF `NETSCAPE2.0` loop count of 0. `table` is the GIF global table (`None` for PNG).
  - `pub fn decode_png(bytes: &[u8]) -> Result<Decoded, String>`
  - `pub fn decode_gif(bytes: &[u8]) -> Result<Decoded, String>`
  - `pub fn decode(path: &Path) -> Result<Decoded, String>`: dispatches on the extension, ASCII case-insensitive `gif`, otherwise PNG.
  - `pub fn sha256_hex(bytes: &[u8]) -> String`: lowercase hex.
- The structural rules are exactly those of `check_release.py`'s `_FrameReader`, `decode_frames`, `decode_gif`, `_lzw_decode` and `_unfilter`: 8-bit RGB without interlace, CRC on every chunk, `IEND` required, `acTL` before `fcTL`/`fdAT`, exactly one `fcTL` before `IDAT`, `fdAT` only after the second `fcTL`, sequence numbers counting from 0, every frame covering the canvas, the frame count `acTL` announces, all five filter types. GIF: `GIF89a`, global table flags `0xF7`, full-canvas frames without a local table or interlace, LZW minimum code size 8, and the decoder's width growth lagging one entry behind (`check_release.py:_lzw_decode`). Port that Python, not `src/gif.rs`.

- [ ] **Step 1: Write the failing decoder tests in `core/cli/tests/cli.rs`**

```rust
mod common;
use common::{decode_gif, decode_png};
use std::process::Command;

fn bin() -> Command { Command::new(env!("CARGO_BIN_EXE_simulacra")) }

#[test]
fn decoders_read_what_the_binary_writes() {
    let dir = tempdir("decoders");               // helper: a fresh dir under env!("CARGO_TARGET_TMPDIR")
    let png = dir.join("a.png");
    let gif = dir.join("a.gif");
    run_ok(&["--style", "plasma", "--width", "7", "--height", "5", "--frames", "3", "--out", path(&png)]);
    run_ok(&["--style", "plasma", "--width", "7", "--height", "5", "--frames", "3", "--out", path(&gif)]);
    let p = decode_png(&std::fs::read(&png).unwrap()).unwrap();
    let g = decode_gif(&std::fs::read(&gif).unwrap()).unwrap();
    assert_eq!((p.width, p.height, p.frames.len()), (7, 5, 3));
    assert_eq!((g.width, g.height, g.frames.len()), (7, 5, 3));
    assert!(p.frames.iter().all(|f| f.len() == 7 * 5 * 3));
    assert_ne!(p.frames[0], p.frames[1]);
}

#[test]
fn png_decoder_rejects_a_bad_crc_and_a_missing_iend() {
    // Write a 2x2 still, then flip one byte inside IHDR's CRC and separately truncate before IEND.
    let bytes = still_png(2, 2);                 // helper: renders --width 2 --height 2 and reads it
    let mut bad = bytes.clone(); bad[29] ^= 1;   // IHDR CRC starts at byte 29 (8 sig + 8 + 13)
    assert!(decode_png(&bad).unwrap_err().contains("CRC"));
    assert!(decode_png(&bytes[..bytes.len() - 12]).unwrap_err().contains("IEND"));
}

#[test]
fn decoders_return_known_pixels_from_hand_built_files() {
    // Built in the test, independently of src/png.rs and src/gif.rs:
    // PNG: a 3x5 RGB image with known bytes; rows use filters 0, 1, 2, 3 and 4 in turn
    //   (filtered by a small helper in the test), compressed with flate2, chunks and CRCs
    //   written with crc32fast. decode_png returns exactly the known bytes.
    // GIF: a 4x3 GIF89a with a hand-written 256-entry table (entry i = [i, 255 - i, i / 2])
    //   and indices [0, 1, 2, 255, 7, 7, 7, 7, 128, 3, 9, 200], LZW-encoded as 9-bit literal
    //   codes (clear, the literals, end) packed LSB first; and a second GIF of 600 literal
    //   indices with a clear code every 254 codes, so the code width stays 9 bits.
    //   decode_gif returns table[index] for each index, in order.
}

#[test]
fn gif_lzw_decoder_handles_table_resets() {
    // 300 x 200 noise-like frame: more than 4096 codes, so the encoder emits clear codes.
    let dir = tempdir("lzw");
    let gif = dir.join("big.gif");
    run_ok(&["--style", "nebula", "--width", "300", "--height", "200", "--out", path(&gif)]);
    let g = decode_gif(&std::fs::read(&gif).unwrap()).unwrap();
    assert_eq!(g.frames[0].len(), 300 * 200 * 3);
}
```

`run_ok(args)` runs `bin()` and asserts success. `tempdir(name)` removes and recreates `CARGO_TARGET_TMPDIR/<name>`. `path(&PathBuf) -> &str`. Define these helpers once in `cli.rs`.

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test --manifest-path core/Cargo.toml -p simulacra_cli --test cli`
Expected: compile error, because `common::decode_png` does not exist.

- [ ] **Step 3: Implement the decoders in `core/cli/tests/common/mod.rs`**

Inflate with `flate2::read::ZlibDecoder`, check CRCs with `crc32fast::hash(tag ++ body)`, and hash with `sha2::Sha256`.

- [ ] **Step 4: Run the tests to make sure they pass**

Run: `cargo test --manifest-path core/Cargo.toml -p simulacra_cli --test cli && cargo clippy --manifest-path core/Cargo.toml -p simulacra_cli --all-targets -- -D warnings`
Expected: 4 passed, and no Clippy warnings.

- [ ] **Step 5: Commit**

```bash
git add core/cli/Cargo.toml core/Cargo.lock core/cli/tests/common/mod.rs core/cli/tests/cli.rs
git commit -m "Add PNG, APNG and GIF decoders for the CLI tests"
```

---

### Task 2: CLI behavior tests

**Files:**

- Modify: `core/cli/tests/cli.rs`

**Interfaces:**

- Consumes: `common::{decode, decode_png, decode_gif, Decoded}` (Task 1), and `simulacra_engine::{PALETTES, palette_stops, SCENE_BUILDERS}` (the CLI's normal dependency, usable from its integration tests).

Every test runs the binary with a working directory under `CARGO_TARGET_TMPDIR` and canvases of at most 64 × 64, so the debug build stays fast. Expected strings are literal and come from `core/cli/src/main.rs:598-607` and from `test_simulacra.py:test_lists_and_errors`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn lists_are_sorted_and_complete() {
    // --list-styles: alphabetical, equals SCENE_BUILDERS, holds 48 names including "waves".
    // --list-palettes: equals PALETTES' names, sorted, 24 of them.
}

#[test]
fn defaults_write_art_png_256_square() {
    // In an empty dir, no arguments: stdout == "wrote art.png (waves, seed=1, palette=deep)\n",
    // stderr empty; art.png decodes to 256x256, 1 frame, no delays, loops == false.
}

#[test]
fn animation_reports_frames_and_fps_and_stores_delays() {
    // --style waves --frames 4 --fps 30 --out a.png  -> stdout
    //   "wrote 4 animation frames to a.png (waves, seed=1, palette=deep, 30 fps)\n";
    //   decoded: 4 frames, loops, delays == [(1, 30); 4]; frames not all equal.
    // Same with --out a.gif -> 4 frames, loops, delays == [(3, 100); 4]  (round(100/30) = 3).
    // --fps 12 GIF -> (8, 100); --fps 65535 GIF -> (2, 100) (at least 2 hundredths).
    // A one-frame GIF: loops == false, delays == [(8, 100)] at the default 12 fps.
}

#[test]
fn gif_follows_the_suffix_in_any_case() {
    // --out a.GIF and --out b.Gif decode as GIF89a; --out c.png.gif as GIF; --out d.gifx as PNG.
}

#[test]
fn gif_pixels_stay_close_to_the_source_rgb() {
    // Table membership follows from decoding through the table, so compare with the source:
    // --style plasma --width 48 --height 32 --seed 4 --palette sunset --frames 3 --fps 10
    // as .png and as .gif: 3 frames each, GIF delays [(10, 100); 3] and loops; every GIF
    // channel within 24 of the APNG's (test_gif_frames_use_one_table_and_the_palette_colors);
    // the table's last 16 entries are greys (r == g == b).
}

#[test]
fn random_seed_is_reported_and_reproducible() {
    // --random-seed: stdout matches "wrote x.png (waves, seed=<N>, palette=deep)\n";
    // re-running with --seed <N> gives identical decoded pixels. Two --random-seed runs
    // normally differ; do not assert that.
}

#[test]
fn palette_stops_render_like_the_built_in_palette() {
    // For "deep" and "sunset": --palette-stops built from palette_stops(name) as "rrggbb,..."
    // decodes to the same pixels as --palette <name> (style blobs, 40x30); stdout says
    // "palette=custom". Also "#0c061c,5814a8,f80" (3-digit form, leading #) is accepted.
}

#[test]
fn repeated_options_take_their_last_value() {
    // --width 8 --width 20 --height 12 --style blobs --style plasma: a 20x12 image,
    // stdout names plasma, pixels equal a plain --width 20 --height 12 --style plasma run.
}

#[test]
fn refused_arguments_exit_2_with_their_message() {
    // Table copied literally from test_simulacra.py:test_lists_and_errors (all 18 rows),
    // e.g. (&["--width", "0"], "width and height must be positive"),
    //      (&["--seed", "3", "--random-seed"], "cannot be used with"), ...
    // Each: exit code 2, message contained in stderr, stdout empty, no output file created.
}

#[test]
fn failed_writes_exit_1_and_leave_no_trace() {
    // (a) --out <dir>/missing/art.png, with [], ["--frames", "2"], and the .gif form:
    //     exit 1, stderr contains "simulacra: error: cannot write", stdout empty,
    //     <dir>/missing does not exist.
    // (b) Existing output preserved: make <dir>/keep.png a directory holding keep.txt, then
    //     --out <dir>/keep.png (still, --frames 2, and keep.gif the same way): exit 1,
    //     keep.txt unchanged, and <dir> holds no ".simulacra-*.tmp" file afterwards.
}
```

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test --manifest-path core/Cargo.toml -p simulacra_cli --test cli`
Expected: the new tests fail, or fail to compile because their bodies are empty `todo!()`s. The decoder tests still pass.

- [ ] **Step 3: Write the test bodies**

The binary's behavior already exists, so this step only fills in the test code. If a test disagrees with the binary, compare with `simulacra.py` (still present) before changing either; behavior changes are out of scope for this PR.

- [ ] **Step 4: Run them to make sure they pass**

Run: `cargo test --manifest-path core/Cargo.toml -p simulacra_cli --test cli && cargo test --manifest-path core/Cargo.toml -p simulacra_cli --test cli --features portable && cargo clippy --manifest-path core/Cargo.toml --all-targets -- -D warnings`
Expected: all pass, and no warnings.

- [ ] **Step 5: Commit**

```bash
git add core/cli/tests/cli.rs
git commit -m "Test the binary's options, messages and outputs in Rust"
```

---

### Task 3: Engine invariants and unit tests

**Files:**

- Create: `core/engine/tests/invariants.rs`
- Modify: `core/engine/src/noise.rs` (tests module), `core/engine/src/palettes.rs` (`palettes_are_sorted_unique_and_have_two_stops`)

**Interfaces:**

- Consumes: `simulacra_engine::{build_scene, render, scene_work, seed32, Palette, PALETTES, palette_stops, SCENE_BUILDERS, STYLES}`; `simulacra_engine::noise::polar_noise(r, theta, seed: u32, around, octaves)`.
- Helper in `invariants.rs`: `fn rgb(style: &str, w: usize, h: usize, seed: i64, palette: &Palette, phase: f64) -> Vec<u8>` = `build_scene(..).expect(style)` followed by `render(&*scene, w, h, seed32(seed), palette).unwrap()`. Add `fn named(name: &str) -> Palette`.

- [ ] **Step 1: Write the failing tests**

In `invariants.rs`, every style comes from `SCENE_BUILDERS`, after a check that `SCENE_BUILDERS` equals a sorted copy of `STYLES` (the same 48 names; `STYLES` is in registry order, not alphabetical) and has 48 entries:

```rust
#[test] fn every_style_renders_with_every_palette()      // 9x7, seed 3: len == 9*7*3
#[test] fn every_style_uses_a_wide_palette_range()       // 64x64, seed 3, "deep": lookup of
    // palette.color(i/100) for i in 0..=100; each sampled pixel (every 3rd row, every 3rd column)
    // maps to the first sample of least squared distance; max position - min position > 0.5
#[test] fn rendering_is_deterministic()                  // 32x24, seed 7, "sunset", phase 0.37: two renders equal
#[test] fn every_style_moves_and_loops()                 // 40x30, seed 5, "rainbow": phase 0.25 != phase 0;
    // phases 1.0, -1.0, 3.0: max |byte difference| from phase 0 <= 3
#[test] fn huge_phases_reduce_to_a_loop_position()       // 20x14, seed 3, "neon": 1e308 == 0.0 exactly;
    // 2f64.powi(40) + 0.25 == 0.25 exactly
#[test] fn every_style_renders_at_tiny_sizes()           // (1,1), (1,9), (9,1), seed 17, "deep"
#[test] fn scenes_stay_well_within_the_work_limit()      // sizes (1,1),(3,2),(1,4000),(4000,1),(64,64),
    // (640,480),(100,2000),(3840,2160); (seed, phase) in (1,0.0),(7,0.81),(12345,0.37); "rainbow";
    // scene_work(..).unwrap(): work <= limit / 8, message naming style, size, seed
#[test] fn wave_styles_are_not_aliased()                 // plasma, spiral, interference, cymatics, optics,
    // opart; seed 1, "deep": roughness(128x128) < roughness(32x32) / 2, where roughness =
    // sum over rows of |a - b| for horizontally adjacent pixels, per channel, / (h * (w - 1))
#[test] fn ring_styles_scale_with_the_canvas()           // rings, gems; seed 4, "deep"; coverage = share of
    // pixels != stop 0; small = coverage(48), large = coverage(144): small > 0.05 and
    // |small - large| < 0.5 * max(small, large)
#[test] fn formerly_flat_styles_have_spatial_detail()    // mandelbrot, petals, starburst, kaleidoscope,
    // horizon, ink; 64x48, seed 1, "deep": > 16 distinct colors, and some row has more than
    // 4 distinct colors (more than 1 for horizon)
```

In `noise.rs` tests:

```rust
#[test]
fn polar_noise_is_continuous_across_the_angle_seam() {
    for r in [0.0, 3.0, 40.0] {
        let a = polar_noise(r, std::f64::consts::PI - 1e-9, 2, 3.0, 2);
        let b = polar_noise(r, -std::f64::consts::PI + 1e-9, 2, 3.0, 2);
        assert!((a - b).abs() < 5e-7, "r = {r}: {a} vs {b}");
    }
}
```

In `palettes.rs`, extend the existing test: every palette has `stops.len() >= 5` and `max(stops[0]) < 64` (the background), with the palette's name in the message.

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test --manifest-path core/Cargo.toml -p simulacra_engine --test invariants`
Expected: compile error, or failures in tests whose bodies are still `todo!()`.

- [ ] **Step 3: Write the test bodies**

If an invariant fails on a style, stop and report it: the Python suite passes today, so a failure means the port differs from what the spec says Rust already guarantees.

- [ ] **Step 4: Run them on both backends**

Run: `cargo test --manifest-path core/Cargo.toml -p simulacra_engine && cargo test --manifest-path core/Cargo.toml -p simulacra_engine --features portable && cargo clippy --manifest-path core/Cargo.toml -p simulacra_engine --all-targets --features portable -- -D warnings`
Expected: all pass. Note the wall time of `--test invariants` in the commit message. If it exceeds 60 s in the debug build, add `[profile.dev.package.simulacra_engine] opt-level = 1` to `core/Cargo.toml` (optimization does not change float results) and note that in the commit.

- [ ] **Step 5: Commit**

```bash
git add core/engine/tests/invariants.rs core/engine/src/noise.rs core/engine/src/palettes.rs
git commit -m "Check the style invariants in Rust"
```

---

### Task 4: Golden workloads, checker and file hashes

**Files:**

- Create: `core/cli/tests/golden/workloads.txt`, `core/cli/tests/golden.sh` (executable), `core/cli/tests/golden_selftest.sh` (executable), `core/cli/tests/golden/SHA256SUMS` (blessed)
- Create or modify: `.gitattributes`

**Interfaces:**

- Produces:
  - `golden.sh check BINARY [--container IMAGE] [--keep DIR]` exits 0 if every hash matches, 1 on any failure, and 2 on usage errors.
  - `golden.sh bless BINARY [--container IMAGE]` rewrites `SHA256SUMS`.
  - Environment: `SIMULACRA_GOLDEN_DIR` (default: `golden/` next to the script) holds `workloads.txt` and `SHA256SUMS`. The self-test uses it.
  - `SHA256SUMS` is `sha256sum` output (`<hex>  <name>`), sorted with `LC_ALL=C sort -k 2`.
  - `--keep DIR` creates `DIR` if missing, refuses a non-empty `DIR`, and leaves exactly one file per workload in it. Task 5 reads that directory.

**`workloads.txt` format:** each line is `<name> <arguments...>`, split on blanks. `#` starts a comment line, and blank lines are skipped. A line containing `{style}` is expanded once per name from `BINARY --list-styles`, replacing every `{style}`. The script itself adds `--out <name>`. Names match `[A-Za-z0-9._-]+`, and anything else fails the check.

**Contents to write** (the spec's workloads: today's `check_release.py` `WORKLOADS`, the fixture settings, custom stops and `--slideshow`):

```text
# Every style, as check_release.py's WORKLOADS (sizes not multiples of each other, a tall
# canvas, negative and 64-bit seeds, phases outside [0, 1), a custom palette, APNG and GIF).
{style}-640x480-s1.png    --style {style} --width 640 --height 480 --seed 1 --phase 0.0 --palette deep
{style}-640x480-s17.png   --style {style} --width 640 --height 480 --seed 17 --phase 0.37 --palette lagoon
{style}-301x517-sm7.png   --style {style} --width 301 --height 517 --seed -7 --phase 1.25 --palette ember
{style}-517x301-s2p40.png --style {style} --width 517 --height 301 --seed 1099511627779 --phase -0.6 --palette-stops 0c061c,5814a8,ff8800
{style}-160x120-f4.png    --style {style} --width 160 --height 120 --seed 5 --phase 0.8 --palette neon --frames 4 --fps 12
{style}-150x110-f3.gif    --style {style} --width 150 --height 110 --seed -3 --phase 0.2 --palette sunset --frames 3 --fps 12
# The portable fixtures' settings (48 x 36, seed 3, phase 0.37, each fixture's own palette),
# which cycle through all 24 built-in palettes. One line per style, palette from
# core/engine/tests/fixtures/<style>.json:
fixture-asteroids.png --style asteroids --width 48 --height 36 --seed 3 --phase 0.37 --palette <its palette>
# ... 48 lines in all
# orbits draws randrange(n_stops): custom palettes of 2 and 7 stops.
orbits-stops2.png --style orbits --width 96 --height 72 --seed 2 --palette-stops 101010,f0e0c0
orbits-stops7.png --style orbits --width 96 --height 72 --seed 2 --palette-stops 000000,0a2840,145078,1e78b4,50a0dc,a0d0f0,ffffff
# Four stops in 3-digit form, as test_native_matches_python_for_custom_stops.
plasma-stops4.png --style plasma --width 96 --height 72 --seed 4 --palette-stops 000,0f0,f0f,fff
rings-stops4.png  --style rings  --width 96 --height 72 --seed 4 --palette-stops 000,0f0,f0f,fff
torus-stops4.png  --style torus  --width 96 --height 72 --seed 4 --palette-stops 000,0f0,f0f,fff
# --slideshow: consecutive seeds, as APNG and GIF.
slideshow-ink-f3.png --style ink --width 64 --height 48 --frames 3 --slideshow --seed -7 --phase 0.37
slideshow-ink-f2.gif --style ink --width 64 --height 48 --frames 2 --slideshow --seed -7
```

Generate the 48 fixture lines once with a throwaway command that reads `"palette"` from each fixture file, and commit them literally. The total is 48 × 6 + 48 + 7 = 343 renders.

**Checker behavior:**

1. Resolve `BINARY`: if it is not a file but `BINARY.exe` is, use that. Otherwise exit 2. Then make `BINARY` and the `--keep DIR` absolute (`$(cd "$(dirname "$x")" && pwd)/$(basename "$x")`, creating `DIR` first) before anything changes directory, so the documented relative invocations (`core/target/release/simulacra`, `--keep out`) work.
2. With `--container IMAGE`, first fail if `docker run --rm IMAGE sh -c 'command -v python3 || command -v python'` succeeds (ported from `check_release.py:require_no_python`).
3. Print `BINARY --version`. Read `--list-styles` (strip `\r`). Fail with `--list-styles printed nothing` if it is empty.
4. Expand the workloads (strip `\r` and use `set -f`) into a jobs file of `name<TAB>arguments`. Fail on an empty list, on a bad name, and on duplicate names (list each).
5. Render every job into the output directory (`--keep DIR`, or a `mktemp -d` removed on exit), working in that directory. Without a container, run the binary directly. With one, copy the binary, the jobs file and `golden.sh` into the directory and run one `docker run --rm --network none --user $(id -u):$(id -g) -v DIR:/work -w /work IMAGE sh /work/golden.sh render /work/<binary> /work/jobs`. `render` is an internal subcommand that runs each job. On the first failed job, print `<name>: simulacra <args> exited with <status>` and the binary's stderr, then exit 1. Remove the copied helper files before hashing.
6. `check`: compare the expanded names with the names in `SHA256SUMS` as sets. Report `no hash for <name>` (a workload missing from `SHA256SUMS`), `hash without a workload: <name>`, `duplicate in SHA256SUMS: <name>`, and an empty manifest. Then compare every hash and print `<name>: expected <x>, got <y>`. On any mismatch, end with `only portable builds match these hashes: a mismatch in attractor or cymatics usually means the binary was built without --features portable`. On success, print `<n> workloads match`.
7. `bless`: steps 1 to 5, then write the sorted `sha256sum` output to `$SIMULACRA_GOLDEN_DIR/SHA256SUMS`.

- [ ] **Step 1: Write the failing self-test `core/cli/tests/golden_selftest.sh`**

The self-test creates a temporary `SIMULACRA_GOLDEN_DIR` and a stub `simulacra` shell script. The stub prints `stub 0` for `--version` and `$STUB_STYLES` (one per line) for `--list-styles`. For a render it writes the joined arguments (excluding `--out`'s value) to `--out`'s file, and it exits 3 if any argument equals `$STUB_FAIL`. Cases, each asserting the exit status and a substring of the output:

| Case | Setup | Expect |
|---|---|---|
| bless then check | 2 styles, 1 template line plus 1 single line; `bless`, then `check` | 0, `3 workloads match` |
| changed output | after bless, edit one hash in SHA256SUMS | 1, `<name>: expected` and `only portable builds` |
| hash without a workload | append a line for `ghost.png` | 1, `hash without a workload: ghost.png` |
| no hash | add a workload line after bless | 1, `no hash for` |
| duplicate workload | two lines with the same name | 1, `duplicate` |
| empty styles | `STUB_STYLES=` and only a template line | 1, `--list-styles printed nothing` |
| failing render | `STUB_FAIL=b` | 1, `exited with 3` |
| non-empty keep | `--keep` a directory holding a file | 2, `not empty` |
| CRLF files | workloads.txt and SHA256SUMS converted to CRLF after bless | 0 |
| keep contents | `check --keep D` | 0, and D holds exactly the 3 names |
| relative paths | from the stub's parent directory: `check ./stub/simulacra --keep rel-out` (both relative) | 0, and `rel-out` holds the 3 names |

- [ ] **Step 2: Run it to make sure it fails**

Run: `sh core/cli/tests/golden_selftest.sh`
Expected: fails at the first case, because `golden.sh` does not exist.

- [ ] **Step 3: Implement `golden.sh`, write `workloads.txt` and add `.gitattributes` lines**

```text
core/cli/tests/golden/* text eol=lf
*.sh text eol=lf
```

- [ ] **Step 4: Run the self-test, then ShellCheck if it is installed**

Run: `sh core/cli/tests/golden_selftest.sh && if command -v shellcheck >/dev/null; then shellcheck -s sh core/cli/tests/golden.sh core/cli/tests/golden_selftest.sh; else echo 'shellcheck not installed; skipped'; fi`
Expected: `all golden.sh self-tests passed`, and ShellCheck exits 0 (its failure fails the command).

- [ ] **Step 5: Bless `SHA256SUMS` from a portable build, after the Python suite passes on this commit**

```bash
.venv/bin/python -m unittest -q
cargo build --release --manifest-path core/Cargo.toml -p simulacra_cli && SIMULACRA_CLI=core/target/release/simulacra .venv/bin/python -m unittest -q -k StandaloneCli test_simulacra
cargo build --release --manifest-path core/Cargo.toml -p simulacra_cli --features portable
python3 -I core/cli/tests/check_release.py check core/target/release/simulacra   # the fixtures still match
time sh core/cli/tests/golden.sh bless core/target/release/simulacra
sh core/cli/tests/golden.sh check core/target/release/simulacra
```

Expected: the suites pass, `check_release.py` prints `48 styles match their portable fixtures`, `SHA256SUMS` has 343 lines, and the check prints `343 workloads match`. Record the bless time for the PR description.

Also confirm that a default-backend binary fails the check, naming `attractor`: `cargo build --release -p simulacra_cli` (without `portable`) followed by `golden.sh check`. Then rebuild with `portable`.

- [ ] **Step 6: Commit**

```bash
git add .gitattributes core/cli/tests/golden.sh core/cli/tests/golden_selftest.sh core/cli/tests/golden/workloads.txt core/cli/tests/golden/SHA256SUMS
git commit -m "Add golden file hashes for the binary and their checker"
```

---

### Task 5: Pixel hashes

**Files:**

- Create: `core/cli/tests/golden_pixels.rs`, `core/cli/tests/golden/PIXELS` (blessed)

**Interfaces:**

- Consumes: `common::{decode, sha256_hex}` (Task 1); a `golden.sh check --keep DIR` directory (Task 4).
- Produces: `PIXELS`, with lines `<hex>  <name>#<frame>`, ordered by file name (bytewise) and then frame index, where `<hex>` is the SHA-256 of that frame's packed RGB.

- [ ] **Step 1: Write the tests**

```rust
mod common;

#[test]
#[ignore = "run on a golden.sh --keep directory: SIMULACRA_GOLDEN_OUT=DIR cargo test --manifest-path core/Cargo.toml -p simulacra_cli --features portable --test golden_pixels -- --ignored"]
fn every_frame_matches_its_pixel_hash() {
    // SIMULACRA_GOLDEN_OUT unset -> panic!("set SIMULACRA_GOLDEN_OUT to a golden.sh --keep directory").
    // An empty directory -> panic naming it.
    // Decode every file (decode()), build lines "<hex>  <name>#<i>".
    // SIMULACRA_BLESS=1 -> write them to golden/PIXELS and return.
    // Otherwise compare_inventories(expected, actual) must return no differences, then every hash must match;
    // report all mismatches in one panic message.
    // Cross-check: for every "fixture-<style>.png#0", the hash equals the "sha256" field of
    // core/engine/tests/fixtures/<style>.json (found by string search; no serde here).
}

#[test]
fn inventory_mismatch_names_each_difference() {
    // compare_inventories(&["a#0", "b#0"], &["b#0", "c#0", "c#0"]) reports
    // "no hash for c#0", "hash without a file: a#0" and "duplicate: c#0".
}
```

`fn compare_inventories(manifest: &[&str], found: &[&str]) -> Vec<String>` lives in `golden_pixels.rs`.

- [ ] **Step 2: Run the unit test, and the ignored test without the variable**

Run: `cargo test --manifest-path core/Cargo.toml -p simulacra_cli --test golden_pixels; cargo test --manifest-path core/Cargo.toml -p simulacra_cli --test golden_pixels -- --ignored`
Expected: before implementation, a compile failure. After implementation, the first passes and the second fails with the `set SIMULACRA_GOLDEN_OUT` message.

- [ ] **Step 3: Implement, then bless `PIXELS` from the Task 4 portable binary**

```bash
out=$(mktemp -d)/golden && sh core/cli/tests/golden.sh check core/target/release/simulacra --keep "$out"
SIMULACRA_GOLDEN_OUT="$out" SIMULACRA_BLESS=1 cargo test --release --manifest-path core/Cargo.toml -p simulacra_cli --features portable --test golden_pixels -- --ignored
SIMULACRA_GOLDEN_OUT="$out" cargo test --release --manifest-path core/Cargo.toml -p simulacra_cli --features portable --test golden_pixels -- --ignored
```

Expected: `PIXELS` has 48 × (5 + 4 + 3) + 5 + 3 + 2 = 586 lines (per style: 4 stills, the fixture, 4 APNG frames and 3 GIF frames; then 5 custom-stop stills and the 3 + 2 slideshow frames), and the check run passes, which includes the fixture cross-check for all 48 styles.

- [ ] **Step 4: Commit**

```bash
git add core/cli/tests/golden_pixels.rs core/cli/tests/golden/PIXELS
git commit -m "Add golden pixel hashes for every rendered frame"
```

---

### Task 6: CI

**Files:**

- Modify: `.github/workflows/tests.yml`, `.github/workflows/release-binaries.yml`

**Interfaces:**

- Consumes: `golden.sh` (Task 4), `golden_selftest.sh` (Task 4) and the `golden_pixels` test (Task 5).

- [ ] **Step 1: Add a `golden` job to `tests.yml`**

The job runs on `ubuntu-latest` with `contents: read` and the pinned checkout used elsewhere. It updates the stable toolchain, then runs:

```bash
sh core/cli/tests/golden_selftest.sh
cargo build --locked --release --manifest-path core/Cargo.toml -p simulacra_cli --features portable
out="$RUNNER_TEMP/golden"
sh core/cli/tests/golden.sh check core/target/release/simulacra --keep "$out"
SIMULACRA_GOLDEN_OUT="$out" cargo test --locked --release --manifest-path core/Cargo.toml -p simulacra_cli --features portable --test golden_pixels -- --ignored
```

The `python` and `rust` jobs stay unchanged; the new `cli.rs` and `invariants.rs` run there through the workspace `cargo test`.

- [ ] **Step 2: Switch `release-binaries.yml`'s verify job to `golden.sh` and remove the compare job**

In `verify`, remove `setup-python` and replace the `check_release.py` step with `sh core/cli/tests/golden.sh check "$binary" ${CONTAINER:+--container "$NO_PYTHON_IMAGE"}`. Drop the hashes upload step and the whole `compare` job. Update the comments above the steps: every target is checked against the same committed hashes, which is why the compare job went.

- [ ] **Step 3: Validate the workflows locally**

Run: `(command -v actionlint >/dev/null && actionlint .github/workflows/tests.yml .github/workflows/release-binaries.yml) || python3 -c "import yaml,sys;[yaml.safe_load(open(f)) for f in sys.argv[1:]]" .github/workflows/tests.yml .github/workflows/release-binaries.yml`
Expected: no errors.

- [ ] **Step 4: Commit, push and measure CI time**

```bash
git add .github/workflows/tests.yml .github/workflows/release-binaries.yml
git commit -m "Check the golden hashes in CI and on every release target"
```

After pushing (Task 7 opens the PR), record the `golden` job's time and each verify job's time in the PR description, as the spec asks ("measured in CI time before finalizing"). If the Windows verify job takes over 15 minutes, raise it with the user before trimming any workloads.

---

### Task 7: Documentation and pull request

**Files:**

- Modify: `AGENTS.md`, `README.md`, `PLAN.md`

- [ ] **Step 1: Update the docs**

- **AGENTS.md:**
  - Progress table: add a row for this PR.
  - Commands: add `golden.sh check`, `golden.sh bless`, the `--keep` plus pixel test pair, the re-bless rule (encoder-only change: `SHA256SUMS` only, with `PIXELS` unchanged and passing first; visual change: both), and `golden_selftest.sh`.
  - CI: the new `golden` job, and `verify` now running `golden.sh` without Python, with no compare job.
  - Architecture: a short "Golden hashes" item pointing at `core/cli/tests/golden/`.
  - Keep docs in sync: a style change re-blesses the golden hashes too.
  - Keep the Python content (removed in PR 2).
- **README.md:** the testing section names the golden check and the Rust integration tests.
- **PLAN.md:** under Phase 6, record that the Rust tests and golden hashes are in place (PR 1 of the cutover design), with the bless time and the CI times.

- [ ] **Step 2: Run every check CI runs**

```bash
python3 -m unittest -q && .venv/bin/python -m unittest -q
cargo test --manifest-path core/Cargo.toml && cargo clippy --manifest-path core/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path core/Cargo.toml --features portable && cargo clippy --manifest-path core/Cargo.toml --all-targets --features portable -- -D warnings
cargo test --manifest-path core/Cargo.toml -p simulacra_engine && cargo clippy --manifest-path core/Cargo.toml -p simulacra_engine --all-targets --features portable -- -D warnings
cargo test --manifest-path core/Cargo.toml -p simulacra_engine --features json,portable && cargo clippy --manifest-path core/Cargo.toml -p simulacra_engine --all-targets --features json,portable -- -D warnings
sh core/cli/tests/golden_selftest.sh
cargo build --release --manifest-path core/Cargo.toml -p simulacra_cli --features portable
out=$(mktemp -d)/golden && sh core/cli/tests/golden.sh check core/target/release/simulacra --keep "$out"
SIMULACRA_GOLDEN_OUT="$out" cargo test --release --manifest-path core/Cargo.toml -p simulacra_cli --features portable --test golden_pixels -- --ignored
```

Expected: everything passes, including `343 workloads match` and the pixel test over 586 frames. No shader or scene builder changes, so the slow checks are not required; say so in the PR.

- [ ] **Step 3: Commit, rebase and open the PR**

```bash
git add AGENTS.md README.md PLAN.md
git commit -m "Document the golden hashes and the Rust tests"
git fetch && git rebase origin/main
git push -u origin phase6/rust-tests
gh pr create --title "Rust tests and golden hashes take over from the Python suite" --body "<summary; bless and CI times; Python suite and StandaloneCliTests passed on the blessing commit; slow checks not needed; 🤖 Generated with [Claude Code](https://claude.com/claude-code)>"
```
