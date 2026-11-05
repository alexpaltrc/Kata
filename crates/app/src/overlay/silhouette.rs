//! The hint the first few captures show: the outline of a window others cover,
//! hidden parts and all, traced in Kata's signature colors from the point nearest
//! the pointer, the way the logo draws itself. Pointing at the window shows it
//! whole, and the outline fades as the selection lands on it.

use std::time::Instant;

use kata_capture::Screenshot;
use kata_editor::Rect;
use kata_ui::{
    motion::{self, Animation, Easing},
    theme::{self, Palette},
};
use windows::{
    Win32::Graphics::Direct2D::{
        Common::{
            D2D_RECT_F, D2D_SIZE_F, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_OPEN,
            D2D1_GRADIENT_STOP,
        },
        D2D1_ARC_SEGMENT, D2D1_ARC_SIZE_SMALL, D2D1_CAP_STYLE_FLAT, D2D1_DASH_STYLE_CUSTOM,
        D2D1_DEFAULT_FLATTENING_TOLERANCE, D2D1_EXTEND_MODE_CLAMP, D2D1_GAMMA_2_2,
        D2D1_LINE_JOIN_ROUND, D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES, D2D1_STROKE_STYLE_PROPERTIES,
        D2D1_SWEEP_DIRECTION_CLOCKWISE, ID2D1Factory, ID2D1LinearGradientBrush, ID2D1PathGeometry,
        ID2D1RenderTarget, ID2D1SolidColorBrush, ID2D1StrokeStyle,
    },
    core::Result,
};
use windows_numerics::Vector2;

use super::around;

/// In seconds: how long the hint lets the overlay settle, and how long it takes to
/// trace.
const DELAY: f32 = 0.25;
const TRACE: f32 = 0.8;
/// The pen's pace, the logo's: off quickly, slowing to a close.
const PEN: Easing = Easing::new(0.4, 0.0, 0.2, 1.0);
/// At 100 % scale: how thick the outline is, and how far its shadow reaches past
/// it on each side.
const WIDTH: f32 = 2.0;
const SHADOW: f32 = 1.0;
/// How dark the shadow is, so the outline stands out over light windows too.
const SHADE: f32 = 0.25;
/// How much of a covered window must show, at 100 % scale, for the pointer to
/// reach it easily.
const REACH: f32 = 48.0;

/// A covered window's outline, and how far it has been traced.
pub struct Silhouette {
    /// Which window, among the shot's.
    pub index: usize,
    outline: Outline,
    colors: [D2D1_COLOR_F; 4],
    /// How thick the shadow is: none in a contrast theme.
    shadow: f32,
    /// The seconds since the overlay appeared.
    clock: Animation,
    /// 1 while it shows, heading to 0 once the pointer reaches the window or a drag
    /// begins.
    opacity: Animation,
}

impl Silhouette {
    /// The hint for `shot`, with the pointer at `pointer`, over window `under`: on
    /// the frontmost covered window but that one, with enough of it showing for the
    /// pointer to reach it easily.
    pub fn pick(shot: &Screenshot, pointer: (i32, i32), under: Option<usize>) -> Option<Self> {
        let windows = &shot.windows;
        let index = (0..windows.len()).find(|&index| {
            let window = &windows[index];
            let reach = (REACH * shot.scale_at(window.rect)).round() as i32;
            let front = windows[..index].iter().map(|front| front.rect);
            Some(index) != under
                && window.covered
                && visible(window.rect, front)
                    .iter()
                    .any(|part| part.width() >= reach && part.height() >= reach)
        })?;
        let window = &windows[index];
        let scale = shot.scale_at(window.rect);
        let width = (WIDTH * scale).round().max(1.0);
        // Hugging the window from outside, and from inside at the screen's edges.
        let outer = window
            .rect
            .inflate(width as i32)
            .intersect(&shot.image.bounds())?;
        let half = width / 2.0;
        let line = D2D_RECT_F {
            left: outer.left as f32 + half,
            top: outer.top as f32 + half,
            right: outer.right as f32 - half,
            bottom: outer.bottom as f32 - half,
        };
        let radius = if window.radius > 0.0 {
            window.radius + half
        } else {
            0.0
        };
        let radius = radius.min((line.right - line.left).min(line.bottom - line.top) / 2.0);
        let center = (pointer.0 as f32 + 0.5, pointer.1 as f32 + 0.5);
        let palette = Palette::current();
        Some(Self {
            index,
            outline: Outline::new(line, radius, width, center),
            colors: if palette.contrast {
                [palette.accent; 4]
            } else {
                theme::signature(palette.light).map(|rgb| theme::color(rgb, 1.0))
            },
            shadow: if palette.contrast {
                0.0
            } else {
                width + 2.0 * (SHADOW * scale).round()
            },
            clock: Animation::idle(0.0),
            opacity: Animation::idle(1.0),
        })
    }

