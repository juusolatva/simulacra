//! `simulacra`: the standalone command-line tool, rendering without Python.
//!
//! The options, defaults and messages follow `simulacra.py`'s command line for
//! every style (`--list-styles`): still PNG images, looping APNG animations
//! (`--frames`) and GIF (a `.gif` `--out`, byte for byte as `simulacra.py`
//! writes it). Animation frames are rendered and encoded several at a time
//! on the thread pool and written to the temporary output file in order as
//! they finish; the number in progress is bounded by the thread count and a
//! memory budget ([`frames_in_flight`]), not by the frame count.

mod gif;
mod output;
mod png;

use std::collections::BTreeMap;
use std::hash::{BuildHasher, Hasher};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::mpsc;

use clap::{ArgGroup, Parser};
use simulacra_engine::{
    PALETTES, Palette, SCENE_BUILDERS, build_scene, palette_stops, py_mod, render, seed32,
};

/// `MAX_FRAME_PIXELS` in Python: the most pixels a frame may have unless
/// `--no-limits` is given.
const MAX_FRAME_PIXELS: u128 = 100_000_000;

/// `MAX_SIDE` in Python: the longest side a frame may have unless
/// `--no-limits` is given. Some scenes grow with a side alone (`woodblock`'s
/// heights per column, `lissajous`'s curve points), and they are built before
/// rendering validates them, so a thin canvas within the pixel limit could
/// otherwise need gigabytes for its scene.
const MAX_SIDE: usize = 100_000;

/// The most pixels an animation may render in all frames together unless
/// `--no-limits` is given. It replaces Python's `MAX_ANIMATION_BYTES` (1 GiB
/// of packed frames, 358 million pixels), which guards memory: the binary
/// holds only the frames in progress ([`frames_in_flight`]), so its limit
/// guards run time and output size instead. At the 4 to 49 million pixels per second and at most 0.7 bytes
/// per pixel measured (PLAN.md Phase 5) that is up to about 18 minutes and
/// 3 GB, or 517 frames at 3840 x 2160. The binary therefore accepts longer
/// animations than `simulacra.py`.
const MAX_TOTAL_PIXELS: u128 = 1 << 32;

/// The most frames an animation may have unless `--no-limits` is given.
/// Building, validating and encoding a frame costs the same however small
/// it is (at most 174 microseconds and 66 bytes per frame at 1 x 1, measured
/// for every style), so [`MAX_TOTAL_PIXELS`] alone would let a tiny canvas
/// run for days. 65,536 frames bound that to about 11 s and 4.3 MB, and it
/// equals counting each frame as at least 256 x 256 pixels (2**32 / 2**16).
const MAX_FRAMES: usize = 1 << 16;

/// The memory one frame in progress may need, per pixel. `lissajous` needs
/// the most: a canvas of 8-byte floats for each of its 2 or 3 curves plus the
/// 3-byte RGB frame, 27 bytes per pixel (measured at 26 for 3 curves at
/// 3840 x 2160; every other style measured at most 11 at 1920 x 1080 and
/// 3840 x 2160). The rest leaves room for the encoded frame, which waits in
/// memory until it is written.
const FRAME_BYTES_PER_PIXEL: usize = 32;

/// The scene memory one frame in progress may need per pixel of `width + 2 *
/// height`, for scenes that grow with the sides rather than the area.
/// `lissajous` samples each curve about once per pixel of its length, 16
/// bytes per point: its x moves at most `width` pixels per radian (`cx * ax *
/// fx`, at most `width / 2 * 0.8 * 2.5`) and its y at most `1.8 * height`, so
/// its 3 curves hold at most `3 * 16 * 2 pi * (width + 1.8 * height)` bytes
/// (plus 66 points each), under 302 bytes per pixel of `width + 2 * height`.
/// Measured on 10 x 100,000 and 100,000 x 10 canvases, no other style
/// needs more than [`FRAME_BYTES_PER_PIXEL`] covers.
const FRAME_BYTES_PER_SIDE: usize = 320;

