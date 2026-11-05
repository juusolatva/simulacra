# Rust rewrite plan

Status: Phase 1 in progress (test workflow and wider native comparison done).

Prepared on 2026-11-04 from the GitHub source review of
[`main` at `6e6e58b7c6a18f25406c89bacc39dad0bde1d2af`](https://github.com/juusolatva/abstract-art/tree/6e6e58b7c6a18f25406c89bacc39dad0bde1d2af).
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

Since the earlier 37-style review, the project added `woodblock`, `cymatics`,
`optics`, `contour`, `marbling`, `attractor`, `opart`, `lunar`, `asteroids`,
`blackhole`, and `supernova`, plus nine palettes.

Two additions increase the importance of native scene preparation:

- `woodblock` constructs surface arrays for each layer across the image width.
- `attractor` searches for parameters, checks orbit quality across animation
  phases, caches chosen parameters, and calculates orbit bounds in Python.

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
| `py` | cdylib | The `abstract_art_core` extension module: scene extraction and exceptions. |

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
  (opt-in with `ABSTRACT_ART_SLOW=1`) runs 200 × 150 and 150 × 200 at phases
  0.37 and 0.81 in its own CI job.
- [x] Measure the `portable` drift now, with the current core: add a feature
  that routes the libm calls listed in Phase 2 through the `libm` crate (leave
  `hypot` as it is, since Phase 2 replaces it), rerun that comparison per
  style, and compare the attractor parameters chosen for each seed. Decide the
  compatibility policy from the result; it shapes the Phase 2 math module and
  the fixtures. Done on 2026-11-07; see "Measured `portable` drift" below.
  The policy decision is still open.
- [ ] Record scene preparation, native rendering, encoding, and total runtime
  separately using release builds.
- [ ] Measure first renders and repeated renders separately. Explicitly record
  whether the attractor parameter cache is cold or warm. The cache
  (`lru_cache(maxsize=64)` keyed by seed) lasts one process, so in the CLI a
  still image or a first frame is cold and later frames are warm.
- [ ] Measure peak memory for still images and animations.
- [ ] Cover preview sizes, 640 × 480, and selected 4K workloads, with several
  seeds, aspect ratios, palettes, and nonzero animation phases.
- [ ] Include fast fields, noise-heavy styles, brush styles, woodblock,
  attractor, lunar, and both GIF and APNG animations.
- [ ] Keep comparisons against Python live, on the same machine, as the native
  test does today: Python and Rust builders for scene data, Python and the
  `system` backend for RGB. Python's output, scene data included, depends on
  the platform libm, and CI runs on `ubuntu-latest` with Python 3.12, so
  hashes or scene data committed from one environment would break when the
  runner image changes.
- [ ] Commit fixtures only for output that does not depend on the
  environment: SHA-256 hashes of packed RGB from the `portable` backend, tied
  to a source revision and generated once the Phase 2 math module exists.
  Store scene data as JSON at small sizes and as a hash at large ones
  (woodblock surfaces and sampled torus and Lissajous curves grow with the
  canvas). Do not commit raw buffers or images. Update fixtures deliberately
  when visual changes are intended.
- [ ] Record the reference environment for measurements and live comparisons:
  CPython version (3.12 or later, since `sum()` is compensated from 3.12 and
  only `random()` is guaranteed stable across versions), operating system,
  architecture, and libm (glibc version).
- [ ] Keep existing tests for determinism, phase-zero behavior, looping,
  palette range, scaling, and writer failure handling.

Acceptance: reproducible measurements distinguish scene, shader, encoding,
and memory costs, and the compatibility policy is decided from the measured
`portable` drift. Historical timing claims are not used as targets for the
expanded style collection.

## Phase 2: Extract the Python-independent engine

Timing: while experimentation continues; preserve current visuals.

- [ ] Expose ordinary Rust types and APIs for render options, palettes, scenes,
  and packed RGB buffers.
- [ ] Split `core` into the `engine`, `cli`, and `py` workspace crates; gate the
  scene `FromPyObject` derives behind the `python` feature.
- [ ] Move PyO3 extraction and Python exception conversion into the adapter.
- [ ] Put every transcendental call behind one engine math module, sorted by
  where Python gets the function:
  - Functions CPython implements itself get exact Rust ports that both
    backends use. `math.hypot` is CPython's own algorithm (`vector_norm`), not
    libm, but Rust's `f64::hypot` calls libm: on glibc 2.43 the two disagreed
    on 12,286 of 2,000,000 random pixel-scale inputs, and CPython's result was
    the correctly rounded one in all 300 disagreements checked. Port
    `vector_norm` from the reference CPython version for two arguments. Its
    error-free products may use `mul_add`, because the fused product is exact
    there, unlike the expression rewrites the porting rules forbid. `sum()`
    and `%` already have ports (`py_sum`, `py_mod`), and `round()` maps to
    `round_ties_even`.
  - Functions Python takes from libm go through two backends: `system`
    (platform libm, used for differential tests against Python) and
    `portable` (the pure-Rust `libm` crate, for release builds). The shaders
    use `exp`, `sin`, `cos`, `atan2`, `powf`, `ln_1p`, `tan`, and `atan`; the
    scene builders add `tanh` (woodblock) and `**`, which is libm `pow`. See
    "Compatibility policy".
- [ ] Generate the `portable` fixtures (Phase 1) and check them in
  `cargo test -p engine`.
- [ ] Build the `py` adapter with the `system` backend for differential tests,
  and provide a `portable` build of it, so styles designed in Python can be
  previewed the way release binaries draw them.
- [ ] Retain existing Python scene builders and differential tests through
  that adapter.
- [ ] Preserve arithmetic order and the existing Python numeric compatibility
  helpers during extraction.
- [ ] Keep the Python extension and standalone engine build paths covered in CI.

Acceptance: `cargo test -p engine` passes with the `python` feature off and
no Python installed; the Python extension continues to match the reference
renderers.

## Phase 2b: Port CPython's random generator

Prerequisite for any seeded scene builder, including the second style in
Phase 3. The compatibility policy must be decided first (Phase 1).

Scene builders call `uniform` (209 call sites), `randint` (44), `random` (11),
`choice` (8), and `randrange` (5) on `random.Random(seed)`.

- [ ] Implement a `PyRandom` module in the engine: MT19937 seeded like CPython
  (`init_by_array` over 32-bit chunks of `abs(seed)`), `random()` from two
  draws, `getrandbits()`, and `_randbelow()` with rejection sampling, plus
  `uniform`, `randint`, `randrange`, and `choice` built on them exactly as
  CPython does.
- [ ] Test it differentially against the pinned CPython over millions of draws
  per helper, including seed 0, negative seeds, `i64::MIN` and `i64::MAX`,
  and seeds on both sides of 2**32 (one-word and two-word keys). Test seeds
  above 2**64 only if the engine API accepts them.
- [ ] Implement `getrandbits()` for more than 32 bits only if a builder needs
  it: the current `randint` and `randrange` bounds are small literals, list
  lengths, and tile counts.
- [ ] Decide the seed range the standalone CLI accepts and document it.
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

- [ ] Start with `blobs` or `mandelbrot`, whose scenes make no random draws
  (`blobs` uses the seed only for noise), then add a seeded style such as
  `waves` once Phase 2b is done.
- [ ] Support dimensions, style, palette, seed, phase, custom palette stops,
  and static PNG output for the initial subset.
- [ ] Validate dimensions and checked buffer-size arithmetic at the engine
  boundary, not only in CLI parsing.
- [ ] Preserve safe temporary-file creation, destination replacement, and
  cleanup on failure. Create the temporary file with mode 0o666 so the umask
  applies, as `_create_temporary()` does: `tempfile::NamedTempFile` creates
  files with mode 0600 and `persist()` keeps it, which would leave outputs
  readable only by their owner. On Unix, use `OpenOptions` with
  `create_new(true)` and `mode(0o666)`.
- [ ] Produce test release binaries for Windows (x86_64), macOS (arm64 and
  x86_64), and Linux (x86_64 and aarch64), built with the `portable` math
  backend, and check the `portable` fixtures on each target.
- [ ] Choose a distribution route, for example GitHub Releases built with
  `cargo-dist`, plus Homebrew and `cargo install`.
- [ ] Sign and notarize macOS binaries so Gatekeeper accepts downloads; check
  Windows SmartScreen behavior and decide whether to sign Windows binaries.
- [ ] Smoke-test those binaries in environments without Python installed.

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
| Complex preparation | Port attractor parameter selection, caching, bounds, and phase updates; the selection must choose the same parameters for each seed. |

Ownership rule: once a style's builder is ported, any change to its Python
builder must include the matching Rust change in the same PR. The
differential test in the Phase 1 test workflow enforces this.

Before the first batch, decide when a ported style's Python renderer and
builder are frozen and replaced by its fixtures. Until then each ported style
lives in four places (Python renderer, Python builder, Rust builder, Rust
shader), which slows the experimentation this plan keeps in Python. The
authoring decision in Phase 6 covers styles added after cutover.

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

The proposed default is to preserve existing seeded artwork during migration.
Confirm this policy in Phase 1, from the measured `portable` drift, before
porting seeded builders. Do not silently substitute a different RNG and assume
matching seed values produce matching scenes.

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
Attractor is the main risk. `_attractor_base()` accepts parameters through
threshold tests on a chaotic orbit (spans of at least 1.0, at least 150
occupied cells, 12 sampled phases, up to 300 attempts), so a different libm
can choose a different parameter set and draw a different attractor for the
same seed.

### Measured `portable` drift

Measured on 2026-11-07 (Fedora, glibc 2.43, CPython 3.14, `libm` 0.2.16).
The `portable` cargo feature of `core` sends `exp`, `sin`, `cos`, `tan`,
`atan`, `atan2`, `powf` and `ln_1p` through the `libm` crate
(`core/src/math.rs`, trait `Libm`); the default build is unchanged and still
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

Resolution: the `system` math backend proves the port against Python on the
same machine; release builds use the `portable` backend, so a given seed
gives the same image on every platform, which the `portable` fixtures check
on every release target. Measure how far `portable` output drifts from the
reference per style in Phase 1 (differing bytes, and whether attractor
parameters or trajectories change) and decide whether that drift is
acceptable before porting seeded builders. "Standalone" and "byte-identical
to Python everywhere" cannot both hold without this choice.

If preserving historical output becomes too restrictive, introduce an explicit
generation-algorithm version as a separate product decision.

## Performance investigations

These are candidates for measurement, not established bottlenecks.

| Target | Investigation |
| --- | --- |
| Attractor preparation | Cold parameter-search cost, cache behavior, and repeated bounds calculations. |
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

1. Record why prebuilt wheels or a bundled Python application are not enough
   (see "Alternatives considered"), or ship one as an interim release.
2. Add the test workflow, and extend the native-versus-Python test to nonzero
   phases and larger sizes.
3. Confirm the compatibility policy from the measured `portable` drift
   (measurement done).
4. Establish the stage-level performance baseline.
5. Extract the Python-independent Rust engine while retaining the adapter,
   including the math module (CPython's `hypot`, `system` and `portable`
   backends) and the `portable` fixtures.
6. Port CPython's random generator.
7. Prove standalone delivery with a small subset of styles.
8. Continue Python style experimentation and migrate settled styles in batches.
