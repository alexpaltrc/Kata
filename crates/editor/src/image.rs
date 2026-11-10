use crate::Rect;

/// An opaque BGRA8 image, stored row by row without padding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl Image {
    pub const BYTES_PER_PIXEL: usize = 4;

    /// # Panics
    /// When `pixels` doesn't hold exactly `width * height` BGRA pixels.
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Self {
        assert_eq!(
            pixels.len(),
            width as usize * height as usize * Self::BYTES_PER_PIXEL,
            "pixel buffer doesn't match {width}x{height}"
        );
        Self {
            width,
            height,
            pixels,
        }
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn pixels_mut(&mut self) -> &mut [u8] {
        &mut self.pixels
    }

    /// Where pixel `(x, y)` starts in `pixels`.
    pub const fn offset(&self, x: i32, y: i32) -> usize {
        y as usize * self.stride() + x as usize * Self::BYTES_PER_PIXEL
    }

    pub const fn stride(&self) -> usize {
        self.width as usize * Self::BYTES_PER_PIXEL
    }

    pub const fn bounds(&self) -> Rect {
        Rect::new(0, 0, self.width as i32, self.height as i32)
    }

    /// Copies the part of the image inside `rect`, or `None` when it falls outside.
    pub fn crop(&self, rect: Rect) -> Option<Self> {
        let rect = rect.intersect(&self.bounds())?;
        let row_len = rect.width() as usize * Self::BYTES_PER_PIXEL;
        let mut pixels = Vec::with_capacity(row_len * rect.height() as usize);
        for y in rect.top..rect.bottom {
            let start = self.offset(rect.left, y);
            pixels.extend_from_slice(&self.pixels[start..start + row_len]);
        }
        Some(Self::new(rect.width() as u32, rect.height() as u32, pixels))
    }

    /// The image `factor` times smaller across and down, each pixel the average of
    /// the block it covers. The last row and column of blocks may be partial.
    pub fn shrunk(&self, factor: u32) -> Self {
        let factor = factor.max(1);
        let (width, height) = (self.width.div_ceil(factor), self.height.div_ceil(factor));
        let mut sums = vec![0u32; width as usize * 3];
        let mut pixels =
            Vec::with_capacity(width as usize * height as usize * Self::BYTES_PER_PIXEL);
        for row in 0..height {
            sums.fill(0);
            let (top, bottom) = (row * factor, ((row + 1) * factor).min(self.height));
            for y in top..bottom {
                let line = &self.pixels[self.offset(0, y as i32)..][..self.stride()];
                for (x, pixel) in line.as_chunks::<4>().0.iter().enumerate() {
                    let sum = &mut sums[x / factor as usize * 3..][..3];
                    for channel in 0..3 {
                        sum[channel] += u32::from(pixel[channel]);
                    }
                }
            }
            for (column, sum) in sums.as_chunks::<3>().0.iter().enumerate() {
                let left = column as u32 * factor;
                let count = (bottom - top) * ((left + factor).min(self.width) - left);
                let average = |sum: u32| ((sum + count / 2) / count) as u8;
                pixels.extend_from_slice(&[average(sum[0]), average(sum[1]), average(sum[2]), 255]);
            }
        }
        Self::new(width, height, pixels)
    }

    /// Copies `from` onto the part of the image inside `area`, with `from`'s top-left
    /// corner placed at `at`.
    pub fn paste(&mut self, from: &Self, at: (i32, i32), area: Rect) {
        let placed = from.bounds().offset(at.0, at.1);
        let Some(area) = area
            .intersect(&self.bounds())
            .and_then(|area| area.intersect(&placed))
        else {
            return;
        };
        let row_len = area.width() as usize * Self::BYTES_PER_PIXEL;
        for y in area.top..area.bottom {
            let source = from.offset(area.left - at.0, y - at.1);
            let target = self.offset(area.left, y);
            self.pixels[target..target + row_len]
                .copy_from_slice(&from.pixels[source..source + row_len]);
        }
    }

    /// Lays `from`, scaled by `scale` with its top-left corner at `origin`, over the
    /// part of the image inside `area`. Growing, each pixel blends the ones around it
    /// with a sharp cubic filter, as Windows draws scaled images; shrinking, it takes
    /// each one under it by how much of it it covers. Bands of rows go to every core.
    pub fn paste_scaled(&mut self, from: &Self, scale: f32, origin: (f32, f32), area: Rect) {
        /// Fewer pixels than this aren't worth another thread.
        const BAND: usize = 1 << 16;
        let Some(area) = area.intersect(&self.bounds()) else {
            return;
        };
        let columns = Taps::new(area.left, area.width(), origin.0, scale, from.width);
        let rows = Taps::new(area.top, area.height(), origin.1, scale, from.height);
        let (width, height) = (area.width() as usize, area.height() as usize);
        let cores = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
        let per = height.div_ceil((width * height / BAND).clamp(1, cores));
        let stride = self.stride();
        let start = self.offset(0, area.top);
        let lines = &mut self.pixels[start..][..height * stride];
        let scaled = Scaled {
            from,
            columns: &columns,
            rows: &rows,
            left: area.left as usize * Self::BYTES_PER_PIXEL,
            width,
        };
        std::thread::scope(|scope| {
            let mut bands = lines.chunks_mut(per * stride).enumerate();
            let own = bands.next();
            for (band, lines) in bands {
                let scaled = &scaled;
                scope.spawn(move || scaled.blend(lines, stride, band * per));
            }
            if let Some((_, lines)) = own {
                scaled.blend(lines, stride, 0);
            }
        });
    }

    /// Lays `from` over the image with its top-left corner at `at`, its corners
    /// rounded by `radius` pixels: what lies outside the curves keeps showing.
    pub fn paste_rounded(&mut self, from: &Self, at: (i32, i32), radius: f32) {
        let shape = from.bounds().offset(at.0, at.1);
        let Some(area) = shape.intersect(&self.bounds()) else {
            return;
        };
        let radius = radius.clamp(0.0, shape.width().min(shape.height()) as f32 / 2.0);
        for y in area.top..area.bottom {
            for x in area.left..area.right {
                let cover = coverage(shape, radius, x, y);
                if cover <= 0.0 {
                    continue;
                }
                let source = from.offset(x - at.0, y - at.1);
                let target = self.offset(x, y);
                for channel in 0..3 {
                    let (over, under) = (
                        f32::from(from.pixels[source + channel]),
                        f32::from(self.pixels[target + channel]),
                    );
                    self.pixels[target + channel] = (under + (over - under) * cover).round() as u8;
                }
            }
        }
    }
}

