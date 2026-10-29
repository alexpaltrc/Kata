//! A slider with a few steps, its value named beside it.

use std::time::Instant;

use super::{Action, Control, HEIGHT};
use crate::{
    access::{Element, Role},
    motion::{self, Animation, Easing},
    paint::{Area, Label, Painter, Point},
};

/// The thumb's radius.
const THUMB: f32 = 10.0;
const RAIL: f32 = 4.0;
/// How far the thumb travels from the first step to the last.
const TRAVEL: f32 = 96.0;
/// Between the slider and its value.
const GAP: f32 = 12.0;
/// The radius of the accent dot inside the thumb.
const DOT: f32 = 6.0;
const DOT_HOVERED: f32 = 7.0;
const DOT_PRESSED: f32 = 5.0;

pub struct Slider {
    /// What assistive technologies call it.
    name: String,
    /// What each step is called.
    names: Vec<Label>,
    value: usize,
    left: f32,
    top: f32,
    /// The thumb's place, from 0 at the first step to 1 at the last.
    thumb: Animation,
    dot: Animation,
    hovered: bool,
    pressed: bool,
}

impl Slider {
    /// A slider called `name`, with one step per name in `names`.
    pub fn new(name: &str, names: Vec<Label>, value: usize) -> Self {
        let mut slider = Self {
            name: name.to_owned(),
            names,
            value,
            left: 0.0,
            top: 0.0,
            thumb: Animation::idle(0.0),
            dot: Animation::idle(DOT),
            hovered: false,
            pressed: false,
        };
        slider.thumb = Animation::idle(slider.place_of(value));
        slider
    }

    pub fn width(&self) -> f32 {
        let name = self.names.iter().map(|name| name.width).fold(0.0, f32::max);
        self.track().width + GAP + name
    }

    pub fn place(&mut self, left: f32, top: f32) {
        (self.left, self.top) = (left, top);
    }

    pub fn value(&self) -> usize {
        self.value
    }

    pub fn set_value(&mut self, value: usize) {
        if value != self.value {
            self.value = value;
            let place = self.place_of(value);
            self.thumb = self
                .thumb
                .retarget(place, motion::FAST, Easing::POINT_TO_POINT);
        }
    }

    /// Where the thumb can be.
    fn track(&self) -> Area {
        Area::new(self.left, self.top, TRAVEL + 2.0 * THUMB, HEIGHT)
    }

    fn place_of(&self, value: usize) -> f32 {
        value as f32 / self.last() as f32
    }

    fn last(&self) -> usize {
        self.names.len().saturating_sub(1).max(1)
    }

    fn step_at(&self, x: f32) -> usize {
        let place = (x - self.left - THUMB) / TRAVEL;
        (place * self.last() as f32)
            .round()
            .clamp(0.0, (self.names.len() - 1) as f32) as usize
    }

    fn update_dot(&mut self) {
        let radius = if self.pressed {
            DOT_PRESSED
        } else if self.hovered {
            DOT_HOVERED
        } else {
            DOT
        };
        self.dot = self.dot.retarget(radius, motion::FAST, Easing::DECELERATE);
    }
}

impl Control for Slider {
    fn contains(&self, point: Point) -> bool {
        self.track().contains(point)
    }

    fn hover(&mut self, point: Option<Point>) -> bool {
        let hovered = point.is_some_and(|point| self.contains(point));
        if hovered == self.hovered {
            return false;
        }
        self.hovered = hovered;
        self.update_dot();
        true
    }

    fn press(&mut self, point: Point) -> bool {
        self.pressed = true;
        self.update_dot();
        self.drag(point)
    }

    fn drag(&mut self, (x, _): Point) -> bool {
        let step = self.step_at(x);
        let changed = step != self.value;
        self.set_value(step);
        changed
    }

    fn release(&mut self, point: Point) -> bool {
        self.pressed = false;
        self.hovered = self.contains(point);
        self.update_dot();
        false
    }

    fn cancel(&mut self) {
        self.pressed = false;
        self.update_dot();
    }

    fn focus_area(&self) -> (Area, f32) {
        (self.track(), THUMB)
    }

    fn act(&mut self, action: Action) -> bool {
        match action.step(self.value, self.names.len()) {
            Some(step) if step != self.value => {
                self.set_value(step);
                true
            }
            _ => false,
        }
    }

    fn describe(&self, control: usize, elements: &mut Vec<Element>) {
        elements.push(Element {
            control,
            name: self.name.clone(),
            role: Role::Slider {
                value: self.value,
                count: self.names.len(),
                text: self.names[self.value].text.clone(),
            },
            area: Area::new(self.left, self.top, self.width(), HEIGHT),
        });
    }

    fn draw(&self, painter: &Painter, now: Instant) {
        let palette = painter.palette;
        let track = self.track();
        let middle = painter.snap(track.middle());
        let x = track.left + THUMB + TRAVEL * self.thumb.value(now);

        let rail = Area::new(track.left, middle - RAIL / 2.0, track.width, RAIL);
        painter.fill(rail, RAIL / 2.0, palette.strong);
        let done = Area::new(rail.left, rail.top, x - rail.left, RAIL);
        painter.fill(done, RAIL / 2.0, palette.accent);

        painter.circle((x, middle), THUMB, palette.control);
        painter.ring((x, middle), THUMB, painter.pixel(), palette.control_stroke);
        painter.circle((x, middle), self.dot.value(now), palette.accent);

        let name = &self.names[self.value];
        painter.text(name, track.right() + GAP, middle, palette.text);
    }

    fn is_animating(&self, now: Instant) -> bool {
        self.thumb.is_running(now) || self.dot.is_running(now)
    }
}
