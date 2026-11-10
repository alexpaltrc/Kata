//! Draws the edits onto the capture. The window shows these same pixels, so what
//! the user sees is exactly what gets saved.

use std::borrow::Cow;
use std::hash::{BuildHasher, RandomState};
use std::ops::Range;
use std::thread;

use crate::document::radii;
use crate::geometry::{RoundedRect, center, ellipse};
use crate::{BLUR_LEVELS, Blur, Figure, Frame, Image, Rect, Spot, Step, figure, marker};

/// How far each blur level spreads every pixel, in pixels at 100 % scale: the
/// standard deviation of the Gaussian blur it follows.
const BLUR_SIZES: [f32; BLUR_LEVELS] = [10.0, 16.0, 26.0];
/// A blur first averages the image into cells this many times smaller than its
/// spread, so a strong blur costs no more than a soft one. Bigger cells would let fine
/// detail, such as text, mottle the blur.
const CELLS_PER_SIZE: f32 = 3.0;
/// How far each blurred pixel's brightness is randomly shifted, in levels out of 255:
/// the same for red, green and blue, so it never tints. A grain too fine to notice,
/// which keeps smooth blurs from banding and the blur from being matched against
/// guessed text.
const GRAIN: f32 = 3.0;
/// How many random shifts the grain holds, read again from a new place on each row.
const GRAIN_RUN: usize = 4096;
/// How much light the image keeps around the focus.
const FOCUS_LIGHT: f32 = 0.5;
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
    /// Makes this session's blur grain unpredictable.
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

    /// Starts over on `image`, another capture, with nothing drawn on it yet: the
    /// next update draws every edit.
    pub fn restart(&mut self, image: &Image) {
        *self = Self {
            base: image.clone(),
            output: image.clone(),
            ..self.alike()
        };
    }

    /// Another renderer whose blurs show this one's grain, with no capture yet: to
    /// draw ahead, elsewhere, once `restart` gives it one.
    pub fn alike(&self) -> Self {
        let none = Image::new(0, 0, Vec::new());
        Self {
            base: none.clone(),
            output: none,
            blurs: Vec::new(),
            focus: None,
            haze: None,
            figures: Vec::new(),
            steps: Vec::new(),
            ..*self
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
                    Kernel::new(level, self.scale),
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
        let kernel = Kernel::new(blur.level, self.scale);
        Some(Patch {
            blur,
            area,
            pixels: blurred(image, area, kernel, self.seed, self.cores),
        })
    }

    /// Blurs the haze again after `base` changed in `area`, and returns where the
    /// haze changed: as far as its kernel reaches from the cells `area` touches.
    fn rehaze(&mut self, area: Rect) -> Rect {
        let Some(haze) = &mut self.haze else {
            return area;
        };
        let kernel = Kernel::new(haze.level, self.scale);
        let (cell, reach) = (kernel.cell as i32, (kernel.reach() * kernel.cell) as i32);
        let bounds = self.base.bounds();
        // On the image's grid of cells, as the haze was blurred all at once.
        let around = |area: Rect| {
            let down = |at: i32| at - at.rem_euclid(cell) - reach;
            let up = |at: i32| at + (-at).rem_euclid(cell) + reach;
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
        let pixels = blurred(&self.base, read, kernel, self.seed, self.cores);
        haze.pixels.paste(&pixels, (read.left, read.top), changed);
        changed
    }

    /// Everything a focus changes besides the dimming: its lit area, its shadow and its
    /// border.
    fn footprint(&self, spot: Spot) -> Rect {
        let bounds = self.base.bounds();
        let reach = if spot.zoom > 1.0 {
            ((SHADOW_REACH + SHADOW_DROP) * self.scale).ceil() as i32
        } else {
            0
        };
        let lit = spot.lit(bounds).inflate(reach + 1);
        let border = spot.border(bounds, self.scale);
        let ink = figure::change(None, border.as_ref());
        let all = ink.iter().fold(lit, |all, part| all.union(part));
        all.intersect(&bounds).unwrap_or_default()
    }

    /// Redraws `area` of the output: `base` in the focus's light, with its border, the
    /// figures and then the steps on top.
    fn compose(&mut self, area: Rect) {
        let bounds = self.base.bounds();
        let Some(area) = area.intersect(&bounds) else {
            return;
        };
        self.light(area);
        if let Some(border) = self.focus.and_then(|spot| spot.border(bounds, self.scale)) {
            figure::draw(&mut self.output, &border, area, self.cores);
        }
        for figure in &self.figures {
            figure::draw(&mut self.output, figure, area, self.cores);
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
    edge: Edge,
    /// The shadow under a magnified area, and its reach.
    shadow: Option<(Edge, f32)>,
    /// Where each column and row of `lit` reads from.
    columns: Vec<Tap>,
    rows: Vec<Tap>,
}

impl Light {
    fn new(spot: Spot, bounds: Rect, scale: f32) -> Self {
        let lit = spot.lit(bounds);
        let edge = |rect: Rect| match spot.frame {
            Frame::Ellipse => Edge::Oval(center(rect), radii(rect)),
            Frame::Soft | Frame::Rectangle => {
                Edge::Round(RoundedRect::new(rect, spot.corner(bounds, scale)))
            }
        };
        let shadow = (spot.zoom > 1.0).then(|| {
            let dropped = lit.offset(0, (SHADOW_DROP * scale).round() as i32);
            (edge(dropped), SHADOW_REACH * scale)
        });
        let (source, target) = (center(spot.rect), center(lit));
        Self {
            lit,
            edge: edge(lit),
            shadow,
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
        let inner = self.inner(y);
        let solid = if inner.is_empty() {
            near.end..near.end
        } else {
            column(inner.start)..column(inner.end)
        };
        dim(&from[..near.start], &mut to[..near.start]);
        dim(&from[near.end..], &mut to[near.end..]);
        for i in (near.start..solid.start).chain(solid.end..near.end) {
            to[i] = self.shade(base, area.left + i as i32, y, &from[i]);
        }
        let left = area.left + solid.start as i32;
        self.lens(base, left, y, &mut to[solid]);
    }

    /// The columns of row `y` fully inside the lit area.
    fn inner(&self, y: i32) -> Range<i32> {
        let lit = self.lit;
        if !(lit.top..lit.bottom).contains(&y) {
            return 0..0;
        }
        let (left, right) = match &self.edge {
            Edge::Round(shape) => {
                let corner = shape.radius.ceil() as i32;
                let clear = (lit.top + corner..lit.bottom - corner).contains(&y);
                let inset = if clear { 0 } else { corner };
                (lit.left + inset, lit.right - inset)
            }
            Edge::Oval(center, radii) => {
                // Inside this smaller ellipse, every pixel lies half a pixel in or more,
                // so it's fully lit.
                let shrink = 1.0 - 0.5 / radii.0.min(radii.1);
                let (across, down) = (radii.0 * shrink, radii.1 * shrink);
                let dy = y as f32 + 0.5 - center.1;
                if shrink <= 0.0 || dy.abs() >= down {
                    return 0..0;
                }
                let half = across * (1.0 - (dy / down).powi(2)).sqrt();
                let left = (center.0 - half - 0.5).ceil() as i32;
                let right = (center.0 + half - 0.5).floor() as i32 + 1;
                (left.max(lit.left), right.min(lit.right))
            }
        };
        left..right.max(left)
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
            (0.5 - self.edge.distance(point)).clamp(0.0, 1.0)
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

/// The edge of a lit area or of its shadow.
enum Edge {
    Round(RoundedRect),
    /// An ellipse's center and radii.
    Oval((f32, f32), (f32, f32)),
}

impl Edge {
    /// How far `point` lies outside the edge; negative inside.
    fn distance(&self, point: (f32, f32)) -> f32 {
        match self {
            Self::Round(shape) => shape.distance(point),
            Self::Oval(center, radii) => ellipse(point, *center, *radii),
        }
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

/// A cell's blue, green and red, in 1/256 levels. The fourth goes unused, so a cell
/// fills a whole lane of the processor.
type Cell = [u16; 4];

/// How a blur level blurs. The image is averaged into square cells, the cells are
/// blurred by three boxes across and three down, which together follow a Gaussian
/// closely, and the cells blend smoothly back into pixels.
#[derive(Clone, Copy)]
struct Kernel {
    /// The cells' side, in pixels.
    cell: usize,
    /// How many cells each box reaches to each side.
    radii: [usize; 3],
}

impl Kernel {
    /// The kernel for blur `level` at display `scale`.
    fn new(level: usize, scale: f32) -> Self {
        let size = BLUR_SIZES[level] * scale;
        let cell = (size / CELLS_PER_SIZE).round().max(1.0) as usize;
        let side = cell as f32;
        // Averaging into cells and blending them back blur a little too, and the
        // boxes add the rest. Spreads add up as variances, here in cells.
        let variance =
            ((size * size - (side * side - 1.0) / 12.0) / (side * side) - 1.0 / 3.0).max(0.0);
        // Boxes of two odd widths a step apart, as many of each as comes closest
        // (Kovesi, "Fast almost-Gaussian filtering").
        let ideal = (4.0 * variance + 1.0).sqrt();
        let narrow = ((ideal + 1.0) / 2.0).floor() * 2.0 - 1.0;
        let narrows =
            (3.0 * narrow * narrow + 12.0 * narrow + 9.0 - 12.0 * variance) / (4.0 * narrow + 4.0);
        let radii = std::array::from_fn(|at| {
            (narrow as usize - 1) / 2 + usize::from(at as f32 >= narrows.round())
        });
        Self { cell, radii }
    }

    /// How many cells away a change in one cell shows: as far as the boxes reach,
    /// and then the two cells each pixel blends on each side.
    fn reach(&self) -> usize {
        self.radii.iter().sum::<usize>() + 2
    }
}

/// `area` of `image`, blurred beyond recovery by `kernel`, with its grain.
fn blurred(image: &Image, area: Rect, kernel: Kernel, seed: u64, cores: usize) -> Image {
    let cell = kernel.cell;
    let (width, height) = (area.width() as usize, area.height() as usize);
    let (columns, rows) = (width.div_ceil(cell), height.div_ceil(cell));
    let cells = across(image, area, kernel, cores);
    let grain = Grain::new(seed, area.right as usize);

    // Down, and blended back into pixels with cubic B-splines, in bands of rows on
    // every core. Each band blurs down only the rows of cells it blends, and those
    // the boxes reach from them, blends each of those rows across into a row of
    // pixels, and then each row of pixels down from four of them.
    let stride = width * Image::BYTES_PER_PIXEL;
    let taps: Vec<[(usize, f32); 4]> = (0..width).map(|x| spline(x, cell, columns)).collect();
    let mut pixels = vec![0; height * stride];
    let band = height.div_ceil(cores).max(BAND_ROWS);
    thread::scope(|scope| {
        for (n, lines) in pixels.chunks_mut(band * stride).enumerate() {
            let (cells, taps, grain) = (&cells, &taps, &grain);
            scope.spawn(move || {
                let top = n * band;
                let bottom = top + lines.len() / stride;
                let blended = spline(top, cell, rows)[0].0..spline(bottom - 1, cell, rows)[3].0 + 1;
                // The rows each box gives, from the last box's back to what the first
                // reads.
                let mut wanted = vec![blended.clone()];
                for &radius in kernel.radii.iter().rev() {
                    let last = &wanted[wanted.len() - 1];
                    wanted.push(last.start.saturating_sub(radius)..(last.end + radius).min(rows));
                }
                let read = wanted.pop().unwrap_or_default();
                let mut done = Cow::Borrowed(&cells[read.start * columns..read.end * columns]);
                for (&radius, range) in kernel.radii.iter().zip(wanted.into_iter().rev()) {
                    done = Cow::Owned(boxed(&done, columns, rows, range, radius));
                }

                // The four rows of cells blended across that the row of pixels at
                // hand blends down, each in the slot its row's number picks.
                let mut wide = vec![[0.0f32; 4]; 4 * width];
                let mut next = blended.start;
                for (y, line) in (top..).zip(lines.chunks_exact_mut(stride)) {
                    let rows = spline(y, cell, rows);
                    while next <= rows[3].0 {
                        let from = &done[(next - blended.start) * columns..][..columns];
                        let to = &mut wide[next % 4 * width..][..width];
                        for (to, taps) in to.iter_mut().zip(taps) {
                            *to = [0.0; 4];
                            for &(column, weight) in taps {
                                for lane in 0..4 {
                                    to[lane] += f32::from(from[column][lane]) * weight;
                                }
                            }
                        }
                        next += 1;
                    }
                    let [(a, wa), (b, wb), (c, wc), (d, wd)] = rows
                        .map(|(row, weight)| (&wide[row % 4 * width..][..width], weight / 256.0));
                    let shifts = &grain.row(area.top as usize + y)[area.left as usize..];
                    let line = line.as_chunks_mut::<4>().0;
                    let pixels = line.iter_mut().zip(a).zip(b).zip(c).zip(d);
                    for (((((to, a), b), c), d), &shift) in pixels.zip(shifts) {
                        // Opacity is pushed far past opaque, so it comes out opaque.
                        let shifts = [shift, shift, shift, 256.0];
                        *to = std::array::from_fn(|lane| {
                            let color = a[lane] * wa + b[lane] * wb + c[lane] * wc + d[lane] * wd;
                            // Clamps what the grain pushed past black or white. Adding
                            // 1.5 × 2²³ leaves the level, rounded, in the lowest byte.
                            let level = (color + shifts[lane]).clamp(0.0, 255.0);
                            (level + 12_582_912.0).to_bits() as u8
                        });
                    }
                }
            });
        }
    });
    Image::new(width as u32, height as u32, pixels)
}

/// `area` of `image` averaged into the cells of `kernel` and blurred across by its
/// boxes, in bands of rows on every core.
fn across(image: &Image, area: Rect, kernel: Kernel, cores: usize) -> Vec<Cell> {
    let cell = kernel.cell;
    let (width, height) = (area.width() as usize, area.height() as usize);
    let (columns, rows) = (width.div_ceil(cell), height.div_ceil(cell));
    let stride = width * Image::BYTES_PER_PIXEL;
    let mut cells = vec![[0; 4]; columns * rows];
    let band = rows.div_ceil(cores).max(BAND_ROWS.div_ceil(cell));
    thread::scope(|scope| {
        for (n, lines) in cells.chunks_mut(band * columns).enumerate() {
            scope.spawn(move || {
                let mut sums = vec![[0u32; 4]; columns];
                for (row, line) in (n * band..).zip(lines.chunks_exact_mut(columns)) {
                    sums.fill([0; 4]);
                    let tall = (height - row * cell).min(cell);
                    for y in row * cell..row * cell + tall {
                        let at = image.offset(area.left, area.top + y as i32);
                        let pixels = image.pixels()[at..at + stride].as_chunks::<4>().0;
                        for (sum, run) in sums.iter_mut().zip(pixels.chunks(cell)) {
                            for pixel in run {
                                for lane in 0..4 {
                                    sum[lane] += u32::from(pixel[lane]);
                                }
                            }
                        }
                    }
                    for (column, (to, sum)) in line.iter_mut().zip(&sums).enumerate() {
                        let share = 256.0 / (tall * (width - column * cell).min(cell)) as f32;
                        *to = sum.map(|sum| (sum as f32 * share + 0.5) as u16);
                    }
                    // A row of cells is a grid one cell wide, on its side.
                    for &radius in &kernel.radii {
                        let done = boxed(line, 1, columns, 0..columns, radius);
                        line.copy_from_slice(&done);
                    }
                }
            });
        }
    });
    cells
}

/// Rows `range` of a grid of cells `columns` wide and `rows` tall, blurred down by a
/// box `radius` cells to each side. `from` holds the rows the box reaches from
/// `range`, those in the grid. Near the grid's top and bottom, the box averages only
/// the cells there are. The sums are whole numbers, so each cell comes out the same
/// however much of the grid is blurred at once.
fn boxed(
    from: &[Cell],
    columns: usize,
    rows: usize,
    range: Range<usize>,
    radius: usize,
) -> Vec<Cell> {
    let first = range.start.saturating_sub(radius);
    let row = |at: usize| &from[(at - first) * columns..][..columns];
    let add = |sums: &mut [[i32; 4]], at: usize, sign: i32| {
        for (sum, cell) in sums.iter_mut().zip(row(at)) {
            for lane in 0..4 {
                sum[lane] += sign * i32::from(cell[lane]);
            }
        }
    };
    // The box around the first row, but for its last row.
    let mut sums = vec![[0i32; 4]; columns];
    for at in first..(range.start + radius).min(rows) {
        add(&mut sums, at, 1);
    }
    let mut to = Vec::with_capacity(range.len() * columns);
    for at in range {
        if at + radius < rows {
            add(&mut sums, at + radius, 1);
        }
        if let Some(gone) = at.checked_sub(radius + 1)
            && gone >= first
        {
            add(&mut sums, gone, -1);
        }
        let count = ((at + radius).min(rows - 1) + 1 - at.saturating_sub(radius)) as f32;
        to.extend(
            sums.iter()
                .map(|sum| sum.map(|sum| (sum as f32 / count + 0.5) as u16)),
        );
    }
    to
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

/// A blur's grain: a run of random brightness shifts, which each row of pixels
/// starts reading at its own random place, so no pattern shows. The same `seed`
/// always gives the same grain.
struct Grain {
    seed: u64,
    shifts: Vec<f32>,
}

impl Grain {
    /// The grain for images up to `width` pixels wide.
    fn new(seed: u64, width: usize) -> Self {
        let shifts = (0..GRAIN_RUN + width)
            .map(|at| {
                let random = random(seed, at as u64) >> 40;
                GRAIN * (random as f32 / (1 << 23) as f32 - 1.0)
            })
            .collect();
        Self { seed, shifts }
    }

    /// The shifts of row `row` of the image, from its first column.
    fn row(&self, row: usize) -> &[f32] {
        &self.shifts[random(!self.seed, row as u64) as usize % GRAIN_RUN..]
    }
}

/// A repeatable random number for each `key`: SplitMix64's finalizer.
fn random(seed: u64, key: u64) -> u64 {
    let mut z = seed ^ key.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
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
    fn restarted_on_another_capture_it_draws_every_edit_again() {
        let (first, second) = (Image::new(64, 48, [90; 4].repeat(64 * 48)), stripes());
        let rect = Rect::new(8, 8, 40, 40);
        let mut renderer = Renderer::new(&first, 1.0);
        renderer.update(&first, &[blur(rect)], None, &[], &[]);
        renderer.restart(&second);
        assert_eq!(renderer.output(), &second);
        assert_eq!(
            renderer.update(&second, &[blur(rect)], None, &[], &[]),
            [rect]
        );
        let output = renderer.output();
        for y in 0..48 {
            for x in 0..64 {
                let shown = pixel(output, x, y);
                if rect.contains(x, y) {
                    assert!((80..=175).contains(&shown[0]), "{x},{y}: {shown:?}");
                } else {
                    assert_eq!(shown, pixel(&second, x, y), "{x},{y}");
                }
            }
        }
    }

    #[test]
    fn one_alike_draws_the_same_grain() {
        let image = stripes();
        let rect = Rect::new(8, 8, 40, 40);
        let mut renderer = Renderer::new(&image, 1.0);
        renderer.update(&image, &[blur(rect)], None, &[], &[]);
        let mut alike = renderer.alike();
        alike.restart(&image);
        alike.update(&image, &[blur(rect)], None, &[], &[]);
        assert_eq!(alike.output(), renderer.output());
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
    fn blur_spreads_as_far_as_its_level_says() {
        // An edge from black to white: a Gaussian blur's spread is half the distance
        // its ramp takes from 16 % to 84 % of the way.
        let width = 400;
        let pixels = (0..width * 100)
            .flat_map(|at| [if at % width < width / 2 { 0 } else { 255 }; 4])
            .collect();
        let image = Image::new(width as u32, 100, pixels);
        for (level, size) in BLUR_SIZES.iter().enumerate() {
            let blurred = blurred(&image, image.bounds(), Kernel::new(level, 1.0), 7, 4);
            // Down a column, the grain averages out.
            let column = |x: i32| {
                (0..100)
                    .map(|y| f32::from(pixel(&blurred, x, y)[0]))
                    .sum::<f32>()
                    / 100.0
            };
            let reach = |share: f32| (0..width).find(|&x| column(x) >= share * 255.0);
            let spread = (reach(0.8413).unwrap() - reach(0.1587).unwrap()) as f32 / 2.0;
            assert!(
                (spread - size).abs() <= size * 0.1,
                "level {level}: {spread}"
            );
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
            frame: Frame::Soft,
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
            frame: Frame::Soft,
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
            frame: Frame::Soft,
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
    fn a_framed_focus_is_edged_in_ink() {
        let white = [255; 4].repeat(100 * 100);
        let image = Image::new(100, 100, white);
        for frame in [Frame::Rectangle, Frame::Ellipse] {
            let mut renderer = Renderer::new(&image, 1.0);
            let focus = Spot {
                rect: Rect::new(10, 10, 90, 70),
                zoom: 1.0,
                blur: None,
                frame,
            };
            renderer.update(&image, &[], Some(focus), &[], &[]);
            let output = renderer.output();
            // Across the left side: the edge, the orange, the edge, and the lit area;
            // and away from it, the dimmed image. An ellipse curves, so its edges
            // take a hint of orange.
            let white = |x| pixel(output, x, 40).iter().all(|&v| v >= 252);
            assert!(white(10) && white(15), "{frame:?}");
            assert_eq!(pixel(output, 12, 40), [34, 90, 236, 255], "{frame:?}");
            assert_eq!(pixel(output, 50, 40), [255; 4], "{frame:?}");
            assert_eq!(pixel(output, 2, 40), [128, 128, 128, 255], "{frame:?}");
        }
    }

    #[test]
    fn an_ellipse_focus_leaves_its_corners_dim() {
        let image = stripes();
        let mut renderer = Renderer::new(&image, 1.0);
        let focus = Spot {
            rect: Rect::new(4, 4, 60, 44),
            zoom: 1.0,
            blur: None,
            frame: Frame::Ellipse,
        };
        renderer.update(&image, &[], Some(focus), &[], &[]);
        let output = renderer.output();
        assert_eq!(pixel(output, 5, 5), [128, 128, 128, 255]);
        assert_eq!(pixel(output, 32, 24), pixel(&image, 32, 24));
        assert_eq!(pixel(output, 33, 24), pixel(&image, 33, 24));
    }

    #[test]
    fn what_is_drawn_whole_is_fully_inside() {
        let bounds = Rect::new(0, 0, 200, 200);
        let rects = [
            Rect::new(20, 20, 120, 80),
            Rect::new(50, 10, 53, 90),
            Rect::new(10, 50, 190, 52),
        ];
        for rect in rects {
            for frame in [Frame::Soft, Frame::Rectangle, Frame::Ellipse] {
                for zoom in [1.0, 1.5] {
                    let spot = Spot {
                        rect,
                        zoom,
                        blur: None,
                        frame,
                    };
                    let light = Light::new(spot, bounds, 1.25);
                    for y in bounds.top - 1..=bounds.bottom {
                        for x in light.inner(y) {
                            let point = (x as f32 + 0.5, y as f32 + 0.5);
                            let distance = light.edge.distance(point);
                            assert!(distance <= -0.5 + 1e-4, "{spot:?} {x},{y}: {distance}");
                        }
                    }
                }
            }
        }
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
            frame: Frame::Soft,
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
        type Scene<'a> = (&'a [Blur], Option<Spot>, &'a [Figure], &'a [Step]);
        let frames: [Scene; 10] = [
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
                    frame: Frame::Ellipse,
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
                    frame: Frame::Rectangle,
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
                    frame: Frame::Ellipse,
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
        // An arrow's ink stops where the drag did: its edges run through those pixels,
        // half white.
        let arrow = figure(Form::Arrow, (8, 32), (50, 32));
        renderer.update(&image, &[], None, &[arrow], &[]);
        let output = renderer.output();
        let half_white = |x| {
            pixel(output, x, 32)[..3]
                .iter()
                .all(|v| v.abs_diff(128) <= 1)
        };
        assert!(half_white(8) && half_white(50));
        assert_eq!(pixel(output, 30, 32), [34, 90, 236, 255]);
        assert_eq!(pixel(output, 7, 32), [0, 0, 0, 255]);
        assert_eq!(pixel(output, 51, 32), [0, 0, 0, 255]);
    }

    #[test]
    fn grain_stays_fine() {
        let grain = Grain::new(7, 100);
        assert!(grain.shifts.iter().all(|shift| shift.abs() <= GRAIN));
        assert!(grain.shifts.iter().any(|&shift| shift != grain.shifts[0]));
    }
}
