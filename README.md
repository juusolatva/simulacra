# Simulacra

[![Tests](https://github.com/juusolatva/simulacra/actions/workflows/tests.yml/badge.svg?branch=main)](https://github.com/juusolatva/simulacra/actions/workflows/tests.yml)
[![Quality Gate Status](https://sonarcloud.io/api/project_badges/measure?project=juusolatva_abstract-art&metric=alert_status)](https://sonarcloud.io/summary/new_code?id=juusolatva_abstract-art)
[![Coverage](https://sonarcloud.io/api/project_badges/measure?project=juusolatva_abstract-art&metric=coverage)](https://sonarcloud.io/summary/overall?id=juusolatva_abstract-art)
[![License: MIT](https://img.shields.io/github/license/juusolatva/simulacra)](LICENSE)

Generate images and seamless animations from mathematical patterns, noise fields, simulated physical processes, and seeded randomness. Some styles are purely abstract (plasma, op art, strange attractors); others are stylized representations of real phenomena, such as a lunar surface, a black hole, a supernova remnant, paper marbling, or Chladni figures. `simulacra` is a single command-line program written in Rust, with **48 styles**, **24 palettes**, static PNG output, and looping animated output as APNG or GIF. It renders in parallel on every processor core.

The project started as a Python script produced by an LLM running in a sandbox loop. It was corrected, tested and then ported to Rust pixel for pixel; the Python reference was retired once the Rust program matched it (see [`PLAN.md`](PLAN.md)).

See the [glossary](GLOSSARY.md) for the project's visual concepts and terminology.

## Contents

- [Installing](#installing)
- [Quick start](#quick-start)
- [Command-line options](#command-line-options)
- [Examples](#examples)
- [Styles](#styles)
- [Palettes](#palettes)
- [Animation](#animation)
- [How rendering works](#how-rendering-works)
- [Performance and limits](#performance-and-limits)
- [Testing](#testing)
- [Adding a style](#adding-a-style)
- [Troubleshooting](#troubleshooting)
- [Project files](#project-files)

## Installing

Install it with Cargo, straight from this repository (it is not published on crates.io):

```bash
cargo install --locked --features portable --git https://github.com/juusolatva/simulacra simulacra_cli
```

From a clone, `cargo install --locked --features portable --path core/cli` does the same. Cargo puts `simulacra` (`simulacra.exe` on Windows) in `~/.cargo/bin`, which the Rust installer adds to `PATH`.

- **Rust:** install a recent stable toolchain with [rustup](https://rustup.rs); run `rustup update` first if an older one fails to build (the crates use the 2024 edition, and CI builds with the latest stable release).
- **Linux:** Rust needs a C linker; on Debian and Ubuntu, `sudo apt install build-essential`.
- **Windows:** Rust needs the Microsoft C++ build tools, which rustup offers to install; follow its prompts.
- **macOS:** not a tested target; the build may work.
- **Options:** `--locked` builds with the dependency versions in the repository's `Cargo.lock`, the ones CI tests. `--features portable` uses the pure-Rust math library, so every platform renders exactly the same image (the committed golden hashes hold for these builds). Without it, the program uses the platform's math library, which is somewhat faster but can differ slightly in two styles: `cymatics` by at most one level in rare pixels, and `attractor`, whose chaotic orbit takes another path through the same curves.
- **Updating and removing:** run the same command again to update to the latest commit on `main` (Cargo says the package is already installed if nothing changed), and `cargo uninstall simulacra_cli` to remove it.

`simulacra --version` prints the crate version, which does not change between commits yet. To build without installing, run `cargo build --release --manifest-path core/Cargo.toml -p simulacra_cli --features portable` and use `core/target/release/simulacra`. No prebuilt binaries are published yet; CI builds and checks them for Linux (x86_64, aarch64) and Windows (x86_64), see [`PLAN.md`](PLAN.md), Phases 3 and 6.

## Quick start

```bash
simulacra
```

This writes `art.png` in the current directory using the defaults: a 256 × 256 image, the `waves` style, seed `1`, and the `deep` palette.

Choose a style and palette, or write an animation:

```bash
simulacra --style petals --palette sunset --seed 7 --out petals.png
simulacra --style mandelbrot --frames 48 --fps 24 --out zoom.png
simulacra --style plasma --frames 36 --out plasma.gif
```

See all options with `simulacra --help`, and the names with `simulacra --list-styles` and `--list-palettes`.

## Command-line options

| Option | Default | Meaning |
| --- | --- | --- |
| `--style` | `waves` | One of the 48 style names listed below. |
| `--seed` | `1` | Integer controlling seeded randomness and noise: a 64-bit signed integer, negative values included. |
| `--random-seed` | off | Pick a random seed instead and print it in the output line, so the image can be reproduced with `--seed`. Cannot be combined with `--seed`. |
| `--palette` | `deep` | One of the 24 palette names listed below. |
| `--palette-stops` | — | A custom palette instead of a named one: two or more comma-separated hex colors from dark to light, such as `0c061c,5814a8,ff8800`. Colors are `rrggbb` or `rgb`, with an optional `#`. Quote the value if it contains `#`, which a shell treats as a comment otherwise. Cannot be combined with `--palette`. |
| `--width` | `256` | Output width in pixels; must be a positive integer. |
| `--height` | `256` | Output height in pixels; must be a positive integer. |
| `--out` | `art.png` | Destination path. Its parent directory must already exist. |
| `--frames` | `1` | Positive frame count. `1` writes a still image; larger values write an animation (APNG, or GIF if `--out` ends in `.gif`). Every style animates one scene in a seamless loop (see [Animation](#animation)). |
| `--phase` | `0` | Loop position; `1.0` repeats `0.0`. For a still image it picks the moment; with `--frames` it is the first frame's position. |
| `--slideshow` | off | Make every frame an unrelated scene with consecutive seeds instead of one moving scene. |
| `--fps` | `12` | Animation frame rate, an integer from 1 to 65,535. Only used with `--frames` greater than 1. GIF delays are whole hundredths of a second, so GIF rates are rounded and capped at 50. |
| `--no-limits` | off | Skip the size limits (see [Performance and limits](#performance-and-limits)). |
| `--list-styles`, `--list-palettes` | — | Print the style or palette names in alphabetical order, one per line, and exit. |
| `-h`, `--help`, `-V`, `--version` | — | Print usage or the version, then exit. |

Names are case-sensitive. Options can appear in any order and must be spelled out (`--wid` does not abbreviate `--width`); a repeated option takes its last value.

A `--out` ending in `.gif` (any case) writes a looping GIF, still or animated. Any other name writes PNG data: `--frames` decides between a static PNG and an APNG, so use `.png` or `.apng` for clarity, and a `.jpg` filename still gets PNG data. A GIF holds 256 colors, so all frames share one table of 240 colors sampled evenly along the palette plus 16 greys for glows; gradients can show slight banding, and the APNG is the higher-quality choice.

A successful run prints the destination, style, seed, and palette, and replaces an existing destination file. Refused arguments and limits print an error and exit with status 2 before anything is rendered; a failure while rendering or writing, such as an `--out` in a missing directory, prints `simulacra: error: …` and exits with status 1.

## Examples

### A flower scene

```bash
simulacra --style petals --palette sunset --seed 7 --width 512 --height 512 --out flower.png
```

### A Mandelbrot image

```bash
simulacra --style mandelbrot --palette ultraviolet --width 512 --height 512 --out mandelbrot.png
```

A still Mandelbrot image uses a fixed viewport that preserves the complex plane's aspect ratio. The renderer does not use the seed, so changing `--seed` will not change this style. With `--frames` it zooms into the seahorse valley and back out.

### A wide landscape

```bash
simulacra --style horizon --palette forest --seed 12 --width 960 --height 360 --out mountains.png
```

### Geometric outlines and flowing strokes

```bash
simulacra --style geo --palette gold --seed 3 --out geometry.png
simulacra --style ink --palette lagoon --seed 17 --out ink.png
```

### Woodblock landscapes in watercolor colors

```bash
simulacra --style woodblock --palette watercolor --seed 5 --out woodblock.png
simulacra --style woodblock --palette watercolor --seed 7 --width 640 --height 480 --frames 48 --fps 12 --out woodblock.gif
```

`watercolor` is a color palette usable with every style. In `woodblock`, the last palette stop supplies the paper and the other stops supply the printed forms and carved marks; the contours remain crisp.

### An animated sequence

```bash
simulacra --style rings --palette rainbow --seed 3 --width 256 --height 256 --frames 24 --fps 12 --out rings.png
```

This creates a two-second sequence that loops seamlessly: one scene seen at 24 positions along a loop. With `--slideshow`, each frame is instead an unrelated scene with the next seed.

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

Each style takes a phase, the position in a loop from 0 up to 1, where 1.0 looks the same as 0.0. With `--frames N`, the program renders one scene at phases `0`, `1/N`, …, `(N - 1)/N` (starting at `--phase` if given), so the animation loops without a visible seam. The frame rate is 12 FPS unless you pass `--fps`, so one loop lasts `N / fps` seconds.

Every style moves. The motion is a rotation, a drift of waves, a circling of clouds, cells, stars or brush strokes, a sweep of jets, a slow zoom (`mandelbrot` dives toward a spiral in the seahorse valley, then back out), or noise that keeps changing shape, depending on the style; `mosaic` steps its patterns along whole tiles. `woodblock` rolls its wave layers and reshapes its hills and cliffs.

Pass `--slideshow` to render seeds `seed`, `seed + 1`, …, `seed + N - 1` instead, a slideshow of unrelated scenes.

- The phase does not change the scene: the same seed always gives the same objects, and phase 0 gives the same still image as a run without `--phase`.
- `mandelbrot` ignores the seed, so its slideshow frames are identical; its normal animation (the zoom) does not depend on the seed either.
- The viewer must support APNG to show a PNG animation. A viewer without animation support may display only the first frame.

Frames are rendered and encoded several at a time, about one per processor thread, and written to the output in order as they finish, so memory depends on the frames in progress rather than the frame count (at 1080p on 12 threads most styles peak around 100 MB). Every file (PNG, APNG and GIF) is written to a temporary file in the destination directory and replaces the destination only after everything has been written; a failure keeps an existing output and removes the temporary file. The finished file gets the usual permissions for a new file under the current umask (typically `0644`). The destination directory must allow creating temporary files and replacing the output.

## How rendering works

1. **Build the scene.** A style's scene builder makes all its random choices from the seed: positions, sizes, parameters, curves. The phase moves the scene along its loop without new random draws.
2. **Shade the pixels.** The style's shader computes an intensity per pixel from coordinates, noise, distances to shapes, curve samples, or accumulated brush strokes, in parallel across rows. Brush and curve styles stamp their strokes in a fixed order first, so the result does not depend on the thread count.
3. **Map to a palette.** The intensity, clamped to `[0, 1]`, is interpolated between the palette's RGB stops.
4. **Encode the image.** The PNG writer stores the 8-bit RGB samples unchanged, with filter type 0 and `zlib` compression; APNG adds frame control chunks, and GIF maps each pixel to the nearest entry of its shared color table.

The noise helpers use interpolated lattice values and fractal Brownian motion (fBm): several noise layers combined at increasing frequencies and decreasing amplitudes, with values in `[0, 1]` centered near 0.5. Angle-based textures sample the noise around a circle so they have no seam at ±π, and animated noise blends a looping stack of layers so the last frame leads back into the first.

Palette stops are sRGB colors, and the writers store samples as given, with no transfer-function conversion. The output does not embed an explicit color profile or sRGB chunk.

The same inputs give the same image on every platform with a `portable` build, which the committed golden hashes check. Changes to rendering algorithms can change the output for an existing seed; saved seeds are not a guarantee of identical images across versions.

## Performance and limits

Every style renders on all processor cores; at 640 × 480 most styles take a few milliseconds to shade, and the slowest about 0.15 s (measured on 8 CPUs; see "Stage baseline" in [`PLAN.md`](PLAN.md)). There is no GPU backend or progress indicator.

To measure scene building and shading per style, run the engine's benchmark (add `--features portable` to measure the portable math):

```bash
cargo run --release --manifest-path core/Cargo.toml -p simulacra_engine --example bench -- --width 1920 --height 1080 --repeat 3
```

It prints the best of the repeats in milliseconds for every style (or `--styles a,b`), and takes `--phase`, `--seed` and `--palette`. Time encoding and whole runs by running the program, with [hyperfine](https://github.com/sharkdp/hyperfine) or a shell loop:

```bash
hyperfine --warmup 1 'simulacra --style ink --width 1920 --height 1080 --out ink.png'
time (for i in 1 2 3; do simulacra --style plasma --frames 12 --out plasma.png; done)
```

The program refuses, before rendering, a frame over 100 million pixels or with a side over 100,000 pixels, more than 2³² pixels across all frames (about 2,070 frames at 1920 × 1080, or 517 at 3840 × 2160) and more than 65,536 frames, since each frame has a fixed cost however small it is. Together these bound run time and file size; `--no-limits` skips them. Format limits apply even then: PNG sides up to 4,294,967,295, GIF sides up to 65,535, and at most 2,147,483,648 APNG frames.

- Start with 128 × 128 or 256 × 256 previews before increasing resolution.
- Doubling width and height produces four times as many pixels. Runtime often rises accordingly, but curve sampling, brush sizes, and per-style calculations can change the scaling.
- Features still based on fixed pixel sizes, such as star dots and some brush widths, become relatively small at large resolutions. Very small images can lose detail through sampling and color quantization.

Output is PNG, APNG or GIF only. There is no JPEG, SVG, or video export, and no built-in viewer.

## Testing

Everything runs with Cargo from the repository root:

```bash
cargo test --manifest-path core/Cargo.toml
cargo clippy --manifest-path core/Cargo.toml --all-targets -- -D warnings
```

`cargo test` covers the engine's unit tests (noise, palettes, the CPython `hypot` and random-generator ports checked against CPython's own results, scene validation), invariants every style keeps (`core/engine/tests/invariants.rs`: a wide palette range, motion and seamless loops, huge phases, tiny canvases, the work limit up to 4K, unaliased waves), and the program's behavior (`core/cli/tests/cli.rs`: options, messages, exit statuses, failed writes, and the decoded PNG, APNG and GIF output).

Two sets of committed hashes pin the exact images of a `portable` build:

- **Portable fixtures** (`core/engine/tests/fixtures/`): one scene per style at 48 × 36 with the hash of its image. `cargo test --manifest-path core/Cargo.toml -p simulacra_engine --features json,portable` renders them and also builds each scene from scratch. After a deliberate change to a style, update them with the ignored `bless_fixtures` test, run on its own: `cargo test --manifest-path core/Cargo.toml -p simulacra_engine --features json,portable --test fixtures -- --ignored --exact bless_fixtures`.
- **Golden hashes** (`core/cli/tests/golden/`): 343 renders of the program (every style at several sizes, seeds, phases and palettes, APNG and GIF animations), with each file's hash in `SHA256SUMS` and each decoded frame's in `PIXELS`. Build a portable binary and check it:

  ```bash
  cargo build --release --manifest-path core/Cargo.toml -p simulacra_cli --features portable
  sh core/cli/tests/golden.sh check core/target/release/simulacra --keep "$DIR"
  SIMULACRA_GOLDEN_OUT="$DIR" cargo test --release --manifest-path core/Cargo.toml -p simulacra_cli --features portable --test golden_pixels -- --ignored
  ```

  `$DIR` must be empty or missing. `golden.sh bless` rewrites `SHA256SUMS`, and `SIMULACRA_BLESS=1` on the pixel test rewrites `PIXELS` (it refuses an incomplete directory). A change that alters file bytes but not images, such as a compression library update, re-blesses `SHA256SUMS` only, after the pixel test passes against the unchanged `PIXELS`; a deliberate visual change re-blesses both. `sh core/cli/tests/golden_selftest.sh` tests the checker itself.

`core/coverage.sh` collects line coverage the way CI does (it needs [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) and `rustup component add llvm-tools-preview`) and writes `lcov.info`; `core/check_lcov.sh` fails if the program's or the shaders' coverage went missing.

These checks catch structural and selected visual regressions; they do not establish that every style looks good for every seed, palette, and size. Inspect rendered images when changing drawing algorithms. If FFmpeg is installed, it can check a generated animation with an independent decoder:

```bash
ffprobe -v error -count_frames -show_entries stream=codec_name,width,height,nb_read_frames -of json rings.png
ffmpeg -v error -i rings.png -f null -
```

## Adding a style

No styles are being added for now; this is the outline for when that changes. A style lives in the engine (`core/engine/src/styles/`):

1. Write a scene struct with a `Style` implementation: `render` shades the pixels (call `exp`, `sin`, `cos` and the other libm functions through the `Libm` trait in `core/engine/src/math.rs`, as `x.m_sin()`; Clippy rejects the `f64` methods), and `validate` range-checks every number, checks indices and lengths, and estimates the work (see `core/engine/src/validate.rs`).
2. Implement `BuildScene::build` for it, drawing all random choices through `PyRandom` from the seed, and moving the scene with the phase without new draws so phase 0 is the still image and phase 1 repeats it.
3. Add one line to `for_each_style!` in `core/engine/src/lib.rs` and one to `scene_builders!` in `core/engine/src/scenes.rs`.
4. Bless its portable fixture (`bless_fixtures`, above) and its golden hashes; its `{style}` workloads fail until then.
5. Describe it in the style table above and in `GLOSSARY.md` if it brings a new term, and inspect square, wide and tall renders.

Express sizes, radii, frequencies and noise scales relative to the canvas, keep wave frequencies well below one cycle per two pixels, and make the style use more than half of its palette at 64 × 64 (`invariants.rs` checks all three kinds of property). See `AGENTS.md` for the conventions.

## Troubleshooting

| Symptom | What to check |
| --- | --- |
| `simulacra` is not found | Check that `~/.cargo/bin` is on `PATH`, or run the binary by its full path. |
| `invalid value` for `--style` or `--palette` | Names are lowercase and case-sensitive; use `--list-styles` and `--list-palettes`. |
| Width, height, or frame-count error | Supply positive integers within the limits, or pass `--no-limits`. |
| `cannot write …: No such file or directory` | Create the parent directory before rendering; the program does not create it. |
| `cannot write …: Permission denied` | Choose a writable location. Writing also needs permission to create a temporary file and replace the destination. |
| A render takes a long time without printing anything | There is no progress display. Reduce dimensions or frame count for a preview. |
| An animation appears static | Check APNG support in the viewer, confirm `--frames` is greater than 1, and check `--slideshow` was not set. |
| Frames jump between unrelated scenes | You passed `--slideshow`; drop it to animate one scene. |
| Output is not the format suggested by its filename | Only a `.gif` name changes the format. Any other name gets PNG data, and `--frames` controls whether it includes animation. |

## Project files

| File | Purpose |
| --- | --- |
| [`core/`](core) | Cargo workspace: `engine` (the renderer: scenes, shaders, palettes, noise; rayon) and `cli` (the `simulacra` program, with its PNG, APNG and GIF writers). `clippy.toml` keeps libm calls in `engine/src/math.rs`. |
| [`core/cli/tests/golden/`](core/cli/tests/golden) | The golden workloads and their file and pixel hashes; `golden.sh` checks and blesses them. |
| [`core/engine/tests/fixtures/`](core/engine/tests/fixtures) | One portable scene per style with the hash of its image. |
| [`core/engine/examples/bench.rs`](core/engine/examples/bench.rs) | Per-style benchmark of scene building and shading. |
| `core/coverage.sh`, `core/check_lcov.sh` | Line coverage as CI collects it, and its safeguards. |
| [`GLOSSARY.md`](GLOSSARY.md) | The project's visual vocabulary: style, palette, seed, scene, frame, phase, animation. |
| [`.github/workflows/`](.github/workflows) | CI: `cargo test` and Clippy on both math backends and the golden check; release builds for Linux and Windows checked against the golden hashes; SonarQube Cloud analysis with Rust coverage; OSV-Scanner. |
| `sonar-project.properties` | SonarQube Cloud project settings. |
| [`PLAN.md`](PLAN.md) | The plan for the Rust application: phases, measured drift, compatibility policy and performance baselines, including the Python era. |
| [`AGENTS.md`](AGENTS.md) | Conventions, triage notes, and known issues for coding agents and contributors. |
| [`CLAUDE.md`](CLAUDE.md) | Points Claude Code at `AGENTS.md`. |
| [`LICENSE`](LICENSE) | MIT license. |
