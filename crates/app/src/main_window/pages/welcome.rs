//! The welcome's pages, which lead a new user through Kata: the Welcome, with the
//! invitation to a first capture; Ready, where that capture comes back as its card;
//! and Premium, which says what's free and what Premium adds. Positions and sizes
//! are in DIPs, and times in seconds since the page came in.

use std::time::{Duration, Instant};

use kata_ui::{
    controls::{Control, HEIGHT},
    motion::{Animation, Easing},
    paint::{Area, Label, Painter},
    theme::{color, signature},
};
use windows::{
    Win32::Graphics::Direct2D::{ID2D1Bitmap1, ID2D1DeviceContext},
    core::Result,
};

use super::{CARD, HINT_GAP, PADDING, Page, Pages, RADIUS, TITLE_BAR};
use crate::{
    card::{self, Pose, Thumbnail},
    strings::strings,
};

/// The Welcome's logo, and the room under it and before the invitation.
const LOGO: f32 = 64.0;
const LOGO_GAP: f32 = 20.0;
const INVITATION_GAP: f32 = 24.0;
/// Between a title and the line under it.
const LINE_GAP: f32 = 4.0;
/// Ready's room for the card, and under it; before the setting; and before the
/// way on.
const CARD_SLOT: f32 = card::CARD.1;
const CARD_GAP: f32 = 32.0;
const SETTING_GAP: f32 = 24.0;
const GO_ON_GAP: f32 = 24.0;
/// Premium's room: before what it adds, under its heading, for each perk, for a
/// perk's glyph and after it, under the last, before the ways on and between them.
const PERKS_GAP: f32 = 24.0;
const ADDS_GAP: f32 = 4.0;
const PERK_ROW: f32 = 32.0;
pub(super) const GLYPH: f32 = 16.0;
const GLYPH_GAP: f32 = 12.0;
const PERKS_END: f32 = 8.0;
const CHOICE_GAP: f32 = 24.0;
const BUTTON_GAP: f32 = 8.0;

/// While the window or the page comes in, nothing else moves.
const LEAD: f32 = 0.25;
/// The Welcome: the logo draws itself; the greeting rises below it, then the
/// invitation; and a glint crosses the greeting and the logo's light.
const GREETING: f32 = LEAD + 0.55;
const FREE: f32 = GREETING + 0.1;
const INVITATION: f32 = LEAD + 0.85;
const GREETING_GLINT: f32 = LEAD + 1.0;
const WELCOME: f32 = GREETING_GLINT + GLINTING;
/// Ready: the card rises and lifts off, and a glint crosses it; meanwhile how long
/// the capture took rises, then how much lighter it is, the setting and the way on.
const LIFT: f32 = LEAD;
const LIFTING: f32 = 0.6;
const CARD_GLINT: f32 = LIFT + 0.45;
const CARD_GLINTING: f32 = 0.8;
const TOOK: f32 = LIFT + 0.3;
const LIGHTER: f32 = TOOK + 0.1;
const SETTING: f32 = TOOK + 0.3;
const GO_ON: f32 = SETTING + 0.15;
/// The card's glint ends last.
const READY: f32 = CARD_GLINT + CARD_GLINTING;
/// Premium: that all is free rises, and a glint crosses it; then what Premium adds,
/// a perk after another; and the ways on.
const ALL_FREE: f32 = LEAD + 0.1;
const FOREVER: f32 = ALL_FREE + 0.1;
const PERKS: f32 = FOREVER + 0.15;
const PERK: f32 = 0.08;
const CHOICE: f32 = PERKS + 0.45;
const FREE_GLINT: f32 = ALL_FREE + 0.4;
const PREMIUM: f32 = FREE_GLINT + GLINTING;
/// What rises takes this long, from this far below.
const RISING: f32 = 0.4;
const RISE: f32 = 8.0;
/// The card starts this far below its place, lying flat.
const CARD_RISE: f32 = 24.0;
/// How long a glint takes over a title, and how wide its band is.
const GLINTING: f32 = 1.1;
const BAND: f32 = 96.0;

/// Kata's first capture, for Ready: as it was left, how long it took to show, and
/// how much lighter it is than with Windows, if weighed yet.
pub struct Ready {
    pub thumbnail: Thumbnail,
    pub seconds: f32,
    pub saved: Option<f32>,
}

