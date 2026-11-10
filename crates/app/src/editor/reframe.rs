//! Reframing a capture put together with others: double-clicked, the others dim
//! and what its space leaves out unfolds around it, faint, to drag into view and
//! to zoom into with the wheel. Each change keeps once it rests, to undo one by one.

use std::{
    mem,
    sync::Arc,
    time::{Duration, Instant},
};

use kata_editor::{Collage, Image, Placement, Shape, Slot};
use kata_ui::{
    motion::{self, Animation, Easing},
    paint::{Area, Painter, Point},
    theme::{self, color, faded},
};
use windows::Win32::{
    Graphics::Direct2D::ID2D1Bitmap1,
    UI::{
        HiDpi::GetSystemMetricsForDpi,
        Input::KeyboardAndMouse::GetDoubleClickTime,
        WindowsAndMessaging::{KillTimer, SM_CXDOUBLECLK, SM_CYDOUBLECLK, SetTimer},
    },
};

use super::{
    Editor, Gesture, TITLE_BAR, ZOOM_STEP,
    canvas::{self, Gpu},
    dock,
};
use crate::card;

/// The timer that says the wheel rested.
pub const REFRAME_TIMER: usize = 3;
/// How long the wheel rests before its zoom keeps, in milliseconds: a turn of it
/// makes one change.
const WHEEL_REST: u32 = 350;
/// How much the other captures dim, and how much of what the space leaves out
/// shows, once it unfolds.
const DIM: f32 = 0.4;
const GHOST: f32 = 0.45;
/// How much the line around all of the capture shows.
const EDGE: f32 = 0.7;

/// A capture being reframed.
pub struct Reframing {
    image: Arc<Image>,
    /// The captures as it's framed now, kept once it rests.
    collage: Collage,
    /// Where it showed, `travel` of the way to where `collage` has it.
    from: Placement,
    travel: Animation,
    /// From 0 to 1 as what its space leaves out unfolds, and back as it folds.
    unfold: Animation,
    /// The wheel turned and hasn't rested yet.
    wheeled: bool,
}

impl Reframing {
    /// Its place among the captures, and its space, as it's framed now.
    fn slot(&self) -> Option<(usize, Slot)> {
        (self.collage.slots().into_iter().enumerate()).find(|(_, slot)| slot.shows(&self.image))
    }

    /// Its space, and where it shows at `now`, gliding.
    fn at(&self, now: Instant) -> Option<(Slot, Placement)> {
        let (_, slot) = self.slot()?;
        let placement = mix(self.from, slot.placement, self.travel.value(now));
        Some((slot, placement))
    }
}

impl Editor {
    /// Whether a capture is being reframed, and not folding back.
    pub(super) fn is_reframing(&self) -> bool {
        (self.reframing.as_ref()).is_some_and(|reframing| reframing.unfold.target() > 0.0)
    }

    /// Whether a press at `point` makes a double-click with the last one, as near in
    /// time and place as Windows asks.
    pub(super) fn clicked_twice(&mut self, point: Point) -> bool {
        let now = Instant::now();
        let dpi = self.dpi();
        let (width, height) = unsafe {
            (
                GetSystemMetricsForDpi(SM_CXDOUBLECLK, dpi),
                GetSystemMetricsForDpi(SM_CYDOUBLECLK, dpi),
            )
        };
        let time = Duration::from_millis(unsafe { GetDoubleClickTime() }.into());
        let near = |from: Point| {
            ((point.0 - from.0) * self.scale).abs() <= width as f32 / 2.0
                && ((point.1 - from.1) * self.scale).abs() <= height as f32 / 2.0
        };
        let twice = (self.click).is_some_and(|(at, from)| now - at <= time && near(from));
        // A third press starts over.
        self.click = (!twice).then_some((now, point));
        twice
    }

    /// Follows the second press of a double-click, at `point`. Returns whether it
    /// takes it: on a capture put together with others, away from the shapes, it
    /// starts reframing it, and inside the one being reframed, it frames it whole.
    pub(super) fn double_click(&mut self, point: Point) -> bool {
        let placed = mem::take(&mut self.placed);
        self.settle();
        if self.is_reframing() {
            let inside = (self.framing_areas(Instant::now()))
                .is_some_and(|(space, _)| space.contains(point));
            if inside {
                self.keep_framing();
                if let Some(reframing) = &self.reframing
                    && let Some((index, _)) = reframing.slot()
                {
                    let whole = reframing.collage.reframed(index, 1.0, (0.5, 0.5));
                    self.glide_framing(self.document.arranged(whole), motion::NORMAL);
                }
            }
            return inside;
        }
        if point.1 < TITLE_BAR
            || self.toolbar.covers(point)
            || self.cropping.is_some()
            || self.incoming.is_some()
        {
            return false;
        }
        let Some(image) = self.piece_under(point) else {
            return false;
        };
        // The step the first press placed goes, as the double-click reframes.
        let on = self.shape_at(point);
        let last = self.edits.steps.len().checked_sub(1).map(Shape::Step);
        let first = placed && on.is_some() && on == last;
        if on.is_some_and(|shape| !self.fills(shape)) && !first {
            return false;
        }
        if first && self.document.retract() {
            self.sync();
            self.unpublished = true;
        }
        self.reframe(image);
        true
    }

