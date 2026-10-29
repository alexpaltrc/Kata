//! The controls in the editor's title bar: the tools, the active tool's slider and
//! saving.

use std::time::Instant;

use kata_editor::{Shape, ZOOMS};
use kata_ui::{
    access::Element,
    controls::{self, Button, Control, HEIGHT, SelectorBar, Slider, Toggle},
    paint::{Fonts, Painter, Point},
};
use windows::core::Result;

use super::TITLE_BAR;
use crate::{settings::EditorSettings, strings::strings};

/// Between the window's edges and the controls, and between related controls.
const EDGE: f32 = 12.0;
/// Between groups of controls.
const GAP: f32 = 20.0;
/// Between the last control and the caption buttons.
const END: f32 = 16.0;

/// What dragging over the capture draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Blur,
    Focus,
}

impl Tool {
    /// The tool that draws `shape`.
    pub fn of(shape: Shape) -> Self {
        match shape {
            Shape::Blur(_) => Self::Blur,
            Shape::Focus => Self::Focus,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Tools,
    Blur,
    Zoom,
    Save,
    Autosave,
}

pub struct Toolbar {
    tools: SelectorBar,
    blur: Slider,
    zoom: Slider,
    save: Button,
    autosave: Toggle,
    /// Which of the parts on show has the keyboard focus, by its place among them.
    focus: Option<usize>,
}

impl Toolbar {
    pub fn new(fonts: &Fonts, settings: &EditorSettings) -> Result<Self> {
        let strings = strings();
        let blur_levels = strings
            .blur_levels
            .iter()
            .map(|name| fonts.label(name))
            .collect::<Result<_>>()?;
        let zooms = ZOOMS
            .iter()
            .map(|&zoom| fonts.label(&strings.percent(zoom)))
            .collect::<Result<_>>()?;
        let tools = vec![fonts.label(strings.blur)?, fonts.label(strings.focus)?];
        // Shown only while autosave is off, and only useful once there's a change.
        let mut save = Button::new(fonts.label(strings.save)?, !settings.autosave);
        save.enable(false);
        Ok(Self {
            tools: SelectorBar::new(tools, settings.tool),
            blur: Slider::new(strings.blur, blur_levels, settings.blur_level),
            zoom: Slider::new(strings.focus, zooms, settings.zoom_level),
            save,
            autosave: Toggle::new(fonts.label(strings.autosave)?, settings.autosave),
            focus: None,
        })
    }

    /// What the toolbar is set to, worth remembering for the next capture.
    pub fn settings(&self) -> EditorSettings {
        EditorSettings {
            tool: self.tool() as usize,
            blur_level: self.blur.value(),
            zoom_level: self.zoom.value(),
            autosave: self.autosave.is_on(),
        }
    }

    /// The narrowest the toolbar fits in, in DIPs, the caption buttons aside.
    pub fn min_width(&self) -> f32 {
        let slider = self.blur.width().max(self.zoom.width());
        EDGE + self.tools.width()
            + GAP
            + slider
            + GAP
            + self.save.width()
            + EDGE
            + self.autosave.width()
            + END
    }

    /// Lays the controls out along the title bar, before the caption buttons that
    /// start at `caption_buttons`.
    pub fn layout(&mut self, caption_buttons: f32) {
        let top = |height: f32| (TITLE_BAR - height) / 2.0;
        self.tools.place(EDGE, top(SelectorBar::HEIGHT));
        let slider = EDGE + self.tools.width() + GAP;
        self.blur.place(slider, top(HEIGHT));
        self.zoom.place(slider, top(HEIGHT));
        let toggle = caption_buttons - END - self.autosave.width();
        self.autosave.place(toggle, top(HEIGHT));
        self.save
            .place(toggle - EDGE - self.save.width(), top(HEIGHT));
    }

    pub fn part_at(&self, point: Point) -> Option<Part> {
        self.parts()
            .into_iter()
            .find(|&part| self.get(part).contains(point))
    }

    /// The controls on show, as assistive technologies know them, each numbered as
    /// its part.
    pub fn describe(&self) -> Vec<Element> {
        let mut elements = Vec::new();
        for part in self.parts() {
            self.get(part).describe(part as usize, &mut elements);
        }
        elements
    }

    /// The part on show with the number `describe` gave it.
    pub fn part(&self, number: usize) -> Option<Part> {
        self.parts()
            .into_iter()
            .find(|&part| part as usize == number)
    }

    pub fn control(&mut self, part: Part) -> &mut dyn Control {
        match part {
            Part::Tools => &mut self.tools,
            Part::Blur => &mut self.blur,
            Part::Zoom => &mut self.zoom,
            Part::Save => &mut self.save,
            Part::Autosave => &mut self.autosave,
        }
    }

    /// Returns whether the toolbar's look changes.
    pub fn hover(&mut self, point: Option<Point>) -> bool {
        let hidden = match self.tool() {
            Tool::Blur => Part::Zoom,
            Tool::Focus => Part::Blur,
        };
        self.control(hidden).hover(None);
        // Every control hears about the pointer, even after one changed.
        let mut changed = false;
        for part in self.parts() {
            changed |= self.control(part).hover(point);
        }
        changed
    }

    /// Moves the keyboard focus to the next control, or the previous one if `back`.
    pub fn tab(&mut self, back: bool) {
        let parts = self.parts();
        let focusable = |index: usize| self.get(parts[index]).is_focusable();
        self.focus = controls::tab(parts.len(), self.focus, back, focusable);
    }

    /// The control with the keyboard focus.
    pub fn focused(&self) -> Option<Part> {
        let part = self.parts()[self.focus?];
        self.get(part).is_focusable().then_some(part)
    }

    pub fn unfocus(&mut self) {
        self.focus = None;
    }

    pub fn draw(&self, painter: &Painter, now: Instant) {
        for part in self.parts() {
            self.get(part).draw(painter, now);
        }
        if let Some(part) = self.focused() {
            let (area, radius) = self.get(part).focus_area();
            painter.focus(area, radius);
        }
    }

    pub fn is_animating(&self, now: Instant) -> bool {
        self.parts()
            .into_iter()
            .any(|part| self.get(part).is_animating(now))
    }

    pub fn tool(&self) -> Tool {
        match self.tools.selected() {
            0 => Tool::Blur,
            _ => Tool::Focus,
        }
    }

    pub fn select_tool(&mut self, tool: Tool) {
        self.tools.select(tool as usize);
    }

    pub fn blur_level(&self) -> usize {
        self.blur.value()
    }

    pub fn set_blur_level(&mut self, level: usize) {
        self.blur.set_value(level);
    }

    /// An index into `ZOOMS`.
    pub fn zoom_level(&self) -> usize {
        self.zoom.value()
    }

    pub fn set_zoom_level(&mut self, level: usize) {
        self.zoom.set_value(level);
    }

    pub fn autosave(&self) -> bool {
        self.autosave.is_on()
    }

    pub fn show_save(&mut self, shown: bool) {
        self.save.show(shown);
    }

    pub fn enable_save(&mut self, enabled: bool) {
        self.save.enable(enabled);
    }

    /// The parts on show: one slider at a time, the active tool's.
    fn parts(&self) -> [Part; 4] {
        let slider = match self.tool() {
            Tool::Blur => Part::Blur,
            Tool::Focus => Part::Zoom,
        };
        [Part::Tools, slider, Part::Save, Part::Autosave]
    }

    fn get(&self, part: Part) -> &dyn Control {
        match part {
            Part::Tools => &self.tools,
            Part::Blur => &self.blur,
            Part::Zoom => &self.zoom,
            Part::Save => &self.save,
            Part::Autosave => &self.autosave,
        }
    }
}
