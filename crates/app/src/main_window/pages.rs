//! The main window's pages: the Welcome, which leads to Kata's first capture;
//! Home, with the button for a new capture; and Settings. Positions and sizes are
//! in DIPs.

use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use kata_ui::{
    Graphics,
    access::Element,
    controls::{self, Button, Control, HEIGHT, IconButton, Info, Shortcut, Toggle},
    motion::{Animation, Easing},
    paint::{Area, Fonts, Label, Painter, Point},
    window::CAPTION_BUTTONS,
};
use windows::core::Result;

use crate::{channel, hotkey::Hotkey, settings::Settings, strings::strings};

/// The title bar's height, as Windows recommends.
pub const TITLE_BAR: f32 = 32.0;
/// Around the cards, and inside them.
const PADDING: f32 = 16.0;
const CARD: f32 = 64.0;
/// Between cards.
const GAP: f32 = 4.0;
/// Between a tip and the window's edges.
const EDGE: f32 = 8.0;
/// The back button's width: its glyph sits 16 from the window's edge and from the
/// title, as Windows recommends.
const BACK: f32 = 48.0;
/// Between the capture button and the shortcut below it.
const HINT_GAP: f32 = 12.0;
/// Between a setting's label and its info glyph's square.
const INFO_GAP: f32 = 4.0;
const RADIUS: f32 = 4.0;
const MIN_WIDTH: f32 = 320.0;
const CARDS: usize = 7;

/// The welcome's logo, and the room under it, the greeting's and the invitation's.
const LOGO: f32 = 64.0;
const LOGO_GAP: f32 = 20.0;
const GREETING_GAP: f32 = 4.0;
const INVITATION_GAP: f32 = 24.0;
/// The welcome's choreography, in seconds: once the window is in, the logo draws
/// itself; the greeting rises below it, then the invitation; and a glint crosses
/// the greeting and the logo's light.
const LEAD: f32 = 0.25;
const GREETING: f32 = LEAD + 0.55;
const FREE: f32 = GREETING + 0.1;
const INVITATION: f32 = LEAD + 0.85;
const GLINT: f32 = LEAD + 1.0;
const GLINTING: f32 = 1.1;
const WELCOME: f32 = GLINT + GLINTING;
/// What rises takes this long, from this far below, in seconds and DIPs.
const RISING: f32 = 0.4;
const RISE: f32 = 8.0;
/// How wide the glint's band is.
const BAND: f32 = 96.0;

/// In the order `numbered` counts them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Page {
    Welcome,
    Home,
    Settings,
}

impl Page {
    /// The page that `page as usize` numbers.
    pub fn numbered(number: usize) -> Self {
        [Self::Welcome, Self::Home, Self::Settings][number.min(2)]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    FirstCapture,
    Capture,
    OpenSettings,
    Back,
    Shortcut,
    ScreenKeysInfo,
    ScreenKeys,
    OpenEditor,
    ToolNames,
    Sound,
    Folder,
    Startup,
}

/// What the shortcut says while it waits for a new one.
#[derive(Clone, Copy)]
pub enum Prompt {
    Press,
    AddModifier,
    InUse,
}

pub struct Pages {
    fonts: Fonts,
    /// The client area.
    room: (f32, f32),
    /// The middle of the title bar's row of buttons and titles.
    title_row: f32,

    logo: Area,
    greeting: Label,
    free: Label,
    first_capture: Button,
    /// How many seconds into the welcome it is.
    welcome: Animation,

    title: Label,
    capture: Button,
    open_settings: IconButton,
    /// The shortcut, under the capture button.
    hint: Label,