    /// Starts tracing: at once in full when animation effects are off.
    pub fn begin(&mut self) {
        self.clock = Animation::clock(DELAY + TRACE);
    }

    /// Fades it out, in step with the selection landing on its window. Whether it
    /// was showing.
    pub fn fade(&mut self) -> bool {
        let showing = self.opacity.target() > 0.0;
        self.opacity = self
            .opacity
            .retarget(0.0, motion::FAST, Easing::POINT_TO_POINT);
        showing
    }

    /// How it looks at `now`, with the overlay `presence` shown.
    pub fn at(&self, now: Instant, presence: f32) -> Hint<'_> {
        Hint {
            silhouette: self,
            traced: PEN.apply((self.clock.value(now) - DELAY) / TRACE),
            opacity: self.opacity.value(now) * presence,
        }
    }

    pub fn is_running(&self, now: Instant) -> bool {
        self.clock.is_running(now) || self.opacity.is_running(now)
    }

    /// Its outline and colors, made ready to draw on `target`.
    pub fn prepare(&self, factory: &ID2D1Factory, target: &ID2D1RenderTarget) -> Result<Drawn> {
        let geometry = self.outline.geometry(factory)?;
        let stops = [0.0, 0.35, 0.7, 1.0];
        let stops: [D2D1_GRADIENT_STOP; 4] = std::array::from_fn(|at| D2D1_GRADIENT_STOP {
            position: stops[at],
            color: self.colors[at],
        });
        let line = self.outline.line;
        unsafe {
            let length = geometry.ComputeLength(None, D2D1_DEFAULT_FLATTENING_TOLERANCE)?;
            let stops = target.CreateGradientStopCollection(
                &stops,
                D2D1_GAMMA_2_2,
                D2D1_EXTEND_MODE_CLAMP,
            )?;
            let colors = target.CreateLinearGradientBrush(
                &D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
                    startPoint: Vector2::new(line.left, line.top),
                    endPoint: Vector2::new(line.right, line.bottom),
                },
                None,
                &stops,
            )?;
            Ok(Drawn {
                geometry,
                length,
                colors,
            })
        }
    }
}

/// A silhouette at one instant.
pub struct Hint<'a> {
    pub silhouette: &'a Silhouette,
    /// How much of the outline is traced, from 0 to 1.
    traced: f32,
    pub opacity: f32,
}

/// A silhouette's outline and colors, ready to draw on a render target.
pub struct Drawn {
    geometry: ID2D1PathGeometry,
    /// How long the outline is, in pixels.
    length: f32,
    colors: ID2D1LinearGradientBrush,
}

