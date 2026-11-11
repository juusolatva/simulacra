# Simulacra

[![Tests](https://github.com/juusolatva/simulacra/actions/workflows/tests.yml/badge.svg?branch=main)](https://github.com/juusolatva/simulacra/actions/workflows/tests.yml)
[![Quality Gate Status](https://sonarcloud.io/api/project_badges/measure?project=juusolatva_abstract-art&metric=alert_status)](https://sonarcloud.io/summary/new_code?id=juusolatva_abstract-art)
[![Coverage](https://sonarcloud.io/api/project_badges/measure?project=juusolatva_abstract-art&metric=coverage)](https://sonarcloud.io/summary/overall?id=juusolatva_abstract-art)
[![License: MIT](https://img.shields.io/github/license/juusolatva/simulacra)](LICENSE)

Generate images and seamless animations from mathematical patterns, noise fields, simulated physical processes, and seeded randomness. Some styles are purely abstract (plasma, op art, strange attractors); others are stylized representations of real phenomena, such as a lunar surface, a black hole, a supernova remnant, paper marbling, or Chladni figures. The script runs on the Python standard library alone, and an optional Rust core makes every style hundreds of times faster. A standalone binary that needs no Python is in preview: it renders every style as a still image or an APNG or GIF animation (see [Standalone binary](#standalone-binary-preview)). It includes **48 styles**, **24 palettes**, static PNG output, and looping animated output as APNG or GIF.

The project started as a script produced by an LLM running in a sandbox loop. The current version includes corrections to rendering geometry, pixel-buffer dimensions, trail sampling, and animation encoding, with regression tests for those behaviors.

See the [glossary](GLOSSARY.md) for the project's visual concepts and terminology.

## Contents

- [Quick start](#quick-start)
- [Command-line options](#command-line-options)
- [Examples](#examples)
- [Styles](#styles)
- [Palettes](#palettes)
- [Animation](#animation)
- [Python API](#python-api)
- [How rendering works](#how-rendering-works)
- [Performance and limitations](#performance-and-limitations)
- [Optional Rust core](#optional-rust-core)
- [Standalone binary (preview)](#standalone-binary-preview)
- [Testing](#testing)
- [Extending the script](#extending-the-script)
- [Troubleshooting](#troubleshooting)
- [Project files](#project-files)

## Quick start

You need Python 3. No third-party packages, installation step, network access, or external image libraries are required to generate images or run the tests. For much faster rendering, build the [optional Rust core](#optional-rust-core). CI runs the test suite on Python 3.12 and 3.14; the Rust core declares Python 3.10 or later.

Run commands from the directory containing `simulacra.py`:

```bash
python3 simulacra.py
```

This writes `art.png` in the current directory using the defaults: a 256 × 256 image, the `waves` style, seed `1`, and the `deep` palette.

Choose a style and palette:

```bash
python3 simulacra.py --style petals --palette sunset --seed 7 --out petals.png
```

See all options:

```bash
python3 simulacra.py --help
```

If your interpreter is named `python` rather than `python3`, substitute that command in the examples.

## Command-line options

| Option | Default | Meaning |
| --- | --- | --- |
| `--style` | `waves` | One of the 48 style names listed below. |
| `--seed` | `1` | Integer controlling seeded randomness and noise. Negative integers are accepted. |
| `--random-seed` | off | Pick a random seed instead and print it in the output line, so the image can be reproduced with `--seed`. Cannot be combined with `--seed`. |
| `--palette` | `deep` | One of the 24 palette names listed below. |
| `--palette-stops` | — | A custom palette instead of a named one: two or more comma-separated hex colors from dark to light, such as `0c061c,5814a8,ff8800`. Colors are `rrggbb` or `rgb`, with an optional `#`. Quote the value if it contains `#`, which a shell treats as a comment otherwise. Cannot be combined with `--palette`. |
| `--width` | `256` | Output width in pixels; must be a positive integer. |
| `--height` | `256` | Output height in pixels; must be a positive integer. |
| `--out` | `art.png` | Destination path. Its parent directory must already exist. |
| `--frames` | `1` | Positive frame count. `1` writes a still image; larger values write an animation (APNG, or GIF if `--out` ends in `.gif`). Every style animates one scene in a seamless loop (see [Animation](#animation)). |
| `--phase` | `0` | Loop position for styles with motion; `1.0` repeats `0.0`. For a still image it picks the moment; with `--frames` it is the first frame's position. |
| `--slideshow` | off | Make every frame an unrelated scene with consecutive seeds, even for styles with motion. |
| `--fps` | `12` | Animation frame rate, an integer from 1 to 65,535. Only used with `--frames` greater than 1. GIF delays are whole hundredths of a second, so GIF rates are rounded and capped at 50. |
| `--no-limits` | off | Skip the size limits (see below). |
| `--list-styles`, `--list-palettes` | — | Print the style or palette names, one per line, and exit. |
| `-h`, `--help` | — | Print usage and available choices, then exit. |

Names are case-sensitive. Options can appear in any order.

The CLI refuses renders over 100 million pixels per frame or with a side over 100,000 pixels, and animations whose frames would need more than 1 GiB of memory in total (`width × height × 3 × frames` bytes). Pass `--no-limits` to override. The limits live in `MAX_FRAME_PIXELS`, `MAX_SIDE`, `MAX_ANIMATION_BYTES` and `check_limits()`; the Python API does not enforce them.

A `--out` ending in `.gif` (any case) writes a looping GIF, still or animated. Any other name writes PNG data: `--frames` decides between a static PNG and an APNG, so use `.png` or `.apng` for clarity, and a `.jpg` filename still gets PNG data. A GIF holds 256 colors, so all frames share one table of 240 colors sampled evenly along the palette plus 16 greys for glows; gradients can show slight banding, and the APNG is the higher-quality choice. The Python API is `write_gif(path, frames, width, height, fps=12, stops=None)`.

A successful run prints the destination, style, seed, and palette. Successful writes replace an existing destination file.

## Examples

### A flower scene

```bash
python3 simulacra.py --style petals --palette sunset --seed 7 --width 512 --height 512 --out flower.png
```

### A Mandelbrot image

```bash
python3 simulacra.py --style mandelbrot --palette ultraviolet --width 512 --height 512 --out mandelbrot.png
```

A still Mandelbrot image uses a fixed viewport that preserves the complex plane's aspect ratio. The renderer does not use the seed, so changing `--seed` will not change this style. With `--frames` it zooms into the seahorse valley and back out.

### A wide landscape

```bash
python3 simulacra.py --style horizon --palette forest --seed 12 --width 960 --height 360 --out mountains.png
```

### Geometric outlines and flowing strokes

```bash
python3 simulacra.py --style geo --palette gold --seed 3 --out geometry.png
python3 simulacra.py --style ink --palette lagoon --seed 17 --out ink.png
```

### Woodblock landscapes in watercolor colors

```bash
python3 simulacra.py --style woodblock --palette watercolor --seed 5 --out woodblock.png
python3 simulacra.py --style woodblock --palette watercolor --seed 7 --width 640 --height 480 --frames 48 --fps 12 --out woodblock.gif
```

`watercolor` is a color palette usable with every style. In `woodblock`, the last palette stop supplies the paper and the other stops supply the printed forms and carved marks; the contours remain crisp.

### An animated sequence

```bash
python3 simulacra.py --style rings --palette rainbow --seed 3 --width 256 --height 256 --frames 24 --fps 12 --out rings.png
```

This creates a two-second sequence that loops seamlessly. `rings` is one of the styles with motion, so the frames are one scene seen at 24 positions along a loop. Every built-in style has motion. With `--slideshow`, each frame is instead an unrelated scene with the next seed.

## Styles

Every style accepts every built-in palette. The descriptions below describe the visual construction. Styles named after real phenomena (astronomical objects, terrain, marbling, standing waves) are stylized representations built from simplified models, not physically accurate simulations.

| Style | Visual construction |
| --- | --- |
| `waves` | Layered sine-wave bands with a noise-modulated background. |
| `blobs` | Smooth fractal value noise mapped through a palette. |
| `orbits` | Concentric ring bands and white stars over a noise background. |
| `mandelbrot` | Escape-time rendering of the Mandelbrot set; the animation zooms into the seahorse valley and back. |
| `gems` | Circular gemstone-inspired forms with banded radial shading. |
| `rings` | Concentric ring bands across the canvas, shaded by a smooth sweep around each circle. |
| `aurora` | Soft, wavy ribbons with Gaussian falloff. |
| `plasma` | Sine plane waves from several directions, warped by noise into flowing cells. |
| `spiral` | Winding polar spiral arms that fade toward the edges. |
| `nebula` | Overlapping radial glow clouds and scattered stars. |
| `kaleidoscope` | A radial noise pattern mirrored across angular sectors. |
| `petals` | Tapered radial flower petals with a bright center. |
| `horizon` | Layered mountain silhouettes against a vertical gradient sky. |
| `starburst` | Narrow rays extending from a glowing central point. |
| `interference` | Plane waves from several directions combined into moiré bands. |
| `constellations` | Glowing stars joined by lines between nearby pairs. |
| `cosmos` | Radially shaded, stepped ripple bands around multiple centers. |
| `galaxy` | A bright central bulge with winding spiral arms. |
| `bloom` | Soft radial glow forms modulated by noise. |
| `flowfield` | Particle trails traced through a smooth noise-driven direction field. |
| `torus` | A projected, glowing torus-knot curve with depth shading over a noise background. |
| `voronoi` | Nearest-site cells with shaded interiors and glowing boundaries. |
| `pinwheel` | Twisting radial arms with a noise background. |
| `lissajous` | Glowing curves formed from combined harmonic oscillations. |
| `geo` | Layered regular polygons with glowing edges. |
| `prism` | Nested polygon facets inside scattered gemstone-inspired forms. |
| `shard` | Irregular polygon shards with halos and crack lines. |
| `circuit` | Glowing right-angled traces routed between node pads. |
| `cosmic` | Star glows over a central cloud and an arm-like feature. |
| `quasar` | Accretion-ring-inspired glows with a central shadow region and bipolar jets. |
| `mandala` | Concentric ring glows, glyph-like marks, and radial spokes. |
| `drift` | Noise fields sampled through a warped coordinate grid. |
| `crystal` | Noise terrain lit as flat facets: normals are snapped to discrete directions and tilts. |
| `ember` | Narrow vertical ember glows with varying brightness and falloff. |
| `comet` | Fading streaks painted with soft brushes; trails wrap across image edges. |
| `ink` | Quadratic Bézier strokes painted with tapered soft brushes. |
| `woodblock` | Stylized layered hills, steep cliffs and rolling waves, with crisp carved contours and palette-colored paper. Seeds choose the landforms and open or cropped compositions; waves roll and land slowly reshapes in a loop. |
| `mosaic` | Glass tiles set in dark grout: a patchwork of checkers, stripes, chevrons, diamond and square rings, lattices, rosettes, mazes and noise smears, often mirrored, sometimes in a ragged shape on a dark ground with scattered sparkles. Inspired by Jan Anderzen's glass mosaic *Palaisinpa kukkana* (*Come Back as a Flower*), 2020, at the Turtola tram stop in Tampere, Finland. |
| `cymatics` | Coupled Chladni standing-wave displacement fields (square or circular modes chosen by the seed), mapping wave troughs, nodal lines, and crests across the palette. |
| `optics` | Unified wave optics combining refractive caustic filaments, thin-film interference fringes, and multi-blade aperture diffraction spikes with Airy rings. |
| `contour` | Topographic map: terraced fbm terrain with thin contour lines and a bold index line on every fifth level. Animates by evolving the terrain. |
| `marbling` | Paper marbling: ink drops and comb strokes are applied in order and undone per pixel, so each pixel is traced back to striped base paint. Drops drift and the combs swing back and forth in a loop. |
| `attractor` | A Clifford strange attractor drawn as a log-scaled density of about six orbit points per pixel (capped at 24 million). Seeds pick one of 512 lively parameter sets (256 in a table, each also mirrored); animation moves the four parameters on small circles. |
| `opart` | Op art: stripes, checkerboard or rings magnified by two or three moving lenses, so the pattern bulges without folding. |
| `lunar` | Lunar surface: overlapping craters with raised rims and central peaks in the large ones, lit by a low sun that casts soft long shadows. Animation swings the sun across the sky. |
| `asteroids` | A tilted asteroid belt: lumpy, tumbling rocks lit from one side, sorted near to far so close ones drift faster, over dust and stars. |
| `blackhole` | A black hole: lensed starfield, a photon ring, and a tilted accretion disk whose far side arches over the top, with Doppler beaming and a turning spiral pattern. |
| `supernova` | A supernova remnant: a knotted filament shell, glowing gas and a pulsing core with diffraction spikes. Animation evolves the filaments and breathes the shell. |

Changing dimensions can change the scene's arrangement as well as the number of pixels. Most shapes, ring radii, and wave frequencies are relative to the canvas, and `blobs`, `nebula`, `drift`, and `crystal` also scale their noise to it. Other noise textures, star sizes, and brush widths are still measured in pixels, so their detail changes with resolution.

## Palettes

Each palette is a list of RGB stops, five or more, starting dark (many styles draw their background from the first stop). The single-hue palettes ramp from dark to light; the multi-hue palettes sweep through several hues. Intermediate colors are obtained by linear interpolation between adjacent stops.

| Palette | Color progression |
| --- | --- |
| `deep` | Dark navy through blue to pale blue. |
| `sunset` | Deep purple through red and coral to orange. |
| `forest` | Dark woodland green through brighter leafy greens. |
| `ember` | Dark brown through rust to warm orange. |
| `ocean` | Deep blue through muted blue and blue-gray. |
| `neon` | Near-black purple through magenta to pink. |
| `gold` | Dark brown through amber to pale gold. |
| `ultraviolet` | Dark violet through purple to lavender. |
| `lagoon` | Dark teal through turquoise to pale mint. |
| `noir` | Near-black through charcoal and slate to silver and paper white. |
| `phosphor` | Vacuum black through CRT phosphor green to bright white-green. |
| `tundra` | Deep peat through spruce, lichen and glacial blue-gray to frost. |
| `rainbow` | Near-black violet through indigo, blue, green, yellow and orange to red (seven stops). |
| `candy` | Dark plum through magenta and pink to aqua and cream. |
| `tropical` | Deep teal through turquoise and lime to hot pink. |
| `jewel` | Near-black through sapphire, emerald and amethyst to gold. |
| `vaporwave` | Dark indigo through violet and magenta to cyan and pale pink. |
| `watercolor` | Muted indigo through mist blue and dusty rose to warm paper. Softens color contrast while keeping the selected style’s edges and texture. |
| `sherbet` | Dark purple through raspberry, coral and orange to cream. |
| `risoprint` | Deep indigo through teal, fluorescent pink and sunflower to warm rice paper. |
| `thermal` | Black through violet and red to orange and pale yellow, like a heat camera. |
| `crimson` | Near-black through oxblood and red to pale rose. |
| `sepia` | Dark umber through brown and tan to cream, like an old photograph. |
| `patina` | Dark bronze through copper to verdigris green and pale mint. |

The `ember` style and the `ember` palette are independent choices. You can use, for example, `--style ember --palette ocean`.

## Animation

Each style takes a `phase`, the position in a loop from 0 up to 1, where 1.0 looks the same as 0.0. With `--frames N`, the CLI renders one scene at phases `0`, `1/N`, …, `(N - 1)/N` (starting at `--phase` if given), so the animation loops without a visible seam. The frame rate is 12 FPS unless you pass `--fps`, so one loop lasts `N / fps` seconds.

Every style moves. The motion is a rotation, a drift of waves, a circling of clouds, cells, stars or brush strokes, a sweep of jets, a slow zoom (`mandelbrot` dives toward a spiral in the seahorse valley, then back out), or noise that keeps changing shape, depending on the style; `mosaic` steps its patterns along whole tiles. `woodblock` rolls its wave layers and reshapes its hills and cliffs. The list is `ANIMATED_STYLES` in the module.

Pass `--slideshow` to render seeds `seed`, `seed + 1`, …, `seed + N - 1` instead, a slideshow of unrelated scenes. (A style added to the registry without motion gets this automatically, and the CLI prints a note.)

- `phase` does not change the scene: the same seed always gives the same objects, and phase 0 gives the same still image as before motion existed.
- `mandelbrot` ignores the seed, so its slideshow frames are identical; its normal animation (the zoom) does not depend on the seed either.
- Use the Python API to choose a different integer frame rate or to supply your own frames. `render(width, height, seed, style, palette_name, phase=0.0)` and `render_rgb(...)` accept `phase`; a style without motion would ignore it.
- The viewer must support APNG to show the animation. A viewer without animation support may display only the first frame.

All writers (PNG, APNG and GIF) use a temporary file in the destination directory and replace the destination only after everything has been written. Encoding or replacement failures preserve an existing output and clean up the temporary file during normal exception handling. The finished file gets the usual permissions for a new file under the current umask (typically `0644`). The destination directory must allow creating temporary files and replacing the output.

## Python API

Import the module from a script in the same directory, or make its directory available on your Python import path. Importing it does not render an image or create files.

### Render and save an image

```python
from pathlib import Path
import simulacra as art

output = Path("output")
output.mkdir(exist_ok=True)

width, height = 320, 240
pixels = art.render(
    width=width,
    height=height,
    seed=7,
    style="voronoi",
    palette_name="lagoon",
)
art.write_png(output / "cells.png", pixels, width, height)
```

`render(width, height, seed, style, palette_name, phase=0.0)` returns a row-major nested list:

```text
pixels[y][x] == (red, green, blue)
```

There are exactly `height` rows and `width` pixels per row. Each rendered pixel is an RGB tuple of integers from 0 to 255. The image is opaque; there is no alpha channel.

The public dispatcher validates dimensions, style, and palette. Invalid dimensions or unknown names raise `ValueError`. Use `render()` rather than individual `render_*()` functions when you want this validation.

For large images, prefer `render_rgb(width, height, seed, style, palette_name)`. It returns the same image as packed `bytes`, row-major with 3 bytes per pixel (`width * height * 3` bytes). That is far smaller than nested tuples, and with the Rust core no per-pixel Python objects are created. Both writers accept either form.

### Write an animation at a custom frame rate

```python
import simulacra as art

width, height = 128, 128
frames = [
    art.render(width, height, seed, "kaleidoscope", "ultraviolet")
    for seed in range(7, 13)
]
art.write_animated_png("patterns.png", frames, width, height, fps=6)
```

`write_animated_png(path, frames, width, height, fps=12)` expects a nonempty sequence of frames with the same dimensions. Pass a list or tuple; the writer uses `len(frames)`, so a generator is not accepted. `fps` must be an integer from 1 to 65,535. Output loops indefinitely, with each frame replacing the complete canvas.

Both image writers accept nested rows or packed RGB bytes and validate their dimensions. Empty animation sequences and invalid frame rates raise `ValueError`. File-system errors, such as a missing output directory or insufficient write permissions, propagate to the caller.

### Custom palettes from hex colors

```bash
python3 simulacra.py --style plasma --palette-stops "#0c061c,#5814a8,#ff8800,#fff0c0" --out custom.png
```

The first color is the background in many styles, so start with a dark one. The output line reports the palette as `custom`. In Python, `parse_hex_palette("#0c061c,5814a8,f80")` returns the same stops as a list of RGB tuples, which you can register as described below.

### Add a palette in Python

```python
import simulacra as art

art.PALETTES["ice"] = [
    (2, 8, 24),
    (12, 38, 72),
    (36, 94, 142),
    (108, 184, 210),
    (220, 248, 255),
]

pixels = art.render(256, 256, 4, "geo", "ice")
art.write_png("ice.png", pixels, 256, 256)
```

Palette entries should be a nonempty list of RGB tuples with channels in the range 0–255. The built-ins have five stops, except `rainbow` with seven; any count of two or more works. Adding an entry at runtime affects that Python process. To make a palette available in subsequent CLI runs, add it to `PALETTES` in `simulacra.py`; CLI palette choices are derived from that dictionary. Add a new built-in palette to the Rust table in `core/engine/src/palettes.rs` as well (the standalone binary reads it); with the Rust core built, `test_native_palettes_match_python` fails until the two match.

## How rendering works

1. **Choose geometry or a field.** A style computes an intensity from coordinates, noise, distances to shapes, curve samples, or accumulated brush strokes.
2. **Map to a palette.** `palette_color()` clamps its scalar input to `[0, 1]` and interpolates RGB stops.
3. **Build a rectangular pixel buffer.** Renderers produce rows of RGB tuples; glow-based renderers first accumulate a flat intensity buffer and then reshape it.
4. **Encode the image.** The writers store the 8-bit RGB samples unchanged, prefix each scanline with PNG filter type 0, compress the samples with `zlib`, and write PNG chunks with CRC checksums.

The noise helpers use interpolated lattice values and fractal Brownian motion (fBm): several noise layers combined at increasing frequencies and decreasing amplitudes. `fbm()` returns values in `[0, 1]` centered near 0.5; `polar_noise()` samples fBm around a circle so angle-based textures have no seam at ±π. Renderers that need randomness create local `random.Random(seed)` instances rather than reseeding Python's global random generator.

Palette stops are sRGB colors, and the writers store samples as given, with no transfer-function conversion. Custom pixel buffers should therefore contain ordinary sRGB values from 0 to 255; out-of-range channels are clamped. The output does not embed an explicit color profile or sRGB chunk.

For a fixed script version and Python environment, the same inputs produce repeatable results. Changes to rendering algorithms can change output for an existing seed; saved seeds are not a guarantee of identical images across versions.

## Performance and limitations

Without the Rust core, rendering runs on the CPU in Python loops: the original 36 styles take about 3 minutes at 640 × 480. With the Rust core, every style is native (the per-pixel ones shade in parallel on all CPU cores, and the brush and curve styles accumulate sequentially to keep the Python result exactly), so all 48 styles take well under a second at 640 × 480 (see [Optional Rust core](#optional-rust-core)). There is no GPU backend or progress indicator. Use `bench_simulacra.py` to measure render times.

- Start with 128 × 128 or 256 × 256 previews before increasing resolution.
- Doubling width and height produces four times as many pixels. Runtime often rises accordingly, but curve sampling, brush sizes, and per-style calculations can change the scaling.
- Styles such as `quasar`, `circuit`, and `drift` perform many noise or geometry calculations per pixel. Actual speed depends on style, size, seed, and hardware.
- `render()` returns Python lists and tuples, which use far more memory than the packed RGB size of `width * height * 3` bytes. The CLI and `render_rgb()` use packed bytes.
- The CLI holds all uncompressed animation frames in memory before writing them, so memory use grows with frame count and pixel count. It refuses renders past the limits above unless you pass `--no-limits`.
- The CLI limits are fixed constants (changing them means editing `MAX_FRAME_PIXELS`, `MAX_SIDE` and `MAX_ANIMATION_BYTES`), and they only estimate the packed frame size. The Python API validates positive dimensions only and has no size cap or memory estimate, so check sizes yourself before calling `render()` on large canvases.
- Features still based on fixed pixel sizes, such as star dots and some brush widths, become relatively small at large resolutions. Very small images can lose detail through sampling and color quantization.

Output is PNG, APNG or GIF only. There is no JPEG, SVG, or video export, and no built-in viewer.

## Optional Rust core

The `core/` directory is a Cargo workspace: `engine` is the renderer, `py` builds `simulacra_core`, a Python extension module made with [PyO3](https://pyo3.rs) and [maturin](https://www.maturin.rs), and `cli` builds the [standalone binary](#standalone-binary-preview). When the extension is importable, `render()`, `render_rgb()`, and the CLI use it automatically for the styles listed in `NATIVE_STYLES`: every style (all 48). When the module is missing, the pure-Python renderers produce the same images.

Python still generates each scene with `random.Random`, so a seed produces the same scene either way. The native output is tested to be byte-identical to the Python output.

Building with `--features portable` (`maturin develop --release -m core/py/Cargo.toml --features portable`) makes the core use the pure-Rust `libm` crate instead of the platform math library, so results are the same on every platform. Released standalone binaries will use it. Its output differs slightly from Python's in two styles: `cymatics` by at most one level in rare pixels, and `attractor`, whose chaotic orbit takes another path through the same curves. Development builds use the default so that native output stays byte-identical to Python. See [`PLAN.md`](PLAN.md) for the measurements.

Build it into a virtual environment (requires a Rust toolchain from [rustup.rs](https://rustup.rs)):

```bash
python3 -m venv .venv
source .venv/bin/activate
pip install maturin
maturin develop --release -m core/py/Cargo.toml
python simulacra.py --style quasar --width 3840 --height 2160 --out quasar-4k.png
```

Rebuild with the same `maturin develop` command after changing Rust code. On an 8-core machine at 640 × 480, native styles take a few milliseconds to about 0.15 s each to shade, against up to 43 s in Python; PNG encoding adds 5–75 ms. `bench_simulacra.py --stages` and `--cli` measure scene building, shading, encoding and peak memory, and the latest baseline is in [`PLAN.md`](PLAN.md).

## Standalone binary (preview)

`core/cli` builds `simulacra`, a command-line tool that renders without Python. It takes the same options as `simulacra.py`, including `--frames`, `--fps` and `--slideshow` for animations, and draws the same pixels for every style (`--list-styles`). A `--out` ending in `.gif` writes a GIF byte for byte identical to the one `simulacra.py` writes. It writes each animation frame to the output as soon as it is rendered, so it keeps only one frame in memory.

```bash
cargo build --release --manifest-path core/Cargo.toml -p simulacra_cli
core/target/release/simulacra --style waves --palette lagoon --seed 17 --width 1280 --height 720 --out waves.png
core/target/release/simulacra --style mandelbrot --frames 48 --fps 24 --out zoom.png
core/target/release/simulacra --style plasma --frames 36 --out plasma.gif
```

Building with `--features portable` gives the same image on every platform, as planned for released binaries; such a build can differ slightly from Python (see [Optional Rust core](#optional-rust-core)). CI builds such test binaries for Linux (x86_64, aarch64) and Windows (x86_64) and checks each one against the committed fixtures with `core/cli/tests/check_release.py`; they are not published yet. See [`PLAN.md`](PLAN.md), Phases 3 and 5, for what comes next.

## Testing

Run the test suite from the project directory:

```bash
python3 -m unittest -v
```

With the Rust core built, run the suite with the virtual environment's interpreter (`.venv/bin/python -m unittest -v`) to also check that native and Python output match; without it, those tests are skipped. Slower checks, a comparison at 200 × 150 and 150 × 200 and a check of every attractor parameter set at 48 phases, run with `SIMULACRA_SLOW=1 .venv/bin/python -m unittest -v -k larger_sizes -k every_attractor_table_entry test_simulacra` (several minutes). Rust unit tests run with `cargo test --manifest-path core/Cargo.toml`; `cargo test --manifest-path core/Cargo.toml -p simulacra_engine --features json,portable` also renders the committed scenes in `core/engine/tests/fixtures/` without Python and checks the hash of each image. With the standalone binary built, `SIMULACRA_CLI=core/target/release/simulacra .venv/bin/python -m unittest -v` also checks that it draws the same pixels and prints the same messages as `simulacra.py`. Run Clippy with `cargo clippy --manifest-path core/Cargo.toml --all-targets -- -D warnings`.

To run one regression test:

```bash
python3 -m unittest -v test_simulacra.RenderingTests.test_large_flowfield_trails_are_connected
```

The Python suite currently contains 89 tests covering:

- Valid dimensions and RGB channels across all registered styles.
- Every style using a wide part of its palette, unaliased wave styles, and shapes that scale with the canvas.
- The noise range and seam-free angular noise.
- Writers that store palette colors unchanged, use normal file permissions without changing the process umask, and preserve existing files on failure.
- A command-line smoke test.
- Packed RGB rendering and writing, and byte-identical output from the Rust core at several sizes, seeds, palettes and phases (with a `portable` core, `attractor` and `cymatics` are held to measured bounds instead).
- Spatial detail in formerly flat renderers and known Mandelbrot interior/exterior points.
- Repeatability without changing global random state.
- Lissajous curve coverage and a single-pixel galaxy edge case.
- Invalid dimensions, style names, palette names, and frame rates.
- Flow-field continuity at larger resolutions and comet continuity on wide canvases.
- Visible quasar-jet contributions.
- APNG chunk order, sequence numbers, timing, CRCs, and decompressed pixel data.
- GIF output: LZW round-trips across table resets, the shared colour table, frame delays, looping and the size limit, all checked with a reference decoder.
- Animation: every style moves with `phase`, loops after one turn, matches its still image at phase 0 and renders identically in Rust and Python at any phase.
- The `mosaic` style: flat tiles, even grout at fractional tile sizes, and native-equals-Python.
- The `attractor` parameter table: entries stay lively and stable at sampled phases, and a seed's choice uses no floating-point math.
- The Rust engine's palette table matching `PALETTES`, and, when `SIMULACRA_CLI` names a built binary, the standalone binary's pixels, messages, list options and errors matching `simulacra.py`.
- Preservation of existing animation output and temporary-file cleanup on failures.

The Rust tests (`cargo test`) add the CPython `hypot` and random-generator ports checked against CPython's own results, the committed `portable` fixtures (one scene per style with the hash of its image, also rendered from the Rust scene builders), the PNG writer and the safe file replacement.

These checks catch structural and selected visual regressions; they do not establish that every style looks good for every seed, palette, and size. Inspect rendered images when changing drawing algorithms.

If FFmpeg is installed separately, you can also check a generated animation with an independent decoder:

```bash
ffprobe -v error -count_frames -show_entries stream=codec_name,width,height,nb_read_frames -of json comets.png
ffmpeg -v error -i comets.png -f null -
```

FFmpeg is optional and is not required by the script or its unit tests.

## Extending the script

To add a style:

1. Implement `render_your_style(width, height, seed, palette_name, phase=0.0)` in `simulacra.py`. A style that does not move can ignore `phase`, but then it is a slideshow under `--frames` until it is added to `ANIMATED_STYLES`.
2. Return exactly `height` rows containing exactly `width` RGB tuples each. Keep channel values between 0 and 255.
3. Use `PALETTES[palette_name]` and `palette_color()` for palette mapping. If randomness is needed, create a local `random.Random(seed)`.
4. Register it in the `STYLES` dictionary. `render()`, the CLI's `--style` choices, and the tests all read from it.
5. Add a description to this README and the module's style list.
6. Run the tests, add a regression check for the new style's intended behavior, and inspect square, wide, and tall renders.
7. Optionally port the per-pixel work to Rust: move the random layout into a `_your_style_scene()` that returns plain data, add a `Style` implementation under `core/engine/src/styles/` that reads it (with the `cfg_attr` line that derives `FromPyObject` under the `python` feature) and validates it (`validate` range-checks every number with `check_numbers!`, checks indices and lengths, and estimates the work; see `core/engine/src/validate.rs`), add one line for it to the registry, `for_each_style!` in `core/engine/src/lib.rs`, which generates both `STYLES` and the adapter's dispatch, and register the scene builder in `NATIVE_STYLES`. Port expressions operation for operation and call `exp`, `sin`, `cos` and the other libm functions through the `Libm` trait in `core/engine/src/math.rs` (`x.m_sin()`); Clippy rejects the `f64` methods (`core/clippy.toml`). `test_native_styles_match_python_exactly` then checks it against the Python renderer. A native style also needs a `portable` fixture: rerun `core/engine/tests/make_fixtures.py` with a `portable` core (see its docstring), or `every_style_has_exactly_one_fixture` fails. For the standalone binary, port the scene builder too, as a `build` function on the scene struct listed in `build_scene` (`core/engine/src/scenes.rs`), drawing through `PyRandom` with only the random methods it supports (see `AGENTS.md`).
8. To make it move, add `_loop(phase, k)` or `_circle_offset()` terms in the scene builder (see `AGENTS.md`), give it an entry in `ANIMATED_STYLES`, and wrap the builder with `_animated_scene()` in `NATIVE_STYLES`. Phase 0 must give the same image as before.

Pay particular attention to coordinate units: normalize positions when a shape should scale with the canvas, distinguish angles from pixel distances, and sample continuous paths densely enough to connect adjacent pixels. Test the saved image as well as the in-memory buffer.

## Troubleshooting

| Symptom | What to check |
| --- | --- |
| `python3` is not found | Check the Python executable installed on your system; it may be named `python`. |
| `invalid choice` | Style and palette names are lowercase and case-sensitive; use `--help` to list valid values. |
| Width, height, or frame-count validation error | Supply positive integers. |
| Output path raises `FileNotFoundError` | Create the parent directory before rendering. The script does not create it. |
| Output path raises `PermissionError` | Choose a writable location. All writers also need permission to create a temporary file and replace the destination. |
| A render takes a long time without printing anything | Rendering has no progress display. Reduce dimensions or frame count for a preview. |
| An animation appears static | Check APNG support in the viewer, confirm `--frames` is greater than 1, and check `--slideshow` was not set. |
| Frames jump between unrelated scenes | You passed `--slideshow`, or the style has no motion (the CLI prints a note). Drop `--slideshow` to animate one scene; see [Animation](#animation). |
| Output is not the format suggested by its filename | Only a `.gif` name changes the format. Any other name gets PNG data, and `--frames` controls whether it includes animation. |

## Project files

| File | Purpose |
| --- | --- |
| [`simulacra.py`](simulacra.py) | Palettes, noise and geometry helpers, all renderers, PNG/APNG/GIF writers, and CLI. |
| [`test_simulacra.py`](test_simulacra.py) | Regression tests (standard library only). |
| [`bench_simulacra.py`](bench_simulacra.py) | Per-style render benchmark with save/compare; `--stages` splits scene, shader and PNG time, and `--cli` times whole CLI runs with peak memory. |
| [`core/`](core) | Cargo workspace: `engine` (the renderer, no Python; rayon), `py` (the optional extension module `simulacra_core`, PyO3) and `cli` (the standalone `simulacra` binary). `clippy.toml` keeps libm calls in `engine/src/math.rs`. |
| [`GLOSSARY.md`](GLOSSARY.md) | The project's visual vocabulary: style, palette, seed, scene, frame, phase, animation. |
| [`.github/workflows/`](.github/workflows) | CI: tests on Python 3.12 and 3.14 with and without the Rust core (default and portable math), `cargo test` and Clippy; SonarQube Cloud analysis with test coverage; OSV-Scanner. |
| `sonar-project.properties`, `.coveragerc` | SonarQube Cloud project settings and the coverage configuration the CI uses. |
| [`.gitignore`](.gitignore) | Excludes Python bytecode, the virtual environment, Rust build output and coverage reports. Generated artwork is not currently ignored. |
| [`README.md`](README.md) | Usage, API, implementation notes, and development guidance. |
| [`PLAN.md`](PLAN.md) | The plan for a standalone Rust application: phases, measured drift, compatibility policy and performance baseline. |
| [`AGENTS.md`](AGENTS.md) | Conventions, triage notes, and known issues for coding agents and contributors. |
| [`CLAUDE.md`](CLAUDE.md) | Points Claude Code at `AGENTS.md`. |
| [`LICENSE`](LICENSE) | MIT license. |
