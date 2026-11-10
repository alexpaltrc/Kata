//! Arranging the captures put together: the white between two of them moves as
//! it's dragged, a grip on it saying it can, and each capture fills its space anew
//! as it grows or shrinks, the shapes on it along.

use std::{sync::Arc, time::Instant};

use kata_editor::Divider;
use kata_ui::{
    motion::{self, Animation, Easing},
    paint::{Area, Painter, Point},
    theme::{self, color, faded},
};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

use super::{
    Editor, Gesture, TITLE_BAR, WHITE,
    canvas::Gpu,
    dock::{self, Shown},
};
use crate::card;

/// How far past either side of the white, in DIPs, the pointer still grabs it.
const REACH: f32 = 4.0;
/// The grip on the white under the pointer, in DIPs across and along it, and how
/// much longer it grows while held.
const GRIP: (f32, f32) = (4.0, 36.0);
const GRIP_HELD: f32 = 0.2;
/// How dark the grip shows over the white until it's held.
const GRIP_DARK: f32 = 0.3;

/// The grip on the white the pointer is over, or was.
pub struct Grip {
    /// Which white, outermost first.
    index: usize,
    /// From 0 to 1 as it shows, and as it's held.
    shown: Animation,
    held: Animation,
}

/// A capture as the white dragged leaves it: how it showed, if it did, and its own
/// copy, and its pixels, for what it shows of itself that it didn't.
pub struct Arranged {
    own: Option<(ID2D1Bitmap1, Area)>,
    from: Option<Shown>,
    to: Shown,
}

impl Editor {
    /// The white between two spaces under `point`, and which it is, outermost first.
    pub(super) fn divider_at(&self, point: Point) -> Option<(usize, Divider)> {
        if point.1 < TITLE_BAR
            || self.cropping.is_some()
            || self.landing.is_some()
            || self.incoming.is_some()
            || self.is_reframing()
        {
            return None;
        }
        let kept = self.bounds();
        let (x, y) = self.view.pixel(point);
        let reach = (REACH / self.view.zoom).ceil() as i32;
        (self.document.collage().dividers().into_iter().enumerate()).find(|(_, divider)| {
            (divider.strip.intersect(&kept))
                .is_some_and(|strip| strip.inflate(reach).contains(x, y))
        })
    }

    /// Takes hold of the `index`th white, `divider`, at `point`.
    pub(super) fn grab_divider(&mut self, index: usize, divider: Divider, point: Point) -> Gesture {
        let strip = divider.strip;
        let middle = (
            (strip.left + strip.right) as f32 / 2.0,
            (strip.top + strip.bottom) as f32 / 2.0,
        );
        let offset = along(divider, middle) - along(divider, self.view.at(point));
        self.arranging = Some(self.document.collage().clone());
        self.show_grip(Some(index));
        if let Some(grip) = &mut self.grip {
            grip.held = grip.held.retarget(1.0, motion::FAST, Easing::DECELERATE);
        }
        Gesture::Divide { index, offset }
    }

    /// Follows the pointer, at `point`, dragging the `index`th white `offset` pixels
    /// from its middle. Returns whether the look changes.
    pub(super) fn divide(&mut self, index: usize, offset: f32, point: Point) -> bool {
        let collage = self.document.collage();
        let Some(divider) = collage.dividers().get(index).copied() else {
            return false;
        };
        let ratio = divider.ratio(along(divider, self.view.at(point)) + offset);
        let arranged = self.document.arranged(collage.divided(index, ratio));
        if self.arranging.as_ref() == Some(&arranged) {
            return false;
        }
        self.arranging = Some(arranged);
        true
    }

    /// Lets go of the white dragged: the captures keep the spaces it left them, if it
    /// moved, or go back to theirs if `kept` is false.
    pub(super) fn let_go_divider(&mut self, kept: bool) {
        let arranged = self.arranging.take();
        if let Some(grip) = &mut self.grip {
            grip.held = grip.held.retarget(0.0, motion::FAST, Easing::DECELERATE);
        }
        if kept
            && let Some(arranged) = arranged
            && self.document.arrange(arranged)
        {
            self.sync();
            self.unpublished = true;
            self.recompose();
        }
        if let Some(gpu) = &mut self.gpu {
            gpu.forget();
        }
    }

