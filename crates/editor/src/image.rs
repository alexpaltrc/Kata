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
            let start = y as usize * self.stride() + rect.left as usize * Self::BYTES_PER_PIXEL;
            pixels.extend_from_slice(&self.pixels[start..start + row_len]);
        }
        Some(Self::new(rect.width() as u32, rect.height() as u32, pixels))
    }
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
}
