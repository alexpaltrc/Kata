//! The capture's file name, centered in the title bar. Clicking it opens a box to
//! type the name in, after the date it keeps; dragging it carries the file away.

use std::time::{Duration, Instant};

use kata_editor::name::{self, Field};
use kata_ui::{
    access::{Element, Role},
    controls::{Action, Control, HEIGHT},
    motion::{self, Animation, Easing},
    paint::{Area, Fonts, Label, Painter, Point},
    theme::faded,
    window,
};
use windows::{
    Win32::UI::{
        Input::KeyboardAndMouse::{
            VIRTUAL_KEY, VK_A, VK_BACK, VK_DELETE, VK_END, VK_HOME, VK_LEFT, VK_RIGHT, VK_SHIFT,
        },
        WindowsAndMessaging::GetCaretBlinkTime,
    },
    core::Result,
};

use crate::strings::strings;

/// Between the box's edges and the text.
const PADDING: f32 = 11.0;
/// How wide the box opens, room allowing.
const WIDTH: f32 = 320.0;
/// The least room the name has in the title bar.
pub const MIN: f32 = 120.0;
const RADIUS: f32 = 4.0;
/// How long the caret blinks after it last moved, as in Windows' own text boxes.
const STEADY: Duration = Duration::from_secs(5);

pub struct Name {
    fonts: Fonts,
    /// When the capture was taken, as its file was first named. Empty without a
    /// file, which leaves nothing to name.
    taken: String,
    /// The file's name, without its extension.
    stem: String,
    /// The file's name, shortened to the room there is.
    shown: Label,
    /// The date every name starts with, and how far after it the name goes.
    prefix: Label,
    lead: f32,
    /// What the box says while it's empty.
    hint: Label,
    /// The name as it's typed, while the box is open.
    field: Option<Field>,
    typed: Label,
    /// The first half of a character typed in two UTF-16 units.
    high: Option<u16>,
    /// How far the text has scrolled to keep the caret in sight.
    scroll: f32,
    /// From 0 at rest to 1 with the box open.
    open: Animation,
    caret: bool,
    /// When the caret last moved, which shows it at once.
    moved: Instant,
    hovered: bool,
    /// Where the pointer went down on it.
    pressed: Option<Point>,
    /// The room in the title bar, from left to right, and the point it centers on.
    room: (f32, f32),
    center: Point,
}

impl Name {
    /// Names the capture whose file is called `stem`, taken at `taken`, if it has a
    /// file.
    pub fn new(fonts: &Fonts, names: Option<(String, String)>) -> Result<Self> {
        let (taken, stem) = names.unwrap_or_default();
        let prefix = fonts.label(&name::prefix(&taken))?;
        Ok(Self {
            fonts: fonts.clone(),
            shown: fonts.label(&stem)?,
            lead: prefix.offset(prefix.text.encode_utf16().count()),
            prefix,
            hint: fonts.label(strings().name)?,
            field: None,
            typed: fonts.label("")?,
            high: None,
            scroll: 0.0,
            open: Animation::idle(0.0),
            caret: true,
            moved: Instant::now(),
            hovered: false,
            pressed: None,
            room: (0.0, 0.0),
            center: (0.0, 0.0),
            taken,
            stem,
        })
    }

    /// Centers it on `center`, within the room from `left` to `right`.
    pub fn place(&mut self, left: f32, right: f32, center: Point) {
        (self.room, self.center) = ((left, right), center);
        self.fit();
    }

    pub fn is_editing(&self) -> bool {
        self.field.is_some()
    }

    /// Opens the box, with the name selected.
    pub fn edit(&mut self) {
        let field = Field::new(name::name(&self.taken, &self.stem));
        (self.scroll, self.high) = (0.0, None);
        self.field = Some(field);
        self.open = self.open.retarget(1.0, motion::FAST, Easing::DECELERATE);
        self.edited();
    }

    /// Closes the box. Returns the file's new name, if it changed.
    pub fn finish(&mut self) -> Option<String> {
        let field = self.close()?;
        let stem = name::stem(&self.taken, &field.text());
        (stem != self.stem).then_some(stem)
    }

    /// Closes the box, forgetting what was typed.
    pub fn discard(&mut self) {
        self.close();
    }

    /// The file took the name `stem`.
    pub fn set(&mut self, stem: String) {
        self.stem = stem;
        self.fit();
    }

    /// Follows `key`, held with Ctrl or Shift. Returns whether that changed anything.
    pub fn key(&mut self, key: VIRTUAL_KEY, ctrl: bool, shift: bool) -> bool {
        let Some(field) = &mut self.field else {
            return false;
        };
        let moved = match key {
            VK_LEFT | VK_RIGHT => field.step(key == VK_RIGHT, ctrl, shift),
            VK_HOME => field.go(0, shift),
            VK_END => field.go(field.len(), shift),
            VK_A if ctrl => field.select_all(),
            VK_BACK | VK_DELETE => {
                let erased = field.erase(key == VK_DELETE, ctrl);
                if erased {
                    self.edited();
                }
                return erased;
            }
            _ => return false,
        };
        if moved {
            self.moved();
        }
        moved
    }