/// An image scaled over a run of columns of another, ready to blend row by row.
struct Scaled<'a> {
    from: &'a Image,
    columns: &'a Taps,
    rows: &'a Taps,
    /// Where the run starts within a row, in bytes, and how many pixels it holds.
    left: usize,
    width: usize,
}

impl Scaled<'_> {
    /// Blends into `lines`, whole rows `stride` bytes long from the `first` one the
    /// rows' taps know.
    fn blend(&self, lines: &mut [u8], stride: usize, first: usize) {
        let (start, end) = self.columns.span();
        let span = (end - start) as usize * Image::BYTES_PER_PIXEL;
        let mut sums = vec![0.0f32; span];
        for (index, line) in lines.chunks_exact_mut(stride).enumerate() {
            // Down first, over just the columns the run takes, then across.
            sums.fill(0.0);
            for (source, weight) in self.rows.of(first + index) {
                let from = &self.from.pixels[self.from.offset(start as i32, source as i32)..];
                for (sum, &byte) in sums.iter_mut().zip(&from[..span]) {
                    *sum += weight * f32::from(byte);
                }
            }
            let sums = sums.as_chunks::<4>().0;
            let out = &mut line[self.left..][..self.width * Image::BYTES_PER_PIXEL];
            for (column, pixel) in out.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                let mut sum = [0.0f32; 4];
                for (source, weight) in self.columns.of(column) {
                    let under = &sums[(source - start) as usize];
                    for channel in 0..4 {
                        sum[channel] += weight * under[channel];
                    }
                }
                // Rounded, and held within a byte where the cubic overshoots.
                let byte = |value: f32| (value + 0.5) as u8;
                *pixel = [byte(sum[0]), byte(sum[1]), byte(sum[2]), 255];
            }
        }
    }
}