/// The memory the frames in progress may use together, by
/// [`frame_bytes`]: up to 15 frames of 1920 x 1080 or 4 of 3840 x 2160, and
/// one frame however large: a single frame needs what it needs, as when
/// frames render one by one, so its size is bounded only by
/// `MAX_FRAME_PIXELS` and `MAX_SIDE`, which `--no-limits` skips.
const FRAMES_IN_FLIGHT_BYTES: usize = 1 << 30;

/// The memory one frame of `width` x `height` may need while it is in
/// progress: [`FRAME_BYTES_PER_PIXEL`] for each pixel plus
/// [`FRAME_BYTES_PER_SIDE`] for each pixel of `width + 2 * height`.
fn frame_bytes(width: usize, height: usize) -> usize {
    let area = width
        .saturating_mul(height)
        .saturating_mul(FRAME_BYTES_PER_PIXEL);
    let sides = height
        .saturating_mul(2)
        .saturating_add(width)
        .saturating_mul(FRAME_BYTES_PER_SIDE);
    area.saturating_add(sides)
}

/// How many frames of `width` x `height` to render and encode at once: one
/// per thread in the pool, within [`FRAMES_IN_FLIGHT_BYTES`], at least one and
/// at most `frames`.
fn frames_in_flight(width: usize, height: usize, frames: usize, threads: usize) -> usize {
    let per_frame = frame_bytes(width, height).max(1);
    (FRAMES_IN_FLIGHT_BYTES / per_frame)
        .min(threads)
        .min(frames)
        .max(1)
}

#[derive(Parser, Debug)]
#[command(
    name = "simulacra",
    version,
    about = "Simulacra: procedurally generated images, without Python",
    group(ArgGroup::new("seeds").args(["seed", "random_seed"])),
    group(ArgGroup::new("palettes").args(["palette", "palette_stops"]))
)]
struct Args {
    /// Style to draw (see --list-styles)
    #[arg(long, default_value = "waves", value_parser = clap::builder::PossibleValuesParser::new(SCENE_BUILDERS))]
    style: String,
    /// Seed; the same seed gives the same image as simulacra.py
    #[arg(long, default_value_t = 1, allow_negative_numbers = true)]
    seed: i64,
    /// Pick a random seed and print it so the image can be reproduced
    #[arg(long)]
    random_seed: bool,
    /// Built-in palette (see --list-palettes)
    #[arg(long, default_value = "deep", value_parser = palette_name)]
    palette: String,
    /// Custom palette: two or more comma-separated hex colors from dark to
    /// light, e.g. 0c061c,5814a8,ff8800 (quote values starting with #)
    #[arg(long, value_name = "HEX,HEX,...", value_parser = parse_hex_palette)]
    palette_stops: Option<Stops>,
    #[arg(long, default_value_t = 256, allow_negative_numbers = true)]
    width: i64,
    #[arg(long, default_value_t = 256, allow_negative_numbers = true)]
    height: i64,
    /// Output file, replaced only after a successful write: GIF if it ends in
    /// .gif, otherwise PNG (APNG with --frames)
    #[arg(long, default_value = "art.png")]
    out: PathBuf,
    /// Number of frames (default 1). Styles with motion animate one scene in a
    /// seamless loop
    #[arg(long, default_value_t = 1, allow_negative_numbers = true)]
    frames: i64,
    /// Loop position for styles with motion; 1.0 repeats 0.0 (default 0).
    /// With --frames, the first frame's position
    #[arg(long, default_value_t = 0.0, allow_negative_numbers = true, value_parser = finite)]
    phase: f64,
    /// Make every frame an unrelated scene with consecutive seeds, even for
    /// styles with motion
    #[arg(long)]
    slideshow: bool,
    /// Animation frame rate with --frames > 1 (default 12)
    #[arg(long, default_value_t = 12, allow_negative_numbers = true, value_parser = fps)]
    fps: u16,
    /// Skip the size limits on pixels per frame and in all frames
    #[arg(long)]
    no_limits: bool,
    /// Print the style names, one per line, and exit
    #[arg(long)]
    list_styles: bool,
    /// Print the palette names, one per line, and exit
    #[arg(long)]
    list_palettes: bool,
}

