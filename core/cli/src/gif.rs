//! GIF encoding, as `write_gif` in `simulacra.py` does it, byte for byte: one
//! global 256-colour table shared by every frame (240 samples along the
//! palette plus 16 greys), pixels mapped to it once per 5-bit colour cell, and
//! the same LZW code stream (`_lzw_codes`). Python's generic colour cube for a
//! missing palette is not ported: the command line always has a palette.
//!
//! Like [`crate::png::Apng`], the encoder does no I/O: [`Gif`] returns the file
//! piece by piece, so a caller can write each frame as soon as it is rendered.

use simulacra_engine::Palette;

/// `GIF_MAX_SIDE` in Python: the format stores each side in 16 bits.
pub const GIF_MAX_SIDE: usize = 65_535;

/// Entries in the colour table, all of which index bytes can address.
const TABLE_SIZE: usize = 256;

/// Why a frame cannot be encoded.
#[derive(Debug, PartialEq, Eq)]
pub enum GifError {
    /// A side is zero or over [`GIF_MAX_SIDE`].
    BadSize,
    /// The pixel buffer does not hold `width * height * 3` bytes.
    WrongLength,
}

impl std::fmt::Display for GifError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadSize => write!(
                f,
                "GIF width and height must be between 1 and {GIF_MAX_SIDE} pixels"
            ),
            Self::WrongLength => f.write_str("pixel buffer must contain width * height * 3 bytes"),
        }
    }
}

/// `_gif_table(stops)` in Python: 240 palette samples, then 16 greys.
fn table(palette: &Palette) -> Vec<[u8; 3]> {
    let mut table: Vec<[u8; 3]> = (0..240)
        .map(|i| palette.color(f64::from(i) / 239.0))
        .collect();
    // Python rounds i * 255 / 15, which is the whole number 17 * i.
    table.extend((0..16u8).map(|i| [17 * i; 3]));
    table
}

/// `_nearest_entry` in Python: the entry closest in squared RGB distance,
/// the lowest index on a tie (as `min` keeps the first minimum).
fn nearest_entry(table: &[[u8; 3]], color: [u8; 3]) -> u8 {
    let distance = |entry: &[u8; 3]| -> i32 {
        (0..3)
            .map(|c| (i32::from(entry[c]) - i32::from(color[c])).pow(2))
            .sum()
    };
    let mut best = 0;
    for (index, entry) in table.iter().enumerate().skip(1) {
        if distance(entry) < distance(&table[best]) {
            best = index;
        }
    }
    u8::try_from(best).expect("the table has 256 entries")
}

/// The 5-bit colour cell of an RGB colour, as one number below 2**15.
fn cell(color: [u8; 3]) -> usize {
    (usize::from(color[0] >> 3) << 10)
        | (usize::from(color[1] >> 3) << 5)
        | usize::from(color[2] >> 3)
}

/// `_GifColors` in Python: a colour in the table keeps its (lowest) entry;
/// any other colour takes the entry nearest the centre of its 5-bit cell,
/// searched once per cell. Python's cache of exact colours only saves time, so
/// it is not ported; the mapping is the same.
struct Colors {
    table: Vec<[u8; 3]>,
    /// Table colours that fall in each cell, with their lowest index.
    members: Vec<Vec<([u8; 3], u8)>>,
    /// The nearest entry to each cell's centre, once searched.
    cells: Vec<Option<u8>>,
}

impl Colors {
    fn new(table: Vec<[u8; 3]>) -> Self {
        let mut members = vec![Vec::new(); 1 << 15];
        for (index, &color) in table.iter().enumerate() {
            let list: &mut Vec<([u8; 3], u8)> = &mut members[cell(color)];
            if !list.iter().any(|&(c, _)| c == color) {
                list.push((color, u8::try_from(index).expect("256 entries")));
            }
        }
        Self {
            table,
            members,
            cells: vec![None; 1 << 15],
        }
    }

    fn index(&mut self, color: [u8; 3]) -> u8 {
        let key = cell(color);
        if let Some(&(_, index)) = self.members[key].iter().find(|&&(c, _)| c == color) {
            return index;
        }
        *self.cells[key].get_or_insert_with(|| {
            let centre = color.map(|c| (c & !7) | 4);
            nearest_entry(&self.table, centre)
        })
    }
}

/// `_lzw_width` in Python: the code width after the table reaches
/// `next_code` entries, at most 12 bits.
fn lzw_width(next_code: usize, width: u32) -> u32 {
    if next_code == 1 << width && width < 12 {
        width + 1
    } else {
        width
    }
}

