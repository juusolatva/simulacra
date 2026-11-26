# Testing

Automated tests run in CI on every pull request (see "CI" in [`AGENTS.md`](AGENTS.md)). The manual checks below cover what they cannot: how the images look, other decoders and viewers, speed, and the downloaded release packages.

Commands run from the repository root. Shorthand used below:

```bash
M="--manifest-path core/Cargo.toml"
B=core/target/release/simulacra   # after cargo build --release $M -p simulacra_cli
```

## Automated tests

```bash
cargo test $M
cargo clippy $M --all-targets -- -D warnings
cargo test $M -p simulacra_engine --features json   # also the portable fixtures and scene validation
cargo clippy $M -p simulacra_engine --all-targets --features json -- -D warnings
```

`fixtures.rs` and `validation.rs` need the engine's `json` feature (scenes load from JSON), so a plain `cargo test` neither runs nor lints them; CI runs both forms.

- **Unit tests:** noise, palettes, scene validation, and the ports of CPython's `hypot` and `random.Random` (`core/engine/tests/py_random.rs` replays the CPython sequences in `random_reference.json`).
- **Invariants** (`core/engine/tests/invariants.rs`): every style uses a wide palette range, moves and loops seamlessly, accepts huge phases and tiny canvases, stays well within the work limit up to 4K, and draws waves below the Nyquist limit.
- **Scene validation** (`core/engine/tests/validation.rs`, `--features json`): every number in every fixture scene set to 1e300 must be refused (except a looping-noise phase, which any finite value may take), and each structural rule and work limit has a malformed-scene case.
- **Program behavior** (`core/cli/tests/cli.rs`): options, messages, exit statuses, failed writes, and the decoded PNG, APNG and GIF output.

### Portable fixtures

`core/engine/tests/fixtures/` holds one scene per style at 48 × 36 with the SHA-256 of the image it renders. `fixtures.rs` (`--features json`) renders them and also builds each scene from scratch with `build_scene`. After a deliberate change to a style, regenerate the changed fixtures with the ignored `bless_fixtures` test, run on its own, then run the fixture tests again:

```bash
cargo test $M -p simulacra_engine --features json --test fixtures -- --ignored --exact bless_fixtures
```

### Golden hashes

`core/cli/tests/golden/` holds 343 renders of the program (every style at several sizes, seeds, phases and palettes, APNG and GIF animations): each file's hash in `SHA256SUMS` and each decoded frame's in `PIXELS`. Build the binary and check both:

```bash
cargo build --release $M -p simulacra_cli
DIR=$(mktemp -d)/golden   # golden.sh creates it; it must be empty or missing
sh core/cli/tests/golden.sh check "$B" --keep "$DIR"
SIMULACRA_GOLDEN_OUT="$DIR" cargo test --release $M -p simulacra_cli --test golden_pixels -- --ignored
sh core/cli/tests/golden_selftest.sh   # tests golden.sh itself against a stub binary
```

`golden.sh check` leaves the rendered files in the `--keep` directory even when a hash differs, so the pixel test can still run on them. `--container IMAGE` runs the binary inside a Docker image.

Re-blessing, in this order:

- **A change to file bytes but not images** (such as a `flate2` or `miniz_oxide` update): run `golden.sh check "$B" --keep "$DIR"` (it fails on `SHA256SUMS`), then the pixel test on `$DIR` without `SIMULACRA_BLESS`, which must pass against the unchanged `PIXELS`; only then `golden.sh bless "$B"`.
- **A deliberate visual change:** `bless_fixtures`, then `golden.sh bless "$B"` (it does not take `--keep`), then `golden.sh check "$B" --keep "$DIR"`, then the pixel test with `SIMULACRA_BLESS=1`, which rewrites `PIXELS`. It refuses a directory with a file missing or not matching `SHA256SUMS`, or fixture frames that do not match the portable fixtures, which is why those two are blessed first.

### Coverage

