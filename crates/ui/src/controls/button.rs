//! An accent button that can fade in and out of view.

use std::time::Instant;

use super::{Control, HEIGHT};
use crate::{
    motion::{self, Animation, Easing},
    paint::{Area, Label, Painter, Point},
    theme::faded,
};

const PADDING: f32 = 12.0;
const RADIUS: f32 = 4.0;

pub struct Button {
    label: Label,
    left: f32,
    top: f32,
    enabled: bool,
    /// From 0 when hidden to 1 when shown.
    presence: Animation,
    hovered: bool,
    pressed: bool,
}

impl Button {
    pub fn new(label: Label, shown: bool) -> Self {
        Self {
            label,
            left: 0.0,
            top: 0.0,
            enabled: true,
            presence: Animation::idle(if shown { 1.0 } else { 0.0 }),
            hovered: false,
            pressed: false,
        }
    }

    pub fn width(&self) -> f32 {
        self.label.width + 2.0 * PADDING
    }

    pub fn place(&mut self, left: f32, top: f32) {
        (self.left, self.top) = (left, top);
    }

    pub fn show(&mut self, shown: bool) {
        let (presence, easing) = if shown {
            (1.0, Easing::DECELERATE)
        } else {
            (0.0, Easing::ACCELERATE)
        };
        self.presence = self.presence.retarget(presence, motion::FAST, easing);
    }

    pub fn enable(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn area(&self) -> Area {
        Area::new(self.left, self.top, self.width(), HEIGHT)
    }
}

impl Control for Button {
    fn contains(&self, point: Point) -> bool {
        self.presence.target() > 0.0 && self.area().contains(point)
    }

    fn hover(&mut self, point: Option<Point>) -> bool {
        let hovered = point.is_some_and(|point| self.contains(point));
        std::mem::replace(&mut self.hovered, hovered) != hovered
    }

    fn press(&mut self, _point: Point) -> bool {
        self.pressed = true;
        false
    }

    fn release(&mut self, point: Point) -> bool {
        let activated = self.pressed && self.enabled && self.contains(point);
        self.pressed = false;
        self.hovered = self.contains(point);
        activated
    }

    fn cancel(&mut self) {
        self.pressed = false;
    }

    fn draw(&self, painter: &Painter, now: Instant) {
        let presence = self.presence.value(now);
        if presence <= 0.0 {
            return;
        }
        let palette = painter.palette;
        let (fill, ink) = if !self.enabled {
            (palette.accent_disabled, palette.on_accent_disabled)
        } else if self.pressed && self.hovered {
            (faded(palette.accent, 0.8), faded(palette.on_accent, 0.7))
        } else if self.hovered {
            (faded(palette.accent, 0.9), palette.on_accent)
        } else {
            (palette.accent, palette.on_accent)
        };
        let area = self.area();
        painter.fill(area, RADIUS, faded(fill, presence));
        let left = area.left + (area.width - self.label.width) / 2.0;
        painter.text(&self.label, left, area.middle(), faded(ink, presence));
    }

    fn is_animating(&self, now: Instant) -> bool {
        self.presence.is_running(now)
    }
}
