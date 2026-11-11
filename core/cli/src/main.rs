//! `simulacra`: the standalone command-line tool, rendering without Python.
//!
//! The options, defaults and messages follow `simulacra.py`'s command line for
//! every style (`--list-styles`): still PNG images, looping APNG animations
//! (`--frames`) and GIF (a `.gif` `--out`, byte for byte as `simulacra.py`
//! writes it). Each animation frame is rendered, encoded and written to the
//! temporary output file before the next one, so only one frame's pixels are
//! held at a time.

mod gif;
mod output;
mod png;

use std::hash::{BuildHasher, Hasher};
use std::path::PathBuf;
use std::process::ExitCode;

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

/// `MAX_ANIMATION_BYTES` in Python: the most bytes of packed frames an
/// animation may need unless `--no-limits` is given. The binary holds only one
/// frame at a time, but keeps Python's limit so both tools accept the same
/// commands (PLAN.md Phase 5 reviews the limits for streaming).
const MAX_ANIMATION_BYTES: u128 = 1 << 30;

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
    /// Skip the size limits on pixels and animation memory
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

/// `check_limits` in Python.
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
    let total = pixels * 3 * frames as u128;
    if frames > 1 && total > MAX_ANIMATION_BYTES {
        return Err(format!(
            "{frames} frames of {width} x {height} need {total} bytes in memory; the limit is {MAX_ANIMATION_BYTES} (use --no-limits to override)"
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

    fn frame(&mut self, rgb: &[u8]) -> Result<Vec<u8>, String> {
        match self {
            Self::Apng(apng) => apng.frame(rgb).map_err(|err| err.to_string()),
            Self::Gif(gif) => gif.frame(rgb).map_err(|err| err.to_string()),
        }
    }

    fn finish(self) -> Result<Vec<u8>, String> {
        match self {
            Self::Apng(apng) => apng.finish().map_err(|err| err.to_string()),
            Self::Gif(gif) => Ok(gif.finish()),
        }
    }
}

/// Renders and writes an APNG or GIF of `frames` frames, one frame at a time.
fn write_frames(args: &Args, frame: &Frame, seed: i64, frames: usize) -> Result<(), String> {
    // Seeds only grow from frame to frame, so if the last frame's plan fits,
    // every frame's does: refuse before rendering anything.
    frame_plan(seed, args.phase, frames, args.slideshow, frames - 1)?;
    let (mut encoder, start) = Encoder::new(args, frame, frames)?;
    let cannot_write = |err: std::io::Error| format!("cannot write {}: {err}", args.out.display());
    let written = output::atomic_write_with(&args.out, |out| {
        out.write_all(&start).map_err(cannot_write)?;
        for index in 0..frames {
            // One frame is a still image at --phase, as in Python's _render_and_write.
            let (frame_seed, phase) = if frames == 1 {
                (seed, args.phase)
            } else {
                frame_plan(seed, args.phase, frames, args.slideshow, index)?
            };
            let rgb = frame.render(frame_seed, phase)?;
            let chunks = encoder.frame(&rgb)?;
            drop(rgb);
            out.write_all(&chunks).map_err(cannot_write)?;
        }
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
    use super::{check_limits, fps, frame_plan, name_wants_gif, parse_hex_palette, random_seed};

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
    fn animation_limits_match_python() {
        assert!(check_limits(1000, 1000, 357).is_ok());
        let err = check_limits(1000, 1000, 358).unwrap_err();
        assert_eq!(
            err,
            "358 frames of 1000 x 1000 need 1074000000 bytes in memory; the limit is 1073741824 (use --no-limits to override)"
        );
        // One frame is a still image, held to the pixel limit only.
        assert!(check_limits(10_000, 10_000, 1).is_ok());
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
}