`core/coverage.sh` collects line coverage the way CI does (it needs [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) and `rustup component add llvm-tools-preview`), writes `lcov.info` and runs `core/check_lcov.sh`, which fails if the binary's or the shaders' coverage went missing.

## Manual checks

### After changing how a style draws

The hashes only say that an image changed, not that it looks right.

1. Render the style before and after the change, square, wide and tall, with a few seeds and palettes, for example:

   ```bash
   for size in "512 512" "960 360" "360 960"; do
     set -- $size
     for seed in 1 7 -3; do
       "$B" --style STYLE --seed "$seed" --width "$1" --height "$2" --out "STYLE-$seed-$1x$2.png"
     done
   done
   ```

2. Render a short animation (`--frames 24`) and watch it loop: no jump at the loop point, no frame that collapses or flashes.
3. Check a tiny (64 × 64) and a large (1920 × 1080) render for lost detail or features that are too small.
4. Put before-and-after renders in the pull request, and re-bless the fixture and both golden manifests in it.

Do not commit the images.

### Another decoder and viewer

The CLI tests decode output with the repository's own test decoders. After a change to a writer (`png.rs`, `gif.rs`), also decode a few files from your build with FFmpeg and open them in a browser or image viewer:

```bash
"$B" --style rings --frames 24 --out rings.png
"$B" --style rings --frames 24 --out rings.gif
for f in rings.png rings.gif; do
  ffprobe -v error -count_frames -show_entries stream=codec_name,width,height,nb_read_frames -of json "$f"
  ffmpeg -v error -i "$f" -f null -
done
```

Expect 24 frames at 256 × 256 and no errors, and an animation that plays and loops in the viewer.

### Performance

Compare against a fresh run of `main` on the same machine, not against older numbers:

```bash
cargo run --release $M -p simulacra_engine --example bench -- --width 1920 --height 1080 --repeat 3
hyperfine --warmup 1 "$B --style ink --width 1920 --height 1080 --out /tmp/ink.png"
```

`bench` prints scene-building and shading time per style (best of the repeats, in milliseconds) and takes `--styles a,b`, `--phase`, `--seed` and `--palette`. `hyperfine` (or a `time` loop) measures whole runs, encoding included; add `--frames` to time animations, and watch peak memory (`/usr/bin/time -v`) when changing the frame pipeline.

### Release packages on a clean machine

After the release workflow drafts a release (see "Releasing" in [`AGENTS.md`](AGENTS.md)), check the downloads before publishing. The packages hold `simulacra` (or `simulacra.exe`), `LICENSE` and `README.md` with no enclosing directory, so unpack them into a directory of their own, never into the checkout.

1. Download the packages and `SHA256SUMS` from the draft into an empty directory; run `sha256sum -c SHA256SUMS` and `gh attestation verify FILE --repo juusolatva/simulacra` for each package.
2. **Linux:** unpack a package and run the golden check on it in a clean image (CI's verify job does the same with a pinned `debian:trixie-slim`), from the checkout of the tagged commit:

   ```bash
   mkdir -p dist && tar -xzf simulacra-X.Y.Z-x86_64-unknown-linux-gnu.tar.gz -C dist   # outside the checkout
   sh core/cli/tests/golden.sh check dist/simulacra --container debian:trixie-slim
   ```

3. **Windows, clean machine:** on a machine or VM without Rust, Python or developer tools, download the zip through a browser, unpack it, and note what Windows SmartScreen shows when `simulacra.exe` is first run. Run `simulacra.exe --version` and a few renders, still and animated.
4. **Windows, golden check (optional):** CI's verify job already ran the golden check on the package on a fresh Windows runner. To repeat it, use a Windows machine with Git Bash (not the clean one): unpack the zip into a directory outside the checkout and run `sh core/cli/tests/golden.sh check DIR/simulacra.exe` from the checkout of the tagged commit.
5. Open a few rendered PNG, APNG and GIF files in the platform's default viewer.
