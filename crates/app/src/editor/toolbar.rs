//! The editor's controls: the capture's name in the title bar and, floating at the
//! bottom, the tools' dock, centered, with the folder button on an island beside it.
//! A tool's options come out of it while the pointer is on it or on them: what it
//! adds, round it as the top of a wheel, and its settings in a capsule over that.

use std::time::Instant;

use kata_editor::{Form, Shape};
use kata_ui::{
    Graphics,
    access::Element,
    controls::{self, Caption, Control, Dock, HEIGHT, IconButton, Item, Slider, tip},
    figure::Figure,
    logo,
    motion::{self, Animation, Easing},
    paint::{Area, Fonts, Label, Painter, Point},
    theme::faded,
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
/// Between the dock and the capture, at the least.
const ABOVE: f32 = 16.0;
/// The options' height, and the room inside them at each end.
const OPTIONS: f32 = 40.0;
const OPTIONS_PADDING: f32 = 16.0;
/// How big a tool's options are as they start out of it.
const SEED: f32 = 0.6;
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
const PILL_RISE: f32 = 4.0;
const PILL_BAND: f32 = 64.0;
/// The island beside the dock, for the folder button: how wide, and how far from the
/// dock.
const ISLAND: f32 = 48.0;
const ISLAND_GAP: f32 = 8.0;
/// The check on the folder button: its size, where its middle is from the button's,
/// the room the island keeps clear around it, and its stroke.
const CHECK: (f32, f32) = (8.0, 6.5);
const CHECK_AT: (f32, f32) = (6.5, 5.5);
const CHECK_ROOM: f32 = 2.5;
const CHECK_STROKE: f32 = 1.75;
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
    Shapes,
    Steps,
    Focus,
    Blur,
}

impl Tool {
    const ALL: [Self; 4] = [Self::Shapes, Self::Steps, Self::Focus, Self::Blur];

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
            Self::Blur => &[],
            Self::Focus => &[],
            Self::Steps => &[Part::StepSize],
            Self::Shapes => &[Part::Form],
        }
    }
}