    /// The capture under `point`, if it's put together with others.
    fn piece_under(&self, point: Point) -> Option<Arc<Image>> {
        let slots = self.document.collage().slots();
        let (x, y) = self.view.pixel(point);
        if slots.len() < 2 || !self.bounds().contains(x, y) {
            return None;
        }
        (slots.into_iter())
            .find(|slot| slot.rect.contains(x, y))
            .map(|slot| slot.image)
    }

    /// Starts reframing `image`: the others dim as what its space leaves out unfolds.
    fn reframe(&mut self, image: Arc<Image>) {
        let collage = self.document.collage().clone();
        let Some(slot) = collage.slots().into_iter().find(|slot| slot.shows(&image)) else {
            return;
        };
        self.select(None);
        self.hovered = None;
        self.show_grip(None);
        self.reframing = Some(Reframing {
            image,
            collage,
            from: slot.placement,
            travel: Animation::idle(1.0),
            unfold: Animation::new(0.0, 1.0, motion::NORMAL, Easing::DECELERATE),
            wheeled: false,
        });
        self.set_cursor();
        self.invalidate();
    }

    /// Ends reframing, keeping the framing on show: what the space left out folds
    /// back into it.
    pub(super) fn leave_reframing(&mut self) {
        if !self.is_reframing() {
            return;
        }
        self.keep_framing();
        if let Some(reframing) = &mut self.reframing {
            reframing.unfold = (reframing.unfold).retarget(0.0, motion::FAST, Easing::DECELERATE);
        }
        self.invalidate();
    }

    /// Keeps the framing on show, if it's new: one change, to undo. Once folding, it
    /// was kept already.
    pub(super) fn keep_framing(&mut self) {
        if !self.is_reframing() {
            return;
        }
        let Some(reframing) = &mut self.reframing else {
            return;
        };
        if mem::take(&mut reframing.wheeled) {
            let _ = unsafe { KillTimer(Some(self.hwnd), REFRAME_TIMER) };
        }
        let collage = reframing.collage.clone();
        if self.document.arrange(collage) {
            self.sync();
            self.unpublished = true;
            self.recompose();
        }
    }

    /// The captures as they are with the one reframed shown as `placement`, or as
    /// near as it can be while it fills its space and shows what's drawn on it.
    fn framed(&self, placement: Placement) -> Option<Collage> {
        let reframing = self.reframing.as_ref()?;
        let (index, slot) = reframing.slot()?;
        let (width, height) = (slot.image.width() as f32, slot.image.height() as f32);
        let rect = slot.rect;
        let middle = (
            (rect.left + rect.right) as f32 / 2.0,
            (rect.top + rect.bottom) as f32 / 2.0,
        );
        let (x, y) = placement.source(middle);
        let zoom = placement.scale / filling(&slot);
        let collage = (reframing.collage).reframed(index, zoom, (x / width, y / height));
        Some(self.document.arranged(collage))
    }

    /// Frames the capture as `collage` has it, at once.
    fn set_framing(&mut self, collage: Collage) -> bool {
        let Some(reframing) = &mut self.reframing else {
            return false;
        };
        if reframing.collage == collage {
            return false;
        }
        reframing.collage = collage;
        reframing.travel = Animation::idle(1.0);
        true
    }

    /// Glides the capture, from where it shows, to how `collage` frames it, in
    /// `duration`.
    fn glide_framing(&mut self, collage: Collage, duration: Duration) {
        let Some(reframing) = &mut self.reframing else {
            return;
        };
        if reframing.collage == collage {
            return;
        }
        if let Some((_, placement)) = reframing.at(Instant::now()) {
            reframing.from = placement;
        }
        reframing.collage = collage;
        reframing.travel = Animation::new(0.0, 1.0, duration, Easing::DECELERATE);
        self.invalidate();
    }

