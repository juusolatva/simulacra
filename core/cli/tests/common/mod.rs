//! Test-only decoders for what the binary writes: PNG and APNG (8-bit RGB,
//! no interlace) and GIF89a with one global table. They are written apart
//! from the encoders in `src/` (the GIF LZW decoder follows the decoder side
//! of the format, whose code width lags the encoder's by one entry), so a
//! bug shared by an encoder and its decoder is unlikely.
#![allow(dead_code)]

use std::io::Read as _;
use std::path::Path;

use sha2::{Digest, Sha256};

const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// A decoded image or animation.
#[derive(Debug)]
pub struct Decoded {
    pub width: u32,
    pub height: u32,
    /// Packed RGB, one entry per frame.
    pub frames: Vec<Vec<u8>>,
    /// The APNG plays forever (`acTL` plays 0), or the GIF has a NETSCAPE2.0
    /// block with loop count 0.
    pub loops: bool,
    /// Each frame's delay as (numerator, denominator) seconds: an APNG
    /// `fcTL` as stored (a zero denominator read as 100), a GIF graphics
    /// control block as (hundredths, 100). Empty for a still PNG.
    pub delays: Vec<(u16, u16)>,
    /// The GIF's global colour table; `None` for PNG.
    pub table: Option<Vec<[u8; 3]>>,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Decodes `path` as GIF if its name ends in `.gif` (any case, including a
/// file named `.gif`), else as PNG: the binary's `name_wants_gif` rule.
pub fn decode(path: &Path) -> Result<Decoded, String> {
    let bytes = std::fs::read(path).map_err(|err| format!("{}: {err}", path.display()))?;
    let is_gif = path
        .file_name()
        .is_some_and(|n| n.to_string_lossy().to_lowercase().ends_with(".gif"));
    let decoded = if is_gif {
        decode_gif(&bytes)
    } else {
        decode_png(&bytes)
    };
    decoded.map_err(|err| format!("{}: {err}", path.display()))
}

fn be32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes(bytes[..4].try_into().unwrap())
}

fn be16(bytes: &[u8]) -> u16 {
    u16::from_be_bytes(bytes[..2].try_into().unwrap())
}

/// The chunks after the signature, as (tag, body), with every CRC checked
/// and `IEND` required.
/// A chunk's tag and body.
type Chunk<'a> = ([u8; 4], &'a [u8]);

fn chunks(data: &[u8]) -> Result<Vec<Chunk<'_>>, String> {
    if !data.starts_with(PNG_SIGNATURE) {
        return Err("not a PNG file".into());
    }
    let mut pos = PNG_SIGNATURE.len();
    let mut out = Vec::new();
    while pos + 12 <= data.len() {
        let length = be32(&data[pos..]) as usize;
        let end = pos + 12 + length;
        if end > data.len() {
            break;
        }
        let tag: [u8; 4] = data[pos + 4..pos + 8].try_into().unwrap();
        let body = &data[pos + 8..pos + 8 + length];
        let mut crc = crc32fast::Hasher::new();
        crc.update(&tag);
        crc.update(body);
        if crc.finalize() != be32(&data[pos + 8 + length..]) {
            return Err(format!("bad CRC in {}", String::from_utf8_lossy(&tag)));
        }
        out.push((tag, body));
        pos = end;
        if &tag == b"IEND" {
            return Ok(out);
        }
    }
    Err("missing IEND".into())
}

/// Collects an image's compressed frames chunk by chunk. Frame 0 is the
/// `IDAT` image, which in an APNG must be the first animation frame.
#[derive(Default)]
struct FrameReader {
    header: Option<(u32, u32, [u8; 5])>,
    frame_count: Option<u32>,
    loops: bool,
    delays: Vec<(u16, u16)>,
    sequence: u32,
    controls: u32,
    frames: Vec<Vec<u8>>,
}

impl FrameReader {
    fn add(&mut self, tag: &[u8; 4], body: &[u8]) -> Result<(), String> {
        // The fixed sizes of the chunks whose fields are read below.
        let minimum = match tag {
            b"IHDR" => 13,
            b"acTL" => 8,
            b"fcTL" => 26,
            b"fdAT" => 4,
            _ => 0,
        };
        if body.len() < minimum {
            return Err(format!("{} is too short", String::from_utf8_lossy(tag)));
        }
        match tag {
            b"IHDR" => {
                if body.len() != 13 {
                    return Err("IHDR has the wrong length".into());
                }
                self.header = Some((
                    be32(body),
                    be32(&body[4..]),
                    body[8..13].try_into().unwrap(),
                ));
            }
            b"acTL" => {
                self.frame_count = Some(be32(body));
                self.loops = be32(&body[4..]) == 0;
            }
            b"IDAT" => {
                if self.frame_count.is_some() && self.controls != 1 {
                    return Err("the APNG's IDAT image is not its first frame".into());
                }
                self.frames[0].extend_from_slice(body);
            }
            b"fcTL" | b"fdAT" => self.animation_chunk(tag, body)?,
            _ => {}
        }
        Ok(())
    }