/// Along one way across a drawing, for each of its pixels, the pixels of an image
/// laid over it that blend into it, and how much each weighs.
struct Taps {
    /// For each pixel, its first source and how many there are.
    spans: Vec<(u32, u32)>,
    /// Each pixel's weights, `width` apart.
    weights: Vec<f32>,
    width: usize,
}

impl Taps {
    /// For the `count` pixels from `start`, of an image `size` pixels long, scaled by
    /// `scale` from `origin`.
    fn new(start: i32, count: i32, origin: f32, scale: f32, size: u32) -> Self {
        // Shrinking, each pixel reaches over just the ones it covers.
        let shrinking = scale < 1.0;
        let reach = if shrinking { 0.5 / scale } else { 2.0 };
        let width = (2.0 * reach).ceil() as usize + 2;
        let mut spans = Vec::with_capacity(count as usize);
        let mut weights = vec![0.0; count as usize * width];
        for (index, own) in weights.chunks_exact_mut(width).enumerate() {
            let center = ((start + index as i32) as f32 + 0.5 - origin) / scale;
            let last = i64::from(size) - 1;
            let (low, high) = (
                (center - reach).floor() as i64,
                (center + reach).ceil() as i64,
            );
            // Past its edges, the image goes on as its edge pixels.
            let (first, end) = (low.clamp(0, last), (high - 1).clamp(0, last) + 1);
            let mut total = 0.0;
            for source in low..high {
                let near = source as f32;
                let weight = if shrinking {
                    ((near + 1.0).min(center + reach) - near.max(center - reach)).max(0.0)
                } else {
                    cubic(near + 0.5 - center)
                };
                own[(source.clamp(0, last) - first) as usize] += weight;
                total += weight;
            }
            if total.abs() > f32::EPSILON {
                own.iter_mut().for_each(|weight| *weight /= total);
            } else {
                own[0] = 1.0;
            }
            spans.push((first as u32, (end - first) as u32));
        }
        Self {
            spans,
            weights,
            width,
        }
    }

    /// The `index`th pixel's sources and their weights.
    fn of(&self, index: usize) -> impl Iterator<Item = (u32, f32)> + '_ {
        let (first, count) = self.spans[index];
        let weights = &self.weights[index * self.width..][..count as usize];
        (first..first + count).zip(weights.iter().copied())
    }

    /// The first source any pixel takes, and the one past the last.
    fn span(&self) -> (u32, u32) {
        let first = self.spans.iter().map(|&(first, _)| first).min();
        let last = self.spans.iter().map(|&(first, count)| first + count).max();
        (first.unwrap_or(0), last.unwrap_or(0))
    }
}

/// Keys' cubic convolution, sharp: a = -0.5.
fn cubic(x: f32) -> f32 {
    let x = x.abs();
    if x < 1.0 {
        (1.5 * x - 2.5) * x * x + 1.0
    } else if x < 2.0 {
        ((-0.5 * x + 2.5) * x - 4.0) * x + 2.0
    } else {
        0.0
    }
}

