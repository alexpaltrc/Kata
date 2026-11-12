//! A quiet line that says there's a code to enter. A click on it turns it into a
//! field, where the code shows as it's typed; a code that isn't one shakes it.

use std::time::Instant;

use super::{Action, Control, HEIGHT};
use crate::{
    access::{Element, Role},
    motion::{self, Animation, Easing},
    paint::{Area, Label, Painter, Point},
};

/// Around the line, at rest.
const PADDING: f32 = 12.0;
/// The field's width, and the room between what's typed and its caret.
const FIELD: f32 = 200.0;
const CARET_GAP: f32 = 2.0;
const CARET: f32 = 16.0;
const RADIUS: f32 = 4.0;
/// How far a refused code shakes the field, and how many times it swings.
const SHAKE: f32 = 6.0;
const SWINGS: f32 = 3.0;

pub struct Code {
    /// What it says at rest.
    ask: Label,
    /// What the empty field says.
    hint: Label,
    /// Whether it's a field now; and what's typed in it, if anything.
    entering: bool,
    typed: Option<Label>,
    center: f32,
    top: f32,
    hovered: bool,
    pressed: bool,
    shake: Animation,
}

impl Code {
    pub fn new(ask: Label, hint: Label) -> Self {
        Self {
            ask,
            hint,
            entering: false,
            typed: None,
            center: 0.0,
            top: 0.0,
            hovered: false,
            pressed: false,
            shake: Animation::idle(1.0),
        }
    }

    /// Centers it on `center`, its top at `top`.
    pub fn place(&mut self, center: f32, top: f32) {
        (self.center, self.top) = (center, top);
    }

    /// Turns it into a field, empty.
    pub fn open(&mut self) {
        (self.entering, self.typed) = (true, None);
    }

    /// Turns it back into its line.
    pub fn close(&mut self) {
        (self.entering, self.typed) = (false, None);
    }

    pub fn is_entering(&self) -> bool {
        self.entering
    }

    /// Shows `typed` in the field, or its hint if nothing is.
    pub fn show(&mut self, typed: Option<Label>) {
        self.typed = typed;
    }

    /// Shakes the field: what's typed isn't a code.
    pub fn refuse(&mut self) {
        self.shake = Animation::new(0.0, 1.0, motion::NORMAL, Easing::DECELERATE);
    }

    fn area(&self) -> Area {
        let width = if self.entering {
            FIELD
        } else {
            self.ask.width + 2.0 * PADDING
        };
        Area::new(self.center - width / 2.0, self.top, width, HEIGHT)
    }
}

impl Control for Code {
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
        let activated = self.pressed && !self.entering && self.contains(point);
        self.pressed = false;
        self.hovered = self.contains(point);
        activated
    }

    fn cancel(&mut self) {
        self.pressed = false;
    }

    fn focus_area(&self) -> (Area, f32) {
        (self.area(), RADIUS)
    }

    fn act(&mut self, action: Action) -> bool {
        action == Action::Activate && !self.entering
    }

    fn describe(&self, control: usize, elements: &mut Vec<Element>) {
        let name = match (&self.typed, self.entering) {
            (Some(typed), _) => format!("{}, {}", self.hint.text, typed.text),
            (None, true) => self.hint.text.clone(),
            (None, false) => self.ask.text.clone(),
        };
        elements.push(Element {
            control,
            name,
            role: Role::Button,
            area: self.area(),
            help: "",
        });
    }

    fn draw(&self, painter: &Painter, now: Instant) {
        let palette = painter.palette;
        let area = self.area();
        if !self.entering {
            let (shade, ink) = if self.pressed && self.hovered {
                (palette.subtle_pressed, palette.text_secondary)
            } else if self.hovered {
                (palette.subtle_hover, palette.text)
            } else {
                (Default::default(), palette.text_secondary)
            };
            painter.fill(painter.align(area), RADIUS, shade);
            painter.text(&self.ask, area.left + PADDING, area.middle(), ink);
            return;
        }
        // It swings less each time, and ends where it was.
        let shaken = self.shake.value(now);
        let swing = (shaken * SWINGS * std::f32::consts::TAU).sin() * (1.0 - shaken) * SHAKE;
        let area = painter.align(Area::new(
            area.left + swing,
            area.top,
            area.width,
            area.height,
        ));
        painter.fill(area, RADIUS, palette.button);
        painter.stroke(area, RADIUS, painter.border(), palette.accent);
        let center = area.left + area.width / 2.0;
        let Some(typed) = &self.typed else {
            let left = center - self.hint.width / 2.0;
            painter.text(&self.hint, left, area.middle(), palette.text_secondary);
            return;
        };
        let left = center - typed.width / 2.0;
        painter.text(typed, left, area.middle(), palette.text);
        let caret = Area::new(
            left + typed.width + CARET_GAP,
            area.middle() - CARET / 2.0,
            painter.border(),
            CARET,
        );
        painter.fill(caret, 0.0, palette.text);
    }

    fn is_animating(&self, now: Instant) -> bool {
        self.shake.is_running(now)
    }
}
