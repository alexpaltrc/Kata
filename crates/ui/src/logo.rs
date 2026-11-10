//! Kata's logo, as on its Start menu tile: the glyph's four corners around a lit
//! area, in Kata's orange, lit from the top left. It can draw itself, a corner at
//! a time, each traced in one of Kata's own colors and warming to orange through
//! the colors that follow its own.

use std::f32::consts::FRAC_PI_2;

use windows::{
    Win32::Graphics::Direct2D::{
        Common::{
            D2D_SIZE_F, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_OPEN,
            D2D1_GRADIENT_STOP,
        },
        D2D1_ARC_SEGMENT, D2D1_ARC_SIZE_SMALL, D2D1_CAP_STYLE_ROUND, D2D1_LINE_JOIN_ROUND,
        D2D1_STROKE_STYLE_PROPERTIES, D2D1_SWEEP_DIRECTION_CLOCKWISE, ID2D1Brush, ID2D1Factory,
        ID2D1PathGeometry, ID2D1StrokeStyle,
    },
    core::Result,
};
use windows_numerics::Vector2;

use crate::{
    motion::Easing,
    paint::{Area, Painter, diagonal, rounded},
    theme::{self, color, mix},
};

/// The corners' stroke on the logo's 16-unit grid: how thick it is, how far from
/// the edge it runs, how far its arms reach and how round it turns.
const STROKE: f32 = 2.0;
const NEAR: f32 = 2.0;
const REACH: f32 = 5.5;
const RADIUS: f32 = 2.0;
/// The lit area, inset from the edge, and its corners.
const INSET: f32 = 5.0;
const LIT_RADIUS: f32 = 1.5;

/// Kata's orange, bright at the top left and deep at the bottom right, and the
/// light inside it.
const BRIGHT: u32 = 0xFFA35E;
const DEEP: u32 = 0xEC5A22;
const GLOW: u32 = 0xFFD6AE;
const LIT: u32 = 0xFF9E5C;

/// In seconds: how far apart the corners start, how long each takes to trace and
/// then to warm to orange, and when the area lights up, how long it takes to show
/// and how long to settle.
const STAGGER: f32 = 0.15;
const TRACE: f32 = 0.4;
const WARM: f32 = 0.45;
const LIGHT_UP: f32 = 0.8;
const KINDLE: f32 = 0.1;
const LIGHTING: f32 = 0.4;
/// How long the logo takes to draw itself, in seconds.
pub const DRAWING: f32 = 3.0 * STAGGER + TRACE + WARM;

/// A pen's stroke: quick to leave, gentle to land.
const PEN: Easing = Easing::new(0.4, 0.0, 0.2, 1.0);
/// How big the light is as it starts growing.
const SEED: f32 = 0.25;

/// The lit area of a logo filling `area`, and the radius of its corners.
fn lit(area: Area) -> (Area, f32) {
    let unit = area.width / 16.0;
    (area.inflate(-INSET * unit), LIT_RADIUS * unit)
}

impl Painter<'_> {
    /// Kata's logo, filling the square `area`, as it is `time` seconds into drawing
    /// itself: from `DRAWING` on, it's whole. Its corners start clockwise from the
    /// top left, each in one of Kata's own colors. A contrast theme's colors draw it
    /// whole at once.
    pub fn logo(&self, area: Area, time: f32) {
        let palette = self.palette;
        let time = if palette.contrast { DRAWING } else { time };
        let Ok(factory) = (unsafe { self.context.GetFactory() }) else {
            return;
        };
        let Ok(round) = round_stroke(&factory) else {
            return;
        };
        let unit = area.width / 16.0;
        let (from, to) = diagonal(area);
        let orange = if palette.contrast {
            None
        } else {
            self.gradient(&stops(BRIGHT, DEEP), from, to).ok()
        };
        let signature = theme::signature(palette.light);
        for (index, corner) in corners(area.inflate(-NEAR * unit)).into_iter().enumerate() {
            let start = index as f32 * STAGGER;
            let traced = PEN.apply(progress(time, start, TRACE));
            if traced <= 0.0 {
                continue;
            }
            let Ok(path) = corner.path(&factory, (REACH - NEAR) * unit, RADIUS * unit, traced)
            else {
                continue;
            };
            let warm = PEN.apply(progress(time, start + TRACE, WARM));
            let stroke = |brush: &ID2D1Brush, opacity: f32| unsafe {
                brush.SetOpacity(opacity);
                self.context
                    .DrawGeometry(&path, brush, STROKE * unit, &round);
            };
            match &orange {
                None => stroke(self.solid(palette.text), 1.0),
                Some(orange) => {
                    // From color to color, so that no step on the way turns gray.
                    let colors = &signature[index..];
                    let last = colors.len() - 1;
                    let run = warm * colors.len() as f32;
                    let step = (run as usize).min(last);
                    let along = run - step as f32;
                    if step < last {
                        let (from, to) = (color(colors[step], 1.0), color(colors[step + 1], 1.0));
                        stroke(self.solid(mix(from, to, along)), 1.0);
                    } else {
                        if along < 1.0 {
                            stroke(self.solid(color(colors[last], 1.0)), 1.0);
                        }
                        if along > 0.0 {
                            stroke(orange, along);
                        }
                    }
                }
            }
        }
        let lighting = progress(time, LIGHT_UP, LIGHTING);
        if lighting <= 0.0 {
            return;
        }
        let (lit, radius) = lit(area);
        let gradient = orange.and_then(|_| self.gradient(&stops(GLOW, LIT), from, to).ok());
        let light: &ID2D1Brush = match &gradient {
            Some(gradient) => gradient,
            None => self.solid(palette.accent),
        };
        // Quick to show, so that it never looks dim on a dark theme.
        let opacity = Easing::DECELERATE.apply(progress(time, LIGHT_UP, KINDLE));
        // It grows a little past its size and settles.
        let scale = SEED + (1.0 - SEED) * Easing::SPRING.apply(lighting);
        let center = (lit.left + lit.width / 2.0, lit.middle());
        self.grown(opacity, scale, center, || unsafe {
            self.context
                .FillRoundedRectangle(&rounded(lit, radius), light);
        });
    }

    /// The logo's four corners alone, in its orange, around any `area`, as around a
    /// capture: their arms reach `arm` and turn round at `radius`, `stroke` wide.
    pub fn frame(&self, area: Area, arm: f32, radius: f32, stroke: f32) {
        let palette = self.palette;
        let Ok(factory) = (unsafe { self.context.GetFactory() }) else {
            return;
        };
        let Ok(round) = round_stroke(&factory) else {
            return;
        };
        let (from, to) = diagonal(area);
        let orange = match palette.contrast {
            true => None,
            false => self.gradient(&stops(BRIGHT, DEEP), from, to).ok(),
        };
        let brush: &ID2D1Brush = match &orange {
            Some(orange) => orange,
            None => self.solid(palette.text),
        };
        for corner in corners(area) {
            if let Ok(path) = corner.path(&factory, arm, radius, 1.0) {
                unsafe {
                    brush.SetOpacity(1.0);
                    self.context.DrawGeometry(&path, brush, stroke, &round);
                }
            }
        }
    }

    /// The painter's own brush, in `color`.
    fn solid(&self, color: D2D1_COLOR_F) -> &ID2D1Brush {
        unsafe { self.brush.SetColor(&color) };
        self.brush
    }
}

