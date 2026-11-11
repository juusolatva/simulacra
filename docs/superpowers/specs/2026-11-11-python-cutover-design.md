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
  format, so `sha256sum -c` checks it directly. Whole files are hashed: the
  PNG/APNG (`flate2` with the pure-Rust `miniz_oxide` backend) and GIF
  encoders are deterministic and pinned by `Cargo.lock`, so a file hash also
  checks the encoders. A dependency update that changes compressed bytes
  without changing pixels is re-blessed after the decoded-pixel check below
  confirms the pixels.
- **Workloads:** every style (from `--list-styles`) at the six sizes, seeds,
  phases and palettes of today's `check_release.py` `WORKLOADS` (640 × 480,
  non-square sizes, negative and 64-bit seeds, phases outside [0, 1), a
  custom palette, a 4-frame APNG, a 3-frame GIF) plus the portable fixture
  setting (48 × 36, seed 3, phase 0.37); every built-in palette at least once
  across styles; custom palettes with 2 and 7 stops for `orbits` (it draws
  `randrange(n_stops)`) and a few other styles; `--slideshow`. About 350
  renders, measured in CI time before finalizing.
- **Checker:** `core/cli/tests/golden.sh check BINARY [--container IMAGE]`
  renders every workload into a temporary directory and runs `sha256sum -c`;
  `golden.sh bless BINARY` rewrites `SHA256SUMS`. POSIX shell and coreutils
  only, so it runs on the Linux and Windows (Git Bash) runners and inside the
  Python-free container image. The hashes hold only for `portable` builds;
  a default-backend binary fails in `attractor` (and possibly `cymatics`),
  which the failure message names as the likely cause.
- **Where it runs:** in `tests.yml` against a release `portable` binary on
  every pull request, and in `release-binaries.yml`'s verify job against
  each downloaded package (replacing `check_release.py check`); the compare
  job is no longer needed, since every target is checked against the same
  committed hashes.
- **Recording:** blessed once in PR 1 from a portable build, after the
  Python suite (including `StandaloneCliTests`) passes on the same commit.

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
    4K (today's `test_native_scenes_stay_well_within_the_work_limit`).
- Existing Rust tests (engine units, `validation.rs`, `py_random.rs`,
  `fixtures.rs`, CLI units) stay.

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
  sends `lcov.info`; `core/coverage.sh` shrinks accordingly;
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
- **Encoder byte changes** from a dependency update need a re-bless; the
  decoded-pixel tests show whether pixels changed.
- **Windows shell:** `golden.sh` must run under Git Bash on the Windows
  runner (`shell: bash` is already the workflow default).

## Out of scope

New styles or palettes, a modular style setup, GitHub Releases, signing,
buffer reuse.