    back: IconButton,
    settings_title: Label,
    shortcut_title: Label,
    shortcut: Shortcut,
    /// In `Prompt`'s order.
    prompts: [Label; 3],
    screen_keys: Toggle,
    screen_keys_info: Info,
    open_editor: Toggle,
    tool_names: Toggle,
    sound: Toggle,
    folder_title: Label,
    folder: PathBuf,
    /// The folder's path, trimmed to fit.
    folder_path: Label,
    change_folder: Button,
    startup: Toggle,
    /// Which of the page's parts has the keyboard focus, by its place among them.
    focus: Option<usize>,
}

impl Pages {
    pub fn new(
        graphics: &Graphics,
        settings: &Settings,
        folder: &Path,
        startup: bool,
    ) -> Result<Self> {
        let fonts = Fonts::new(graphics)?;
        let text = strings();
        let mut pages = Self {
            room: (0.0, 0.0),
            title_row: TITLE_BAR / 2.0,
            logo: Area::default(),
            greeting: fonts.title(text.greeting)?,
            free: fonts.label(text.free)?,
            first_capture: Button::new(fonts.label(text.first_capture)?),
            welcome: welcome(),
            title: fonts.caption(channel::name())?,
            capture: Button::new(fonts.label(text.new_capture)?),
            open_settings: IconButton::new(fonts.icon('\u{E713}', 16.0)?, text.settings),
            hint: fonts.caption("")?,
            back: IconButton::new(fonts.icon('\u{E72B}', 12.0)?, text.back),
            settings_title: fonts.caption(text.settings)?,
            shortcut_title: fonts.label(text.shortcut)?,
            shortcut: Shortcut::new(text.shortcut, Vec::new()),
            prompts: [
                fonts.label(text.press_shortcut)?,
                fonts.label(text.add_modifier)?,
                fonts.label(text.in_use)?,
            ],
            screen_keys: Toggle::new(fonts.label(text.screen_keys)?, settings.screen_keys)
                .explained(text.screen_keys_help),
            screen_keys_info: Info::new(
                fonts.icon('\u{E946}', 16.0)?,
                fonts.wrapped(text.screen_keys_help, Info::TEXT_WIDTH)?,
            ),
            open_editor: Toggle::new(fonts.label(text.open_editor)?, settings.open_editor),
            tool_names: Toggle::new(fonts.label(text.tool_names)?, settings.tool_names),
            sound: Toggle::new(fonts.label(text.sound)?, settings.sound),
            folder_title: fonts.label(text.folder)?,
            folder: folder.to_owned(),
            folder_path: fonts.caption("")?,
            change_folder: Button::standard(fonts.label(text.change)?).named(text.change_folder),
            startup: Toggle::new(fonts.label(text.start_with_windows)?, startup),
            focus: None,
            fonts,
        };
        pages.set_hotkey(settings.hotkey)?;
        Ok(pages)
    }

    /// The smallest client area that fits both pages.
    pub fn min_size(&self) -> (f32, f32) {
        let rows = [
            self.shortcut_title.width + PADDING + self.shortcut.width(),
            self.screen_keys.width() + INFO_GAP + Info::SIZE,
            self.open_editor.width(),
            self.tool_names.width(),
            self.sound.width(),
            self.folder_title.width + PADDING + self.change_folder.width(),
            self.startup.width(),
        ];
        let cards = rows.into_iter().fold(0.0, f32::max) + 4.0 * PADDING;
        let title_bar = BACK + self.settings_title.width + PADDING + CAPTION_BUTTONS;
        let height = self.cards_top() + CARDS as f32 * (CARD + GAP) - GAP + PADDING;
        (MIN_WIDTH.max(cards).max(title_bar + PADDING), height)
    }

