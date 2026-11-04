//! Draws the edits onto the capture. The window shows these same pixels, so what
//! the user sees is exactly what gets saved.

use std::hash::{BuildHasher, RandomState};
use std::ops::Range;
use std::thread;

use crate::geometry::{RoundedRect, center};
use crate::{BLUR_LEVELS, Blur, Figure, Image, Rect, Spot, Step, figure, marker};

/// The side of the cells each blur level averages, in pixels at 100 % scale.
const BLUR_CELLS: [f32; BLUR_LEVELS] = [8.0, 14.0, 24.0];
/// How far each cell's brightness is randomly shifted, in levels out of 255: the same
/// for red, green and blue, so it never tints. Too little to see, but it keeps the
/// blur from being matched against guessed text.
const BLUR_NOISE: f32 = 4.0;
/// How each cell's average blends with the two cells on each side, across and then
/// down, before the cells blend into each other. Without it, detail finer than a
/// cell, such as text, mottles the blur.
const SOFTEN: [f32; 5] = [0.0625, 0.25, 0.375, 0.25, 0.0625];
/// How much light the image keeps around the focus.
const FOCUS_LIGHT: f32 = 0.5;
/// The lit area's corner radius at 100 %.
pub const FOCUS_CORNER: f32 = 8.0;
/// A magnified focus floats over the image: its shadow's reach and drop at 100 %,
/// and its darkness.
const SHADOW_REACH: f32 = 24.0;
const SHADOW_DROP: f32 = 4.0;
const SHADOW_DARKNESS: f32 = 0.4;
/// The fewest rows a core draws at once, so a small change isn't worth splitting.
const BAND_ROWS: usize = 32;

/// Keeps a rendering of the capture with its edits, redrawing only what changes.
pub struct Renderer {
    scale: f32,
    /// How many rows the focus draws at once.
    cores: usize,
    /// Makes this session's blur noise unpredictable.
    seed: u64,
    /// The capture with its blurs.
    base: Image,
    output: Image,
    /// The blurs in `base`.
    blurs: Vec<Patch>,
    focus: Option<Spot>,
    /// `base` blurred all over, while the focus blurs what's around it.
    haze: Option<Haze>,
    figures: Vec<Figure>,
    steps: Vec<Step>,
}

/// A blur and its pixels, which cover the part of it inside the image.
struct Patch {
    blur: Blur,
    area: Rect,
    pixels: Image,
}

/// The image blurred all over at a blur level.
struct Haze {
    level: usize,
    pixels: Image,
}

impl Renderer {
    /// `scale` is the display scale the capture was taken at.
    pub fn new(image: &Image, scale: f32) -> Self {
        Self {
            scale,
            cores: thread::available_parallelism().map_or(1, usize::from),
            seed: RandomState::new().hash_one(0),
            base: image.clone(),
            output: image.clone(),
            blurs: Vec::new(),
            focus: None,
            haze: None,
            figures: Vec::new(),
            steps: Vec::new(),
        }
    }

    pub fn output(&self) -> &Image {
        &self.output
    }

