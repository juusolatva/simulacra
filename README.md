# simulacra

[![Tests](https://github.com/juusolatva/simulacra/actions/workflows/tests.yml/badge.svg?branch=main)](https://github.com/juusolatva/simulacra/actions/workflows/tests.yml)
[![Quality Gate Status](https://sonarcloud.io/api/project_badges/measure?project=juusolatva_abstract-art&metric=alert_status)](https://sonarcloud.io/summary/new_code?id=juusolatva_abstract-art)
[![Coverage](https://sonarcloud.io/api/project_badges/measure?project=juusolatva_abstract-art&metric=coverage)](https://sonarcloud.io/summary/overall?id=juusolatva_abstract-art)
[![License: MIT](https://img.shields.io/github/license/juusolatva/simulacra)](LICENSE)

Generate images and seamless animations from mathematical patterns, noise fields, simulated physical processes, and seeded randomness. Some styles are purely abstract (plasma, op art, strange attractors); others are stylized representations of real phenomena, such as a lunar surface, a black hole, a supernova remnant, paper marbling, or Chladni figures. `simulacra` is a single command-line program written in Rust, with **48 styles**, **24 palettes**, static PNG output, and looping animated output as APNG or GIF. It renders in parallel on every processor core.

## Contents

- [Installing](#installing)
- [Quick start](#quick-start)
- [Command-line options](#command-line-options)
- [Examples](#examples)
- [Styles](#styles)
- [Palettes](#palettes)
- [Animation](#animation)
- [Output and reproducibility](#output-and-reproducibility)
- [Performance and limits](#performance-and-limits)
- [Troubleshooting](#troubleshooting)
- [Development](#development)

## Installing

### Prebuilt binaries

Packages for Linux (x86_64, aarch64) and Windows (x86_64) are on the [Releases](https://github.com/juusolatva/simulacra/releases) page, with a `SHA256SUMS` file. Unpack one and put `simulacra` (`simulacra.exe` on Windows) somewhere on `PATH`. To check that a download was built by this repository's release workflow, run `gh attestation verify FILE --repo juusolatva/simulacra`. macOS is not a target.

### With Cargo

Install from this repository (it is not published on crates.io):

```bash
cargo install --locked --git https://github.com/juusolatva/simulacra simulacra_cli
```

From a clone, `cargo install --locked --path core/cli` does the same. Cargo puts `simulacra` in `~/.cargo/bin`, which the Rust installer adds to `PATH`.

- **Rust:** a recent stable toolchain from [rustup](https://rustup.rs); run `rustup update` if an older one fails to build (the crates use the 2024 edition).
- **Linux:** Rust needs a C linker; on Debian and Ubuntu, `sudo apt install build-essential`.
- **Windows:** Rust needs the Microsoft C++ build tools, which rustup offers to install.
- **`--locked`** builds with the dependency versions in `Cargo.lock`, the ones CI tests.
- **Updating and removing:** run the install command again to update to the latest commit on `main`, and `cargo uninstall simulacra_cli` to remove it.

`simulacra --version` prints the crate version. To build without installing, run `cargo build --release --manifest-path core/Cargo.toml -p simulacra_cli` and use `core/target/release/simulacra`.

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

A `--out` ending in `.gif` (any case) writes a looping GIF, still or animated. Any other name writes PNG data: `--frames` decides between a static PNG and an APNG, so use `.png` or `.apng` for clarity; a `.jpg` filename still gets PNG data. A GIF holds 256 colors, so all frames share one table of 240 colors sampled evenly along the palette plus 16 greys for glows; gradients can show slight banding, and the APNG is the higher-quality choice.

A successful run prints the destination, style, seed, and palette, and replaces an existing destination file. Refused arguments and limits print an error and exit with status 2 before anything is rendered; a failure while rendering or writing, such as an `--out` in a missing directory, prints `simulacra: error: …` and exits with status 1.

## Examples

```bash
# A flower scene
simulacra --style petals --palette sunset --seed 7 --width 512 --height 512 --out flower.png

# A Mandelbrot image (the seed does not change this style; with --frames it zooms into the seahorse valley and back)
simulacra --style mandelbrot --palette ultraviolet --width 512 --height 512 --out mandelbrot.png

# A wide landscape
simulacra --style horizon --palette forest --seed 12 --width 960 --height 360 --out mountains.png

# Geometric outlines and flowing strokes
simulacra --style geo --palette gold --seed 3 --out geometry.png
simulacra --style ink --palette lagoon --seed 17 --out ink.png

# Woodblock landscapes in watercolor colors, still and animated
simulacra --style woodblock --palette watercolor --seed 5 --out woodblock.png
simulacra --style woodblock --palette watercolor --seed 7 --width 640 --height 480 --frames 48 --fps 12 --out woodblock.gif

# A two-second loop: one scene seen at 24 positions
simulacra --style rings --palette rainbow --seed 3 --frames 24 --fps 12 --out rings.png
```

## Styles

Every style accepts every built-in palette. Styles named after real phenomena (astronomical objects, terrain, marbling, standing waves) are stylized representations built from simplified models, not physically accurate simulations.

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
| `woodblock` | Stylized layered hills, steep cliffs and rolling waves, with crisp carved contours and palette-colored paper (the last palette stop). Seeds choose the landforms and open or cropped compositions; waves roll and land slowly reshapes in a loop. |
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

Changing dimensions can change the scene's arrangement as well as the number of pixels. Most shapes, ring radii, and wave frequencies are relative to the canvas, but some noise textures, star sizes, and brush widths are measured in pixels, so their detail changes with resolution.

## Palettes

Each built-in palette is a list of five or more sRGB stops, starting dark (many styles draw their background from the first stop); colors in between are interpolated linearly. The `ember` style and the `ember` palette are independent choices.

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

## Animation

Each style takes a phase, the position in a loop from 0 up to 1, where 1.0 looks the same as 0.0. With `--frames N`, the program renders one scene at phases `0`, `1/N`, …, `(N - 1)/N` (starting at `--phase` if given), so the animation loops without a visible seam; one loop lasts `N / fps` seconds. Every style moves: a rotation, drifting waves, circling clouds, cells, stars or brush strokes, sweeping jets, a slow zoom (`mandelbrot`), or noise that keeps changing shape.

Pass `--slideshow` to render seeds `seed`, `seed + 1`, …, `seed + N - 1` instead, a slideshow of unrelated scenes.

- The phase does not change the scene: the same seed always gives the same objects, and phase 0 gives the same still image as a run without `--phase`.
- `mandelbrot` ignores the seed, so its slideshow frames are identical.
- The viewer must support APNG to show a PNG animation; one without animation support may display only the first frame.

## Output and reproducibility

Every file is written to a temporary file in the destination directory and replaces the destination only after everything has been written; a failure keeps an existing output and removes the temporary file. The finished file gets the usual permissions for a new file under the current umask (typically `0644`), and the destination directory must allow creating temporary files and replacing the output. Animation frames are rendered several at a time, about one per processor thread, and written in order as they finish, so memory depends on the frames in progress rather than the frame count (at 1080p on 12 threads most styles peak around 100 MB).

Palette stops are sRGB colors, and the writers store samples as given, with no transfer-function conversion and no embedded color profile.

The same inputs give the same image on every platform: the program uses a pure-Rust math library rather than the platform's, and committed hashes check this on every release target. A later version may change the image a seed gives if a style's drawing changes.

## Performance and limits

Every style renders on all processor cores; at 640 × 480 most styles take a few milliseconds to shade and the slowest about 0.15 s. There is no GPU backend or progress indicator.

The program refuses, before rendering, a frame over 100 million pixels or with a side over 100,000 pixels, more than 2³² pixels across all frames (about 2,070 frames at 1920 × 1080, or 517 at 3840 × 2160) and more than 65,536 frames. Together these bound run time and file size; `--no-limits` skips them. Format limits apply even then: PNG sides up to 4,294,967,295, GIF sides up to 65,535, and at most 2,147,483,648 APNG frames.

- Start with 128 × 128 or 256 × 256 previews before increasing resolution.
- Doubling width and height produces four times as many pixels; runtime often rises accordingly.
- Features based on fixed pixel sizes, such as star dots and some brush widths, become relatively small at large resolutions.

## Troubleshooting

| Symptom | What to check |
| --- | --- |
| `simulacra` is not found | Check that `~/.cargo/bin` (or wherever you unpacked it) is on `PATH`, or run the binary by its full path. |
| `invalid value` for `--style` or `--palette` | Names are lowercase and case-sensitive; use `--list-styles` and `--list-palettes`. |
| Width, height, or frame-count error | Supply positive integers within the limits, or pass `--no-limits`. |
| `cannot write …: No such file or directory` | Create the parent directory before rendering; the program does not create it. |
| `cannot write …: Permission denied` | Choose a writable location. Writing also needs permission to create a temporary file and replace the destination. |
| A render takes a long time without printing anything | There is no progress display. Reduce dimensions or frame count for a preview. |
| An animation appears static | Check APNG support in the viewer, confirm `--frames` is greater than 1, and check `--slideshow` was not set. |
| Frames jump between unrelated scenes | You passed `--slideshow`; drop it to animate one scene. |
| Output is not the format suggested by its filename | Only a `.gif` name changes the format. Any other name gets PNG data, and `--frames` controls whether it includes animation. |

## Development

| File | Purpose |
| --- | --- |
| [`core/`](core) | Cargo workspace: `engine` (package `simulacra_engine`, the renderer: scenes, shaders, palettes, noise) and `cli` (package `simulacra_cli`, the `simulacra` program with its PNG, APNG and GIF writers). |
| [`core/engine/tests/fixtures/`](core/engine/tests/fixtures) | One scene per style with the hash of its image. |
| [`core/cli/tests/golden/`](core/cli/tests/golden) | The golden workloads and their file and pixel hashes; `golden.sh` checks and blesses them. |
| [`.github/workflows/`](.github/workflows) | CI: tests and Clippy, the golden check, release builds for Linux and Windows, SonarQube Cloud with coverage, OSV-Scanner. |
| [`TESTING.md`](TESTING.md) | Automated tests and the manual checks. |
| [`AGENTS.md`](AGENTS.md) | Architecture, conventions, CI, releasing and known issues, for contributors and coding agents. |
| [`LICENSE`](LICENSE) | MIT license. |
