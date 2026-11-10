# Rust rewrite plan

Status (2026-11-10): Phase 1 is done except wider baseline coverage (more
seeds, aspect ratios and palettes). Phase 2 meets its acceptance criterion:
`core/` is a Cargo workspace with `simulacra_engine` (no PyO3 unless its
`python` feature is on) and the `simulacra_core` adapter; the math module has
the `Libm` backends and the CPython `hypot` port; one registry
(`for_each_style!`) lists the styles; scenes load from JSON (`serde`), and the
`portable` fixtures (one committed scene per style with the SHA-256 of its
render) pass in `cargo test -p simulacra_engine --features json,portable`
without Python. Open in Phase 2: the scene builders' `tanh` and `pow` (with
Phase 4). Phase 2b is done:
`simulacra_engine::PyRandom` reproduces CPython's `random.Random` for every
helper the scene builders use. Phase 3 has started: the `simulacra` binary
(`core/cli`) renders `blobs`, `mandelbrot` and `waves` to PNG without Python,
with scene builders ported to the engine, pixel-identical to `simulacra.py`
(PR #32), and test release binaries for Linux and Windows render the `portable`
fixtures on each target (`release-binaries.yml`), and the engine validates
every scene before rendering it. Phase 3 has the distribution route, Windows
signing and clean-machine smoke tests left, which wait for the first release.
Phase 4 has started: style experimentation has stopped, ported styles are
frozen in Python (see "Freeze policy"), batch 1 ported 19 more builders,
batch 2 another 12, batch 3 another 8 and batch 4 another 4, so the binary
renders 46 styles.
Phases 5 and 6 have not started.

Prepared on 2026-11-04 from the GitHub source review of
[`main` at `6e6e58b7c6a18f25406c89bacc39dad0bde1d2af`](https://github.com/juusolatva/simulacra/tree/6e6e58b7c6a18f25406c89bacc39dad0bde1d2af).
Revised the same day after a local review of that revision. The review ran no
performance benchmarks. The two measurements it cites (`hypot` agreement and a
larger native-versus-Python comparison) were run on Fedora with glibc 2.43
and CPython 3.14.8.

## Goal and timing

Deliver an easy-to-install standalone Rust application with strong rendering
and animation performance. End users should not need Python, a virtual
environment, maturin, or a Rust toolchain.

Continue experimenting with styles in Python while their composition and
animation behavior evolve. Establish performance baselines and separate the
Rust engine from Python during this period. Port scene generation in batches
as styles settle, preserving the Python implementation as a temporary reference.

Reuse the existing Rust shaders. The main migration work is scene generation,
application orchestration, encoding, packaging, and compatibility.

### Alternatives considered

The install goal alone could be met without a rewrite:

- Prebuilt wheels that include the native core (built with `maturin-action`),
  installed with `uv tool install` or `pipx`. `uv` can fetch Python itself,
  but Python is still present at run time.
- A PyInstaller or Nuitka bundle of the Python application and the native
  core, which needs no Python installation. Signing, notarization, and
  SmartScreen work are the same as for a Rust binary.

Rendering is already native, so neither route gives up shader performance, and
streaming animation frames (Phase 5) is possible in Python too:
`_atomic_write()` already accepts lazy chunks. Before Phase 3, record why
these routes are not enough (for example startup time, binary size, or one
codebase), or ship one of them as an interim release while the port proceeds.
Decided on 2026-11-06 and confirmed on 2026-11-10, after the repository went
public: there is no interim release. The first release comes when the Rust
rewrite is complete.

## Reviewed implementation

| Area | State at the reviewed revision | Migration implication |
| --- | --- | --- |
| Styles | 48, all animated and implemented in Rust | Reuse shaders; migrate scene builders. |
| Palettes | 24 built-ins, plus custom palettes | Preserve names, stops, interpolation, and custom input. |
| Python | Scene generation, random draws, CLI, animation planning, writers, reference renderers | Retain for experimentation, then remove as a runtime dependency. |
| Rust | Typed scenes extracted through PyO3, native rendering, Rayon shading | Extract a Python-independent engine API. |
| Output | PNG, APNG, GIF | Preserve formats, timing, looping, and atomic file replacement. |
| Animation memory | All uncompressed frames retained before writing | Replace with incremental rendering and encoding. |
| Tests | 81 Python test methods, plus Rust unit tests. The native-versus-Python test covers sizes up to 64 × 48 at phase 0 only | Preserve differential and behavioral coverage during migration; extend it to nonzero phases and larger sizes. |
| CI | The Python suite runs only inside the SonarQube job (skipped for fork pull requests, `ubuntu-latest`, Python 3.12). No workflow runs `cargo test` or Clippy | Add a dedicated test workflow before splitting the crate. |
| Math | Python takes `sin`, `cos`, `exp`, `atan2`, `pow`, and others from libm but implements `hypot` itself; Rust calls libm for all of them | Port CPython's own algorithms exactly; route only libm functions through a backend. |
| Benchmark | Times `render_rgb()`, excluding encoding and file writing | Add stage timing, cold/warm measurements, and peak memory. |

This table records the reviewed revision. Phase 1 has since added the test
workflow (every pull request, `cargo test` and Clippy), the native-versus-Python
test at nonzero phases and larger sizes, the `portable` math feature and the
stage benchmarks.

Since the earlier 37-style review, the project added `woodblock`, `cymatics`,
`optics`, `contour`, `marbling`, `attractor`, `opart`, `lunar`, `asteroids`,
`blackhole`, and `supernova`, plus nine palettes.

Two additions increase the importance of native scene preparation:

- `woodblock` constructs surface arrays for each layer across the image width.
- `attractor` searched for parameters, checked orbit quality across animation
  phases and cached the chosen parameters. It now picks from a fixed table
  (see "Attractor parameter table"); it still calculates orbit bounds in
  Python.

## Target architecture

```text
Rust engine
  Math, noise, palettes, typed scenes, scene construction, rendering

Rust application
  CLI, animation pipeline, encoders, resource limits, atomic output

Python adapter during migration
  Python scene conversion and access to the Rust engine
```

The engine must build and run without Python or PyO3. Python conversion belongs
in an optional adapter. Keep the existing Python builders usable through this
adapter until their Rust replacements are verified.

Proposed Cargo workspace layout:

| Crate | Kind | Contents |
| --- | --- | --- |
| `engine` | library | Math, noise, palettes, scenes, scene builders, rendering. No PyO3. |
| `cli` | binary | Argument parsing, animation pipeline, encoders, limits, atomic output. |
| `py` | cdylib | The `simulacra_core` extension module: scene extraction and exceptions. |

Today `pyo3` is a required dependency of `core` and the scene structs derive
`FromPyObject` directly (about 100 PyO3 references across the style modules).
Gate those derives with `#[cfg_attr(feature = "python", derive(FromPyObject))]`
so scene structs are not duplicated; the `py` crate enables the feature.

Use one authoritative style metadata registry where practical, so style names,
dispatch, animation capabilities, and CLI listings do not drift apart.

## Phase 1: Baselines, CI, and the compatibility decision

Timing: while style experimentation continues.

- [x] Add a test workflow (`.github/workflows/tests.yml`) that runs on every pull request, including those
  from forks: the Python suite without and with the core, `cargo test`, and
  Clippy. Today the suite runs only inside the SonarQube job, which is skipped
  for fork pull requests, and no workflow runs `cargo test` or Clippy. The
  Phase 4 ownership rule depends on this workflow.
- [x] Extend `test_native_styles_match_python_exactly`, which covers sizes up
  to 64 × 48 at phase 0 only: add a nonzero phase to the existing cases, and a
  larger case in a slower job. On 2026-11-04 all 48 styles were byte-identical
  at 200 × 150 and 150 × 200 at phases 0.37 and 0.81, which took about 2.5
  minutes of pure-Python rendering. Done: the fast test also runs phases 0.37
  and -1.81, and `test_native_styles_match_python_exactly_at_larger_sizes`
  (opt-in with `SIMULACRA_SLOW=1`) runs 200 × 150 and 150 × 200 at phases
  0.37 and 0.81 in its own CI job. Since 2026-11-08 that job is its own workflow
  (`slow-tests.yml`) and runs only on a manual dispatch (about 10 minutes on a runner); run it locally before a
  pull request that changes a shader or a scene builder.
- [x] Measure the `portable` drift now, with the current core: add a feature
  that routes the libm calls listed in Phase 2 through the `libm` crate (leave
  `hypot` as it is, since Phase 2 replaces it), rerun that comparison per
  style, and compare the attractor parameters chosen for each seed. Decide the
  compatibility policy from the result; it shapes the Phase 2 math module and
  the fixtures. Done on 2026-11-06; see "Measured `portable` drift" below.
  Policy decided on 2026-11-06: release builds use `portable` (see
  "Compatibility policy").
- [x] Record scene preparation, native rendering, encoding, and total runtime
  separately using release builds (`bench_simulacra.py --stages` and
  `--cli`; see "Stage baseline" below).
- [x] Measure first renders and repeated renders separately. Explicitly record
  whether the attractor parameter cache is cold or warm. The cache
  (`lru_cache(maxsize=64)` keyed by seed) lasts one process, so in the CLI a
  still image or a first frame is cold and later frames are warm. (The
  attractor parameter table has since removed that cache.)
- [x] Measure peak memory for still images and animations.
- [ ] Cover preview sizes, 640 × 480, and selected 4K workloads, with several
  seeds, aspect ratios, palettes, and nonzero animation phases. Partly done:
  the baseline uses one seed and palette at 640 × 480 and 4K, phase 0.37;
  `--stages` takes any size, seed, palette and phase for more.
- [x] Include fast fields, noise-heavy styles, brush styles, woodblock,
  attractor, lunar, and both GIF and APNG animations.
- [x] Keep comparisons against Python live, on the same machine, as the native
  test does today (in place: CI builds the core and compares on the runner):
  Python and Rust builders for scene data, Python and the
  `system` backend for RGB. Python's output, scene data included, depends on
  the platform libm, and CI runs on `ubuntu-latest` with Python 3.12, so
  hashes or scene data committed from one environment would break when the
  runner image changes.
- [x] Commit fixtures only for output that does not depend on the
  environment: SHA-256 hashes of packed RGB from the `portable` backend, tied
  to a source revision and generated once the Phase 2 math module exists
  (blocked on Phase 2).
  Store scene data as JSON at small sizes and as a hash at large ones
  (woodblock surfaces and sampled torus and Lissajous curves grow with the
  canvas). Do not commit raw buffers or images. Update fixtures deliberately
  when visual changes are intended. Done on 2026-11-08:
  `core/engine/tests/fixtures/<style>.json` holds one scene per style at
  48 × 36 (seed 3, phase 0.37, palettes cycling through all 24) with the
  SHA-256 of its `portable` render, about 190 KB in all. The scenes are
  generated once by `core/engine/tests/make_fixtures.py` (which needs a
  `portable` core) and committed, because Python's scene data depends on the
  platform libm; the hashes do not. Large sizes are not stored, since a hash
  alone cannot be rendered without its scene.
- [x] Record the reference environment for measurements and live comparisons:
  CPython version (3.12 or later, since `sum()` is compensated from 3.12 and
  only `random()` is guaranteed stable across versions), operating system,
  architecture, and libm (glibc version). Recorded with each measurement below
  (Fedora, x86_64, glibc 2.43, CPython 3.14); CI compares on `ubuntu-latest`
  with CPython 3.12 and 3.14.
- [x] Keep existing tests for determinism, phase-zero behavior, looping,
  palette range, scaling, and writer failure handling (a standing rule for
  every phase).

Acceptance: reproducible measurements distinguish scene, shader, encoding,
and memory costs, and the compatibility policy is decided from the measured
`portable` drift. Historical timing claims are not used as targets for the
expanded style collection.

### Stage baseline

Measured on 2026-11-06 on the reference machine (Fedora, x86_64, 8 CPUs,
glibc 2.43, CPython 3.14.10, release build of the core with the `system`
backend), seed 1, palette `deep`. Single runs vary by about ±30%; rerun on the
same machine before comparing.

`bench_simulacra.py --stages --width W --height H --phase 0.37 --repeat 3`,
totals over
all 48 native styles (best of 3, except the cold scene, which is one run):

| Size | Scene, cold | Scene, warm | Shader | PNG encode and write |
| --- | --- | --- | --- | --- |
| 640 × 480 | 0.72 s | 0.19 s | 1.44 s | 1.36 s |

Notable per style:

- `attractor`: the cold scene took 0.50 s at any size (the parameter search
  in `_attractor_base()`); warm, 0.003 s. The parameter table removed the
  search: on 2026-11-06 the cold scene at 640 × 480 went from 0.10 s to
  0.0007 s on another machine (`--stages --styles attractor`, default seed). Its shader is 0.16 s at 640 × 480
  and 2.5 s at 4K (the sequential orbit).
- `mosaic`: its scene builder takes 0.18 s at 640 × 480 and 0.24 s at 4K on
  every frame, the only warm scene cost that matters.
- At 4K (3840 × 2160) the slowest shaders are `lunar` 3.7 s, `ink` 3.3 s
  (sequential stamping), `attractor` 2.5 s and `contour` 1.7 s. PNG encoding
  (zlib level 9) adds 0.2 to 1.1 s per still.
- At 640 × 480 PNG encoding costs as much as shading in total, and more than
  shading for most fast styles.

`bench_simulacra.py --cli` (a fresh process per workload, one run each, wall
time and peak resident memory). These runs were about 20% slower than the
stage runs above, a sign of the machine's run-to-run variance:

| Workload | Seconds | Peak MiB |
| --- | --- | --- |
| Still 640 × 480 `waves` | 0.22 | 33 |
| Still 640 × 480 `attractor` | 0.74 | 35 |
| Still 640 × 480 `woodblock` | 0.20 | 33 |
| Still 640 × 480 `lunar` | 0.38 | 35 |
| Still 4K `plasma` | 1.19 | 102 |
| Still 4K `ink` | 3.54 | 102 |
| APNG 320 × 240, 48 frames, `plasma` | 0.98 | 42 |
| APNG 640 × 480, 48 frames, `attractor` | 8.14 | 75 |
| APNG 640 × 480, 48 frames, `woodblock` | 2.89 | 76 |
| GIF 320 × 240, 48 frames, `plasma` | 3.82 | 48 |
| GIF 640 × 480, 48 frames, `woodblock` | 9.21 | 85 |

The same 48 `woodblock` frames at 640 × 480 take 9.2 s end to end as GIF and
2.9 s as APNG. Encoding alone (best of 3, frames already rendered) takes 8.2 s
as GIF (the pure-Python color mapping and LZW encoder) and 2.4 s as APNG, so
GIF encoding is the largest single cost in animated output. Among the APNG
runs, `attractor` is slow because its sequential orbit shader runs per frame.
Peak memory grows with the retained frames (48 frames at 640 × 480 are
42 MiB of packed RGB). The `attractor` rows predate the parameter table, so
they include about 0.5 s of parameter search per process.

## Phase 2: Extract the Python-independent engine

Timing: while experimentation continues; preserve current visuals.

- [x] Expose ordinary Rust types and APIs for render options, palettes, scenes,
  and packed RGB buffers. Done: `Palette` with a typed `PaletteError` and the
  built-in `PALETTES` table, the `Style` trait returning packed RGB, the
  checked `render()` with `RenderError` (Phase 3), `seed32`, the `STYLES`
  registry and `PORTABLE`. Scenes come from serde (`deserialize_scene`) or,
  for ported styles, from `build_scene`; their fields stay private.
- [x] Split `core` into the `engine`, `cli`, and `py` workspace crates; gate the
  scene `FromPyObject` derives behind the `python` feature. Done for `engine`
  (`core/engine`, package `simulacra_engine`) and `py` (`core/py`, package
  `simulacra_core`); `core/Cargo.toml` is the workspace root with the shared
  release profile, lockfile and `clippy.toml`. The `cli` crate is created with
  the first binary in Phase 3. Every derive is
  `#[cfg_attr(feature = "python", derive(pyo3::FromPyObject), pyo3(from_item_all))]`.
  Native output was byte-identical before and after (all 48 styles at four
  sizes, seeds, palettes and phases, plus `fbm` and `palette_color`).
- [x] Move PyO3 extraction and Python exception conversion into the adapter.
  `core/py/src/lib.rs` holds the module, the dispatch generated from the
  engine's `for_each_style!` registry, and the conversion of `PaletteError`
  and unknown styles to `ValueError`.
- [ ] Put every transcendental call behind one engine math module, sorted by
  where Python gets the function. Partly done: `core/engine/src/math.rs` has the
  `Libm` trait with both backends (the `portable` cargo feature), and
  `core/clippy.toml` rejects direct `f64` libm calls elsewhere, `f64::hypot`
  included. `math::py_hypot` ports CPython's `hypot` (done; see below). The
  module is in the engine crate (private to it).
  - Functions CPython implements itself get exact Rust ports that both
    backends use. `math.hypot` is CPython's own algorithm (`vector_norm`), not
    libm, but Rust's `f64::hypot` calls libm: on glibc 2.43 the two disagreed
    on 12,286 of 2,000,000 random pixel-scale inputs, and CPython's result was
    the correctly rounded one in all 300 disagreements checked. Port
    `vector_norm` from the reference CPython version for two arguments. Its
    error-free products may use `mul_add`, because the fused product is exact
    there, unlike the expression rewrites the porting rules forbid. Done:
    `math::py_hypot` (the algorithm is the same in CPython 3.12 and 3.14)
    replaced all 26 `f64::hypot` calls in the shaders. It matched
    `math.hypot` bit for bit on 3.5 million inputs (pixel-scale and
    canvas-relative offsets, random bit patterns over the whole exponent
    range, subnormals, values near overflow, infinities and NaN), under both
    backends, including the 12,715 inputs where glibc 2.43's `hypot`
    disagreed; a sample of those is in the Rust unit tests. The `portable`
    drift did not change (`cymatics` drift comes from `cos`). The port costs
    shader time where `hypot` sits in an inner loop: at 640 × 480 (best of
    5, hardware FMA) `circuit` went from 17 to 25 ms and `cosmic` from 11 to
    17 ms; most styles were within noise. The cost is the algorithm (a
    second `sqrt`-and-divide step and compensated sums), not the call:
    inlining it changed nothing. `sum()`
    and `%` already have ports (`py_sum`, `py_mod`), and `round()` maps to
    `round_ties_even`.
  - Functions Python takes from libm go through two backends: `system`
    (platform libm, used for differential tests against Python) and
    `portable` (the pure-Rust `libm` crate, for release builds). The shaders
    use `exp`, `sin`, `cos`, `atan2`, `powf`, `ln_1p`, `tan`, and `atan`; the
    scene builders add `tanh` (woodblock) and `**`, which is libm `pow`. See
    "Compatibility policy".
- [x] Generate the `portable` fixtures (Phase 1) and check them in
  `cargo test -p engine`. Done: `tests/fixtures.rs` loads each scene through
  serde (`scene_from_json`, `serde_json` with `float_roundtrip`, without which
  floats can be off in the last bit; a unit test guards it) and compares the
  hash under `portable`. The hashes were computed through the PyO3 adapter,
  so a match also shows that both ways of loading a scene agree. With the
  platform libm the fixtures only have to load and render. Scene structs
  derive `serde::Deserialize` under the engine's `serde` feature, and
  `deserialize_scene` takes any serde format, so a hand-edited format (TOML
  or YAML, preferred over JSON for anything people edit) is one more feature.
- [x] Build the `py` adapter with the `system` backend for differential tests,
  and provide a `portable` build of it, so styles designed in Python can be
  previewed the way release binaries draw them. (`maturin develop -m
  core/py/Cargo.toml`, with `--features portable` for the second.)
- [x] Retain existing Python scene builders and differential tests through
  that adapter.
- [x] Preserve arithmetic order and the existing Python numeric compatibility
  helpers during extraction. (Native output was byte-identical before and
  after the split and the serde derives.)
- [x] Keep the Python extension and standalone engine build paths covered in CI.
  The Rust job tests and lints the workspace (engine with `python`, through
  feature unification) and then the engine alone, checking that its
  dependency tree has no PyO3.

Acceptance: `cargo test -p engine` passes with the `python` feature off and
no Python installed; the Python extension continues to match the reference
renderers.

## Phase 2b: Port CPython's random generator

Prerequisite for any seeded scene builder, including the second style in
Phase 3. The compatibility policy must be decided first (Phase 1).

Scene builders call `uniform` (205 call sites), `randint` (44), `random` (11),
`choice` (9), `randrange` (5) and `getrandbits` (1, the attractor's one-bit
mirror draw) on `random.Random(seed)`.

- [x] Implement a `PyRandom` module in the engine: MT19937 seeded like CPython
  (`init_by_array` over 32-bit chunks of `abs(seed)`), `random()` from two
  draws, `getrandbits()`, and `_randbelow()` with rejection sampling, plus
  `uniform`, `randint`, `randrange`, and `choice` built on them exactly as
  CPython does. Done in `core/engine/src/random.rs`, ported from
  `Modules/_randommodule.c` and `Lib/random.py`; the two make the same draws
  in CPython 3.12.0 and 3.14.4 (the differences are error handling, a lazy
  import for string seeds, and `randint` calling `_randbelow` directly where
  3.12 went through `randrange`). The builders call only `random.Random(seed)`
  with a plain integer seed and single-argument `randrange`, so that is the
  API.
- [x] Test it differentially against the pinned CPython over millions of draws
  per helper, including seed 0, negative seeds, `i64::MIN` and `i64::MAX`,
  and seeds on both sides of 2**32 (one-word and two-word keys). Test seeds
  above 2**64 only if the engine API accepts them. Done: about a million calls
  per helper (300 seeds across the `i64` range, random arguments including
  ranges just above a power of two, where rejection sampling repeats most)
  matched CPython 3.14 with no mismatch under both math backends. The
  committed check is `core/engine/tests/random_reference.json`: 14 seeds
  (the edge cases above) with 400 interleaved calls each, enough to refill
  the 624-word state mid-sequence, replayed by `tests/py_random.rs`. CPython's
  generator is platform-independent, so unlike scene data this can be
  committed; CPython 3.12.13 and 3.14.4 write identical files
  (`make_random_reference.py`). Seeds above 2**64 are not accepted.
- [x] Implement `getrandbits()` for more than 32 bits only if a builder needs
  it: the current `randint` and `randrange` bounds are small literals, list
  lengths, and tile counts. Done anyway, up to 128 bits: `randint` over the
  whole `i64` range has a width of up to 2**64, which needs 65 bits.
- [x] Decide the seed range the standalone CLI accepts and document it.
  Decided: `i64`. `PyRandom::new(seed: i64)` matches `random.Random(seed)` for
  every `i64`.
  Python accepts any integer, and `--random-seed` draws below 2**31; `i64` is
  a reasonable bound.
  - Take `abs(seed)` with `unsigned_abs()`, since `i64::MIN.abs()` overflows.
  - The slideshow's `seed + i` needs checked arithmetic.
  - Noise only sees seeds modulo 2**32 (`_seed32()`), including derived ones
    such as `seed + 7919 * layer` in looping noise, `seed + int(ro * 10.0)`
    in `quasar`, and `seed + 101 * (index + 1)` in `mosaic`, so wrapping
    arithmetic matches Python there. Only the random generator sees the full
    seed.

Acceptance: `PyRandom` reproduces CPython's sequences for every helper the
scene builders use, across the supported seed range.

## Phase 3: Prove standalone delivery with a small application

- [x] Start with `blobs` or `mandelbrot`, whose scenes make no random draws
  (`blobs` uses the seed only for noise), then add a seeded style such as
  `waves` once Phase 2b is done. Done: all three. The engine's
  `build_scene` (`scenes.rs`, listed in `SCENE_BUILDERS`) ports
  `_blobs_scene`, `_mandelbrot_scene` (its `cos` and `exp` through `Libm`)
  and `_waves_scene` (through `PyRandom`); other styles return `None`.
  Rust-built scenes render the `portable` fixture hashes
  (`ported_scene_builders_render_the_fixture_hash`).
- [x] Support dimensions, style, palette, seed, phase, custom palette stops,
  and static PNG output for the initial subset. Done in `core/cli` (binary
  `simulacra`, clap): the options, defaults, limits and messages of
  `simulacra.py` for still images, plus `--random-seed`, `--no-limits`,
  `--list-styles` and `--list-palettes`. `--frames` and GIF output come with
  Phase 5. The 24 built-in palettes are an engine table (`palettes.rs`) that
  a Python test keeps equal to `PALETTES` through `_core.palettes()`. PNG
  encoding (`flate2`, level 9, filter None) gives the same decoded scanlines
  as Python, not the same compressed bytes. `StandaloneCliTests` (when
  `SIMULACRA_CLI` names a binary; CI builds one in the default-core jobs)
  requires identical pixels and messages; 84 offline cases over the three
  styles, sizes, negative and large seeds, phases and custom stops matched.
- [x] Validate dimensions and checked buffer-size arithmetic at the engine
  boundary, not only in CLI parsing. Done: `simulacra_engine::render` rejects
  an empty canvas and a `width * height * 3` that overflows (`RenderError`);
  the PNG writer rejects sides above 2**32 - 1.
- [x] Validate scenes at the engine boundary before any untrusted scene source
  exists (at the latest with a CLI option that reads a scene file). The
  shaders trusted their scene: an index out of range (constellation pairs, an
  `orbits` ring's palette stop through the unchecked `Palette::stop`), a
  `mosaic` grid that does not match its `cols` and `rows`, `woodblock`
  surfaces shorter than the canvas width, empty polygons, or a huge iteration
  count (`ink` steps, `comet` speed and reach, constellation line steps,
  `attractor` points, `mandelbrot` iterations, long lists multiplying the
  per-pixel work) panicked or stalled rendering, and a float that saturated
  to `i64::MAX` overflowed the next integer addition. Raised by Qodo on PR
  #30. Done (`core/engine/src/validate.rs`): every style implements
  `Style::validate(&Canvas)`, and the checked `simulacra_engine::render` (used
  by the CLI and, now, the Python adapter) calls `validate_scene` first.
  Every float must be finite and at most 2**40 in size (a looping-noise
  `phase` only finite, since `fbm_loop` reduces it modulo 1 and Python passes
  `phase=1e300` through); indices, lengths and counts must fit the canvas, the
  palette and each other (`lissajous` at most 8 curves and `horizon` at most
  64 layers, as each needs a canvas-sized buffer or row); and the estimated work, in inner loop iterations with
  sequential stamps weighted 8 and attractor steps 128 to approximate time,
  must stay within `WORK_BASE + WORK_PER_PIXEL * pixels` (2**27 + 16384 per
  pixel). Measured over sizes from 1 x 1 to 3840 x 2160 and 4000 x 1 strips,
  built-in scenes use at most 8.3% of the limit (`ink` at 4K); a test holds
  them to an eighth. On a 12-thread machine the limit is roughly 20 to 30
  seconds of rendering at 4K. `marbling`'s band arithmetic and the noise
  lattice's neighbour indices were rewritten to avoid overflow with identical
  results (a valid `blackhole` scene's lensing can derive coordinates that
  saturate the lattice). Tests: every number in every
  fixture set to 1e300 must be rejected (which catches a field missing from a
  style's checks), and each structural rule and work limit has a
  malformed-scene case (`core/engine/tests/validation.rs`, and
  `test_native_render_rejects_invalid_scenes` through the adapter).
- [x] Preserve safe temporary-file creation, destination replacement, and
  cleanup on failure. Create the temporary file with mode 0o666 so the umask
  applies, as `_create_temporary()` does: `tempfile::NamedTempFile` creates
  files with mode 0600 and `persist()` keeps it, which would leave outputs
  readable only by their owner. On Unix, use `OpenOptions` with
  `create_new(true)` and `mode(0o666)`. Done (`core/cli/src/output.rs`):
  `.simulacra-<hex>.tmp` next to the destination, created exclusively with
  mode 0o666, renamed over the destination, removed on failure; tested.
- [x] Produce test release binaries for Windows (x86_64) and Linux (x86_64
  and aarch64), built with the `portable` math backend, and check the
  `portable` fixtures on each target. macOS was dropped as a target
  (decided 2026-11-10): there is no Mac to test a download on, and so no
  signing or notarization either. Done
  (`.github/workflows/release-binaries.yml`): each target builds on a native
  runner (`ubuntu-24.04`, `ubuntu-24.04-arm`, `windows-2025`), checks that `simulacra_cli` does not pull in PyO3, runs
  the engine's portable fixtures and the CLI tests there, and packages the
  binary (tar.gz, zip on Windows) as a 14-day artifact; nothing is
  published. A separate job downloads each package on a fresh runner and
  runs `core/cli/tests/check_release.py`: every style in `--list-styles`
  must render its fixture's SHA-256 from the decoded PNG, so ported styles
  join automatically. A last job requires 12 larger workloads (640 × 480 and
  non-square sizes, negative and 64-bit seeds, phases outside [0, 1), custom
  stops) to hash identically on all three targets. The Linux binaries link
  glibc from the build runner (Ubuntu 24.04); the oldest glibc to support,
  or a static musl build instead, is part of choosing the distribution
  route.
- [ ] Choose a distribution route, for example GitHub Releases built with
  `cargo-dist`, plus `cargo install`.
- [ ] Check Windows SmartScreen behavior and decide whether to sign Windows
  binaries. (macOS signing and notarization are dropped with the target.)
- [ ] Smoke-test those binaries in environments without Python installed.
  Partly done: on Linux the verify job runs the downloaded binary in a
  pinned `debian:trixie-slim` image with no Python and no Rust (the checker
  refuses an image that has Python). The Windows runner has Python
  installed, so there the evidence is that the binary does not link it
  (PyO3 absent from the dependency tree); a clean machine or VM check
  belongs with the release.

Acceptance: downloaded executables generate images that match the `portable`
fixtures on every target, without Python or build tools. Distribution is
proven before all scene builders are migrated.

## Phase 4: Port scene generation in batches

Timing: migrate styles as their behavior settles.

| Batch | Work |
| --- | --- |
| Simple fields and small scenes | Establish common scene construction, dispatch, and phase handling. |
| Geometry and object scenes | Port random layouts, stable ordering, sorting, and shape construction, including the space scenes and shared stars. |
| Sampled and prepared scenes | Port torus/Lissajous sampling, woodblock surfaces, and mosaic tile patterns. |
| Complex preparation | Port attractor parameter selection (a table lookup with `choice()` and `getrandbits(1)`), bounds, and phase updates. |

Freeze policy (decided 2026-11-10): style experimentation has stopped and the
rewrite to Rust has priority, so a ported style's Python renderer and builder
are frozen from the PR that ports its builder. They stay in `simulacra.py` as
the reference the tests compare against (native against Python, Rust scenes
against Python scenes, the binary against Python) until Phase 6 retires
Python, but they are not edited. A deliberate change to a frozen style (a new
look or a bug fix) is made in Rust only. The same PR takes the style out of
the Python comparisons, regenerates its fixture from the Rust builder, and
makes `simulacra.py` render the style from the Rust builder when the core is
installed: `_render_native` builds every scene in Python, so without that
step a Rust-only change would reach the binary but not Python users with the
core. Without the core, `simulacra.py` keeps the style's old look (or refuses
the style) until Phase 6. The first such change has to add all of this:
`make_fixtures.py` builds its scenes in Python, `_render_native` has no Rust
builder path, and the comparisons cover every style in `NATIVE_STYLES` or
`SCENE_BUILDERS`. This replaces the earlier ownership rule, which required
every change to a ported Python builder to be mirrored in Rust in the same
PR. A frozen style no longer has four places to keep in step. An accidental
edit is still caught: a change to a frozen Python builder fails
`RustSceneBuilderTests` (and `StandaloneCliTests`), and a change to a frozen
Python renderer fails `test_native_styles_match_python_exactly`, which
compares the native core with the pure-Python renderer. A Rust builder change
also fails `ported_scene_builders_render_the_fixture_hash`, and a Rust shader
change the portable fixtures. The authoring decision in Phase 6 covers styles
added after cutover.

Batch 1 (2026-11-10): the 18 per-pixel fields with short builders (`aurora`,
`bloom`, `cosmos`, `crystal`, `drift`, `galaxy`, `gems`, `horizon`,
`interference`, `kaleidoscope`, `nebula`, `petals`, `pinwheel`, `plasma`,
`rings`, `spiral`, `starburst`, `voronoi`), plus `orbits`, the first builder
that depends on the palette (`randrange(n_stops)`). With `blobs`,
`mandelbrot` and `waves` from Phase 3, 22 of the 48 builders are ported.

- Common construction: a ported scene struct implements `BuildScene`
  (`build(&SceneParams)`, the arguments of `NATIVE_STYLES[style]`), and one
  `scene_builders!` list in `scenes.rs` generates `SCENE_BUILDERS`,
  `build_scene` and `build_scene_json`. Shared pieces: `circle_offset` and
  `golden_start` (Python's `_circle_offset` and its golden-angle starts) and
  `ROUGH_TAU` (the `6.28` many builders draw phases up to) in
  `styles/mod.rs`.
- Scene comparison: scene structs also derive `serde::Serialize`, and the
  adapter's `_core.build_scene(style, width, height, seed, stops, phase)`
  returns the Rust-built scene as JSON. `RustSceneBuilderTests` compares it
  with the Python builder field by field, with floats bit for bit: six sizes
  from 1 × 1 to 3840 × 2160, seeds including negative ones, 2**40 + 3 and
  both ends of `i64`, palettes of 2, 5 and 7 stops, and phases 0, 0.37, 1,
  -1.81, 2.5, 1e9 + 0.125 and 1e300 (which cover phase zero and loop
  closure, since Python's own tests hold its scenes to the still image and
  to the loop), plus 200 seeds at assorted sizes and phases. With a
  `portable` core the floats only need to agree within 1e-12 of
  max(1, |value|) (see "Measured `portable` drift").
- Rust-only checks: every ported builder's scene passes validation at sizes
  from 1 × 1 to 300 × 2 with negative and huge phases
  (`built_scenes_are_valid`), and the portable fixtures render their hashes
  from Rust-built scenes.
- The binary: `StandaloneCliTests` now takes its styles from
  `--list-styles` (which must equal `SCENE_BUILDERS`): 264 runs over 22
  styles, 2 sizes, 3 seeds and 2 phases gave the same pixels as Python.
- Ordering: builders draw in Python's order, which is left to right within
  a tuple or dict display; `voronoi` draws every site before moving any.
  `crystal` normalises its light with `py_sum`, matching Python's
  compensated `sum()`.
- Performance at 640 × 480 (seed 3, phase 0.37), end to end: the binary took
  0.00 to 0.02 s per style with a peak of 4.4 to 7.0 MB; `simulacra.py` with
  the core took 0.05 to 0.08 s and about 36 MB, mostly interpreter start-up.
  Scene building is not measurable at this scale (about 25 µs for `orbits`
  in Python).
- No intentional visual change, so no contact sheet was needed.

Batch 2 (2026-11-10): the geometry and object scenes. The four space styles
(`asteroids`, `blackhole`, `lunar`, `supernova`) share a Rust `sky_stars`
(Python's `_sky_stars`); then `circuit`, `geo`, `mandala`, `prism` and
`shard` in `shapes.rs`, `quasar`, `cosmic` and `constellations`. 34 of the
48 builders are ported.

- Shared pieces: `drift_stars` (Python's `_drift_stars`, for `cosmic` and
  `constellations`) and `last_index` (Python's `width - 1` as a float,
  subtracted before rounding) in `styles/mod.rs`; `constellations` uses
  `fields::diagonal` for its exact `sqrt(width * width + height * height)`.
- Ordering: `asteroids` and `mandala` sort with Rust's stable `sort_by`, as
  Python's `sort` is stable (rocks by `-fade`, rings by radius), and `quasar`
  sorts its orbit radii. `prism` places every gem before any gets its
  layers, and a layer below one pixel still makes its four draws before it is
  skipped. `shard` turns points with `turned_about`, which adds the offset to
  the point so phase 0 is exact. `constellations` chooses pairs from the
  starting positions and measures them again after the drift.
- Quasar's ring seeds: Python's `_seed32(seed + int(ro * 10.0))`. At huge
  canvas sizes `ro * 10.0` exceeds `i64`, so Rust reduces the truncated value
  modulo 2**32 with float `%` (exact) before a wrapping add, which leaves the
  low 32 bits unchanged; `test_rust_scenes_match_python_at_huge_sizes`
  covers it.
- Checks: `RustSceneBuilderTests` (exact on the default core; on a portable
  core within the tolerance, largest difference 2.2e-14 of max(1, |value|) in
  `mandala`'s glyph positions), `StandaloneCliTests` over the 34 styles, the
  release check (34 styles render their portable fixtures from Rust-built
  scenes) and the slow native-versus-Python checks. On a portable core,
  Rust-built and Python-built scenes rendered the same pixels in all 2400
  cases for the 12 new styles (5 sizes up to 640 × 480, 5 seeds, 2 palettes,
  4 phases).
- Performance at 640 × 480 (seed 3, phase 0.37), end to end: the binary took
  0.01 to 0.07 s per style (`lunar` slowest) with a peak of 4.6 to 6.7 MB;
  `simulacra.py` with the core took 0.06 to 0.12 s and about 36 MB. The PNG
  files differ only in compression; the decoded pixels are identical.
- No intentional visual change, so no contact sheet was needed.

Batch 3 (2026-11-10): the remaining per-pixel fields (`contour`, `cymatics`,
`opart`, `optics`) and the brush builders (`comet`, `ember`, `flowfield`,
`ink`). 42 of the 48 builders are ported.

- `ink`'s `steps` (`max(8, ceil(length * 2))`) is now a float holding a whole
  number in the Rust scene: Python's unbounded int outgrows `i64` on sides near
  2**64 (`test_rust_scenes_match_python_at_huge_sizes`), but it is always a
  float's exact value, and the comparison accepts a Rust float against a
  Python int as for `Petals.count`. Validation requires it to be a whole
  number of at least 1 (Python would divide by zero at 0); the shader loops to
  `steps as i64`, so output is unchanged and the fixture loads as before.
- `comet` and `ember` draw positions up to `width - 1.0` (a float
  subtraction), `flowfield` and `ink` up to Python's int `width - 1`
  (`last_index`), as their Python builders do.
- Checks: `RustSceneBuilderTests` (exact on the default core; on a portable
  core within the tolerance, largest difference 6.7e-16 of max(1, |value|) in
  `ember`), `StandaloneCliTests` over the 42 styles, the release check (42
  styles render their portable fixtures from Rust-built scenes) and the slow
  native-versus-Python checks. On a portable core, Rust-built and
  Python-built scenes rendered the same pixels in all 1600 cases for the 8
  new styles (5 sizes up to 640 × 480, 5 seeds, 2 palettes, 4 phases).
- Performance at 640 × 480 (seed 3, phase 0.37), end to end: the binary took
  0.00 to 0.03 s per style with a peak of 4.6 to 6.8 MB; `simulacra.py` with
  the core took 0.05 to 0.09 s and about 36 MB. The decoded pixels are
  identical.
- No intentional visual change, so no contact sheet was needed.

Batch 4 (2026-11-10): the sampled and prepared scenes (`torus`, `lissajous`,
`woodblock`, `mosaic`). 46 of the 48 builders are ported.

- `torus` and `lissajous` share `sample_by_length`, the port of
  `_sample_by_length`: the length of 512 coarse segments is summed with
  `py_sum` (Python's `sum()` is compensated) from `py_hypot` distances.
  `torus` takes its bounds with `py_min` and `py_max`, which keep Python's
  first-of-equal choice. `lissajous`'s `reach` and `n_curves` are floats
  holding Python's ints, as for `Petals.count`.
- `woodblock`'s cliffs need `tanh`, so `Libm` gained `m_tanh` (the platform
  libm by default, the `libm` crate under `portable`), and Clippy's message
  for `f64::tanh` now points to it.
- `mosaic` ports the regions and the ten tile patterns. Python's `//` and `%`
  on ints are `div_euclid` and `rem_euclid` (every divisor is positive), and
  the region, silhouette and twinkle seeds are Python's exact sums reduced
  to 32 bits (`seed32` plus a wrapping add), all the noise and the hash
  (`noise::hash`, now crate-visible) read of them.
- `lissajous`, `woodblock` and `mosaic` build scenes that grow with the
  canvas, as in Python: curve points about a pixel apart, a height per
  column and a value per tile. The tile values take at most about 2.7 times
  the frame's bytes, while the curve points and column heights grow with a
  side alone, so they outweigh the frame only on thin canvases (under about
  60 pixels across for `lissajous`, 19 for `woodblock`). The CLI's pixel limit
  applies before any scene is built, but under `--no-limits` a side near
  `u32::MAX` asks for memory in proportion. The huge-side tests (Rust's
  `huge_sides_do_not_overflow` and `test_rust_scenes_match_python_at_huge_sizes`)
  skip these three; Python could not build those scenes either.
- Checks: `RustSceneBuilderTests` (exact on the default core; on a portable
  core within the tolerance, largest difference 9.2e-15 of max(1, |value|) in
  `woodblock`'s surfaces, none in `mosaic`), `StandaloneCliTests` over the 46
  styles, the release check (46 styles render their portable fixtures from
  Rust-built scenes) and the slow native-versus-Python checks. On a portable
  core, Rust-built and Python-built scenes rendered the same pixels in all 800
  cases for the 4 new styles (5 sizes up to 640 × 480, 5 seeds, 2 palettes,
  4 phases).
- Performance at 640 × 480 (seed 3, phase 0.37), end to end: the binary took
  0.00 to 0.02 s per style with a peak of 4.7 to 9.4 MB (`lissajous` keeps a
  distance buffer per curve); `simulacra.py` with the core took 0.07 to
  0.10 s and about 36 MB. The decoded pixels are identical.
- No intentional visual change, so no contact sheet was needed.

For each batch:

- [ ] Compare Python and Rust scene data before comparing rendered RGB: the
  `py` adapter exposes the Rust builders and returns Python objects, and
  tests require exact float equality with the Python builders.
- [ ] Check representative dimensions, seeds, palettes, and phases.
- [ ] Verify phase zero and loop closure, including negative and large phases.
- [ ] Preserve ordering where it affects floating-point accumulation,
  occlusion, or inverse operations, including marbling's operation sequence.
- [ ] Retain selected behavioral tests, not only output hashes.
- [ ] Review visual output when intentionally changing appearance.
- [ ] Record the batch's performance against the baseline.

Acceptance: every migrated style can construct its scenes and render without
Python, with compatibility and visual checks passing.

## Phase 5: Stream animation and encode natively

Begin the pipeline design with the small application; complete all formats
before application cutover.

- [ ] Render, encode, and release frames incrementally.
- [ ] If stages overlap, use a small bounded queue rather than retaining the
  entire animation. Coordinate frame-level work with Rayon to avoid excessive
  concurrency and memory use.
- [ ] Separate fixed seeded scene content from phase-dependent updates where
  this avoids repeated work without changing output.
- [ ] Reuse buffers where practical and avoid the full-frame copy currently
  needed when Rust returns a `Vec<u8>` as Python `bytes`.
- [ ] Preserve PNG/APNG/GIF dimensions, frame ordering, looping, and timing.
- [ ] Compute frame phases as the Python CLI does (`args.phase % 1.0 + i /
  frames`, with Python's float `%` from `py_mod`), so animation frames match.
- [ ] Preserve GIF palette construction and color mapping unless an intentional
  change is documented; an encoder's default quantization may differ. Feed
  the `gif` crate the existing 256-entry table and indexed frames rather than
  letting it quantize.
- [ ] Write PNG and APNG with the `png` crate. Matching the size of zlib
  level 9 output is not a goal; record size and time tradeoffs instead.
  Phase 3's still-image writer (`core/cli/src/png.rs`) uses `flate2` directly
  instead (filter None, level 9, as Python); decide here whether APNG
  extends it or both move to the `png` crate.
- [ ] Preserve atomic output replacement and cleanup on encoding failures.
- [ ] Review resource limits for the streaming design: bounded frame retention
  does not by itself bound total render work or output size.
- [ ] Validate decoded pixels, metadata, and frame timing with independent
  decoders. Identical compressed file bytes are not required.

Acceptance: retained animation frame buffers remain bounded as frame count
grows, with existing output behavior preserved.

## Phase 6: Complete distribution and retire duplication

- [ ] Support all 48 reviewed styles, 24 palettes, custom palettes, and three
  output formats; update the inventory if experimentation adds more.
- [ ] Preserve CLI defaults, listing commands, random-seed reporting, phase,
  `--frames`, slideshow, FPS behavior, resource limits and `--no-limits`,
  validation, and useful error messages.
- [ ] Add version reporting and platform-specific installation instructions.
  (`simulacra --version` exists, from the crate version 0.1.0; nothing sets
  release versions yet.)
- [ ] Automate release builds and packaged-executable smoke tests for supported
  operating systems and architectures.
- [ ] Run Rust unit tests, Clippy, compatibility checks, writer failure tests,
  decoder checks, and representative performance checks before release.
- [ ] Collect Rust coverage (for example with `cargo-llvm-cov`) and report it
  alongside Python coverage, since SonarQube's Python-only figure loses
  meaning as logic moves to Rust.
- [ ] Remove Python as an application runtime dependency only after full parity.
- [ ] Retain the Python reference temporarily for regression testing.
- [ ] Decide where new styles are authored after cutover: designed in Python
  through the binding and then ported, or written directly in Rust. Keep the
  Python binding if the first.

Acceptance: all supported functionality works from a downloadable executable
on a machine without Python, with documented compatibility and performance.

## Compatibility policy

Decided on 2026-11-06, from the measured drift below: the standalone release
binaries (Phase 3 onward) are built with `--features portable`, and the
resulting drift from the Python reference is accepted. Development builds of
the Python extension (`maturin develop`, with or without `--release`) keep the
default `system` backend, because the differential tests need it; the
`portable` feature is opt-in until the release pipeline sets it. Seeded artwork is otherwise preserved: the random generator, draw
order and numeric behavior are ported exactly, and the `system` backend still
proves each port against Python. Accepted consequences: `cymatics` may differ
by one level in rare pixels, and every `attractor` image carries different
point-density noise. (About 9% of `attractor` seeds used to draw a different
attractor too; the parameter table fixed that.) Do not silently substitute a different
RNG and assume matching seed values produce matching scenes.

Exact compatibility requires matching seed initialization, including negative
and large integer seeds; random helpers such as `random()`, `uniform()`,
`choice()`, and `randrange()`; and the order and number of random draws.
Palette stop counts can affect the random sequence in `orbits`.

Preserve relevant numeric behavior, including modulo, compensated summation,
rounding, powers, sorting ties, and expression order. Separate deliberate visual
changes from behavior-preserving migration commits.

Test numerical compatibility on supported platforms early. Attractor follows
a chaotic recurrence using `sin` and `cos`; small differences can grow into
different trajectories. Same-platform Python/Rust equality does not establish
cross-platform identity. Record the supported reference environments and
document any platform limitations rather than promising untested equality.

The math library is the main source of platform differences. Rust's `f64`
transcendental functions call the platform libm, and so do most of Python's
`math` functions (`hypot` is the exception; see Phase 2):

- A dynamically linked Linux binary depends on the user's glibc version, and
  glibc has changed results before (2.28 replaced the double-precision exp,
  exp2, log, log2, and pow).
- A static musl build uses musl's libm, which differs from glibc and therefore
  from the Python reference.
- Windows and macOS each have their own libm.

The `libm` crate is ported from musl's libm, so `portable` differs from the
glibc reference much as a musl build would. What it adds is the same result
on every target.

Beyond attractor, threshold and first-hit styles (contour lines, asteroids,
marbling, voronoi ties) can change visibly from a one-ulp difference. In
practice a one-ulp difference rarely changes an 8-bit sample: the `hypot`
mismatch produced no differing bytes in any style at 200 × 150 (Phase 1).
Attractor was the main risk. `_attractor_base()` accepted parameters through
threshold tests on a chaotic orbit (spans of at least 1.0, at least 150
occupied cells, 12 sampled phases, up to 300 attempts), so a different libm
could choose a different parameter set and draw a different attractor for the
same seed. It now picks from a fixed table (see "Attractor parameter table").

### Measured `portable` drift

Measured on 2026-11-06 (Fedora, glibc 2.43, CPython 3.14, `libm` 0.2.16).
The `portable` cargo feature of `core` sends `exp`, `sin`, `cos`, `tan`,
`atan`, `atan2`, `powf` and `ln_1p` through the `libm` crate
(`core/engine/src/math.rs`, trait `Libm`); the default build is unchanged and still
byte-identical to Python. Renders compared: every style at 200 × 150
(seed 3, lagoon), 150 × 200 (seed 17, thermal), 160 × 120 (seed -7, neon) and
640 × 480 (seed 1, deep), each at phases 0, 0.37 and 0.81.

| Style | Shaders on `portable`, scenes from Python | Scenes on `portable` math too |
| --- | --- | --- |
| 46 styles | Byte-identical | Byte-identical |
| `cymatics` | 2 bytes differ by 1 (one render of 12) | Same |
| `attractor` | Every render differs: 7.9% of bytes, up to 157 levels | 8.1% of bytes, up to 251 levels |

- Attractor renders with the same parameters show the same curves; the
  differences are point-density noise along them, because the chaotic orbit
  samples the same attractor along a different path.
- Parameter selection is the visible risk: with the scene builder's `sin` and
  `cos` (and the other libm functions it uses) taken from the `libm` crate,
  46 of seeds 0 to 499 (9%) chose different attractor parameters, so those
  seeds draw a different attractor. The scene check replaced Python's `math`
  functions through `ctypes`; `**` (libm `pow`) was not replaced.
- Speed: with the `portable` shaders a full pass over all styles at
  1280 × 960 took about 12% longer (5.8 s against 6.5 s, including Python
  scene time); `lunar` was about 20% slower.

Scene builders in Rust (measured 2026-11-10, Phase 4 batch 1): with
`portable` math, the ported builders' `sin`, `cos`, `exp` and `pow` come from
the `libm` crate, so their scenes can differ from Python's in the last bits:
circle offsets (`cos(angle) - cos(start)`), plane-wave directions in
`interference` and `plasma`, and the ring breathing in `orbits`. The largest
difference over the test grid was 1.8e-15 of max(1, |value|), in `orbits`. Rendering
Rust-built against Python-built scenes on a `portable` core over 200 cases
per style (5 sizes up to 640 × 480, 5 seeds, 2 palettes, 4 phases) changed no
pixel in any of the 22 styles ported by then (batch 2 repeated the check for
its 12 styles; see above), and every Rust-built fixture scene
renders the recorded hash. `RustSceneBuilderTests` therefore compares scenes
exactly on the default core and within 1e-12 of max(1, |value|) on a
portable one.

Resolution: the `system` math backend proves the port against Python on the
same machine; release builds use the `portable` backend, so a given seed
gives the same image on every platform, which the `portable` fixtures check
on every release target. The drift was measured in Phase 1 (see "Measured
`portable` drift") and accepted. "Standalone" and "byte-identical to Python
everywhere" cannot both hold, and this policy chooses the first.

If preserving historical output becomes too restrictive, introduce an explicit
generation-algorithm version as a separate product decision.

### Attractor parameter table

Decided on 2026-11-06: `attractor` should pick the same parameters for a seed
under either libm, even if that changes which figure a seed draws now. This
was a deliberate visual change.

`_attractor_base()` now draws one of 256 parameter sets in `_ATTRACTOR_TABLE`
with `choice()` and mirrors it (x and y swapped) on one `getrandbits(1)`. That
gives 512 figures from integer draws alone, so no libm can change the choice.
The table was searched offline (candidates from a seeded generator, rounded
to three decimals). A figure counts as stable when the orbit from four start
points within 2e-9 of (0.1, 0.1) gives nearly the same figure under both
glibc's and the `libm` crate's `sin` and `cos`: the start points stand in for
the rounding differences between libms. Each entry passes in both orientations
at all 384 phases `i / 384` with spans of at least 1.25 and at least 200 of
1,024 cells (the old search needed 1.0 and 150), cell counts within a ratio of
1.35 and spans within 1.15 between runs. At the 48 phases the tests check it
passes with headroom (1.3, 220, 1.2 and 1.1). Each also keeps a bright density
image (mean log density over lit pixels of at least 0.27; the tests need
0.25), and entries and their mirror images are at least 0.25 apart in every
parameter. Since the choice uses no floating-point math, no libm changes which
figure a seed picks (the old search: 46 of 500 seeds); the point-density noise
along the orbit remains.

The first version of the table checked only liveliness at 48 phases from one
start point. Between those phases some entries were bistable (the orbit lands
on a thin or a broad figure depending on rounding) or fell into a narrow
periodic window (the figure collapses to a few cells for a frame): 86 of 256
failed somewhere at the 48 midpoints. The current table was rebuilt with the
stability check above. Out of sample, at 768 phases (half of them new), 213 of 786,432 checks fail
(0.03%), all at new phases: periodic windows are dense in a chaotic map, so
sampling cannot rule them all out. The orbit the renderer actually draws
(start (0.1, 0.1), glibc) collapses in 13 of 196,608 new frames (0.007%), each
window narrower than 1/768 of the loop, so roughly one 60-frame animation in
250 may show one collapsed frame.

The tests check a sample of the table at 6 phases, and with `SIMULACRA_SLOW`
every entry at the 48 phases, both orientations, with the test limits.

## Performance investigations

These are candidates for measurement, not established bottlenecks.

| Target | Investigation |
| --- | --- |
| Attractor preparation | Repeated bounds calculations (the parameter search and its cache are gone). |
| Attractor rendering | Sequential orbit recurrence; integer hit counts do not make successive orbit steps independently parallelizable. |
| Woodblock | Native surface generation and avoiding transfer of per-column Python objects. |
| Lunar | Repeated crater-height evaluation during shadow marching; investigate spatial filtering while preserving contribution order. |
| Contour | Reuse of neighboring terrain samples evaluated separately today. |
| Brush and trail styles | Exact accumulation order and possible disjoint-region parallelism. |
| GIF | Native color mapping and LZW encoding. |
| PNG/APNG | Filtering, compression settings, buffering, throughput, and output-size tradeoffs; compression already runs in native zlib today. |
| All animations | Streaming, buffer reuse, scene preparation reuse, and bounded concurrency. |

All reviewed styles already have native shaders. Removing the Python frontend
alone should not be expected to repeat the original Python-to-Rust rendering
speedup. Judge improvements using end-to-end measurements and peak memory.

## Immediate next work

1. Decided: no interim release (prebuilt wheels or a bundled Python
   application). The first release comes when the Rust rewrite is complete
   (decided 2026-11-06, confirmed 2026-11-10 after the repository went public).
2. Done: add the test workflow, and extend the native-versus-Python test to nonzero
   phases and larger sizes.
3. Done: measure `portable` drift and decide the compatibility policy
   (release builds use `portable`).
4. Done: establish the stage-level performance baseline.
   Also done: `attractor` picks its parameters from a table, so the choice
   no longer depends on libm (see "Attractor parameter table").
5. Done: the Python-independent Rust engine (workspace split, math module
   with both backends and the CPython `hypot` port, scenes from JSON, the
   `portable` fixtures). Render options follow with Phase 3.
6. Done: port CPython's random generator (`PyRandom`).
7. In progress: prove standalone delivery with a small subset of styles. Done:
   the `simulacra` binary for `blobs`, `mandelbrot` and `waves`, and test
   release binaries for Linux and Windows checked against the fixtures, and
   scene validation in the engine. The distribution route and signing wait
   for the release.
8. In progress: Phase 4, porting the scene builders. Style experimentation
   has stopped and ported styles are frozen in Python (decided 2026-11-10,
   see "Freeze policy"). Batches 1 to 4 are done (46 of 48 builders
   ported). Next are `attractor` and `marbling`.
