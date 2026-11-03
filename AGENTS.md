# AGENTS.md

Guidance for coding agents working in this repository. User-facing documentation lives in [README.md](README.md).

## Project overview

`abstract_art.py` is a generative art tool: 36 styles, 9 palettes, PNG/APNG writers and a CLI. `test_abstract_art.py` holds the `unittest` suite and `bench_abstract_art.py` the benchmark. The pure-Python module needs only the standard library and stays usable on its own as the reference implementation. `core/` is an optional Rust extension that renders all 36 styles natively. Third-party dependencies are allowed; the standard-library-only rule has been dropped.

## Progress

| Stage | Status | Where |
| --- | --- | --- |
| Bug fixes: color handling, noise range, dark and flat styles, writers, `STYLES` registry, tests | Done | PR #1 |
| SonarQube cleanup: complexity ≤ 15, umask-safe writers | Done | PR #1 |
| Benchmark script (`bench_abstract_art.py`) | Done | PR #2 |
| Python algorithmic fixes: exact culling, segment-based torus and lissajous | Done | PR #2 |
| Rust core (PyO3 + rayon) with packed `render_rgb()` | Done | PR #2 |
| Native styles, batch 1 (18): blobs, bloom, circuit, cosmic, drift, galaxy, geo, interference, kaleidoscope, mandala, nebula, orbits, pinwheel, plasma, prism, quasar, shard, waves | Done | PR #2 |
| Native styles, batch 2 (11 per-pixel): crystal, voronoi, cosmos, mandelbrot, aurora, spiral, petals, starburst, rings, gems, horizon | Done | PR #3 |
| Native styles, batch 3 (7 brush and trail): torus, lissajous, constellations, ink, flowfield, comet, ember | Done: sequential stamping in Python's order, parallel shading (`core/src/styles/brush.rs`) | this branch |
| More functionality and options | **Next** (performance is complete) | — |

Measured at 640×480: all 36 styles took 178 s in pure Python at the start. With all 36 styles native, a run on a fast machine takes about 0.2 s in total, and about 1.4 s for the seven brush and curve styles at 4K, the slowest being `ink` (0.8 s, sequential stamping). Compare against a fresh baseline, not these numbers.

## Commands

```bash
python3 -m unittest -q                     # full suite, pure Python (native tests skip)
.venv/bin/python -m unittest -q            # with the Rust core: also checks native == Python
source .venv/bin/activate && maturin develop --release -m core/Cargo.toml   # rebuild core
cargo test --manifest-path core/Cargo.toml && cargo clippy --manifest-path core/Cargo.toml --all-targets
python3 bench_abstract_art.py --width 640 --height 480 --compare <saved.json>
python3 abstract_art.py --style ink --palette lagoon --seed 17 --out ink.png
python3 abstract_art.py --help             # lists styles (from STYLES) and palettes
```

Do not commit generated images.

## Architecture

