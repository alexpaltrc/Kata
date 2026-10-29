//! An on/off switch, its label on the left and the switch on the right.

use std::time::Instant;

use super::{Action, Control, HEIGHT};
use crate::{
    access::{Element, Role},
    motion::{self, Animation, Easing},
    paint::{Area, Label, Painter, Point},
    theme::{faded, mix},
};

const SWITCH: (f32, f32) = (40.0, 20.0);
/// Between the label and the switch.
const GAP: f32 = 12.0;
const KNOB: f32 = 6.0;
const KNOB_HOVERED: f32 = 7.0;

pub struct Toggle {
    label: Label,
    on: bool,
    left: f32,
    top: f32,
    /// The area it answers to, which can be wider and taller than it looks.
    size: (f32, f32),
    /// From 0 when off to 1 when on.
    state: Animation,
    knob: Animation,
    hovered: bool,
    pressed: bool,
}

impl Toggle {
    pub fn new(label: Label, on: bool) -> Self {
        let size = (label.width + GAP + SWITCH.0, HEIGHT);
        Self {
            label,
            on,
            left: 0.0,
            top: 0.0,
            size,
            state: Animation::idle(if on { 1.0 } else { 0.0 }),
            knob: Animation::idle(KNOB),
            hovered: false,
            pressed: false,
        }
    }

    pub fn width(&self) -> f32 {
        self.label.width + GAP + SWITCH.0
    }

    pub fn place(&mut self, left: f32, top: f32) {
        (self.left, self.top) = (left, top);
    }

    /// Spreads it over `width` by `height` DIPs, as a whole row of settings: the
    /// label stays on the left and the switch moves to the right.
    pub fn stretch(&mut self, width: f32, height: f32) {
        self.size = (width.max(self.width()), height.max(HEIGHT));
    }

    pub fn is_on(&self) -> bool {
        self.on
    }

    /// Switches it without a click.
    pub fn set(&mut self, on: bool) {
        if on != self.on {
            self.on = on;
            let state = if on { 1.0 } else { 0.0 };
            self.state = self
                .state
                .retarget(state, motion::FAST, Easing::POINT_TO_POINT);
        }
    }

    fn area(&self) -> Area {
        Area::new(self.left, self.top, self.size.0, self.size.1)
    }

    /// The switch, on the right, centered on `middle`.
    fn switch(&self, middle: f32) -> Area {
        let (width, height) = SWITCH;
        Area::new(
            self.area().right() - width,
            middle - height / 2.0,
            width,
            height,
        )
    }

    fn update_knob(&mut self) {
        let radius = if self.hovered || self.pressed {
            KNOB_HOVERED
        } else {
            KNOB
        };
        self.knob = self.knob.retarget(radius, motion::FAST, Easing::DECELERATE);
    }
}

impl Control for Toggle {
    fn contains(&self, point: Point) -> bool {
        self.area().contains(point)
    }

    fn hover(&mut self, point: Option<Point>) -> bool {
        let hovered = point.is_some_and(|point| self.contains(point));
        if hovered == self.hovered {
            return false;
        }
        self.hovered = hovered;
        self.update_knob();
        true
    }

    fn press(&mut self, _point: Point) -> bool {
        self.pressed = true;
        self.update_knob();
        false
    }

    fn release(&mut self, point: Point) -> bool {
        let activated = self.pressed && self.contains(point);
        self.pressed = false;
        self.hovered = self.contains(point);
        if activated {
            self.set(!self.on);
        }
        self.update_knob();
        activated
    }

    fn cancel(&mut self) {
        self.pressed = false;
        self.update_knob();
    }

    fn focus_area(&self) -> (Area, f32) {
        (self.switch(self.area().middle()), SWITCH.1 / 2.0)
    }

    fn act(&mut self, action: Action) -> bool {
        if action == Action::Activate {
            self.set(!self.on);
        }
        action == Action::Activate
    }

    fn describe(&self, control: usize, elements: &mut Vec<Element>) {
        elements.push(Element {
            control,
            name: self.label.text.clone(),
            role: Role::Toggle { on: self.on },
            area: self.area(),
        });
    }

    fn draw(&self, painter: &Painter, now: Instant) {
        let palette = painter.palette;
        let area = self.area();
        let middle = painter.snap(area.middle());
        painter.text(&self.label, area.left, middle, palette.text);

        let (width, height) = SWITCH;
        let radius = height / 2.0;
        let switch = self.switch(middle);
        let on = self.state.value(now);
        let accent = if self.hovered {
            faded(palette.accent, 0.9)
        } else {
            palette.accent
        };
        painter.fill(switch, radius, faded(palette.alt, 1.0 - on));
        painter.stroke(switch, radius, 1.0, faded(palette.strong, 1.0 - on));
        painter.fill(switch, radius, faded(accent, on));

        let x = switch.left + radius + (width - height) * on;
        let knob = mix(palette.text_secondary, palette.on_accent, on);
        painter.circle((x, middle), self.knob.value(now), knob);
    }

    fn is_animating(&self, now: Instant) -> bool {
        self.state.is_running(now) || self.knob.is_running(now)
    }
}
