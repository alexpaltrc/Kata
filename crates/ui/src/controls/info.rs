//! An info glyph beside a setting. Once the pointer rests on it, a tooltip explains
//! the setting; assistive technologies hear the same from the setting itself.

use std::time::Instant;

use super::{Action, Control};
use crate::{
    access::Element,
    motion::{self, Animation, Easing},
    paint::{Area, Label, Painter, Point},
};

/// Between the glyph's square and the tooltip.
const GAP: f32 = 4.0;
/// Around the tooltip's text: on each side, above and below, as in Fluent.
const PADDING: (f32, f32, f32) = (9.0, 6.0, 8.0);
const RADIUS: f32 = 8.0;

pub struct Info {
    glyph: Label,
    /// What the tooltip says, in lines up to `TEXT_WIDTH`.
    text: Label,
    left: f32,
    top: f32,
    /// What the tooltip stays within.
    bounds: Area,
    hovered: bool,
    /// From 0 while the tooltip is hidden to 1 once it shows.
    shown: Animation,
}

impl Info {
    /// The square around the glyph that answers to the pointer.
    pub const SIZE: f32 = 24.0;
    /// How wide the tooltip's lines can be: it stays within Fluent's 320 DIPs, and
    /// within the narrowest window.
    pub const TEXT_WIDTH: f32 = 270.0;

    pub fn new(glyph: Label, text: Label) -> Self {
        Self {
            glyph,
            text,
            left: 0.0,
            top: 0.0,
            bounds: Area::default(),
            hovered: false,
            shown: Animation::idle(0.0),
        }
    }

    /// Puts the glyph's square at `left`, centered on `middle`, with the tooltip
    /// kept within `bounds`.
    pub fn place(&mut self, left: f32, middle: f32, bounds: Area) {
        (self.left, self.top) = (left, middle - Self::SIZE / 2.0);
        self.bounds = bounds;
    }

    /// Whether the pointer is on the glyph while the tooltip is hidden, so that
    /// resting there would show it.
    pub fn awaits_rest(&self) -> bool {
        self.hovered && self.shown.target() == 0.0
    }

    /// The pointer rested at `point`. Returns whether that shows the tooltip.
    pub fn rest(&mut self, point: Point) -> bool {
        let on_glyph = self.contains(point);
        if on_glyph {
            self.show(true);
        }
        on_glyph
    }

    /// Draws the tooltip, while it shows. It goes over everything else.
    pub fn draw_tip(&self, painter: &Painter, now: Instant) {
        let palette = painter.palette;
        painter.layer(self.shown.value(now), 0.0, || {
            let tip = painter.align(self.tip());
            painter.shadow(tip, RADIUS, palette.shadow);
            painter.fill(tip, RADIUS, palette.flyout);
            painter.stroke(tip, RADIUS, painter.border(), palette.flyout_stroke);
            let (side, above, _) = PADDING;
            let middle = tip.top + above + self.text.height / 2.0;
            painter.text(&self.text, tip.left + side, middle, palette.text);
        });
    }

    fn show(&mut self, shown: bool) {
        let target = if shown { 1.0 } else { 0.0 };
        self.shown = self
            .shown
            .retarget(target, motion::FAST, Easing::DECELERATE);
    }

    fn area(&self) -> Area {
        Area::new(self.left, self.top, Self::SIZE, Self::SIZE)
    }

    /// Centered above the glyph, or below it when there's no room above.
    fn tip(&self) -> Area {
        let (side, above, below) = PADDING;
        let width = self.text.width + 2.0 * side;
        let height = above + self.text.height + below;
        let glyph = self.area();
        let centered = glyph.left + (glyph.width - width) / 2.0;
        let left = centered
            .min(self.bounds.right() - width)
            .max(self.bounds.left);
        let over = glyph.top - GAP - height;
        let top = if over >= self.bounds.top {
            over
        } else {
            glyph.bottom() + GAP
        };
        Area::new(left, top, width, height)
    }
}

impl Control for Info {
    fn contains(&self, point: Point) -> bool {
        self.area().contains(point)
    }

    fn hover(&mut self, point: Option<Point>) -> bool {
        let hovered = point.is_some_and(|point| self.contains(point));
        if hovered == self.hovered {
            return false;
        }
        self.hovered = hovered;
        if !hovered {
            self.show(false);
        }
        true
    }

    /// A click puts the tooltip away, as in Fluent.
    fn press(&mut self, _point: Point) -> bool {
        self.show(false);
        false
    }

    fn release(&mut self, _point: Point) -> bool {
        false
    }

    fn cancel(&mut self) {}

    fn is_focusable(&self) -> bool {
        false
    }

    fn focus_area(&self) -> (Area, f32) {
        (self.area(), Self::SIZE / 2.0)
    }

    fn act(&mut self, _action: Action) -> bool {
        false
    }

    /// Nothing: the setting it explains says it all.
    fn describe(&self, _control: usize, _elements: &mut Vec<Element>) {}

    fn draw(&self, painter: &Painter, _now: Instant) {
        let palette = painter.palette;
        let ink = if self.hovered {
            palette.text
        } else {
            palette.text_secondary
        };
        let area = self.area();
        let left = area.left + (area.width - self.glyph.width) / 2.0;
        painter.text(&self.glyph, left, area.middle(), ink);
    }

    fn is_animating(&self, now: Instant) -> bool {
        self.shown.is_running(now)
    }
}