#[derive(Clone, Debug)]
struct Stops(Vec<(i64, i64, i64)>);

fn palette_name(name: &str) -> Result<String, String> {
    if palette_stops(name).is_some() {
        return Ok(name.to_owned());
    }
    let names: Vec<&str> = PALETTES.iter().map(|(n, _)| *n).collect();
    Err(format!(
        "unknown palette (choose from {})",
        names.join(", ")
    ))
}

fn finite(text: &str) -> Result<f64, String> {
    match text.trim().parse::<f64>() {
        Ok(value) if value.is_finite() => Ok(value),
        Ok(_) => Err("phase must be a finite number".into()),
        Err(err) => Err(err.to_string()),
    }
}

/// `_fps` in Python: a whole number from 1 to 65535.
fn fps(text: &str) -> Result<u16, String> {
    let value: i64 = text
        .trim()
        .parse()
        .map_err(|err: std::num::ParseIntError| err.to_string())?;
    u16::try_from(value)
        .ok()
        .filter(|&fps| fps >= 1)
        .ok_or_else(|| "fps must be between 1 and 65535".into())
}

/// `parse_hex_palette` in Python: comma-separated `rrggbb` or `rgb` colors, each
/// with an optional leading `#`; at least two.
fn parse_hex_palette(text: &str) -> Result<Stops, String> {
    let mut stops = Vec::new();
    for item in text.split(',') {
        let item = item.trim();
        let digits = item.strip_prefix('#').unwrap_or(item);
        let valid = matches!(digits.len(), 3 | 6) && digits.bytes().all(|b| b.is_ascii_hexdigit());
        if !valid {
            return Err(format!(
                "invalid hex color {item:?} (use rrggbb or rgb, e.g. #ff8800)"
            ));
        }
        let full: String = if digits.len() == 3 {
            digits.chars().flat_map(|c| [c, c]).collect()
        } else {
            digits.to_owned()
        };
        let channel = |i: usize| i64::from_str_radix(&full[i..i + 2], 16).expect("hex digits");
        stops.push((channel(0), channel(2), channel(4)));
    }
    if stops.len() < 2 {
        return Err("a palette needs at least two colors".into());
    }
    Ok(Stops(stops))
}

/// `check_limits` in Python, with [`MAX_TOTAL_PIXELS`] in place of Python's
/// limit on animation memory.
fn check_limits(width: usize, height: usize, frames: usize) -> Result<(), String> {
    let pixels = width as u128 * height as u128;
    if pixels > MAX_FRAME_PIXELS {
        return Err(format!(
            "{width} x {height} is {pixels} pixels; the limit is {MAX_FRAME_PIXELS} (use --no-limits to override)"
        ));
    }
    if width.max(height) > MAX_SIDE {
        return Err(format!(
            "{width} x {height} has a side over {MAX_SIDE} pixels, the limit (use --no-limits to override)"
        ));
    }
    if frames > MAX_FRAMES {
        return Err(format!(
            "{frames} frames; the limit is {MAX_FRAMES} (use --no-limits to override)"
        ));
    }
    let total = pixels * frames as u128;
    if frames > 1 && total > MAX_TOTAL_PIXELS {
        return Err(format!(
            "{frames} frames of {width} x {height} are {total} pixels in all; the limit is {MAX_TOTAL_PIXELS} (use --no-limits to override)"
        ));
    }
    Ok(())
}

/// A seed below 2**31, like Python's `secrets.randbelow(2 ** 31)`, from the
/// standard library's randomly keyed hasher (no extra dependency).
fn random_seed() -> i64 {
    let bits = std::collections::hash_map::RandomState::new()
        .build_hasher()
        .finish();
    (bits >> 33) as i64
}

fn list_names(args: &Args) -> Option<Vec<&'static str>> {
    if !(args.list_styles || args.list_palettes) {
        return None;
    }
    let mut names = Vec::new();
    if args.list_styles {
        names.extend(SCENE_BUILDERS);
    }
    if args.list_palettes {
        names.extend(PALETTES.iter().map(|(n, _)| *n));
    }
    Some(names)
}