    /// Arranges both pages in a client area of `room`, with the caption buttons at
    /// `caption_buttons`. The title bar's buttons and titles line up with them.
    pub fn layout(&mut self, room: (f32, f32), caption_buttons: Area) -> Result<()> {
        self.room = room;
        self.title_row = caption_buttons.middle();
        let above = LOGO + LOGO_GAP + self.greeting.height + GREETING_GAP + self.free.height;
        let group = above + INVITATION_GAP + HEIGHT + HINT_GAP + self.hint.height;
        let top = TITLE_BAR + (room.1 - TITLE_BAR - group) / 2.0;
        self.logo = Area::new((room.0 - LOGO) / 2.0, top, LOGO, LOGO);
        let invitation = &mut self.first_capture;
        invitation.place(
            (room.0 - invitation.width()) / 2.0,
            top + above + INVITATION_GAP,
        );
        let group = HEIGHT + HINT_GAP + self.hint.height;
        let top = TITLE_BAR + (room.1 - TITLE_BAR - group) / 2.0;
        self.capture
            .place((room.0 - self.capture.width()) / 2.0, top);
        // Edge to edge, as the caption buttons: Settings beside them, as wide as
        // one, and Back at the window's edge. Windows' bounds for the caption
        // buttons start a little left of the first, so Settings goes by their end.
        let (top, height) = (caption_buttons.top, caption_buttons.height);
        let width = CAPTION_BUTTONS / 3.0;
        let left = caption_buttons.right() - CAPTION_BUTTONS - width;
        self.open_settings
            .bleed(Area::new(left, top, width, height));
        self.back.bleed(Area::new(0.0, top, BACK, height));
        let [
            shortcut,
            screen_keys,
            open_editor,
            tool_names,
            sound,
            folder,
            startup,
        ] = self.cards();
        self.shortcut
            .place(shortcut.right() - PADDING, shortcut.middle());
        for (toggle, card) in [
            (&mut self.screen_keys, screen_keys),
            (&mut self.open_editor, open_editor),
            (&mut self.tool_names, tool_names),
            (&mut self.sound, sound),
            (&mut self.startup, startup),
        ] {
            toggle.place(card.left + PADDING, card.top);
            toggle.stretch(card.width - 2.0 * PADDING, card.height);
        }
        let left = self.screen_keys.label_right() + INFO_GAP;
        let below_title = Area::new(EDGE, TITLE_BAR, room.0 - 2.0 * EDGE, room.1 - TITLE_BAR);
        self.screen_keys_info
            .place(left, screen_keys.middle(), below_title);
        let change = &mut self.change_folder;
        let left = folder.right() - PADDING - change.width();
        change.place(left, folder.middle() - HEIGHT / 2.0);
        self.trim_folder()
    }

    pub fn part_at(&self, page: Page, point: Point) -> Option<Part> {
        parts(page)
            .iter()
            .copied()
            .find(|&part| self.get(part).contains(point))
    }

    /// `page`'s controls, as assistive technologies know them, each numbered as
    /// its part.
    pub fn describe(&self, page: Page) -> Vec<Element> {
        let mut elements = Vec::new();
        for &part in parts(page) {
            self.get(part).describe(part as usize, &mut elements);
        }
        elements
    }

    pub fn control(&mut self, part: Part) -> &mut dyn Control {
        match part {
            Part::FirstCapture => &mut self.first_capture,
            Part::Capture => &mut self.capture,
            Part::OpenSettings => &mut self.open_settings,
            Part::Back => &mut self.back,
            Part::Shortcut => &mut self.shortcut,
            Part::ScreenKeysInfo => &mut self.screen_keys_info,
            Part::ScreenKeys => &mut self.screen_keys,
            Part::OpenEditor => &mut self.open_editor,
            Part::ToolNames => &mut self.tool_names,
            Part::Sound => &mut self.sound,
            Part::Folder => &mut self.change_folder,
            Part::Startup => &mut self.startup,
        }
    }

    fn get(&self, part: Part) -> &dyn Control {
        match part {
            Part::FirstCapture => &self.first_capture,
            Part::Capture => &self.capture,
            Part::OpenSettings => &self.open_settings,
            Part::Back => &self.back,
            Part::Shortcut => &self.shortcut,
            Part::ScreenKeysInfo => &self.screen_keys_info,
            Part::ScreenKeys => &self.screen_keys,
            Part::OpenEditor => &self.open_editor,
            Part::ToolNames => &self.tool_names,
            Part::Sound => &self.sound,
            Part::Folder => &self.change_folder,
            Part::Startup => &self.startup,
        }
    }

    /// Follows the pointer over `page`. Returns whether its look changes.
    pub fn hover(&mut self, page: Page, point: Option<Point>) -> bool {
        // Only the part under the pointer sees it: the info glyph sits on its setting.
        let under = point.and_then(|point| self.part_at(page, point));
        parts(page).iter().fold(false, |changed, &part| {
            let point = point.filter(|_| under == Some(part));
            self.control(part).hover(point) | changed
        })
    }

    /// Whether resting the pointer where it is would show a tooltip on `page`.
    pub fn awaits_rest(&self, page: Page) -> bool {
        page == Page::Settings && self.screen_keys_info.awaits_rest()
    }

    /// The pointer rested at `point` on `page`. Returns whether a tooltip shows.
    pub fn rest(&mut self, page: Page, point: Point) -> bool {
        page == Page::Settings && self.screen_keys_info.rest(point)
    }