    /// Shows the grip on the `index`th white, or hides it with none. Returns whether
    /// that changes anything.
    pub(super) fn show_grip(&mut self, index: Option<usize>) -> bool {
        let fade = |shown: &mut Animation, to: f32| {
            let changed = shown.target() != to;
            *shown = shown.retarget(to, motion::FAST, Easing::DECELERATE);
            changed
        };
        match (index, &mut self.grip) {
            (Some(index), Some(grip)) if grip.index == index => fade(&mut grip.shown, 1.0),
            (Some(index), _) => {
                self.grip = Some(Grip {
                    index,
                    shown: Animation::new(0.0, 1.0, motion::FAST, Easing::DECELERATE),
                    held: Animation::idle(0.0),
                });
                true
            }
            (None, Some(grip)) => fade(&mut grip.shown, 0.0),
            (None, None) => false,
        }
    }

    /// Whether the grip still fades in or out.
    pub(super) fn grip_moves(&self, now: Instant) -> bool {
        (self.grip.as_ref())
            .is_some_and(|grip| grip.shown.is_running(now) || grip.held.is_running(now))
    }

    /// Lets go of the grip once it faded away.
    pub(super) fn grip_ended(&mut self, now: Instant) {
        if (self.grip.as_ref())
            .is_some_and(|grip| grip.shown.target() == 0.0 && !grip.shown.is_running(now))
        {
            self.grip = None;
        }
    }

    /// The captures as the white dragged leaves them, while it is, each with its own
    /// copy if it shows more of itself than it did.
    pub(super) fn arranged(&self, gpu: &mut Gpu) -> Option<Vec<Arranged>> {
        let arranged = self.arranging.as_ref()?;
        let kept = self.bounds();
        let then = dock::shown(self.document.collage(), kept, self.view);
        let now = dock::shown(arranged, kept, self.view);
        let pieces = (now.into_iter())
            .map(|(image, to)| {
                let from = (then.iter())
                    .find(|(old, _)| Arc::ptr_eq(old, &image))
                    .map(|&(_, from)| from);
                let own = !from.is_some_and(|from| from.covers(&to));
                let own = own
                    .then(|| gpu.piece(&image))
                    .flatten()
                    .map(|bitmap| (bitmap, card::part(image.bounds())));
                Arranged { own, from, to }
            })
            .collect();
        Some(pieces)
    }

    /// The captures filling the spaces the white dragged leaves them, in place of the
    /// image: each from the image as it is, the shapes on it along, and from its own
    /// copy for what it didn't show.
    pub(super) fn draw_arranged(
        &self,
        painter: &Painter,
        image: &ID2D1Bitmap1,
        pieces: &[Arranged],
    ) {
        let capture = self.view.area(self.bounds());
        painter.fill(capture, 0.0, WHITE);
        let border = painter.pixel();
        painter.stroke(
            capture.inflate(border),
            0.0,
            border,
            self.palette.card_stroke,
        );
        for piece in pieces {
            let to = piece.to;
            painter.clip(to.space, || {
                if let Some((own, whole)) = &piece.own {
                    painter.bitmap(own, *whole, to.frame);
                }
                if let Some(from) = piece.from {
                    painter.bitmap(image, card::part(from.part), from.over(to.frame));
                }
            });
        }
    }

    /// The grip on the white under the pointer, or held: gray, and Kata's violet as
    /// it's held, or the highlight in contrast themes.
    pub(super) fn draw_grip(&self, painter: &Painter, now: Instant) {
        let Some(grip) = &self.grip else {
            return;
        };
        let shown = grip.shown.value(now);
        if shown <= 0.0 || self.cropping.is_some() || self.landing.is_some() {
            return;
        }
        let collage = self.arranging.as_ref().unwrap_or(self.document.collage());
        let Some(divider) = collage.dividers().get(grip.index).copied() else {
            return;
        };
        let Some(strip) = divider.strip.intersect(&self.bounds()) else {
            return;
        };
        let strip = self.view.area(strip);
        let held = grip.held.value(now);
        let (thick, long) = (GRIP.0, GRIP.1 * (1.0 + GRIP_HELD * held));
        let (width, height) = if divider.across {
            (thick.min(strip.width), long.min(strip.height / 2.0))
        } else {
            (long.min(strip.width / 2.0), thick.min(strip.height))
        };
        let area = painter.align(Area::new(
            strip.left + (strip.width - width) / 2.0,
            strip.middle() - height / 2.0,
            width,
            height,
        ));
        let radius = area.width.min(area.height) / 2.0;
        let palette = &self.palette;
        if palette.contrast {
            painter.fill(area, radius, faded(palette.accent, shown));
        } else {
            // The white between is white in either theme.
            let violet = theme::signature(true)[1];
            painter.fill(
                area,
                radius,
                color(0x000000, GRIP_DARK * shown * (1.0 - held)),
            );
            painter.fill(area, radius, color(violet, shown * held));
        }
    }
}

/// How far along `point` is the way `divider` moves.
fn along(divider: Divider, (x, y): (f32, f32)) -> f32 {
    if divider.across { x } else { y }
}