impl Drawn {
    /// Draws `hint` over its shadow, which takes `shade`'s brush: from the origin,
    /// its two ends grow around the window, one each way, until they meet.
    pub unsafe fn draw(
        &self,
        factory: &ID2D1Factory,
        target: &ID2D1RenderTarget,
        shade: &ID2D1SolidColorBrush,
        hint: &Hint,
    ) {
        let each = self.length / 2.0 * hint.traced;
        if each < 0.5 || hint.opacity <= 0.0 {
            return;
        }
        // Dashes count in strokes' widths, so each stroke has its own.
        let style = |width: f32| {
            let dashes = [each, self.length - 2.0 * each, each, self.length];
            (hint.traced < 1.0)
                .then(|| stroke(factory, &dashes.map(|dash| dash / width)))
                .flatten()
        };
        let (width, shadow) = (hint.silhouette.outline.width, hint.silhouette.shadow);
        unsafe {
            if shadow > 0.0 {
                shade.SetColor(&theme::color(0x000000, SHADE * hint.opacity));
                target.DrawGeometry(&self.geometry, shade, shadow, style(shadow).as_ref());
            }
            self.colors.SetOpacity(hint.opacity);
            target.DrawGeometry(&self.geometry, &self.colors, width, style(width).as_ref());
        }
    }
}

/// A dashed stroke with flat ends, `dashes` long in strokes' widths, from the start.
fn stroke(factory: &ID2D1Factory, dashes: &[f32]) -> Option<ID2D1StrokeStyle> {
    let dashed = D2D1_STROKE_STYLE_PROPERTIES {
        startCap: D2D1_CAP_STYLE_FLAT,
        endCap: D2D1_CAP_STYLE_FLAT,
        dashCap: D2D1_CAP_STYLE_FLAT,
        lineJoin: D2D1_LINE_JOIN_ROUND,
        miterLimit: 10.0,
        dashStyle: D2D1_DASH_STYLE_CUSTOM,
        dashOffset: 0.0,
    };
    unsafe { factory.CreateStrokeStyle(&dashed, Some(dashes)) }.ok()
}

/// The center line of a rounded rectangle's edge, in image pixels, traced
/// clockwise from its origin and back.
struct Outline {
    line: D2D_RECT_F,
    radius: f32,
    width: f32,
    /// Where the trace starts and ends: on the straight `side` nearest the pointer,
    /// at the point nearest it.
    side: usize,
    origin: Vector2,
}

impl Outline {
    fn new(line: D2D_RECT_F, radius: f32, width: f32, pointer: (f32, f32)) -> Self {
        let mut outline = Self {
            line,
            radius,
            width,
            side: 0,
            origin: Vector2::zero(),
        };
        (outline.side, outline.origin) = outline.nearest(pointer);
        outline
    }

    /// Its straight sides, clockwise from the top, each from its start to its end.
    /// A corner joins each one's end to the next one's start.
    fn sides(&self) -> [(Vector2, Vector2); 4] {
        let D2D_RECT_F {
            left,
            top,
            right,
            bottom,
        } = self.line;
        let r = self.radius;
        let point = Vector2::new;
        [
            (point(left + r, top), point(right - r, top)),
            (point(right, top + r), point(right, bottom - r)),
            (point(right - r, bottom), point(left + r, bottom)),
            (point(left, bottom - r), point(left, top + r)),
        ]
    }

    /// The straight side nearest `(x, y)`, and its point nearest it. Where it can,
    /// that point lies on a pixel's edge, so the trace's two ends meet without a seam.
    fn nearest(&self, (x, y): (f32, f32)) -> (usize, Vector2) {
        let along = |at: f32, a: f32, b: f32| {
            let (low, high) = (a.min(b).ceil(), a.max(b).floor());
            if low <= high {
                at.round().clamp(low, high)
            } else {
                (a + b) / 2.0
            }
        };
        let distance = |point: Vector2| (point.X - x).powi(2) + (point.Y - y).powi(2);
        self.sides()
            .into_iter()
            .map(|(start, end)| {
                if start.Y == end.Y {
                    Vector2::new(along(x, start.X, end.X), start.Y)
                } else {
                    Vector2::new(start.X, along(y, start.Y, end.Y))
                }
            })
            .enumerate()
            .min_by(|(_, a), (_, b)| distance(*a).total_cmp(&distance(*b)))
            .unwrap_or_default()
    }

