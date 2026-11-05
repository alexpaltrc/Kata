//! The welcome's signals in the editor, as timelines in seconds: once a change
//! first saves itself, a tip under the folder button says so, and the button
//! opens to name the folder; and the name is carried a little way off, as if
//! dragged, with the capture hanging under it as a card, to show the capture
//! drags away from there.

use std::f32::consts::PI;

use kata_ui::{
    motion::{Easing, FAST},
    paint::Area,
};

use crate::card::Pose;

/// The folder button beats, and a tip comes out from under it to say the capture
/// saved itself; once the tip is back in, the button opens into a pill whose
/// folder name rises into place; a glint crosses the name, and the pill holds a
/// moment and closes. The tip goes faster than it came.
const POP: f32 = 0.25;
const TELLS: f32 = 0.1;
const TOLD: f32 = TELLS + 1.6;
const HIDE: f32 = 0.1;
const OPENS: f32 = TOLD + HIDE;
const OPEN: f32 = 0.25;
const GLINT: f32 = 0.8;
const CLOSES: f32 = OPENS + OPEN + 1.4;
pub const SAVED: f32 = CLOSES + FAST.as_secs_f32();
/// How much the button grows as it beats.
const GROWS: f32 = 0.12;

/// The name shows as under the pointer, grip and all, and the capture rises as a
/// card to hang under it; the name is carried a little way off and back, the card
/// following a beat late; and the card settles back. When each starts and how
/// long it takes, how far the name goes, across and down, how late the card
/// follows, and how far under the name it hangs.
const SHOWS: f32 = 0.2;
const LIFTS: f32 = 0.1;
const RISEN: f32 = LIFTS + 0.35;
const CARRIES: f32 = RISEN + 0.05;
const CARRY: f32 = 0.45;
const RETURNS: f32 = CARRIES + CARRY + 0.25;
const RETURN: f32 = 0.45;
const SETTLES: f32 = RETURNS + RETURN;
const SETTLE: f32 = 0.35;
pub const REHEARSAL: f32 = SETTLES + SETTLE;
const CARRIED: (f32, f32) = (20.0, 6.0);
const LAG: f32 = 0.06;
const HANG: f32 = 10.0;

/// How much the folder button grows as it beats.
pub fn pop(seconds: f32) -> f32 {
    let progress = seconds / POP;
    if (0.0..1.0).contains(&progress) {
        1.0 + GROWS * (PI * progress).sin()
    } else {
        1.0
    }
}

/// How much the tip that says the capture saved itself shows, from 0 to 1.
pub fn told(seconds: f32) -> f32 {
    let shown = Easing::DECELERATE.apply((seconds - TELLS) / FAST.as_secs_f32());
    let hidden = Easing::ACCELERATE.apply((seconds - TOLD) / HIDE);
    shown * (1.0 - hidden)
}

/// How the pill the folder button opens into shows: how far past the button it
/// reaches, how opaque its fill is, how opaque the folder's name is and how far
/// below its place, as a share of its rise, and how far across the name the glint
/// is, while it crosses.
#[derive(Debug)]
pub struct Pill {
    pub reach: f32,
    pub fill: f32,
    pub shown: f32,
    pub below: f32,
    pub glint: Option<f32>,
}

/// The pill, for a folder's name that takes it `reach` DIPs past the button.
pub fn pill(seconds: f32, reach: f32) -> Pill {
    let after = |start: f32, length: f32| (seconds - start) / length;
    let opened = Easing::DECELERATE.apply(after(OPENS, OPEN));
    let closed = Easing::DECELERATE.apply(after(CLOSES, FAST.as_secs_f32()));
    // The name rises in over the second half of the opening, and is gone by the
    // first half of the closing.
    let entered = (2.0 * opened - 1.0).clamp(0.0, 1.0);
    let gone = (2.0 * closed).min(1.0);
    let crossed = after(OPENS + OPEN, GLINT);
    let fill = opened * (1.0 - closed);
    Pill {
        reach: reach * fill,
        fill,
        shown: entered * (1.0 - gone),
        below: 1.0 - entered,
        glint: (0.0..1.0)
            .contains(&crossed)
            .then(|| Easing::SWEEP.apply(crossed)),
    }
}

