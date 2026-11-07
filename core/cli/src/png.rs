//! Static PNG encoding, as `write_png` in `simulacra.py` does it: 8-bit RGB,
//! filter type 0 (None) on every scanline, and one zlib stream at level 9.
//! Samples are stored unchanged (palette stops are sRGB). The compressed bytes
//! can differ from Python's zlib, but the decoded pixels are the same.

use std::io::Write as _;

use flate2::Compression;
use flate2::write::ZlibEncoder;

const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

/// Why a frame cannot be encoded.
#[derive(Debug)]
pub enum PngError {
    /// PNG stores each side as a 32-bit integer.
    TooWide,
    /// The pixel buffer does not hold `width * height * 3` bytes.
    WrongLength,
    Io(std::io::Error),
}

impl std::fmt::Display for PngError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooWide => f.write_str("PNG width and height cannot exceed 4294967295"),
            Self::WrongLength => f.write_str("pixel buffer must contain width * height * 3 bytes"),
            Self::Io(err) => write!(f, "compressing the image failed: {err}"),
        }
    }
}

fn chunk(out: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]) {
    let length = u32::try_from(data.len()).expect("chunk sizes are checked by the caller");
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(tag);
    out.extend_from_slice(data);
    let mut crc = crc32fast::Hasher::new();
    crc.update(tag);
    crc.update(data);
    out.extend_from_slice(&crc.finalize().to_be_bytes());
}

/// Encodes packed RGB (`width * height * 3` bytes, row-major) as a PNG file.
pub fn encode(rgb: &[u8], width: usize, height: usize) -> Result<Vec<u8>, PngError> {
    let (Ok(w), Ok(h)) = (u32::try_from(width), u32::try_from(height)) else {
        return Err(PngError::TooWide);
    };
    // On a 32-bit target width * 3 can overflow even when width fits in a u32.
    let stride = width.checked_mul(3).ok_or(PngError::TooWide)?;
    if Some(rgb.len()) != stride.checked_mul(height) {
        return Err(PngError::WrongLength);
    }
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    for row in rgb.chunks_exact(stride) {
        encoder.write_all(&[0]).map_err(PngError::Io)?; // filter type 0 (None)
        encoder.write_all(row).map_err(PngError::Io)?;
    }
    let idat = encoder.finish().map_err(PngError::Io)?;
    if u32::try_from(idat.len()).is_err() {
        return Err(PngError::TooWide);
    }
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit, truecolor, deflate, filter 0, no interlace
    let mut out = Vec::with_capacity(idat.len() + 64);
    out.extend_from_slice(SIGNATURE);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &idat);
    chunk(&mut out, b"IEND", &[]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{PngError, encode};
    use std::io::Read as _;

    /// Splits a PNG into `(tag, payload)` chunks, checking each CRC.
    fn chunks(png: &[u8]) -> Vec<([u8; 4], Vec<u8>)> {
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let (mut offset, mut out) = (8, Vec::new());
        while offset < png.len() {
            let size = u32::from_be_bytes(png[offset..offset + 4].try_into().unwrap()) as usize;
            let tag: [u8; 4] = png[offset + 4..offset + 8].try_into().unwrap();
            let data = png[offset + 8..offset + 8 + size].to_vec();
            let crc = u32::from_be_bytes(
                png[offset + 8 + size..offset + 12 + size]
                    .try_into()
                    .unwrap(),
            );
            let mut hasher = crc32fast::Hasher::new();
            hasher.update(&tag);
            hasher.update(&data);
            assert_eq!(crc, hasher.finalize());
            out.push((tag, data));
            offset += size + 12;
        }
        out
    }

    #[test]
    fn encodes_unfiltered_rgb_scanlines() {
        let rgb: Vec<u8> = (0..2 * 3 * 3).map(|i| i as u8 * 13).collect();
        let png = encode(&rgb, 2, 3).unwrap();
        let parts = chunks(&png);
        let tags: Vec<&[u8; 4]> = parts.iter().map(|(t, _)| t).collect();
        assert_eq!(tags, [b"IHDR", b"IDAT", b"IEND"]);
        assert_eq!(parts[0].1, [0, 0, 0, 2, 0, 0, 0, 3, 8, 2, 0, 0, 0]);
        let mut raw = Vec::new();
        flate2::read::ZlibDecoder::new(&parts[1].1[..])
            .read_to_end(&mut raw)
            .unwrap();
        let expected: Vec<u8> = rgb
            .chunks(6)
            .flat_map(|row| [&[0][..], row].concat())
            .collect();
        assert_eq!(raw, expected);
    }

    #[test]
    fn rejects_a_buffer_of_the_wrong_size() {
        assert!(matches!(encode(&[0; 5], 1, 2), Err(PngError::WrongLength)));
    }
}
