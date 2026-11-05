//! A subtle button that shows only a glyph, such as a title bar's back button.

use std::time::Instant;

use super::{Action, Control};
use crate::{
    access::{Element, Role},
    paint::{Area, Label, Painter, Point},
};

const RADIUS: f32 = 4.0;

pub struct IconButton {
    glyph: Label,
    /// What assistive technologies call it, as the glyph says nothing to them.
    name: String,
    area: Area,
    /// Whether it fills its part of the title bar edge to edge, square, as the
    /// caption buttons do.
    bleeds: bool,
    hovered: bool,
    pressed: bool,
}

impl IconButton {
    pub const WIDTH: f32 = 40.0;
    /// Short enough to fit beside the caption buttons.
    pub const HEIGHT: f32 = 28.0;

    pub fn new(glyph: Label, name: &str) -> Self {
        Self {
            glyph,
            name: name.to_owned(),
            area: Area::default(),
            bleeds: false,
            hovered: false,
            pressed: false,
        }
    }

    pub fn place(&mut self, left: f32, top: f32) {
        self.area = Area::new(left, top, Self::WIDTH, Self::HEIGHT);
        self.bleeds = false;
    }

    /// Fills `area` of the title bar edge to edge, square, as the caption buttons
    /// beside it do.
    pub fn bleed(&mut self, area: Area) {
        self.area = area;
        self.bleeds = true;
    }
}

impl Control for IconButton {
    fn contains(&self, point: Point) -> bool {
        self.area.contains(point)
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

    fn focus_area(&self) -> (Area, f32) {
        // Inside an edge-to-edge button, as the window's edge would cut a ring around it.
        let inset = if self.bleeds { -4.0 } else { 0.0 };
        (self.area.inflate(inset), RADIUS)
    }

    fn act(&mut self, action: Action) -> bool {
        action == Action::Activate
    }

    fn describe(&self, control: usize, elements: &mut Vec<Element>) {
        elements.push(Element {
            control,
            name: self.name.clone(),
            role: Role::Button,
            area: self.area,
            help: "",
        });
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
        let area = painter.align(self.area);
        let radius = if self.bleeds { 0.0 } else { RADIUS };
        painter.fill(area, radius, fill);
        let left = area.left + (area.width - self.glyph.width) / 2.0;
        painter.text(&self.glyph, left, area.middle(), ink);
    }

    fn is_animating(&self, _now: Instant) -> bool {
        false
    }
}
