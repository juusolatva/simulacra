# Retire Python: a Rust-only Simulacra (PLAN.md Phase 6)

## Goal

Make the standalone `simulacra` binary the whole application and remove
Python from the repository: the reference renderer `simulacra.py`, its tests,
the PyO3 extension and every Python helper script. Keeping a second
implementation next to the binary would make every future change twice the
work and a source of disagreements.

Success:

- The repository builds, tests and releases with Cargo alone; CI installs no
  Python.
- Every style and palette renders exactly as it does now in a `portable`
  build (the release configuration), checked on every release target by
  committed golden hashes recorded while the Python comparison still passed.
- The tests keep the general invariants and the CLI behavior that the Python
  suite checked, now as Rust tests with literal expectations.

## Decisions (2026-11-11)

- **Scope:** retire `simulacra.py` and its tests too, not only the extension.
- **Tests:** golden hashes plus the general invariants (approach A). The
  Python-versus-Rust comparisons go with the reference; style-specific Python
  tests (woodblock landforms, mosaic grout, lunar shadows, the attractor
  table criteria, ...) are dropped, since the golden hashes catch any change
  to those styles and no styles are being added or changed for a while.
- **Python helper scripts:** ported to Rust or shell, or dropped; no Python
  remains. Committed reference data (`random_reference.json`, the fixture
  scenes) stays, but the generators that need the Python reference go.
- **Math backends:** decided after Python is gone, from a measurement (PR 3).
- **Delivery:** three pull requests, each leaving the repository releasable
  and its documentation accurate.
- **Not in this work:** new styles or palettes, a modular style setup (left
  for later; it would cut against the single portable binary), and GitHub
  Releases (after the cutover).

## PR 1: Rust tests take over

Python stays in this PR, and its suite (including `StandaloneCliTests` against
a default-backend binary) must still pass, which shows that the golden hashes
are recorded from output that matches the Python reference (exactly on the
default backend; within the accepted `portable` drift in `attractor` and
`cymatics`).

### Golden hashes (`core/cli/tests/golden/`)

- `workloads.txt` (hand-edited): one workload per line, an output name and
  the binary's arguments; `#` starts a comment. A line containing `{style}`
  is a template expanded once for every style the binary lists, for example
  `{style}-640x480-s17.png --style {style} --width 640 --height 480 --seed 17 --phase 0.37 --palette lagoon`,
  so the file stays short and a style added later is covered automatically
  (its missing hashes fail the check until blessed). Other lines are single
  workloads, such as the palette sweep.
- `SHA256SUMS` (generated): the SHA-256 of each output file in `sha256sum`
  format. Whole files are hashed: the PNG/APNG (`flate2` with the pure-Rust
  `miniz_oxide` backend) and GIF encoders are deterministic and pinned by
  `Cargo.lock`, so a file hash also checks the encoders.
- `PIXELS` (generated): the SHA-256 of every decoded frame's packed RGB, one
  line per frame (`name#0`, `name#1`, ...), so a record of the images
  themselves exists apart from their encoding. It is checked and written by
  the pixel test below, never by `golden.sh bless`.
- **Re-blessing:** a change that alters file bytes but not images (a
  `flate2` or `miniz_oxide` update, say) re-blesses `SHA256SUMS` only; the
  pixel test must pass against the unchanged `PIXELS` first, and the pull
  request's diff then shows `PIXELS` untouched. A deliberate visual change
  rewrites both, and its diff shows which workloads and frames changed.
- **Workloads:** every style (from `--list-styles`) at the six sizes, seeds,
  phases and palettes of today's `check_release.py` `WORKLOADS` (640 × 480,
  non-square sizes, negative and 64-bit seeds, phases outside [0, 1), a
  custom palette, a 4-frame APNG, a 3-frame GIF) plus the portable fixture
  setting (48 × 36, seed 3, phase 0.37); every built-in palette at least once
  across styles; custom palettes with 2 and 7 stops for `orbits` (it draws
  `randrange(n_stops)`) and a few other styles; `--slideshow`. About 350
  renders, measured in CI time before finalizing.
- **Checker:** `core/cli/tests/golden.sh check BINARY [--container IMAGE]
  [--keep DIR]` expands the workloads, renders each into a temporary
  directory (or `DIR`, kept for the pixel test) and compares. `sha256sum -c`
  alone would ignore an output with no manifest line, so the check first
  requires the expanded workload names and the manifest names to be the same
  set: it fails on an empty inventory, on a duplicate name in either list, on
  a workload with no hash and on a hash with no workload, naming each; then
  it compares every hash. `golden.sh bless BINARY` rewrites `SHA256SUMS`
  (after the same duplicate and empty checks on the workloads). POSIX shell
  and coreutils only, so it runs on the Linux and Windows (Git Bash) runners
  and inside the Python-free container image. The hashes hold only for
  `portable` builds; a default-backend binary fails in `attractor` (and
  possibly `cymatics`), which the failure message names as the likely cause.
