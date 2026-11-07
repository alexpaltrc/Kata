//! The editor's controls: the capture's name, saving and its folder in the title bar,
//! and two capsules floating at the bottom, centered: the tools' dock and, above it,
//! the chosen tool's options, with a hint above them while the capture is zoomed.

use std::{iter, mem, time::Instant};

use kata_editor::{Form, Frame, Shape, ZOOMS};
use kata_ui::{
    Graphics,
    access::Element,
    controls::{self, Control, Dock, HEIGHT, IconButton, Item, Slider, tip},
    figure::Figure,
    motion::{self, Animation, Easing},
    paint::{Area, Fonts, Label, Painter, Point},
    theme::faded,
    window,
};
use windows::core::Result;

use super::{
    TITLE_BAR, hints, icons,
    name::{self, Name},
};
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
/// The zoom hint's height, its keycap's, the room after the key, and how far it
/// rises and grows as it shows.
const HINT: f32 = 32.0;
const KEYCAP: f32 = 22.0;
const KEY_PADDING: f32 = 8.0;
/// The line between what the space bar does and the crop button: how tall, and the
/// room before it and after it, where the button brings its own.
const DIVIDER: f32 = 16.0;
const BEFORE_DIVIDER: f32 = 12.0;
const AFTER_DIVIDER: f32 = 4.0;
const RISE: f32 = 8.0;
const GROW: f32 = 0.1;
/// The pill the folder button opens into, to name the folder the capture saved
/// itself in: between the name and the button, and before the name; its corners;
/// how far the name rises as it shows and goes; and the glint's band of light
/// across it.
const PILL_GAP: f32 = 8.0;
const PILL_PADDING: f32 = 12.0;
const PILL_RADIUS: f32 = 4.0;
const PILL_RISE: f32 = 4.0;
const PILL_BAND: f32 = 64.0;
/// How far the tip that says the capture saved itself comes out from under the
/// folder button.
const TIP_RISE: f32 = 4.0;

/// That the capture saved itself: the note that says so, the folder's name, and
/// the clock the folder button says it on.
struct Saved {
    note: Label,
    folder: Label,
    clock: Animation,
}

/// What dragging or clicking on the capture adds, in the dock's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Blur,
    Focus,
    Steps,
    Shapes,
}

impl Tool {
    const ALL: [Self; 4] = [Self::Blur, Self::Focus, Self::Steps, Self::Shapes];

    /// The tool that adds `shape`.
    pub fn of(shape: Shape) -> Self {
        match shape {
            Shape::Blur(_) => Self::Blur,
            Shape::Focus => Self::Focus,
            Shape::Step(_) => Self::Steps,
            Shape::Figure(_) => Self::Shapes,
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
            Self::Focus => &[Part::Frame, Part::Zoom, Part::Around],
            Self::Steps => &[Part::StepSize],
            Self::Shapes => &[Part::Form],
        }
    }
}

/// The shapes' forms, in their picker's order.
const FORMS: [Form; 3] = [Form::Rectangle, Form::Ellipse, Form::Arrow];
/// The focus's frames, in their picker's order.
const FRAMES: [Frame; 3] = [Frame::Soft, Frame::Rectangle, Frame::Ellipse];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Tools,
    Blur,
    Zoom,
    Around,
    StepSize,
    Form,
    Frame,
    Crop,
    Name,
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
    around: Setting,
    step_size: Setting,
    forms: Dock,
    frames: Dock,
    name: Name,
    folder: IconButton,
    /// That the capture saved itself, which the folder button says the first few
    /// times.
    saved: Option<Saved>,
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
    /// The hint that the space bar resets the zoom: the key, what it does, and from 0,
    /// hidden, to 1, in full.
    space: Label,
    reset_zoom: Label,
    /// The button that crops the capture to the part in view, beside the hint.
    crop: IconButton,
    hint: Animation,
}

