# AGENTS.md

Guidance for coding agents working in this repository. User-facing documentation lives in [README.md](README.md).

## Project overview

`abstract_art.py` is a single-file, standard-library-only generative art tool: 36 styles, 9 palettes, and PNG/APNG writers, plus a CLI. `test_abstract_art.py` holds the `unittest` suite. There are no third-party dependencies; keep it that way unless the user explicitly agrees otherwise.

## Commands

```bash
python3 -m unittest -q                     # full suite (~10 s)
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

Everything is pure Python with per-pixel loops. The slowest styles take about 1–2 s at 128×128, which works out to minutes at 1024×1024. The plan is to profile first and make the cheap Python fixes. Rust via PyO3 is the fallback if that is not enough; it would keep the Python API and CLI unchanged.

- **`render_torus` does far too much work on large canvases** (from the Greptile review of PR #1). Both the sample count (`max(2200, 4 * (width + height))`) and the glow radius grow with canvas size, so doubling both dimensions makes stamping about 8× slower, and neighbouring samples repaint the same pixels. Fix: keep the canvas-relative glow width, but compute each pixel's distance to the curve once (for example, stamp the minimum distance, as `_curve_distance_grid` does, then apply the glow per pixel).
- **`render_quasar`** became slower (about 1.3 s to 1.8 s at 128²) after its noise was switched to the seam-free `polar_noise()`. It evaluates 3-octave noise once per orbit ring per pixel; that noise could be computed once per pixel or cached by radius.
- **Missing bounding-box culling:** `render_orbits`, `render_nebula`, `render_cosmic` and `render_constellations` test every star at every pixel. `render_geo`, `render_shard` and `render_circuit` measure every polygon edge or segment at every pixel.
- **PNG encoding** builds a `bytearray` per pixel in Python. A 256-entry lookup table or `bytes` joins would be faster.
- **No resource guard:** there are no limits on `--width`, `--height` or `--frames`, and the CLI holds all animation frames in memory before writing them.

### Rendering quirks (not bugs, but worth knowing)

- **Mandelbrot:** `mandelbrot` ignores the seed by design, so `--frames N` produces identical frames.
- **Animation:** consecutive seeds are rendered as unrelated frames, a slideshow rather than continuous motion. A shared time parameter would be needed for real animation.
- **Comet length:** `render_comet` scales comet length by `width` only, so trails are short on tall canvases.
- **Cosmic arm:** the arm in `render_cosmic` is placed along a straight ray (`dist_along` formula), so despite the docstring it is not really curved.
- **Pixel-based sizes:** some features are still measured in pixels: star dots in `orbits`, `nebula` and `constellations`; brush and ember widths in `ember` and `comet`; and pixel-scaled noise in `waves`, `orbits`, `interference`, `bloom`, `torus`, `pinwheel`, `galaxy` and `quasar`. Their detail changes with resolution.
- **Lissajous sampling:** `render_lissajous` uses a fixed 1400 samples per curve, which may leave gaps on very large canvases.
- **Noise octaves:** `fbm()` uses the same seed for every octave, so the octaves are correlated at the lattice points. Changing this would alter every noise-based style.
