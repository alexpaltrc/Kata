//! Draws the figures: rectangles, ellipses and arrows, as lines of Kata's ink.

use std::f32::consts::FRAC_1_SQRT_2;

use crate::geometry::{RoundedRect, center, segment};
use crate::ink::Ink;
use crate::{Figure, Form, Image, Rect};

/// A usual figure's line, white edges included, beside a usual marker at 100 %.
pub(crate) const LINE: f32 = 6.0;
/// The white edge on each side of a usual line.
const EDGE: f32 = 1.0;
/// A rectangle's corner radius, along the middle of a usual line.
const CORNER: f32 = 5.0;
/// An arrow's head: how long and wide it gets, and how round its corners are, in
/// lines.
const HEAD: (f32, f32) = (4.0, 3.5);
const HEAD_ROUND: f32 = 0.2;
/// The share of a short arrow its head takes, and the smallest it gets, as a share of
/// its usual size.
const HEAD_SHARE: f32 = 0.4;
const SMALLEST_HEAD: f32 = 0.6;
/// The rows an arrow is inked in at once.
const BAND: i32 = 64;

/// Everything that changes when a figure turns from `old` into `new`: the lines and
/// shadows of both. While a figure is dragged the two mostly overlap, so each
/// overlapping pair of parts counts once.
pub fn change(old: Option<&Figure>, new: Option<&Figure>) -> Vec<Rect> {
    let parts = |figure: Option<&Figure>| figure.map_or_else(Vec::new, |f| Trace::new(f).parts());
    let (old, new) = (parts(old), parts(new));
    let mut change = Vec::new();
    for at in 0..old.len().max(new.len()) {
        match (old.get(at), new.get(at)) {
            (Some(a), Some(b)) if a.intersect(b).is_some() => change.push(a.union(b)),
            (a, b) => change.extend(a.into_iter().chain(b)),
        }
    }
    change
}

/// Draws `figure` over what `image` shows, only inside `area`.
pub fn draw(image: &mut Image, figure: &Figure, area: Rect) {
    let trace = Trace::new(figure);
    for part in trace.parts() {
        if let Some(part) = part.intersect(&area) {
            trace
                .ink
                .paint(image, part, |point| trace.outside(point), |_| 0.0);
        }
    }
}

/// A figure's line, ready to measure.
pub struct Trace {
    ink: Ink,
    /// Half the line's width.
    half: f32,
    /// The pixels from one end of the figure to the other.
    rect: Rect,
    path: Path,
}

/// Where a line runs.
enum Path {
    /// Along the outer edge of the line.
    Rectangle(RoundedRect),
    /// Along the middle of the line: the ellipse's center and radii.
    Ellipse((f32, f32), (f32, f32)),
    /// Along the middle of the shaft, and around the inside of the head: the shaft's
    /// ends and half its width, the head's corners and how round they are.
    Arrow {
        shaft: ((f32, f32), (f32, f32)),
        half: f32,
        head: [(f32, f32); 3],
        round: f32,
    },
}

impl Trace {
    pub fn new(figure: &Figure) -> Self {
        let size = figure.line as f32 / LINE;
        let half = figure.line as f32 / 2.0;
        let rect = figure.rect();
        let path = match figure.form {
            Form::Rectangle => Path::Rectangle(outer(figure)),
            Form::Ellipse => Path::Ellipse(
                center(rect),
                (
                    (rect.width() as f32 / 2.0 - half).max(0.5),
                    (rect.height() as f32 / 2.0 - half).max(0.5),
                ),
            ),
            Form::Arrow => arrow(figure, half),
        };
        Self {
            ink: Ink::new(size, EDGE),
            half,
            rect,
            path,
        }
    }

    /// How far `point` lies outside the line; negative on it.
    pub fn outside(&self, point: (f32, f32)) -> f32 {
        match &self.path {
            Path::Rectangle(outer) => (outer.distance(point) + self.half).abs() - self.half,
            Path::Ellipse(center, radii) => ellipse(point, *center, *radii).abs() - self.half,
            Path::Arrow {
                shaft,
                half,
                head,
                round,
            } => {
                let shaft = segment(point, shaft.0, shaft.1) - half;
                shaft.min(triangle(point, *head) - round)
            }
        }
    }

    /// Where the ink falls, in parts that don't overlap, so no pixel is inked twice: a
    /// frame around the hollow of a rectangle or an ellipse, and bands of rows along
    /// an arrow.
    fn parts(&self) -> Vec<Rect> {
        let margin = self.ink.margin();
        let line = 2.0 * self.half;
        let outer = self.rect.inflate(margin);
        match &self.path {
            Path::Rectangle(shape) => {
                // Inside the line, its inner corners round off a little.
                let corner = (shape.radius - line).max(0.0);
                let inset = (line + corner).ceil() as i32 + margin;
                frame(outer, self.rect.inflate(-inset))
            }
            Path::Ellipse(center, radii) => {
                // The largest rectangle inside the line's inner edge.
                let half = |radius: f32| (radius - self.half) * FRAC_1_SQRT_2 - margin as f32;
                let (across, down) = (half(radii.0), half(radii.1));
                let hollow = Rect::new(
                    (center.0 - across).ceil() as i32,
                    (center.1 - down).ceil() as i32,
                    (center.0 + across).floor() as i32,
                    (center.1 + down).floor() as i32,
                );
                frame(outer, hollow)
            }
            Path::Arrow { shaft, head, .. } => {
                // The head is convex, so its edges span all of it.
                let [a, b, c] = *head;
                let lines = [*shaft, (a, b), (b, c), (c, a)];
                let reach = self.half + margin as f32;
                let rows = lines.iter().flat_map(|&(a, b)| [a.1, b.1]);
                let top = (rows.clone().fold(f32::INFINITY, f32::min) - reach).floor() as i32;
                let bottom = (rows.fold(f32::NEG_INFINITY, f32::max) + reach).ceil() as i32;
                (top..bottom)
                    .step_by(BAND as usize)
                    .filter_map(|y| {
                        let end = (y + BAND).min(bottom);
                        let (left, right) = lines
                            .iter()
                            .filter_map(|&line| span(line, y as f32 - reach, end as f32 + reach))
                            .fold((f32::INFINITY, f32::NEG_INFINITY), |(l, r), (a, b)| {
                                (l.min(a), r.max(b))
                            });
                        (left <= right).then(|| {
                            let (left, right) = (left - reach, right + reach);
                            Rect::new(left.floor() as i32, y, right.ceil() as i32, end)
                        })
                    })
                    .collect()
            }
        }
    }
}

