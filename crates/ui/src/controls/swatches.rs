//! A row of colors to pick one from, as dots. The last can be a color of the
//! user's own, ringed in Kata's colors to set it apart from the ready-made ones.

use std::{f32::consts::TAU, time::Instant};

use super::{Action, Control};
use crate::{
    access::{Element, Role},
    paint::{Area, Painter, Point},
    theme::{color, signature},
};

/// A dot's size, and the room each one takes in the row.
const DOT: f32 = 18.0;
const SLOT: f32 = 28.0;
/// The ring round the chosen dot: how far from it, and how thick.
const RING_GAP: f32 = 2.0;
const RING: f32 = 2.0;
/// The ring in Kata's colors round a color of the user's own.
const OWN_RING: f32 = 2.5;

pub struct Swatches {
    /// The colors, as `0xRRGGBB`, each with what assistive technologies call it.
    colors: Vec<(u32, String)>,
    /// Whether the last color is the user's own.
    own: bool,
    selected: usize,
    left: f32,
    middle: f32,
    hovered: Option<usize>,
    pressed: Option<usize>,
}

impl Swatches {
    /// The colors in `colors`, the one at `selected` chosen. With `own`, the last is
    /// a color of the user's own.
    pub fn new(colors: Vec<(u32, String)>, own: bool, selected: usize) -> Self {
        Self {
            selected: selected.min(colors.len().saturating_sub(1)),
            colors,
            own,
            left: 0.0,
            middle: 0.0,
            hovered: None,
            pressed: None,
        }
    }

    pub fn width(&self) -> f32 {
        self.colors.len() as f32 * SLOT
    }

    /// Puts the row's right end at `right`, centered on `middle`.
    pub fn place(&mut self, right: f32, middle: f32) {
        (self.left, self.middle) = (right - self.width(), middle);
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn select(&mut self, index: usize) {
        self.selected = index.min(self.colors.len().saturating_sub(1));
    }

    /// Gives the color at `index` another value.
    pub fn set_color(&mut self, index: usize, rgb: u32) {
        if let Some((color, _)) = self.colors.get_mut(index) {
            *color = rgb;
        }
    }

    fn slot(&self, index: usize) -> Area {
        let left = self.left + index as f32 * SLOT;
        Area::new(left, self.middle - SLOT / 2.0, SLOT, SLOT)
    }

    fn center(&self, index: usize) -> Point {
        let slot = self.slot(index);
        (slot.left + SLOT / 2.0, self.middle)
    }

    fn at(&self, point: Point) -> Option<usize> {
        (0..self.colors.len()).find(|&index| self.slot(index).contains(point))
    }
}

impl Control for Swatches {
    fn contains(&self, point: Point) -> bool {
        self.at(point).is_some()
    }

    fn hover(&mut self, point: Option<Point>) -> bool {
        let hovered = point.and_then(|point| self.at(point));
        std::mem::replace(&mut self.hovered, hovered) != hovered
    }

    fn press(&mut self, point: Point) -> bool {
        self.pressed = self.at(point);
        false
    }

    fn release(&mut self, point: Point) -> bool {
        match self.pressed.take() {
            Some(index) if self.at(point) == Some(index) && index != self.selected => {
                self.selected = index;
                true
            }
            _ => false,
        }
    }

    fn cancel(&mut self) {
        self.pressed = None;
    }

    fn focus_area(&self) -> (Area, f32) {
        (self.slot(self.selected), SLOT / 2.0)
    }

    fn act(&mut self, action: Action) -> bool {
        match action.step(self.selected, self.colors.len()) {
            Some(index) if index != self.selected => {
                self.selected = index;
                true
            }
            _ => false,
        }
    }

    fn describe(&self, control: usize, elements: &mut Vec<Element>) {
        let colors = self.colors.iter().enumerate();
        elements.extend(colors.map(|(index, (_, name))| Element {
            control,
            name: name.clone(),
            role: Role::Choice {
                index,
                selected: index == self.selected,
            },
            area: self.slot(index),
            help: "",
        }));
    }

    fn draw(&self, painter: &Painter, _now: Instant) {
        let palette = painter.palette;
        let last = self.colors.len().saturating_sub(1);
        for (index, &(rgb, _)) in self.colors.iter().enumerate() {
            let center = self.center(index);
            let radius = DOT / 2.0;
            if self.hovered == Some(index) {
                painter.circle(center, SLOT / 2.0, palette.subtle_hover);
            }
            if self.own && index == last && !palette.contrast {
                // Kata's four colors, a quarter of the way round each.
                let colors = signature(palette.light);
                for (quarter, &tint) in colors.iter().enumerate() {
                    let from = quarter as f32 * TAU / 4.0;
                    let ring = radius - OWN_RING / 2.0;
                    painter.arc(
                        center,
                        ring,
                        from,
                        from + TAU / 4.0,
                        OWN_RING,
                        color(tint, 1.0),
                    );
                }
                painter.circle(center, radius - OWN_RING - 1.0, color(rgb, 1.0));
            } else {
                painter.circle(center, radius, color(rgb, 1.0));
            }
            if index == self.selected {
                let ring = radius + RING_GAP + RING;
                painter.ring(center, ring, RING, palette.text);
            }
        }
    }

    fn is_animating(&self, _now: Instant) -> bool {
        false
    }
}
