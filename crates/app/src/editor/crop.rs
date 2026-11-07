//! Cropping the capture to the part in view. Four corners in Kata's signature colors
//! mark what stays; cropping zooms out to show the whole capture, dims what goes,
//! closes in on what stays and glides it into place, where the corners open out and
//! fade. Undoing plays it back.

use std::time::Instant;

use kata_editor::{Edits, Rect};
use kata_ui::{
    Graphics,
    figure::Figure,
    motion::{Animation, Easing},
    paint::{Area, Painter},
    theme::{self, color},
};
use windows::core::Result;

use super::canvas::View;
use crate::card;

/// The corners' arms and thickness, in DIPs, and how far out they open as they go.
const ARM: f32 = 16.0;
const THICK: f32 = 3.0;
const SPREAD: f32 = 6.0;
/// What sets the corners apart from any capture, around them.
const HALO: f32 = 1.0;
const HALO_OPACITY: f32 = 0.3;
/// How dark what's cut away gets.
const DIM: f32 = 0.5;
/// The cut, in seconds: zooming out, a beat, closing in on what stays, and gliding it
/// into place, with the corners opening out once it's most of the way.
const OUT: f32 = 0.3;
const HOLD: f32 = 0.1;
const CLOSE: f32 = 0.26;
const SETTLE: f32 = 0.44;
const FLASH: f32 = 0.4;
const CLOSE_AT: f32 = OUT + HOLD;
const SETTLE_AT: f32 = CLOSE_AT + CLOSE;
const FLASH_AT: f32 = SETTLE_AT + 0.6 * SETTLE;
/// Undone: zooming out, opening up to what was cut, a beat, and zooming back in.
const BACK: f32 = 0.28;
const OPEN: f32 = 0.26;
const BACK_HOLD: f32 = 0.08;
const IN: f32 = 0.34;
const IN_AT: f32 = BACK + OPEN + BACK_HOLD;

/// The capture cut from `large` to `small`, or back as it's undone.
pub struct Cropping {
    large: Rect,
    small: Rect,
    undoing: bool,
    /// Where the view starts, where it shows `large` whole, and where it ends.
    views: [View; 3],
    /// How much the corners showed as it started.
    marked: f32,
    /// The shapes on show before the cut closes, or before it opens as it's undone:
    /// so the steps it leaves out take their numbers with them as it does.
    shapes: Edits,
    clock: Animation,
}

impl Cropping {
    /// Cuts `large` to `small`, or undoes that, through `views`. The corners start
    /// showing `marked`, and `shapes`, those from before, show until the cut closes or
    /// opens.
    pub fn new(
        (large, small): (Rect, Rect),
        undoing: bool,
        views: [View; 3],
        marked: f32,
        shapes: Edits,
    ) -> Self {
        let length = if undoing {
            IN_AT + IN
        } else {
            FLASH_AT + FLASH
        };
        Self {
            large,
            small,
            undoing,
            views,
            marked,
            shapes,
            clock: Animation::clock(length),
        }
    }

    pub fn is_running(&self, now: Instant) -> bool {
        self.clock.is_running(now)
    }

    pub fn large(&self) -> Rect {
        self.large
    }

    pub fn small(&self) -> Rect {
        self.small
    }

    /// The shapes from before, until the cut closes, or opens as it's undone.
    pub fn shapes(&self, now: Instant) -> Option<&Edits> {
        let t = self.clock.value(now);
        let turn = if self.undoing { BACK } else { SETTLE_AT };
        (t < turn).then_some(&self.shapes)
    }

    /// Where the capture shows.
    pub fn view(&self, now: Instant) -> View {
        let t = self.clock.value(now);
        let [start, wide, end] = self.views;
        if self.undoing {
            if t < IN_AT {
                start.toward(wide, along(t, 0.0, BACK, Easing::DECELERATE))
            } else {
                wide.toward(end, along(t, IN_AT, IN, Easing::DECELERATE))
            }
        } else if t < SETTLE_AT {
            start.toward(wide, along(t, 0.0, OUT, Easing::DECELERATE))
        } else {
            wide.toward(end, along(t, SETTLE_AT, SETTLE, Easing::SPRING))
        }
    }