- **Pixel test:** `core/cli/tests/golden_pixels.rs`, an ignored test run
  explicitly (`cargo test -p simulacra_cli --test golden_pixels -- --ignored`)
  with `SIMULACRA_GOLDEN_OUT` pointing at a `golden.sh --keep` directory. It
  decodes every output with the test decoders below (every APNG and GIF
  frame) and requires the frame names to equal `PIXELS`' names exactly (same
  inventory rules) and every frame hash to match; with `SIMULACRA_BLESS=1` it
  rewrites `PIXELS`. Run without `SIMULACRA_GOLDEN_OUT`, it fails rather than
  passing vacuously.
- **Where it runs:** in `tests.yml` on every pull request, `golden.sh check
  --keep` against a release `portable` binary and then the pixel test on
  that directory; in `release-binaries.yml`'s verify job, `golden.sh check`
  against each downloaded package (replacing `check_release.py check`; no
  Rust there, and the file hashes imply the pixel hashes). The compare job is
  no longer needed, since every target is checked against the same committed
  hashes.
- **Recording:** both manifests blessed once in PR 1 from a portable build,
  after the Python suite (including `StandaloneCliTests`) passes on the same
  commit.

### Rust integration tests

- **CLI behavior** (`core/cli/tests/cli.rs`, running `CARGO_BIN_EXE_simulacra`):
  the list options (alphabetical styles, sorted palettes), defaults
  (`art.png`, 256 × 256, `waves`, `deep`, seed 1), the `wrote …` lines,
  `--random-seed` reported and reproducible, `--palette-stops` equal to the
  matching built-in palette's output, `--fps` setting APNG and GIF delays,
  frame counts, GIF for a `.gif` name in any case, repeated options, every
  error in today's `test_lists_and_errors` table with its exit status, and
  failed writes (exit 1, existing output preserved, no temporary file left).
  Small canvases, so the debug build stays fast.
- **Decoded pixels:** a small PNG/APNG reader (chunks, CRCs, `flate2`
  inflate, `acTL`/`fcTL`) and a GIF reader (LZW decoding written separately
  from the encoder) in the test code, used to check frame counts, timing,
  sizes and that pixels use only palette colors where the format promises
  it. These replace the Python reference decoders.
