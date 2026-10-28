//! The main window's two pages: Home, with the button for a new capture, and
//! Settings. Positions and sizes are in DIPs.

use std::{
    path::{Path, PathBuf},
    time::Instant,
};

use kata_ui::{
    Graphics,
    controls::{Button, Control, HEIGHT, IconButton, Shortcut, Toggle},
    paint::{Area, Fonts, Label, Painter, Point},
    window::CAPTION_BUTTONS,
};
use windows::core::Result;

use crate::{hotkey::Hotkey, settings::Settings, strings::strings};

/// The title bar's height.
pub const TITLE_BAR: f32 = 48.0;
/// Around the cards, and inside them.
const PADDING: f32 = 16.0;
const CARD: f32 = 64.0;
/// Between cards.
const GAP: f32 = 4.0;
/// Between a title bar button and the window's edge or the caption buttons.
const EDGE: f32 = 8.0;
/// Between the capture button and the shortcut below it.
const HINT_GAP: f32 = 12.0;
const RADIUS: f32 = 4.0;
const MIN_WIDTH: f32 = 320.0;
const CARDS: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Page {
    Home,
    Settings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Capture,
    OpenSettings,
    Back,
    Shortcut,
    OpenEditor,
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
    open_editor: Toggle,
    sound: Toggle,
    folder_title: Label,
    folder: PathBuf,
    /// The folder's path, trimmed to fit.
    folder_path: Label,
    change_folder: Button,
    startup: Toggle,
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
            title: fonts.caption("Kata")?,
            capture: Button::new(fonts.label(text.new_capture)?, true),
            open_settings: IconButton::new(fonts.icon('\u{E713}', 16.0)?),
            hint: fonts.caption("")?,
            back: IconButton::new(fonts.icon('\u{E72B}', 12.0)?),
            settings_title: fonts.caption(text.settings)?,
            shortcut_title: fonts.label(text.shortcut)?,
            shortcut: Shortcut::new(Vec::new()),
            prompts: [
                fonts.label(text.press_shortcut)?,
                fonts.label(text.add_modifier)?,
                fonts.label(text.in_use)?,
            ],
            open_editor: Toggle::new(fonts.label(text.open_editor)?, settings.open_editor),
            sound: Toggle::new(fonts.label(text.sound)?, settings.sound),
            folder_title: fonts.label(text.folder)?,
            folder: folder.to_owned(),
            folder_path: fonts.caption("")?,
            change_folder: Button::standard(fonts.label(text.change)?),
            startup: Toggle::new(fonts.label(text.start_with_windows)?, startup),
            fonts,
        };
        pages.set_hotkey(settings.hotkey)?;
        Ok(pages)
    }

    /// The smallest client area that fits both pages.
    pub fn min_size(&self) -> (f32, f32) {
        let rows = [
            self.shortcut_title.width + PADDING + self.shortcut.width(),
            self.open_editor.width(),
            self.sound.width(),
            self.folder_title.width + PADDING + self.change_folder.width(),
            self.startup.width(),
        ];
        let cards = rows.into_iter().fold(0.0, f32::max) + 4.0 * PADDING;
        let title_bar = EDGE + IconButton::WIDTH + self.settings_title.width + CAPTION_BUTTONS;
        let height = self.cards_top() + CARDS as f32 * (CARD + GAP) - GAP + PADDING;
        (MIN_WIDTH.max(cards).max(title_bar + PADDING), height)
    }

    /// Arranges both pages in a client area of `room`, with the caption buttons
    /// starting at `caption_buttons`.
    pub fn layout(&mut self, room: (f32, f32), caption_buttons: f32) -> Result<()> {
        self.room = room;
        let group = HEIGHT + HINT_GAP + self.hint.height;
        let top = TITLE_BAR + (room.1 - TITLE_BAR - group) / 2.0;
        self.capture
            .place((room.0 - self.capture.width()) / 2.0, top);
        let left = caption_buttons - EDGE - IconButton::WIDTH;
        self.open_settings.place(left, EDGE);

        self.back.place(EDGE, EDGE);
        let [shortcut, open_editor, sound, folder, startup] = self.cards();
        self.shortcut
            .place(shortcut.right() - PADDING, shortcut.middle());
        for (toggle, card) in [
            (&mut self.open_editor, open_editor),
            (&mut self.sound, sound),
            (&mut self.startup, startup),
        ] {
            toggle.place(card.left + PADDING, card.top);
            toggle.stretch(card.width - 2.0 * PADDING, card.height);
        }
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

    pub fn control(&mut self, part: Part) -> &mut dyn Control {
        match part {
            Part::Capture => &mut self.capture,
            Part::OpenSettings => &mut self.open_settings,
            Part::Back => &mut self.back,
            Part::Shortcut => &mut self.shortcut,
            Part::OpenEditor => &mut self.open_editor,
            Part::Sound => &mut self.sound,
            Part::Folder => &mut self.change_folder,
            Part::Startup => &mut self.startup,
        }
    }

    fn get(&self, part: Part) -> &dyn Control {
        match part {
            Part::Capture => &self.capture,
            Part::OpenSettings => &self.open_settings,
            Part::Back => &self.back,
            Part::Shortcut => &self.shortcut,
            Part::OpenEditor => &self.open_editor,
            Part::Sound => &self.sound,
            Part::Folder => &self.change_folder,
            Part::Startup => &self.startup,
        }
    }

    /// Follows the pointer over `page`. Returns whether its look changes.
    pub fn hover(&mut self, page: Page, point: Option<Point>) -> bool {
        parts(page).iter().fold(false, |changed, &part| {
            self.control(part).hover(point) | changed
        })
    }

    pub fn draw(&self, page: Page, painter: &Painter, now: Instant) {
        let palette = painter.palette;
        match page {
            Page::Home => {
                painter.text(&self.title, PADDING, TITLE_BAR / 2.0, palette.text);
                let left = (self.room.0 - self.hint.width) / 2.0;
                let middle = self.capture.area().bottom() + HINT_GAP + self.hint.height / 2.0;
                painter.text(&self.hint, left, middle, palette.text_secondary);
            }
            Page::Settings => {
                let left = EDGE + IconButton::WIDTH;
                painter.text(&self.settings_title, left, TITLE_BAR / 2.0, palette.text);
                let cards = self.cards();
                for card in cards {
                    let card = painter.align(card);
                    painter.fill(card, RADIUS, palette.card);
                    painter.stroke(card, RADIUS, painter.border(), palette.control_stroke);
                }
                let [shortcut, _, _, folder, _] = cards;
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
        for &part in parts(page) {
            self.get(part).draw(painter, now);
        }
    }

    pub fn is_animating(&self, page: Page, now: Instant) -> bool {
        parts(page)
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

    pub fn open_editor(&self) -> bool {
        self.open_editor.is_on()
    }

    pub fn sound(&self) -> bool {
        self.sound.is_on()
    }

    pub fn startup(&self) -> bool {
        self.startup.is_on()
    }

    fn cards_top(&self) -> f32 {
        TITLE_BAR + EDGE
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
        let left = self.cards()[3].left + PADDING;
        let width = self.change_folder.area().left - PADDING - left;
        let path = self.folder.to_string_lossy();
        self.folder_path = self.fonts.path(&path, width)?;
        Ok(())
    }
}

fn parts(page: Page) -> &'static [Part] {
    match page {
        Page::Home => &[Part::Capture, Part::OpenSettings],
        Page::Settings => &[
            Part::Back,
            Part::Shortcut,
            Part::OpenEditor,
            Part::Sound,
            Part::Folder,
            Part::Startup,
        ],
    }
}