    /// Takes hold of the capture reframed at `point`, anywhere in all of it, to drag
    /// it: where it glides, it stops. `None` away from it.
    pub(super) fn grab_framing(&mut self, point: Point) -> Option<Gesture> {
        let now = Instant::now();
        let (_, frame) = self.framing_areas(now)?;
        if !frame.contains(point) {
            return None;
        }
        let (_, live) = self.reframing.as_ref()?.at(now)?;
        if let Some(collage) = self.framed(live) {
            self.set_framing(collage);
        }
        // What changed keeps first, for letting go to go back to.
        self.keep_framing();
        let (_, start) = self.reframing.as_ref()?.at(now)?;
        Some(Gesture::Pan { from: point, start })
    }

    /// Follows the pointer at `point`, dragging the capture grabbed at `from` where it
    /// showed as `start`. Returns whether the look changes.
    pub(super) fn pan(&mut self, from: Point, start: Placement, point: Point) -> bool {
        let zoom = self.view.zoom;
        let origin = (
            start.origin.0 + (point.0 - from.0) / zoom,
            start.origin.1 + (point.1 - from.1) / zoom,
        );
        let Some(collage) = self.framed(Placement { origin, ..start }) else {
            return false;
        };
        self.set_framing(collage)
    }

    /// Lets go of the capture dragged from `from`, at `point`: a click on what its
    /// space leaves out ends reframing.
    pub(super) fn let_go_framing(&mut self, from: Point, point: Point) {
        let inside =
            (self.framing_areas(Instant::now())).is_some_and(|(space, _)| space.contains(point));
        if !inside && !self.dragged(from, point) {
            self.leave_reframing();
        }
    }

    /// Lets go of the capture dragged, back to where it was.
    pub(super) fn cancel_pan(&mut self) {
        let collage = self.document.collage().clone();
        self.glide_framing(collage, motion::NORMAL);
    }

    /// Zooms the capture reframed `notches` turns of the wheel, around `at`.
    pub(super) fn wheel_framing(&mut self, at: Point, notches: f32) {
        let Some((_, slot)) = self.reframing.as_ref().and_then(Reframing::slot) else {
            return;
        };
        let to = slot.placement;
        let filling = filling(&slot);
        let zoom = to.scale / filling * ZOOM_STEP.powf(notches);
        let scale = filling * zoom.clamp(1.0, Collage::MOST_ZOOM);
        // The point under the pointer stays under it.
        let point = self.view.at(at);
        let (x, y) = to.source(point);
        let origin = (point.0 - x * scale, point.1 - y * scale);
        let Some(collage) = self.framed(Placement { scale, origin }) else {
            return;
        };
        self.glide_framing(collage, motion::FAST);
        if let Some(reframing) = &mut self.reframing {
            reframing.wheeled = true;
        }
        unsafe { SetTimer(Some(self.hwnd), REFRAME_TIMER, WHEEL_REST, None) };
    }

    /// The wheel rested: its zoom keeps once the capture stops gliding.
    pub(super) fn wheel_rested(&mut self) {
        let _ = unsafe { KillTimer(Some(self.hwnd), REFRAME_TIMER) };
        if let Some(reframing) = &mut self.reframing {
            reframing.wheeled = false;
        }
        self.invalidate();
    }

    /// Follows the document going back or forth in its history from `before`: the
    /// capture reframed glides to how it's framed now, if that's all that changed.
    /// Otherwise reframing ends at once, for the captures to land. Returns whether
    /// it glides.
    pub(super) fn follow_framing(&mut self, before: &Collage) -> bool {
        if !self.is_reframing() {
            self.reframing = None;
            return false;
        }
        let collage = self.document.collage();
        let (then, now) = (before.slots(), collage.slots());
        let framed = (self.reframing.as_ref()).is_some_and(|reframing| {
            then.len() == now.len()
                && then
                    .iter()
                    .zip(&now)
                    .any(|(_, slot)| slot.shows(&reframing.image))
                && then.iter().zip(&now).all(|(then, now)| {
                    Arc::ptr_eq(&then.image, &now.image)
                        && then.rect == now.rect
                        && (now.shows(&reframing.image) || then.placement == now.placement)
                })
        });
        if !framed {
            self.reframing = None;
            return false;
        }
        let collage = collage.clone();
        self.glide_framing(collage, motion::NORMAL);
        self.recompose();
        true
    }

    /// Keeps the framing once the capture rests, and lets go of it once folded.
    pub(super) fn reframing_rests(&mut self, now: Instant) {
        let Some(reframing) = &self.reframing else {
            return;
        };
        let panning = matches!(self.gesture, Some(Gesture::Pan { .. }));
        let rests = !panning && !reframing.wheeled && !reframing.travel.is_running(now);
        let folded = reframing.unfold.target() == 0.0 && !reframing.unfold.is_running(now);
        if rests && !folded && reframing.collage != *self.document.collage() {
            self.keep_framing();
        }
        if folded {
            self.reframing = None;
            // Its own copy takes as much memory on the graphics card as the image.
            if let Some(gpu) = &mut self.gpu {
                gpu.forget();
            }
        }
    }

