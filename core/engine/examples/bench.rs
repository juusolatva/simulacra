//! Times scene building and shading for each style, as the minimum over
//! repeats. Encoding and whole runs are timed by running the binary (see the
//! README, "Performance and limits").
//!
//! ```text
//! cargo run --release --manifest-path core/Cargo.toml -p simulacra_engine [--features portable] \
//!     --example bench -- [--width N] [--height N] [--phase X] [--seed N] [--repeat N] \
//!     [--palette NAME] [--styles a,b,...]
//! ```

use std::process::ExitCode;
use std::time::{Duration, Instant};

use simulacra_engine::{
    PORTABLE, Palette, SCENE_BUILDERS, build_scene, palette_stops, render, seed32,
};

/// The binary's `MAX_SIDE` and `MAX_FRAME_PIXELS`.
const MAX_SIDE: usize = 100_000;
const MAX_FRAME_PIXELS: usize = 100_000_000;

struct Options {
    width: usize,
    height: usize,
    phase: f64,
    seed: i64,
    repeat: usize,
    palette: String,
    styles: Vec<String>,
}

fn parse(mut args: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut options = Options {
        width: 640,
        height: 480,
        phase: 0.37,
        seed: 1,
        repeat: 3,
        palette: "deep".to_owned(),
        styles: SCENE_BUILDERS.iter().map(|s| (*s).to_owned()).collect(),
    };
    while let Some(flag) = args.next() {
        let value = args.next().ok_or_else(|| format!("{flag} needs a value"))?;
        let number = |v: &str| {
            v.parse::<usize>()
                .map_err(|_| format!("{flag}: not a whole number: {v}"))
        };
        match flag.as_str() {
            "--width" => options.width = number(&value)?,
            "--height" => options.height = number(&value)?,
            "--repeat" => options.repeat = number(&value)?.max(1),
            "--seed" => {
                options.seed = value
                    .parse()
                    .map_err(|_| format!("--seed: not an integer: {value}"))?
            }
            "--phase" => {
                options.phase = value
                    .parse()
                    .map_err(|_| format!("--phase: not a number: {value}"))?
            }
            "--palette" => options.palette = value,
            "--styles" => options.styles = value.split(',').map(str::to_owned).collect(),
            _ => return Err(format!("unknown option {flag}")),
        }
    }
    if options.width == 0 || options.height == 0 {
        return Err("width and height must be positive".to_owned());
    }
    // The binary's limits: some scenes grow with a side alone and are built
    // before the checked render can refuse the canvas.
    if options.width.max(options.height) > MAX_SIDE
        || options.width.saturating_mul(options.height) > MAX_FRAME_PIXELS
    {
        return Err(format!(
            "{} x {} is over the limits ({MAX_SIDE} pixels a side, {MAX_FRAME_PIXELS} in all)",
            options.width, options.height
        ));
    }
    if let Some(style) = options
        .styles
        .iter()
        .find(|s| !SCENE_BUILDERS.contains(&s.as_str()))
    {
        return Err(format!("unknown style '{style}'"));
    }
    if palette_stops(&options.palette).is_none() {
        return Err(format!("unknown palette '{}'", options.palette));
    }
    Ok(options)
}

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn main() -> ExitCode {
    let options = match parse(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("bench: {message}");
            return ExitCode::from(2);
        }
    };
    let palette = Palette::new(palette_stops(&options.palette).unwrap().to_vec()).unwrap();
    let (w, h) = (options.width, options.height);
    println!(
        "# {w}x{h}, phase {}, seed {}, palette {}, best of {}, {} math",
        options.phase,
        options.seed,
        options.palette,
        options.repeat,
        if PORTABLE { "portable" } else { "system" }
    );
    println!("style\tscene_ms\tshade_ms");
    let (mut scene_total, mut shade_total) = (0.0, 0.0);
    for style in &options.styles {
        let (mut scene_best, mut shade_best) = (Duration::MAX, Duration::MAX);
        for _ in 0..options.repeat {
            let start = Instant::now();
            let scene = build_scene(style, w, h, options.seed, &palette, options.phase)
                .expect("listed style");
            let built = Instant::now();
            let rgb = render(scene.as_ref(), w, h, seed32(options.seed), &palette);
            let shaded = Instant::now();
            if let Err(err) = rgb {
                eprintln!("bench: {style}: {err}");
                return ExitCode::FAILURE;
            }
            scene_best = scene_best.min(built - start);
            shade_best = shade_best.min(shaded - built);
        }
        scene_total += ms(scene_best);
        shade_total += ms(shade_best);
        println!("{style}\t{:.3}\t{:.3}", ms(scene_best), ms(shade_best));
    }
    println!("total\t{scene_total:.3}\t{shade_total:.3}");
    ExitCode::SUCCESS
}
