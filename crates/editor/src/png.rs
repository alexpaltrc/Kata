//! PNG files (ISO/IEC 15948), written by Kata itself.
//!
//! A capture with 256 colors or fewer, as most windows are, stores each pixel as an
//! index into its colors, packed as tightly as their number allows. Any other
//! stores RGB, each row with the filter that leaves it most repetitive.

use crate::{Image, deflate};

const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1A, b'\n'];
const TRUECOLOR: u8 = 2;
const INDEXED: u8 = 3;
/// No color has this key: colors only use the low 24 bits.
const NO_COLOR: u32 = u32::MAX;

impl Image {
    /// The image as a PNG file.
    pub fn to_png(&self) -> Vec<u8> {
        let (depth, kind, colors, rows) = match index(self) {
            Some((colors, indices)) => {
                let depth = match colors.len() {
                    0..=2 => 1,
                    3..=4 => 2,
                    5..=16 => 4,
                    _ => 8,
                };
                let rgb = colors.iter().flat_map(|color| {
                    let [blue, green, red, _] = color.to_le_bytes();
                    [red, green, blue]
                });
                (
                    depth,
                    INDEXED,
                    rgb.collect(),
                    packed_rows(self, &indices, depth),
                )
            }
            None => (8, TRUECOLOR, Vec::new(), filtered_rows(self)),
        };

        let mut header = Vec::with_capacity(13);
        header.extend_from_slice(&self.width().to_be_bytes());
        header.extend_from_slice(&self.height().to_be_bytes());
        // Then: compression, filter method and interlacing, the only kinds there are.
        header.extend_from_slice(&[depth, kind, 0, 0, 0]);

        let pixels = deflate::zlib(&rows);
        let mut png = Vec::with_capacity(pixels.len() + colors.len() + 64);
        png.extend_from_slice(&SIGNATURE);
        chunk(&mut png, b"IHDR", &header);
        if kind == INDEXED {
            chunk(&mut png, b"PLTE", &colors);
        }
        chunk(&mut png, b"IDAT", &pixels);
        chunk(&mut png, b"IEND", &[]);
        png
    }
}

fn chunk(png: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    png.extend_from_slice(&(data.len() as u32).to_be_bytes());
    png.extend_from_slice(kind);
    png.extend_from_slice(data);
    png.extend_from_slice(&crc32(&[kind, data]).to_be_bytes());
}

/// The image's colors, as `0x00RRGGBB`, and each pixel's index into them; `None`
/// past 256 colors.
fn index(image: &Image) -> Option<(Vec<u32>, Vec<u8>)> {
    // An open-addressed table, twice as big as the colors it can hold.
    const SLOTS: usize = 512;
    let mut keys = [NO_COLOR; SLOTS];
    let mut values = [0u8; SLOTS];
    let mut colors = Vec::new();
    let mut indices = Vec::with_capacity(image.pixels().len() / Image::BYTES_PER_PIXEL);
    // Neighbors tend to share a color, so most pixels skip the table.
    let (mut last, mut last_index) = (NO_COLOR, 0);
    for pixel in image.pixels().as_chunks::<4>().0 {
        let color = u32::from_le_bytes([pixel[0], pixel[1], pixel[2], 0]);
        if color != last {
            let mut slot = (color.wrapping_mul(0x9E37_79B1) >> 23) as usize;
            while keys[slot] != color {
                if keys[slot] == NO_COLOR {
                    if colors.len() == 256 {
                        return None;
                    }
                    keys[slot] = color;
                    values[slot] = colors.len() as u8;
                    colors.push(color);
                    break;
                }
                slot = (slot + 1) % SLOTS;
            }
            (last, last_index) = (color, values[slot]);
        }
        indices.push(last_index);
    }
    Some((colors, indices))
}

/// Rows of `depth`-bit indices, first pixel in the high bits, each row unfiltered:
/// indices are names, not amounts, so differences between them mean nothing.
fn packed_rows(image: &Image, indices: &[u8], depth: u8) -> Vec<u8> {
    let width = image.width() as usize;
    let per_byte = usize::from(8 / depth);
    let row_len = width.div_ceil(per_byte);
    let mut rows = Vec::with_capacity((1 + row_len) * image.height() as usize);
    for row in indices.chunks_exact(width) {
        rows.push(0);
        if depth == 8 {
            rows.extend_from_slice(row);
            continue;
        }
        rows.extend(row.chunks(per_byte).map(|group| {
            let byte = group.iter().fold(0u8, |byte, &index| byte << depth | index);
            byte << (depth * (per_byte - group.len()) as u8)
        }));
    }
    rows
}