/// How lifted off the capture is in its rehearsal, from 0 to 1.
pub fn rehearsal_lift(seconds: f32) -> f32 {
    let risen = Easing::DECELERATE.apply((seconds - LIFTS) / (RISEN - LIFTS));
    let settled = Easing::DECELERATE.apply((seconds - SETTLES) / SETTLE);
    risen * (1.0 - settled)
}

/// The name in its rehearsal: how far it's carried off, across and down, with
/// `room` DIPs to its right, and how much it shows as under the pointer, from 0
/// to 1.
pub fn rehearsed_name(seconds: f32, room: f32) -> ((f32, f32), f32) {
    // It's carried out and back like a spring, a little past each mark.
    let out = Easing::SPRING.apply((seconds - CARRIES) / CARRY);
    let back = Easing::SPRING.apply((seconds - RETURNS) / RETURN);
    let carried = out - back;
    let shown = Easing::DECELERATE.apply(seconds / SHOWS);
    let hidden = Easing::DECELERATE.apply((seconds - SETTLES) / SETTLE);
    let across = CARRIED.0.min(room.max(0.0));
    (
        (across * carried, CARRIED.1 * carried),
        shown * (1.0 - hidden),
    )
}

/// The card a capture `size` DIPs across and down, lying `flat`, rises as in its
/// rehearsal, to hang under its name, which is over `name(seconds)` then.
pub fn rehearsed_card(
    seconds: f32,
    flat: Pose,
    size: (f32, f32),
    name: impl Fn(f32) -> Area,
) -> Pose {
    let name = name(seconds - LAG);
    let under = (name.left + name.width / 2.0, name.bottom() + HANG);
    flat.toward(Pose::hanging(size, under), rehearsal_lift(seconds))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tip_says_it_saved_itself_before_the_pill_names_the_folder() {
        assert_eq!(told(0.0), 0.0);
        assert_eq!(told(TOLD), 1.0);
        assert!(told(OPENS).abs() < 1e-4);
        assert_eq!(pill(OPENS, 60.0).reach, 0.0);
        let named = pill(OPENS + OPEN, 60.0);
        assert!((named.reach - 60.0).abs() < 1e-3);
        assert!((named.shown - 1.0).abs() < 1e-4 && named.below.abs() < 1e-4);
        assert!(named.glint.is_some());
        assert!(pill(SAVED, 60.0).reach.abs() < 1e-3);
    }

    #[test]
    fn the_button_beats_as_it_opens() {
        assert!(pop(POP / 2.0) > 1.1);
        assert_eq!(pop(POP), 1.0);
    }

    #[test]
    fn the_rehearsal_starts_and_ends_at_rest() {
        assert_eq!(rehearsal_lift(0.0), 0.0);
        assert_eq!(rehearsal_lift(RISEN), 1.0);
        assert!(rehearsal_lift(REHEARSAL).abs() < 1e-4);
        assert_eq!(rehearsed_name(0.0, 100.0), ((0.0, 0.0), 0.0));
        let ((across, down), shown) = rehearsed_name(REHEARSAL, 100.0);
        assert!(across.abs() < 1e-3 && down.abs() < 1e-3 && shown.abs() < 1e-4);
    }

    #[test]
    fn the_name_is_carried_only_as_far_as_there_is_room() {
        let ((across, down), shown) = rehearsed_name(RETURNS, 100.0);
        assert!((across - CARRIED.0).abs() < 1e-3 && (down - CARRIED.1).abs() < 1e-3);
        assert_eq!(shown, 1.0);
        assert!((rehearsed_name(RETURNS, 5.0).0.0 - 5.0).abs() < 1e-3);
    }
}