/// `_wants_gif` in Python: `--out` ends in `.gif`, in any case.
fn wants_gif(args: &Args) -> bool {
    name_wants_gif(args.out.as_os_str())
}

/// Compares the whole name, not `Path::extension`, which is `None` for a file
/// named `.gif` (Python's `'.gif'.lower().endswith('.gif')` is true) and
/// would need UTF-8; a lossy string keeps the ASCII suffix of any name.
fn name_wants_gif(name: &std::ffi::OsStr) -> bool {
    name.to_string_lossy().to_lowercase().ends_with(".gif")
}

/// Checks the arguments in `_check_args`'s order; returns width, height and
/// the frame count.
fn dimensions(args: &Args) -> Result<(usize, usize, usize), String> {
    let positive = |v: i64| usize::try_from(v).ok().filter(|&v| v > 0);
    let (Some(width), Some(height)) = (positive(args.width), positive(args.height)) else {
        return Err("width and height must be positive".into());
    };
    let Some(frames) = positive(args.frames) else {
        return Err("frames must be positive".into());
    };
    if wants_gif(args) {
        if width.max(height) > gif::GIF_MAX_SIDE {
            return Err(format!(
                "GIF width and height cannot exceed {} pixels",
                gif::GIF_MAX_SIDE
            ));
        }
    } else if u32::try_from(width).is_err() || u32::try_from(height).is_err() {
        // PNG stores each side as a 32-bit integer; refuse before rendering.
        return Err(format!("PNG width and height cannot exceed {}", u32::MAX));
    } else if frames > png::APNG_MAX_FRAMES {
        // A format limit, like the sides: --no-limits cannot lift it.
        return Err(format!(
            "an APNG cannot hold more than {} frames",
            png::APNG_MAX_FRAMES
        ));
    }
    if !args.no_limits {
        check_limits(width, height, frames)?;
    }
    Ok((width, height, frames))
}

/// `_frame_plan` in Python: the seed and phase of frame `index`. Every style
/// moves, so frames step through one loop from `--phase` (reduced with
/// Python's float `%` so the steps survive huge phases), unless `--slideshow`
/// gives each frame the next seed at the same phase.
fn frame_plan(
    seed: i64,
    phase: f64,
    frames: usize,
    slideshow: bool,
    index: usize,
) -> Result<(i64, f64), String> {
    if slideshow {
        let seed = i64::try_from(index)
            .ok()
            .and_then(|i| seed.checked_add(i))
            .ok_or("--slideshow seeds would pass 9223372036854775807")?;
        return Ok((seed, phase));
    }
    // Python divides two ints exactly; both are far below 2**53 here.
    Ok((seed, py_mod(phase, 1.0) + index as f64 / frames as f64))
}

/// One frame's palette, size and scene parameters.
struct Frame<'a> {
    style: &'a str,
    width: usize,
    height: usize,
    palette: &'a Palette,
}

impl Frame<'_> {
    fn render(&self, seed: i64, phase: f64) -> Result<Vec<u8>, String> {
        let scene = build_scene(
            self.style,
            self.width,
            self.height,
            seed,
            self.palette,
            phase,
        )
        .ok_or_else(|| format!("no scene builder for {}", self.style))?;
        render(
            scene.as_ref(),
            self.width,
            self.height,
            seed32(seed),
            self.palette,
        )
        .map_err(|err| err.to_string())
    }
}

/// The encoder of a streamed output file: APNG, or GIF with any frame count.
enum Encoder {
    Apng(png::Apng),
    Gif(gif::Gif),
}

impl Encoder {
    /// The encoder for `--out` and the bytes that begin the file.
    fn new(args: &Args, frame: &Frame, frames: usize) -> Result<(Self, Vec<u8>), String> {
        if wants_gif(args) {
            let (gif, start) = gif::Gif::new(
                frame.width,
                frame.height,
                frame.palette,
                args.fps,
                frames > 1,
            )
            .map_err(|err| err.to_string())?;
            return Ok((Self::Gif(gif), start));
        }
        let count = u32::try_from(frames).map_err(|_| png::PngError::FrameCount.to_string())?;
        let (apng, start) = png::Apng::new(frame.width, frame.height, count, args.fps)
            .map_err(|err| err.to_string())?;
        Ok((Self::Apng(apng), start))
    }

