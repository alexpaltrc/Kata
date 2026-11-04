//! A floating row of icons with one chosen, such as an editor's tools. A highlight
//! glides to the chosen one, stretching as it goes, and its icon fills in. Their
//! names show under the icons, or in a tooltip once the pointer rests on one. A
//! compact dock sits inside another capsule, such as a tool's options.

use std::time::Instant;

use super::{Action, Control, tip};
use crate::{
    access::{Element, Role},
    figure::Figure,
    motion::{self, Animation, Easing},
    paint::{Area, Label, Painter, Point},
    theme::{faded, mix},
};

/// Around the items, inside the dock.
const PADDING: f32 = 4.0;
/// An item's size while the names are hidden.
const ITEM: (f32, f32) = (48.0, 40.0);
/// An item's size in a compact dock.
const COMPACT: (f32, f32) = (40.0, 32.0);
/// An item's height while the names show.
const NAMED: f32 = 52.0;
/// Between a name and its item's edges, and between the icon and its name.
const NAME_PADDING: f32 = 8.0;
const NAME_GAP: f32 = 2.0;
/// How strongly the highlight tints the chosen item with the accent color, on
/// light and dark surfaces.
const TINT: (f32, f32) = (0.12, 0.16);

/// One of the dock's choices.
pub struct Item {
    /// Its icon, drawn `Dock::ICON` DIPs square, outlined.
    pub outlined: Figure,
    /// The same icon filled in, as it shows while chosen.
    pub filled: Figure,
    pub name: Label,
}

pub struct Dock {
    items: Vec<Item>,
    /// Whether the names show under the icons.
    named: bool,
    /// Whether it's compact: smaller, unnamed and without a capsule of its own.
    compact: bool,
    left: f32,
    top: f32,
    selected: usize,
    /// The highlight's left and right edges, from the dock's left edge.
    start: Animation,
    end: Animation,
    /// How chosen each item looks, from 0 to 1.
    lit: Vec<Animation>,
    hovered: Option<usize>,
    pressed: Option<usize>,
    /// The item the tooltip names, and how much it shows.
    tip: usize,
    shown: Animation,
    /// What the tooltip stays within.
    bounds: Area,
}

impl Dock {
    /// The icons' size.
    pub const ICON: f32 = 20.0;

    pub fn new(items: Vec<Item>, selected: usize, named: bool) -> Self {
        Self::build(items, selected, named, false)
    }

    pub fn compact(items: Vec<Item>, selected: usize) -> Self {
        Self::build(items, selected, false, true)
    }

    fn build(items: Vec<Item>, selected: usize, named: bool, compact: bool) -> Self {
        let lit = (0..items.len())
            .map(|index| Animation::idle(if index == selected { 1.0 } else { 0.0 }))
            .collect();
        let mut dock = Self {
            items,
            named,
            compact,
            left: 0.0,
            top: 0.0,
            selected,
            start: Animation::idle(0.0),
            end: Animation::idle(0.0),
            lit,
            hovered: None,
            pressed: None,
            tip: selected,
            shown: Animation::idle(0.0),
            bounds: Area::default(),
        };
        let (start, end) = dock.span(selected);
        (dock.start, dock.end) = (Animation::idle(start), Animation::idle(end));
        dock
    }

    pub fn width(&self) -> f32 {
        self.items.len() as f32 * self.item_size().0 + 2.0 * PADDING
    }

    pub fn height(&self) -> f32 {
        self.item_size().1 + 2.0 * PADDING
    }

