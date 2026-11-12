//! The main window's pages: the welcome's, in `welcome`; Home, with the button for
//! a new capture; and Settings. Positions and sizes are in DIPs.

mod welcome;

use std::{
    path::{Path, PathBuf},
    time::Instant,
};

use kata_ui::{
    Graphics,
    access::Element,
    controls::{
        self, Button, Caption, Control, HEIGHT, Hue, IconButton, Info, Segments, Shortcut,
        Swatches, Toggle,
    },
    figure::Figure,
    logo,
    motion::{self, Animation, Easing},
    paint::{Area, Fonts, Label, Painter, Point},
    theme::color,
};
use windows::core::Result;

use crate::{
    channel,
    hotkey::Hotkey,
    settings::{After, INKS, Perk, Settings, tinted},
    strings::strings,
};
pub use welcome::Ready;
use welcome::{First, GLYPH};

/// The title bar's height: the taller of the two Windows recommends, with room for
/// buttons of its own.
pub const TITLE_BAR: f32 = 48.0;
/// Between the title bar's own buttons and what's beside them.
const BAR_GAP: f32 = 8.0;
/// Around the cards, and inside them.
const PADDING: f32 = 16.0;
const CARD: f32 = 64.0;
/// Above and below Premium's card in Settings; the rim that says it's the user's;
/// and Kata's orange, for its crown.
const CARD_TOP: f32 = 8.0;
const RIM: f32 = 1.5;
const ORANGE: u32 = 0xEC_5A22;
/// A setting's row in its group's card, and the room above a group, with its title.
const ROW: f32 = 52.0;
const HEADING: f32 = 36.0;
/// How many settings each group has, from top to bottom.
const GROUPS: [usize; 3] = [4, 3, 1];
/// The glyph that leads a setting or the window's title, and the room after it.
const ICON: f32 = 16.0;
const ICON_GAP: f32 = 16.0;
/// Where a setting's words start in its row, after its glyph.
const LEAD: f32 = PADDING + ICON + ICON_GAP;
/// A key of the shortcut under the capture button: its height, the room at each
/// side of its name, and between keys.
const KEY: f32 = 24.0;
const KEY_PADDING: f32 = 8.0;
const KEY_GAP: f32 = 4.0;
/// The frame around the capture button and its shortcut, the logo's four corners:
/// how far out it stands, across and down; how much it closes in under the pointer,
/// as if focusing; and its corners' arms, turn and stroke.
const FRAME: (f32, f32) = (40.0, 28.0);
const FOCUS: f32 = 6.0;
const FRAME_ARM: f32 = 14.0;
const FRAME_TURN: f32 = 5.0;
const FRAME_STROKE: f32 = 2.5;
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
const CARDS: usize = 8;
/// Between the colors and the hue beside them.
const HUE_GAP: f32 = 8.0;

/// In the order `numbered` counts them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Page {
    Welcome,
    Ready,
    Premium,
    Home,
    Settings,
}

impl Page {
    /// The page that `page as usize` numbers.
    pub fn numbered(number: usize) -> Self {
        const PAGES: [Page; 5] = [
            Page::Welcome,
            Page::Ready,
            Page::Premium,
            Page::Home,
            Page::Settings,
        ];
        PAGES[number.min(PAGES.len() - 1)]
    }