    pub fn draw(&self, page: Page, painter: &Painter, now: Instant) {
        let palette = painter.palette;
        match page {
            Page::Welcome => {
                painter.text(&self.title, PADDING, self.title_row, palette.text);
                self.draw_welcome(painter, now);
            }
            Page::Home => {
                painter.text(&self.title, PADDING, self.title_row, palette.text);
                self.draw_hint(painter, self.capture.area());
            }
            Page::Settings => {
                painter.text(&self.settings_title, BACK, self.title_row, palette.text);
                let cards = self.cards();
                for card in cards {
                    let card = painter.align(card);
                    painter.fill(card, RADIUS, palette.card);
                    painter.stroke(card, RADIUS, painter.border(), palette.control_stroke);
                }
                let [shortcut, _, _, _, _, folder, _] = cards;
                let left = shortcut.left + PADDING;
                painter.text(&self.shortcut_title, left, shortcut.middle(), palette.text);
                // The folder's title and path, one above the other.
                let (title, path) = (&self.folder_title, &self.folder_path);
                let top = folder.middle() - (title.height + path.height) / 2.0;
                let left = folder.left + PADDING;
                painter.text(title, left, top + title.height / 2.0, palette.text);
                let middle = top + title.height + path.height / 2.0;
                painter.text(path, left, middle, palette.text_secondary);
            }
        }
        let controls = || {
            for &part in parts(page) {
                self.get(part).draw(painter, now);
            }
        };
        if page == Page::Welcome {
            // The invitation comes last, with the shortcut under it.
            let (opacity, offset) = self.rising(INVITATION, now);
            painter.lowered(opacity, offset, || {
                controls();
                self.draw_hint(painter, self.first_capture.area());
            });
        } else {
            controls();
        }
        if let Some(part) = self.focused(page) {
            let (area, radius) = self.get(part).focus_area();
            painter.focus(area, radius);
        }
        if page == Page::Settings {
            self.screen_keys_info.draw_tip(painter, now);
        }
    }

    /// The logo draws itself, the greeting rises below it, and a glint crosses them.
    fn draw_welcome(&self, painter: &Painter, now: Instant) {
        let palette = painter.palette;
        let time = self.welcome.value(now);
        painter.logo(painter.align(self.logo), time - LEAD);
        let (greeting, free) = (&self.greeting, &self.free);
        let left = (self.room.0 - greeting.width) / 2.0;
        let middle = self.logo.bottom() + LOGO_GAP + greeting.height / 2.0;
        let (opacity, offset) = self.rising(GREETING, now);
        painter.lowered(opacity, offset, || {
            painter.text(greeting, left, middle, palette.text);
        });
        let (opacity, offset) = self.rising(FREE, now);
        let below = middle + greeting.height / 2.0 + GREETING_GAP + free.height / 2.0;
        painter.lowered(opacity, offset, || {
            let left = (self.room.0 - free.width) / 2.0;
            painter.text(free, left, below, palette.text_secondary);
        });
        let sweep = (time - GLINT) / GLINTING;
        if sweep > 0.0 && sweep < 1.0 {
            let x = left - BAND / 2.0 + (greeting.width + BAND) * Easing::SWEEP.apply(sweep);
            painter.glint(&[(greeting, left)], middle, x, BAND);
        }
    }

    /// The shortcut, centered under `button`.
    fn draw_hint(&self, painter: &Painter, button: Area) {
        let left = (self.room.0 - self.hint.width) / 2.0;
        let middle = button.bottom() + HINT_GAP + self.hint.height / 2.0;
        painter.text(&self.hint, left, middle, painter.palette.text_secondary);
    }

    /// How far into the welcome something rising at `start` seconds is: its opacity,
    /// and how far it still is below its place.
    fn rising(&self, start: f32, now: Instant) -> (f32, f32) {
        let progress = (self.welcome.value(now) - start) / RISING;
        let risen = Easing::DECELERATE.apply(progress);
        (risen, (1.0 - risen) * RISE)
    }

    /// Plays the welcome from its start.
    pub fn greet(&mut self) {
        self.welcome = welcome();
    }

