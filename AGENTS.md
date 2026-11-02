# AGENTS.md

Guidance for coding agents working in this repository. User-facing documentation lives in [README.md](README.md).

## Project overview

`abstract_art.py` is a single-file, standard-library-only generative art tool: 36 styles, 9 palettes, and PNG/APNG writers, plus a CLI. `test_abstract_art.py` holds the `unittest` suite. The pure-Python module needs only the standard library and stays usable on its own as the reference implementation. Third-party dependencies are allowed (the standard-library-only rule has been dropped), and a native Rust core is being added for speed.

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
- **Rust core (`core/`, module `abstract_art_core`):** optional; imported as `_core` (None when missing). Python generates the scene (all `random.Random` use) and passes plain numbers to Rust, which shades rows in parallel (rayon) and returns packed RGB bytes. `NATIVE_STYLES` maps style names to Python wrappers that build the scene and call the core; `render()`/`render_rgb()` use it automatically. Pass seeds through `_seed32()`.
- **Native output must be byte-identical to Python.** Port expressions operation-for-operation in the same order (no algebraic rewrites, no `mul_add`, no `clamp` where Python's `max/min` NaN behavior differs), and keep the Python renderer as the reference. `test_native_styles_match_python_exactly` enforces this; extend its cases when porting a style. Split a style into `_x_scene()` (random layout) and `_x_value()` (per pixel) first, verifying unchanged output with a fingerprint.
- **Packed frames:** `render_rgb()` returns `width * height * 3` bytes; the writers accept rows or packed bytes (`_packed_rgb`). The CLI uses packed frames.
- **Helpers for keeping functions small:** `_shade(width, height, stops, value)` builds rows from a per-pixel `value(x, y)`. `_box_distances()` yields the cells around a point for curve stamping. `_glow_pixels()` maps a flat intensity buffer to rows.

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
- **pythonsecurity:S8707 (path traversal on `--out`):** accepted. A local CLI writes where its user asks.
- **S1172 (`seed` unused in `render_mandelbrot`):** accepted, because the shared renderer signature requires it.

## Known issues and follow-ups

### Performance (the next planned work)

Pure Python takes about 178 s for all 36 styles at 640×480 (see `bench_abstract_art.py`). The Rust core covers the 18 per-pixel styles in `NATIVE_STYLES` (blobs, bloom, circuit, cosmic, drift, galaxy, geo, interference, kaleidoscope, mandala, nebula, orbits, pinwheel, plasma, prism, quasar, shard, waves). They are 78–918× faster, and all 36 styles now take about 25 s. Algorithmic fixes are done in Python first because they carry over to Rust. Single-run timings on this machine vary by about ±30%, so use `--repeat` and compare against a fresh baseline.

- **Culling has been done** for the cases that keep output byte-identical: row filtering (`_RowFilter`) for stars in `orbits`, `nebula`, `cosmic` and `constellations`, and for glyphs in `mandala`; exact-zero ring bands in `mandala`; column bounds in `ember`; and bounding-box pruning of segments in `circuit` (`_nearest_segment`). A skip is allowed only where a contribution is exactly zero, either from a hard cutoff or because `exp()` underflows past `GAUSSIAN_ZERO` standard deviations.
- **`render_torus` and `render_lissajous`** now sample their curves at a spacing tied to the glow width and measure distances to line segments (`_sample_by_length`, `_segment_box`), so curve drawing grows with the pixel count. This fixed the torus issue Greptile raised on PR #1. Their remaining cost is per-pixel work (background noise in torus).
- **Remaining Python styles, now the bulk of the time:**
  - Per-pixel styles, straightforward to port next: crystal (computes a height field, then gradients), voronoi (sorts site distances per pixel), cosmos, mandelbrot, aurora, spiral, petals, starburst, rings, gems, horizon.
  - Brush-and-trail styles that accumulate stamps in a fixed order: torus and lissajous (stamping plus per-pixel shading), constellations (lines plus stars), ink, flowfield, comet, ember. They need either sequential stamping in Rust or a per-pixel gather to stay byte-identical, because floating-point addition order changes results.
- **Scene builders:** a native style needs a `_x_scene()` returning plain data (numbers, tuples, lists). A scene must reproduce the exact random draws, including any that depend on the palette: `orbits` takes the stop count because `randrange(n_stops)` consumes a number of random bits that depends on `n_stops`.
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