    /// Redraws `image` with `blurs`, `focus`, `figures` and `steps`, returning the
    /// areas that changed.
    pub fn update(
        &mut self,
        image: &Image,
        blurs: &[Blur],
        focus: Option<Spot>,
        figures: &[Figure],
        steps: &[Step],
    ) -> Vec<Rect> {
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

        // Around a focus that blurs, the haze follows `base`, and a change in it
        // spreads to the cells beside.
        let level = focus.and_then(|spot| spot.blur);
        let mut dirty = stale.clone();
        if self.haze.as_ref().map(|haze| haze.level) != level {
            self.haze = level.map(|level| Haze {
                level,
                pixels: blurred(
                    &self.base,
                    bounds,
                    cell(level, self.scale),
                    self.seed,
                    self.cores,
                ),
            });
        } else if self.haze.is_some() {
            dirty = stale.iter().map(|&area| self.rehaze(area)).collect();
        }
        // Showing, hiding or blurring around the focus changes the whole image.
        if self.focus.map(|spot| spot.blur) != focus.map(|spot| spot.blur) {
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
        // A figure moved, came or went.
        for at in 0..self.figures.len().max(figures.len()) {
            let (old, new) = (self.figures.get(at), figures.get(at));
            if old != new {
                let parts = figure::change(old, new).into_iter();
                dirty.extend(parts.filter_map(|part| part.intersect(&bounds)));
            }
        }
        // A step moved, came, went or took another number.
        for at in 0..self.steps.len().max(steps.len()) {
            let (old, new) = (self.steps.get(at), steps.get(at));
            if old != new {
                let marks = old.into_iter().chain(new);
                dirty.extend(
                    marks.filter_map(|step| marker::footprint(step.rect).intersect(&bounds)),
                );
            }
        }
        self.figures = figures.to_vec();
        self.steps = steps.to_vec();
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
        let cell = cell(blur.level, self.scale);
        Some(Patch {
            blur,
            area,
            pixels: blurred(image, area, cell, self.seed, self.cores),
        })
    }

    /// Blurs the haze again after `base` changed in `area`, and returns where the
    /// haze changed: each pixel blends the cells up to two away from its own, and each
    /// of those was softened with the cells up to two away from it.
    fn rehaze(&mut self, area: Rect) -> Rect {
        let Some(haze) = &mut self.haze else {
            return area;
        };
        let cell = cell(haze.level, self.scale);
        let bounds = self.base.bounds();
        // On the image's grid of cells, as the haze was blurred all at once.
        let around = |area: Rect| {
            let reach = (2 + SOFTEN.len() / 2) as i32 * cell as i32;
            let down = |at: i32| at - at.rem_euclid(cell as i32) - reach;
            let up = |at: i32| at + (-at).rem_euclid(cell as i32) + reach;
            let grown = Rect::new(
                down(area.left),
                down(area.top),
                up(area.right),
                up(area.bottom),
            );
            grown.intersect(&bounds).unwrap_or_default()
        };
        let changed = around(area);
        let read = around(changed);
        let pixels = blurred(&self.base, read, cell, self.seed, self.cores);
        haze.pixels.paste(&pixels, (read.left, read.top), changed);
        changed
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

    /// Redraws `area` of the output: `base` in the focus's light, with the figures and
    /// then the steps on top.
    fn compose(&mut self, area: Rect) {
        let Some(area) = area.intersect(&self.base.bounds()) else {
            return;
        };
        self.light(area);
        for figure in &self.figures {
            figure::draw(&mut self.output, figure, area);
        }
        for (at, step) in self.steps.iter().enumerate() {
            marker::draw(&mut self.output, step.rect, at + 1, area);
        }
    }

    /// Redraws `area` of the output from `base`, in bands of rows drawn at once on
    /// every core.
    fn light(&mut self, area: Rect) {
        let bounds = self.base.bounds();
        let Some(spot) = self.focus else {
            self.output.paste(&self.base, (0, 0), area);
            return;
        };
        let light = Light::new(spot, bounds, self.scale);
        let footprint = self.footprint(spot);
        let (base, stride) = (&self.base, self.base.stride());
        let around = self.haze.as_ref().map_or(base, |haze| &haze.pixels);
        let band = (area.height() as usize).div_ceil(self.cores).max(BAND_ROWS);
        let rows = area.top as usize * stride..area.bottom as usize * stride;
        thread::scope(|scope| {
            for (n, rows) in self.output.pixels_mut()[rows]
                .chunks_mut(band * stride)
                .enumerate()
            {
                let top = area.top + (n * band) as i32;
                let light = &light;
                scope.spawn(move || {
                    for (y, row) in (top..).zip(rows.chunks_exact_mut(stride)) {
                        light.row(base, around, footprint, area, y, row);
                    }
                });
            }
        });
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

    /// Draws the part of row `y` inside `area` into `to`, the output's whole row: the
    /// lit area from `base`, and `around` dimmed around it. `footprint` is where the
    /// focus does more than dim.
    fn row(
        &self,
        base: &Image,
        around: &Image,
        footprint: Rect,
        area: Rect,
        y: i32,
        to: &mut [u8],
    ) {
        let span = base.offset(area.left, 0)..base.offset(area.right, 0);
        let row = base.offset(0, y);
        let from = around.pixels()[row..][span.clone()].as_chunks::<4>().0;
        let to = to[span].as_chunks_mut::<4>().0;
        // Where a column falls in `area`'s rows.
        let column = |x: i32| (x.clamp(area.left, area.right) - area.left) as usize;
        let near = if (footprint.top..footprint.bottom).contains(&y) {
            column(footprint.left)..column(footprint.right)
        } else {
            0..0
        };
        let solid = if self.solid.contains(&y) {
            column(self.lit.left)..column(self.lit.right)
        } else {
            near.end..near.end
        };
        dim(&from[..near.start], &mut to[..near.start]);
        dim(&from[near.end..], &mut to[near.end..]);
        for i in (near.start..solid.start).chain(solid.end..near.end) {
            to[i] = self.shade(base, area.left + i as i32, y, &from[i]);
        }
        let left = area.left + solid.start as i32;
        self.lens(base, left, y, &mut to[solid]);
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

/// `area` of `image`, blurred beyond recovery: every `cell` × `cell` square is
/// averaged into one color, and those colors are blended smoothly into each other.
fn blurred(image: &Image, area: Rect, cell: usize, seed: u64, cores: usize) -> Image {
    let (width, height) = (area.width() as usize, area.height() as usize);
    let (columns, rows) = (width.div_ceil(cell), height.div_ceil(cell));

    let stride = width * Image::BYTES_PER_PIXEL;
    // Where the first cell falls on the image's grid of cells, so each cell's noise
    // stays the same however much of the image is blurred at once.
    let origin = (area.left as usize / cell, area.top as usize / cell);

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
    let averages: Vec<[f32; 3]> = sums
        .iter()
        .enumerate()
        .map(|(at, sum)| {
            let count = side(at % columns, width) * side(at / columns, height);
            sum.map(|sum| sum as f32 / count)
        })
        .collect();
    let softened = soften(&soften(&averages, columns, true), columns, false);
    let colors: Vec<[f32; 3]> = softened
        .iter()
        .enumerate()
        .map(|(at, color)| {
            let key = (origin.0 + at % columns, origin.1 + at / columns);
            let shift = BLUR_NOISE * noise(seed, key);
            color.map(|value| value + shift)
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
    // Down, the most work, in bands of rows on every core.
    let mut pixels = vec![0; height * stride];
    let band = height.div_ceil(cores).max(BAND_ROWS);
    thread::scope(|scope| {
        for (n, lines) in pixels.chunks_mut(band * stride).enumerate() {
            let wide = &wide;
            scope.spawn(move || {
                for (y, to) in (n * band..).zip(lines.chunks_exact_mut(stride)) {
                    let [a, b, c, d] = spline(y, cell, rows)
                        .map(|(row, weight)| (&wide[row * stride..][..stride], weight));
                    for (i, value) in to.iter_mut().enumerate() {
                        let blended = a.0[i] * a.1 + b.0[i] * b.1 + c.0[i] * c.1 + d.0[i] * d.1;
                        // Rounds, and clamps what the noise pushed past black or white.
                        *value = (blended + 0.5) as u8;
                    }
                }
            });
        }
    });
    Image::new(width as u32, height as u32, pixels)
}

/// The cells of `colors`, a grid `columns` wide, each blended with those beside it by
/// `SOFTEN`: across, or down. Past the edges, the edge cells stand in.
fn soften(colors: &[[f32; 3]], columns: usize, across: bool) -> Vec<[f32; 3]> {
    let rows = colors.len() / columns;
    let reach = SOFTEN.len() / 2;
    let near = |at: usize, tap: usize, len: usize| (at + tap).saturating_sub(reach).min(len - 1);
    (0..colors.len())
        .map(|at| {
            let (column, row) = (at % columns, at / columns);
            let mut color = [0.0; 3];
            for (tap, weight) in SOFTEN.iter().enumerate() {
                let from = if across {
                    row * columns + near(column, tap, columns)
                } else {
                    near(row, tap, rows) * columns + column
                };
                for (sum, value) in color.iter_mut().zip(colors[from]) {
                    *sum += value * weight;
                }
            }
            color
        })
        .collect()
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

/// The side of the cells blur `level` averages, in pixels at `scale`.
fn cell(level: usize, scale: f32) -> usize {
    (BLUR_CELLS[level] * scale).round().max(2.0) as usize
}

/// A repeatable random number between -1 and 1 for each cell, by its column and row.
fn noise(seed: u64, (column, row): (usize, usize)) -> f32 {
    // SplitMix64's finalizer.
    let key = ((row as u64) << 32) | column as u64;
    let mut z = seed ^ key.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^= z >> 31;
    (z >> 40) as f32 / (1u64 << 23) as f32 - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Form, ZOOMS};

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
        assert!(renderer.update(&image, &[], None, &[], &[]).is_empty());
        assert_eq!(renderer.output(), &image);
    }

    #[test]
    fn blur_flattens_inside_and_leaves_outside() {
        let image = stripes();
        let mut renderer = Renderer::new(&image, 1.0);
        let rect = Rect::new(8, 8, 40, 40);
        renderer.update(&image, &[blur(rect)], None, &[], &[]);
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
            blur: None,
        };
        renderer.update(&image, &[], Some(focus), &[], &[]);
        let output = renderer.output();
        assert_eq!(pixel(output, 1, 1), [128, 128, 128, 255]);
        assert_eq!(pixel(output, 30, 20), pixel(&image, 30, 20));
        assert_eq!(pixel(output, 31, 20), pixel(&image, 31, 20));
    }

    #[test]
    fn focus_can_blur_around_too() {
        let image = stripes();
        let mut renderer = Renderer::new(&image, 1.0);
        let focus = Spot {
            rect: Rect::new(10, 10, 50, 40),
            zoom: 1.0,
            blur: Some(0),
        };
        renderer.update(&image, &[], Some(focus), &[], &[]);
        let output = renderer.output();
        // Away from the focus, the stripes blend into a dimmed gray.
        for x in 0..4 {
            assert!((48..=80).contains(&pixel(output, x, 1)[0]), "{x}");
        }
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
            blur: None,
        };
        renderer.update(&image, &[], Some(focus), &[], &[]);
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
        let mark = |left, top| Step {
            rect: Rect::new(left, top, left + 12, top + 12),
        };
        let hazy = |zoom, level| Spot {
            rect: Rect::new(20, 20, 36, 36),
            zoom,
            blur: Some(level),
        };
        let figure = |form, from, to| Figure {
            form,
            from,
            to,
            line: 4,
        };
        let (square, ring, arrow) = (
            figure(Form::Rectangle, (4, 4), (40, 30)),
            figure(Form::Ellipse, (20, 8), (60, 44)),
            figure(Form::Arrow, (60, 40), (24, 12)),
        );
        // Each frame's blurs, focus, figures and steps.
        type Frame<'a> = (&'a [Blur], Option<Spot>, &'a [Figure], &'a [Step]);
        let frames: [Frame; 10] = [
            (
                &[blur(Rect::new(0, 0, 20, 20))],
                None,
                &[square],
                &[mark(30, 4)],
            ),
            (
                &[
                    blur(Rect::new(0, 0, 20, 20)),
                    blur(Rect::new(10, 10, 30, 30)),
                ],
                Some(Spot {
                    rect: Rect::new(20, 20, 40, 40),
                    zoom: 1.5,
                    blur: None,
                }),
                &[square, arrow],
                &[mark(30, 4), mark(2, 30)],
            ),
            (
                &[blur(Rect::new(10, 10, 30, 30))],
                Some(Spot {
                    rect: Rect::new(30, 10, 50, 30),
                    zoom: 2.0,
                    blur: None,
                }),
                &[ring, arrow],
                &[mark(2, 30)],
            ),
            (
                &[Blur {
                    rect: Rect::new(10, 10, 30, 30),
                    level: 2,
                }],
                Some(Spot {
                    rect: Rect::new(30, 10, 50, 30),
                    zoom: 1.0,
                    blur: None,
                }),
                &[figure(Form::Arrow, (8, 40), (8, 40)), ring],
                &[mark(2, 30), mark(40, 30)],
            ),
            // A blur moving away from the image's corner, under a haze.
            (
                &[blur(Rect::new(0, 0, 20, 20))],
                Some(hazy(1.0, 0)),
                &[ring],
                &[],
            ),
            (
                &[blur(Rect::new(40, 30, 52, 42))],
                Some(hazy(1.0, 0)),
                &[ring],
                &[],
            ),
            (
                &[blur(Rect::new(44, 30, 56, 42))],
                Some(hazy(1.5, 0)),
                &[],
                &[],
            ),
            (
                &[blur(Rect::new(44, 30, 56, 42))],
                Some(hazy(1.5, 2)),
                &[arrow],
                &[],
            ),
            (&[], None, &[arrow], &[mark(40, 30)]),
            (&[], None, &[], &[]),
        ];
        let mut renderer = Renderer::new(&image, 1.0);
        for (blurs, focus, figures, steps) in frames {
            renderer.update(&image, blurs, focus, figures, steps);
            let mut fresh = Renderer::new(&image, 1.0);
            fresh.seed = renderer.seed;
            fresh.update(&image, blurs, focus, figures, steps);
            assert_eq!(renderer.output(), fresh.output());
        }
        assert_eq!(renderer.output(), &image);
    }

    #[test]
    fn a_step_is_orange_with_a_white_edge_and_number() {
        let black = [0, 0, 0, 255].repeat(64 * 64);
        let image = Image::new(64, 64, black);
        let mut renderer = Renderer::new(&image, 1.0);
        let rect = Rect::new(18, 18, 46, 46);
        renderer.update(&image, &[], None, &[], &[Step { rect }]);
        let output = renderer.output();
        // The edge, the disc beside the number, and the 1's stem.
        assert_eq!(pixel(output, 32, 19), [255, 255, 255, 255]);
        assert_eq!(pixel(output, 22, 32), [34, 90, 236, 255]);
        assert_eq!(pixel(output, 33, 32), [255, 255, 255, 255]);
        assert_eq!(pixel(output, 2, 2), [0, 0, 0, 255]);
    }

    #[test]
    fn a_figure_is_an_orange_line_edged_in_white() {
        let black = [0, 0, 0, 255].repeat(64 * 64);
        let image = Image::new(64, 64, black);
        let mut renderer = Renderer::new(&image, 1.0);
        let figure = |form, from, to| Figure {
            form,
            from,
            to,
            line: 6,
        };
        let square = figure(Form::Rectangle, (8, 8), (55, 55));
        renderer.update(&image, &[], None, &[square], &[]);
        let output = renderer.output();
        // Across the left side: the edge, the orange, the edge, and the hollow.
        assert_eq!(pixel(output, 8, 32), [255, 255, 255, 255]);
        assert_eq!(pixel(output, 10, 32), [34, 90, 236, 255]);
        assert_eq!(pixel(output, 13, 32), [255, 255, 255, 255]);
        assert_eq!(pixel(output, 32, 32), [0, 0, 0, 255]);
        assert_eq!(pixel(output, 2, 32), [0, 0, 0, 255]);
        // An arrow's ink stops where the drag did: its edge runs through that pixel.
        let arrow = figure(Form::Arrow, (8, 32), (50, 32));
        renderer.update(&image, &[], None, &[arrow], &[]);
        let output = renderer.output();
        assert_eq!(pixel(output, 30, 32), [34, 90, 236, 255]);
        assert_eq!(pixel(output, 50, 32), [128, 128, 128, 255]);
        assert_eq!(pixel(output, 51, 32), [0, 0, 0, 255]);
    }

    #[test]
    fn noise_stays_in_range() {
        for cell in 0..1000 {
            let value = noise(7, (cell, cell / 7));
            assert!((-1.0..1.0).contains(&value));
        }
    }
}
