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

use crate::paint::{Painter, Point};

/// The standard height of a control.
pub const HEIGHT: f32 = 32.0;

/// How a control follows the pointer and draws itself.
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

    fn draw(&self, painter: &Painter, now: Instant);

    fn is_animating(&self, now: Instant) -> bool;
}