    /// Types `unit`, half a character or all of it, as `WM_CHAR` brings it. Returns
    /// whether that changed anything.
    pub fn type_unit(&mut self, unit: u16) -> bool {
        let text = match (self.high.take(), unit) {
            (_, 0xD800..=0xDBFF) => {
                self.high = Some(unit);
                return false;
            }
            (Some(high), 0xDC00..=0xDFFF) => String::from_utf16_lossy(&[high, unit]),
            (_, unit) => char::from_u32(unit.into())
                .map(String::from)
                .unwrap_or_default(),
        };
        self.insert(&text)
    }

    /// Types `text` over the selection. Returns whether that changed anything.
    pub fn insert(&mut self, text: &str) -> bool {
        let inserted = self.field.as_mut().is_some_and(|field| field.insert(text));
        if inserted {
            self.edited();
        }
        inserted
    }

    /// What's selected, if anything is.
    pub fn selected(&self) -> Option<String> {
        let text = self.field.as_ref()?.selected();
        (!text.is_empty()).then_some(text)
    }

    /// Erases what's selected, if anything is, and returns it.
    pub fn cut(&mut self) -> Option<String> {
        let text = self.selected()?;
        self.field.as_mut()?.erase(true, false);
        self.edited();
        Some(text)
    }

    /// Where the pointer went down on the name at rest, to drag the file away from.
    pub fn grabbed(&self) -> Option<Point> {
        self.pressed.filter(|_| !self.is_editing())
    }

    /// The caret blinks: it hides if it showed, or shows if it hid. Returns whether it
    /// keeps blinking.
    pub fn blink(&mut self, now: Instant) -> bool {
        let blinking = self.is_editing() && now - self.moved < STEADY;
        self.caret = !blinking || !self.caret;
        blinking
    }

    /// The name's area at rest.
    fn rest(&self) -> Area {
        self.area(self.shown.width + 2.0 * PADDING)
    }

    /// The box's area, open.
    fn opened(&self) -> Area {
        let (left, right) = self.room;
        let width = WIDTH.max(self.rest().width).min(right - left);
        self.area(width)
    }

    /// Centered, `width` DIPs wide, as far as the room allows.
    fn area(&self, width: f32) -> Area {
        let (left, right) = self.room;
        let (x, y) = self.center;
        let left = (x - width / 2.0).min(right - width).max(left);
        Area::new(left, y - HEIGHT / 2.0, width, HEIGHT)
    }

    /// Where the name starts, past the date, with the box open.
    fn start(&self) -> f32 {
        self.opened().left + PADDING - self.scroll + self.lead
    }

    /// Shortens the name to the room there is.
    fn fit(&mut self) {
        let (left, right) = self.room;
        if let Ok(shown) = self.fonts.line(&self.stem, right - left - 2.0 * PADDING) {
            self.shown = shown;
        }
        self.reveal();
    }

    fn close(&mut self) -> Option<Field> {
        let field = self.field.take()?;
        self.open = self.open.retarget(0.0, motion::FAST, Easing::DECELERATE);
        Some(field)
    }

    /// The text changed.
    fn edited(&mut self) {
        if let Some(field) = &self.field
            && let Ok(typed) = self.fonts.label(&field.text())
        {
            self.typed = typed;
        }
        self.moved();
    }

    /// The caret or the selection moved.
    fn moved(&mut self) {
        (self.caret, self.moved) = (true, Instant::now());
        self.reveal();
    }

    /// Scrolls the text just enough to show the caret, leaving no room after the end.
    fn reveal(&mut self) {
        let Some(field) = &self.field else {
            return;
        };
        let room = self.opened().width - 2.0 * PADDING;
        let caret = self.lead + self.typed.offset(field.units(field.caret()));
        let end = self.lead + self.typed.offset(field.units(field.len()));
        let most = caret.min((end - room).max(0.0));
        self.scroll = self.scroll.max(caret - room).min(most).max(0.0);
    }

    /// The character the caret goes before when the pointer is down at `x`.
    fn index_at(&self, x: f32) -> usize {
        let Some(field) = &self.field else {
            return 0;
        };
        field.index(self.typed.position(x - self.start()))
    }