/// Packs codes into bytes, least significant bit first (`_gif_lzw`).
#[derive(Default)]
struct BitWriter {
    out: Vec<u8>,
    buffer: u32,
    bits: u32,
}

impl BitWriter {
    fn put(&mut self, code: usize, width: u32) {
        self.buffer |= u32::try_from(code).expect("codes fit 12 bits") << self.bits;
        self.bits += width;
        while self.bits >= 8 {
            self.out.push(self.buffer.to_le_bytes()[0]);
            self.buffer >>= 8;
            self.bits -= 8;
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.bits > 0 {
            self.out.push(self.buffer.to_le_bytes()[0]);
        }
        self.out
    }
}

/// The LZW dictionary: `prefix << 8 | value` to code, as a dense table (0 is
/// empty, since codes start at 258) that a reset clears key by key.
struct Dictionary {
    codes: Vec<u16>,
    used: Vec<usize>,
}

impl Dictionary {
    fn new() -> Self {
        Self {
            codes: vec![0; 4097 << 8],
            used: Vec::with_capacity(4096),
        }
    }

    fn get(&self, key: usize) -> Option<usize> {
        let code = self.codes[key];
        (code != 0).then_some(usize::from(code))
    }

    fn insert(&mut self, key: usize, code: usize) {
        self.codes[key] = u16::try_from(code).expect("codes fit 13 bits");
        self.used.push(key);
    }

    fn clear(&mut self) {
        for key in self.used.drain(..) {
            self.codes[key] = 0;
        }
    }
}

/// `_gif_lzw(indices)` in Python: GIF LZW with minimum code size 8, emitting
/// exactly the codes `_lzw_codes` yields, packed into bytes.
fn lzw(indices: &[u8], dictionary: &mut Dictionary) -> Vec<u8> {
    const CLEAR: usize = 256;
    const END: usize = 257;
    let mut out = BitWriter::default();
    let (mut next_code, mut width) = (258, 9);
    out.put(CLEAR, width);
    let Some((&first, rest)) = indices.split_first() else {
        out.put(END, width);
        return out.finish();
    };
    let mut prefix = usize::from(first);
    for &value in rest {
        let value = usize::from(value);
        let key = (prefix << 8) | value;
        if let Some(found) = dictionary.get(key) {
            prefix = found;
            continue;
        }
        out.put(prefix, width);
        dictionary.insert(key, next_code);
        // The decoder adds its entry one code after the encoder, so the code
        // width grows once next_code passes 2**width.
        width = lzw_width(next_code, width);
        next_code += 1;
        if next_code == 4097 {
            out.put(CLEAR, width);
            dictionary.clear();
            (next_code, width) = (258, 9);
        }
        prefix = value;
    }
    out.put(prefix, width);
    // The decoder adds one more entry here, which can widen the end code.
    width = lzw_width(next_code, width);
    out.put(END, width);
    dictionary.clear();
    out.finish()
}

/// `_gif_delay(fps)` in Python: hundredths of a second, at least 2 (viewers
/// treat 0 and 1 as 10), rounding half to even like Python's `round`.
pub fn delay(fps: u16) -> u16 {
    let hundredths = (100.0 / f64::from(fps)).round_ties_even();
    // 100 / fps is at most 100, so the cast is exact.
    (hundredths as u16).max(2)
}

/// A GIF built one frame at a time, laid out as `write_gif` writes it: the
/// screen descriptor with the global table, a looping block when there is
/// more than one frame, then per frame a graphics control block (the delay,
/// no disposal), an image descriptor for the whole canvas and LZW data in
/// 255-byte sub-blocks.
pub struct Gif {
    width: u16,
    height: u16,
    delay: u16,
    colors: Colors,
    dictionary: Dictionary,
}

impl Gif {
    /// Starts a GIF; returns it with the bytes that begin the file. `looping`
    /// adds the NETSCAPE2.0 block, which Python writes for more than one frame.
    pub fn new(
        width: usize,
        height: usize,
        palette: &Palette,
        fps: u16,
        looping: bool,
    ) -> Result<(Self, Vec<u8>), GifError> {
        let side = |v: usize| u16::try_from(v).ok().filter(|&v| v > 0);
        let (Some(w), Some(h)) = (side(width), side(height)) else {
            return Err(GifError::BadSize);
        };
        let table = table(palette);
        let mut out = Vec::with_capacity(13 + 3 * TABLE_SIZE + 19);
        out.extend_from_slice(b"GIF89a");
        out.extend_from_slice(&w.to_le_bytes());
        out.extend_from_slice(&h.to_le_bytes());
        out.extend_from_slice(&[0xF7, 0, 0]); // global table of 256 entries, 8 bits
        out.extend(table.iter().flatten());
        if looping {
            out.extend_from_slice(b"\x21\xFF\x0BNETSCAPE2.0\x03\x01\x00\x00\x00");
        }
        let gif = Self {
            width: w,
            height: h,
            delay: delay(fps),
            colors: Colors::new(table),
            dictionary: Dictionary::new(),
        };
        Ok((gif, out))
    }