    /// Whether it's one of the welcome's, which come in in turn.
    fn is_welcome(self) -> bool {
        self < Self::Home
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    FirstCapture,
    ReadyKeys,
    GoOn,
    StayFree,
    GetPremium,
    Capture,
    OpenSettings,
    Back,
    Premium,
    Shortcut,
    ScreenKeysInfo,
    ScreenKeys,
    After,
    ToolNames,
    Ink,
    Hue,
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
    caption: Caption,
    /// The client area.
    room: (f32, f32),
    /// The middle of the title bar's row of buttons and titles.
    title_row: f32,

    logo: Area,
    greeting: Label,
    free: Label,
    first_capture: Button,
    /// How many seconds into the welcome's page on show it is.
    clock: Animation,

    first: Option<First>,
    /// How long the first capture took to show, and how much lighter it is than
    /// with Windows, which rises once weighed.
    took: Label,
    lighter: Label,
    lighter_rise: Option<Animation>,
    /// The setting that makes Kata the screenshot app, on its card; the way on; and
    /// what it promises.
    ready_keys: Toggle,
    ready_setting: Area,
    go_on: Button,
    stays_here: Label,
    /// Where Ready's parts start.
    ready_top: f32,

    all_free: Label,
    forever: Label,
    premium_adds: Label,
    /// Each perk's glyph, and what it says.
    perks: [(Label, Label); 3],
    /// Where to find each perk, said under it once Premium is the user's.
    perk_guides: [Label; 3],
    perks_card: Area,
    stay_free: Button,
    get_premium: Button,
    /// Where Premium's parts start.
    premium_top: f32,

    /// Whether Premium is the user's; and its card in Settings: a crown, what it
    /// is in two lines, and the way to its page.
    premium: bool,
    crown: Figure,
    premium_title: Label,
    premium_line: Label,
    premium_button: Button,

    title: Label,
    capture: Button,
    open_settings: IconButton,
    /// The shortcut, under the welcome's invitation.
    hint: Label,
    /// The shortcut's keys and Print Screen, as keycaps under the capture button.
    keys: Vec<Label>,
    print_screen: Label,
    /// How far the frame around the capture button has closed in, from 0 to 1.
    framing: Animation,

    back: IconButton,
    settings_title: Label,
    /// The color of the shapes and the steps: its title, the colors to pick from, the
    /// last of them the user's own, and that one's hue.
    ink_title: Label,
    ink: Swatches,
    hue: Hue,
    /// How many free tries are left of a shortcut and of a color of the user's own,
    /// said under their titles where they count.
    shortcut_tries: Option<Label>,
    ink_tries: Option<Label>,
    group_titles: [Label; 3],
    /// The glyph that leads each setting, in the rows' order.
    row_icons: [Label; CARDS],
    shortcut_title: Label,
    shortcut: Shortcut,
    /// In `Prompt`'s order.
    prompts: [Label; 3],
    screen_keys: Toggle,
    screen_keys_info: Info,
    /// What follows a capture: its title and its three choices.
    after_title: Label,
    after: Segments,
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
            caption: Caption::new(&fonts, text.caption)?,
            room: (0.0, 0.0),
            title_row: TITLE_BAR / 2.0,
            logo: Area::default(),
            greeting: fonts.title(text.greeting)?,
            free: fonts.label(text.free)?,
            first_capture: Button::new(fonts.label(text.first_capture)?),
            clock: Animation::idle(0.0),
            first: None,
            took: fonts.title(&text.ready(0.0))?,
            lighter: fonts.label(&text.lighter(0.0))?,
            lighter_rise: None,
            ready_keys: Toggle::new(fonts.label(text.make_default)?, settings.screen_keys)
                .described(fonts.caption(text.make_default_detail)?)
                .explained(text.make_default_detail),
            ready_setting: Area::default(),
            go_on: Button::new(fonts.label(text.go_on)?),
            stays_here: fonts.caption(text.stays_here)?,
            ready_top: 0.0,
            all_free: fonts.title(text.all_free)?,
            forever: fonts.label(text.forever)?,
            premium_adds: fonts.caption(text.premium_adds)?,
            perks: [
                (fonts.icon('\u{E765}', GLYPH)?, fonts.label(text.perks[0])?),
                (fonts.icon('\u{E790}', GLYPH)?, fonts.label(text.perks[1])?),
                (fonts.icon('\u{E7C4}', GLYPH)?, fonts.label(text.perks[2])?),
            ],
            perk_guides: [
                fonts.caption(text.perk_guides[0])?,
                fonts.caption(text.perk_guides[1])?,
                fonts.caption(text.perk_guides[2])?,
            ],
            perks_card: Area::default(),
            stay_free: Button::standard(fonts.label(text.stay_free)?),
            get_premium: Button::standard(fonts.label(text.get_premium)?),
            premium_top: 0.0,
            premium: false,
            crown: crown(graphics)?,
            premium_title: fonts.label("")?,
            premium_line: fonts.caption("")?,
            premium_button: Button::new(fonts.label("")?),
            title: fonts.caption(channel::name())?,
            capture: Button::new(fonts.label(text.new_capture)?),
            open_settings: IconButton::new(fonts.icon('\u{E713}', 16.0)?, text.settings),
            hint: fonts.caption("")?,
            keys: Vec::new(),
            print_screen: fonts.caption(&Hotkey::print_screen().label())?,
            framing: Animation::idle(0.0),
            group_titles: [
                fonts.caption(text.groups[0])?,
                fonts.caption(text.groups[1])?,
                fonts.caption(text.groups[2])?,
            ],
            row_icons: [
                fonts.icon('\u{E765}', ICON)?,
                fonts.icon('\u{E722}', ICON)?,
                fonts.icon('\u{E767}', ICON)?,
                fonts.icon('\u{E7E8}', ICON)?,
                fonts.icon('\u{E70F}', ICON)?,
                fonts.icon('\u{E8EC}', ICON)?,
                fonts.icon('\u{E790}', ICON)?,
                fonts.icon('\u{E8B7}', ICON)?,
            ],
            back: IconButton::new(fonts.icon('\u{E72B}', 12.0)?, text.back),
            settings_title: fonts.caption(text.settings)?,
            ink_title: fonts.label(text.ink)?,
            ink: Swatches::new(
                (INKS.iter().zip(text.inks))
                    .map(|(&ink, name)| (ink, name.to_owned()))
                    .chain([(tinted(settings.hue as f32), text.own_ink.to_owned())])
                    .collect(),
                true,
                settings.ink,
            ),
            hue: Hue::new(text.hue, tinted, settings.hue),
            shortcut_tries: None,
            ink_tries: None,
            shortcut_title: fonts.label(text.shortcut)?,
            shortcut: Shortcut::new(text.shortcut, Vec::new()),
            prompts: [
                fonts.label(text.press_shortcut)?,
                fonts.label(text.add_modifier)?,
                fonts.label(text.in_use)?,
            ],
            screen_keys: Toggle::new(fonts.label(text.make_default)?, settings.screen_keys)
                .explained(text.screen_keys_help),
            screen_keys_info: Info::new(
                fonts.icon('\u{E946}', 16.0)?,
                fonts.wrapped(text.screen_keys_help, Info::TEXT_WIDTH)?,
            ),
            after_title: fonts.label(text.after)?,
            after: Segments::new(
                (text.afters.iter())
                    .map(|word| fonts.label(word))
                    .collect::<Result<_>>()?,
                settings.after as usize,
            ),
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
        pages.set_premium(crate::settings::premium())?;
        pages.count_tries()?;
        Ok(pages)
    }

