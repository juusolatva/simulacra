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
