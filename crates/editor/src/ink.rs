//! Kata's ink, for the steps and the figures: persimmon, edged in white, over a soft
//! shadow.

use crate::{Image, Rect};

/// Kata's persimmon orange, as the image stores it: blue, green, red.
const ORANGE: [f32; 3] = [34.0, 90.0, 236.0];
/// The shadow under a usual mark at 100 %: its reach and drop, and its darkness.
const SHADOW_REACH: f32 = 4.0;
const SHADOW_DROP: f32 = 1.0;
const SHADOW_DARKNESS: f32 = 0.3;

/// How a mark is inked, in pixels.
pub struct Ink {
    /// The white edge around the orange.
    edge: f32,
    /// How far the shadow reaches past the mark, and how far below it it falls.
    reach: f32,
    drop: f32,
}

impl Ink {
    /// The ink of a mark `size` times as big as a usual one at 100 %, edged there in
    /// `edge` pixels of white.
    pub fn new(size: f32, edge: f32) -> Self {
        Self {
            edge: edge * size,
            reach: SHADOW_REACH * size,
            drop: SHADOW_DROP * size,
        }
    }

    /// How far past a mark its ink reaches, shadow included.
    pub fn margin(&self) -> i32 {
        (self.reach + self.drop).ceil() as i32 + 1
    }

    /// Inks a mark over what `image` shows, only inside `area`. `outside` tells how far
    /// a point lies outside the mark, negative inside, and `white` how much of a point
    /// on the orange shows white instead.
    pub fn paint(
        &self,
        image: &mut Image,
        area: Rect,
        outside: impl Fn((f32, f32)) -> f32,
        white: impl Fn((f32, f32)) -> f32,
    ) {
        let stride = image.stride();
        self.paint_rows(image.pixels_mut(), 0, stride, area, outside, white);
    }

    /// Inks a mark as `paint` does, into `rows`: an image's rows from `top` on,
    /// `stride` bytes each, which `area` lies inside.
    pub fn paint_rows(
        &self,
        rows: &mut [u8],
        top: i32,
        stride: usize,
        area: Rect,
        outside: impl Fn((f32, f32)) -> f32,
        white: impl Fn((f32, f32)) -> f32,
    ) {
        for y in area.top..area.bottom {
            for x in area.left..area.right {
                let point = (x as f32 + 0.5, y as f32 + 0.5);
                let shadow = outside((point.0, point.1 - self.drop)).max(0.0);
                let fade = (1.0 - shadow / self.reach).max(0.0);
                let keep = 1.0 - SHADOW_DARKNESS * fade * fade;
                let distance = outside(point);
                let edge = (0.5 - distance).clamp(0.0, 1.0);
                let orange = (0.5 - distance - self.edge).clamp(0.0, 1.0);
                let ink = if orange > 0.0 { white(point) } else { 0.0 };

                let at = (y - top) as usize * stride + x as usize * Image::BYTES_PER_PIXEL;
                let pixel = &mut rows[at..at + 3];
                for (channel, value) in pixel.iter_mut().enumerate() {
                    let mut color = f32::from(*value) * keep;
                    color += (255.0 - color) * edge;
                    color += (ORANGE[channel] - color) * orange;
                    color += (255.0 - color) * ink;
                    *value = (color + 0.5) as u8;
                }
            }
        }
    }
}