    /// The smallest client area that fits every page.
    pub fn min_size(&self) -> (f32, f32) {
        let rows = [
            self.welcome_width(),
            self.shortcut_title.width + PADDING + self.shortcut.width(),
            self.screen_keys.width() + INFO_GAP + Info::SIZE,
            self.after_title.width + PADDING + self.after.width(),
            self.tool_names.width(),
            self.sound.width(),
            self.folder_title.width + PADDING + self.change_folder.width(),
            self.startup.width(),
            self.ink_title.width + PADDING + Hue::WIDTH + HUE_GAP + self.ink.width(),
        ];
        let cards = rows.into_iter().fold(0.0, f32::max) + LEAD + 3.0 * PADDING;
        let title_bar = BACK + self.settings_title.width + PADDING + Caption::WIDTH;
        let height = self.rows()[CARDS - 1].bottom() + PADDING;
        let premium = LEAD + self.premium_title.width.max(self.premium_line.width);
        let cards = cards.max(premium + self.premium_button.width() + 3.0 * PADDING);
        (MIN_WIDTH.max(cards).max(title_bar + PADDING), height)
    }

    /// Arranges the pages in a client area of `room`. The title bar's buttons and
    /// titles share its middle with the caption buttons.
    pub fn layout(&mut self, room: (f32, f32)) -> Result<()> {
        self.room = room;
        self.caption.layout(room.0, TITLE_BAR);
        self.layout_welcome();
        let group = HEIGHT + HINT_GAP + KEY;
        let top = TITLE_BAR + (room.1 - TITLE_BAR - group) / 2.0;
        self.capture
            .place((room.0 - self.capture.width()) / 2.0, top);
        // Buttons of their own, unlike the caption buttons: Settings before those, and
        // Back at the window's edge.
        let top = (TITLE_BAR - IconButton::HEIGHT) / 2.0;
        let left = self.caption.left() - BAR_GAP - IconButton::WIDTH;
        self.open_settings.place(left, top);
        self.back.place((BACK - IconButton::WIDTH) / 2.0, top);
        let [
            shortcut,
            screen_keys,
            sound,
            startup,
            open_editor,
            tool_names,
            ink,
            folder,
        ] = self.rows();
        self.after
            .place(open_editor.right() - PADDING, open_editor.middle());
        self.ink.place(ink.right() - PADDING, ink.middle());
        let colors = ink.right() - PADDING - self.ink.width();
        self.hue.place(colors - HUE_GAP, ink.middle());
        self.shortcut
            .place(shortcut.right() - PADDING, shortcut.middle());
        for (toggle, row) in [
            (&mut self.screen_keys, screen_keys),
            (&mut self.sound, sound),
            (&mut self.startup, startup),
            (&mut self.tool_names, tool_names),
        ] {
            // After the row's glyph.
            toggle.place(row.left + LEAD, row.top);
            toggle.stretch(row.width - LEAD - PADDING, row.height);
        }
        let left = self.screen_keys.label_right() + INFO_GAP;
        let below_title = Area::new(EDGE, TITLE_BAR, room.0 - 2.0 * EDGE, room.1 - TITLE_BAR);
        self.screen_keys_info
            .place(left, screen_keys.middle(), below_title);
        if let Some(card) = self.premium_card() {
            let button = &mut self.premium_button;
            let left = card.right() - PADDING - button.width();
            button.place(left, card.middle() - HEIGHT / 2.0);
        }
        let change = &mut self.change_folder;
        let left = folder.right() - PADDING - change.width();
        change.place(left, folder.middle() - HEIGHT / 2.0);
        self.trim_folder()
    }

