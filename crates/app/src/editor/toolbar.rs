//! The editor's controls: saving and the capture's folder in the title bar, and two
//! capsules floating at the bottom, centered: the tools' dock and, above it, the
//! chosen tool's options.

use std::{mem, time::Instant};

use kata_editor::{Shape, ZOOMS};
use kata_ui::{
    Graphics,
    access::Element,
    controls::{self, Button, Control, Dock, HEIGHT, IconButton, Item, Slider, Toggle},
    figure::Figure,
    motion::{self, Animation, Easing},
    paint::{Area, Fonts, Painter, Point},
    window,
};
use windows::core::Result;

use super::{TITLE_BAR, icons};
use crate::{settings::EditorSettings, strings::strings};

/// Between the window's edges and the controls, and between related controls.
const EDGE: f32 = 12.0;
/// Between the last control and the caption buttons.
const END: f32 = 16.0;
/// Between the window's bottom edge and the dock.
const BOTTOM: f32 = 12.0;
/// Between the dock and the options above it.
const STACK: f32 = 8.0;
/// Between the options and the capture, at the least.
const ABOVE: f32 = 16.0;
/// The options' height, and the room inside them at each end.
const OPTIONS: f32 = 40.0;
const OPTIONS_PADDING: f32 = 16.0;
/// The icon that leads each setting, and the room between it and the setting.
const SETTING_ICON: f32 = 16.0;
const ICON_GAP: f32 = 8.0;
/// Between two settings.
const GAP: f32 = 20.0;

/// What dragging over the capture draws, in the dock's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Blur,
    Focus,
}

impl Tool {
    const ALL: [Self; 2] = [Self::Blur, Self::Focus];

    /// The tool that draws `shape`.
    pub fn of(shape: Shape) -> Self {
        match shape {
            Shape::Blur(_) => Self::Blur,
            Shape::Focus => Self::Focus,
        }
    }

    /// The tool at `index` in the dock.
    fn at(index: usize) -> Self {
        Self::ALL[index.min(Self::ALL.len() - 1)]
    }

    /// The settings it shows in the options, in order.
    fn options(self) -> &'static [Part] {
        match self {
            Self::Blur => &[Part::Blur],
            Self::Focus => &[Part::Zoom],
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
    Folder,
}

/// A slider among the options, led by an icon that says what it changes.
struct Setting {
    icon: Figure,
    slider: Slider,
    /// The icon's left edge, and the options' middle.
    left: f32,
    middle: f32,
}

impl Setting {
    fn new(icon: Figure, slider: Slider) -> Self {
        Self {
            icon,
            slider,
            left: 0.0,
            middle: 0.0,
        }
    }

    fn width(&self) -> f32 {
        SETTING_ICON + ICON_GAP + self.slider.width()
    }

    fn place(&mut self, left: f32, middle: f32) {
        (self.left, self.middle) = (left, middle);
        let slider = left + SETTING_ICON + ICON_GAP;
        self.slider.place(slider, middle - HEIGHT / 2.0);
    }

    fn draw(&self, painter: &Painter, now: Instant) {
        let top = self.middle - SETTING_ICON / 2.0;
        let ink = painter.palette.text_secondary;
        painter.figure(&self.icon, (self.left, top), ink);
        self.slider.draw(painter, now);
    }
}

pub struct Toolbar {
    tools: Dock,
    blur: Setting,
    zoom: Setting,
    save: Button,
    autosave: Toggle,
    folder: IconButton,
    /// The part on show with the keyboard focus.
    focus: Option<Part>,
    /// The window's size, in DIPs.
    window: (f32, f32),
    /// The tool whose options show, and the one whose options fade out for them.
    showing: Tool,
    leaving: Tool,
    /// From 0 as the options change to 1 once the new ones show in full.
    fade: Animation,
    /// The options' width, which glides from one tool's to the next.
    width: Animation,
}

impl Toolbar {
    /// With the tools' names under their icons when `named`.
    pub fn new(
        graphics: &Graphics,
        fonts: &Fonts,
        settings: &EditorSettings,
        named: bool,
    ) -> Result<Self> {
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
        let item = |(outlined, filled), name| -> Result<Item> {
            Ok(Item {
                outlined,
                filled,
                name: fonts.caption(name)?,
            })
        };
        let tools = vec![
            item(icons::blur(graphics, Dock::ICON)?, strings.blur)?,
            item(icons::focus(graphics, Dock::ICON)?, strings.focus)?,
        ];
        let (droplet, _) = icons::blur(graphics, SETTING_ICON)?;
        // Shown only while autosave is off, and only useful once there's a change.
        let mut save = Button::new(fonts.label(strings.save)?, !settings.autosave);
        save.enable(false);
        let tool = Tool::at(settings.tool);
        let mut toolbar = Self {
            tools: Dock::new(tools, tool as usize, named),
            blur: Setting::new(
                droplet,
                Slider::new(strings.blur, blur_levels, settings.blur_level),
            ),
            zoom: Setting::new(
                icons::zoom(graphics, SETTING_ICON)?,
                Slider::new(strings.focus, zooms, settings.zoom_level),
            ),
            save,
            autosave: Toggle::new(fonts.label(strings.autosave)?, settings.autosave),
            folder: IconButton::new(fonts.icon('\u{E838}', 16.0)?, strings.open_file_location),
            focus: None,
            window: (0.0, 0.0),
            showing: tool,
            leaving: tool,
            fade: Animation::idle(1.0),
            width: Animation::idle(0.0),
        };
        toolbar.width = Animation::idle(toolbar.options_width(tool));
        Ok(toolbar)
    }

