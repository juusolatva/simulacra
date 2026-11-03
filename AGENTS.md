# AGENTS.md

Guidance for coding agents working in this repository. User-facing documentation lives in [README.md](README.md).

## Project overview

`abstract_art.py` is a generative art tool: 37 styles, 15 palettes, PNG, APNG and GIF writers and a CLI. `test_abstract_art.py` holds the `unittest` suite and `bench_abstract_art.py` the benchmark. The pure-Python module needs only the standard library and stays usable on its own as the reference implementation. `core/` is an optional Rust extension that renders all 37 styles natively. Third-party dependencies are allowed; the standard-library-only rule has been dropped.

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
| Native styles, batch 3 (7 brush and trail): torus, lissajous, constellations, ink, flowfield, comet, ember | Done: sequential stamping in Python's order, parallel shading (`core/src/styles/brush.rs`) | PR #4 |
| CLI safety and polish: size limits, `--fps`, `--random-seed`, `--list-styles`, `--list-palettes` | Done | PR #5 |
| More built-in palettes: rainbow, candy, tropical, jewel, vaporwave, sherbet; custom palettes from the command line (`--palette-stops`, `parse_hex_palette()`) | Done | PR #6 |
| Real animation, batch 1 (13 styles): `phase` parameter, `ANIMATED_STYLES`, `--phase`, `--slideshow`; aurora, galaxy, gems, geo, interference, kaleidoscope, petals, plasma, rings, spiral, starburst, voronoi, waves | Done | PR #7 |
| Real animation, batch 2 (8 styles): looping noise (`fbm(..., phase=)` / `fbm_loop`), `_circle_offset()`; blobs, bloom, cosmos, crystal, drift, horizon, nebula, pinwheel | Done | PR #8 |
| New style `mosaic` (inspired by Jan Anderzen's *Palaisinpa kukkana* / *Come Back as a Flower*, Turtola tram stop, Tampere, 2020): per-tile patterns, animated, native | Done | PR #9 |
| Real animation, batch 3 (6 brush and trail styles): comet, constellations, ember, flowfield, ink, lissajous move by circling positions or turning phases in their scene builders, so Rust is unchanged | Done | PR #11 |
| Real animation, batch 4 (5 styles): torus turns about the vertical axis, quasar's jets sweep around and flare, mandala's rings counter-rotate, orbits' stars circle and rings breathe, cosmic's arm turns | Done | PR #12 |
| Real animation, batch 5 (the last 4): circuit pads orbit, prism layers and shards turn, mandelbrot zooms into the seahorse valley and back. Every style now moves. Mandelbrot is the only Rust change (it takes scale, centre and iteration count from its scene) | Done | PR #13 |
| GIF output (`write_gif`: own LZW encoder, one shared 256-color table sampled from the palette, chosen by a `.gif` `--out`) | Done | PR #10 |
| CI: SonarQube Cloud with coverage (about 96%), OSV-Scanner | Done | PR #14 |
| Further output formats | Not planned: PNG, APNG and GIF are enough | — |

Measured at 640×480: the original 36 styles took 178 s in pure Python at the start (`mosaic` came later). With every style native, a run on a fast machine takes about 0.2 s in total, and about 1.4 s for the seven brush and curve styles at 4K, the slowest being `ink` (0.8 s, sequential stamping). Compare against a fresh baseline, not these numbers.

## Commands

```bash
python3 -m unittest -q                     # full suite, pure Python (native tests skip)
.venv/bin/python -m unittest -q            # with the Rust core: also checks native == Python
source .venv/bin/activate && maturin develop --release -m core/Cargo.toml   # rebuild core
cargo test --manifest-path core/Cargo.toml && cargo clippy --manifest-path core/Cargo.toml --all-targets
python3 bench_abstract_art.py --width 640 --height 480 --compare <saved.json>
python3 abstract_art.py --style ink --palette lagoon --seed 17 --out ink.png
python3 abstract_art.py --help             # lists styles (from STYLES) and palettes
# coverage as CI runs it (pure Python first, then with the core built, appended):
python -m coverage run -m unittest -q && python -m coverage run --append -m unittest -q && python -m coverage report
```

Do not commit generated images.

## CI

- **SonarQube Cloud (`.github/workflows/sonarqube.yml`, `sonar-project.properties`, `.coveragerc`):** runs the suite twice under `coverage` (once without the Rust core, once with it built via `maturin develop`, appended into one report) and scans with `coverage.xml`. Only Python coverage is collected; Rust is exercised through the same tests. The scan needs `SONAR_TOKEN` and skips pull requests from forks, which cannot read secrets. SonarQube's Automatic Analysis must be off, because it conflicts with CI analysis.
- **OSV-Scanner (`.github/workflows/osv-scanner.yml`):** scans `-r ./`, which finds `core/Cargo.lock` (the Python module has no dependencies). Pull requests and merge groups use the PR workflow, and pushes to main, a weekly schedule and manual runs use the full scan. SARIF upload is off (`upload-sarif: false`) while the repository is private, since code scanning needs GitHub Code Security there; turn it back on when the repository is public. Actions are pinned by commit SHA with the version in a comment.

## Architecture

- **Registry:** `STYLES` maps each style name to its `render_*` function. It drives `render()`, the CLI `--style` choices and the tests, and is the only place a style is registered.
- **Renderer contract:** `render_x(width, height, seed, palette_name, phase=0.0)` returns `height` rows of `width` `(r, g, b)` integer tuples (every built-in style takes `phase`; see Animation below). Map intensities through `palette_color(stops, t)` with `t` in `[0, 1]`.
- **Randomness:** use a local `random.Random(seed)`. Never touch the global `random` state; a test enforces this. Output must be deterministic for a given seed, size and palette.
- **Noise:** `fbm()` returns values in `[0, 1]` centered near 0.5. Do **not** treat it as `[-1, 1]` (`n * 0.5 + 0.5` was a past bug in 8 styles). Use `_contrast(n, gain)` to stretch it, and `polar_noise()` for anything indexed by an angle, because raw `theta` creates a seam at ±π.
- **Colors:** palette stops are sRGB, and the writers store samples unchanged. Do not reintroduce a linear-to-sRGB conversion.
- **Writers:** `_atomic_write()` writes to a temporary file created with `os.open(..., 0o666)` and then calls `os.replace()`. Never call `os.umask()`, since it is process-wide and unsafe with threads.
- **Rust core (`core/`, module `abstract_art_core`):** optional; imported as `_core` (None when missing). Python generates the scene (all `random.Random` use) with a `_x_scene()` builder that returns plain data. `NATIVE_STYLES` maps each native style to its builder, and `_core.render(style, width, height, seed, stops, scene)` reads the scene into a per-style Rust struct (`#[derive(FromPyObject)]`, `from_item_all`), shades rows in parallel (rayon) and returns packed RGB bytes. `render()` and `render_rgb()` use it automatically. Pass seeds through `_seed32()`. `_core.STYLES` must match `NATIVE_STYLES` (tested).
- **Native output must be byte-identical to Python.** Port expressions operation-for-operation in the same order (no algebraic rewrites, no `mul_add`, no `clamp` where Python's `max/min` NaN behavior differs), and keep the Python renderer as the reference. `test_native_styles_match_python_exactly` enforces this; extend its cases when porting a style. Move the style's random layout into `_x_scene()` first and have the Python renderer use it, verifying unchanged output with a fingerprint. Rust helpers that replicate Python semantics live in `core/src/geometry.rs` (`py_mod`, `py_sum`, polygon and segment distances) and `core/src/styles/mod.rs` (`gaussian`, `unit`, `center`, `small_side`).
- **Animation (`phase`):** a style moves by adding `_loop(phase, k)` (`tau * k * phase`, `k` a whole number, negative to reverse) to phases and angles in its `_x_scene()`, so the Rust shader needs no change and native stays byte-identical. Keep it free of new random draws (derive per-item values from the index) so scenes at phase 0 match the still image, and give each style an entry in `ANIMATED_STYLES` and `_animated_scene()` in `NATIVE_STYLES`. Offsets that are not whole turns, such as the drift in `voronoi`, are measured from their starting value so phase 0 adds exactly 0.0.
- **Looping noise:** `fbm(..., phase=)` (Rust `fbm_loop`) blends a stack of ordinary 2D noise layers, each the same noise under the seed plus `7919 * layer`. Layer 0 is the real seed, so phase 0 is exactly the still noise, and the layer index wraps, so phase 1 repeats phase 0. Octave `i` steps through `_NOISE_LAYERS + i` layers per loop so the octaves do not all stall or pulse together. The blend is linear on purpose; with an eased blend the speed varied about 3x and hitched at the loop point. A style animates its noise by passing `phase` to `fbm` and putting `"phase"` in its scene for the shader. Circling objects use `_circle_offset()`, whose offset is exactly zero at phase 0.
- **Brush and trail styles** animate by moving their scene data: positions circle with `_circle_offset()` (comet, ember, flowfield starts, ink control points, constellation stars via `_drift_stars()`), and lissajous turns its curve phases. Constellation pairs are chosen from the starting positions so the line set stays constant.
- **Rotation and sweeps:** `torus` rotates its model points with `_turned()` (an exact identity at angle 0) and takes the depth range from the turned knot but the centre and scale from the unturned one; `quasar` adds `_loop()` to the jet's spin angle; `mandala` adds alternating loops to ring and spoke phases; `orbits` and `cosmic` use `_circle_offset()` / `_drift_stars()`. Noise seeded per ring (quasar, mandala) stays fixed because the Rust core owns that noise.
- **Mandelbrot zoom:** `_mandelbrot_scene()` gives the Rust shader `scale`, `center` and `iterations`. The zoom factor is `exp(-depth * ease)` with a cosine ease, so it dives and returns; the centre moves with `1 - closing` so the target stays on screen, and phase 0 is exactly the old view (`closing` is exactly 1.0). Shards use `_turned_about()`, which adds the rotation offset to the point (rebuilding it from the centre would not be exact at angle 0).
- **Animation is complete:** all 37 styles are in `ANIMATED_STYLES`. The slideshow path in `_frame_plan` remains for a future style without motion and for `--slideshow`.
- **Per-tile styles (`mosaic`):** `_mosaic_scene()` computes one palette position per tile on a grid about `_MOSAIC_TILES` tiles across the short side, so the work does not grow with the canvas, and the Rust shader (`core/src/styles/mosaic.rs`) only expands tiles and grout to pixels. Noise there is sampled in tile units, and `value_noise` has an 8-unit cell, so tile-space noise needs scales near 1 rather than the 0.01 used per pixel. Patterns move by whole tiles (`_mosaic_shift`), wrapping after one period per loop.
- **GIF (`write_gif`):** the table is 240 palette samples plus 16 greys, shared by all frames; pixels map to the nearest entry once per 5-bit colour cell (cached across frames), and `_lzw_codes()` is a variable-width LZW encoder whose width growth mirrors the decoder, which lags one entry behind. Delays are whole hundredths of a second (`_gif_delay`). Tests decode the output with a reference decoder in `test_abstract_art.py`.
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
- **S1172 (`seed` unused in `render_mandelbrot`):** accepted, because the shared renderer signature requires it. Other S1172 findings are real: a helper that nothing calls any more (for example `_no_scene` after mandelbrot got a scene) should be deleted.
- **S9073 (composite assertion):** split `assert a and b` into two asserts, including in test helpers.
- **Workflow with PRs:** SonarQube Cloud (CI analysis), Semgrep and Greptile run on every PR; Greptile reviews only when the PR is opened, not on later pushes, so answer its findings with a reply on the thread (the user resolves threads).

## Known issues and follow-ups

Open ideas, none started: turn OSV-Scanner's SARIF upload back on once the repository is public (`upload-sarif` in `osv-scanner.yml`); stream animation frames to the writers instead of holding them all in memory; collect Rust coverage; animate the per-ring turbulence in `quasar` and `mandala`, which needs the Rust noise to take a phase.

### Performance

Pure Python took about 178 s for the original 36 styles at 640×480 (see `bench_abstract_art.py`). The Rust core now covers every style in `NATIVE_STYLES`. Algorithmic fixes were done in Python first because they carry over to Rust. Single-run timings on this machine vary by about ±30%, so use `--repeat` and compare against a fresh baseline.

- **Culling has been done** for the cases that keep output byte-identical: row filtering (`_RowFilter`) for stars in `orbits`, `nebula`, `cosmic` and `constellations`, and for glyphs in `mandala`; exact-zero ring bands in `mandala`; column bounds in `ember`; and bounding-box pruning of segments in `circuit` (`_nearest_segment`). A skip is allowed only where a contribution is exactly zero, either from a hard cutoff or because `exp()` underflows past `GAUSSIAN_ZERO` standard deviations.
- **`render_torus` and `render_lissajous`** now sample their curves at a spacing tied to the glow width and measure distances to line segments (`_sample_by_length`, `_segment_box`), so curve drawing grows with the pixel count. This fixed the torus issue Greptile raised on PR #1. Their remaining cost is per-pixel work (background noise in torus).
- **Brush, trail and curve styles** (torus, lissajous, constellations, ink, flowfield, comet, ember) accumulate in a fixed order, and float addition order changes results. Rust stamps sequentially in exactly Python's order into a shared buffer and then shades in parallel (`core/src/styles/brush.rs`). Python still samples the torus and lissajous curves and picks the constellation pairs in the scene builder; Rust does the per-pixel and per-stamp work. Parallelizing the stamping (for example one band of rows per thread, each replaying all stamps in order) is possible if `ink` at very large sizes matters.
- **Scene builders:** a native style needs a `_x_scene()` returning plain data (numbers, tuples, lists). A scene must reproduce the exact random draws, including any that depend on the palette: `orbits` takes the stop count because `randrange(n_stops)` consumes a number of random bits that depends on `n_stops`.
- **Python numeric semantics in ports:** `x ** n` with a float base is libm `pow`, so use `powf` (not `powi`, which multiplies repeatedly and can differ in the last bit). `round()` is half-to-even (`round_ties_even`), `int()` truncates (`as i64`, fine for non-negative values), and sorting `(distance, value)` tuples breaks distance ties by value (see `Voronoi`).
- **Python `sum()` is compensated since 3.12.** Rust must use `geometry::py_sum` wherever Python sums floats with `sum()` (currently `render_geo`), and `geometry::py_mod` for Python's float `%`.
- **PNG encoding:** `_packed_rgb` has a fast `bytes()` path for in-range integer channels (about 0.03 s at 640×480), and `zlib` level 9 compression dominates the encoding time at 4K.
- **Resource guards:** the CLI enforces `MAX_FRAME_PIXELS` (100 M) and `MAX_ANIMATION_BYTES` (1 GiB of packed frames) through `check_limits()`; `--no-limits` skips them and the Python API does not enforce them. The CLI still holds all animation frames in memory; streaming them to the writer would need `write_animated_png` to accept an iterator with a known count.

### Rendering quirks (not bugs, but worth knowing)

- **Mandelbrot:** `mandelbrot` ignores the seed by design; its animation is a zoom that does not depend on the seed, and only `--slideshow` gives identical frames.
- **Animation:** styles in `ANIMATED_STYLES` move with `phase` (currently every style); a style outside it is a slideshow of consecutive seeds under `--frames` (the CLI prints a note). `phase` 0 must give exactly the still image the style produced before it moved, so check that with fingerprints when adding a style.
- **Comet length:** `render_comet` scales comet length by `width` only, so trails are short on tall canvases.
- **Cosmic arm:** the arm in `render_cosmic` is placed along a straight ray (`dist_along` formula), so despite the docstring it is not really curved.
- **Pixel-based sizes:** some features are still measured in pixels: star dots in `orbits`, `nebula` and `constellations`; brush and ember widths in `ember` and `comet`; and pixel-scaled noise in `waves`, `orbits`, `interference`, `bloom`, `torus`, `pinwheel`, `galaxy` and `quasar`. Their detail changes with resolution.
- **Noise octaves:** `fbm()` uses the same seed for every octave, so the octaves are correlated at the lattice points. Changing this would alter every noise-based style.