/// RGB rows, each led by the filter that suits it best.
fn filtered_rows(image: &Image) -> Vec<u8> {
    const BPP: usize = 3;
    let row_len = image.width() as usize * BPP;
    let mut rows = Vec::with_capacity((1 + row_len) * image.height() as usize);
    let mut above = vec![0; row_len];
    let mut line = vec![0; row_len];
    // Sub, Up, Average and Paeth, after None: the row itself.
    let mut filtered = [(); 4].map(|()| vec![0u8; row_len]);
    let mut pairs = Pairs::new();
    for pixels in image.pixels().chunks_exact(image.stride()) {
        for (rgb, bgra) in line
            .as_chunks_mut::<BPP>()
            .0
            .iter_mut()
            .zip(pixels.as_chunks::<4>().0)
        {
            *rgb = [bgra[2], bgra[1], bgra[0]];
        }

        let [sub, up, average, paeth_row] = &mut filtered;
        // The first pixel has nothing to its left: it counts as zero.
        let first = BPP.min(row_len);
        for i in 0..first {
            sub[i] = line[i];
            average[i] = line[i].wrapping_sub(above[i] / 2);
            paeth_row[i] = line[i].wrapping_sub(above[i]);
        }
        for (out, (&x, &b)) in up.iter_mut().zip(line.iter().zip(&above)) {
            *out = x.wrapping_sub(b);
        }
        let (current, left) = (&line[first..], &line[..row_len - first]);
        let (over, over_left) = (&above[first..], &above[..row_len - first]);
        for (out, (&x, &a)) in sub[first..].iter_mut().zip(current.iter().zip(left)) {
            *out = x.wrapping_sub(a);
        }
        for (out, ((&x, &a), &b)) in average[first..]
            .iter_mut()
            .zip(current.iter().zip(left).zip(over))
        {
            *out = x.wrapping_sub(((u16::from(a) + u16::from(b)) / 2) as u8);
        }
        for (out, (((&x, &a), &b), &c)) in paeth_row[first..]
            .iter_mut()
            .zip(current.iter().zip(left).zip(over).zip(over_left))
        {
            *out = x.wrapping_sub(paeth(a, b, c));
        }

        let candidates: [&[u8]; 5] = [&line, sub, up, average, paeth_row];
        let (filter, row) = candidates
            .into_iter()
            .enumerate()
            .min_by_key(|&(_, row)| pairs.count(row))
            .expect("five filters");
        rows.push(filter as u8);
        rows.extend_from_slice(row);
        std::mem::swap(&mut above, &mut line);
    }
    rows
}

/// Whichever of left, up and up-left is closest to `left + up - up_left`.
fn paeth(left: u8, up: u8, up_left: u8) -> u8 {
    let (a, b, c) = (i16::from(left), i16::from(up), i16::from(up_left));
    let estimate = a + b - c;
    let (to_left, to_up, to_up_left) = (
        (estimate - a).abs(),
        (estimate - b).abs(),
        (estimate - c).abs(),
    );
    if to_left <= to_up && to_left <= to_up_left {
        left
    } else if to_up <= to_up_left {
        up
    } else {
        up_left
    }
}

/// Counts the different pairs of neighboring bytes in a row. The fewer there are,
/// the more the row repeats itself, and repeats are what DEFLATE shrinks.
struct Pairs {
    /// The last count that saw each pair, so no count has to clear it.
    seen: Vec<u32>,
    /// Which count this is.
    round: u32,
}

impl Pairs {
    fn new() -> Self {
        Self {
            seen: vec![0; 1 << 16],
            round: 0,
        }
    }

    fn count(&mut self, bytes: &[u8]) -> usize {
        self.round += 1;
        let mut count = 0;
        for pair in bytes.windows(2) {
            let seen = &mut self.seen[usize::from(pair[0]) << 8 | usize::from(pair[1])];
            if *seen != self.round {
                *seen = self.round;
                count += 1;
            }
        }
        count
    }
}