/// One of the glyph's corners, traced from the end of one arm, round its turn, to
/// the end of the other.
struct Corner {
    /// Where its arms would meet, if it didn't turn round.
    vertex: Vector2,
    /// Which way it heads along its first arm and then along its second.
    along: Vector2,
    then: Vector2,
}

impl Corner {
    /// The part of the corner traced so far, `traced` of the way along it: its arms
    /// reach `arm` from where they'd meet, and it turns round at `radius`.
    fn path(
        &self,
        factory: &ID2D1Factory,
        arm: f32,
        radius: f32,
        traced: f32,
    ) -> Result<ID2D1PathGeometry> {
        let (straight, turn) = (arm - radius, FRAC_PI_2 * radius);
        let mut left = traced.clamp(0.0, 1.0) * (2.0 * straight + turn);
        let (vertex, along, then) = (self.vertex, self.along, self.then);
        let start = vertex - along * arm;
        unsafe {
            let path = factory.CreatePathGeometry()?;
            let sink = path.Open()?;
            sink.BeginFigure(start, D2D1_FIGURE_BEGIN_HOLLOW);
            sink.AddLine(start + along * left.min(straight));
            left -= straight;
            if left > 0.0 {
                // Round the turn's center, from facing back along the second arm.
                let angle = (left / radius).min(FRAC_PI_2);
                let center = vertex - along * radius + then * radius;
                let to = center - then * (radius * angle.cos()) + along * (radius * angle.sin());
                sink.AddArc(&D2D1_ARC_SEGMENT {
                    point: to,
                    size: D2D_SIZE_F {
                        width: radius,
                        height: radius,
                    },
                    rotationAngle: 0.0,
                    sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                    arcSize: D2D1_ARC_SIZE_SMALL,
                });
                left -= turn;
                if left > 0.0 {
                    sink.AddLine(vertex + then * (radius + left.min(straight)));
                }
            }
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            sink.Close()?;
            Ok(path)
        }
    }
}

/// The four corners of `area`, clockwise from the top left, each traced clockwise.
fn corners(area: Area) -> [Corner; 4] {
    let (left, top) = (area.left, area.top);
    let (right, bottom) = (area.right(), area.bottom());
    let (up, down) = (Vector2::new(0.0, -1.0), Vector2::new(0.0, 1.0));
    let (back, ahead) = (Vector2::new(-1.0, 0.0), Vector2::new(1.0, 0.0));
    let corner = |x, y, along, then| Corner {
        vertex: Vector2::new(x, y),
        along,
        then,
    };
    [
        corner(left, top, up, ahead),
        corner(right, top, ahead, down),
        corner(right, bottom, down, back),
        corner(left, bottom, back, up),
    ]
}

/// From 0 to 1 over the `duration` seconds that start at `start`.
fn progress(time: f32, start: f32, duration: f32) -> f32 {
    ((time - start) / duration).clamp(0.0, 1.0)
}

/// From `from` to `to`, solid.
fn stops(from: u32, to: u32) -> [D2D1_GRADIENT_STOP; 2] {
    [
        D2D1_GRADIENT_STOP {
            position: 0.0,
            color: color(from, 1.0),
        },
        D2D1_GRADIENT_STOP {
            position: 1.0,
            color: color(to, 1.0),
        },
    ]
}

/// Round ends, as a pen leaves them.
fn round_stroke(factory: &ID2D1Factory) -> Result<ID2D1StrokeStyle> {
    let round = D2D1_STROKE_STYLE_PROPERTIES {
        startCap: D2D1_CAP_STYLE_ROUND,
        endCap: D2D1_CAP_STYLE_ROUND,
        dashCap: D2D1_CAP_STYLE_ROUND,
        lineJoin: D2D1_LINE_JOIN_ROUND,
        miterLimit: 10.0,
        ..Default::default()
    };
    unsafe { factory.CreateStrokeStyle(&round, None) }
}