    fn draw_field(&self, painter: &Painter, field: &Field, middle: f32) {
        let palette = painter.palette;
        let start = self.start();
        painter.text(
            &self.prefix,
            start - self.lead,
            middle,
            palette.text_secondary,
        );
        if field.is_empty() {
            painter.text(&self.hint, start, middle, palette.text_secondary);
        } else {
            painter.text(&self.typed, start, middle, palette.text);
        }
        let height = self.typed.height;
        let top = middle - height / 2.0;
        let x = |index: usize| start + self.typed.offset(field.units(index));
        let selection = field.selection();
        if !selection.is_empty() {
            let (from, to) = (x(selection.start), x(selection.end));
            let area = painter.align(Area::new(from.min(to), top, (to - from).abs(), height));
            painter.fill(area, 0.0, palette.accent);
            painter.clip(area, || {
                painter.text(&self.typed, start, middle, palette.on_accent);
            });
        } else if self.caret {
            let caret = Area::new(
                painter.snap(x(field.caret())),
                top,
                painter.border(),
                height,
            );
            painter.fill(caret, 0.0, palette.text);
        }
    }
}

impl Control for Name {
    fn contains(&self, point: Point) -> bool {
        let area = if self.is_editing() {
            self.opened()
        } else {
            self.rest()
        };
        !self.taken.is_empty() && area.contains(point)
    }

    fn hover(&mut self, point: Option<Point>) -> bool {
        let hovered = point.is_some_and(|point| self.contains(point));
        std::mem::replace(&mut self.hovered, hovered) != hovered
    }

    /// With the box open, puts the caret where the pointer is, selecting up to it
    /// with Shift.
    fn press(&mut self, point: Point) -> bool {
        self.pressed = Some(point);
        let index = self.index_at(point.0);
        if let Some(field) = &mut self.field {
            field.go(index, window::held(VK_SHIFT));
            self.moved();
        }
        false
    }

    /// With the box open, selects up to where the pointer is.
    fn drag(&mut self, (x, _): Point) -> bool {
        let index = self.index_at(x);
        let Some(field) = &mut self.field else {
            return false;
        };
        let moved = field.go(index, true);
        if moved {
            self.moved();
        }
        moved
    }

    fn release(&mut self, point: Point) -> bool {
        let activated = self.pressed.take().is_some() && !self.is_editing() && self.contains(point);
        self.hovered = self.contains(point);
        activated
    }

    fn cancel(&mut self) {
        self.pressed = None;
    }

    fn is_focusable(&self) -> bool {
        !self.taken.is_empty() && !self.is_editing()
    }

    fn focus_area(&self) -> (Area, f32) {
        (self.rest(), RADIUS)
    }

    fn act(&mut self, action: Action) -> bool {
        action == Action::Activate && self.is_focusable()
    }

    fn describe(&self, control: usize, elements: &mut Vec<Element>) {
        if !self.taken.is_empty() {
            elements.push(Element {
                control,
                name: self.stem.clone(),
                role: Role::Button { enabled: true },
                area: self.rest(),
                help: strings().rename,
            });
        }
    }

    fn draw(&self, painter: &Painter, now: Instant) {
        if self.taken.is_empty() {
            return;
        }
        let palette = painter.palette;
        let open = self.open.value(now);
        let (rest, opened) = (self.rest(), self.opened());
        let lerp = |from: f32, to: f32| from + (to - from) * open;
        let area = painter.align(Area::new(
            lerp(rest.left, opened.left),
            rest.top,
            lerp(rest.width, opened.width),
            HEIGHT,
        ));
        // At rest, a subtle button.
        let shade = if self.pressed.is_some() && self.hovered {
            palette.subtle_pressed
        } else if self.hovered {
            palette.subtle_hover
        } else {
            Default::default()
        };
        painter.fill(area, RADIUS, faded(shade, 1.0 - open));
        // Open, a text box, underlined in the accent color while it's typed in.
        if open > 0.0 {
            painter.fill(area, RADIUS, faded(palette.input, open));
            let stroke = faded(palette.control_stroke, open);
            painter.stroke(area, RADIUS, painter.border(), stroke);
            let line = Area::new(area.left, area.bottom() - 2.0, area.width, 2.0);
            painter.clip(line, || {
                painter.fill(area, RADIUS, faded(palette.accent, open));
            });
        }
        let inside = Area::new(area.left + 1.0, area.top, area.width - 2.0, area.height);
        painter.clip(inside, || match &self.field {
            Some(field) => self.draw_field(painter, field, area.middle()),
            None => painter.text(
                &self.shown,
                area.left + PADDING,
                area.middle(),
                palette.text,
            ),
        });
    }

    fn is_animating(&self, now: Instant) -> bool {
        self.open.is_running(now)
    }
}

/// How often the caret blinks, in milliseconds, if it does.
pub fn blink_time() -> Option<u32> {
    let time = unsafe { GetCaretBlinkTime() };
    (time != 0 && time != u32::MAX).then_some(time)
}