/// How much of pixel `(x, y)` lies inside `shape` with its corners rounded by
/// `radius`: 1 inside, 0 outside, in between along the curves.
fn coverage(shape: Rect, radius: f32, x: i32, y: i32) -> f32 {
    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
    let (left, top) = (shape.left as f32 + radius, shape.top as f32 + radius);
    let (right, bottom) = (shape.right as f32 - radius, shape.bottom as f32 - radius);
    let dx = (left - px).max(px - right).max(0.0);
    let dy = (top - py).max(py - bottom).max(0.0);
    if dx == 0.0 || dy == 0.0 {
        return 1.0;
    }
    (radius - dx.hypot(dy) + 0.5).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 4x3 image whose blue channel encodes each pixel's index.
    fn numbered() -> Image {
        let pixels = (0..12u8).flat_map(|i| [i, 0, 0, 255]).collect();
        Image::new(4, 3, pixels)
    }

    fn blues(image: &Image) -> Vec<u8> {
        image
            .pixels()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|px| px[0])
            .collect()
    }

    #[test]
    fn shrunk_averages_each_block_partial_ones_too() {
        let shrunk = numbered().shrunk(2);
        assert_eq!((shrunk.width(), shrunk.height()), (2, 2));
        // (0 + 1 + 4 + 5) / 4, (2 + 3 + 6 + 7) / 4, then the last row alone.
        assert_eq!(blues(&shrunk), [3, 5, 9, 11]);
        assert_eq!(numbered().shrunk(1), numbered());
    }

    #[test]
    fn crop_copies_the_inner_pixels() {
        let crop = numbered().crop(Rect::new(1, 1, 3, 3)).unwrap();
        assert_eq!((crop.width(), crop.height()), (2, 2));
        assert_eq!(blues(&crop), [5, 6, 9, 10]);
    }

    #[test]
    fn crop_clips_to_the_image() {
        let crop = numbered().crop(Rect::new(-5, 2, 2, 10)).unwrap();
        assert_eq!(blues(&crop), [8, 9]);
    }

    #[test]
    fn crop_outside_is_none() {
        assert_eq!(numbered().crop(Rect::new(10, 10, 20, 20)), None);
    }

    #[test]
    fn paste_places_and_clips() {
        let blank = || Image::new(4, 3, [7, 0, 0, 255].repeat(12));
        let patch = numbered().crop(Rect::new(1, 1, 3, 3)).unwrap();

        let mut image = blank();
        image.paste(&patch, (3, 2), image.bounds());
        assert_eq!(blues(&image), [7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 5]);

        let mut image = blank();
        image.paste(&patch, (1, 1), Rect::new(0, 0, 2, 3));
        assert_eq!(blues(&image), [7, 7, 7, 7, 7, 5, 7, 7, 7, 9, 7, 7]);
    }

    #[test]
    fn paste_scaled_keeps_a_plain_image_plain() {
        let gray = Image::new(5, 3, [90, 90, 90, 255].repeat(15));
        let mut image = Image::new(20, 10, [0, 0, 0, 255].repeat(200));
        image.paste_scaled(&gray, 2.5, (3.3, -1.7), Rect::new(4, 0, 15, 5));
        let blue = |x: i32, y: i32| image.pixels()[image.offset(x, y)];
        assert_eq!((blue(4, 0), blue(14, 4), blue(9, 2)), (90, 90, 90));
        // Outside the area, untouched.
        assert_eq!((blue(3, 0), blue(15, 0), blue(4, 5)), (0, 0, 0));
    }

    #[test]
    fn paste_scaled_blends_what_it_grows_and_what_it_shrinks() {
        let edge = Image::new(2, 1, vec![0, 0, 0, 255, 200, 0, 0, 255]);
        let mut grown = Image::new(4, 1, vec![0; 16]);
        grown.paste_scaled(&edge, 2.0, (0.0, 0.0), grown.bounds());
        let [a, b, c, d] = blues(&grown)[..] else {
            panic!("four pixels");
        };
        // Sharp, as Windows draws it: a touch past the bright side.
        assert_eq!(a, 0);
        assert!(a < b && b < c && c < d && (200..220).contains(&d));
        assert!((i32::from(b) + i32::from(c) - 200).abs() <= 1);

        let steps = Image::new(4, 1, [0, 0, 200, 200].map(|v| [v, 0, 0, 255]).concat());
        let mut shrunk = Image::new(2, 1, vec![0; 8]);
        shrunk.paste_scaled(&steps, 0.5, (0.0, 0.0), shrunk.bounds());
        let blues = blues(&shrunk);
        assert!(blues[0] < 50 && blues[1] > 150);
    }

    #[test]
    fn paste_rounded_keeps_what_lies_past_the_corners() {
        let mut image = Image::new(20, 20, [0, 0, 0, 255].repeat(400));
        let white = Image::new(16, 16, [255, 255, 255, 255].repeat(256));
        image.paste_rounded(&white, (2, 2), 6.0);
        let blue = |x: i32, y: i32| image.pixels()[image.offset(x, y)];
        // Outside the shape, and past its top-left curve: untouched.
        assert_eq!(blue(1, 1), 0);
        assert_eq!(blue(2, 2), 0);
        // Along the edges and inside: covered.
        assert_eq!(blue(10, 2), 255);
        assert_eq!(blue(2, 10), 255);
        assert_eq!(blue(10, 10), 255);
        // On the curve: blended.
        assert!((1..255).contains(&blue(3, 4)));
        // Every corner is rounded alike.
        assert_eq!(blue(17, 17), 0);
        assert_eq!(blue(17, 2), 0);
    }
}