    /// What the toolbar is set to, worth remembering for the next capture.
    pub fn settings(&self) -> EditorSettings {
        EditorSettings {
            tool: self.tool() as usize,
            blur_level: self.blur.slider.value(),
            zoom_level: self.zoom.slider.value(),
            autosave: self.autosave.is_on(),
        }
    }

    /// The narrowest the window fits the toolbar in, in DIPs: the title bar's
    /// controls beside the caption buttons, and the widest capsule at the bottom.
    pub fn min_width(&self) -> f32 {
        let title = EDGE
            + self.save.width()
            + EDGE
            + self.autosave.width()
            + EDGE
            + IconButton::WIDTH
            + END
            + window::CAPTION_BUTTONS;
        let options = Tool::ALL
            .map(|tool| self.options_width(tool))
            .into_iter()
            .fold(0.0, f32::max);
        title.max(self.tools.width().max(options) + 2.0 * EDGE)
    }

    /// The room the capsules take at the bottom of the window, in DIPs.
    pub fn bottom(&self) -> f32 {
        BOTTOM + self.tools.height() + STACK + OPTIONS + ABOVE
    }

    /// Lays the controls out in a window of `window` DIPs: along the title bar,
    /// before the caption buttons that start at `caption_buttons`, and centered at
    /// the bottom.
    pub fn layout(&mut self, window: (f32, f32), caption_buttons: f32) {
        self.window = window;
        let top = |height: f32| (TITLE_BAR - height) / 2.0;
        let folder = caption_buttons - END - IconButton::WIDTH;
        self.folder.place(folder, top(IconButton::HEIGHT));
        let toggle = folder - EDGE - self.autosave.width();
        self.autosave.place(toggle, top(HEIGHT));
        self.save
            .place(toggle - EDGE - self.save.width(), top(HEIGHT));

        let (width, height) = window;
        let dock = (width - self.tools.width()) / 2.0;
        let bounds = Area::new(0.0, TITLE_BAR, width, height - TITLE_BAR);
        self.tools
            .place(dock, height - BOTTOM - self.tools.height(), bounds);
        let middle = self.options_top() + OPTIONS / 2.0;
        for tool in Tool::ALL {
            let mut left = self.options_area(tool).left + OPTIONS_PADDING;
            for &part in tool.options() {
                if let Some(setting) = self.setting_mut(part) {
                    setting.place(left, middle);
                    left += setting.width() + GAP;
                }
            }
        }
    }

    pub fn part_at(&self, point: Point) -> Option<Part> {
        self.parts()
            .into_iter()
            .find(|&part| self.get(part).contains(point))
    }

    /// Whether `point` is on the toolbar: on a control, or between the options.
    pub fn covers(&self, point: Point) -> bool {
        self.part_at(point).is_some() || self.options_area(self.tool()).contains(point)
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
            Part::Blur => &mut self.blur.slider,
            Part::Zoom => &mut self.zoom.slider,
            Part::Save => &mut self.save,
            Part::Autosave => &mut self.autosave,
            Part::Folder => &mut self.folder,
        }
    }

    /// Returns whether the toolbar's look changes.
    pub fn hover(&mut self, point: Option<Point>) -> bool {
        let parts = self.parts();
        // The other tools' settings let go of the pointer.
        for tool in Tool::ALL {
            for part in tool.options() {
                if !parts.contains(part) {
                    self.control(*part).hover(None);
                }
            }
        }
        // Every control hears about the pointer, even after one changed.
        let mut changed = false;
        for part in parts {
            changed |= self.control(part).hover(point);
        }
        changed
    }

    /// Whether resting the pointer where it is would show a tool's name.
    pub fn awaits_rest(&self) -> bool {
        self.tools.awaits_rest()
    }

    /// The pointer rested at `point`. Returns whether that shows a tool's name.
    pub fn rest(&mut self, point: Point) -> bool {
        self.tools.rest(point)
    }

    /// Moves the keyboard focus to the next control, or the previous one if `back`.
    pub fn tab(&mut self, back: bool) {
        let parts = self.parts();
        let focus = self
            .focus
            .and_then(|focus| parts.iter().position(|&part| part == focus));
        let focusable = |index: usize| self.get(parts[index]).is_focusable();
        self.focus = controls::tab(parts.len(), focus, back, focusable).map(|index| parts[index]);
    }