    /// `page`'s parts on show: Premium's card only where Premium shows, and the way
    /// to get it only until it's got.
    fn parts(&self, page: Page) -> Vec<Part> {
        let shown = |part: &Part| match part {
            Part::Premium => self.premium_card().is_some(),
            Part::GetPremium => !self.premium,
            // A hue to pick only while the color is the user's own.
            Part::Hue => self.ink.selected() == INKS.len(),
            _ => true,
        };
        parts(page).iter().copied().filter(shown).collect()
    }

    /// `page`'s part with the number `describe` gave it.
    pub fn part(&self, page: Page, number: usize) -> Option<Part> {
        (self.parts(page).into_iter()).find(|&part| part as usize == number)
    }

    /// Premium's card, at the top of Settings, where Premium shows: in Kata Dev
    /// only, until Kata is for sale.
    fn premium_card(&self) -> Option<Area> {
        let width = self.room.0 - 2.0 * PADDING;
        channel::is_dev().then(|| Area::new(PADDING, TITLE_BAR + CARD_TOP, width, CARD))
    }

    /// The color chosen for the shapes and the steps, as `Settings` keeps it, and
    /// the hue of the user's own.
    pub fn ink(&self) -> (usize, usize) {
        (self.ink.selected(), self.hue.hue())
    }

    /// Goes back to the color at `ink`.
    pub fn set_ink(&mut self, ink: usize) {
        self.ink.select(ink);
    }

    /// Shows the hue on the color of the user's own, as it changes.
    pub fn follow_hue(&mut self) {
        self.ink
            .set_color(INKS.len(), tinted(self.hue.hue() as f32));
    }

