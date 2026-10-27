//! Draws the edits onto the capture. The window shows these same pixels, so what
//! the user sees is exactly what gets saved.

use std::hash::{BuildHasher, RandomState};
use std::ops::Range;

use crate::{BLUR_LEVELS, Blur, Image, Rect, Spot};

/// The side of the cells each blur level averages, in pixels at 100 % scale.
const BLUR_CELLS: [f32; BLUR_LEVELS] = [8.0, 14.0, 24.0];
/// How far each cell's color is randomly shifted, in levels out of 255. Too little to
/// see, but it keeps the blur from being matched against guessed text.
const BLUR_NOISE: f32 = 4.0;
/// How much light the image keeps around the focus.
const FOCUS_LIGHT: f32 = 0.5;
/// The lit area's corner radius at 100 %.
pub const FOCUS_CORNER: f32 = 8.0;
/// A magnified focus floats over the image: its shadow's reach and drop at 100 %,
/// and its darkness.
const SHADOW_REACH: f32 = 24.0;
const SHADOW_DROP: f32 = 4.0;
const SHADOW_DARKNESS: f32 = 0.4;

/// Keeps a rendering of the capture with its edits, redrawing only what changes.
pub struct Renderer {
    scale: f32,
    /// Makes this session's blur noise unpredictable.
    seed: u64,
    /// The capture with its blurs.
    base: Image,
    output: Image,
    /// The blurs in `base`.
    blurs: Vec<Patch>,
    focus: Option<Spot>,
}

/// A blur and its pixels, which cover the part of it inside the image.
struct Patch {
    blur: Blur,
    area: Rect,
    pixels: Image,
}

impl Renderer {
    /// `scale` is the display scale the capture was taken at.
    pub fn new(image: &Image, scale: f32) -> Self {
        Self {
            scale,
            seed: RandomState::new().hash_one(0),
            base: image.clone(),
            output: image.clone(),
            blurs: Vec::new(),
            focus: None,
        }
    }

    pub fn output(&self) -> &Image {
        &self.output
    }

    /// Redraws `image` with `blurs` and `focus`, returning the areas that changed.
    pub fn update(&mut self, image: &Image, blurs: &[Blur], focus: Option<Spot>) -> Vec<Rect> {
        let bounds = image.bounds();
        let mut stale: Vec<Rect> = self
            .blurs
            .iter()
            .map(|patch| &patch.blur)
            .filter(|blur| !blurs.contains(blur))
            .chain(blurs.iter().filter(|blur| !self.has_blur(blur)))
            .filter_map(|blur| blur.rect.intersect(&bounds))
            .collect();
        stale.dedup();

        let mut kept = std::mem::take(&mut self.blurs);
        self.blurs = blurs
            .iter()
            .filter_map(|blur| match kept.iter().position(|old| old.blur == *blur) {
                Some(at) => Some(kept.swap_remove(at)),
                None => self.patch(image, *blur),
            })
            .collect();
        for area in &stale {
            self.base.paste(image, (0, 0), *area);
            for patch in &self.blurs {
                let at = (patch.area.left, patch.area.top);
                self.base.paste(&patch.pixels, at, *area);
            }
        }

        let mut dirty = stale.clone();
        if self.focus.is_some() != focus.is_some() {
            dirty = vec![bounds];
        } else if let Some(old) = self.focus
            && let Some(new) = focus
            && old != new
        {
            dirty.push(self.footprint(old).union(&self.footprint(new)));
        }
        if let Some(spot) = focus
            && stale
                .iter()
                .any(|area| area.intersect(&spot.rect.inflate(1)).is_some())
        {
            dirty.push(self.footprint(spot));
        }
        self.focus = focus;
        for area in &dirty {
            self.compose(*area);
        }
        dirty
    }

    fn has_blur(&self, blur: &Blur) -> bool {
        self.blurs.iter().any(|patch| patch.blur == *blur)
    }

    fn patch(&self, image: &Image, blur: Blur) -> Option<Patch> {
        let area = blur.rect.intersect(&image.bounds())?;
        let cell = (BLUR_CELLS[blur.level] * self.scale).round().max(2.0) as usize;
        Some(Patch {
            blur,
            area,
            pixels: blurred(image, area, cell, self.seed),
        })
    }