    /// Encodes the next frame from packed RGB and returns its blocks.
    pub fn frame(&mut self, rgb: &[u8]) -> Result<Vec<u8>, GifError> {
        let pixels = usize::from(self.width) * usize::from(self.height);
        if Some(rgb.len()) != pixels.checked_mul(3) {
            return Err(GifError::WrongLength);
        }
        let indices: Vec<u8> = rgb
            .as_chunks::<3>()
            .0
            .iter()
            .map(|&pixel| self.colors.index(pixel))
            .collect();
        let data = lzw(&indices, &mut self.dictionary);
        drop(indices);
        let mut out = Vec::with_capacity(data.len() + data.len() / 255 + 32);
        out.extend_from_slice(b"\x21\xF9\x04\x04"); // disposal 1: do not dispose
        out.extend_from_slice(&self.delay.to_le_bytes());
        out.extend_from_slice(&[0, 0]); // no transparent colour, block end
        out.push(0x2C);
        out.extend_from_slice(&[0; 4]); // left and top
        out.extend_from_slice(&self.width.to_le_bytes());
        out.extend_from_slice(&self.height.to_le_bytes());
        out.extend_from_slice(&[0, 8]); // no local table; minimum code size 8
        for block in data.chunks(255) {
            out.push(u8::try_from(block.len()).expect("blocks hold at most 255 bytes"));
            out.extend_from_slice(block);
        }
        out.push(0);
        Ok(out)
    }

    /// The trailer that ends the file.
    pub fn finish(self) -> Vec<u8> {
        vec![0x3B]
    }
}

#[cfg(test)]
mod tests {
    use super::{Colors, Dictionary, Gif, GifError, delay, lzw, nearest_entry, table};
    use simulacra_engine::Palette;

    fn deep() -> Palette {
        let stops = simulacra_engine::palette_stops("deep").unwrap().to_vec();
        Palette::new(stops).unwrap()
    }

    /// The reference decoder from `test_simulacra.py` (`lzw_decode`).
    fn lzw_decode(data: &[u8]) -> Vec<u8> {
        // Codes 256 and 257 (clear, end) hold empty placeholders.
        let fresh = || {
            let mut table: Vec<Vec<u8>> = (0..=255u8).map(|i| vec![i]).collect();
            table.extend([vec![], vec![]]);
            table
        };
        let (mut table, mut out) = (fresh(), Vec::new());
        let (mut width, mut bits, mut buffer, mut position) = (9, 0, 0u32, 0);
        let mut previous: Option<Vec<u8>> = None;
        loop {
            while bits < width {
                buffer |= u32::from(data[position]) << bits;
                position += 1;
                bits += 8;
            }
            let code = (buffer & ((1 << width) - 1)) as usize;
            buffer >>= width;
            bits -= width;
            if code == 256 {
                (table, width, previous) = (fresh(), 9, None);
                continue;
            }
            if code == 257 {
                return out;
            }
            let entry = match table.get(code) {
                Some(entry) => entry.clone(),
                None => {
                    let p = previous.clone().unwrap();
                    [p.clone(), vec![p[0]]].concat()
                }
            };
            if let Some(p) = previous {
                table.push([p, vec![entry[0]]].concat());
                if table.len() == 1 << width && width < 12 {
                    width += 1;
                }
            }
            out.extend_from_slice(&entry);
            previous = Some(entry);
        }
    }

    #[test]
    fn lzw_round_trips_across_table_resets() {
        let mut dictionary = Dictionary::new();
        // A pseudo-random stream long enough to fill the table several times,
        // then runs that grow long dictionary strings.
        let mut state = 12_345u32;
        let mut indices: Vec<u8> = (0..60_000)
            .map(|_| {
                state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                (state >> 16) as u8
            })
            .collect();
        indices.extend(std::iter::repeat_n(7, 50_000));
        for data in [&indices[..], &indices[..1], &[], &[1, 1, 1, 1]] {
            assert_eq!(lzw_decode(&lzw(data, &mut dictionary)), data);
        }
    }

