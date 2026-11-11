//! PNG and APNG encoding, as `write_png` and `write_animated_png` in
//! `simulacra.py` do it: 8-bit RGB, filter type 0 (None) on every scanline, and
//! one zlib stream at level 9 per frame. Samples are stored unchanged (palette
//! stops are sRGB). The compressed bytes can differ from Python's zlib, but the
//! decoded pixels are the same.
//!
//! The encoder does no I/O: [`encode`] returns a whole PNG file, and [`Apng`]
//! returns an animation piece by piece, so a caller can write each frame as
//! soon as it is rendered and keep only one frame in memory.

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
    /// An APNG got a frame count, frame rate or number of frames it cannot hold.
    FrameCount,
    Io(std::io::Error),
}

impl std::fmt::Display for PngError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooWide => f.write_str("PNG width and height cannot exceed 4294967295"),
            Self::WrongLength => f.write_str("pixel buffer must contain width * height * 3 bytes"),
            Self::FrameCount => f.write_str(
                "an APNG needs 1 to 2147483648 frames, all of them added, and a frame rate of 1 to 65535",
            ),
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

/// The signature and `IHDR` chunk for an 8-bit RGB image.
fn header(width: usize, height: usize) -> Result<Vec<u8>, PngError> {
    let (Ok(w), Ok(h)) = (u32::try_from(width), u32::try_from(height)) else {
        return Err(PngError::TooWide);
    };
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit, truecolor, deflate, filter 0, no interlace
    let mut out = Vec::with_capacity(64);
    out.extend_from_slice(SIGNATURE);
    chunk(&mut out, b"IHDR", &ihdr);
    Ok(out)
}

/// Compresses packed RGB (`width * height * 3` bytes, row-major) into the
/// payload of an `IDAT` chunk, with room for `reserve` more bytes in a chunk.
fn compress(rgb: &[u8], width: usize, height: usize, reserve: usize) -> Result<Vec<u8>, PngError> {
    if u32::try_from(width).is_err() || u32::try_from(height).is_err() {
        return Err(PngError::TooWide);
    }
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
    let data = encoder.finish().map_err(PngError::Io)?;
    if data
        .len()
        .checked_add(reserve)
        .is_none_or(|size| u32::try_from(size).is_err())
    {
        return Err(PngError::TooWide);
    }
    Ok(data)
}

/// Encodes packed RGB (`width * height * 3` bytes, row-major) as a PNG file.
pub fn encode(rgb: &[u8], width: usize, height: usize) -> Result<Vec<u8>, PngError> {
    let mut out = header(width, height)?;
    let idat = compress(rgb, width, height, 0)?;
    out.reserve(idat.len() + 24);
    chunk(&mut out, b"IDAT", &idat);
    chunk(&mut out, b"IEND", &[]);
    Ok(out)
}

/// A looping APNG built one frame at a time, laid out as `write_animated_png`
/// writes it: `acTL` after `IHDR`, then for each frame an `fcTL` (full canvas,
/// delay `1 / fps` seconds, no disposal, source blending) followed by `IDAT`
/// for the first frame and `fdAT` for the others, sharing one sequence count.
pub struct Apng {
    width: usize,
    height: usize,
    frames: u32,
    fps: u16,
    written: u32,
    sequence: u32,
}

impl Apng {
    /// Starts an animation of `frames` frames; returns it with the bytes that
    /// begin the file (signature, `IHDR` and `acTL`).
    pub fn new(
        width: usize,
        height: usize,
        frames: u32,
        fps: u16,
    ) -> Result<(Self, Vec<u8>), PngError> {
        // Frame i > 0 uses sequence numbers 2i - 1 and 2i, which must fit a u32.
        if frames == 0 || frames > 1 << 31 || fps == 0 {
            return Err(PngError::FrameCount);
        }
        let mut out = header(width, height)?;
        let mut actl = Vec::with_capacity(8);
        actl.extend_from_slice(&frames.to_be_bytes());
        actl.extend_from_slice(&0u32.to_be_bytes()); // loop forever
        chunk(&mut out, b"acTL", &actl);
        let apng = Self {
            width,
            height,
            frames,
            fps,
            written: 0,
            sequence: 0,
        };
        Ok((apng, out))
    }