- **Registry:** `STYLES` maps each style name to its `render_*` function. It drives `render()`, the CLI `--style` choices and the tests, and is the only place a style is registered.
- **Renderer contract:** `render_x(width, height, seed, palette_name)` returns `height` rows of `width` `(r, g, b)` integer tuples. Map intensities through `palette_color(stops, t)` with `t` in `[0, 1]`.
- **Randomness:** use a local `random.Random(seed)`. Never touch the global `random` state; a test enforces this. Output must be deterministic for a given seed, size and palette.
- **Noise:** `fbm()` returns values in `[0, 1]` centered near 0.5. Do **not** treat it as `[-1, 1]` (`n * 0.5 + 0.5` was a past bug in 8 styles). Use `_contrast(n, gain)` to stretch it, and `polar_noise()` for anything indexed by an angle, because raw `theta` creates a seam at ±π.
- **Colors:** palette stops are sRGB, and the writers store samples unchanged. Do not reintroduce a linear-to-sRGB conversion.
- **Writers:** `_atomic_write()` writes to a temporary file created with `os.open(..., 0o666)` and then calls `os.replace()`. Never call `os.umask()`, since it is process-wide and unsafe with threads.
- **Rust core (`core/`, module `abstract_art_core`):** optional; imported as `_core` (None when missing). Python generates the scene (all `random.Random` use) with a `_x_scene()` builder that returns plain data. `NATIVE_STYLES` maps each native style to its builder, and `_core.render(style, width, height, seed, stops, scene)` reads the scene into a per-style Rust struct (`#[derive(FromPyObject)]`, `from_item_all`), shades rows in parallel (rayon) and returns packed RGB bytes. `render()` and `render_rgb()` use it automatically. Pass seeds through `_seed32()`. `_core.STYLES` must match `NATIVE_STYLES` (tested).
- **Native output must be byte-identical to Python.** Port expressions operation-for-operation in the same order (no algebraic rewrites, no `mul_add`, no `clamp` where Python's `max/min` NaN behavior differs), and keep the Python renderer as the reference. `test_native_styles_match_python_exactly` enforces this; extend its cases when porting a style. Move the style's random layout into `_x_scene()` first and have the Python renderer use it, verifying unchanged output with a fingerprint. Rust helpers that replicate Python semantics live in `core/src/geometry.rs` (`py_mod`, `py_sum`, polygon and segment distances) and `core/src/styles/mod.rs` (`gaussian`, `unit`, `center`, `small_side`).
- **Packed frames:** `render_rgb()` returns `width * height * 3` bytes; the writers accept rows or packed bytes (`_packed_rgb`). The CLI uses packed frames.
- **Helpers for keeping functions small:** `_shade(width, height, stops, value)` builds rows from a per-pixel `value(x, y)`. `_sample_by_length()` and `_segment_box()` sample curves and yield the pixels near a segment. `_RowFilter` hands per-pixel code only the objects that can reach a row. `_glow_pixels()` maps a flat intensity buffer to rows.

## Conventions

- **Size independence:** express shape sizes, radii, wave frequencies and noise scales relative to the canvas (`min(width, height)` or `max(width, height)`), not in pixels. Keep wave frequencies well below Nyquist (π radians per pixel). The tests check palette range, aliasing and canvas scaling.
- **Brightness:** prefer `1 - exp(-k * v)` tone mapping over dividing by a fixed count, which left several styles nearly black. Every style must span more than 0.5 of its palette at 64×64 (`test_every_style_uses_a_wide_palette_range`).
- **Cognitive complexity ≤ 15 per function** (SonarQube S3776). Move per-pixel or per-sample bodies into module-level helpers placed directly above the style that uses them, and use comprehensions or `_shade()` for the row loops.
- **Behavior-preserving refactors:** fingerprint every style's output before and after (hash `render()` at a few sizes, seeds and palettes) and require identical hashes. When a change is meant to alter visuals, render a contact sheet of all styles and inspect it.
- **Naming:** local names and parameters use `snake_case` (S117); no single uppercase names such as `R`.
- **Keep docs in sync:** when adding or renaming a style, update `STYLES`, the module docstring's style list, the README style table and, if relevant, the tests.

## SonarQube triage notes

Project key: `juusolatva_abstract-art`.

- **S2245 (pseudorandom generator):** false positive. Seeded `random.Random` is used deliberately for reproducible art.
- **S5863 in the determinism test:** false positive. The two sides are separate renders that must be equal.
- **pythonsecurity:S8707 (path traversal on `--out`, or on the benchmark's `--save` and `--compare`):** accepted. A local tool reads and writes where its user asks.
- **Re-raised findings:** moving code (for example into a new `_x_scene()` or helper) makes SonarQube report the same S2245 or S8707 again at the new location. Triage them the same way, noting in the comment that the code moved.
- **S1172 (`seed` unused in `render_mandelbrot`):** accepted, because the shared renderer signature requires it.

## Known issues and follow-ups

### Performance

Pure Python takes about 178 s for all 36 styles at 640×480 (see `bench_abstract_art.py`). The Rust core now covers every style in `NATIVE_STYLES`. Algorithmic fixes were done in Python first because they carry over to Rust. Single-run timings on this machine vary by about ±30%, so use `--repeat` and compare against a fresh baseline.

- **Culling has been done** for the cases that keep output byte-identical: row filtering (`_RowFilter`) for stars in `orbits`, `nebula`, `cosmic` and `constellations`, and for glyphs in `mandala`; exact-zero ring bands in `mandala`; column bounds in `ember`; and bounding-box pruning of segments in `circuit` (`_nearest_segment`). A skip is allowed only where a contribution is exactly zero, either from a hard cutoff or because `exp()` underflows past `GAUSSIAN_ZERO` standard deviations.
- **`render_torus` and `render_lissajous`** now sample their curves at a spacing tied to the glow width and measure distances to line segments (`_sample_by_length`, `_segment_box`), so curve drawing grows with the pixel count. This fixed the torus issue Greptile raised on PR #1. Their remaining cost is per-pixel work (background noise in torus).
- **Brush, trail and curve styles** (torus, lissajous, constellations, ink, flowfield, comet, ember) accumulate in a fixed order, and float addition order changes results. Rust stamps sequentially in exactly Python's order into a shared buffer and then shades in parallel (`core/src/styles/brush.rs`). Python still samples the torus and lissajous curves and picks the constellation pairs in the scene builder; Rust does the per-pixel and per-stamp work. Parallelizing the stamping (for example one band of rows per thread, each replaying all stamps in order) is possible if `ink` at very large sizes matters.
- **Scene builders:** a native style needs a `_x_scene()` returning plain data (numbers, tuples, lists). A scene must reproduce the exact random draws, including any that depend on the palette: `orbits` takes the stop count because `randrange(n_stops)` consumes a number of random bits that depends on `n_stops`.
- **Python numeric semantics in ports:** `x ** n` with a float base is libm `pow`, so use `powf` (not `powi`, which multiplies repeatedly and can differ in the last bit). `round()` is half-to-even (`round_ties_even`), `int()` truncates (`as i64`, fine for non-negative values), and sorting `(distance, value)` tuples breaks distance ties by value (see `Voronoi`).
- **Python `sum()` is compensated since 3.12.** Rust must use `geometry::py_sum` wherever Python sums floats with `sum()` (currently `render_geo`), and `geometry::py_mod` for Python's float `%`.
- **PNG encoding:** `_packed_rgb` has a fast `bytes()` path for in-range integer channels (about 0.03 s at 640×480), and `zlib` level 9 compression dominates the encoding time at 4K.
- **No resource guard:** there are no limits on `--width`, `--height` or `--frames`, and the CLI holds all animation frames in memory before writing them.

### Rendering quirks (not bugs, but worth knowing)

- **Mandelbrot:** `mandelbrot` ignores the seed by design, so `--frames N` produces identical frames.
- **Animation:** consecutive seeds are rendered as unrelated frames, a slideshow rather than continuous motion. A shared time parameter would be needed for real animation.
- **Comet length:** `render_comet` scales comet length by `width` only, so trails are short on tall canvases.
- **Cosmic arm:** the arm in `render_cosmic` is placed along a straight ray (`dist_along` formula), so despite the docstring it is not really curved.
- **Pixel-based sizes:** some features are still measured in pixels: star dots in `orbits`, `nebula` and `constellations`; brush and ember widths in `ember` and `comet`; and pixel-scaled noise in `waves`, `orbits`, `interference`, `bloom`, `torus`, `pinwheel`, `galaxy` and `quasar`. Their detail changes with resolution.
- **Noise octaves:** `fbm()` uses the same seed for every octave, so the octaves are correlated at the lattice points. Changing this would alter every noise-based style.
