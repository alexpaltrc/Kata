//! Docking another capture: dragged over the editor from File Explorer, it lights up
//! the half of a space it would take, as windows snap side by side, and let go there
//! it lands as the captures make room. Undoing it, or making it again, plays out the
//! same way.

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
use crate::settings::Perk;
use crate::{
    card::{self, Pose},
    import::Phase,
    report,
};

/// The message the editor hears a drag over it by, a step at a time.
pub const DOCK: u32 = WM_APP + 1;
/// The white between two captures, in DIPs at the scale the first was taken.
const GAP: f32 = 16.0;
/// How near the collage's edge, in DIPs, a capture docks against them all.
const EDGE: f32 = 14.0;
/// The least a space measures across and down, in DIPs at the scale the first
/// capture was taken.
const LEAST: f32 = 64.0;
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

/// Where a capture docks: against a side of one of the pieces, or of them all,
/// taking `rect` of the image as it is now.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Place {
    target: Option<usize>,
    side: Side,
    rect: Rect,
}

/// The zone lit where the capture would land: gliding from `from` to where `place`
/// is, `travel` of the way, and showing `shown` as much.
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
    /// The image as it was, edits and all, for those that move or leave.
    before: Option<ID2D1Bitmap1>,
    /// Where the captures and the white between them showed.
    white: Area,
    pieces: Vec<Landed>,
    /// The zone it was let go over.
    zone: Option<Area>,
}

/// A capture as it lands: how it showed, if it did, and how it shows now, if it
/// stays.
struct Landed {
    /// Its own copy, and its pixels, for what shows of it on the way and neither
    /// before nor now.
    own: Option<(ID2D1Bitmap1, Area)>,
    from: Option<Shown>,
    to: Option<Shown>,
    /// The card it was, let go.
    card: Option<Pose>,
}

/// How a capture shows.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct Shown {
    /// Its part of the image.
    pub(super) part: Rect,
    /// Where that shows, in DIPs.
    pub(super) space: Area,
    /// Where all of the capture would, in DIPs.
    pub(super) frame: Area,
    /// What of the capture shows, in its pixels.
    pub(super) source: Area,
}

impl Shown {
    /// Where its part shows over the capture shown at `frame`.
    pub(super) fn over(&self, frame: Area) -> Area {
        let scale = (
            frame.width / self.frame.width,
            frame.height / self.frame.height,
        );
        Area::new(
            frame.left + (self.space.left - self.frame.left) * scale.0,
            frame.top + (self.space.top - self.frame.top) * scale.1,
            self.space.width * scale.0,
            self.space.height * scale.1,
        )
    }

    /// Whether it shows all of the capture that `to` does.
    pub(super) fn covers(&self, to: &Self) -> bool {
        // A rounding's worth of a pixel doesn't count.
        const SLACK: f32 = 0.5;
        let (from, to) = (self.source, to.source);
        to.left >= from.left - SLACK
            && to.top >= from.top - SLACK
            && to.left + to.width <= from.left + from.width + SLACK
            && to.top + to.height <= from.top + from.height + SLACK
    }
}

/// How the captures showed, before they moved.
pub struct Before {
    pub(super) collage: Collage,
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
        self.leave_reframing();
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

    /// A capture yet to be taken is to be placed: the zone shows where it would
    /// land, with no card, as there's nothing of it to show.
    pub(super) fn expect(&mut self, at: Point) {
        self.settle();
        self.leave_reframing();
        // As this one for its shape: the captures share the space alike.
        let whole = self.whole();
        let pixels = vec![0; (whole.width() * whole.height()) as usize * Image::BYTES_PER_PIXEL];
        let image = Image::new(whole.width() as u32, whole.height() as u32, pixels);
        self.incoming = Some(Incoming {
            image: Arc::new(image),
            bitmap: None,
            at,
            shown: Animation::idle(0.0),
            glint: Animation::idle(1.0),
            place: None,
            zone: None,
        });
        self.over(at);
        self.invalidate();
    }

    /// Follows the pointer placing a capture yet to be taken. Returns whether it
    /// would dock there.
    pub(super) fn expected_at(&mut self, at: Point) -> bool {
        self.over(at)
    }

    /// Stops placing a capture yet to be taken.
    pub(super) fn unexpect(&mut self) {
        self.leave();
    }