impl Toolbar {
    /// With the tools' names under their icons when `named`, and the capture's file
    /// called after `names`, as `Name::new` takes them.
    pub fn new(
        graphics: &Graphics,
        fonts: &Fonts,
        settings: &EditorSettings,
        named: bool,
        names: Option<(String, String)>,
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
        let around = iter::once(strings.no_blur)
            .chain(strings.blur_levels)
            .map(|name| fonts.label(name))
            .collect::<Result<_>>()?;
        let step_sizes = strings
            .step_sizes
            .iter()
            .map(|name| fonts.label(name))
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
            item(icons::steps(graphics, Dock::ICON)?, strings.steps)?,
            item(icons::shapes(graphics, Dock::ICON)?, strings.shapes)?,
        ];
        let forms = vec![
            item(icons::rectangle(graphics, Dock::ICON)?, strings.forms[0])?,
            item(icons::ellipse(graphics, Dock::ICON)?, strings.forms[1])?,
            item(icons::arrow(graphics, Dock::ICON)?, strings.forms[2])?,
        ];
        let frames = vec![
            item(icons::soft(graphics, Dock::ICON)?, strings.frames[0])?,
            item(icons::rectangle(graphics, Dock::ICON)?, strings.frames[1])?,
            item(icons::ellipse(graphics, Dock::ICON)?, strings.frames[2])?,
        ];
        let droplet = || icons::blur(graphics, SETTING_ICON).map(|(outlined, _)| outlined);
        let tool = Tool::at(settings.tool);
        let mut toolbar = Self {
            tools: Dock::new(tools, tool as usize, named),
            blur: Setting::new(
                droplet()?,
                Slider::new(strings.blur, blur_levels, settings.blur_level),
            ),
            zoom: Setting::new(
                icons::zoom(graphics, SETTING_ICON)?,
                Slider::new(strings.focus, zooms, settings.zoom_level),
            ),
            around: Setting::new(
                droplet()?,
                Slider::new(strings.blur_around, around, settings.around),
            ),
            step_size: Setting::new(
                icons::steps(graphics, SETTING_ICON)?.0,
                Slider::new(strings.step_size, step_sizes, settings.step_size),
            ),
            forms: Dock::compact(forms, settings.form),
            frames: Dock::compact(frames, settings.frame),
            name: Name::new(fonts, names)?,
            folder: IconButton::new(fonts.icon('\u{E838}', 16.0)?, strings.open_file_location),
            saved: None,
            focus: None,
            window: (0.0, 0.0),
            showing: tool,
            leaving: tool,
            fade: Animation::idle(1.0),
            width: Animation::idle(0.0),
            space: fonts.caption(strings.space)?,
            reset_zoom: fonts.caption(strings.reset_zoom)?,
            crop: IconButton::labeled(fonts.icon('\u{E8C6}', 14.0)?, fonts.caption(strings.crop)?),
            hint: Animation::idle(0.0),
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
            around: self.around.slider.value(),
            step_size: self.step_size.slider.value(),
            form: self.forms.selected(),
            frame: self.frames.selected(),
        }
    }

    /// The narrowest the window fits the toolbar in, in DIPs: the title bar's
    /// controls beside the caption buttons, and the widest capsule at the bottom.
    pub fn min_width(&self) -> f32 {
        let title = EDGE + name::MIN + EDGE + IconButton::WIDTH + END + window::CAPTION_BUTTONS;
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
        let center = (window.0 / 2.0, TITLE_BAR / 2.0);
        self.name.place(EDGE, folder - EDGE, center);

        let (width, height) = window;
        let dock = (width - self.tools.width()) / 2.0;
        let bounds = Area::new(0.0, TITLE_BAR, width, height - TITLE_BAR);
        self.tools
            .place(dock, height - BOTTOM - self.tools.height(), bounds);
        let middle = self.options_top() + OPTIONS / 2.0;
        for tool in Tool::ALL {
            // A picker leads, with its own room around it.
            let area = self.options_area(tool);
            let mut left = area.left;
            if let Some(picker) = self.picker_mut(tool) {
                picker.place(left, area.top, bounds);
                left += picker.width();
            }
            left += OPTIONS_PADDING;
            for &part in tool.options() {
                if let Some(setting) = self.setting_mut(part) {
                    setting.place(left, middle);
                    left += setting.width() + GAP;
                }
            }
        }
        // The crop button ends the hint, as round as the keycap that starts it.
        let hint = self.hint_area();
        let inset = (HINT - KEYCAP) / 2.0;
        let left = hint.right() - inset - self.crop.width();
        self.crop.round(left, hint.top + inset, KEYCAP);
    }

    pub fn part_at(&self, point: Point) -> Option<Part> {
        self.parts()
            .into_iter()
            .find(|&part| self.get(part).contains(point))
    }

    /// Whether `point` is on the toolbar: on a control, between the options or on the
    /// zoom hint.
    pub fn covers(&self, point: Point) -> bool {
        self.part_at(point).is_some()
            || self.options_area(self.tool()).contains(point)
            || (self.hint.target() > 0.0 && self.hint_area().contains(point))
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
            Part::Around => &mut self.around.slider,
            Part::StepSize => &mut self.step_size.slider,
            Part::Form => &mut self.forms,
            Part::Frame => &mut self.frames,
            Part::Crop => &mut self.crop,
            Part::Name => &mut self.name,
            Part::Folder => &mut self.folder,
        }
    }

    /// Returns whether the toolbar's look changes.
    pub fn hover(&mut self, point: Option<Point>) -> bool {
        let parts = self.parts();
        // The other tools' settings let go of the pointer, and the crop button too
        // once the hint goes.
        let parted = Tool::ALL.iter().flat_map(|tool| tool.options());
        for &part in parted.chain(&[Part::Crop]) {
            if !parts.contains(&part) {
                self.control(part).hover(None);
            }
        }
        // Every control hears about the pointer, even after one changed.
        let mut changed = false;
        for part in parts {
            changed |= self.control(part).hover(point);
        }
        changed
    }

    /// Whether resting the pointer where it is would show a tool's name, or a form's or
    /// a frame's.
    pub fn awaits_rest(&self) -> bool {
        self.tools.awaits_rest() || self.picker(self.tool()).is_some_and(Dock::awaits_rest)
    }

    /// The pointer rested at `point`. Returns whether that shows a tool's name, or a
    /// form's or a frame's.
    pub fn rest(&mut self, point: Point) -> bool {
        let tool = self.tool();
        self.tools.rest(point)
            || self
                .picker_mut(tool)
                .is_some_and(|picker| picker.rest(point))
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
        let options = self.tool().options();
        for part in self.parts() {
            match part {
                _ if options.contains(&part) => {}
                Part::Folder => self.draw_folder(painter, now),
                // It shows with the hint, which draws it.
                Part::Crop => {}
                _ => self.get(part).draw(painter, now),
            }
        }
        self.draw_hint(painter, now);
        if let Some(part) = self.focused() {
            let (area, radius) = self.get(part).focus_area();
            painter.focus(area, radius);
        }
        // The tools' names make room for the hint, above it.
        let tips = self.options_top() - (STACK + HINT) * self.hint.value(now);
        self.tools.draw_tip(painter, tips, now);
        self.forms.draw_tip(painter, tips, now);
        self.frames.draw_tip(painter, tips, now);
        self.draw_told(painter, now);
    }

    pub fn is_animating(&self, now: Instant) -> bool {
        self.fade.is_running(now)
            || self.width.is_running(now)
            || self.hint.is_running(now)
            || (self.saved.as_ref()).is_some_and(|saved| saved.clock.is_running(now))
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
        self.width = self.width.retarget(
            self.options_width(tool),
            motion::NORMAL,
            Easing::POINT_TO_POINT,
        );
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

    /// The blur level around the focus, if it blurs what's around.
    pub fn around(&self) -> Option<usize> {
        self.around.slider.value().checked_sub(1)
    }

    pub fn set_around(&mut self, level: Option<usize>) {
        self.around
            .slider
            .set_value(level.map_or(0, |level| level + 1));
    }

    /// An index into `STEP_SIZES`.
    pub fn step_size(&self) -> usize {
        self.step_size.slider.value()
    }

    pub fn set_step_size(&mut self, size: usize) {
        self.step_size.slider.set_value(size);
    }

    /// The form new shapes take.
    pub fn form(&self) -> Form {
        FORMS[self.forms.selected()]
    }

    pub fn set_form(&mut self, form: Form) {
        if let Some(index) = FORMS.iter().position(|&each| each == form) {
            self.forms.select(index);
        }
    }

    /// How a new focus is framed.
    pub fn frame(&self) -> Frame {
        FRAMES[self.frames.selected()]
    }

    pub fn set_frame(&mut self, frame: Frame) {
        if let Some(index) = FRAMES.iter().position(|&each| each == frame) {
            self.frames.select(index);
        }
    }

    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn name_mut(&mut self) -> &mut Name {
        &mut self.name
    }

    /// Says on `clock` that the capture saved itself, with `note`, and in `folder`:
    /// the button beats, a tip under it says so, and the button opens to name the
    /// folder.
    pub fn show_saved(&mut self, note: Label, folder: Label, clock: Animation) {
        self.saved = Some(Saved {
            note,
            folder,
            clock,
        });
    }

    /// The folder button, beating and opening into a pill that names the folder,
    /// room beside the name allowing.
    fn draw_folder(&self, painter: &Painter, now: Instant) {
        let Some(saved) = (self.saved.as_ref()).filter(|saved| saved.clock.is_running(now)) else {
            return self.folder.draw(painter, now);
        };
        let seconds = saved.clock.value(now);
        let button = self.folder.area();
        let reach = saved.folder.width + PILL_GAP;
        if reach <= button.left - EDGE - self.name.extent(now).right() {
            self.draw_pill(painter, &saved.folder, &hints::pill(seconds, reach));
        }
        let center = (button.left + button.width / 2.0, button.middle());
        painter.grown(1.0, hints::pop(seconds), center, || {
            self.folder.draw(painter, now);
        });
    }

    /// The pill the folder button opens into, as `pill` says, with the `folder`'s
    /// name ending by the button and rising into place and away.
    fn draw_pill(&self, painter: &Painter, folder: &Label, pill: &hints::Pill) {
        if pill.fill <= 0.0 {
            return;
        }
        let palette = painter.palette;
        let button = self.folder.area();
        let area = Area::new(
            button.left - pill.reach,
            button.top,
            button.width + pill.reach,
            button.height,
        );
        let area = painter.align(area);
        painter.fill(area, PILL_RADIUS, faded(palette.subtle_hover, pill.fill));
        if pill.shown <= 0.0 {
            return;
        }
        let left = button.left - folder.width - PILL_GAP + PILL_PADDING;
        let middle = button.middle() + PILL_RISE * pill.below;
        painter.clip(area, || {
            painter.text(folder, left, middle, faded(palette.text, pill.shown));
            if !palette.contrast
                && let Some(glint) = pill.glint
            {
                let x = left - PILL_BAND / 2.0 + (folder.width + PILL_BAND) * glint;
                painter.glint(&[(folder, left)], middle, x, PILL_BAND);
            }
        });
    }

    /// The tip that comes out from under the folder button to say the capture saved
    /// itself, and goes back in.
    fn draw_told(&self, painter: &Painter, now: Instant) {
        let Some(saved) = (self.saved.as_ref()).filter(|saved| saved.clock.is_running(now)) else {
            return;
        };
        let told = hints::told(saved.clock.value(now));
        if told <= 0.0 {
            return;
        }
        let bounds = Area::new(0.0, 0.0, self.window.0, self.window.1);
        painter.lowered(1.0, -TIP_RISE * (1.0 - told), || {
            tip::draw(painter, &saved.note, self.folder.area(), bounds, told);
        });
    }

    /// The options' capsule, as wide as it glides to, with the settings crossfading
    /// inside it as the tool changes.
    fn draw_options(&self, painter: &Painter, now: Instant) {
        let width = self.width.value(now);
        let left = (self.window.0 - width) / 2.0;
        let area = painter.align(Area::new(left, self.options_top(), width, OPTIONS));
        let fade = self.fade.value(now);
        painter.flyout(area, OPTIONS / 2.0);
        painter.clip(area, || {
            painter.layer(1.0 - fade, 0.0, || {
                self.draw_settings(painter, self.leaving, now)
            });
            painter.layer(fade, 0.0, || self.draw_settings(painter, self.showing, now));
        });
    }

    /// Shows the hint that the space bar resets the zoom while `zoomed`, and hides it
    /// otherwise.
    pub fn hint_zoom(&mut self, zoomed: bool) {
        let (to, easing) = if zoomed {
            (1.0, Easing::DECELERATE)
        } else {
            (0.0, Easing::ACCELERATE)
        };
        self.hint = self.hint.retarget(to, motion::NORMAL, easing);
    }

    /// Whether the crop button is hovered or has the keyboard focus, so the corners
    /// of what cropping keeps show.
    pub fn previews_crop(&self) -> bool {
        self.hint.target() > 0.0 && (self.crop.is_hovered() || self.focused() == Some(Part::Crop))
    }

    /// The zoom hint: a keycap and what the key does, then the crop button, in a pill
    /// that rises out of the options and grows into place as it shows, and sinks back
    /// into them as it hides.
    fn draw_hint(&self, painter: &Painter, now: Instant) {
        let shown = self.hint.value(now);
        let area = self.hint_area();
        let base = (area.left + area.width / 2.0, area.bottom());
        painter.lowered(1.0, RISE * (1.0 - shown), || {
            painter.grown(shown, 1.0 - GROW * (1.0 - shown), base, || {
                let palette = painter.palette;
                painter.flyout(area, HINT / 2.0);
                let inset = (HINT - KEYCAP) / 2.0;
                let key = painter.align(Area::new(
                    area.left + inset,
                    area.top + inset,
                    self.keycap_width(),
                    KEYCAP,
                ));
                painter.fill(key, KEYCAP / 2.0, palette.button);
                painter.stroke(key, KEYCAP / 2.0, painter.border(), palette.control_stroke);
                let left = key.left + (key.width - self.space.width) / 2.0;
                painter.text(&self.space, left, key.middle(), palette.text);
                let left = key.right() + KEY_PADDING;
                let ink = palette.text_secondary;
                painter.text(&self.reset_zoom, left, area.middle(), ink);
                let left = painter.snap(left + self.reset_zoom.width + BEFORE_DIVIDER);
                let divider = Area::new(
                    left,
                    area.middle() - DIVIDER / 2.0,
                    painter.pixel(),
                    DIVIDER,
                );
                painter.fill(painter.align(divider), 0.0, palette.control_stroke);
                self.crop.draw(painter, now);
            });
        });
    }

    /// Where the zoom hint shows in full, centered above the options.
    fn hint_area(&self) -> Area {
        let inset = (HINT - KEYCAP) / 2.0;
        let hint = inset + self.keycap_width() + KEY_PADDING + self.reset_zoom.width;
        let divider = BEFORE_DIVIDER + 1.0 + AFTER_DIVIDER;
        let width = hint + divider + self.crop.width() + inset;
        let left = (self.window.0 - width) / 2.0;
        Area::new(left, self.options_top() - STACK - HINT, width, HINT)
    }

    /// The zoom hint's keycap: as wide as its key, but never narrower than round.
    fn keycap_width(&self) -> f32 {
        (self.space.width + 2.0 * KEY_PADDING).max(KEYCAP)
    }

    fn draw_settings(&self, painter: &Painter, tool: Tool, now: Instant) {
        for &part in tool.options() {
            match self.setting(part) {
                Some(setting) => setting.draw(painter, now),
                None => self.get(part).draw(painter, now),
            }
        }
    }

    fn dock_top(&self) -> f32 {
        self.window.1 - BOTTOM - self.tools.height()
    }

    fn options_top(&self) -> f32 {
        self.dock_top() - STACK - OPTIONS
    }

    fn options_width(&self, tool: Tool) -> f32 {
        // A picker brings its own room around it.
        let picker = self.picker(tool).map_or(0.0, Dock::width);
        let settings: Vec<&Setting> = (tool.options().iter())
            .filter_map(|&part| self.setting(part))
            .collect();
        if settings.is_empty() {
            return picker;
        }
        let widths: f32 = settings.iter().map(|setting| setting.width()).sum();
        picker + widths + (settings.len() - 1) as f32 * GAP + 2.0 * OPTIONS_PADDING
    }

    /// The icons `tool`'s options lead with, to pick what it adds.
    fn picker(&self, tool: Tool) -> Option<&Dock> {
        match tool {
            Tool::Focus => Some(&self.frames),
            Tool::Shapes => Some(&self.forms),
            Tool::Blur | Tool::Steps => None,
        }
    }

    fn picker_mut(&mut self, tool: Tool) -> Option<&mut Dock> {
        match tool {
            Tool::Focus => Some(&mut self.frames),
            Tool::Shapes => Some(&mut self.forms),
            Tool::Blur | Tool::Steps => None,
        }
    }

    /// Where `tool`'s options show, once the capsule has glided to them.
    fn options_area(&self, tool: Tool) -> Area {
        let width = self.options_width(tool);
        let left = (self.window.0 - width) / 2.0;
        Area::new(left, self.options_top(), width, OPTIONS)
    }

    /// The parts on show: the dock, the chosen tool's settings, the crop button while
    /// the capture is zoomed, and the title bar's.
    fn parts(&self) -> Vec<Part> {
        let mut parts = vec![Part::Tools];
        parts.extend_from_slice(self.tool().options());
        if self.hint.target() > 0.0 {
            parts.push(Part::Crop);
        }
        parts.extend([Part::Name, Part::Folder]);
        parts
    }

    fn get(&self, part: Part) -> &dyn Control {
        match part {
            Part::Tools => &self.tools,
            Part::Blur => &self.blur.slider,
            Part::Zoom => &self.zoom.slider,
            Part::Around => &self.around.slider,
            Part::StepSize => &self.step_size.slider,
            Part::Form => &self.forms,
            Part::Frame => &self.frames,
            Part::Crop => &self.crop,
            Part::Name => &self.name,
            Part::Folder => &self.folder,
        }
    }

    fn setting(&self, part: Part) -> Option<&Setting> {
        match part {
            Part::Blur => Some(&self.blur),
            Part::Zoom => Some(&self.zoom),
            Part::Around => Some(&self.around),
            Part::StepSize => Some(&self.step_size),
            _ => None,
        }
    }

    fn setting_mut(&mut self, part: Part) -> Option<&mut Setting> {
        match part {
            Part::Blur => Some(&mut self.blur),
            Part::Zoom => Some(&mut self.zoom),
            Part::Around => Some(&mut self.around),
            Part::StepSize => Some(&mut self.step_size),
            _ => None,
        }
    }
}