    #[test]
    fn lzw_matches_python_codes() {
        // Python: _gif_lzw(bytes([1, 2, 1, 2, 1, 2])) == b'\x00\x03\x08\x10(0 '
        let mut dictionary = Dictionary::new();
        assert_eq!(
            lzw(&[1, 2, 1, 2, 1, 2], &mut dictionary),
            b"\x00\x03\x08\x10(0 "
        );
    }

    #[test]
    fn the_table_samples_the_palette_then_greys() {
        let table = table(&deep());
        assert_eq!(table.len(), 256);
        // Python: _gif_table(PALETTES['deep'])[:2], [120], [239], [240:243], [255]
        assert_eq!(table[0], [13, 19, 49]);
        assert_eq!(table[1], [13, 19, 49]);
        assert_eq!(table[120], [61, 90, 153]);
        assert_eq!(table[239], [168, 208, 230]);
        assert_eq!(&table[240..243], [[0; 3], [17; 3], [34; 3]]);
        assert_eq!(table[255], [255; 3]);
    }

    #[test]
    fn colours_map_like_python() {
        let table = table(&deep());
        let mut colors = Colors::new(table.clone());
        // Python: list(_GifColors(table).indices(bytes([0, 0, 0, 13, 19, 49,
        // 61, 90, 153, 200, 10, 90, 201, 12, 93]))) == [240, 0, 120, 246, 246]
        // A table colour keeps its own, lowest entry (entries 0 and 1 are
        // equal; black is only a grey); others take the entry nearest their
        // 5-bit cell's centre, here (204, 12, 92).
        let got = [
            [0, 0, 0],
            [13, 19, 49],
            [61, 90, 153],
            [200, 10, 90],
            [201, 12, 93],
        ]
        .map(|c| colors.index(c));
        assert_eq!(got, [240, 0, 120, 246, 246]);
        assert_eq!(nearest_entry(&table, [204, 12, 92]), 246);
        // Ties go to the lowest index.
        assert_eq!(
            nearest_entry(&[[0, 0, 0], [2, 0, 0], [2, 0, 0]], [1, 0, 0]),
            0
        );
        assert_eq!(
            nearest_entry(&[[9, 9, 9], [2, 0, 0], [0, 0, 0]], [1, 0, 0]),
            1
        );
    }

    #[test]
    fn delays_round_like_python() {
        // Python: [_gif_delay(f) for f in (1, 3, 8, 12, 24, 40, 50, 51, 65535)]
        let got: Vec<u16> = [1, 3, 8, 12, 24, 40, 50, 51, 65535].map(delay).to_vec();
        assert_eq!(got, [100, 33, 12, 8, 4, 2, 2, 2, 2]);
    }

    #[test]
    fn frames_follow_write_gif() {
        let (mut gif, start) = Gif::new(2, 1, &deep(), 12, true).unwrap();
        assert_eq!(&start[..13], b"GIF89a\x02\x00\x01\x00\xF7\x00\x00");
        assert_eq!(start.len(), 13 + 768 + 19);
        assert!(start.ends_with(b"NETSCAPE2.0\x03\x01\x00\x00\x00"));
        let frame = gif.frame(&[0, 0, 0, 255, 255, 255]).unwrap();
        assert_eq!(&frame[..8], b"\x21\xF9\x04\x04\x08\x00\x00\x00");
        assert_eq!(
            &frame[8..19],
            b"\x2C\x00\x00\x00\x00\x02\x00\x01\x00\x00\x08"
        );
        let size = usize::from(frame[19]);
        assert_eq!(frame.len(), 19 + 1 + size + 1);
        assert_eq!(lzw_decode(&frame[20..20 + size]), [240, 255]);
        assert_eq!(gif.frame(&[0; 3]), Err(GifError::WrongLength));
        assert_eq!(gif.finish(), [0x3B]);
        let (_, still) = Gif::new(2, 1, &deep(), 12, false).unwrap();
        assert_eq!(still.len(), 13 + 768);
        assert!(matches!(
            Gif::new(65_536, 1, &deep(), 12, false),
            Err(GifError::BadSize)
        ));
        assert!(Gif::new(65_535, 1, &deep(), 12, false).is_ok());
    }
}
