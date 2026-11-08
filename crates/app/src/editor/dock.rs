//! Docking another capture: dragged over the editor from File Explorer, it lights up
//! a zone on the side of the capture it would dock against, as windows snap side by
//! side, and let go there it lands as the captures make room. Undoing it, or making
//! it again, plays out the same way.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use kata_editor::{Collage, Image, Rect, Side};
use kata_ui::{
    motion::{self, Animation, Easing},
    paint::{Area, Painter, Point},
    theme::{self, color, faded},
};
use windows::Win32::{
    Foundation::{LPARAM, WPARAM},
    Graphics::Direct2D::{Common::D2D1_COLOR_F, ID2D1Bitmap1},
    UI::WindowsAndMessaging::WM_APP,
};

use super::{Editor, FLIGHT, View, WHITE, crop::along};
use crate::{
    card::{self, Pose},
    import::{MOST_ACROSS, Phase},
    report,
};

/// The message the editor hears a drag over it by, a step at a time.
pub const DOCK: u32 = WM_APP + 1;
/// The white between two captures, in DIPs at the scale the first was taken.
const GAP: f32 = 16.0;
/// The most pixels the captures take together: four 4K screens, and then some.
const MOST_PIXELS: i64 = 64 << 20;
/// How much the zone's fill shows, over light themes and dark.
const ZONE_LIGHT: f32 = 0.16;
const ZONE_DARK: f32 = 0.22;
/// How long the zone takes to glide to where the capture would land.
const ZONE_GLIDE: Duration = motion::NORMAL;
/// The landing, in seconds: the captures making room, the card settling with a
/// spring, the zone closing on where it lands, and the corners opening out after.
const MAKE_ROOM: f32 = 0.45;
const SETTLE: f32 = 0.55;
const CLOSE: f32 = 0.3;
const CORNERS_AT: f32 = 0.45;
const CORNERS: f32 = 0.35;
const LANDING: f32 = CORNERS_AT + CORNERS;
/// How much a capture shrinks as it leaves, or grows as it comes back.
const SHRINK: f32 = 0.06;

/// Another capture, dragged over the editor.
pub struct Incoming {
    image: Arc<Image>,
    /// Its copy on the graphics card, for its card.
    bitmap: Option<ID2D1Bitmap1>,
    /// The pointer, in DIPs.
    at: Point,
    /// From 0 to 1 as its card shows at the pointer, and back as it leaves.
    shown: Animation,
    /// The glint that crosses the card as it shows.
    glint: Animation,
    /// Where it docks if let go now, if it fits there.
    place: Option<Place>,
    zone: Option<Zone>,
}

/// Where a capture docks: against a side of one of the pieces, or of them all.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Place {
    target: Option<usize>,
    side: Side,
}

/// The zone lit where the capture would land: gliding from `from` to the half of
/// `place` it lands in, `travel` of the way, and showing `shown` as much.
struct Zone {
    place: Place,
    from: Area,
    travel: Animation,
    shown: Animation,
}

/// The captures moving to where they go, as one docks, leaves or comes back.
pub struct Landing {
    /// Seconds since it started.
    clock: Animation,
    /// The captures as they were, for those that leave.
    before: Option<ID2D1Bitmap1>,
    /// Where the captures and the white between them showed.
    white: Area,
    /// Those that stay: their part of the image now, and where they showed.
    moves: Vec<(Rect, Area)>,
    /// Those that leave: their part of the image before, and where they showed.
    leaving: Vec<(Rect, Area)>,
    /// Those that come: their part of the image now, and the card the one let go
    /// was.
    arriving: Vec<(Rect, Option<Pose>)>,
    /// The zone it was let go over.
    zone: Option<Area>,
}

/// How the captures showed, before they moved.
pub struct Before {
    collage: Collage,
    kept: Rect,
    view: View,
}

impl Editor {
    /// Follows a step of a drag over the window, as `import` tells it. Returns
    /// whether the editor takes what's dragged.
    pub(super) fn docking(&mut self, wparam: WPARAM, lparam: LPARAM) -> bool {
        let at = self.pointer();
        match Phase::of(wparam) {
            Some(Phase::Enter) => {
                let image = unsafe { Box::from_raw(lparam.0 as *mut Image) };
                self.enter(Arc::new(*image), at);
                true
            }
            Some(Phase::Over) => self.over(at),
            Some(Phase::Leave) => {
                self.leave();
                false
            }
            Some(Phase::Drop) => self.drop_incoming(),
            None => false,
        }
    }

