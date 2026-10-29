//! Fluent controls, drawn with Direct2D. Positions and sizes are in DIPs.

mod button;
mod icon_button;
mod selector_bar;
mod shortcut;
mod slider;
mod toggle;

use std::time::Instant;

pub use button::Button;
pub use icon_button::IconButton;
pub use selector_bar::SelectorBar;
pub use shortcut::Shortcut;
pub use slider::Slider;
pub use toggle::Toggle;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    VIRTUAL_KEY, VK_DOWN, VK_END, VK_HOME, VK_LEFT, VK_RETURN, VK_RIGHT, VK_SPACE, VK_UP,
};

use crate::{
    access::Element,
    paint::{Area, Painter, Point},
};

/// The standard height of a control.
pub const HEIGHT: f32 = 32.0;

/// How a control follows the pointer and the keyboard, and draws itself.
pub trait Control {
    fn contains(&self, point: Point) -> bool;

    /// The pointer moved over the window (`Some`) or left it. Returns whether the
    /// control's look changes.
    fn hover(&mut self, point: Option<Point>) -> bool;

    /// The pointer went down on the control. Returns whether its value changed.
    fn press(&mut self, point: Point) -> bool;

    /// The pointer moved while pressing the control. Returns whether its value
    /// changed.
    fn drag(&mut self, _point: Point) -> bool {
        false
    }

    /// The pointer went up. Returns whether that activated the control.
    fn release(&mut self, point: Point) -> bool;

    /// The press ended without the pointer going up, as when another window took it.
    fn cancel(&mut self);

    /// Whether Tab stops on it.
    fn is_focusable(&self) -> bool {
        true
    }

    /// What the focus visual goes around, and its corner radius.
    fn focus_area(&self) -> (Area, f32);

    /// The keyboard, or an assistive technology such as Narrator, asks `action` of
    /// the control. Returns whether that activated it or changed its value.
    fn act(&mut self, action: Action) -> bool;

    /// Adds what assistive technologies know of the control to `elements`, under
    /// the window's own number for it, `control`.
    fn describe(&self, control: usize, elements: &mut Vec<Element>);

    fn draw(&self, painter: &Painter, now: Instant);

    fn is_animating(&self, now: Instant) -> bool;
}

/// What the keyboard or an assistive technology asks of a control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Enter or Space.
    Activate,
    /// Left or Down.
    Previous,
    /// Right or Up.
    Next,
    /// Home.
    First,
    /// End.
    Last,
    /// Straight to a step, as Narrator asks.
    To(usize),
}

impl Action {
    /// What `key` asks of the control with the keyboard focus.
    pub fn of(key: VIRTUAL_KEY) -> Option<Self> {
        match key {
            VK_RETURN | VK_SPACE => Some(Self::Activate),
            VK_LEFT | VK_DOWN => Some(Self::Previous),
            VK_RIGHT | VK_UP => Some(Self::Next),
            VK_HOME => Some(Self::First),
            VK_END => Some(Self::Last),
            _ => None,
        }
    }

    /// The step among `count` that this action goes to from `current`, for actions
    /// that move through steps.
    pub fn step(self, current: usize, count: usize) -> Option<usize> {
        let last = count.checked_sub(1)?;
        match self {
            Self::Activate => None,
            Self::Previous => Some(current.saturating_sub(1)),
            Self::Next => Some((current + 1).min(last)),
            Self::First => Some(0),
            Self::Last => Some(last),
            Self::To(step) => Some(step.min(last)),
        }
    }
}

/// Where Tab, or Shift + Tab when `back`, takes the keyboard focus from `focus`,
/// among `count` controls in order, of which only those `focusable` can take it.
/// It goes around from the last to the first.
pub fn tab(
    count: usize,
    focus: Option<usize>,
    back: bool,
    focusable: impl Fn(usize) -> bool,
) -> Option<usize> {
    let last = count.checked_sub(1)?;
    let step = if back { last } else { 1 };
    // From nowhere, Tab starts at the first control and Shift + Tab at the last.
    let from = focus.unwrap_or(if back { 0 } else { last });
    (1..=count)
        .map(|n| (from + n * step) % count)
        .find(|&index| focusable(index))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_goes_around_and_skips_what_cant_take_focus() {
        let all = |_| true;
        assert_eq!(tab(3, None, false, all), Some(0));
        assert_eq!(tab(3, None, true, all), Some(2));
        assert_eq!(tab(3, Some(2), false, all), Some(0));
        assert_eq!(tab(3, Some(0), true, all), Some(2));
        assert_eq!(tab(3, Some(0), false, |index| index != 1), Some(2));
        assert_eq!(tab(3, Some(0), false, |_| false), None);
        assert_eq!(tab(0, None, false, all), None);
    }

    #[test]
    fn actions_step_within_bounds() {
        assert_eq!(Action::Previous.step(0, 3), Some(0));
        assert_eq!(Action::Next.step(1, 3), Some(2));
        assert_eq!(Action::Next.step(2, 3), Some(2));
        assert_eq!(Action::Last.step(0, 3), Some(2));
        assert_eq!(Action::First.step(2, 3), Some(0));
        assert_eq!(Action::To(1).step(0, 3), Some(1));
        assert_eq!(Action::To(9).step(0, 3), Some(2));
        assert_eq!(Action::Activate.step(1, 3), None);
        assert_eq!(Action::Next.step(0, 0), None);
    }
}