/// The shapes' forms, in their picker's order.
const FORMS: [Form; 3] = [Form::Rectangle, Form::Ellipse, Form::Arrow];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Tools,
    StepSize,
    Form,
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
    step_size: Setting,
    forms: Dock,
    name: Name,
    folder: IconButton,
    caption: Caption,
    /// That the capture saved itself, which the folder button says the first few
    /// times.
    saved: Option<Saved>,
    /// How far the folder button's check is into drawing itself, in seconds.
    check: Animation,
    /// The part on show with the keyboard focus.
    focus: Option<Part>,
    /// The window's size, in DIPs.
    window: (f32, f32),
    /// The tool the pointer is on, or on whose options it is.
    pointed: Option<Tool>,
    /// The tool whose options are out, above it: the one the pointer is on, or the
    /// chosen one while the keyboard is on the dock or on its options.
    open: Option<Tool>,
    /// The tool whose options show, or showed last, and how far out of it they
    /// are, from 0, away, to 1.
    shown: Tool,
    out: Animation,
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
            item(icons::shapes(graphics, Dock::ICON)?, strings.shapes)?,
            item(icons::steps(graphics, Dock::ICON)?, strings.steps)?,
            item(icons::focus(graphics, Dock::ICON)?, strings.focus)?,
            item(icons::blur(graphics, Dock::ICON)?, strings.blur)?,
        ];
        let forms = vec![
            item(icons::rectangle(graphics, Dock::ICON)?, strings.forms[0])?,
            item(icons::ellipse(graphics, Dock::ICON)?, strings.forms[1])?,
            item(icons::arrow(graphics, Dock::ICON)?, strings.forms[2])?,
        ];
        let tool = Tool::at(settings.tool);
        let toolbar = Self {
            tools: Dock::new(tools, tool as usize, named),
            step_size: Setting::new(
                icons::steps(graphics, SETTING_ICON)?.0,
                Slider::new(strings.step_size, step_sizes, settings.step_size),
            ),
            forms: Dock::arced(forms, settings.form),
            name: Name::new(fonts, names)?,
            folder: IconButton::new(fonts.icon('\u{E838}', 16.0)?, strings.open_file_location),
            caption: Caption::new(fonts, strings.caption)?,
            saved: None,
            check: Animation::idle(logo::CHECKING),
            focus: None,
            window: (0.0, 0.0),
            pointed: None,
            open: None,
            shown: tool,
            out: Animation::idle(0.0),
            space: fonts.caption(strings.space)?,
            reset_zoom: fonts.caption(strings.reset_zoom)?,
            crop: IconButton::labeled(fonts.icon('\u{E8C6}', 14.0)?, fonts.caption(strings.crop)?),
            hint: Animation::idle(0.0),
        };
        Ok(toolbar)
    }

    /// What the toolbar is set to, worth remembering for the next capture.
    pub fn settings(&self) -> EditorSettings {
        EditorSettings {
            tool: self.tool() as usize,
            step_size: self.step_size.slider.value(),
            form: self.forms.selected(),
        }
    }

    /// The narrowest the window fits the toolbar in, in DIPs: the title bar's
    /// controls beside the caption buttons, and the widest capsule at the bottom.
    pub fn min_width(&self) -> f32 {
        let title = EDGE + name::MIN + END + Caption::WIDTH;
        let options = Tool::ALL
            .map(|tool| self.options_width(tool))
            .into_iter()
            .fold(0.0, f32::max);
        // The dock stays centered, so the island beside it takes room at both sides.
        let dock = self.tools.width() + 2.0 * (ISLAND_GAP + ISLAND);
        title.max(dock.max(options) + 2.0 * EDGE)
    }

    /// The island beside the dock that holds the folder button: a capsule of its
    /// own, as tall as the dock.
    fn island(&self) -> Area {
        let left = (self.window.0 + self.tools.width()) / 2.0 + ISLAND_GAP;
        Area::new(left, self.dock_top(), ISLAND, self.tools.height())
    }

    /// The capture saved itself: the folder button's check draws itself again.
    pub fn saved(&mut self) {
        self.check = Animation::clock(logo::CHECKING);
    }

    /// The room the dock takes at the bottom of the window, in DIPs. A tool's options
    /// come out over the capture.
    pub fn bottom(&self) -> f32 {
        BOTTOM + self.tools.height() + ABOVE
    }

    /// Lays the controls out in a window of `window` DIPs: along the title bar,
    /// before the caption buttons, and centered at the bottom.
    pub fn layout(&mut self, window: (f32, f32)) {
        self.window = window;
        self.caption.layout(window.0, TITLE_BAR);
        let center = (window.0 / 2.0, TITLE_BAR / 2.0);
        self.name.place(EDGE, self.caption.left() - END, center);
        let island = self.island();
        let inset = (ISLAND - IconButton::WIDTH) / 2.0;
        self.folder.round(
            island.left + inset,
            island.top + inset,
            island.height - 2.0 * inset,
        );

        let (width, height) = window;
        let dock = (width - self.tools.width()) / 2.0;
        let bounds = Area::new(0.0, TITLE_BAR, width, height - TITLE_BAR);
        self.tools
            .place(dock, height - BOTTOM - self.tools.height(), bounds);
        for tool in Tool::ALL {
            // Its picker goes round the tool, and its settings over that.
            let center = self.tool_center(tool);
            if let Some(picker) = self.picker_mut(tool) {
                picker.place_around(center, bounds);
            }
            let area = self.options_area(tool);
            let middle = area.middle();
            let mut left = area.left + OPTIONS_PADDING;
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
            || (self.open).is_some_and(|tool| self.options_area(tool).contains(point))
            || self.island().contains(point)
            || (self.hint.target() > 0.0 && self.hint_area().contains(point))
    }

    /// The controls on show, as assistive technologies know them, each numbered as
    /// its part.
    pub fn describe(&self) -> Vec<Element> {
        let mut elements = Vec::new();
        for part in self.parts() {
            self.get(part).describe(part as usize, &mut elements);
        }
        self.caption.describe(&mut elements);
        elements
    }

    /// The window's minimize, maximize and close buttons.
    pub fn caption(&mut self) -> &mut Caption {
        &mut self.caption
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
            Part::StepSize => &mut self.step_size.slider,
            Part::Form => &mut self.forms,
            Part::Crop => &mut self.crop,
            Part::Name => &mut self.name,
            Part::Folder => &mut self.folder,
        }
    }

    /// Returns whether the toolbar's look changes.
    pub fn hover(&mut self, point: Option<Point>) -> bool {
        // A tool's options come out while the pointer is on it, and stay out while
        // it's on them or on its way there.
        let on = point.and_then(|point| self.tools.index_at(point).map(Tool::at));
        let kept =
            (self.pointed).filter(|&tool| point.is_some_and(|at| self.reach(tool).contains(at)));
        self.pointed = on.or(kept);
        let opened = self.reveal();
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
        let mut changed = opened;
        for part in parts {
            changed |= self.control(part).hover(point);
        }
        changed
    }

    /// Brings out the options of the tool the pointer is on, or the chosen one's
    /// while the keyboard is on the dock or on them, and puts away any other's.
    /// Returns whether that changes what shows.
    fn reveal(&mut self) -> bool {
        let tool = self.tool();
        let keyed = (self.focus)
            .filter(|&part| part == Part::Tools || tool.options().contains(&part))
            .map(|_| tool);
        let open = self.pointed.or(keyed);
        if open == self.open {
            return false;
        }
        self.out = match open {
            Some(tool) => {
                // Another tool's start halfway out: quick, but still from the tool.
                let from = if tool == self.shown {
                    self.out.value(Instant::now())
                } else {
                    0.5
                };
                self.shown = tool;
                Animation::new(from.min(1.0), 1.0, motion::NORMAL, Easing::SPRING)
            }
            None => (self.out).retarget(0.0, motion::FAST, Easing::ACCELERATE),
        };
        self.open = open;
        true
    }

    /// The tool that `part` is an option of, if it's one.
    pub fn owner(&self, part: Part) -> Option<Tool> {
        (Tool::ALL.into_iter()).find(|tool| tool.options().contains(&part))
    }

    /// Whether resting the pointer where it is would show a tool's name, or a form's or
    /// a frame's.
    pub fn awaits_rest(&self) -> bool {
        let picker = self.open.and_then(|tool| self.picker(tool));
        self.tools.awaits_rest() || picker.is_some_and(Dock::awaits_rest)
    }

    /// The pointer rested at `point`. Returns whether that shows a tool's name, or a
    /// form's or a frame's.
    pub fn rest(&mut self, point: Point) -> bool {
        let picker = self.open.and_then(|tool| self.picker_mut(tool));
        picker.is_some_and(|picker| picker.rest(point)) || self.tools.rest(point)
    }

    /// Moves the keyboard focus to the next control, or the previous one if `back`.
    pub fn tab(&mut self, back: bool) {
        let parts = self.parts();
        let focus = self
            .focus
            .and_then(|focus| parts.iter().position(|&part| part == focus));
        let focusable = |index: usize| self.get(parts[index]).is_focusable();
        self.focus = controls::tab(parts.len(), focus, back, focusable).map(|index| parts[index]);
        self.reveal();
    }

    /// The control with the keyboard focus.
    pub fn focused(&self) -> Option<Part> {
        self.focus
            .filter(|&part| self.parts().contains(&part) && self.get(part).is_focusable())
    }

    pub fn unfocus(&mut self) {
        self.focus = None;
        self.reveal();
    }

    pub fn draw(&self, painter: &Painter, now: Instant) {
        self.caption.draw(painter);
        self.draw_hint(painter, now);
        for part in self.parts() {
            match part {
                // They show with what draws them: the options and the hint.
                _ if self.owner(part).is_some() => {}
                Part::Crop => {}
                Part::Folder => self.draw_folder(painter, now),
                _ => self.get(part).draw(painter, now),
            }
        }
        self.draw_options(painter, now);
        if let Some(part) = self.focused() {
            let (area, radius) = self.get(part).focus_area();
            painter.focus(area, radius);
        }
        // The names make room for what's above the dock: the options, or the hint.
        let tips = match self.open {
            Some(tool) => self.reach(tool).top,
            None => self.dock_top() - (STACK + HINT) * self.hint.value(now),
        };
        self.tools.draw_tip(painter, tips, now);
        self.forms.draw_tip(painter, tips, now);
        self.draw_told(painter, now);
    }

    pub fn is_animating(&self, now: Instant) -> bool {
        self.out.is_running(now)
            || self.hint.is_running(now)
            || self.check.is_running(now)
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

    /// Shows the chosen tool's options, after the dock changed the tool: under the
    /// pointer that chose it, or with the keyboard on the dock.
    pub fn follow_tool(&mut self) {
        if self.pointed.is_some() {
            self.pointed = Some(self.tool());
        }
        self.reveal();
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

    /// The folder button on its island, with the check that says the capture is
    /// saved. The first few times, the button beats and the island opens to name the
    /// folder, room beside it allowing.
    fn draw_folder(&self, painter: &Painter, now: Instant) {
        let island = self.island();
        let saved = (self.saved.as_ref()).filter(|saved| saved.clock.is_running(now));
        let pill = saved.and_then(|saved| {
            let reach = saved.folder.width + PILL_GAP;
            let fits = reach <= self.window.0 - EDGE - island.right();
            fits.then(|| hints::pill(saved.clock.value(now), reach))
        });
        let reach = pill.as_ref().map_or(0.0, |pill| pill.reach);
        let capsule = Area::new(island.left, island.top, island.width + reach, island.height);
        painter.flyout(capsule, island.height / 2.0);
        if let Some((saved, pill)) = saved.zip(pill.as_ref()) {
            self.draw_pill(painter, &saved.folder, pill);
        }
        let button = self.folder.area();
        let center = (button.left + button.width / 2.0, button.middle());
        let pop = saved.map_or(1.0, |saved| hints::pop(saved.clock.value(now)));
        painter.grown(1.0, pop, center, || {
            self.folder.draw(painter, now);
            // At the folder's foot, on a patch of the island that sets it apart.
            let at = (center.0 + CHECK_AT.0, center.1 + CHECK_AT.1);
            painter.circle(at, CHECK.0 / 2.0 + CHECK_ROOM, painter.palette.flyout);
            let check = Area::new(at.0 - CHECK.0 / 2.0, at.1 - CHECK.1 / 2.0, CHECK.0, CHECK.1);
            painter.check(check, CHECK_STROKE, self.check.value(now));
        });
    }

    /// The pill the folder button opens into, as `pill` says, with the `folder`'s
    /// name starting by the button and rising into place and away.
    fn draw_pill(&self, painter: &Painter, folder: &Label, pill: &hints::Pill) {
        if pill.fill <= 0.0 {
            return;
        }
        let palette = painter.palette;
        let button = self.folder.area();
        let area = Area::new(
            button.left,
            button.top,
            button.width + pill.reach,
            button.height,
        );
        let area = painter.align(area);
        let radius = button.height / 2.0;
        painter.fill(area, radius, faded(palette.subtle_hover, pill.fill));
        if pill.shown <= 0.0 {
            return;
        }
        let left = button.right() + PILL_GAP - PILL_PADDING;
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

    /// A tool's options, coming out of the tool they belong to and going back into
    /// it: its picker round it, as the top of a wheel, and its settings in a capsule
    /// over that.
    fn draw_options(&self, painter: &Painter, now: Instant) {
        let out = self.out.value(now);
        if out <= 0.0 {
            return;
        }
        let tool = self.shown;
        let grown = SEED + (1.0 - SEED) * out;
        painter.grown(out.min(1.0), grown, self.tool_center(tool), || {
            if let Some(picker) = self.picker(tool) {
                picker.draw(painter, now);
            }
            if self.has_settings(tool) {
                let area = painter.align(self.options_area(tool));
                painter.flyout(area, OPTIONS / 2.0);
                for &part in tool.options() {
                    if let Some(setting) = self.setting(part) {
                        setting.draw(painter, now);
                    }
                }
            }
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
        // A tool's options take its place while they're out.
        let shown = self.hint.value(now) * (1.0 - self.out.value(now)).clamp(0.0, 1.0);
        if shown <= 0.0 {
            return;
        }
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
        Area::new(left, self.dock_top() - STACK - HINT, width, HINT)
    }

    /// The zoom hint's keycap: as wide as its key, but never narrower than round.
    fn keycap_width(&self) -> f32 {
        (self.space.width + 2.0 * KEY_PADDING).max(KEYCAP)
    }

    fn dock_top(&self) -> f32 {
        self.window.1 - BOTTOM - self.tools.height()
    }

    /// The middle of `tool`'s place in the dock, which its options come out of.
    fn tool_center(&self, tool: Tool) -> Point {
        let item = self.tools.item_area(tool as usize);
        (item.left + item.width / 2.0, item.middle())
    }

    /// Whether `tool` has settings, which go in a capsule.
    fn has_settings(&self, tool: Tool) -> bool {
        (tool.options().iter()).any(|&part| self.setting(part).is_some())
    }

    /// How wide the capsule of `tool`'s settings is.
    fn options_width(&self, tool: Tool) -> f32 {
        let settings: Vec<&Setting> = (tool.options().iter())
            .filter_map(|&part| self.setting(part))
            .collect();
        let widths: f32 = settings.iter().map(|setting| setting.width()).sum();
        widths + settings.len().saturating_sub(1) as f32 * GAP + 2.0 * OPTIONS_PADDING
    }

    /// Everything of `tool`'s that the pointer can be on without its options going
    /// away: the tool, its options and the way between them.
    fn reach(&self, tool: Tool) -> Area {
        let item = self.tools.item_area(tool as usize);
        let mut areas = vec![item];
        areas.extend(self.picker(tool).map(Dock::reach));
        if self.has_settings(tool) {
            areas.push(self.options_area(tool));
        }
        let edge = |pick: fn(f32, f32) -> f32, of: fn(&Area) -> f32, start: f32| {
            areas.iter().map(of).fold(start, pick)
        };
        let (left, top) = (
            edge(f32::min, |area| area.left, f32::MAX),
            edge(f32::min, |area| area.top, f32::MAX),
        );
        let right = edge(f32::max, |area| area.right(), f32::MIN);
        Area::new(left, top, right - left, item.bottom() - top)
    }

    /// The icons `tool`'s options lead with, to pick what it adds.
    fn picker(&self, tool: Tool) -> Option<&Dock> {
        match tool {
            Tool::Shapes => Some(&self.forms),
            Tool::Blur | Tool::Focus | Tool::Steps => None,
        }
    }

    fn picker_mut(&mut self, tool: Tool) -> Option<&mut Dock> {
        match tool {
            Tool::Shapes => Some(&mut self.forms),
            Tool::Blur | Tool::Focus | Tool::Steps => None,
        }
    }

    /// The capsule of `tool`'s settings: over the tool, above its picker if it has
    /// one, and within the window.
    fn options_area(&self, tool: Tool) -> Area {
        let width = self.options_width(tool);
        let bottom = match self.picker(tool) {
            Some(picker) => picker.reach().top,
            None => self.dock_top(),
        };
        let centered = self.tool_center(tool).0 - width / 2.0;
        let left = centered.min(self.window.0 - EDGE - width).max(EDGE);
        Area::new(left, bottom - STACK - OPTIONS, width, OPTIONS)
    }

    /// The parts on show: the dock, the options that are out, the crop button while
    /// the capture is zoomed, and the title bar's.
    fn parts(&self) -> Vec<Part> {
        let mut parts = vec![Part::Tools];
        parts.extend_from_slice(self.open.map_or(&[], Tool::options));
        if self.hint.target() > 0.0 {
            parts.push(Part::Crop);
        }
        parts.extend([Part::Name, Part::Folder]);
        parts
    }

    fn get(&self, part: Part) -> &dyn Control {
        match part {
            Part::Tools => &self.tools,
            Part::StepSize => &self.step_size.slider,
            Part::Form => &self.forms,
            Part::Crop => &self.crop,
            Part::Name => &self.name,
            Part::Folder => &self.folder,
        }
    }

    fn setting(&self, part: Part) -> Option<&Setting> {
        match part {
            Part::StepSize => Some(&self.step_size),
            _ => None,
        }
    }

    fn setting_mut(&mut self, part: Part) -> Option<&mut Setting> {
        match part {
            Part::StepSize => Some(&mut self.step_size),
            _ => None,
        }
    }
}