    /// The control with the keyboard focus.
    pub fn focused(&self) -> Option<Part> {
        self.focus
            .filter(|&part| self.parts().contains(&part) && self.get(part).is_focusable())
    }

    pub fn unfocus(&mut self) {
        self.focus = None;
    }

    pub fn draw(&self, painter: &Painter, now: Instant) {
        self.draw_options(painter, now);
        for part in self.parts() {
            if self.setting(part).is_none() {
                self.get(part).draw(painter, now);
            }
        }
        if let Some(part) = self.focused() {
            let (area, radius) = self.get(part).focus_area();
            painter.focus(area, radius);
        }
        self.tools.draw_tip(painter, self.options_top(), now);
    }

    pub fn is_animating(&self, now: Instant) -> bool {
        self.fade.is_running(now)
            || self.width.is_running(now)
            || self
                .parts()
                .into_iter()
                .any(|part| self.get(part).is_animating(now))
    }

    pub fn tool(&self) -> Tool {
        Tool::at(self.tools.selected())
    }

    pub fn select_tool(&mut self, tool: Tool) {
        self.tools.select(tool as usize);
        self.follow_tool();
    }

    /// Shows the chosen tool's options, after the dock changed the tool.
    pub fn follow_tool(&mut self) {
        let tool = self.tool();
        if tool == self.showing {
            return;
        }
        self.leaving = mem::replace(&mut self.showing, tool);
        self.fade = Animation::new(0.0, 1.0, motion::FAST, Easing::DECELERATE);
        let width = self.options_width(tool);
        self.width = self
            .width
            .retarget(width, motion::NORMAL, Easing::POINT_TO_POINT);
    }

    pub fn blur_level(&self) -> usize {
        self.blur.slider.value()
    }

    pub fn set_blur_level(&mut self, level: usize) {
        self.blur.slider.set_value(level);
    }

    /// An index into `ZOOMS`.
    pub fn zoom_level(&self) -> usize {
        self.zoom.slider.value()
    }

    pub fn set_zoom_level(&mut self, level: usize) {
        self.zoom.slider.set_value(level);
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

    /// The options' capsule, as wide as it glides to, with the settings crossfading
    /// inside it as the tool changes.
    fn draw_options(&self, painter: &Painter, now: Instant) {
        let width = self.width.value(now);
        let left = (self.window.0 - width) / 2.0;
        let area = painter.align(Area::new(left, self.options_top(), width, OPTIONS));
        painter.flyout(area, OPTIONS / 2.0);
        let fade = self.fade.value(now);
        painter.clip(area, || {
            painter.layer(1.0 - fade, 0.0, || {
                self.draw_settings(painter, self.leaving, now)
            });
            painter.layer(fade, 0.0, || self.draw_settings(painter, self.showing, now));
        });
    }

    fn draw_settings(&self, painter: &Painter, tool: Tool, now: Instant) {
        for &part in tool.options() {
            if let Some(setting) = self.setting(part) {
                setting.draw(painter, now);
            }
        }
    }

    fn options_top(&self) -> f32 {
        self.window.1 - BOTTOM - self.tools.height() - STACK - OPTIONS
    }

    fn options_width(&self, tool: Tool) -> f32 {
        let settings = tool.options().iter().filter_map(|&part| self.setting(part));
        let (count, width) = settings.fold((0, 0.0), |(count, width), setting| {
            (count + 1, width + setting.width())
        });
        width + (count.max(1) - 1) as f32 * GAP + 2.0 * OPTIONS_PADDING
    }

    /// Where `tool`'s options show, once the capsule has glided to them.
    fn options_area(&self, tool: Tool) -> Area {
        let width = self.options_width(tool);
        let left = (self.window.0 - width) / 2.0;
        Area::new(left, self.options_top(), width, OPTIONS)
    }

    /// The parts on show: the dock, the chosen tool's settings and the title bar's.
    fn parts(&self) -> Vec<Part> {
        let mut parts = vec![Part::Tools];
        parts.extend_from_slice(self.tool().options());
        parts.extend([Part::Save, Part::Autosave, Part::Folder]);
        parts
    }

    fn get(&self, part: Part) -> &dyn Control {
        match part {
            Part::Tools => &self.tools,
            Part::Blur => &self.blur.slider,
            Part::Zoom => &self.zoom.slider,
            Part::Save => &self.save,
            Part::Autosave => &self.autosave,
            Part::Folder => &self.folder,
        }
    }

    fn setting(&self, part: Part) -> Option<&Setting> {
        match part {
            Part::Blur => Some(&self.blur),
            Part::Zoom => Some(&self.zoom),
            _ => None,
        }
    }

    fn setting_mut(&mut self, part: Part) -> Option<&mut Setting> {
        match part {
            Part::Blur => Some(&mut self.blur),
            Part::Zoom => Some(&mut self.zoom),
            _ => None,
        }
    }
}
