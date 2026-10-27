//! A row of text items with one selected, marked by a pill that glides between them.

use std::time::Instant;

use super::Control;
use crate::{
    motion::{self, Animation, Easing},
    paint::{Area, Label, Painter, Point},
};

const PADDING: f32 = 12.0;
/// The selection pill's size.
const PILL: (f32, f32) = (16.0, 3.0);

pub struct SelectorBar {
    items: Vec<Label>,
    left: f32,
    top: f32,
    selected: usize,
    /// The pill's center, from the bar's left edge.
    pill: Animation,
    hovered: Option<usize>,
    pressed: Option<usize>,
}

impl SelectorBar {
    pub const HEIGHT: f32 = 40.0;

    pub fn new(items: Vec<Label>, selected: usize) -> Self {
        let mut bar = Self {
            items,
            left: 0.0,
            top: 0.0,
            selected,
            pill: Animation::idle(0.0),
            hovered: None,
            pressed: None,
        };
        bar.pill = Animation::idle(bar.center(selected));
        bar
    }

    pub fn width(&self) -> f32 {
        self.items
            .iter()
            .map(|item| item.width + 2.0 * PADDING)
            .sum()
    }

    pub fn place(&mut self, left: f32, top: f32) {
        (self.left, self.top) = (left, top);
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn select(&mut self, index: usize) {
        if index != self.selected {
            self.selected = index;
            let center = self.center(index);
            self.pill = self
                .pill
                .retarget(center, motion::NORMAL, Easing::POINT_TO_POINT);
        }
    }

    fn item(&self, index: usize) -> Area {
        let before: f32 = self.items[..index]
            .iter()
            .map(|item| item.width + 2.0 * PADDING)
            .sum();
        let width = self.items[index].width + 2.0 * PADDING;
        Area::new(self.left + before, self.top, width, Self::HEIGHT)
    }

    fn center(&self, index: usize) -> f32 {
        let item = self.item(index);
        item.left - self.left + item.width / 2.0
    }

    fn item_at(&self, point: Point) -> Option<usize> {
        (0..self.items.len()).find(|&index| self.item(index).contains(point))
    }
}

impl Control for SelectorBar {
    fn contains(&self, point: Point) -> bool {
        self.item_at(point).is_some()
    }

    fn hover(&mut self, point: Option<Point>) -> bool {
        let hovered = point.and_then(|point| self.item_at(point));
        std::mem::replace(&mut self.hovered, hovered) != hovered
    }

    fn press(&mut self, point: Point) -> bool {
        self.pressed = self.item_at(point);
        false
    }

    fn release(&mut self, point: Point) -> bool {
        match self.pressed.take() {
            Some(index) if self.item_at(point) == Some(index) && index != self.selected => {
                self.select(index);
                true
            }
            _ => false,
        }
    }

    fn cancel(&mut self) {
        self.pressed = None;
    }

    fn draw(&self, painter: &Painter, now: Instant) {
        let palette = painter.palette;
        for (index, label) in self.items.iter().enumerate() {
            let item = self.item(index);
            if self.hovered == Some(index) {
                let back = Area::new(item.left, item.top + 4.0, item.width, item.height - 8.0);
                let color = if self.pressed == Some(index) {
                    palette.subtle_pressed
                } else {
                    palette.subtle_hover
                };
                painter.fill(back, 4.0, color);
            }
            let color = if index == self.selected {
                palette.text
            } else {
                palette.text_secondary
            };
            painter.text(label, item.left + PADDING, item.middle(), color);
        }
        let (width, height) = PILL;
        let center = self.left + self.pill.value(now);
        let pill = Area::new(
            painter.snap(center - width / 2.0),
            painter.snap(self.top + Self::HEIGHT - 4.0 - height),
            width,
            height,
        );
        painter.fill(pill, height / 2.0, palette.accent);
    }

    fn is_animating(&self, now: Instant) -> bool {
        self.pill.is_running(now)
    }
}
