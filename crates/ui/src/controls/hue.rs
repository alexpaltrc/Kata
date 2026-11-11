//! A slider through every hue: a track that shows them all, and a thumb in the hue
//! it's on. What color each hue is, the window says: it knows how it wants them.

use std::time::Instant;

use super::{Action, Control};
use crate::{
    access::{Element, Role},
    paint::{Area, Painter, Point},
    theme::color,
};

/// Every hue, in degrees.
const HUES: usize = 360;
const WIDTH: f32 = 132.0;
const TRACK: f32 = 6.0;
const THUMB: f32 = 9.0;
/// How many colors the track is painted with.
const STOPS: usize = 13;
/// How tall it is to the pointer.
const HEIGHT: f32 = 28.0;

pub struct Hue {
    name: String,
    /// The color of each hue, as `0xRRGGBB`.
    tint: fn(f32) -> u32,
    hue: usize,
    left: f32,
    middle: f32,
    hovered: bool,
    dragging: bool,
}

impl Hue {
    /// A slider called `name` on `hue`, in degrees, whose hues look as `tint` says.
    pub fn new(name: &str, tint: fn(f32) -> u32, hue: usize) -> Self {
        Self {
            name: name.to_owned(),
            tint,
            hue: hue % HUES,
            left: 0.0,
            middle: 0.0,
            hovered: false,
            dragging: false,
        }
    }

    pub const WIDTH: f32 = WIDTH + 2.0 * THUMB;

    /// Puts its right end at `right`, centered on `middle`.
    pub fn place(&mut self, right: f32, middle: f32) {
        (self.left, self.middle) = (right - Self::WIDTH, middle);
    }

    /// The hue it's on, in degrees.
    pub fn hue(&self) -> usize {
        self.hue
    }

    fn area(&self) -> Area {
        Area::new(self.left, self.middle - HEIGHT / 2.0, Self::WIDTH, HEIGHT)
    }

    fn track(&self) -> Area {
        Area::new(self.left + THUMB, self.middle - TRACK / 2.0, WIDTH, TRACK)
    }

    /// Follows the pointer to `x`. Returns whether the hue changed.
    fn follow(&mut self, x: f32) -> bool {
        let along = ((x - self.track().left) / WIDTH).clamp(0.0, 1.0);
        let hue = ((along * (HUES - 1) as f32).round() as usize).min(HUES - 1);
        std::mem::replace(&mut self.hue, hue) != hue
    }
}

impl Control for Hue {
    fn contains(&self, point: Point) -> bool {
        self.area().contains(point)
    }

    fn hover(&mut self, point: Option<Point>) -> bool {
        let hovered = point.is_some_and(|point| self.contains(point));
        std::mem::replace(&mut self.hovered, hovered) != hovered
    }

    fn press(&mut self, point: Point) -> bool {
        self.dragging = true;
        self.follow(point.0)
    }

    fn drag(&mut self, point: Point) -> bool {
        self.dragging && self.follow(point.0)
    }

    /// The hue changes as the thumb moves, so letting go changes nothing more.
    fn release(&mut self, _point: Point) -> bool {
        self.dragging = false;
        false
    }

    fn cancel(&mut self) {
        self.dragging = false;
    }

    fn focus_area(&self) -> (Area, f32) {
        (self.area(), HEIGHT / 2.0)
    }

    fn act(&mut self, action: Action) -> bool {
        match action.step(self.hue, HUES) {
            Some(hue) if hue != self.hue => {
                self.hue = hue;
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
                value: self.hue,
                count: HUES,
                text: format!("{}°", self.hue),
            },
            area: self.area(),
            help: "",
        });
    }

    fn draw(&self, painter: &Painter, _now: Instant) {
        let palette = painter.palette;
        let track = painter.align(self.track());
        let stops: Vec<_> = (0..STOPS)
            .map(|stop| {
                let hue = stop as f32 / (STOPS - 1) as f32 * (HUES - 1) as f32;
                color((self.tint)(hue), 1.0)
            })
            .collect();
        painter.spectrum(track, TRACK / 2.0, &stops);
        let along = self.hue as f32 / (HUES - 1) as f32;
        let center = (track.left + along * track.width, self.middle);
        let grown = if self.hovered || self.dragging {
            1.0
        } else {
            0.0
        };
        // White round the hue, as the ink is edged, over a thin dark line that sets it
        // apart from a light window.
        painter.circle(
            center,
            THUMB + grown + painter.border(),
            palette.control_stroke,
        );
        painter.circle(center, THUMB + grown, color(0xFF_FFFF, 1.0));
        painter.circle(
            center,
            THUMB + grown - 2.5,
            color((self.tint)(self.hue as f32), 1.0),
        );
    }

    fn is_animating(&self, _now: Instant) -> bool {
        false
    }
}