    /// Encodes one frame's pixels; frames can be encoded on several threads
    /// at once. The result goes to [`Encoder::frame`], in frame order.
    fn encode(&self, rgb: &[u8]) -> Result<Vec<u8>, String> {
        match self {
            Self::Apng(apng) => apng.compress(rgb).map_err(|err| err.to_string()),
            Self::Gif(gif) => gif.frame(rgb).map_err(|err| err.to_string()),
        }
    }

    /// The bytes that put the next frame, encoded by [`Encoder::encode`], in
    /// the file. A GIF frame's blocks are already complete; an APNG frame's
    /// chunks carry sequence numbers, so they are added in order.
    fn frame(&self, encoded: Vec<u8>) -> Result<Vec<u8>, String> {
        match self {
            Self::Apng(apng) => apng.frame(&encoded).map_err(|err| err.to_string()),
            Self::Gif(_) => Ok(encoded),
        }
    }

    fn finish(&self) -> Result<Vec<u8>, String> {
        match self {
            Self::Apng(apng) => apng.finish().map_err(|err| err.to_string()),
            Self::Gif(gif) => Ok(gif.finish()),
        }
    }
}

/// A frame job's reply to the writer, sent when the job ends: its result, or
/// `None` if the job panicked (sent while unwinding, so the writer stops
/// waiting; the scope then propagates the panic).
struct Reply<T> {
    index: usize,
    result: Option<Result<T, String>>,
    sender: mpsc::Sender<(usize, Option<Result<T, String>>)>,
}

impl<T> Drop for Reply<T> {
    fn drop(&mut self) {
        // The writer stops listening only after an error or a panic, when no
        // more results are needed.
        let _ = self.sender.send((self.index, self.result.take()));
    }
}

/// Runs `make(0..frames)` on the thread pool, at most `window` at a time, and
/// passes the results to `write` in index order as they become available.
/// Results are handled strictly in order, so the error reported is the first
/// failing frame's, as when frames are made one by one; on an error the
/// frames in progress finish and are discarded. A panic in `make` propagates.
fn in_order<T: Send>(
    frames: usize,
    window: usize,
    make: impl Fn(usize) -> Result<T, String> + Sync,
    mut write: impl FnMut(T) -> Result<(), String>,
) -> Result<(), String> {
    let (sender, receiver) = mpsc::channel();
    let make = &make;
    rayon::in_place_scope(|scope| {
        let spawn = |index: usize| {
            let reply = Reply {
                index,
                result: None,
                sender: sender.clone(),
            };
            scope.spawn(move |_| {
                let mut reply = reply;
                reply.result = Some(make(index));
            });
        };
        let mut next = window.max(1).min(frames);
        (0..next).for_each(spawn);
        let mut pending = BTreeMap::new();
        for index in 0..frames {
            let result = loop {
                if let Some(result) = pending.remove(&index) {
                    break result;
                }
                // Frame `index` has been spawned and its job holds a sender.
                let (done, result) = receiver.recv().expect("this function holds a sender");
                let Some(result) = result else {
                    return Ok(()); // a job panicked: the scope propagates it
                };
                pending.insert(done, result);
            };
            write(result?)?;
            if next < frames {
                spawn(next);
                next += 1;
            }
        }
        Ok(())
    })
}

