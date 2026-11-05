//! A push button, in the accent color for the main action or standard otherwise.

use std::time::Instant;

use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

use super::{Action, Control, HEIGHT};
use crate::{
    access::{Element, Role},
    paint::{Area, Label, Painter, Point},
    theme::faded,
};

const PADDING: f32 = 12.0;
const RADIUS: f32 = 4.0;

pub struct Button {
    label: Label,
    /// What assistive technologies call it: its label, unless that needs context.
    name: String,
    accent: bool,
    /// The least it measures across, whatever its label.
    least: f32,
    left: f32,
    top: f32,
    hovered: bool,
    pressed: bool,
}

impl Button {
    /// An accent button.
    pub fn new(label: Label) -> Self {
        Self {
            name: label.text.clone(),
            label,
            accent: true,
            least: 0.0,
            left: 0.0,
            top: 0.0,
            hovered: false,
            pressed: false,
        }
    }

    pub fn standard(label: Label) -> Self {
        Self {
            accent: false,
            ..Self::new(label)
        }
    }

    /// Has assistive technologies call it `name`, for a label that says too little
    /// on its own.
    pub fn named(self, name: &str) -> Self {
        Self {
            name: name.to_owned(),
            ..self
        }
    }

    pub fn width(&self) -> f32 {
        (self.label.width + 2.0 * PADDING).max(self.least)
    }

    /// Makes it at least `width` DIPs across, as wide as the buttons beside it.
    pub fn widen(&mut self, width: f32) {
        self.least = width;
    }

    pub fn place(&mut self, left: f32, top: f32) {
        (self.left, self.top) = (left, top);
    }

    pub fn area(&self) -> Area {
        Area::new(self.left, self.top, self.width(), HEIGHT)
    }

    fn draw_accent(&self, painter: &Painter) {
        let palette = painter.palette;
        let (fill, ink) = if self.pressed && self.hovered {
            (faded(palette.accent, 0.8), faded(palette.on_accent, 0.7))
        } else if self.hovered {
            (faded(palette.accent, 0.9), palette.on_accent)
        } else {
            (palette.accent, palette.on_accent)
        };
        painter.fill(self.area(), RADIUS, fill);
        self.draw_label(painter, ink);
    }

    fn draw_standard(&self, painter: &Painter) {
        let palette = painter.palette;
        let (shade, ink) = if self.pressed && self.hovered {
            (palette.subtle_pressed, palette.text_secondary)
        } else if self.hovered {
            (palette.subtle_hover, palette.text)
        } else {
            (Default::default(), palette.text)
        };
        let area = painter.align(self.area());
        painter.fill(area, RADIUS, palette.button);
        painter.fill(area, RADIUS, shade);
        painter.stroke(area, RADIUS, painter.border(), palette.control_stroke);
        self.draw_label(painter, ink);
    }

    fn draw_label(&self, painter: &Painter, color: D2D1_COLOR_F) {
        let area = self.area();
        let left = area.left + (area.width - self.label.width) / 2.0;
        painter.text(&self.label, left, area.middle(), color);
    }
}

impl Control for Button {
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

    fn is_focusable(&self) -> bool {
        true
    }

    fn focus_area(&self) -> (Area, f32) {
        (self.area(), RADIUS)
    }

    fn act(&mut self, action: Action) -> bool {
        action == Action::Activate
    }

    fn describe(&self, control: usize, elements: &mut Vec<Element>) {
        elements.push(Element {
            control,
            name: self.name.clone(),
            role: Role::Button,
            area: self.area(),
            help: "",
        });
    }

    fn draw(&self, painter: &Painter, _now: Instant) {
        if self.accent {
            self.draw_accent(painter);
        } else {
            self.draw_standard(painter);
        }
    }

    fn is_animating(&self, _now: Instant) -> bool {
        false
    }
}