    /// Docks `image`, just taken, where it was placed, at `at`. Returns whether it
    /// fit there.
    pub(super) fn dock_at(&mut self, image: Arc<Image>, at: Point) -> bool {
        self.enter(image, at);
        self.drop_incoming()
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
        // Joining captures is Premium's, past its free tries.
        if !Perk::Joining.allowed() {
            self.leave();
            self.offer_premium();
            return false;
        }
        let Some(incoming) = self.incoming.take() else {
            return false;
        };
        Perk::Joining.used();
        let card = Pose::held(self.size_of(&incoming.image), incoming.at);
        let zone = (incoming.zone.as_ref()).map(|zone| self.zone_area(zone, now));
        let before = self.before();
        let image = incoming.image;
        if let (Some(gpu), Some(bitmap)) = (&mut self.gpu, incoming.bitmap) {
            gpu.adopt(image.clone(), bitmap);
        }
        (self.document).dock(image.clone(), place.target, place.side, self.gap());
        self.selected = None;
        self.sync();
        self.unpublished = true;
        self.land(before, Some((&image, card)), zone);
        true
    }

    /// Where the capture docks if let go at `at`: against the side nearest to it of
    /// the piece under it, or of them all near their edge or off them. None if its
    /// space would be too small.
    fn place_at(&self, image: &Arc<Image>, at: Point) -> Option<Place> {
        let (x, y) = self.view.pixel(at);
        let kept = self.bounds();
        let pieces = self.document.pieces();
        let edge = (EDGE / self.view.zoom).ceil() as i32;
        let target = if pieces.len() > 1 && !kept.inflate(-edge).contains(x, y) {
            None
        } else {
            self.document.piece_at(x, y)
        };
        let rect = target
            .and_then(|index| pieces.get(index).copied())
            .unwrap_or(kept);
        let side = Side::nearest(rect, x as f32 + 0.5, y as f32 + 0.5);
        let docked = self
            .document
            .docked(image.clone(), target, side, self.gap());
        let slot = docked.slots().into_iter().find(|slot| slot.shows(image))?;
        let least = (LEAST * self.capture_scale).round() as i32;
        (slot.rect.width() >= least && slot.rect.height() >= least).then_some(Place {
            target,
            side,
            // What's cropped away goes as it docks.
            rect: slot.rect.offset(kept.left, kept.top),
        })
    }

    /// The white between two captures, in pixels.
    fn gap(&self) -> i32 {
        (GAP * self.capture_scale).round() as i32
    }

    /// Where the zone shows.
    fn zone_area(&self, zone: &Zone, now: Instant) -> Area {
        let to = self.view.area(zone.place.rect);
        zone.from.toward(to, zone.travel.value(now))
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
    /// those that stay glide to their space, those that go fade, and those that come
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
        let then = shown(&before.collage, before.kept, before.view);
        let now = shown(self.document.collage(), self.bounds(), self.view);
        let mut pieces = Vec::new();
        for (image, to) in &now {
            let from = (then.iter())
                .find(|(old, _)| Arc::ptr_eq(old, image))
                .map(|&(_, from)| from);
            let card = card
                .filter(|(dropped, _)| Arc::ptr_eq(dropped, image))
                .map(|(_, pose)| pose);
            // Only one coming as a card, or showing more of itself than it did, needs
            // its own copy: the image before shows the rest.
            let own = card.is_some() || from.is_some_and(|from| !from.covers(to));
            let own = own.then(|| self.own(image)).flatten();
            pieces.push(Landed {
                own,
                from,
                to: Some(*to),
                card,
            });
        }
        for (image, from) in &then {
            if !now.iter().any(|(new, _)| Arc::ptr_eq(new, image)) {
                pieces.push(Landed {
                    own: None,
                    from: Some(*from),
                    to: None,
                    card: None,
                });
            }
        }
        // Those landing hold on to their own copies until they land.
        if let Some(gpu) = &mut self.gpu {
            gpu.forget();
        }
        self.landing = Some(Landing {
            clock: Animation::clock(LANDING),
            before: was,
            white: before.view.area(before.kept),
            pieces,
            zone,
        });
        self.invalidate();
    }

    /// `image`'s own copy on the graphics card, and its pixels.
    fn own(&mut self, image: &Arc<Image>) -> Option<(ID2D1Bitmap1, Area)> {
        let bitmap = self.gpu.as_mut()?.piece(image)?;
        Some((bitmap, card::part(image.bounds())))
    }

    /// Starts over on the document's image, another now: nothing of it is drawn on,
    /// or on the graphics card, yet.
    pub(super) fn recompose(&mut self) {
        self.renderer.restart(self.document.image());
        self.reload();
    }