/// Renders and writes an APNG or GIF of `frames` frames, rendering and
/// encoding up to [`frames_in_flight`] frames at once.
fn write_frames(args: &Args, frame: &Frame, seed: i64, frames: usize) -> Result<(), String> {
    // Seeds only grow from frame to frame, so if the last frame's plan fits,
    // every frame's does: refuse before rendering anything.
    frame_plan(seed, args.phase, frames, args.slideshow, frames - 1)?;
    let (encoder, start) = Encoder::new(args, frame, frames)?;
    let cannot_write = |err: std::io::Error| format!("cannot write {}: {err}", args.out.display());
    let make = |index: usize| {
        // One frame is a still image at --phase, as in Python's _render_and_write.
        let (frame_seed, phase) = if frames == 1 {
            (seed, args.phase)
        } else {
            frame_plan(seed, args.phase, frames, args.slideshow, index)?
        };
        let rgb = frame.render(frame_seed, phase)?;
        encoder.encode(&rgb)
    };
    let window = frames_in_flight(
        frame.width,
        frame.height,
        frames,
        rayon::current_num_threads(),
    );
    let written = output::atomic_write_with(&args.out, |out| {
        out.write_all(&start).map_err(cannot_write)?;
        in_order(frames, window, make, |encoded| {
            let chunks = encoder.frame(encoded)?;
            out.write_all(&chunks).map_err(cannot_write)
        })?;
        let end = encoder.finish()?;
        out.write_all(&end).map_err(cannot_write)
    });
    written.map_err(|err| match err {
        output::WriteError::Io(err) => cannot_write(err),
        output::WriteError::Content(message) => message,
    })
}

fn run(args: &Args) -> Result<String, String> {
    let (width, height, frames) = dimensions(args)?;
    let seed = if args.random_seed {
        random_seed()
    } else {
        args.seed
    };
    let (palette_label, stops) = match &args.palette_stops {
        Some(Stops(stops)) => ("custom", stops.clone()),
        None => (
            args.palette.as_str(),
            palette_stops(&args.palette).expect("validated").to_vec(),
        ),
    };
    let palette = Palette::new(stops).map_err(|err| err.to_string())?;
    let frame = Frame {
        style: &args.style,
        width,
        height,
        palette: &palette,
    };
    if frames > 1 || wants_gif(args) {
        write_frames(args, &frame, seed, frames)?;
    } else {
        let rgb = frame.render(seed, args.phase)?;
        let encoded = png::encode(&rgb, width, height).map_err(|err| err.to_string())?;
        output::atomic_write(&args.out, &encoded)
            .map_err(|err| format!("cannot write {}: {err}", args.out.display()))?;
    }
    let (out, style) = (args.out.display(), &args.style);
    Ok(if frames > 1 {
        format!(
            "wrote {frames} animation frames to {out} ({style}, seed={seed}, palette={palette_label}, {} fps)",
            args.fps
        )
    } else {
        format!("wrote {out} ({style}, seed={seed}, palette={palette_label})")
    })
}