    /// Puts the dock's top-left corner at `left` and `top`, with the tooltip kept
    /// within `bounds`.
    pub fn place(&mut self, left: f32, top: f32, bounds: Area) {
        (self.left, self.top) = (left, top);
        self.bounds = bounds;
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn select(&mut self, index: usize) {
        if index == self.selected {
            return;
        }
        // The edge in front sets off first and the one behind follows, so the
        // highlight stretches toward the new item and settles there.
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
        for (item, lit) in self.lit.iter_mut().enumerate() {
            let target = if item == index { 1.0 } else { 0.0 };
            *lit = lit.retarget(target, motion::FAST, Easing::DECELERATE);
        }
        self.selected = index;
    }

    /// Whether the pointer is on an item while its name is hidden, so that resting
    /// there would show it in a tooltip.
    pub fn awaits_rest(&self) -> bool {
        !self.named && self.hovered.is_some() && self.shown.target() == 0.0
    }

    /// The pointer rested at `point`. Returns whether that shows a tooltip.
    pub fn rest(&mut self, point: Point) -> bool {
        let Some(index) = self.item_at(point).filter(|_| !self.named) else {
            return false;
        };
        self.tip = index;
        self.show(true);
        true
    }

    /// Draws the tooltip, while it shows: over the item it names, just above `top`,
    /// such as the top of what sits on the dock. It goes over everything else.
    pub fn draw_tip(&self, painter: &Painter, top: f32, now: Instant) {
        let item = self.item(self.tip);
        let bottom = self.top + self.height();
        let column = Area::new(item.left, top, item.width, bottom - top);
        let shown = self.shown.value(now);
        tip::draw(
            painter,
            &self.items[self.tip].name,
            column,
            self.bounds,
            shown,
        );
    }

    fn show(&mut self, shown: bool) {
        let target = if shown { 1.0 } else { 0.0 };
        self.shown = self
            .shown
            .retarget(target, motion::FAST, Easing::DECELERATE);
    }

    fn area(&self) -> Area {
        Area::new(self.left, self.top, self.width(), self.height())
    }

    fn item_size(&self) -> (f32, f32) {
        if self.compact {
            return COMPACT;
        }
        if !self.named {
            return ITEM;
        }
        let name = self
            .items
            .iter()
            .map(|item| item.name.width)
            .fold(0.0, f32::max);
        (ITEM.0.max(name + 2.0 * NAME_PADDING), NAMED)
    }

    /// Where an item's left and right edges are, from the dock's left edge.
    fn span(&self, index: usize) -> (f32, f32) {
        let width = self.item_size().0;
        let start = PADDING + index as f32 * width;
        (start, start + width)
    }

    fn item(&self, index: usize) -> Area {
        let (width, height) = self.item_size();
        let (start, _) = self.span(index);
        Area::new(self.left + start, self.top + PADDING, width, height)
    }

    fn item_at(&self, point: Point) -> Option<usize> {
        (0..self.items.len()).find(|&index| self.item(index).contains(point))
    }
}

impl Control for Dock {
    /// The whole dock, so the pointer never reaches what's behind it.
    fn contains(&self, point: Point) -> bool {
        self.area().contains(point)
    }

    fn hover(&mut self, point: Option<Point>) -> bool {
        let hovered = point.and_then(|point| self.item_at(point));
        let (tip, shown) = (self.tip, self.shown.target());
        if !point.is_some_and(|point| self.contains(point)) {
            self.show(false);
        } else if let Some(index) = hovered
            && shown > 0.0
        {
            // While a tooltip shows, the next item's takes its place at once.
            self.tip = index;
        }
        let hover_changed = std::mem::replace(&mut self.hovered, hovered) != hovered;
        hover_changed || tip != self.tip || shown != self.shown.target()
    }

    /// A click puts the tooltip away, as in Fluent.
    fn press(&mut self, point: Point) -> bool {
        self.show(false);
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

    /// The chosen item: the arrow keys move the choice along with the focus.
    fn focus_area(&self) -> (Area, f32) {
        let item = self.item(self.selected);
        (item, item.height / 2.0)
    }

    fn act(&mut self, action: Action) -> bool {
        match action.step(self.selected, self.items.len()) {
            Some(index) if index != self.selected => {
                self.select(index);
                true
            }
            _ => false,
        }
    }

    /// Each item, on its own.
    fn describe(&self, control: usize, elements: &mut Vec<Element>) {
        elements.extend(self.items.iter().enumerate().map(|(index, item)| Element {
            control,
            name: item.name.text.clone(),
            role: Role::Choice {
                index,
                selected: index == self.selected,
            },
            area: self.item(index),
            help: "",
        }));
    }

    fn draw(&self, painter: &Painter, now: Instant) {
        let palette = painter.palette;
        let dock = self.area();
        if !self.compact {
            painter.flyout(dock, dock.height / 2.0);
        }
        let radius = self.item_size().1 / 2.0;
        if let Some(index) = self.hovered {
            let color = if self.pressed == Some(index) {
                palette.subtle_pressed
            } else {
                palette.subtle_hover
            };
            painter.fill(self.item(index), radius, color);
        }
        let (start, end) = (self.start.value(now), self.end.value(now));
        let row = self.item(0);
        let highlight = Area::new(self.left + start, row.top, end - start, row.height);
        let tint = if palette.light { TINT.0 } else { TINT.1 };
        painter.fill(highlight, radius, faded(palette.accent, tint));

        for (index, item) in self.items.iter().enumerate() {
            let area = self.item(index);
            let lit = self.lit[index].value(now);
            let content = if self.named {
                Self::ICON + NAME_GAP + item.name.height
            } else {
                Self::ICON
            };
            let top = area.top + (area.height - content) / 2.0;
            let origin = (area.left + (area.width - Self::ICON) / 2.0, top);
            let ink = mix(palette.text, palette.accent, lit);
            painter.figure(&item.outlined, origin, faded(ink, 1.0 - lit));
            painter.figure(&item.filled, origin, faded(ink, lit));
            if self.named {
                let name = &item.name;
                let left = area.left + (area.width - name.width) / 2.0;
                let middle = top + Self::ICON + NAME_GAP + name.height / 2.0;
                let ink = mix(palette.text_secondary, palette.accent, lit);
                painter.text(name, left, middle, ink);
            }
        }
    }

    fn is_animating(&self, now: Instant) -> bool {
        self.start.is_running(now)
            || self.end.is_running(now)
            || self.shown.is_running(now)
            || self.lit.iter().any(|lit| lit.is_running(now))
    }
}
