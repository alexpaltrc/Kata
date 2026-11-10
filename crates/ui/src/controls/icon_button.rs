//! A subtle button that shows a glyph, such as a title bar's back button, and maybe
//! what it does after it.

use std::time::Instant;

use super::{Action, Control};
use crate::{
    access::{Element, Role},
    paint::{Area, Label, Painter, Point},
};

const RADIUS: f32 = 4.0;
/// Inside a button that says what it does: the room at each end, and between the
/// glyph and its words.
const PADDING: f32 = 8.0;
const GAP: f32 = 6.0;

/// How the button sits where it's placed.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Fit {
    /// On its own, with rounded corners.
    Alone,
    /// Round at both ends, inside a capsule.
    Round,
}

pub struct IconButton {
    glyph: Label,
    /// What it does, after the glyph, when it says so.
    label: Option<Label>,
    /// What assistive technologies call it, as the glyph says nothing to them.
    name: String,
    area: Area,
    fit: Fit,
    hovered: bool,
    pressed: bool,
}

impl IconButton {
    pub const WIDTH: f32 = 40.0;
    /// As tall as a title bar's own buttons are in Windows 11.
    pub const HEIGHT: f32 = 36.0;

    pub fn new(glyph: Label, name: &str) -> Self {
        Self {
            glyph,
            label: None,
            name: name.to_owned(),
            area: Area::default(),
            fit: Fit::Alone,
            hovered: false,
            pressed: false,
        }
    }

    /// Says what it does, in `label`, after the glyph.
    pub fn labeled(glyph: Label, label: Label) -> Self {
        Self {
            name: label.text.clone(),
            label: Some(label),
            ..Self::new(glyph, "")
        }
    }

    pub fn width(&self) -> f32 {
        match &self.label {
            Some(label) => PADDING + self.glyph.width + GAP + label.width + PADDING,
            None => Self::WIDTH,
        }
    }

    pub fn place(&mut self, left: f32, top: f32) {
        self.area = Area::new(left, top, self.width(), Self::HEIGHT);
        self.fit = Fit::Alone;
    }

    /// Places it `height` DIPs tall and round at both ends, inside a capsule.
    pub fn round(&mut self, left: f32, top: f32, height: f32) {
        self.area = Area::new(left, top, self.width(), height);
        self.fit = Fit::Round;
    }

    pub fn area(&self) -> Area {
        self.area
    }

    pub fn is_hovered(&self) -> bool {
        self.hovered
    }

    fn radius(&self) -> f32 {
        match self.fit {
            Fit::Alone => RADIUS,
            Fit::Round => self.area.height / 2.0,
        }
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
        (self.area, self.radius())
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
        painter.fill(area, self.radius(), fill);
        match &self.label {
            Some(label) => {
                let left = area.left + PADDING;
                painter.text(&self.glyph, left, area.middle(), ink);
                let left = left + self.glyph.width + GAP;
                painter.text(label, left, area.middle(), ink);
            }
            None => {
                let left = area.left + (area.width - self.glyph.width) / 2.0;
                painter.text(&self.glyph, left, area.middle(), ink);
            }
        }
    }

    fn is_animating(&self, _now: Instant) -> bool {
        false
    }
}
