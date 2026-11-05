//! The welcome's signals in the editor, as timelines in seconds: once a change
//! first saves itself, a copy of the capture flies into its folder, which opens to
//! name it; and the capture rises toward its name, to show it drags away from there.

use std::f32::consts::PI;

use kata_ui::{
    motion::{Easing, FAST},
    paint::Area,
};

use super::canvas::Pose;

/// The saved copy shows over the capture, lifts off as a card as a glint crosses
/// it, then arcs into the folder button, landing with a pop; the button opens into
/// a pill naming the folder, a glint crosses the name, and the pill holds a moment
/// and closes.
const SHOWN: f32 = 0.1;
const LIFT: f32 = SHOWN + 0.35;
const LANDED: f32 = LIFT + 0.6;
const POP: f32 = 0.25;
const OPENS: f32 = LANDED + 0.15;
const OPEN: f32 = 0.25;
const GLINT: f32 = 0.8;
const CLOSES: f32 = OPENS + OPEN + 1.0;
pub const SAVED: f32 = CLOSES + FAST.as_secs_f32();
/// How far the card rises as it lifts off, how high it arcs, the most it measures
/// as it lands, and how much the button grows then.
const RISE: f32 = 20.0;
const ARC: f32 = 50.0;
const LANDS: (f32, f32) = (22.0, 14.0);
const GROWS: f32 = 0.12;

/// The capture rises as a card to hang under its name, bobs there as if held, and
/// settles back: how long it takes, when it's risen and when it heads back, how
/// far it bobs, and how far under the name it hangs.
pub const REHEARSAL: f32 = 1.5;
const RISEN: f32 = 0.35;
const RETURNS: f32 = 1.15;
const BOB: f32 = 4.0;
const HANG: f32 = 10.0;

/// The saved copy of a capture `size` DIPs across and down, lying `flat`, on its
/// way to the folder button centered on `folder`, until it lands: how it's posed,
/// how opaque, and how far across it the glint is, while it crosses.
pub fn saved_card(
    seconds: f32,
    flat: Pose,
    size: (f32, f32),
    folder: (f32, f32),
) -> Option<(Pose, f32, Option<f32>)> {
    let (x, y) = flat.center();
    let lifted = Pose::card(size, (x, y - RISE));
    // It shows in right over what it copies, which leaves no seam, before it moves.
    if seconds < SHOWN {
        return Some((flat, seconds / SHOWN, None));
    }
    if seconds < LIFT {
        let progress = (seconds - SHOWN) / (LIFT - SHOWN);
        let pose = flat.toward(lifted, Easing::DECELERATE.apply(progress));
        return Some((pose, 1.0, Some(Easing::SWEEP.apply(progress))));
    }
    if seconds >= LANDED {
        return None;
    }
    let progress = (seconds - LIFT) / (LANDED - LIFT);
    let along = Easing::SWEEP.apply(progress);
    let landed = Pose::card(size, folder).shrunk(LANDS);
    let height = 4.0 * ARC * along * (1.0 - along);
    let pose = lifted.toward(landed, along).moved((0.0, -height));
    // It fades into the button over its last stretch.
    Some((pose, 1.0 - ((progress - 0.7) / 0.3).clamp(0.0, 1.0), None))
}

/// How much the folder button grows as the copy lands in it.
pub fn pop(seconds: f32) -> f32 {
    let progress = (seconds - LANDED) / POP;
    if (0.0..1.0).contains(&progress) {
        1.0 + GROWS * (PI * progress).sin()
    } else {
        1.0
    }
}

/// From 0, closed, to 1, as open as the pill naming the folder gets.
pub fn pill(seconds: f32) -> f32 {
    let opened = Easing::DECELERATE.apply((seconds - OPENS) / OPEN);
    let closed = Easing::DECELERATE.apply((seconds - CLOSES) / FAST.as_secs_f32());
    opened * (1.0 - closed)
}

/// How far across the folder's name the glint is, while it crosses.
pub fn folder_glint(seconds: f32) -> Option<f32> {
    let progress = (seconds - OPENS - OPEN) / GLINT;
    (0.0..1.0)
        .contains(&progress)
        .then(|| Easing::SWEEP.apply(progress))
}

/// How lifted off the capture is in its rehearsal, from 0 to 1, and how far up it
/// bobs.
fn rehearsal(seconds: f32) -> (f32, f32) {
    if seconds < RISEN {
        (Easing::DECELERATE.apply(seconds / RISEN), 0.0)
    } else if seconds < RETURNS {
        let progress = (seconds - RISEN) / (RETURNS - RISEN);
        (1.0, BOB * (PI * progress).sin())
    } else {
        let progress = (seconds - RETURNS) / (REHEARSAL - RETURNS);
        (1.0 - Easing::DECELERATE.apply(progress), 0.0)
    }
}

/// How lifted off the capture is in its rehearsal, from 0 to 1.
pub fn rehearsal_lift(seconds: f32) -> f32 {
    rehearsal(seconds).0
}

/// The card a capture `size` DIPs across and down, lying `flat`, rises as in its
/// rehearsal, to hang under its name over `name`.
pub fn rehearsed_card(seconds: f32, flat: Pose, size: (f32, f32), name: Area) -> Pose {
    let (lift, bob) = rehearsal(seconds);
    let under = (name.left + name.width / 2.0, name.bottom() + HANG);
    flat.toward(Pose::hanging(size, under), lift)
        .moved((0.0, -bob))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pill_opens_after_the_copy_lands_and_closes_by_the_end() {
        assert_eq!(pill(LANDED), 0.0);
        assert!((pill(OPENS + OPEN) - 1.0).abs() < 1e-4);
        assert!(pill(SAVED).abs() < 1e-4);
    }

    #[test]
    fn the_button_pops_only_as_the_copy_lands() {
        assert_eq!(pop(LANDED - 0.01), 1.0);
        assert!(pop(LANDED + POP / 2.0) > 1.1);
        assert_eq!(pop(LANDED + POP), 1.0);
    }

    #[test]
    fn the_rehearsal_starts_and_ends_at_rest() {
        assert_eq!(rehearsal(0.0), (0.0, 0.0));
        assert!(rehearsal(REHEARSAL).0.abs() < 1e-4);
        assert_eq!(rehearsal(RISEN + 0.01).0, 1.0);
    }
}
