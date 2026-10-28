//! A subtle button that shows only a glyph, such as a title bar's back button.

use std::time::Instant;

use super::{Control, HEIGHT};
use crate::paint::{Area, Label, Painter, Point};

const RADIUS: f32 = 4.0;

pub struct IconButton {
    glyph: Label,
    left: f32,
    top: f32,
    hovered: bool,
    pressed: bool,
}

impl IconButton {
    pub const WIDTH: f32 = 40.0;

    pub fn new(glyph: Label) -> Self {
        Self {
            glyph,
            left: 0.0,
            top: 0.0,
            hovered: false,
            pressed: false,
        }
    }

    pub fn place(&mut self, left: f32, top: f32) {
        (self.left, self.top) = (left, top);
    }

    fn area(&self) -> Area {
        Area::new(self.left, self.top, Self::WIDTH, HEIGHT)
    }
}

impl Control for IconButton {
    fn contains(&self, point: Point) -> bool {
        self.area().contains(point)
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
        let activated = self.pressed && self.contains(point);
        self.pressed = false;
        self.hovered = self.contains(point);
        activated
    }

    fn cancel(&mut self) {
        self.pressed = false;
    }

    fn draw(&self, painter: &Painter, _now: Instant) {
        let palette = painter.palette;
        let (fill, ink) = if self.pressed && self.hovered {
            (palette.subtle_pressed, palette.text_secondary)
        } else if self.hovered {
            (palette.subtle_hover, palette.text)
        } else {
            (Default::default(), palette.text)
        };
        let area = painter.align(self.area());
        painter.fill(area, RADIUS, fill);
        let left = area.left + (area.width - self.glyph.width) / 2.0;
        painter.text(&self.glyph, left, area.middle(), ink);
    }

    fn is_animating(&self, _now: Instant) -> bool {
        false
    }
}