    /// The part of the capture that shows, in its pixels, as its edges close in on
    /// what stays, or open up to what was cut.
    pub fn shown(&self, now: Instant) -> Area {
        let t = self.clock.value(now);
        let (large, small) = (card::part(self.large), card::part(self.small));
        if self.undoing {
            between(small, large, along(t, BACK, OPEN, Easing::SWEEP))
        } else {
            between(large, small, along(t, CLOSE_AT, CLOSE, Easing::SWEEP))
        }
    }

    /// How dark what's cut away is.
    pub fn dim(&self, now: Instant) -> f32 {
        let t = self.clock.value(now);
        DIM * if self.undoing {
            1.0 - along(t, IN_AT, IN, Easing::DECELERATE)
        } else {
            along(t, 0.0, OUT, Easing::DECELERATE)
        }
    }

    /// How much the corners around what stays show.
    pub fn marks(&self, now: Instant) -> f32 {
        let t = self.clock.value(now);
        if self.undoing {
            if t < IN_AT {
                along(t, 0.0, BACK, Easing::DECELERATE)
            } else {
                1.0 - along(t, IN_AT, IN, Easing::DECELERATE)
            }
        } else if t < FLASH_AT {
            let out = along(t, 0.0, OUT, Easing::DECELERATE);
            self.marked + (1.0 - self.marked) * out
        } else {
            1.0 - along(t, FLASH_AT, FLASH, Easing::DECELERATE)
        }
    }
}

/// How far along `easing` a motion `length` seconds long from `start` is at `t`.
fn along(t: f32, start: f32, length: f32, easing: Easing) -> f32 {
    easing.apply((t - start) / length)
}

/// The area `t` of the way from `a` to `b`.
fn between(a: Area, b: Area, t: f32) -> Area {
    let mix = |from: f32, to: f32| from + (to - from) * t;
    Area::new(
        mix(a.left, b.left),
        mix(a.top, b.top),
        mix(a.width, b.width),
        mix(a.height, b.height),
    )
}

/// The corners around what cropping keeps, each in a signature color, as the rim
/// runs: blue at the top left to orange at the bottom right.
pub struct Corners {
    line: Figure,
    halo: Figure,
}

impl Corners {
    /// One corner, built once, with its outer edge at the origin and its arms
    /// running right and down.
    pub fn new(graphics: &Graphics) -> Result<Self> {
        let middle = THICK / 2.0;
        let path = Figure::new(graphics, |pen| {
            pen.start((middle, ARM));
            pen.line((middle, middle));
            pen.line((ARM, middle));
        })?;
        Ok(Self {
            line: path.traced(THICK)?,
            halo: path.traced(THICK + 2.0 * HALO)?,
        })
    }

    /// Marks `area` with a corner just outside each of its own, `shown` of the way
    /// in: gathering in as they show, opening out as they go.
    pub fn draw(&self, painter: &Painter, area: Area, shown: f32) {
        let palette = painter.palette;
        let [blue, violet, pink, orange] = theme::signature(palette.light);
        let out = THICK + SPREAD * (1.0 - shown);
        let area = painter.align(area);
        let (left, top) = (painter.snap(area.left - out), painter.snap(area.top - out));
        let (right, bottom) = (
            painter.snap(area.right() + out),
            painter.snap(area.bottom() + out),
        );
        let corners = [
            ((left, top), blue),
            ((right, top), violet),
            ((right, bottom), orange),
            ((left, bottom), pink),
        ];
        painter.layer(shown, 0.0, || {
            for (turns, (corner, rgb)) in corners.into_iter().enumerate() {
                painter.turned(90.0 * turns as f32, corner, || {
                    // Contrast themes draw them all in the highlight, edged by nothing.
                    if palette.contrast {
                        painter.figure(&self.line, corner, palette.accent);
                    } else {
                        painter.figure(&self.halo, corner, color(0x000000, HALO_OPACITY));
                        painter.figure(&self.line, corner, color(rgb, 1.0));
                    }
                });
            }
        });
    }
}