    /// A capture came in: its card shows at the pointer, `at`, crossed by a glint.
    fn enter(&mut self, image: Arc<Image>, at: Point) {
        self.settle();
        let bitmap = self.gpu.as_ref().and_then(|gpu| {
            card::bitmap(gpu.surface.context(), &image)
                .inspect_err(|error| report("drop", error))
                .ok()
        });
        self.incoming = Some(Incoming {
            image,
            bitmap,
            at,
            shown: Animation::new(0.0, 1.0, motion::NORMAL, Easing::DECELERATE),
            glint: Animation::new(0.0, 1.0, FLIGHT, Easing::SWEEP),
            place: None,
            zone: None,
        });
        self.over(at);
        self.invalidate();
    }

    /// Follows the pointer, at `at`, dragging the capture: the zone glides to where
    /// it would land. Returns whether it docks if let go there.
    fn over(&mut self, at: Point) -> bool {
        let Some(mut incoming) = self.incoming.take() else {
            return false;
        };
        let now = Instant::now();
        let place = self.place_at(&incoming.image, at);
        // Windows asks again and again, the pointer still or not.
        let changed = at != incoming.at || place != incoming.place;
        incoming.at = at;
        if place != incoming.place {
            incoming.place = place;
            if let Some(place) = place {
                let (from, shown) = match &incoming.zone {
                    Some(zone) if zone.shown.value(now) > 0.0 => {
                        (self.zone_area(zone, now), zone.shown)
                    }
                    // It grows out of the pointer.
                    _ => (Area::new(at.0, at.1, 0.0, 0.0), Animation::idle(0.0)),
                };
                incoming.zone = Some(Zone {
                    place,
                    from,
                    travel: Animation::new(0.0, 1.0, ZONE_GLIDE, Easing::DECELERATE),
                    shown: shown.retarget(1.0, motion::FAST, Easing::DECELERATE),
                });
            } else if let Some(zone) = &mut incoming.zone {
                zone.shown = zone.shown.retarget(0.0, motion::FAST, Easing::DECELERATE);
            }
        }
        let docks = incoming.place.is_some();
        self.incoming = Some(incoming);
        if changed {
            self.invalidate();
        }
        docks
    }

    /// The capture left, or the drag was given up: its card and the zone fade.
    fn leave(&mut self) {
        if let Some(incoming) = &mut self.incoming {
            incoming.place = None;
            incoming.shown = (incoming.shown).retarget(0.0, motion::FAST, Easing::DECELERATE);
            if let Some(zone) = &mut incoming.zone {
                zone.shown = zone.shown.retarget(0.0, motion::FAST, Easing::DECELERATE);
            }
        }
        self.invalidate();
    }

    /// The capture was let go: it docks where the zone is, if it fits. Returns
    /// whether it did.
    fn drop_incoming(&mut self) -> bool {
        let now = Instant::now();
        let Some(place) = self.incoming.as_ref().and_then(|incoming| incoming.place) else {
            self.leave();
            return false;
        };
        let Some(incoming) = self.incoming.take() else {
            return false;
        };
        let card = Pose::held(self.size_of(&incoming.image), incoming.at);
        let zone = (incoming.zone.as_ref()).map(|zone| self.zone_area(zone, now));
        let before = self.before();
        let gap = (GAP * self.capture_scale).round() as i32;
        let image = incoming.image;
        (self.document).dock(image.clone(), place.target, place.side, gap);
        self.selected = None;
        self.sync();
        self.unpublished = true;
        self.land(before, Some((&image, card)), zone);
        true
    }

    /// Where the capture docks if let go at `at`: against the side nearest to it of
    /// the piece under it, or of them all off them. None if it wouldn't fit.
    fn place_at(&self, image: &Arc<Image>, at: Point) -> Option<Place> {
        let (x, y) = self.view.pixel(at);
        let target = self.document.piece_at(x, y);
        let rect = self.dock_rect(target)?;
        let side = Side::nearest(rect, x as f32 + 0.5, y as f32 + 0.5);
        let gap = (GAP * self.capture_scale).round() as i32;
        let docked = self.document.docked(image.clone(), target, side, gap);
        let (width, height) = docked.size();
        let fits = width as u32 <= MOST_ACROSS
            && height as u32 <= MOST_ACROSS
            && i64::from(width) * i64::from(height) <= MOST_PIXELS;
        fits.then_some(Place { target, side })
    }