    /// Whether the capture reframed still glides, unfolds or folds.
    pub(super) fn reframing_moves(&self, now: Instant) -> bool {
        (self.reframing.as_ref()).is_some_and(|reframing| {
            reframing.travel.is_running(now) || reframing.unfold.is_running(now)
        })
    }

    /// Whether `point` is on the capture reframed, anywhere in all of it, while one
    /// is.
    pub(super) fn over_framing(&self, point: Point) -> Option<bool> {
        let areas = self
            .is_reframing()
            .then(|| self.framing_areas(Instant::now()));
        areas.map(|areas| areas.is_some_and(|(_, frame)| frame.contains(point)))
    }

    /// Where the capture reframed has its space, and where all of it shows, at `now`.
    fn framing_areas(&self, now: Instant) -> Option<(Area, Area)> {
        let (slot, placement) = self.reframing.as_ref()?.at(now)?;
        let space = self.view.area(slot.rect.intersect(&self.bounds())?);
        let (width, height) = (slot.image.width() as f32, slot.image.height() as f32);
        let frame = self.view.span(Area::new(
            placement.origin.0,
            placement.origin.1,
            width * placement.scale,
            height * placement.scale,
        ));
        Some((space, frame))
    }

    /// The capture reframed's own copy, while it is.
    pub(super) fn framing_own(&self, gpu: &mut Gpu) -> Option<ID2D1Bitmap1> {
        gpu.piece(&self.reframing.as_ref()?.image)
    }

    /// The capture reframed over the others, dimmed: what its space leaves out
    /// unfolding around it, faint, and Kata's violet around both, or the
    /// highlight in contrast themes.
    pub(super) fn draw_reframing(
        &self,
        painter: &Painter,
        image: &ID2D1Bitmap1,
        own: Option<&ID2D1Bitmap1>,
        now: Instant,
    ) {
        if self.cropping.is_some() || self.landing.is_some() {
            return;
        }
        let Some(reframing) = &self.reframing else {
            return;
        };
        let Some((space, frame)) = self.framing_areas(now) else {
            return;
        };
        let unfold = reframing.unfold.value(now);
        let whole = card::part(reframing.image.bounds());
        let kept = self.bounds();
        painter.fill(self.view.area(kept), 0.0, color(0x000000, DIM * unfold));
        let unfolded = space.toward(frame, unfold);
        if let Some(own) = own {
            painter.clip(unfolded, || {
                painter.faded_bitmap(own, whole, frame, GHOST * unfold);
            });
        }
        // Its space shows it whole, with the shapes on it from the image.
        let shown = (dock::shown(self.document.collage(), kept, self.view).into_iter())
            .find(|(shown, _)| Arc::ptr_eq(shown, &reframing.image));
        painter.clip(space, || {
            if let Some(own) = own {
                painter.bitmap(own, whole, frame);
            }
            if let Some((_, shown)) = shown {
                painter.bitmap(image, card::part(shown.part), shown.over(frame));
            }
        });
        let palette = &self.palette;
        let violet = if palette.contrast {
            palette.accent
        } else {
            color(theme::signature(palette.light)[1], 1.0)
        };
        let lines = || {
            let line = painter.pixel();
            let edge = painter.align(unfolded).inflate(line);
            painter.stroke(edge, 0.0, line, faded(violet, EDGE));
            canvas::draw_outline(painter, space, 0.0, canvas::SELECTED, violet);
        };
        if unfold < 1.0 {
            painter.layer(unfold, 0.0, lines);
        } else {
            lines();
        }
    }
}

/// How much `slot`'s capture scales to just fill its space.
fn filling(slot: &Slot) -> f32 {
    let (width, height) = (slot.image.width() as f32, slot.image.height() as f32);
    (slot.rect.width() as f32 / width).max(slot.rect.height() as f32 / height)
}

/// The placement `progress` of the way from `from` to `to`: the point between them
/// that stays put stays put all the way.
fn mix(from: Placement, to: Placement, progress: f32) -> Placement {
    let mix = |from: f32, to: f32| from + (to - from) * progress;
    Placement {
        scale: mix(from.scale, to.scale),
        origin: (
            mix(from.origin.0, to.origin.0),
            mix(from.origin.1, to.origin.1),
        ),
    }
}
