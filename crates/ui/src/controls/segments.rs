//! A few choices side by side, as words, with one chosen: a raised highlight
//! glides to it, stretching on its way, as a dock's does.

use std::time::Instant;

use super::{Action, Control, HEIGHT};
use crate::{
    access::{Element, Role},
    motion::{self, Animation, Easing},
    paint::{Area, Label, Painter, Point},
};

/// Around each word, inside its segment; and around the segments, inside the
/// control.
const PADDING: f32 = 12.0;
const INSET: f32 = 3.0;
const RADIUS: f32 = 4.0;

pub struct Segments {
    words: Vec<Label>,
    selected: usize,
    left: f32,
    top: f32,
    /// The highlight's left and right edges, from the control's left edge.
    start: Animation,
    end: Animation,
    hovered: Option<usize>,
    pressed: Option<usize>,
}

impl Segments {
    /// The choices `words` name, the one at `selected` chosen.
    pub fn new(words: Vec<Label>, selected: usize) -> Self {
        let mut segments = Self {
            selected: selected.min(words.len().saturating_sub(1)),
            words,
            left: 0.0,
            top: 0.0,
            start: Animation::idle(0.0),
            end: Animation::idle(0.0),
            hovered: None,
            pressed: None,
        };
        let (start, end) = segments.span(segments.selected);
        (segments.start, segments.end) = (Animation::idle(start), Animation::idle(end));
        segments
    }

    pub fn width(&self) -> f32 {
        self.span(self.words.len().saturating_sub(1)).1 + INSET
    }

    /// Puts its right end at `right`, centered on `middle`.
    pub fn place(&mut self, right: f32, middle: f32) {
        (self.left, self.top) = (right - self.width(), middle - HEIGHT / 2.0);
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    fn select(&mut self, index: usize) {
        // The edge in front sets off first and the one behind follows, so the
        // highlight stretches toward the new choice and settles there.
        let forward = index > self.selected;
        let (lead, trail) = (motion::FAST, motion::NORMAL);
        let (start, end) = self.span(index);
        let ease = Easing::POINT_TO_POINT;
        self.start = self
            .start
            .retarget(start, if forward { trail } else { lead }, ease);
        self.end = self
            .end
            .retarget(end, if forward { lead } else { trail }, ease);
        self.selected = index;
    }

    /// Where a segment's left and right edges are, from the control's left edge.
    fn span(&self, index: usize) -> (f32, f32) {
        let width = |word: &Label| word.width + 2.0 * PADDING;
        let start = INSET + self.words[..index].iter().map(width).sum::<f32>();
        (start, start + self.words.get(index).map_or(0.0, width))
    }

    fn area(&self) -> Area {
        Area::new(self.left, self.top, self.width(), HEIGHT)
    }

    fn segment(&self, index: usize) -> Area {
        let (start, end) = self.span(index);
        let height = HEIGHT - 2.0 * INSET;
        Area::new(self.left + start, self.top + INSET, end - start, height)
    }

    fn at(&self, point: Point) -> Option<usize> {
        (0..self.words.len()).find(|&index| self.segment(index).contains(point))
    }
}

impl Control for Segments {
    fn contains(&self, point: Point) -> bool {
        self.area().contains(point)
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
                self.select(index);
                true
            }
            _ => false,
        }
    }

    fn cancel(&mut self) {
        self.pressed = None;
    }

    fn focus_area(&self) -> (Area, f32) {
        (self.area(), RADIUS)
    }

    fn act(&mut self, action: Action) -> bool {
        match action.step(self.selected, self.words.len()) {
            Some(index) if index != self.selected => {
                self.select(index);
                true
            }
            _ => false,
        }
    }

    fn describe(&self, control: usize, elements: &mut Vec<Element>) {
        let words = self.words.iter().enumerate();
        elements.extend(words.map(|(index, word)| Element {
            control,
            name: word.text.clone(),
            role: Role::Choice {
                index,
                selected: index == self.selected,
            },
            area: self.segment(index),
            help: "",
        }));
    }

    fn draw(&self, painter: &Painter, now: Instant) {
        let palette = painter.palette;
        let area = painter.align(self.area());
        painter.fill(area, RADIUS, palette.alt);
        painter.stroke(area, RADIUS, painter.border(), palette.control_stroke);
        if let Some(index) = self.hovered.filter(|&index| index != self.selected) {
            let shade = if self.pressed == Some(index) {
                palette.subtle_pressed
            } else {
                palette.subtle_hover
            };
            painter.fill(self.segment(index), RADIUS - 1.0, shade);
        }
        let (start, end) = (self.start.value(now), self.end.value(now));
        let row = self.segment(0);
        let highlight = Area::new(self.left + start, row.top, end - start, row.height);
        let highlight = painter.align(highlight);
        painter.fill(highlight, RADIUS - 1.0, palette.button);
        painter.stroke(
            highlight,
            RADIUS - 1.0,
            painter.border(),
            palette.control_stroke,
        );
        for (index, word) in self.words.iter().enumerate() {
            let segment = self.segment(index);
            let left = segment.left + (segment.width - word.width) / 2.0;
            let ink = if index == self.selected {
                palette.text
            } else {
                palette.text_secondary
            };
            painter.text(word, left, segment.middle(), ink);
        }
    }

    fn is_animating(&self, now: Instant) -> bool {
        self.start.is_running(now) || self.end.is_running(now)
    }
}