- **Invariants** (engine-level, `core/engine/tests/invariants.rs`, through
  `build_scene` and the checked `render`, at small sizes):
  - every style renders with every built-in palette at the requested size;
  - every style spans more than half of its palette at 64 × 64 (the
    `test_every_style_uses_a_wide_palette_range` method: nearest of 101
    palette samples, every third pixel);
  - rendering is deterministic;
  - every style moves (phase 0.25 differs from 0) and loops (phases 1, -1 and
    3 within 3 levels of phase 0);
  - built-in scenes stay within an eighth of the work limit at sizes up to
    4K (today's `test_native_scenes_stay_well_within_the_work_limit`);
  - huge finite phases reduce to the same loop position: every style renders
    phase `1e308` exactly as phase 0, and `2**40 + 0.25` exactly as 0.25
    (`test_huge_phases_are_reduced_to_a_loop_position`);
  - every style renders at 1 × 1, 1 × 9 and 9 × 1 (generalizing
    `test_galaxy_single_pixel`; all 48 pass today);
  - wave frequencies stay below Nyquist: `plasma`, `spiral`,
    `interference`, `cymatics`, `optics` and `opart` are less than half as
    rough at 128 × 128 as at 32 × 32 (`test_wave_styles_are_not_aliased`);
  - `rings` and `gems` cover a similar share of the canvas at 48 and 144
    pixels (`test_formerly_fixed_size_styles_scale_with_canvas`);
  - `mandelbrot`, `petals`, `starburst`, `kaleidoscope`, `horizon` and `ink`
    keep spatial detail (`test_previously_flat_styles_have_spatial_detail`).
- **Engine unit tests added next to the code:** `polar_noise` is continuous
  across the ±π seam at several radii (`noise.rs`;
  `test_polar_noise_is_continuous_across_the_angle_seam`), and every built-in
  palette has at least 5 stops and a dark first stop, every channel below 64,
  since many styles take their background from it (`palettes.rs`, extending
  `palettes_are_sorted_unique_and_have_two_stops`;
  `test_every_palette_is_valid_and_starts_dark`).
- Existing Rust tests (engine units, `validation.rs`, `py_random.rs`,
  `fixtures.rs`, CLI units) stay. Several already cover Python tests: `fbm`'s
  range and the looping noise (`noise.rs`), the GIF encoder and its table
  (`gif.rs`), atomic writes and the umask-default mode (`output.rs`), frame
  phases including huge ones (`main.rs`), scene rejection (`validation.rs`).
- **Accepted reductions** (Python tests with no Rust replacement):
  - the Python-versus-Rust comparisons (native renders, Rust scenes,
    `StandaloneCliTests`, noise and palettes), which go with the reference;
  - style-specific tests: flowfield and comet trails, quasar jets, the
    mandelbrot interior, lissajous span, woodblock, mosaic, cymatics, optics,
    contour, opart, marbling, the attractor table criteria and framing, the
    space styles, constellation lines and cosmos bands;
  - Python-API-only tests: global random state, packed-RGB clamping and
    conversion, the writers' row input, `inspect.signature`, the pruned
    nearest-segment helper, open-curve sampling and the styles-without-motion
    check (every style moves).

## PR 2: Remove Python

- **Delete:** `simulacra.py`, `test_simulacra.py`, `bench_simulacra.py`,
  `.coveragerc`, `core/cli/tests/check_release.py`,
  `core/cli/tests/check_coverage.py`, `core/engine/tests/make_fixtures.py`,
  `core/engine/tests/make_random_reference.py`, `core/py` (the
  `simulacra_core` extension), the engine's `python` feature with every
  `cfg_attr(feature = "python", ...)` derive, PyO3 and the maturin
  configuration. `random_reference.json` and the fixture scenes stay as
  committed data; the files that describe them say they were generated from
  CPython and the Python builders before the cutover.
- **Fixture bless:** `SIMULACRA_BLESS=1 cargo test -p simulacra_engine
  --features json,portable --test fixtures` rewrites a fixture's scene from
  the Rust builder and its hash, for deliberate changes (replacing
  `make_fixtures.py`); without it the committed scenes are only checked.
- **Benchmark:** an engine example (`cargo run --release --example bench`)
  times scene building and shading per style at a given size and phase, with
  repeats; encoding and whole runs are timed by running the binary (a short
  shell loop or `hyperfine`, documented). PLAN.md's Python-era baselines
  stay as history.
- **CI:** `tests.yml` keeps `cargo test` and Clippy on both backends, the
  engine without other crates, the portable fixtures and the golden check,
  and drops the Python matrix; `slow-tests.yml` is deleted (its checks
  compared with Python); `sonarqube.yml` runs `cargo llvm-cov` over the
  workspace tests (the CLI integration tests run the instrumented binary) and
  sends `lcov.info`; `core/coverage.sh` shrinks accordingly, and its LCOV
  safeguards move from `check_coverage.py` into a POSIX shell and `awk`
  check (`core/check_lcov.sh`) so that lost subprocess instrumentation fails
  CI instead of lowering the figure: it fails on a malformed `DA` line, if
  the line holding `let args = Args::parse();` in `main()` of
  `core/cli/src/main.rs` (found by its text; only the binary runs it, not
  the unit tests) has no hit, or if nothing under `core/engine/src/styles`
  has a hit. Its own negative cases (a report without those hits, a
  malformed line) are run once by hand and recorded in the PR;
  `release-binaries.yml` drops `setup-python`. `sonar-project.properties`
  lists only the Rust sources and drops the Python settings.
- **CodeQL:** default setup in the repository settings lists `actions`,
  `python` and `rust`; after the merge the owner removes `python` there (a
  settings change, not a file).
- **Docs:** the README leads with the binary (install, options, styles,
  palettes, animation, limits, testing, project files); the Python API,
  Python quick start, optional Rust core and "Extending the script" sections
  go or become Rust equivalents ("Adding a style" in Rust, noting that styles
  are not being added for now). AGENTS.md is rewritten for a Rust-only
  repository (the freeze policy ends: styles live in Rust only, and a
  deliberate change re-blesses the golden hashes and fixtures). PLAN.md
  records the cutover and ticks Phase 6's runtime-removal items.

## PR 3: Measure the math backends, then decide

With Python gone, the platform-libm (default) backend no longer has a
reference to match; it remains only for speed.

1. Measure `portable` against the platform libm with the new benchmark: every
   style as a still at 1920 × 1080 and as a 12-frame animation, on Linux (this
   machine and CI) and Windows (CI), with repeats.
2. Present the numbers with a recommendation; the owner decides between:
   - **portable only:** remove the platform backend (the `Libm` trait keeps one
     implementation, `clippy.toml`'s rules and the CI backend matrix
     simplify), and the install command drops `--features portable`;
   - **portable default, `system-libm` opt-in:** the default build becomes
     what `portable` is today, and a `system-libm` feature selects the
     platform libm for speed; CI keeps building, testing and linting it, and
     the golden hashes apply only to the default.

Either way the golden hashes stay unchanged, since they are recorded from
`portable` builds.

## Risks

- **Golden coverage is narrower than the Python comparison** (which compared
  every style at many sizes, seeds and phases). Mitigation: the workload set
  above, the engine fixtures, and the invariants; and no style changes are
  planned.
- **Encoder byte changes** from a dependency update need a re-bless of
  `SHA256SUMS`; the pixel test against the unchanged `PIXELS` shows whether
  any image changed.
- **Windows shell:** `golden.sh` must run under Git Bash on the Windows
  runner (`shell: bash` is already the workflow default).

## Out of scope

New styles or palettes, a modular style setup, GitHub Releases, signing,
buffer reuse.