/// The first capture's card, with its copy on the graphics card once drawn.
pub(super) struct First {
    thumbnail: Thumbnail,
    bitmap: Option<ID2D1Bitmap1>,
}

impl Pages {
    /// Arranges the welcome's pages in `self.room`, each centered below the title bar.
    pub(super) fn layout_welcome(&mut self) {
        let room = self.room;
        let top = |group: f32| TITLE_BAR + (room.1 - TITLE_BAR - group) / 2.0;
        let centered = |width: f32| (room.0 - width) / 2.0;

        let above = LOGO + LOGO_GAP + self.greeting.height + LINE_GAP + self.free.height;
        let group = above + INVITATION_GAP + HEIGHT + HINT_GAP + self.hint.height;
        let welcome = top(group);
        self.logo = Area::new(centered(LOGO), welcome, LOGO, LOGO);
        let invitation = &mut self.first_capture;
        invitation.place(
            centered(invitation.width()),
            welcome + above + INVITATION_GAP,
        );

        let above = CARD_SLOT + CARD_GAP + self.took.height + LINE_GAP + self.lighter.height;
        let below = SETTING_GAP + CARD + GO_ON_GAP + HEIGHT + HINT_GAP + self.stays_here.height;
        self.ready_top = top(above + below);
        let setting = Area::new(
            PADDING,
            self.ready_top + above + SETTING_GAP,
            room.0 - 2.0 * PADDING,
            CARD,
        );
        self.ready_setting = setting;
        self.ready_keys.place(setting.left + PADDING, setting.top);
        self.ready_keys
            .stretch(setting.width - 2.0 * PADDING, setting.height);
        let go_on = &mut self.go_on;
        go_on.place(centered(go_on.width()), setting.bottom() + GO_ON_GAP);

        let heading = PADDING + self.premium_adds.height + ADDS_GAP;
        let perks = heading + self.perks.len() as f32 * PERK_ROW + PERKS_END;
        let above = self.all_free.height + LINE_GAP + self.forever.height + PERKS_GAP;
        self.premium_top = top(above + perks + CHOICE_GAP + HEIGHT);
        self.perks_card = Area::new(
            PADDING,
            self.premium_top + above,
            room.0 - 2.0 * PADDING,
            perks,
        );
        // As wide as each other, so neither way on weighs more.
        let width = self.stay_free.width().max(self.get_premium.width());
        let left = centered(2.0 * width + BUTTON_GAP);
        let top = self.perks_card.bottom() + CHOICE_GAP;
        for (index, button) in [&mut self.stay_free, &mut self.get_premium]
            .into_iter()
            .enumerate()
        {
            button.widen(width);
            button.place(left + index as f32 * (width + BUTTON_GAP), top);
        }
    }

    /// The width the welcome's pages need inside a card: Ready's setting, and
    /// Premium's perks and ways on.
    pub(super) fn welcome_width(&self) -> f32 {
        let perks = self
            .perks
            .iter()
            .map(|(_, perk)| perk.width)
            .fold(0.0, f32::max);
        let choice = 2.0 * self.stay_free.width().max(self.get_premium.width()) + BUTTON_GAP;
        self.ready_keys
            .width()
            .max(GLYPH + GLYPH_GAP + perks)
            .max(choice - 2.0 * PADDING)
    }

    /// Draws `page`, one of the welcome's, its parts coming in in turn.
    pub(super) fn draw_welcome(&self, page: Page, painter: &Painter, now: Instant) {
        match page {
            Page::Welcome => self.draw_greeting(painter, now),
            Page::Ready => self.draw_ready(painter, now),
            _ => self.draw_premium(painter, now),
        }
    }

    /// The logo draws itself, the greeting rises below it, and a glint crosses them;
    /// then comes the invitation, with the shortcut under it.
    fn draw_greeting(&self, painter: &Painter, now: Instant) {
        let palette = painter.palette;
        let time = self.clock.value(now);
        painter.logo(painter.align(self.logo), time - LEAD);
        let (greeting, free) = (&self.greeting, &self.free);
        let left = self.centered(greeting);
        let middle = self.logo.bottom() + LOGO_GAP + greeting.height / 2.0;
        self.rise(painter, GREETING, now, || {
            painter.text(greeting, left, middle, palette.text);
        });
        let below = middle + greeting.height / 2.0 + LINE_GAP + free.height / 2.0;
        self.rise(painter, FREE, now, || {
            painter.text(free, self.centered(free), below, palette.text_secondary);
        });
        self.glint(painter, greeting, left, middle, time - GREETING_GLINT);
        self.rise(painter, INVITATION, now, || {
            self.first_capture.draw(painter, now);
            self.draw_hint(painter, self.first_capture.area());
        });
    }