    /// Encodes the next frame from packed RGB and returns its chunks.
    pub fn frame(&mut self, rgb: &[u8]) -> Result<Vec<u8>, PngError> {
        if self.written == self.frames {
            return Err(PngError::FrameCount);
        }
        let first = self.written == 0;
        let data = compress(rgb, self.width, self.height, if first { 0 } else { 4 })?;
        let mut fctl = Vec::with_capacity(26);
        fctl.extend_from_slice(&self.sequence.to_be_bytes());
        for value in [self.width, self.height] {
            let value = u32::try_from(value).expect("checked by header");
            fctl.extend_from_slice(&value.to_be_bytes());
        }
        fctl.extend_from_slice(&[0; 8]); // x and y offsets
        fctl.extend_from_slice(&1u16.to_be_bytes());
        fctl.extend_from_slice(&self.fps.to_be_bytes());
        fctl.extend_from_slice(&[0, 0]); // dispose none, blend source
        let mut out = Vec::with_capacity(data.len() + 64);
        chunk(&mut out, b"fcTL", &fctl);
        self.sequence += 1;
        if first {
            chunk(&mut out, b"IDAT", &data);
        } else {
            let mut fdat = Vec::with_capacity(data.len() + 4);
            fdat.extend_from_slice(&self.sequence.to_be_bytes());
            fdat.extend_from_slice(&data);
            chunk(&mut out, b"fdAT", &fdat);
            self.sequence += 1;
        }
        self.written += 1;
        Ok(out)
    }

    /// Returns the closing `IEND` chunk once every frame has been added.
    pub fn finish(self) -> Result<Vec<u8>, PngError> {
        if self.written != self.frames {
            return Err(PngError::FrameCount);
        }
        let mut out = Vec::with_capacity(12);
        chunk(&mut out, b"IEND", &[]);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::{Apng, PngError, encode};
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

    fn inflate(data: &[u8]) -> Vec<u8> {
        let mut raw = Vec::new();
        flate2::read::ZlibDecoder::new(data)
            .read_to_end(&mut raw)
            .unwrap();
        raw
    }

    #[test]
    fn animations_follow_write_animated_png() {
        let frames: Vec<Vec<u8>> = (0..3u8).map(|f| vec![f * 40; 2 * 2 * 3]).collect();
        let (mut apng, mut file) = Apng::new(2, 2, 3, 24).unwrap();
        for rgb in &frames {
            file.extend(apng.frame(rgb).unwrap());
        }
        file.extend(apng.finish().unwrap());
        let parts = chunks(&file);
        let tags: Vec<&[u8; 4]> = parts.iter().map(|(t, _)| t).collect();
        assert_eq!(
            tags,
            [
                b"IHDR", b"acTL", b"fcTL", b"IDAT", b"fcTL", b"fdAT", b"fcTL", b"fdAT", b"IEND"
            ]
        );
        assert_eq!(
            parts[1].1,
            [0, 0, 0, 3, 0, 0, 0, 0],
            "3 frames, loop forever"
        );
        let expected_fctl = |sequence: u8| {
            [
                vec![0, 0, 0, sequence, 0, 0, 0, 2, 0, 0, 0, 2],
                vec![0; 8],
                vec![0, 1, 0, 24, 0, 0],
            ]
            .concat()
        };
        assert_eq!(parts[2].1, expected_fctl(0));
        assert_eq!(parts[4].1, expected_fctl(1));
        assert_eq!(parts[6].1, expected_fctl(3));
        assert_eq!(&parts[5].1[..4], [0, 0, 0, 2]);
        assert_eq!(&parts[7].1[..4], [0, 0, 0, 4]);
        let unfiltered = |rgb: &[u8]| -> Vec<u8> {
            rgb.chunks(6)
                .flat_map(|row| [&[0][..], row].concat())
                .collect()
        };
        assert_eq!(inflate(&parts[3].1), unfiltered(&frames[0]));
        assert_eq!(inflate(&parts[5].1[4..]), unfiltered(&frames[1]));
        assert_eq!(inflate(&parts[7].1[4..]), unfiltered(&frames[2]));
    }

    #[test]
    fn animations_check_their_frame_count() {
        assert!(matches!(Apng::new(1, 1, 0, 12), Err(PngError::FrameCount)));
        assert!(matches!(Apng::new(1, 1, 2, 0), Err(PngError::FrameCount)));
        assert!(Apng::new(1, 1, 1 << 31, 12).is_ok());
        assert!(matches!(
            Apng::new(1, 1, (1 << 31) + 1, 12),
            Err(PngError::FrameCount)
        ));
        let (mut apng, _) = Apng::new(1, 1, 1, 12).unwrap();
        assert!(matches!(apng.frame(&[0; 2]), Err(PngError::WrongLength)));
        apng.frame(&[0; 3]).unwrap();
        assert!(matches!(apng.frame(&[0; 3]), Err(PngError::FrameCount)));
        let (apng, _) = Apng::new(1, 1, 2, 12).unwrap();
        assert!(matches!(apng.finish(), Err(PngError::FrameCount)));
    }
}
