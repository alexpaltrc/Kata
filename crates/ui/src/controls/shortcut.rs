//! A keyboard shortcut shown as keycaps. A click on it asks for a new one, and
//! while it waits it shows a prompt instead.

use std::time::Instant;

use super::{Control, HEIGHT};
use crate::paint::{Area, Label, Painter, Point};

/// Inside a keycap, around its key.
const PADDING: f32 = 8.0;
/// Around the prompt.
const PROMPT_PADDING: f32 = 12.0;
const GAP: f32 = 4.0;
const RADIUS: f32 = 4.0;

pub struct Shortcut {
    keys: Vec<Label>,
    /// What it says while it waits for a new shortcut.
    prompt: Option<Label>,
    right: f32,
    middle: f32,
    hovered: bool,
    pressed: bool,
}

impl Shortcut {
    pub fn new(keys: Vec<Label>) -> Self {
        Self {
            keys,
            prompt: None,
            right: 0.0,
            middle: 0.0,
            hovered: false,
            pressed: false,
        }
    }

    pub fn set_keys(&mut self, keys: Vec<Label>) {
        self.keys = keys;
    }

    /// Lines its right edge up with `right` and centers it on `middle`.
    pub fn place(&mut self, right: f32, middle: f32) {
        (self.right, self.middle) = (right, middle);
    }

    /// Shows `prompt` while it waits for a new shortcut.
    pub fn listen(&mut self, prompt: Label) {
        self.prompt = Some(prompt);
    }

    pub fn stop(&mut self) {
        self.prompt = None;
    }

    pub fn is_listening(&self) -> bool {
        self.prompt.is_some()
    }

    pub fn width(&self) -> f32 {
        match &self.prompt {
            Some(prompt) => prompt.width + 2.0 * PROMPT_PADDING,
            None => {
                let keys: f32 = self.keys.iter().map(keycap).sum();
                keys + GAP * self.keys.len().saturating_sub(1) as f32
            }
        }
    }

    fn area(&self) -> Area {
        let width = self.width();
        Area::new(
            self.right - width,
            self.middle - HEIGHT / 2.0,
            width,
            HEIGHT,
        )
    }
}

/// A keycap's width: as wide as its key, but never narrower than it's tall.
fn keycap(key: &Label) -> f32 {
    (key.width + 2.0 * PADDING).max(HEIGHT)
}

impl Control for Shortcut {
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
        let activated = self.pressed && !self.is_listening() && self.contains(point);
        self.pressed = false;
        self.hovered = self.contains(point);
        activated
    }

    fn cancel(&mut self) {
        self.pressed = false;
    }

    fn draw(&self, painter: &Painter, _now: Instant) {
        let palette = painter.palette;
        let area = self.area();
        let border = painter.border();
        if let Some(prompt) = &self.prompt {
            let area = painter.align(area);
            painter.fill(area, RADIUS, palette.button);
            painter.stroke(area, RADIUS, border, palette.accent);
            let left = area.left + PROMPT_PADDING;
            painter.text(prompt, left, area.middle(), palette.text_secondary);
            return;
        }

        let (shade, ink) = if self.pressed && self.hovered {
            (palette.subtle_pressed, palette.text_secondary)
        } else if self.hovered {
            (palette.subtle_hover, palette.text)
        } else {
            (Default::default(), palette.text)
        };
        let mut left = area.left;
        for key in &self.keys {
            let width = keycap(key);
            let cap = painter.align(Area::new(left, area.top, width, HEIGHT));
            painter.fill(cap, RADIUS, palette.button);
            painter.fill(cap, RADIUS, shade);
            painter.stroke(cap, RADIUS, border, palette.control_stroke);
            let text = cap.left + (cap.width - key.width) / 2.0;
            painter.text(key, text, cap.middle(), ink);
            left += width + GAP;
        }
    }

    fn is_animating(&self, _now: Instant) -> bool {
        false
    }
}