    /// What a capture docks against at `target`: one of the pieces, or them all.
    fn dock_rect(&self, target: Option<usize>) -> Option<Rect> {
        match target {
            Some(index) => self.document.pieces().get(index).copied(),
            None => Some(self.bounds()),
        }
    }

    /// Where the zone shows.
    fn zone_area(&self, zone: &Zone, now: Instant) -> Area {
        let half = (self.dock_rect(zone.place.target))
            .map_or(zone.from, |rect| self.view.area(zone.place.side.half(rect)));
        zone.from.toward(half, zone.travel.value(now))
    }

    /// The zone's fill, `opacity` as much: violet, as the corners go from blue to
    /// pink, or the highlight in contrast themes.
    fn zone_color(&self, opacity: f32) -> D2D1_COLOR_F {
        let palette = &self.palette;
        if palette.contrast {
            faded(palette.accent, 0.3 * opacity)
        } else {
            let violet = theme::signature(palette.light)[1];
            let fill = if palette.light { ZONE_LIGHT } else { ZONE_DARK };
            color(violet, fill * opacity)
        }
    }

    /// `image`'s size in DIPs, shown pixel for pixel.
    fn size_of(&self, image: &Image) -> (f32, f32) {
        (
            image.width() as f32 / self.scale,
            image.height() as f32 / self.scale,
        )
    }

    /// How the captures show now, to play out what comes next from it.
    pub(super) fn before(&self) -> Before {
        Before {
            collage: self.document.collage().clone(),
            kept: self.bounds(),
            view: self.view,
        }
    }

    /// Plays the captures moving from how they showed `before` to how they show now:
    /// those that stay glide to their place, those that go fade, and those that come
    /// grow in, or land as `card` over `zone` for the one let go.
    pub(super) fn land(
        &mut self,
        before: Before,
        card: Option<(&Arc<Image>, Pose)>,
        zone: Option<Area>,
    ) {
        let was = self.gpu.as_ref().map(|gpu| gpu.image.clone());
        self.recompose();
        self.layout();
        let shown = |collage: &Collage, kept: Rect| {
            let slots = collage.slots().into_iter();
            slots
                .filter_map(|slot| Some((slot.rect.intersect(&kept)?, slot.image)))
                .collect::<Vec<_>>()
        };
        let then = shown(&before.collage, before.kept);
        let now = shown(self.document.collage(), self.bounds());
        let mut landing = Landing {
            clock: Animation::clock(LANDING),
            before: was,
            white: before.view.area(before.kept),
            moves: Vec::new(),
            leaving: Vec::new(),
            arriving: Vec::new(),
            zone,
        };
        for (part, image) in &now {
            match then.iter().find(|(_, old)| Arc::ptr_eq(old, image)) {
                Some(&(old, _)) => landing.moves.push((*part, before.view.area(old))),
                None => {
                    let dropped = card.filter(|(dropped, _)| Arc::ptr_eq(dropped, image));
                    landing
                        .arriving
                        .push((*part, dropped.map(|(_, pose)| pose)));
                }
            }
        }
        for (old, image) in &then {
            if !now.iter().any(|(_, new)| Arc::ptr_eq(new, image)) {
                landing.leaving.push((*old, before.view.area(*old)));
            }
        }
        self.landing = Some(landing);
        self.invalidate();
    }

    /// Starts over on the document's image, another now: nothing of it is drawn on,
    /// or on the graphics card, yet.
    fn recompose(&mut self) {
        self.renderer.restart(self.document.image());
        self.stale.clear();
        let bitmap = (self.gpu.as_ref())
            .map(|gpu| card::bitmap(gpu.surface.context(), self.renderer.output()));
        match (bitmap, &mut self.gpu) {
            (Some(Ok(bitmap)), Some(gpu)) => gpu.image = bitmap,
            // Made again, whole, on the next frame.
            _ => self.gpu = None,
        }
    }

    /// Whether a capture dragged over the editor, or landing, still moves.
    pub(super) fn docking_moves(&self, now: Instant) -> bool {
        let incoming = self.incoming.as_ref().is_some_and(|incoming| {
            incoming.shown.is_running(now)
                || incoming.glint.is_running(now)
                || (incoming.zone.as_ref())
                    .is_some_and(|zone| zone.travel.is_running(now) || zone.shown.is_running(now))
        });
        incoming || self.landing.is_some()
    }

