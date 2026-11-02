# Abstract Art

Generate abstract images from mathematical patterns, noise fields, and seeded randomness using only the Python standard library. The script includes **36 styles**, **9 palettes**, static PNG output, and looping animated PNG (APNG) output.

The project started as a script produced by an LLM running in a sandbox loop. The current version includes corrections to rendering geometry, pixel-buffer dimensions, trail sampling, and animation encoding, with regression tests for those behaviors.

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
- [Testing](#testing)
- [Extending the script](#extending-the-script)
- [Troubleshooting](#troubleshooting)
- [Project files](#project-files)

## Quick start

You need Python 3. No third-party packages, installation step, network access, or external image libraries are required to generate images or run the tests. The current test suite has been run with Python 3.14.4; the repository does not declare a minimum supported Python version.

Run commands from the directory containing `abstract_art.py`:

```bash
python3 abstract_art.py
```

This writes `art.png` in the current directory using the defaults: a 256 × 256 image, the `waves` style, seed `1`, and the `deep` palette.

Choose a style and palette:

```bash
python3 abstract_art.py --style petals --palette sunset --seed 7 --out petals.png
```

See all options:

```bash
python3 abstract_art.py --help
```

If your interpreter is named `python` rather than `python3`, substitute that command in the examples.

## Command-line options

| Option | Default | Meaning |
| --- | --- | --- |
| `--style` | `waves` | One of the 36 style names listed below. |
| `--seed` | `1` | Integer controlling seeded randomness and noise. Negative integers are accepted. |
| `--palette` | `deep` | One of the 9 palette names listed below. |
| `--width` | `256` | Output width in pixels; must be a positive integer. |
| `--height` | `256` | Output height in pixels; must be a positive integer. |
| `--out` | `art.png` | Destination path. Its parent directory must already exist. |
| `--frames` | `1` | Positive frame count. `1` writes a static PNG; larger values write APNG. |
| `-h`, `--help` | — | Print usage and available choices, then exit. |

Names are case-sensitive. Options can appear in any order. There is no CLI option for frame rate; CLI animations use 12 frames per second.

The filename extension does not choose the format: `--frames` determines whether the output is static PNG or APNG. Use `.png` or `.apng` for clarity. Supplying a `.jpg` filename still writes PNG data.

A successful run prints the destination, style, seed, and palette. Successful writes replace an existing destination file.

## Examples

### A flower composition

```bash
python3 abstract_art.py --style petals --palette sunset --seed 7 --width 512 --height 512 --out flower.png
```

### A Mandelbrot image

```bash
python3 abstract_art.py --style mandelbrot --palette ultraviolet --width 512 --height 512 --out mandelbrot.png
```

The Mandelbrot viewport is fixed and preserves the complex plane's aspect ratio. Its renderer does not use the seed, so changing `--seed` will not change this style.

### A wide landscape

```bash
python3 abstract_art.py --style horizon --palette forest --seed 12 --width 960 --height 360 --out mountains.png
```

### Geometric outlines and flowing strokes

```bash
python3 abstract_art.py --style geo --palette gold --seed 3 --out geometry.png
python3 abstract_art.py --style ink --palette lagoon --seed 17 --out ink.png
```

### An animated sequence

```bash
python3 abstract_art.py --style comet --palette neon --seed 1 --width 256 --height 256 --frames 12 --out comets.png
```

This creates a one-second sequence that loops indefinitely. It shows independently generated compositions; it does not animate a comet continuously through a shared scene.

## Styles

Every style accepts every built-in palette. The descriptions below describe the visual construction; astronomical names refer to artistic effects rather than physical simulations.

| Style | Visual construction |
| --- | --- |
| `waves` | Layered sine-wave bands with a noise-modulated background. |
| `blobs` | Smooth fractal value noise mapped through a palette. |
| `orbits` | Concentric ring bands and white stars over a noise background. |
| `mandelbrot` | Escape-time rendering of the Mandelbrot set with a fixed viewport. |
| `gems` | Small circular gemstone-inspired forms with radial shading. |
| `rings` | Small concentric ring bands near the image center. |
| `aurora` | Soft, wavy ribbons with Gaussian falloff. |
| `plasma` | Layered sine waves combined with noise to form interference-like textures. |
| `spiral` | Narrow, winding polar spiral bands. |
| `nebula` | Overlapping radial glow clouds and scattered stars. |
| `kaleidoscope` | A radial noise pattern mirrored across angular sectors. |
| `petals` | Tapered radial flower petals with a bright center. |
| `horizon` | Layered mountain silhouettes against a vertical gradient sky. |
| `starburst` | Narrow rays extending from a glowing central point. |
| `interference` | Multiple sine patterns combined into repeating bands. |
| `constellations` | Glowing stars joined by lines between nearby pairs. |
| `cosmos` | Radially shaded, stepped ripple bands around multiple centers. |
| `galaxy` | A bright central bulge with winding spiral arms. |
| `bloom` | Soft radial glow forms modulated by noise. |
| `flowfield` | Particle trails traced through a smooth noise-driven direction field. |
| `torus` | A projected, glowing torus-knot curve over a noise background. |
| `voronoi` | Nearest-site cells with shaded interiors and glowing boundaries. |
| `pinwheel` | Twisting radial arms with a noise background. |
| `lissajous` | Glowing curves formed from combined harmonic oscillations. |
| `geo` | Layered regular polygons with glowing edges. |
| `prism` | Nested polygon facets inside scattered gemstone-inspired forms. |
| `shard` | Irregular polygon shards with halos and crack lines. |
| `circuit` | Glowing line paths connecting scattered node pads. |
| `cosmic` | Star glows over a central cloud and an arm-like feature. |
| `quasar` | Accretion-ring-inspired glows with a central shadow region and bipolar jets. |
| `mandala` | Concentric ring glows, glyph-like marks, and radial spokes. |
| `drift` | Noise fields sampled through a warped coordinate grid. |
| `crystal` | Terrain-inspired shading based on noise gradients and quantized directions. |
| `ember` | Narrow vertical ember glows with varying brightness and falloff. |
| `comet` | Fading streaks painted with soft brushes; trails wrap across image edges. |
| `ink` | Quadratic Bézier strokes painted with tapered soft brushes. |

Changing dimensions can change the composition, not just the number of pixels. Some styles use coordinates relative to the canvas, while others use fixed pixel sizes or noise scales. For example, the small `rings` pattern occupies less of the canvas as resolution increases.

## Palettes

Each palette contains five RGB stops, ordered from dark to light. Intermediate colors are obtained by linear interpolation between adjacent stops.

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

The `ember` style and the `ember` palette are independent choices. You can use, for example, `--style ember --palette ocean`.

## Animation

With `--frames N`, the CLI renders seeds `seed`, `seed + 1`, …, `seed + N - 1`, then writes them as full-canvas frames in a looping APNG. Frame rate is fixed at 12 FPS, so one loop lasts `N / 12` seconds.

Animation currently means a sequence of seeded images:

- Objects are regenerated between frames. There is no persistent scene, shared time parameter, motion interpolation, or crossfade.
- A style such as `mandelbrot` that ignores the seed produces repeated identical frames.
- Use the Python API to choose a different integer frame rate or to supply your own related frames.
- The viewer must support APNG to show the animation. A viewer without animation support may display only the first frame.

The animation writer uses a temporary file in the destination directory, closes it after all frames are encoded, and then replaces the destination. Encoding or replacement failures preserve an existing output and clean up the temporary file during normal exception handling. The destination directory must allow creating temporary files and replacing the output.

The static PNG writer writes directly to the destination after encoding the pixels; it does not use the same temporary-file replacement mechanism.

## Python API

Import the module from a script in the same directory, or make its directory available on your Python import path. Importing it does not render an image or create files.

### Render and save an image

```python
from pathlib import Path
import abstract_art as art

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

`render(width, height, seed, style, palette_name)` returns a row-major nested list:

```text
pixels[y][x] == (red, green, blue)
```

There are exactly `height` rows and `width` pixels per row. Each rendered pixel is an RGB tuple of integers from 0 to 255. The image is opaque; there is no alpha channel.

The public dispatcher validates dimensions, style, and palette. Invalid dimensions or unknown names raise `ValueError`. Use `render()` rather than individual `render_*()` functions when you want this validation.

### Write an animation at a custom frame rate

```python
import abstract_art as art

width, height = 128, 128
frames = [
    art.render(width, height, seed, "kaleidoscope", "ultraviolet")
    for seed in range(7, 13)
]
art.write_animated_png("patterns.png", frames, width, height, fps=6)
```

`write_animated_png(path, frames, width, height, fps=12)` expects a nonempty sequence of frames with the same dimensions. Pass a list or tuple; the writer uses `len(frames)`, so a generator is not accepted. `fps` must be an integer from 1 to 65,535. Output loops indefinitely, with each frame replacing the complete canvas.

Both image writers validate pixel-array dimensions. Empty animation sequences and invalid frame rates raise `ValueError`. File-system errors, such as a missing output directory or insufficient write permissions, propagate to the caller.

### Add a palette in Python

```python
import abstract_art as art

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

Palette entries should be a nonempty list of RGB tuples with channels in the range 0–255. Five stops match the built-ins. Adding an entry at runtime affects that Python process. To make a palette available in subsequent CLI runs, add it to `PALETTES` in `abstract_art.py`; CLI palette choices are derived from that dictionary.

## How rendering works

1. **Choose geometry or a field.** A style computes an intensity from coordinates, noise, distances to shapes, curve samples, or accumulated brush strokes.
2. **Map to a palette.** `palette_color()` clamps its scalar input to `[0, 1]` and interpolates RGB stops.
3. **Build a rectangular pixel buffer.** Renderers produce rows of RGB tuples; glow-based renderers first accumulate a flat intensity buffer and then reshape it.
4. **Encode the image.** The writers apply a linear-to-sRGB transfer function, prefix each scanline with PNG filter type 0, compress the samples with `zlib`, and write PNG chunks with CRC checksums.

The noise helpers use interpolated lattice values and fractal Brownian motion (fBm): several noise layers combined at increasing frequencies and decreasing amplitudes. Renderers that need randomness create local `random.Random(seed)` instances rather than reseeding Python's global random generator.

Both writers apply the same color conversion. Custom pixel buffers passed to the writers are treated as linear RGB values scaled to 0–255; already sRGB-encoded samples would receive an additional conversion. The output uses 8-bit RGB samples and does not embed an explicit color profile or sRGB chunk.

For a fixed script version and Python environment, the same inputs produce repeatable results. Changes to rendering algorithms can change output for an existing seed; saved seeds are not a guarantee of identical images across versions.

## Performance and limitations

Rendering runs on the CPU in Python loops. There is no NumPy acceleration, GPU backend, parallel rendering, or progress indicator.

- Start with 128 × 128 or 256 × 256 previews before increasing resolution.
- Doubling width and height produces four times as many pixels. Runtime often rises accordingly, but curve sampling, brush sizes, and per-style calculations can change the scaling.
- Styles such as `quasar`, `mandala`, and `crystal` perform many noise or geometry calculations per pixel. Actual speed depends on style, size, seed, and hardware.
- Pixel buffers use Python lists and tuples, so memory use is substantially larger than the packed RGB size of `width * height * 3` bytes.
- The CLI holds all uncompressed animation frames in memory before writing them. Memory use grows with frame count and pixel count; keep both modest.
- There is no configurable resource cap or maximum-resolution guard. Positive dimensions are validated, but the script does not estimate whether a render will fit in memory.
- Features based on fixed pixel sizes may become small at large resolutions. Very small images can lose detail through sampling and color quantization.

Output is PNG/APNG only. There is no JPEG, SVG, GIF, or video export, and no built-in viewer.

## Testing

Run the standard-library test suite from the project directory:

```bash
python3 -m unittest -v
```

To run one regression test:

```bash
python3 -m unittest -v test_abstract_art.RenderingTests.test_large_flowfield_trails_are_connected
```

The suite currently contains 14 tests covering:

- Valid dimensions and RGB channels across all dispatched styles.
- Spatial detail in formerly flat renderers and known Mandelbrot interior/exterior points.
- Repeatability without changing global random state.
- Lissajous curve coverage and a single-pixel galaxy edge case.
- Invalid dimensions, style names, palette names, and frame rates.
- Flow-field continuity at larger resolutions and comet continuity on wide canvases.
- Visible quasar-jet contributions.
- APNG chunk order, sequence numbers, timing, CRCs, and decompressed pixel data.
- Preservation of existing animation output and temporary-file cleanup on failures.

These checks catch structural and selected visual regressions; they do not establish that every style looks good for every seed, palette, and size. Inspect rendered images when changing drawing algorithms.

If FFmpeg is installed separately, you can also check a generated animation with an independent decoder:

```bash
ffprobe -v error -count_frames -show_entries stream=codec_name,width,height,nb_read_frames -of json comets.png
ffmpeg -v error -i comets.png -f null -
```

FFmpeg is optional and is not required by the script or its unit tests.

## Extending the script

To add a style:

1. Implement `render_your_style(width, height, seed, palette_name)` in `abstract_art.py`.
2. Return exactly `height` rows containing exactly `width` RGB tuples each. Keep channel values between 0 and 255.
3. Use `PALETTES[palette_name]` and `palette_color()` for palette mapping. If randomness is needed, create a local `random.Random(seed)`.
4. Add a branch to `render()` and add the style name to the CLI's `--style` choices. These are currently maintained separately.
5. Add a description to this README and the module's style list.
6. Run the tests, add a regression check for the new style's intended behavior, and inspect square, wide, and tall renders.

Pay particular attention to coordinate units: normalize positions when a shape should scale with the canvas, distinguish angles from pixel distances, and sample continuous paths densely enough to connect adjacent pixels. Test the saved image as well as the in-memory buffer.

## Troubleshooting

| Symptom | What to check |
| --- | --- |
| `python3` is not found | Check the Python executable installed on your system; it may be named `python`. |
| `invalid choice` | Style and palette names are lowercase and case-sensitive; use `--help` to list valid values. |
| Width, height, or frame-count validation error | Supply positive integers. |
| Output path raises `FileNotFoundError` | Create the parent directory before rendering. The script does not create it. |
| Output path raises `PermissionError` | Choose a writable location. APNG also needs permission to create a temporary file and replace the destination. |
| A render takes a long time without printing anything | Rendering has no progress display. Reduce dimensions or frame count for a preview. |
| An animation appears static | Check APNG support in the viewer, confirm `--frames` is greater than 1, and avoid a seed-independent style such as `mandelbrot` if you want changing frames. |
| Frames jump between unrelated compositions | This is the current seeded-sequence behavior. Continuous motion requires supplying related frames through the Python API or extending a renderer. |
| Output is not the format suggested by its filename | The script always writes PNG data; `--frames` controls whether it includes animation. |

## Project files

| File | Purpose |
| --- | --- |
| [`abstract_art.py`](abstract_art.py) | Palettes, noise and geometry helpers, all renderers, PNG/APNG writers, and CLI. |
| [`test_abstract_art.py`](test_abstract_art.py) | Standard-library regression tests. |
| [`.gitignore`](.gitignore) | Excludes Python bytecode and cache directories. Generated artwork is not currently ignored. |
| [`README.md`](README.md) | Usage, API, implementation notes, and development guidance. |
