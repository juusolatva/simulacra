//! The `simulacra` binary as a user runs it: options, messages, exit
//! statuses and the files it writes, read back with the test decoders in
//! `common`.

mod common;

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use common::{decode_gif, decode_png};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_simulacra"))
}

/// A fresh, empty directory under Cargo's temporary directory for tests.
fn tempdir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("cli")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn path(p: &Path) -> &str {
    p.to_str().expect("UTF-8 path")
}

fn run(args: &[&str]) -> Output {
    bin().args(args).output().expect("run simulacra")
}

fn run_ok(args: &[&str]) -> String {
    let out = run(args);
    assert!(
        out.status.success(),
        "simulacra {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

fn still_png(width: u32, height: u32) -> Vec<u8> {
    let dir = tempdir(&format!("still-{width}x{height}"));
    let out = dir.join("a.png");
    run_ok(&[
        "--width",
        &width.to_string(),
        "--height",
        &height.to_string(),
        "--out",
        path(&out),
    ]);
    fs::read(out).unwrap()
}

#[test]
fn decoders_read_what_the_binary_writes() {
    let dir = tempdir("decoders");
    let png = dir.join("a.png");
    let gif = dir.join("a.gif");
    let common = [
        "--style", "plasma", "--width", "7", "--height", "5", "--frames", "3",
    ];
    run_ok(&[&common[..], &["--out", path(&png)]].concat());
    run_ok(&[&common[..], &["--out", path(&gif)]].concat());
    let p = decode_png(&fs::read(&png).unwrap()).unwrap();
    let g = decode_gif(&fs::read(&gif).unwrap()).unwrap();
    assert_eq!((p.width, p.height, p.frames.len()), (7, 5, 3));
    assert_eq!((g.width, g.height, g.frames.len()), (7, 5, 3));
    assert!(
        p.frames
            .iter()
            .chain(&g.frames)
            .all(|f| f.len() == 7 * 5 * 3)
    );
    assert_ne!(p.frames[0], p.frames[1]);
}

#[test]
fn png_decoder_rejects_a_bad_crc_and_a_missing_iend() {
    let bytes = still_png(2, 2);
    let mut bad = bytes.clone();
    bad[29] ^= 1; // IHDR's CRC: 8 signature + 8 length and tag + 13 body
    assert!(decode_png(&bad).unwrap_err().contains("CRC"));
    assert!(
        decode_png(&bytes[..bytes.len() - 12])
            .unwrap_err()
            .contains("IEND")
    );
}

// Hand-built files, written here without the binary's encoders.

fn chunk(out: &mut Vec<u8>, tag: &[u8; 4], body: &[u8]) {
    out.extend((body.len() as u32).to_be_bytes());
    out.extend(tag);
    out.extend(body);
    let mut crc = crc32fast::Hasher::new();
    crc.update(tag);
    crc.update(body);
    out.extend(crc.finalize().to_be_bytes());
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = i16::from(a) + i16::from(b) - i16::from(c);
    let (pa, pb, pc) = (
        (p - i16::from(a)).abs(),
        (p - i16::from(b)).abs(),
        (p - i16::from(c)).abs(),
    );
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

/// Filters `row` with `kind` (PNG specification, section 9), 3 bytes per pixel.
fn filter(kind: u8, row: &[u8], prior: &[u8]) -> Vec<u8> {
    (0..row.len())
        .map(|i| {
            let left = if i >= 3 { row[i - 3] } else { 0 };
            let corner = if i >= 3 { prior[i - 3] } else { 0 };
            let up = prior[i];
            let predicted = match kind {
                0 => 0,
                1 => left,
                2 => up,
                3 => ((u16::from(left) + u16::from(up)) / 2) as u8,
                _ => paeth(left, up, corner),
            };
            row[i].wrapping_sub(predicted)
        })
        .collect()
}

#[test]
fn decoders_return_known_pixels_from_hand_built_files() {
    // PNG: 3 x 5, rows filtered with types 0 to 4 in turn.
    let (width, height) = (3usize, 5usize);
    let pixels: Vec<u8> = (0..width * height * 3)
        .map(|i| (i * 37 % 251) as u8)
        .collect();
    let mut raw = Vec::new();
    let mut prior = vec![0; width * 3];
    for (y, row) in pixels.chunks(width * 3).enumerate() {
        raw.push(y as u8);
        raw.extend(filter(y as u8, row, &prior));
        prior = row.to_vec();
    }
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    z.write_all(&raw).unwrap();
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = Vec::new();
    header.extend((width as u32).to_be_bytes());
    header.extend((height as u32).to_be_bytes());
    header.extend([8, 2, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &header);
    chunk(&mut png, b"IDAT", &z.finish().unwrap());
    chunk(&mut png, b"IEND", &[]);
    let decoded = decode_png(&png).unwrap();
    assert_eq!((decoded.width, decoded.height), (3, 5));
    assert_eq!(decoded.frames, vec![pixels]);

    // GIF: literal 9-bit codes, with a clear code every 254 codes so the
    // decoder's table never reaches 512 entries and the width stays 9 bits.
    let table: Vec<[u8; 3]> = (0..=255u8).map(|i| [i, 255 - i, i / 2]).collect();
    let small = [0u8, 1, 2, 255, 7, 7, 7, 7, 128, 3, 9, 200];
    let long: Vec<u8> = (0..600u32).map(|i| (i * 7 % 256) as u8).collect();
    for (w, h, indices) in [(4u16, 3u16, &small[..]), (30, 20, &long[..])] {
        let gif = hand_gif(w, h, &table, indices);
        let decoded = decode_gif(&gif).unwrap();
        let expected: Vec<u8> = indices
            .iter()
            .flat_map(|&i| table[usize::from(i)])
            .collect();
        assert_eq!(
            (decoded.width, decoded.height),
            (u32::from(w), u32::from(h))
        );
        assert_eq!(decoded.frames, vec![expected]);
        assert_eq!(decoded.table.as_deref(), Some(&table[..]));
    }
}

fn hand_gif(width: u16, height: u16, table: &[[u8; 3]], indices: &[u8]) -> Vec<u8> {
    let mut codes = vec![256u16];
    for (n, &i) in indices.iter().enumerate() {
        if n > 0 && n % 254 == 0 {
            codes.push(256);
        }
        codes.push(u16::from(i));
    }
    codes.push(257);
    let (mut packed, mut buffer, mut bits) = (Vec::new(), 0u32, 0);
    for code in codes {
        buffer |= u32::from(code) << bits;
        bits += 9;
        while bits >= 8 {
            packed.push(buffer as u8);
            buffer >>= 8;
            bits -= 8;
        }
    }
    if bits > 0 {
        packed.push(buffer as u8);
    }
    let mut gif = b"GIF89a".to_vec();
    gif.extend(width.to_le_bytes());
    gif.extend(height.to_le_bytes());
    gif.extend([0xF7, 0, 0]);
    gif.extend(table.iter().flatten());
    gif.push(0x2C);
    gif.extend([0, 0, 0, 0]);
    gif.extend(width.to_le_bytes());
    gif.extend(height.to_le_bytes());
    gif.extend([0, 8]);
    for block in packed.chunks(255) {
        gif.push(block.len() as u8);
        gif.extend(block);
    }
    gif.extend([0, 0x3B]);
    gif
}

#[test]
fn gif_lzw_decoder_handles_table_resets() {
    // A noisy 300 x 200 frame needs far more than 4096 codes.
    let dir = tempdir("lzw");
    let gif = dir.join("big.gif");
    run_ok(&[
        "--style",
        "nebula",
        "--width",
        "300",
        "--height",
        "200",
        "--out",
        path(&gif),
    ]);
    let g = decode_gif(&fs::read(&gif).unwrap()).unwrap();
    assert_eq!(g.frames[0].len(), 300 * 200 * 3);
}

// Options, messages and exit statuses.

use simulacra_engine::{PALETTES, SCENE_BUILDERS, palette_stops};

/// Runs the binary in `dir` (so a relative `--out` lands there).
fn run_in(dir: &Path, args: &[&str]) -> Output {
    bin()
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run simulacra")
}

fn stdout(out: &Output) -> &str {
    std::str::from_utf8(&out.stdout).unwrap()
}

fn stderr(out: &Output) -> &str {
    std::str::from_utf8(&out.stderr).unwrap()
}

fn pixels(file: &Path) -> Vec<Vec<u8>> {
    common::decode(file).unwrap().frames
}

#[test]
fn lists_are_sorted_and_complete() {
    let styles: Vec<String> = run_ok(&["--list-styles"])
        .lines()
        .map(String::from)
        .collect();
    let mut sorted = styles.clone();
    sorted.sort();
    assert_eq!(styles, sorted);
    assert_eq!(styles, SCENE_BUILDERS);
    assert_eq!(styles.len(), 48);
    assert!(styles.iter().any(|s| s == "waves"));
    let palettes: Vec<String> = run_ok(&["--list-palettes"])
        .lines()
        .map(String::from)
        .collect();
    let names: Vec<&str> = PALETTES.iter().map(|(name, _)| *name).collect();
    assert_eq!(palettes, names);
    let mut sorted = palettes.clone();
    sorted.sort();
    assert_eq!(palettes, sorted);
    assert_eq!(palettes.len(), 24);
}

#[test]
fn defaults_write_art_png_256_square() {
    let dir = tempdir("defaults");
    let out = run_in(&dir, &[]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(
        stdout(&out),
        "wrote art.png (waves, seed=1, palette=deep)\n"
    );
    assert_eq!(stderr(&out), "");
    let image = common::decode(&dir.join("art.png")).unwrap();
    assert_eq!(
        (image.width, image.height, image.frames.len()),
        (256, 256, 1)
    );
    assert!(image.delays.is_empty());
    assert!(!image.loops);
}

#[test]
fn animation_reports_frames_and_fps_and_stores_delays() {
    let dir = tempdir("animation");
    let small = ["--width", "16", "--height", "12"];
    let out = run_in(
        &dir,
        &[
            &small[..],
            &[
                "--style", "waves", "--frames", "4", "--fps", "30", "--out", "a.png",
            ],
        ]
        .concat(),
    );
    assert_eq!(
        stdout(&out),
        "wrote 4 animation frames to a.png (waves, seed=1, palette=deep, 30 fps)\n"
    );
    let apng = common::decode(&dir.join("a.png")).unwrap();
    assert_eq!(apng.frames.len(), 4);
    assert!(apng.loops);
    assert_eq!(apng.delays, [(1, 30); 4]);
    assert!(apng.frames.windows(2).any(|w| w[0] != w[1]));
    for (fps, hundredths) in [("30", 3), ("12", 8), ("65535", 2)] {
        run_in(
            &dir,
            &[
                &small[..],
                &["--frames", "4", "--fps", fps, "--out", "a.gif"],
            ]
            .concat(),
        );
        let gif = common::decode(&dir.join("a.gif")).unwrap();
        assert_eq!(gif.frames.len(), 4, "fps {fps}");
        assert!(gif.loops);
        assert_eq!(gif.delays, [(hundredths, 100); 4], "fps {fps}");
    }
    run_in(&dir, &[&small[..], &["--out", "still.gif"]].concat());
    let still = common::decode(&dir.join("still.gif")).unwrap();
    assert_eq!(still.frames.len(), 1);
    assert!(!still.loops);
    assert_eq!(still.delays, [(8, 100)]);
}

#[test]
fn gif_follows_the_suffix_in_any_case() {
    let dir = tempdir("suffix");
    for name in ["a.GIF", "b.Gif", "c.png.gif", "d.gifx"] {
        let out = run_in(&dir, &["--width", "4", "--height", "4", "--out", name]);
        assert!(out.status.success(), "{name}: {}", stderr(&out));
        let bytes = fs::read(dir.join(name)).unwrap();
        let expect_gif = name != "d.gifx";
        assert_eq!(bytes.starts_with(b"GIF89a"), expect_gif, "{name}");
        assert_eq!(bytes.starts_with(b"\x89PNG"), !expect_gif, "{name}");
    }
}

#[test]
fn gif_pixels_stay_close_to_the_source_rgb() {
    let dir = tempdir("gif-colors");
    let args = [
        "--style",
        "plasma",
        "--width",
        "48",
        "--height",
        "32",
        "--seed",
        "4",
        "--palette",
        "sunset",
        "--frames",
        "3",
        "--fps",
        "10",
    ];
    run_in(&dir, &[&args[..], &["--out", "a.png"]].concat());
    run_in(&dir, &[&args[..], &["--out", "a.gif"]].concat());
    let source = common::decode(&dir.join("a.png")).unwrap();
    let gif = common::decode(&dir.join("a.gif")).unwrap();
    assert_eq!((gif.frames.len(), gif.loops), (3, true));
    assert_eq!(gif.delays, [(10, 100); 3]);
    for (rgb, mapped) in source.frames.iter().zip(&gif.frames) {
        let worst = rgb
            .iter()
            .zip(mapped)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap();
        assert!(worst <= 24, "a GIF channel is {worst} from the source");
    }
    let table = gif.table.unwrap();
    assert!(table[240..].iter().all(|&[r, g, b]| r == g && g == b));
}

#[test]
fn random_seed_is_reported_and_reproducible() {
    let dir = tempdir("random-seed");
    let small = ["--width", "12", "--height", "9"];
    let out = run_in(
        &dir,
        &[&small[..], &["--random-seed", "--out", "x.png"]].concat(),
    );
    let line = stdout(&out);
    let seed = line
        .strip_prefix("wrote x.png (waves, seed=")
        .and_then(|rest| rest.strip_suffix(", palette=deep)\n"))
        .unwrap_or_else(|| panic!("unexpected output {line:?}"));
    seed.parse::<i64>().expect("a whole-number seed");
    run_in(
        &dir,
        &[&small[..], &["--seed", seed, "--out", "y.png"]].concat(),
    );
    assert_eq!(pixels(&dir.join("x.png")), pixels(&dir.join("y.png")));
}

fn hex_stops(name: &str) -> String {
    palette_stops(name)
        .unwrap()
        .iter()
        .map(|(r, g, b)| format!("{r:02x}{g:02x}{b:02x}"))
        .collect::<Vec<_>>()
        .join(",")
}

#[test]
fn palette_stops_render_like_the_built_in_palette() {
    let dir = tempdir("palette-stops");
    let small = ["--style", "blobs", "--width", "40", "--height", "30"];
    for name in ["deep", "sunset"] {
        run_in(
            &dir,
            &[&small[..], &["--palette", name, "--out", "named.png"]].concat(),
        );
        let stops = hex_stops(name);
        let out = run_in(
            &dir,
            &[
                &small[..],
                &["--palette-stops", &stops, "--out", "custom.png"],
            ]
            .concat(),
        );
        assert_eq!(
            stdout(&out),
            "wrote custom.png (blobs, seed=1, palette=custom)\n"
        );
        assert_eq!(
            pixels(&dir.join("named.png")),
            pixels(&dir.join("custom.png")),
            "{name}"
        );
    }
    let out = run_in(
        &dir,
        &[
            &small[..],
            &[
                "--palette-stops",
                "#0c061c,5814a8,f80",
                "--out",
                "short.png",
            ],
        ]
        .concat(),
    );
    assert!(out.status.success(), "{}", stderr(&out));
}

#[test]
fn repeated_options_take_their_last_value() {
    let dir = tempdir("repeated");
    let out = run_in(
        &dir,
        &[
            "--width", "8", "--width", "20", "--height", "12", "--style", "blobs", "--style",
            "plasma", "--out", "a.png",
        ],
    );
    assert_eq!(stdout(&out), "wrote a.png (plasma, seed=1, palette=deep)\n");
    run_in(
        &dir,
        &[
            "--width", "20", "--height", "12", "--style", "plasma", "--out", "b.png",
        ],
    );
    let image = common::decode(&dir.join("a.png")).unwrap();
    assert_eq!((image.width, image.height), (20, 12));
    assert_eq!(image.frames, pixels(&dir.join("b.png")));
}

#[test]
fn refused_arguments_exit_2_with_their_message() {
    let big = (1u64 << 32).to_string();
    let seed = ((1u64 << 63) - 2).to_string();
    let apng_frames = ((1u64 << 31) + 1).to_string();
    let cases: &[(&[&str], &str)] = &[
        (&["--width", "0"], "width and height must be positive"),
        (
            &["--width", "10001", "--height", "10000"],
            "the limit is 100000000",
        ),
        (
            &["--width", "1", "--height", "100000000"],
            "1 x 100000000 has a side over 100000 pixels",
        ),
        (
            &["--no-limits", "--width", &big, "--height", "1"],
            "cannot exceed 4294967295",
        ),
        (&["--phase", "inf"], "phase must be a finite number"),
        (&["--frames", "0"], "frames must be positive"),
        (
            &["--width", "0", "--frames", "0"],
            "width and height must be positive",
        ),
        (
            &["--slideshow", "--frames", "4", "--seed", &seed],
            "--slideshow seeds would pass 9223372036854775807",
        ),
        (
            &["--width", "1", "--height", "1", "--frames", "65537"],
            "65537 frames; the limit is 65536",
        ),
        (
            &[
                "--no-limits",
                "--width",
                "1",
                "--height",
                "1",
                "--frames",
                &apng_frames,
            ],
            "an APNG cannot hold more than 2147483648 frames",
        ),
        (&["--fps", "0"], "fps must be between 1 and 65535"),
        (&["--fps", "65536"], "fps must be between 1 and 65535"),
        (
            &["--width", "1920", "--height", "1080", "--frames", "2072"],
            "2072 frames of 1920 x 1080 are 4296499200 pixels in all; the limit is 4294967296",
        ),
        (&["--palette-stops", "ff0000"], "at least two colors"),
        (
            &["--out", "image.gif", "--width", "65536"],
            "GIF width and height cannot exceed 65535 pixels",
        ),
        (&["--style", "no-such-style"], "no-such-style"),
        (&["--seed", "3", "--random-seed"], "cannot be used with"),
    ];
    let dir = tempdir("refused");
    for (args, message) in cases {
        let out = run_in(&dir, args);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {}", stderr(&out));
        assert!(stderr(&out).contains(message), "{args:?}: {}", stderr(&out));
        assert_eq!(stdout(&out), "", "{args:?}");
        assert_eq!(
            fs::read_dir(&dir).unwrap().count(),
            0,
            "{args:?} left a file"
        );
    }
}

#[test]
fn failed_writes_exit_1_and_leave_no_trace() {
    let dir = tempdir("failed-writes");
    let missing = dir.join("missing").join("art.png");
    let missing_gif = dir.join("missing").join("art.gif");
    let small = ["--width", "4", "--height", "4"];
    for extra in [&[][..], &["--frames", "2"], &["--out", path(&missing_gif)]] {
        let out = run(&[&small[..], &["--out", path(&missing)], extra].concat());
        assert_eq!(out.status.code(), Some(1), "{extra:?}");
        assert!(
            stderr(&out).contains("simulacra: error: cannot write"),
            "{}",
            stderr(&out)
        );
        assert_eq!(stdout(&out), "");
    }
    assert!(!dir.join("missing").exists());

    // The final rename onto an existing directory fails: what was there stays,
    // and the temporary file is removed.
    for (name, extra) in [
        ("keep.png", &[][..]),
        ("keep.png", &["--frames", "2"]),
        ("keep.gif", &[]),
    ] {
        let target = dir.join(name);
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("keep.txt"), "kept").unwrap();
        let out = run(&[&small[..], &["--out", path(&target)], extra].concat());
        assert_eq!(
            out.status.code(),
            Some(1),
            "{name} {extra:?}: {}",
            stderr(&out)
        );
        assert!(
            stderr(&out).contains("simulacra: error: cannot write"),
            "{}",
            stderr(&out)
        );
        assert_eq!(fs::read_to_string(target.join("keep.txt")).unwrap(), "kept");
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .filter(|n| n.starts_with(".simulacra-"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }
}

#[test]
fn decode_follows_the_binarys_gif_rule_for_a_leading_dot_name() {
    // The binary writes GIF for any name ending in .gif, including ".gif".
    let dir = tempdir("dot-gif");
    run_in(&dir, &["--width", "4", "--height", "3", "--out", ".gif"]);
    let decoded = common::decode(&dir.join(".gif")).unwrap();
    assert!(decoded.table.is_some());
}

#[test]
fn png_decoder_rejects_short_animation_chunks_without_panicking() {
    let mut header = Vec::new();
    header.extend(2u32.to_be_bytes());
    header.extend(2u32.to_be_bytes());
    header.extend([8, 2, 0, 0, 0]);
    for (tag, body) in [
        (b"acTL", &[0u8; 3][..]),
        (b"fcTL", &[0u8; 3][..]),
        (b"fcTL", &[0u8; 10][..]),
    ] {
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        chunk(&mut png, b"IHDR", &header);
        if tag != b"acTL" {
            chunk(&mut png, b"acTL", &[0, 0, 0, 1, 0, 0, 0, 0]);
        }
        chunk(&mut png, tag, body);
        chunk(&mut png, b"IEND", &[]);
        let err = decode_png(&png).unwrap_err();
        assert!(err.contains("too short"), "{err}");
    }
}