    /// The outline as one open figure, clockwise from its origin back to it.
    fn geometry(&self, factory: &ID2D1Factory) -> Result<ID2D1PathGeometry> {
        let sides = self.sides();
        let corner = D2D_SIZE_F {
            width: self.radius,
            height: self.radius,
        };
        unsafe {
            let geometry = factory.CreatePathGeometry()?;
            let sink = geometry.Open()?;
            sink.BeginFigure(self.origin, D2D1_FIGURE_BEGIN_HOLLOW);
            let mut at = self.origin;
            for step in 0..sides.len() {
                let side = (self.side + step) % sides.len();
                let (end, next) = (sides[side].1, sides[(side + 1) % sides.len()].0);
                if end != at {
                    sink.AddLine(end);
                }
                if self.radius > 0.0 {
                    sink.AddArc(&D2D1_ARC_SEGMENT {
                        point: next,
                        size: corner,
                        rotationAngle: 0.0,
                        sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                        arcSize: D2D1_ARC_SIZE_SMALL,
                    });
                }
                at = next;
            }
            if at != self.origin {
                sink.AddLine(self.origin);
            }
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            sink.Close()?;
            Ok(geometry)
        }
    }
}

/// The parts of `rect` that the windows `front` of it leave showing.
fn visible(rect: Rect, front: impl Iterator<Item = Rect>) -> Vec<Rect> {
    front.fold(vec![rect], |parts, front| {
        parts
            .into_iter()
            .flat_map(|part| match part.intersect(&front) {
                Some(hidden) => around(part, hidden).collect(),
                None => vec![part],
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 2-pixel outline's center line, around a window with 8-pixel corners.
    fn outline(pointer: (f32, f32)) -> Outline {
        let line = D2D_RECT_F {
            left: 101.0,
            top: 51.0,
            right: 501.0,
            bottom: 351.0,
        };
        Outline::new(line, 9.0, 2.0, pointer)
    }

    #[test]
    fn the_trace_starts_nearest_the_pointer() {
        // Inside, nearer the right side than any other.
        let near_right = outline((480.0, 200.3));
        assert_eq!(near_right.side, 1);
        assert_eq!(near_right.origin, Vector2::new(501.0, 200.0));
        // Outside, above the top side.
        let above = outline((260.7, 10.0));
        assert_eq!(above.side, 0);
        assert_eq!(above.origin, Vector2::new(261.0, 51.0));
    }

    #[test]
    fn the_trace_never_starts_on_a_corner() {
        // Beyond the bottom-left corner, it starts where the bottom side ends.
        let beyond = outline((60.0, 450.0));
        assert_eq!(beyond.side, 2);
        assert_eq!(beyond.origin, Vector2::new(110.0, 351.0));
    }

    #[test]
    fn the_trace_starts_on_a_pixels_edge() {
        // A 3-pixel outline, at 150 %: its corners end halfway through a pixel.
        let line = D2D_RECT_F {
            left: 100.5,
            top: 50.5,
            right: 500.5,
            bottom: 350.5,
        };
        let outline = Outline::new(line, 13.0, 3.0, (80.0, 20.0));
        assert_eq!(outline.side, 0);
        assert_eq!(outline.origin, Vector2::new(114.0, 50.5));
    }

    #[test]
    fn what_shows_of_a_covered_window() {
        // A window in front covers the bottom-right quarter.
        let rect = Rect::new(0, 0, 100, 100);
        let parts = visible(rect, [Rect::new(50, 50, 150, 150)].into_iter());
        let area: i32 = parts.iter().map(|part| part.width() * part.height()).sum();
        assert_eq!(area, 7500);
        let hidden = Rect::new(50, 50, 100, 100);
        assert!(parts.iter().all(|part| part.intersect(&hidden).is_none()));
        // Two windows hide it all.
        let front = [Rect::new(0, 0, 100, 60), Rect::new(0, 40, 100, 100)];
        assert!(visible(rect, front.into_iter()).is_empty());
    }
}