/// A rectangle's outer edge, its corners rounded.
pub(crate) fn outer(figure: &Figure) -> RoundedRect {
    let line = figure.line as f32;
    RoundedRect::new(figure.rect(), (CORNER / LINE + 0.5) * line)
}

/// An arrow: a filled head, its corners rounded, on a shaft. The head's tip and the
/// shaft's tail sit in from the drag's ends, so the ink stops where the drag did.
fn arrow(figure: &Figure, half: f32) -> Path {
    let pixel = |(x, y): (i32, i32)| (x as f32 + 0.5, y as f32 + 0.5);
    let (tail, tip) = (pixel(figure.from), pixel(figure.to));
    let (dx, dy) = (tip.0 - tail.0, tip.1 - tail.1);
    let length = dx.hypot(dy);
    if length < 1.0 {
        return Path::Arrow {
            shaft: (tail, tail),
            half,
            head: [tail; 3],
            round: 0.0,
        };
    }
    let (along, down) = (dx / length, dy / length);
    let line = 2.0 * half;
    let size = (HEAD_SHARE * length / (HEAD.0 * line)).clamp(SMALLEST_HEAD, 1.0);
    let [long, wide, round] = [HEAD.0, HEAD.1, HEAD_ROUND].map(|lines| lines * line * size);
    // A point `back` from the tip toward the tail, and `aside` from the middle.
    let at = |back: f32, aside: f32| {
        (
            tip.0 - along * back - down * aside,
            tip.1 - down * back + along * aside,
        )
    };
    let (base, side) = (long - round, wide / 2.0 - round);
    // The shaft runs into the head. Behind it, the shaft takes a line to reach its
    // width, so a short arrow never shows a stub.
    let half = half * ((length - long) / line).clamp(0.0, 1.0);
    Path::Arrow {
        shaft: (at(length - half, 0.0), at(base, 0.0)),
        half,
        head: [at(round, 0.0), at(base, side), at(base, -side)],
        round,
    }
}

/// How far `point` lies outside the triangle with `corners`; negative inside.
fn triangle(point: (f32, f32), corners: [(f32, f32); 3]) -> f32 {
    let [a, b, c] = corners;
    let edges = [(a, b), (b, c), (c, a)];
    let distance = edges
        .iter()
        .map(|&(a, b)| segment(point, a, b))
        .fold(f32::INFINITY, f32::min);
    // Inside, the point lies on the same side of every edge.
    let sides = edges.map(|(a, b)| (b.0 - a.0) * (point.1 - a.1) - (b.1 - a.1) * (point.0 - a.0));
    let inside = sides.iter().all(|&side| side > 0.0) || sides.iter().all(|&side| side < 0.0);
    if inside { -distance } else { distance }
}

/// How far `point` lies outside the ellipse around `center` with `radii`; negative
/// inside. Exact on the ellipse and close beside it, which is all a line needs.
fn ellipse(point: (f32, f32), center: (f32, f32), radii: (f32, f32)) -> f32 {
    let (x, y) = (point.0 - center.0, point.1 - center.1);
    let k0 = ((x / radii.0).powi(2) + (y / radii.1).powi(2)).sqrt();
    let k1 = ((x / radii.0.powi(2)).powi(2) + (y / radii.1.powi(2)).powi(2)).sqrt();
    if k1 > 0.0 {
        k0 * (k0 - 1.0) / k1
    } else {
        -radii.0.min(radii.1)
    }
}

/// The columns the part of line `a` to `b` between rows `top` and `bottom` spans.
fn span((a, b): ((f32, f32), (f32, f32)), top: f32, bottom: f32) -> Option<(f32, f32)> {
    let (low, high) = if a.1 <= b.1 { (a, b) } else { (b, a) };
    if high.1 < top || low.1 > bottom {
        return None;
    }
    let (start, end) = if high.1 > low.1 {
        let x = |y: f32| low.0 + (high.0 - low.0) * (y - low.1) / (high.1 - low.1);
        (x(top.max(low.1)), x(bottom.min(high.1)))
    } else {
        (a.0, b.0)
    };
    Some((start.min(end), start.max(end)))
}

/// `outer` without `hollow`, in up to four parts that don't overlap.
fn frame(outer: Rect, hollow: Rect) -> Vec<Rect> {
    if hollow.is_empty() {
        return vec![outer];
    }
    vec![
        Rect::new(outer.left, outer.top, outer.right, hollow.top),
        Rect::new(outer.left, hollow.top, hollow.left, hollow.bottom),
        Rect::new(hollow.right, hollow.top, outer.right, hollow.bottom),
        Rect::new(outer.left, hollow.bottom, outer.right, outer.bottom),
    ]
}