    /// The first capture rises as its card, lifting off, and a glint crosses it; how
    /// long it took and how much lighter it is rise below it, then the setting that
    /// makes Kata the screenshot app, and the way on with what it promises.
    fn draw_ready(&self, painter: &Painter, now: Instant) {
        let palette = painter.palette;
        let time = self.clock.value(now);
        if let Some(First {
            thumbnail,
            bitmap: Some(bitmap),
        }) = &self.first
        {
            // Pixel for pixel, as far as it fits.
            let size = (
                thumbnail.size.0 as f32 * painter.pixel(),
                thumbnail.size.1 as f32 * painter.pixel(),
            );
            let center = (self.room.0 / 2.0, self.ready_top + CARD_SLOT / 2.0);
            let lifted = Pose::card(size, center);
            let lifting = (time - LIFT) / LIFTING;
            let laid = lifted.laid().moved((0.0, CARD_RISE));
            let pose = laid.toward(lifted, Easing::DECELERATE.apply(lifting));
            let sweep = (time - CARD_GLINT) / CARD_GLINTING;
            let glint = (sweep > 0.0 && sweep < 1.0).then(|| Easing::SWEEP.apply(sweep));
            let opacity = Easing::DECELERATE.apply(2.0 * lifting);
            let kept = thumbnail.image.bounds();
            painter.lowered(opacity, 0.0, || {
                card::draw(painter, bitmap, kept, pose, glint)
            });
        }
        let took = self.ready_top + CARD_SLOT + CARD_GAP + self.took.height / 2.0;
        self.rise(painter, TOOK, now, || {
            painter.text(&self.took, self.centered(&self.took), took, palette.text);
        });
        if let Some(rise) = self.lighter_rise {
            let lighter = &self.lighter;
            let middle = took + self.took.height / 2.0 + LINE_GAP + lighter.height / 2.0;
            risen(painter, rise.value(now) / RISING, || {
                let left = self.centered(lighter);
                painter.text(lighter, left, middle, palette.text_secondary);
            });
        }
        self.rise(painter, SETTING, now, || {
            let card = painter.align(self.ready_setting);
            painter.fill(card, RADIUS, palette.card);
            painter.stroke(card, RADIUS, painter.border(), palette.control_stroke);
            self.ready_keys.draw(painter, now);
        });
        self.rise(painter, GO_ON, now, || {
            self.go_on.draw(painter, now);
            let below = self.go_on.area().bottom() + HINT_GAP + self.stays_here.height / 2.0;
            let left = self.centered(&self.stays_here);
            painter.text(&self.stays_here, left, below, palette.text_secondary);
        });
    }

    /// That all is free rises, and a glint crosses it; then what Premium adds, a perk
    /// after another, each with its glyph in one of Kata's own colors; and the ways
    /// on, side by side.
    fn draw_premium(&self, painter: &Painter, now: Instant) {
        let palette = painter.palette;
        let time = self.clock.value(now);
        let (all_free, forever) = (&self.all_free, &self.forever);
        let left = self.centered(all_free);
        let middle = self.premium_top + all_free.height / 2.0;
        self.rise(painter, ALL_FREE, now, || {
            painter.text(all_free, left, middle, palette.text);
        });
        let below = middle + all_free.height / 2.0 + LINE_GAP + forever.height / 2.0;
        self.rise(painter, FOREVER, now, || {
            let left = self.centered(forever);
            painter.text(forever, left, below, palette.text_secondary);
        });
        self.glint(painter, all_free, left, middle, time - FREE_GLINT);
        let colors = signature(palette.light);
        self.rise(painter, PERKS, now, || {
            let card = painter.align(self.perks_card);
            painter.fill(card, RADIUS, palette.card);
            painter.stroke(card, RADIUS, painter.border(), palette.control_stroke);
            let left = self.perks_card.left + PADDING;
            let adds = &self.premium_adds;
            let top = self.perks_card.top + PADDING;
            painter.text(adds, left, top + adds.height / 2.0, palette.text_secondary);
            let top = top + adds.height + ADDS_GAP;
            for (index, (glyph, perk)) in self.perks.iter().enumerate() {
                let middle = top + (index as f32 + 0.5) * PERK_ROW;
                let start = PERKS + (index + 1) as f32 * PERK;
                // In a contrast theme, the system's colors.
                let ink = if palette.contrast {
                    palette.text
                } else {
                    color(colors[index], 1.0)
                };
                self.rise(painter, start, now, || {
                    painter.text(glyph, left + (GLYPH - glyph.width) / 2.0, middle, ink);
                    painter.text(perk, left + GLYPH + GLYPH_GAP, middle, palette.text);
                });
            }
        });
        self.rise(painter, CHOICE, now, || {
            self.stay_free.draw(painter, now);
            self.get_premium.draw(painter, now);
        });
    }