    /// Copies all the renderer has drawn to the graphics card again.
    pub(super) fn reload(&mut self) {
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

    /// The captures as they land, in place of the image: the white between them
    /// growing, each gliding to its space as it frames itself anew, and the zone
    /// closing on the one let go.
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
        // Those leaving go under those making room.
        for landed in &landing.pieces {
            if let (Some(from), None, Some(before)) = (landed.from, landed.to, &landing.before) {
                let area = from.space;
                painter.grown(1.0 - room, 1.0 - SHRINK * room, center(area), || {
                    painter.bitmap(before, card::part(from.part), area);
                });
            }
        }
        for landed in &landing.pieces {
            match (landed.from, landed.to) {
                (Some(from), Some(to)) => {
                    let frame = from.frame.toward(to.frame, room);
                    painter.clip(from.space.toward(to.space, room), || {
                        if let Some((own, whole)) = &landed.own {
                            painter.bitmap(own, *whole, frame);
                        }
                        if let Some(before) = &landing.before {
                            painter.bitmap(before, card::part(from.part), from.over(frame));
                        }
                        painter.faded_bitmap(image, card::part(to.part), to.over(frame), room);
                    });
                }
                (None, Some(to)) if landed.card.is_none() => {
                    let area = to.space;
                    painter.grown(room, 1.0 - SHRINK * (1.0 - room), center(area), || {
                        painter.bitmap(image, card::part(to.part), area);
                    });
                }
                _ => {}
            }
        }
        if let Some(zone) = landing.zone
            && let Some(to) = landing.arriving()
        {
            let closed = along(t, 0.0, CLOSE, Easing::DECELERATE);
            let area = zone.toward(to.space, closed);
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
    /// let go settling into its space as it frames itself, its corners opening out
    /// once it lands.
    pub(super) fn draw_cards(&self, painter: &Painter, image: &ID2D1Bitmap1, now: Instant) {
        if let Some(landing) = &self.landing {
            let t = landing.clock.value(now);
            let settled = along(t, 0.0, SETTLE, Easing::SPRING);
            let framed = along(t, 0.0, SETTLE, Easing::DECELERATE);
            for landed in &landing.pieces {
                if let (Some(card), Some(to)) = (landed.card, landed.to) {
                    let pose = card.toward(Pose::flat(to.space), settled);
                    match &landed.own {
                        Some((own, whole)) => {
                            let part = fitted(whole.toward(to.source, framed), pose.area());
                            card::draw(painter, own, part, pose, None);
                        }
                        None => card::draw(painter, image, card::part(to.part), pose, None),
                    }
                }
            }
            if let Some(to) = landing.arriving() {
                // They close in with the zone, or gather in around one coming back.
                let closed = along(t, 0.0, CLOSE, Easing::DECELERATE);
                let (area, gathered) = match landing.zone {
                    Some(zone) => (zone.toward(to.space, closed), 1.0),
                    None => (to.space, closed),
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
                let whole = card::part(incoming.image.bounds());
                painter.grown(shown, 0.9 + 0.1 * shown, incoming.at, || {
                    card::draw(painter, bitmap, whole, pose, glint);
                });
            }
        }
    }
}

impl Landing {
    /// How the first capture that wasn't there shows now.
    fn arriving(&self) -> Option<Shown> {
        (self.pieces.iter())
            .find(|landed| landed.from.is_none())
            .and_then(|landed| landed.to)
    }
}

/// How each capture in `collage` shows of `kept`, as `view` has it.
pub(super) fn shown(collage: &Collage, kept: Rect, view: View) -> Vec<(Arc<Image>, Shown)> {
    (collage.slots().into_iter())
        .filter_map(|slot| {
            let part = slot.rect.intersect(&kept)?;
            let placement = slot.placement;
            let (width, height) = (slot.image.width() as f32, slot.image.height() as f32);
            let (left, top) = placement.source((part.left as f32, part.top as f32));
            let (right, bottom) = placement.source((part.right as f32, part.bottom as f32));
            let shown = Shown {
                part,
                space: view.area(part),
                frame: view.span(Area::new(
                    placement.origin.0,
                    placement.origin.1,
                    width * placement.scale,
                    height * placement.scale,
                )),
                source: Area::new(left, top, right - left, bottom - top),
            };
            Some((slot.image, shown))
        })
        .collect()
}

/// The middle of `part` as wide or as tall as it must be to fill `area` unstretched.
fn fitted(part: Area, area: Area) -> Area {
    let aspect = area.width / area.height.max(f32::EPSILON);
    let (width, height) = if part.width > part.height * aspect {
        (part.height * aspect, part.height)
    } else {
        (part.width, part.width / aspect.max(f32::EPSILON))
    };
    Area::new(
        part.left + (part.width - width) / 2.0,
        part.top + (part.height - height) / 2.0,
        width,
        height,
    )
}

fn center(area: Area) -> Point {
    (area.left + area.width / 2.0, area.middle())
}