    fn animation_chunk(&mut self, tag: &[u8; 4], body: &[u8]) -> Result<(), String> {
        let name = String::from_utf8_lossy(tag);
        if self.frame_count.is_none() {
            return Err(format!("{name} without acTL"));
        }
        if be32(body) != self.sequence {
            return Err(format!("{name} out of sequence"));
        }
        self.sequence += 1;
        if tag == b"fdAT" {
            if self.controls < 2 {
                return Err("fdAT outside a frame after the first".into());
            }
            self.frames
                .last_mut()
                .unwrap()
                .extend_from_slice(&body[4..]);
            return Ok(());
        }
        let (width, height, _) = self.header.ok_or("fcTL before IHDR")?;
        if (be32(&body[4..]), be32(&body[8..])) != (width, height)
            || be32(&body[12..]) != 0
            || be32(&body[16..]) != 0
        {
            return Err("an APNG frame does not cover the canvas".into());
        }
        let (numerator, denominator) = (be16(&body[20..]), be16(&body[22..]));
        self.delays
            .push((numerator, if denominator == 0 { 100 } else { denominator }));
        self.controls += 1;
        if self.controls > 1 {
            self.frames.push(Vec::new());
        }
        Ok(())
    }
}

/// Decodes a PNG or APNG; see `Decoded`.
pub fn decode_png(data: &[u8]) -> Result<Decoded, String> {
    let mut reader = FrameReader {
        frames: vec![Vec::new()],
        ..FrameReader::default()
    };
    for (tag, body) in chunks(data)? {
        reader.add(&tag, body)?;
    }
    let (width, height, rest) = reader.header.ok_or("missing IHDR")?;
    if rest != [8, 2, 0, 0, 0] {
        return Err(format!(
            "expected 8-bit RGB without interlace, got {rest:?}"
        ));
    }
    if let Some(count) = reader.frame_count {
        if reader.controls == 0 {
            return Err("acTL without fcTL".into());
        }
        if count as usize != reader.frames.len() {
            return Err(format!(
                "acTL announces {count} frames, found {}",
                reader.frames.len()
            ));
        }
    }
    if reader.frames.iter().any(Vec::is_empty) {
        return Err("a frame has no image data".into());
    }
    let stride = width as usize * 3;
    let frames = reader
        .frames
        .iter()
        .map(|compressed| {
            let mut raw = Vec::new();
            flate2::read::ZlibDecoder::new(&compressed[..])
                .read_to_end(&mut raw)
                .map_err(|err| format!("inflate: {err}"))?;
            if raw.len() != height as usize * (stride + 1) {
                return Err("image data has the wrong length".to_string());
            }
            unfilter(&raw, stride)
        })
        .collect::<Result<_, _>>()?;
    Ok(Decoded {
        width,
        height,
        frames,
        loops: reader.loops,
        delays: reader.delays,
        table: None,
    })
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

/// Undoes each scanline's filter (PNG specification, section 9).
fn unfilter(raw: &[u8], stride: usize) -> Result<Vec<u8>, String> {
    let mut out: Vec<u8> = Vec::with_capacity(raw.len());
    let mut prior = vec![0u8; stride];
    for line in raw.chunks(stride + 1) {
        let kind = line[0];
        let mut row = line[1..].to_vec();
        for i in 0..stride {
            let left = if i >= 3 { row[i - 3] } else { 0 };
            let corner = if i >= 3 { prior[i - 3] } else { 0 };
            let up = prior[i];
            let predicted = match kind {
                0 => 0,
                1 => left,
                2 => up,
                3 => ((u16::from(left) + u16::from(up)) / 2) as u8,
                4 => paeth(left, up, corner),
                _ => return Err(format!("unknown filter type {kind}")),
            };
            row[i] = row[i].wrapping_add(predicted);
        }
        out.extend_from_slice(&row);
        prior = row;
    }
    Ok(out)
}

/// Decodes GIF LZW data with minimum code size 8 into colour indices.
fn lzw_decode(data: &[u8]) -> Result<Vec<u8>, String> {
    const CLEAR: usize = 256;
    const END: usize = 257;
    let fresh = || -> Vec<Vec<u8>> {
        (0..=255u8)
            .map(|i| vec![i])
            .chain([Vec::new(), Vec::new()])
            .collect()
    };
    let mut table = fresh();
    let (mut width, mut bits, mut buffer, mut pos) = (9u32, 0u32, 0u32, 0usize);
    let mut previous: Option<Vec<u8>> = None;
    let mut out = Vec::new();
    loop {
        while bits < width {
            let byte = *data.get(pos).ok_or("LZW data ends without an end code")?;
            buffer |= u32::from(byte) << bits;
            pos += 1;
            bits += 8;
        }
        let code = (buffer & ((1 << width) - 1)) as usize;
        buffer >>= width;
        bits -= width;
        if code == CLEAR {
            table = fresh();
            width = 9;
            previous = None;
            continue;
        }
        if code == END {
            return Ok(out);
        }
        let entry = if code < table.len() {
            table[code].clone()
        } else if let (Some(prev), true) = (&previous, code == table.len()) {
            let mut e = prev.clone();
            e.push(prev[0]);
            e
        } else {
            return Err(format!("LZW code {code} is not in the table"));
        };
        if let Some(prev) = &previous
            && table.len() < 4096
        {
            let mut e = prev.clone();
            e.push(entry[0]);
            table.push(e);
            if table.len() == 1 << width && width < 12 {
                width += 1;
            }
        }
        out.extend_from_slice(&entry);
        previous = Some(entry);
    }
}

/// The data sub-blocks starting at `pos`, joined, and the position after them.
fn sub_blocks(data: &[u8], mut pos: usize) -> Result<(Vec<u8>, usize), String> {
    let mut joined = Vec::new();
    loop {
        let size = usize::from(*data.get(pos).ok_or("GIF ends inside a block")?);
        if size == 0 {
            return Ok((joined, pos + 1));
        }
        joined.extend_from_slice(
            data.get(pos + 1..pos + 1 + size)
                .ok_or("GIF ends inside a block")?,
        );
        pos += 1 + size;
    }
}

fn le16(bytes: &[u8]) -> u16 {
    u16::from_le_bytes(bytes[..2].try_into().unwrap())
}

/// Decodes a GIF89a with a global table of 256 colours and full-canvas
/// frames without local tables or interlacing, as the binary writes them.
pub fn decode_gif(data: &[u8]) -> Result<Decoded, String> {
    if data.len() < 13 + 768 || &data[..6] != b"GIF89a" {
        return Err("not a GIF89a file".into());
    }
    let (width, height) = (le16(&data[6..]), le16(&data[8..]));
    if data[10] != 0xF7 {
        return Err("expected a global table of 256 colours".into());
    }
    let table: Vec<[u8; 3]> = data[13..13 + 768]
        .chunks(3)
        .map(|c| [c[0], c[1], c[2]])
        .collect();
    let mut pos = 13 + 768;
    let (mut frames, mut loops, mut delays) = (Vec::new(), false, Vec::new());
    loop {
        match data.get(pos) {
            Some(0x3B) => break,
            Some(0x21) => {
                let label = *data.get(pos + 1).ok_or("GIF ends inside an extension")?;
                let (payload, next) = sub_blocks(data, pos + 2)?;
                if label == 0xF9 && payload.len() >= 3 {
                    delays.push((le16(&payload[1..]), 100));
                } else if label == 0xFF && payload.starts_with(b"NETSCAPE2.0") {
                    loops = payload.get(11..14) == Some(&[1, 0, 0]);
                }
                pos = next;
            }
            Some(0x2C) => {
                let descriptor = data
                    .get(pos + 1..pos + 11)
                    .ok_or("GIF ends inside an image")?;
                let geometry = [
                    le16(descriptor),
                    le16(&descriptor[2..]),
                    le16(&descriptor[4..]),
                    le16(&descriptor[6..]),
                ];
                if geometry != [0, 0, width, height] || descriptor[8] & 0xC0 != 0 {
                    return Err(
                        "expected full-canvas GIF frames without a local table or interlace".into(),
                    );
                }
                if descriptor[9] != 8 {
                    return Err("expected LZW minimum code size 8".into());
                }
                let (packed, next) = sub_blocks(data, pos + 11)?;
                let indices = lzw_decode(&packed)?;
                if indices.len() != usize::from(width) * usize::from(height) {
                    return Err("a GIF frame has the wrong number of pixels".into());
                }
                frames.push(
                    indices
                        .iter()
                        .flat_map(|&i| table[usize::from(i)])
                        .collect(),
                );
                pos = next;
            }
            Some(other) => return Err(format!("unexpected GIF block {other:#x}")),
            None => return Err("GIF ends without a trailer".into()),
        }
    }
    Ok(Decoded {
        width: u32::from(width),
        height: u32::from(height),
        frames,
        loops,
        delays,
        table: Some(table),
    })
}