    /// Everything a focus changes besides the dimming: its lit area and shadow.
    fn footprint(&self, spot: Spot) -> Rect {
        let bounds = self.base.bounds();
        let reach = if spot.zoom > 1.0 {
            ((SHADOW_REACH + SHADOW_DROP) * self.scale).ceil() as i32
        } else {
            0
        };
        let lit = spot.lit(bounds).inflate(reach + 1);
        lit.intersect(&bounds).unwrap_or_default()
    }

    /// Redraws `area` of the output from `base`.
    fn compose(&mut self, area: Rect) {
        let bounds = self.base.bounds();
        let Some(area) = area.intersect(&bounds) else {
            return;
        };
        let Some(spot) = self.focus else {
            self.output.paste(&self.base, (0, 0), area);
            return;
        };
        let light = Light::new(spot, bounds, self.scale);
        let footprint = self.footprint(spot);
        let len = area.width() as usize * Image::BYTES_PER_PIXEL;
        // Where a column falls in `area`'s rows.
        let column = |x: i32| (x.clamp(area.left, area.right) - area.left) as usize;
        for y in area.top..area.bottom {
            let row = self.base.offset(area.left, y);
            let from = self.base.pixels()[row..row + len].as_chunks::<4>().0;
            let to = self.output.pixels_mut()[row..row + len]
                .as_chunks_mut::<4>()
                .0;
            let near = if (footprint.top..footprint.bottom).contains(&y) {
                column(footprint.left)..column(footprint.right)
            } else {
                0..0
            };
            let solid = if light.solid.contains(&y) {
                column(light.lit.left)..column(light.lit.right)
            } else {
                near.end..near.end
            };
            dim(&from[..near.start], &mut to[..near.start]);
            dim(&from[near.end..], &mut to[near.end..]);
            for i in (near.start..solid.start).chain(solid.end..near.end) {
                to[i] = light.shade(&self.base, area.left + i as i32, y, &from[i]);
            }
            let left = area.left + solid.start as i32;
            light.lens(&self.base, left, y, &mut to[solid]);
        }
    }
}

/// How a focus lights the pixels around it.
struct Light {
    /// Where the magnified area shows.
    lit: Rect,
    shape: RoundedRect,
    /// The shadow under a magnified area, and its reach.
    shadow: Option<(RoundedRect, f32)>,
    /// The rows clear of the corners, where all of `lit` is fully lit.
    solid: Range<i32>,
    /// Where each column and row of `lit` reads from.
    columns: Vec<Tap>,
    rows: Vec<Tap>,
}

impl Light {
    fn new(spot: Spot, bounds: Rect, scale: f32) -> Self {
        let lit = spot.lit(bounds);
        let shape = RoundedRect::new(lit, FOCUS_CORNER * scale);
        let corner = shape.radius.ceil() as i32;
        let shadow = (spot.zoom > 1.0).then(|| {
            let dropped = lit.offset(0, (SHADOW_DROP * scale).round() as i32);
            (
                RoundedRect::new(dropped, FOCUS_CORNER * scale),
                SHADOW_REACH * scale,
            )
        });
        let (source, target) = (center(spot.rect), center(lit));
        Self {
            lit,
            shape,
            shadow,
            solid: lit.top + corner..lit.bottom - corner,
            columns: taps(
                lit.left..lit.right,
                target.0,
                source.0,
                spot.zoom,
                bounds.right,
            ),
            rows: taps(
                lit.top..lit.bottom,
                target.1,
                source.1,
                spot.zoom,
                bounds.bottom,
            ),
        }
    }