fn main() -> ExitCode {
    let args = Args::parse();
    if let Some(names) = list_names(&args) {
        println!("{}", names.join("\n"));
        return ExitCode::SUCCESS;
    }
    match run(&args) {
        Ok(message) => {
            println!("{message}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("simulacra: error: {message}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        check_limits, fps, frame_plan, frames_in_flight, in_order, name_wants_gif,
        parse_hex_palette, random_seed,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[test]
    fn hex_palettes_parse_like_python() {
        let stops = parse_hex_palette("#0c061c, 5814a8,f80").unwrap().0;
        assert_eq!(stops, [(12, 6, 28), (88, 20, 168), (255, 136, 0)]);
        assert!(parse_hex_palette("ff8800").is_err(), "one color");
        assert!(parse_hex_palette("ff8800,12345").is_err(), "five digits");
        assert!(parse_hex_palette("ff8800,ggg").is_err(), "not hex");
    }

    #[test]
    fn limits_match_python() {
        assert!(check_limits(10_000, 10_000, 1).is_ok());
        let err = check_limits(10_001, 10_000, 1).unwrap_err();
        assert_eq!(
            err,
            "10001 x 10000 is 100010000 pixels; the limit is 100000000 (use --no-limits to override)"
        );
        assert!(check_limits(100_000, 1000, 1).is_ok());
        let err = check_limits(1, 100_000_000, 1).unwrap_err();
        assert_eq!(
            err,
            "1 x 100000000 has a side over 100000 pixels, the limit (use --no-limits to override)"
        );
    }

    #[test]
    fn animations_are_limited_by_total_pixels() {
        // Python refuses 358 frames of 1000 x 1000 (over 1 GiB of frames in
        // memory); the binary holds only the frames in progress, so its budget is
        // 2**32 pixels.
        assert!(check_limits(1000, 1000, 358).is_ok());
        assert!(check_limits(3840, 2160, 517).is_ok());
        assert!(
            check_limits(65_536, 256, 256).is_ok(),
            "exactly 2**32 pixels"
        );
        let err = check_limits(65_536, 256, 257).unwrap_err();
        assert_eq!(
            err,
            "257 frames of 65536 x 256 are 4311744512 pixels in all; the limit is 4294967296 (use --no-limits to override)"
        );
        // One frame is a still image, held to the pixel limit only.
        assert!(check_limits(10_000, 10_000, 1).is_ok());
    }

    #[test]
    fn tiny_animations_are_limited_by_frame_count() {
        assert!(check_limits(1, 1, 65_536).is_ok());
        let err = check_limits(1, 1, 65_537).unwrap_err();
        assert_eq!(
            err,
            "65537 frames; the limit is 65536 (use --no-limits to override)"
        );
        // Up to 256 x 256 the frame count binds; above, the pixel total does.
        assert!(check_limits(256, 256, 65_536).is_ok());
        assert!(check_limits(257, 256, 65_536).is_err());
        assert!(check_limits(1, 1, 1 << 31).is_err(), "within 2**32 pixels");
    }

    #[test]
    fn frame_rates_parse_like_python() {
        assert_eq!(fps(" 24 "), Ok(24));
        assert_eq!(fps("65535"), Ok(65535));
        for text in ["0", "-3", "65536"] {
            assert_eq!(
                fps(text),
                Err("fps must be between 1 and 65535".into()),
                "{text}"
            );
        }
        assert!(fps("1.5").is_err());
    }

    #[test]
    fn frame_phases_match_python() {
        // Expected values from simulacra.py's _frame_plan: (phase % 1.0) + i / frames.
        let phases = |phase: f64, frames: usize| -> Vec<f64> {
            (0..frames)
                .map(|i| frame_plan(5, phase, frames, false, i).unwrap())
                .inspect(|&(seed, _)| assert_eq!(seed, 5))
                .map(|(_, phase)| phase)
                .collect()
        };
        assert_eq!(phases(0.0, 4), [0.0, 0.25, 0.5, 0.75]);
        assert_eq!(phases(-0.25, 2), [0.75, 1.25]);
        assert_eq!(phases(1.25, 3), [0.25, 0.25 + 1.0 / 3.0, 0.25 + 2.0 / 3.0]);
        assert_eq!(phases(1e300, 2), [0.0, 0.5], "1e300 % 1.0 == 0.0");
        assert_eq!(phases(-1e-20, 1), [1.0], "-1e-20 % 1.0 == 1.0 in Python");
    }

    #[test]
    fn slideshows_step_the_seed() {
        assert_eq!(frame_plan(-1, 0.3, 3, true, 2), Ok((1, 0.3)));
        assert_eq!(
            frame_plan(i64::MAX - 1, 0.3, 3, true, 1),
            Ok((i64::MAX, 0.3))
        );
        assert!(frame_plan(i64::MAX, 0.3, 3, true, 1).is_err());
    }

    #[test]
    fn gif_names_match_python() {
        // Python: [n.lower().endswith('.gif') for n in names]
        let wants = |name: &str| name_wants_gif(std::ffi::OsStr::new(name));
        for name in [
            "a.gif",
            "a.GIF",
            "dir/b.GiF",
            ".gif",
            "dir/.gif",
            "c.png.gif",
        ] {
            assert!(wants(name), "{name}");
        }
        for name in ["a.png", "gif", "a.gif.png", "a.gi", "a_gif"] {
            assert!(!wants(name), "{name}");
        }
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            let name = std::ffi::OsStr::from_bytes(b"\xff\xfe.gif");
            assert!(name_wants_gif(name), "a name that is not UTF-8");
        }
    }

    #[test]
    fn random_seeds_stay_below_2_pow_31() {
        for _ in 0..100 {
            assert!((0..1 << 31).contains(&random_seed()));
        }
    }

    /// Makes frame `index` after a delay that varies from frame to frame, so
    /// jobs finish out of order, counting how many run at once.
    fn slow_make(running: &AtomicUsize, most: &AtomicUsize, index: usize) -> Result<usize, String> {
        let now = running.fetch_add(1, Ordering::SeqCst) + 1;
        most.fetch_max(now, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis((index * 7 % 5) as u64));
        running.fetch_sub(1, Ordering::SeqCst);
        Ok(index)
    }

    #[test]
    fn frames_are_written_in_order_with_a_bounded_window() {
        let (running, most) = (AtomicUsize::new(0), AtomicUsize::new(0));
        let mut written = Vec::new();
        let make = |index| slow_make(&running, &most, index);
        in_order(40, 3, make, |index| {
            written.push(index);
            Ok(())
        })
        .unwrap();
        assert_eq!(written, (0..40).collect::<Vec<_>>());
        assert!(most.load(Ordering::SeqCst) <= 3);
        let mut written = Vec::new();
        in_order(2, 8, Ok, |index| {
            written.push(index);
            Ok(())
        })
        .unwrap();
        assert_eq!(written, [0, 1]);
        in_order(0, 4, Ok::<usize, String>, |_| panic!("no frames to write")).unwrap();
    }

    #[test]
    fn the_first_failing_frame_is_reported() {
        // Frame 6 fails at once, frame 3 only after a while: frame 3's error
        // is the one reported, as when frames are made one by one.
        let mut written = Vec::new();
        let make = |index: usize| match index {
            3 => {
                std::thread::sleep(Duration::from_millis(50));
                Err("frame 3".to_string())
            }
            6 => Err("frame 6".to_string()),
            _ => Ok(index),
        };
        let result = in_order(10, 8, make, |index| {
            written.push(index);
            Ok(())
        });
        assert_eq!(result, Err("frame 3".to_string()));
        assert_eq!(written, [0, 1, 2]);
    }

    #[test]
    fn a_write_error_stops_the_animation() {
        let made = AtomicUsize::new(0);
        let make = |index| {
            made.fetch_add(1, Ordering::SeqCst);
            Ok(index)
        };
        let result = in_order(100, 4, make, |index| {
            if index == 5 {
                Err("disk full".to_string())
            } else {
                Ok(())
            }
        });
        assert_eq!(result, Err("disk full".to_string()));
        // Frames 0 to 5 were written or refused; at most a window more began.
        assert!(made.load(Ordering::SeqCst) <= 6 + 4);
    }

    #[test]
    fn a_panicking_frame_propagates_its_panic() {
        let caught = std::panic::catch_unwind(|| {
            in_order(
                20,
                4,
                |index| {
                    assert_ne!(index, 5, "frame 5 broke");
                    Ok(index)
                },
                |_| Ok(()),
            )
        });
        let payload = caught.expect_err("the panic reaches the caller");
        let message = payload
            .downcast_ref::<String>()
            .expect("assert_ne! panics with a String");
        assert!(message.contains("frame 5 broke"), "{message}");
    }

    #[test]
    fn frames_in_flight_follow_threads_and_the_memory_budget() {
        // 1 GiB at 32 bytes per pixel plus 320 per pixel of width + 2 * height.
        assert_eq!(frames_in_flight(1920, 1080, 100, 12), 12);
        assert_eq!(frames_in_flight(1920, 1080, 100, 64), 15);
        assert_eq!(frames_in_flight(3840, 2160, 100, 64), 4);
        // Thin canvases are bounded by their scene's points, not their area:
        // 32 MB of pixels but 64 MB of lissajous points at most.
        assert_eq!(frames_in_flight(10, 100_000, 64, 64), 11);
        assert_eq!(frames_in_flight(100_000, 10, 64, 64), 16);
        assert_eq!(frames_in_flight(100_000, 1_000, 100, 64), 1);
        assert_eq!(frames_in_flight(usize::MAX, usize::MAX, 100, 64), 1);
        assert_eq!(frames_in_flight(64, 64, 3, 12), 3);
        assert_eq!(frames_in_flight(64, 64, 1, 12), 1);
    }
}