    /// A glint crossing `title`, drawn from `left` and centered on `middle`, `time`
    /// seconds after it set off.
    fn glint(&self, painter: &Painter, title: &Label, left: f32, middle: f32, time: f32) {
        let sweep = time / GLINTING;
        if sweep > 0.0 && sweep < 1.0 {
            let x = left - BAND / 2.0 + (title.width + BAND) * Easing::SWEEP.apply(sweep);
            painter.glint(&[(title, left)], middle, x, BAND);
        }
    }

    /// Draws what `draw` paints as it rises into place, from `start` on.
    fn rise(&self, painter: &Painter, start: f32, now: Instant, draw: impl FnOnce()) {
        risen(painter, (self.clock.value(now) - start) / RISING, draw);
    }

    /// Where `label` starts, centered across the window.
    fn centered(&self, label: &Label) -> f32 {
        (self.room.0 - label.width) / 2.0
    }

    /// Plays `page` from its start, if it's one of the welcome's.
    pub fn start(&mut self, page: Page) {
        let length = match page {
            Page::Welcome => WELCOME,
            Page::Ready => READY,
            Page::Premium => PREMIUM,
            Page::Home | Page::Settings => return,
        };
        self.clock = Animation::clock(length);
        if page == Page::Ready
            && let Some(rise) = &mut self.lighter_rise
        {
            *rise = Animation::clock(RISING).delayed(Duration::from_secs_f32(LIGHTER));
        }
    }

    /// Whether the welcome's `page` is still coming in.
    pub(super) fn welcome_animating(&self, page: Page, now: Instant) -> bool {
        self.clock.is_running(now)
            || (page == Page::Ready && self.lighter_rise.is_some_and(|rise| rise.is_running(now)))
    }

    /// Has Ready show the first capture, as `ready` tells.
    pub fn set_ready(&mut self, ready: Ready) -> Result<()> {
        self.took = self.fonts.title(&strings().ready(ready.seconds))?;
        self.first = Some(First {
            thumbnail: ready.thumbnail,
            bitmap: None,
        });
        self.lighter_rise = None;
        match ready.saved {
            Some(saved) => self.weighed(saved),
            None => Ok(()),
        }
    }

    /// Has Ready say the first capture is `saved` lighter than with Windows, rising
    /// in its turn, or at once if that's gone by. Under half a percent, it says
    /// nothing.
    pub fn weighed(&mut self, saved: f32) -> Result<()> {
        if saved < 0.005 {
            return Ok(());
        }
        self.lighter = self.fonts.label(&strings().lighter(saved))?;
        let wait = (LIGHTER - self.clock.value(Instant::now())).max(0.0);
        self.lighter_rise = Some(Animation::clock(RISING).delayed(Duration::from_secs_f32(wait)));
        Ok(())
    }

    /// Copies the first capture to the graphics card that `context` draws on, if it
    /// isn't there yet.
    pub fn upload(&mut self, context: &ID2D1DeviceContext) -> Result<()> {
        if let Some(first) = &mut self.first
            && first.bitmap.is_none()
        {
            first.bitmap = Some(card::bitmap(context, &first.thumbnail.image)?);
        }
        Ok(())
    }

    /// Forgets the copies on a graphics card that went away.
    pub fn forget_device(&mut self) {
        if let Some(first) = &mut self.first {
            first.bitmap = None;
        }
    }
}

/// Draws what `draw` paints `progress` of the way into place, rising.
fn risen(painter: &Painter, progress: f32, draw: impl FnOnce()) {
    let risen = Easing::DECELERATE.apply(progress);
    painter.lowered(risen, (1.0 - risen) * RISE, draw);
}
