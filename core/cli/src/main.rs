//! `simulacra`: the standalone command-line tool, rendering without Python.
//!
//! The options, defaults and messages follow `simulacra.py`'s command line for
//! still images. Only styles whose scene builder has been ported to Rust are
//! available (`--list-styles`); animation output (APNG and GIF) comes later.

mod output;
mod png;

use std::hash::{BuildHasher, Hasher};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{ArgGroup, Parser};
use simulacra_engine::{
    PALETTES, Palette, SCENE_BUILDERS, build_scene, palette_stops, render, seed32,
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
    /// Output PNG file, replaced only after a successful write
    #[arg(long, default_value = "art.png")]
    out: PathBuf,
    /// Loop position for styles with motion; 1.0 repeats 0.0 (default 0)
    #[arg(long, default_value_t = 0.0, allow_negative_numbers = true, value_parser = finite)]
    phase: f64,
    /// Skip the size limit on pixels
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

/// `check_limits` in Python, for one frame.
fn check_limits(width: usize, height: usize) -> Result<(), String> {
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

fn dimensions(args: &Args) -> Result<(usize, usize), String> {
    let side = |v: i64| usize::try_from(v).ok().filter(|&v| v > 0);
    let (Some(width), Some(height)) = (side(args.width), side(args.height)) else {
        return Err("width and height must be positive".into());
    };
    if args
        .out
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("gif"))
    {
        return Err(
            "GIF output is not available in the standalone binary yet; use a .png --out".into(),
        );
    }
    if u32::try_from(width).is_err() || u32::try_from(height).is_err() {
        // PNG stores each side as a 32-bit integer; refuse before rendering.
        return Err(format!("PNG width and height cannot exceed {}", u32::MAX));
    }
    if !args.no_limits {
        check_limits(width, height)?;
    }
    Ok((width, height))
}

fn run(args: &Args) -> Result<String, String> {
    let (width, height) = dimensions(args)?;
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
    let scene = build_scene(&args.style, width, height, seed, &palette, args.phase)
        .ok_or_else(|| format!("no scene builder for {}", args.style))?;
    let rgb = render(scene.as_ref(), width, height, seed32(seed), &palette)
        .map_err(|err| err.to_string())?;
    let encoded = png::encode(&rgb, width, height).map_err(|err| err.to_string())?;
    output::atomic_write(&args.out, &encoded)
        .map_err(|err| format!("cannot write {}: {err}", args.out.display()))?;
    Ok(format!(
        "wrote {} ({}, seed={seed}, palette={palette_label})",
        args.out.display(),
        args.style
    ))
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
    use super::{check_limits, parse_hex_palette, random_seed};

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
        assert!(check_limits(10_000, 10_000).is_ok());
        let err = check_limits(10_001, 10_000).unwrap_err();
        assert_eq!(
            err,
            "10001 x 10000 is 100010000 pixels; the limit is 100000000 (use --no-limits to override)"
        );
        assert!(check_limits(100_000, 1000).is_ok());
        let err = check_limits(1, 100_000_000).unwrap_err();
        assert_eq!(
            err,
            "1 x 100000000 has a side over 100000 pixels, the limit (use --no-limits to override)"
        );
    }

    #[test]
    fn random_seeds_stay_below_2_pow_31() {
        for _ in 0..100 {
            assert!((0..1 << 31).contains(&random_seed()));
        }
    }
}