    /// Says how many free tries are left of what Premium adds here, where they
    /// count.
    pub fn count_tries(&mut self) -> Result<()> {
        let text = strings();
        let said = |perk: Perk| -> Result<Option<Label>> {
            (perk.tries())
                .map(|tries| self.fonts.caption(&text.tries(tries)))
                .transpose()
        };
        self.shortcut_tries = said(Perk::Shortcut)?;
        self.ink_tries = said(Perk::Color)?;
        Ok(())
    }

    /// Says Premium is the user's, or isn't: its card and its page follow.
    pub fn set_premium(&mut self, premium: bool) -> Result<()> {
        let text = strings();
        let fonts = &self.fonts;
        let card = if premium {
            text.premium_owned
        } else {
            text.premium_card
        };
        self.premium_title = fonts.label(card[0])?;
        self.premium_line = fonts.caption(card[1])?;
        self.premium_button = match premium {
            true => Button::standard(fonts.label(card[2])?),
            false => Button::new(fonts.label(card[2])?),
        };
        let [title, line, heading, way_on] = match premium {
            true => text.premium_thanks,
            false => [
                text.all_free,
                text.forever,
                text.premium_adds,
                text.stay_free,
            ],
        };
        self.all_free = fonts.title(title)?;
        self.forever = fonts.label(line)?;
        self.premium_adds = fonts.caption(heading)?;
        self.stay_free = Button::standard(fonts.label(way_on)?);
        self.premium = premium;
        self.count_tries()?;
        self.layout(self.room)
    }

    pub fn part_at(&self, page: Page, point: Point) -> Option<Part> {
        self.parts(page)
            .into_iter()
            .find(|&part| self.get(part).contains(point))
    }

    /// `page`'s controls, as assistive technologies know them, each numbered as
    /// its part.
    pub fn describe(&self, page: Page) -> Vec<Element> {
        let mut elements = Vec::new();
        for part in self.parts(page) {
            self.get(part).describe(part as usize, &mut elements);
        }
        self.caption.describe(&mut elements);
        elements
    }

    /// The window's minimize, maximize and close buttons.
    pub fn caption(&mut self) -> &mut Caption {
        &mut self.caption
    }

    /// Draws them: they stay put while a page comes in.
    pub fn draw_caption(&self, painter: &Painter) {
        self.caption.draw(painter);
    }

    pub fn control(&mut self, part: Part) -> &mut dyn Control {
        match part {
            Part::FirstCapture => &mut self.first_capture,
            Part::ReadyKeys => &mut self.ready_keys,
            Part::GoOn => &mut self.go_on,
            Part::StayFree => &mut self.stay_free,
            Part::GetPremium => &mut self.get_premium,
            Part::Capture => &mut self.capture,
            Part::OpenSettings => &mut self.open_settings,
            Part::Back => &mut self.back,
            Part::Premium => &mut self.premium_button,
            Part::Shortcut => &mut self.shortcut,
            Part::ScreenKeysInfo => &mut self.screen_keys_info,
            Part::ScreenKeys => &mut self.screen_keys,
            Part::After => &mut self.after,
            Part::ToolNames => &mut self.tool_names,
            Part::Ink => &mut self.ink,
            Part::Hue => &mut self.hue,
            Part::Sound => &mut self.sound,
            Part::Folder => &mut self.change_folder,
            Part::Startup => &mut self.startup,
        }
    }

    fn get(&self, part: Part) -> &dyn Control {
        match part {
            Part::FirstCapture => &self.first_capture,
            Part::ReadyKeys => &self.ready_keys,
            Part::GoOn => &self.go_on,
            Part::StayFree => &self.stay_free,
            Part::GetPremium => &self.get_premium,
            Part::Capture => &self.capture,
            Part::OpenSettings => &self.open_settings,
            Part::Back => &self.back,
            Part::Premium => &self.premium_button,
            Part::Shortcut => &self.shortcut,
            Part::ScreenKeysInfo => &self.screen_keys_info,
            Part::ScreenKeys => &self.screen_keys,
            Part::After => &self.after,
            Part::ToolNames => &self.tool_names,
            Part::Ink => &self.ink,
            Part::Hue => &self.hue,
            Part::Sound => &self.sound,
            Part::Folder => &self.change_folder,
            Part::Startup => &self.startup,
        }
    }