    /// Fills `to` with the magnified pixels of row `y`, from column `left` on, all
    /// inside `lit`.
    fn lens(&self, image: &Image, left: i32, y: i32, to: &mut [[u8; 4]]) {
        if to.is_empty() {
            return;
        }
        let row = self.rows[(y - self.lit.top) as usize];
        let line = |y: usize| {
            let at = y * image.stride();
            image.pixels()[at..at + image.stride()].as_chunks::<4>().0
        };
        let (upper, lower) = (line(row.near), line(row.far));
        let columns = &self.columns[(left - self.lit.left) as usize..][..to.len()];
        for (to, column) in to.iter_mut().zip(columns) {
            let (a, b) = (upper[column.near], upper[column.far]);
            let (c, d) = (lower[column.near], lower[column.far]);
            let mix =
                |p: u8, q: u8| u32::from(p) * (256 - column.weight) + u32::from(q) * column.weight;
            let blend = |channel: usize| {
                let top = mix(a[channel], b[channel]);
                let bottom = mix(c[channel], d[channel]);
                ((top * (256 - row.weight) + bottom * row.weight + (1 << 15)) >> 16) as u8
            };
            *to = [blend(0), blend(1), blend(2), u8::MAX];
        }
    }

    /// A pixel on the edge of the lit area or in its shadow.
    fn shade(&self, image: &Image, x: i32, y: i32, from: &[u8; 4]) -> [u8; 4] {
        let point = (x as f32 + 0.5, y as f32 + 0.5);
        let mut keep = FOCUS_LIGHT;
        if let Some((shadow, reach)) = &self.shadow {
            let fade = (1.0 - shadow.distance(point).max(0.0) / reach).max(0.0);
            keep *= 1.0 - SHADOW_DARKNESS * fade * fade;
        }
        let coverage = if self.lit.contains(x, y) {
            (0.5 - self.shape.distance(point)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let mut inside = [[0; 4]];
        if coverage > 0.0 {
            self.lens(image, x, y, &mut inside);
        }
        let blend = |channel: usize| {
            let outside = f32::from(from[channel]) * keep;
            (outside + (f32::from(inside[0][channel]) - outside) * coverage).round() as u8
        };
        [blend(0), blend(1), blend(2), u8::MAX]
    }
}

/// Where a magnified column or row reads from: between `near` and `far`, `weight`
/// 256ths of the way.
#[derive(Clone, Copy)]
struct Tap {
    near: usize,
    far: usize,
    weight: u32,
}

/// The taps for `range`, magnified by `zoom` from around `source` to around `target`,
/// reading inside `0..len`.
fn taps(range: Range<i32>, target: f32, source: f32, zoom: f32, len: i32) -> Vec<Tap> {
    let last = len - 1;
    range
        .map(|at| {
            let position = source + (at as f32 + 0.5 - target) / zoom - 0.5;
            let position = position.clamp(0.0, last as f32);
            let near = position.floor();
            Tap {
                near: near as usize,
                far: (near as i32 + 1).min(last) as usize,
                weight: ((position - near) * 256.0).round() as u32,
            }
        })
        .collect()
}

/// The pixels away from the focus.
fn dim(from: &[[u8; 4]], to: &mut [[u8; 4]]) {
    const LIGHT: u32 = (FOCUS_LIGHT * 256.0) as u32;
    let dim = |value: u8| ((u32::from(value) * LIGHT + 128) >> 8) as u8;
    for (from, to) in from.iter().zip(to) {
        *to = [dim(from[0]), dim(from[1]), dim(from[2]), u8::MAX];
    }
}

fn center(rect: Rect) -> (f32, f32) {
    (
        (rect.left + rect.right) as f32 / 2.0,
        (rect.top + rect.bottom) as f32 / 2.0,
    )
}

/// A rectangle with rounded corners, measured from pixel edges.
struct RoundedRect {
    center: (f32, f32),
    half: (f32, f32),
    radius: f32,
}

impl RoundedRect {
    fn new(rect: Rect, radius: f32) -> Self {
        let half = (rect.width() as f32 / 2.0, rect.height() as f32 / 2.0);
        Self {
            center: center(rect),
            half,
            radius: radius.min(half.0).min(half.1),
        }
    }

    /// How far `point` lies outside the shape; negative inside.
    fn distance(&self, point: (f32, f32)) -> f32 {
        let qx = (point.0 - self.center.0).abs() - (self.half.0 - self.radius);
        let qy = (point.1 - self.center.1).abs() - (self.half.1 - self.radius);
        let (ox, oy) = (qx.max(0.0), qy.max(0.0));
        (ox * ox + oy * oy).sqrt() + qx.max(qy).min(0.0) - self.radius
    }
}

/// `area` of `image`, blurred beyond recovery: every `cell` × `cell` square is
/// averaged into one color, and those colors are blended smoothly into each other.
fn blurred(image: &Image, area: Rect, cell: usize, seed: u64) -> Image {
    let (width, height) = (area.width() as usize, area.height() as usize);
    let (columns, rows) = (width.div_ceil(cell), height.div_ceil(cell));

    let stride = width * Image::BYTES_PER_PIXEL;

    let mut sums = vec![[0u32; 3]; columns * rows];
    for y in 0..height {
        let row = image.offset(area.left, area.top + y as i32);
        let pixels = image.pixels()[row..row + stride].as_chunks::<4>().0;
        let sums = &mut sums[y / cell * columns..][..columns];
        for (sum, run) in sums.iter_mut().zip(pixels.chunks(cell)) {
            for pixel in run {
                for channel in 0..3 {
                    sum[channel] += u32::from(pixel[channel]);
                }
            }
        }
    }
    let side = |index: usize, len: usize| (len - index * cell).min(cell) as f32;
    let colors: Vec<[f32; 3]> = sums
        .iter()
        .enumerate()
        .map(|(at, sum)| {
            let count = side(at % columns, width) * side(at / columns, height);
            std::array::from_fn(|channel| {
                sum[channel] as f32 / count + BLUR_NOISE * noise(seed, at, channel)
            })
        })
        .collect();

    // Cubic B-spline blending, first across each row of cells, then down. Alpha
    // blends along as 255.
    let across: Vec<[(usize, f32); 4]> = (0..width).map(|x| spline(x, cell, columns)).collect();
    let mut wide = vec![0.0f32; rows * stride];
    for (line, colors) in wide
        .chunks_exact_mut(stride)
        .zip(colors.chunks_exact(columns))
    {
        for (to, taps) in line.as_chunks_mut::<4>().0.iter_mut().zip(&across) {
            for &(column, weight) in taps {
                for channel in 0..3 {
                    to[channel] += colors[column][channel] * weight;
                }
            }
            to[3] = 255.0;
        }
    }
    let mut pixels = vec![0; height * stride];
    for (y, to) in pixels.chunks_exact_mut(stride).enumerate() {
        let [a, b, c, d] =
            spline(y, cell, rows).map(|(row, weight)| (&wide[row * stride..][..stride], weight));
        for (i, value) in to.iter_mut().enumerate() {
            let blended = a.0[i] * a.1 + b.0[i] * b.1 + c.0[i] * c.1 + d.0[i] * d.1;
            // Rounds, and clamps what the noise pushed past black or white.
            *value = (blended + 0.5) as u8;
        }
    }
    Image::new(width as u32, height as u32, pixels)
}

/// The four cells around pixel `at` and their cubic B-spline weights.
fn spline(at: usize, cell: usize, count: usize) -> [(usize, f32); 4] {
    let position = (at as f32 + 0.5) / cell as f32 - 0.5;
    let first = position.floor();
    let t = position - first;
    let weights = [
        (1.0 - t).powi(3) / 6.0,
        (3.0 * t.powi(3) - 6.0 * t * t + 4.0) / 6.0,
        (-3.0 * t.powi(3) + 3.0 * t * t + 3.0 * t + 1.0) / 6.0,
        t.powi(3) / 6.0,
    ];
    std::array::from_fn(|i| {
        let index = (first as isize - 1 + i as isize).clamp(0, count as isize - 1);
        (index as usize, weights[i])
    })
}

/// A repeatable random number between -1 and 1 for each cell and channel.
fn noise(seed: u64, cell: usize, channel: usize) -> f32 {
    // SplitMix64's finalizer.
    let key = ((cell as u64) << 2) | channel as u64;
    let mut z = seed ^ key.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^= z >> 31;
    (z >> 40) as f32 / (1u64 << 23) as f32 - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ZOOMS;

    /// A 64x48 image of 1-pixel black and white stripes: the hardest thing to hide.
    fn stripes() -> Image {
        let pixels = (0..64 * 48)
            .flat_map(|i| {
                let v = if i % 2 == 0 { 0 } else { 255 };
                [v, v, v, 255]
            })
            .collect();
        Image::new(64, 48, pixels)
    }

    fn pixel(image: &Image, x: i32, y: i32) -> [u8; 4] {
        let at = image.offset(x, y);
        image.pixels()[at..at + 4].try_into().unwrap()
    }

    fn blur(rect: Rect) -> Blur {
        Blur { rect, level: 0 }
    }

    #[test]
    fn nothing_to_draw_shows_the_capture() {
        let image = stripes();
        let mut renderer = Renderer::new(&image, 1.0);
        assert!(renderer.update(&image, &[], None).is_empty());
        assert_eq!(renderer.output(), &image);
    }

    #[test]
    fn blur_flattens_inside_and_leaves_outside() {
        let image = stripes();
        let mut renderer = Renderer::new(&image, 1.0);
        let rect = Rect::new(8, 8, 40, 40);
        renderer.update(&image, &[blur(rect)], None);
        let output = renderer.output();
        for y in 0..48 {
            for x in 0..64 {
                let (shown, taken) = (pixel(output, x, y), pixel(&image, x, y));
                if rect.contains(x, y) {
                    assert!((80..=175).contains(&shown[0]), "{x},{y}: {shown:?}");
                } else {
                    assert_eq!(shown, taken, "{x},{y}");
                }
            }
        }
    }

    #[test]
    fn focus_dims_around_and_keeps_inside() {
        let image = stripes();
        let mut renderer = Renderer::new(&image, 1.0);
        let focus = Spot {
            rect: Rect::new(10, 10, 50, 40),
            zoom: 1.0,
        };
        renderer.update(&image, &[], Some(focus));
        let output = renderer.output();
        assert_eq!(pixel(output, 1, 1), [128, 128, 128, 255]);
        assert_eq!(pixel(output, 30, 20), pixel(&image, 30, 20));
        assert_eq!(pixel(output, 31, 20), pixel(&image, 31, 20));
    }

    #[test]
    fn magnifying_doubles_what_is_inside() {
        let pixels = (0..64 * 48)
            .flat_map(|i| [(i % 64) as u8 * 4, 0, 0, 255])
            .collect();
        let image = Image::new(64, 48, pixels);
        let mut renderer = Renderer::new(&image, 1.0);
        let focus = Spot {
            rect: Rect::new(24, 16, 40, 32),
            zoom: ZOOMS[3],
        };
        renderer.update(&image, &[], Some(focus));
        // The lit area spans columns 16..48 around the same center, and each column
        // shows half a column of the capture: 2 levels instead of 4.
        let output = renderer.output();
        let blue = |x| pixel(output, x, 24)[0];
        assert_eq!(blue(32), 127);
        assert_eq!(blue(36), 135);
        assert_eq!(blue(40), 143);
    }

    #[test]
    fn redrawing_only_changes_matches_drawing_from_scratch() {
        let image = stripes();
        let steps: [(&[Blur], Option<Spot>); 5] = [
            (&[blur(Rect::new(0, 0, 20, 20))], None),
            (
                &[
                    blur(Rect::new(0, 0, 20, 20)),
                    blur(Rect::new(10, 10, 30, 30)),
                ],
                Some(Spot {
                    rect: Rect::new(20, 20, 40, 40),
                    zoom: 1.5,
                }),
            ),
            (
                &[blur(Rect::new(10, 10, 30, 30))],
                Some(Spot {
                    rect: Rect::new(30, 10, 50, 30),
                    zoom: 2.0,
                }),
            ),
            (
                &[Blur {
                    rect: Rect::new(10, 10, 30, 30),
                    level: 2,
                }],
                Some(Spot {
                    rect: Rect::new(30, 10, 50, 30),
                    zoom: 1.0,
                }),
            ),
            (&[], None),
        ];
        let mut renderer = Renderer::new(&image, 1.0);
        for (blurs, focus) in steps {
            renderer.update(&image, blurs, focus);
            let mut fresh = Renderer::new(&image, 1.0);
            fresh.seed = renderer.seed;
            fresh.update(&image, blurs, focus);
            assert_eq!(renderer.output(), fresh.output());
        }
        assert_eq!(renderer.output(), &image);
    }

    #[test]
    fn noise_stays_in_range() {
        for cell in 0..1000 {
            let value = noise(7, cell, cell % 3);
            assert!((-1.0..1.0).contains(&value));
        }
    }
}