    /// Moves the keyboard focus to `page`'s next control, or the previous one if
    /// `back`.
    pub fn tab(&mut self, page: Page, back: bool) {
        let parts = parts(page);
        let focusable = |index: usize| self.get(parts[index]).is_focusable();
        self.focus = controls::tab(parts.len(), self.focus, back, focusable);
    }

    /// The control on `page` with the keyboard focus.
    pub fn focused(&self, page: Page) -> Option<Part> {
        let &part = parts(page).get(self.focus?)?;
        self.get(part).is_focusable().then_some(part)
    }

    pub fn unfocus(&mut self) {
        self.focus = None;
    }

    /// Following a change of page, a keyboard focus starts over at the first control.
    pub fn refocus(&mut self) {
        self.focus = self.focus.map(|_| 0);
    }

    pub fn is_animating(&self, page: Page, now: Instant) -> bool {
        (page == Page::Welcome && self.welcome.is_running(now))
            || parts(page)
                .iter()
                .any(|&part| self.get(part).is_animating(now))
    }

    pub fn set_hotkey(&mut self, hotkey: Hotkey) -> Result<()> {
        let keys = hotkey.keys();
        let keys = keys.iter().map(|key| self.fonts.label(key));
        self.shortcut.set_keys(keys.collect::<Result<_>>()?);
        self.hint = self.fonts.caption(&hotkey.label())?;
        Ok(())
    }

    /// Has the shortcut wait for a new one, saying `prompt`.
    pub fn listen(&mut self, prompt: Prompt) {
        self.shortcut.listen(self.prompts[prompt as usize].clone());
    }

    pub fn stop_listening(&mut self) {
        self.shortcut.stop();
    }

    pub fn is_listening(&self) -> bool {
        self.shortcut.is_listening()
    }

    pub fn set_folder(&mut self, folder: &Path) -> Result<()> {
        folder.clone_into(&mut self.folder);
        self.trim_folder()
    }

    pub fn screen_keys(&self) -> bool {
        self.screen_keys.is_on()
    }

    pub fn open_editor(&self) -> bool {
        self.open_editor.is_on()
    }

    pub fn tool_names(&self) -> bool {
        self.tool_names.is_on()
    }

    pub fn sound(&self) -> bool {
        self.sound.is_on()
    }

    pub fn startup(&self) -> bool {
        self.startup.is_on()
    }

    pub fn set_startup(&mut self, on: bool) {
        self.startup.set(on);
    }

    fn cards_top(&self) -> f32 {
        TITLE_BAR + PADDING
    }

    /// The settings' cards, from top to bottom.
    fn cards(&self) -> [Area; CARDS] {
        let width = self.room.0 - 2.0 * PADDING;
        let top = self.cards_top();
        std::array::from_fn(|row| {
            let top = top + row as f32 * (CARD + GAP);
            Area::new(PADDING, top, width, CARD)
        })
    }

    /// Fits the folder's path between its card's edge and the Change button.
    fn trim_folder(&mut self) -> Result<()> {
        let left = self.cards()[5].left + PADDING;
        let width = self.change_folder.area().left - PADDING - left;
        let path = self.folder.to_string_lossy();
        self.folder_path = self.fonts.path(&path, width)?;
        Ok(())
    }
}

/// The welcome's clock, counting its seconds from now: at its end at once when
/// animation effects are off.
fn welcome() -> Animation {
    let linear = Easing::new(0.0, 0.0, 1.0, 1.0);
    Animation::new(0.0, WELCOME, Duration::from_secs_f32(WELCOME), linear)
}

/// `page`'s part with the number `describe` gave it.
pub fn part(page: Page, number: usize) -> Option<Part> {
    parts(page)
        .iter()
        .copied()
        .find(|&part| part as usize == number)
}

fn parts(page: Page) -> &'static [Part] {
    match page {
        Page::Welcome => &[Part::FirstCapture],
        Page::Home => &[Part::Capture, Part::OpenSettings],
        Page::Settings => &[
            Part::Back,
            Part::Shortcut,
            // Before its setting, which it sits on.
            Part::ScreenKeysInfo,
            Part::ScreenKeys,
            Part::OpenEditor,
            Part::ToolNames,
            Part::Sound,
            Part::Folder,
            Part::Startup,
        ],
    }
}