    /// Follows the pointer over `page`. Returns whether its look changes.
    pub fn hover(&mut self, page: Page, point: Option<Point>) -> bool {
        // Only the part under the pointer sees it: the info glyph sits on its setting.
        let under = point.and_then(|point| self.part_at(page, point));
        // The frame closes in on the capture button under the pointer.
        let framed = f32::from(u8::from(under == Some(Part::Capture)));
        if framed != self.framing.target() {
            let spring = Easing::SPRING;
            self.framing = self.framing.retarget(framed, motion::NORMAL, spring);
        }
        self.parts(page).into_iter().fold(false, |changed, part| {
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
        let controls = || {
            for part in self.parts(page) {
                self.get(part).draw(painter, now);
            }
        };
        match page {
            // Their controls come in in turn.
            Page::Welcome | Page::Ready | Page::Premium => {
                self.draw_title(painter);
                self.draw_welcome(page, painter, now);
            }
            Page::Home => {
                self.draw_title(painter);
                self.draw_frame(painter, now);
                self.draw_keys(painter);
                controls();
            }
            Page::Settings => {
                painter.text(&self.settings_title, BACK, self.title_row, palette.text);
                if let Some(card) = self.premium_card() {
                    self.draw_premium_card(painter, card);
                }
                let rows = self.rows();
                let mut first = 0;
                for (count, title) in GROUPS.into_iter().zip(&self.group_titles) {
                    let top = rows[first].top;
                    let middle = top - HEADING + (HEADING + PADDING) / 2.0;
                    painter.text(title, PADDING, middle, palette.text_secondary);
                    let card = Area::new(PADDING, top, rows[first].width, count as f32 * ROW);
                    let card = painter.align(card);
                    painter.fill(card, RADIUS, palette.card);
                    painter.stroke(card, RADIUS, painter.border(), palette.control_stroke);
                    // A hairline between a group's settings.
                    for row in &rows[first + 1..first + count] {
                        let line = Area::new(row.left, row.top, row.width, painter.border());
                        painter.fill(painter.align(line), 0.0, palette.control_stroke);
                    }
                    first += count;
                }
                for (row, icon) in rows.iter().zip(&self.row_icons) {
                    painter.text(icon, row.left + PADDING, row.middle(), palette.text);
                }
                let [shortcut, _, _, _, after, .., ink, folder] = rows;
                // A title, over how many free tries are left when they count.
                let titled = |title: &Label, tries: &Option<Label>, row: Area| {
                    let lines = title.height + tries.as_ref().map_or(0.0, |tries| tries.height);
                    let top = row.middle() - lines / 2.0;
                    let left = row.left + LEAD;
                    painter.text(title, left, top + title.height / 2.0, palette.text);
                    if let Some(tries) = tries {
                        let middle = top + title.height + tries.height / 2.0;
                        painter.text(tries, left, middle, palette.text_secondary);
                    }
                };
                titled(&self.shortcut_title, &self.shortcut_tries, shortcut);
                titled(&self.ink_title, &self.ink_tries, ink);
                titled(&self.after_title, &None, after);
                // The folder's title and path, one above the other.
                let (title, path) = (&self.folder_title, &self.folder_path);
                let top = folder.middle() - (title.height + path.height) / 2.0;
                let left = folder.left + LEAD;
                painter.text(title, left, top + title.height / 2.0, palette.text);
                let middle = top + title.height + path.height / 2.0;
                painter.text(path, left, middle, palette.text_secondary);
                controls();
            }
        }
        if let Some(part) = self.focused(page) {
            let (area, radius) = self.get(part).focus_area();
            painter.focus(area, radius);
        }
        if page == Page::Settings {
            self.screen_keys_info.draw_tip(painter, now);
        }
    }

    /// Premium's card: a crown, what Premium is in two lines, and the way to its
    /// page. Once Premium is the user's, the crown takes Kata's orange and the card
    /// a rim in Kata's own colors.
    fn draw_premium_card(&self, painter: &Painter, card: Area) {
        let palette = painter.palette;
        let card = painter.align(card);
        painter.fill(card, RADIUS, palette.card);
        painter.stroke(card, RADIUS, painter.border(), palette.control_stroke);
        let owned = self.premium && !palette.contrast;
        if owned {
            painter.rim(card, RADIUS, RIM, 1.0);
        }
        let ink = if owned {
            color(ORANGE, 1.0)
        } else {
            palette.text
        };
        let crown = (card.left + PADDING, card.middle() - ICON / 2.0);
        painter.figure(&self.crown, crown, ink);
        let (title, line) = (&self.premium_title, &self.premium_line);
        let top = card.middle() - (title.height + line.height) / 2.0;
        let left = card.left + LEAD;
        painter.text(title, left, top + title.height / 2.0, palette.text);
        let middle = top + title.height + line.height / 2.0;
        painter.text(line, left, middle, palette.text_secondary);
    }

    /// The shortcut, centered under `button`.
    fn draw_hint(&self, painter: &Painter, button: Area) {
        let left = (self.room.0 - self.hint.width) / 2.0;
        let middle = button.bottom() + HINT_GAP + self.hint.height / 2.0;
        painter.text(&self.hint, left, middle, painter.palette.text_secondary);
    }

    /// The shortcut, centered under `button`.    pub(super) fn draw_hint(&self, painter: &Painter, button: Area) {        let left = (self.room.0 - self.hint.width) / 2.0;        let middle = button.bottom() + HINT_GAP + self.hint.height / 2.0;        painter.text(&self.hint, left, middle, painter.palette.text_secondary);    }
    /// Kata's logo and name, at the start of the title bar.
    fn draw_title(&self, painter: &Painter) {
        let logo = Area::new(PADDING, self.title_row - ICON / 2.0, ICON, ICON);
        painter.logo(painter.align(logo), logo::DRAWING);
        let left = PADDING + ICON + ICON_GAP;
        painter.text(&self.title, left, self.title_row, painter.palette.text);
    }

    /// The keys that start a capture: Print Screen while Kata answers to Windows'
    /// capture keys, the shortcut otherwise.
    fn keys(&self) -> &[Label] {
        if self.screen_keys.is_on() {
            std::slice::from_ref(&self.print_screen)
        } else {
            &self.keys
        }
    }

    /// How wide `keys` are as keycaps in a row.
    fn keys_width(keys: &[Label]) -> f32 {
        let caps = keys
            .iter()
            .map(|key| KEY.max(key.width + 2.0 * KEY_PADDING));
        caps.sum::<f32>() + KEY_GAP * keys.len().saturating_sub(1) as f32
    }

    /// The keys, as keycaps centered under the capture button.
    fn draw_keys(&self, painter: &Painter) {
        let palette = painter.palette;
        let keys = self.keys();
        let top = self.capture.area().bottom() + HINT_GAP;
        let mut left = (self.room.0 - Self::keys_width(keys)) / 2.0;
        for key in keys {
            let width = KEY.max(key.width + 2.0 * KEY_PADDING);
            let cap = painter.align(Area::new(left, top, width, KEY));
            painter.fill(cap, RADIUS, palette.button);
            painter.stroke(cap, RADIUS, painter.border(), palette.control_stroke);
            let text = cap.left + (cap.width - key.width) / 2.0;
            painter.text(key, text, cap.middle(), palette.text_secondary);
            left += width + KEY_GAP;
        }
    }

    /// The logo's four corners around the capture button and its keys, as around a
    /// capture. Under the pointer they close in, as if focusing.
    fn draw_frame(&self, painter: &Painter, now: Instant) {
        let button = self.capture.area();
        let width = button.width.max(Self::keys_width(self.keys()));
        let closer = FOCUS * self.framing.value(now);
        let frame = Area::new(
            (self.room.0 - width) / 2.0 - FRAME.0 + closer,
            button.top - FRAME.1 + closer,
            width + 2.0 * (FRAME.0 - closer),
            HEIGHT + HINT_GAP + KEY + 2.0 * (FRAME.1 - closer),
        );
        painter.frame(frame, FRAME_ARM, FRAME_TURN, FRAME_STROKE);
    }

    /// Moves the keyboard focus to `page`'s next control, or the previous one if
    /// `back`.
    pub fn tab(&mut self, page: Page, back: bool) {
        let parts = self.parts(page);
        let focusable = |index: usize| self.get(parts[index]).is_focusable();
        self.focus = controls::tab(parts.len(), self.focus, back, focusable);
    }

    /// The control on `page` with the keyboard focus.
    pub fn focused(&self, page: Page) -> Option<Part> {
        let part = *self.parts(page).get(self.focus?)?;
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
        (page.is_welcome() && self.welcome_animating(page, now))
            || self.framing.is_running(now)
            || self
                .parts(page)
                .into_iter()
                .any(|part| self.get(part).is_animating(now))
    }

    pub fn set_hotkey(&mut self, hotkey: Hotkey) -> Result<()> {
        let keys = hotkey.keys();
        let keys = keys.iter().map(|key| self.fonts.label(key));
        self.shortcut.set_keys(keys.collect::<Result<_>>()?);
        self.hint = self.fonts.caption(&hotkey.label())?;
        let keys = hotkey.keys();
        let keys = keys.iter().map(|key| self.fonts.caption(key));
        self.keys = keys.collect::<Result<_>>()?;
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

    /// Brings the welcome's switch for Kata as the screenshot app and the settings'
    /// in line with `part`, whichever of them was switched. Returns whether it's on.
    pub fn switch_screen_keys(&mut self, part: Part) -> bool {
        let on = if part == Part::ReadyKeys {
            self.ready_keys.is_on()
        } else {
            self.screen_keys.is_on()
        };
        self.ready_keys.set(on);
        self.screen_keys.set(on);
        on
    }

    /// What follows a capture, as chosen.
    pub fn after(&self) -> After {
        After::ALL[self.after.selected()]
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

    /// The settings' rows, from top to bottom, each group under its heading.
    fn rows(&self) -> [Area; CARDS] {
        let width = self.room.0 - 2.0 * PADDING;
        let mut rows = [Area::default(); CARDS];
        let above = self
            .premium_card()
            .map_or(TITLE_BAR, |card| card.bottom() + CARD_TOP);
        let (mut row, mut top) = (0, above - PADDING);
        for count in GROUPS {
            top += HEADING;
            for _ in 0..count {
                rows[row] = Area::new(PADDING, top, width, ROW);
                row += 1;
                top += ROW;
            }
        }
        rows
    }

    /// Fits the folder's path between its card's edge and the Change button.
    fn trim_folder(&mut self) -> Result<()> {
        let left = self.rows()[CARDS - 1].left + LEAD;
        let width = self.change_folder.area().left - PADDING - left;
        let path = self.folder.to_string_lossy();
        self.folder_path = self.fonts.path(&path, width)?;
        Ok(())
    }
}

/// A crown, `ICON` DIPs square: what Premium is drawn as.
fn crown(graphics: &Graphics) -> Result<Figure> {
    Figure::new(graphics, |pen| {
        pen.start((1.5, 4.5));
        pen.line((5.0, 8.0));
        pen.line((8.0, 2.5));
        pen.line((11.0, 8.0));
        pen.line((14.5, 4.5));
        pen.line((13.5, 13.5));
        pen.line((2.5, 13.5));
        pen.close();
    })
}

fn parts(page: Page) -> &'static [Part] {
    match page {
        Page::Welcome => &[Part::FirstCapture],
        Page::Ready => &[Part::ReadyKeys, Part::GoOn],
        Page::Premium => &[Part::StayFree, Part::GetPremium],
        Page::Home => &[Part::Capture, Part::OpenSettings],
        Page::Settings => &[
            Part::Back,
            Part::Premium,
            Part::Shortcut,
            // Before its setting, which it sits on.
            Part::ScreenKeysInfo,
            Part::ScreenKeys,
            Part::Sound,
            Part::Startup,
            Part::After,
            Part::ToolNames,
            // Before the colors, which it sits beside.
            Part::Hue,
            Part::Ink,
            Part::Folder,
        ],
    }
}