    /// Lets go of the card that faded away and of the landing that ended.
    pub(super) fn docking_ended(&mut self, now: Instant) {
        if (self.incoming.as_ref()).is_some_and(|incoming| {
            incoming.shown.target() == 0.0 && !incoming.shown.is_running(now)
        }) {
            self.incoming = None;
        }
        if (self.landing.as_ref()).is_some_and(|landing| !landing.clock.is_running(now)) {
            self.landing = None;
        }
    }

    /// The captures as they land, in place of the capture: the white between them
    /// growing, each gliding to its place, and the zone closing on the one let go.
    pub(super) fn draw_landing(
        &self,
        painter: &Painter,
        image: &ID2D1Bitmap1,
        landing: &Landing,
        now: Instant,
    ) {
        let t = landing.clock.value(now);
        let room = along(t, 0.0, MAKE_ROOM, Easing::DECELERATE);
        let white = landing.white.toward(self.view.area(self.bounds()), room);
        painter.fill(white, 0.0, WHITE);
        let border = painter.pixel();
        painter.stroke(white.inflate(border), 0.0, border, self.palette.card_stroke);
        if let Some(before) = &landing.before {
            for &(part, area) in &landing.leaving {
                painter.grown(1.0 - room, 1.0 - SHRINK * room, center(area), || {
                    painter.bitmap(before, card::part(part), area);
                });
            }
        }
        for &(part, from) in &landing.moves {
            let area = from.toward(self.view.area(part), room);
            painter.bitmap(image, card::part(part), area);
        }
        for &(part, card) in &landing.arriving {
            let area = self.view.area(part);
            if card.is_none() {
                painter.grown(room, 1.0 - SHRINK * (1.0 - room), center(area), || {
                    painter.bitmap(image, card::part(part), area);
                });
            }
        }
        if let Some(zone) = landing.zone
            && let Some(&(part, _)) = landing.arriving.first()
        {
            let closed = along(t, 0.0, CLOSE, Easing::DECELERATE);
            let area = zone.toward(self.view.area(part), closed);
            painter.fill(area, 0.0, self.zone_color(1.0 - closed));
        }
    }

    /// The zone a capture dragged over the editor would land in, with the corners
    /// around it.
    pub(super) fn draw_zone(&self, painter: &Painter, now: Instant) {
        let Some(zone) = self
            .incoming
            .as_ref()
            .and_then(|incoming| incoming.zone.as_ref())
        else {
            return;
        };
        let shown = zone.shown.value(now);
        if shown > 0.0 {
            let area = self.zone_area(zone, now);
            painter.fill(area, 0.0, self.zone_color(shown));
            self.corners.draw(painter, area, shown);
        }
    }

    /// Over everything: the card of a capture dragged over the editor, or the one
    /// let go settling into place, its corners opening out once it lands.
    pub(super) fn draw_cards(&self, painter: &Painter, image: &ID2D1Bitmap1, now: Instant) {
        if let Some(landing) = &self.landing {
            let t = landing.clock.value(now);
            let settled = along(t, 0.0, SETTLE, Easing::SPRING);
            for &(part, card) in &landing.arriving {
                if let Some(card) = card {
                    let pose = card.toward(Pose::flat(self.view.area(part)), settled);
                    card::draw(painter, image, part, pose, None);
                }
            }
            if let Some(&(part, _)) = landing.arriving.first() {
                // They close in with the zone, or gather in around one coming back.
                let slot = self.view.area(part);
                let closed = along(t, 0.0, CLOSE, Easing::DECELERATE);
                let (area, gathered) = match landing.zone {
                    Some(zone) => (zone.toward(slot, closed), 1.0),
                    None => (slot, closed),
                };
                let shown = gathered * (1.0 - along(t, CORNERS_AT, CORNERS, Easing::DECELERATE));
                self.corners.draw(painter, area, shown);
            }
        }
        if let Some(incoming) = &self.incoming
            && let Some(bitmap) = &incoming.bitmap
        {
            let shown = incoming.shown.value(now);
            if shown > 0.0 {
                let pose = Pose::held(self.size_of(&incoming.image), incoming.at);
                let glint = incoming
                    .glint
                    .is_running(now)
                    .then(|| incoming.glint.value(now));
                painter.grown(shown, 0.9 + 0.1 * shown, incoming.at, || {
                    card::draw(painter, bitmap, incoming.image.bounds(), pose, glint);
                });
            }
        }
    }
}

fn center(area: Area) -> Point {
    (area.left + area.width / 2.0, area.middle())
}