/// The checksum that closes every chunk.
fn crc32(parts: &[&[u8]]) -> u32 {
    const TABLE: [u32; 256] = {
        let mut table = [0; 256];
        let mut n = 0;
        while n < 256 {
            let mut c = n as u32;
            let mut bit = 0;
            while bit < 8 {
                c = if c & 1 == 1 {
                    0xEDB8_8320 ^ c >> 1
                } else {
                    c >> 1
                };
                bit += 1;
            }
            table[n] = c;
            n += 1;
        }
        table
    };
    let mut crc = !0u32;
    for &byte in parts.iter().copied().flatten() {
        crc = TABLE[((crc ^ u32::from(byte)) & 0xFF) as usize] ^ crc >> 8;
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(width: u32, height: u32, color: impl Fn(u32, u32) -> [u8; 3]) -> Image {
        let pixels = (0..height)
            .flat_map(|y| (0..width).map(move |x| (x, y)))
            .flat_map(|(x, y)| {
                let [red, green, blue] = color(x, y);
                [blue, green, red, 255]
            })
            .collect();
        Image::new(width, height, pixels)
    }

    /// The chunks of `png`, as kind and data, after checking each one's checksum.
    fn chunks(png: &[u8]) -> Vec<(String, Vec<u8>)> {
        assert_eq!(png[..8], SIGNATURE);
        let mut chunks = Vec::new();
        let mut rest = &png[8..];
        while !rest.is_empty() {
            let len = u32::from_be_bytes(rest[..4].try_into().unwrap()) as usize;
            let (kind, data) = (&rest[4..8], &rest[8..8 + len]);
            let crc = u32::from_be_bytes(rest[8 + len..12 + len].try_into().unwrap());
            assert_eq!(crc, crc32(&[kind, data]));
            chunks.push((String::from_utf8(kind.to_vec()).unwrap(), data.to_vec()));
            rest = &rest[12 + len..];
        }
        chunks
    }

    #[test]
    fn crc32_matches_its_definition() {
        assert_eq!(crc32(&[b"123456789"]), 0xCBF4_3926);
        assert_eq!(crc32(&[b"1234", b"56789"]), 0xCBF4_3926);
    }

    #[test]
    fn few_colors_are_packed_as_indices() {
        let cases = [(2, 1), (3, 2), (16, 4), (17, 8), (256, 8)];
        for (count, depth) in cases {
            let png = image(20, 20, |x, y| [((x + 20 * y) % count) as u8, 0, 0]).to_png();
            let chunks = chunks(&png);
            let kinds: Vec<_> = chunks.iter().map(|(kind, _)| kind.as_str()).collect();
            assert_eq!(kinds, ["IHDR", "PLTE", "IDAT", "IEND"]);
            assert_eq!(chunks[0].1[8..10], [depth, INDEXED], "{count} colors");
            assert_eq!(chunks[1].1.len(), 3 * count as usize);
        }
    }

    #[test]
    fn many_colors_are_stored_as_rgb() {
        let png = image(20, 20, |x, y| [x as u8, y as u8, 7]).to_png();
        let chunks = chunks(&png);
        let kinds: Vec<_> = chunks.iter().map(|(kind, _)| kind.as_str()).collect();
        assert_eq!(kinds, ["IHDR", "IDAT", "IEND"]);
        assert_eq!(
            chunks[0].1,
            [0, 0, 0, 20, 0, 0, 0, 20, 8, TRUECOLOR, 0, 0, 0]
        );
    }

    #[test]
    fn indices_pack_from_the_high_bits() {
        let image = image(5, 1, |x, _| [[0, 9][x as usize % 2], 0, 0]);
        let (_, indices) = index(&image).unwrap();
        assert_eq!(indices, [0, 1, 0, 1, 0]);
        assert_eq!(packed_rows(&image, &indices, 1), [0, 0b0101_0000]);
        assert_eq!(packed_rows(&image, &indices, 4), [0, 0x01, 0x01, 0x00]);
    }

    #[test]
    fn paeth_picks_the_closest_neighbor() {
        assert_eq!(paeth(10, 30, 40), 10);
        assert_eq!(paeth(10, 20, 10), 20);
        assert_eq!(paeth(10, 30, 20), 20);
        assert_eq!(paeth(5, 5, 5), 5);
    }
}
