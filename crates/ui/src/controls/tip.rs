//! The tooltip that names or explains a control once the pointer rests on it.

use crate::paint::{Area, Label, Painter};

/// Between the control and the tooltip.
const GAP: f32 = 4.0;
/// Around the tooltip's text: on each side, above and below, as in Fluent.
const PADDING: (f32, f32, f32) = (9.0, 6.0, 8.0);
const RADIUS: f32 = 8.0;

/// Draws a tooltip saying `text` at `opacity`, centered above `anchor`, or below it
/// when there's no room above, and kept within `bounds`. It goes over everything
/// else.
pub fn draw(painter: &Painter, text: &Label, anchor: Area, bounds: Area, opacity: f32) {
    painter.layer(opacity, 0.0, || {
        let tip = painter.align(area(text, anchor, bounds));
        painter.flyout(tip, RADIUS);
        let (side, above, _) = PADDING;
        let middle = tip.top + above + text.height / 2.0;
        painter.text(text, tip.left + side, middle, painter.palette.text);
    });
}

fn area(text: &Label, anchor: Area, bounds: Area) -> Area {
    let (side, above, below) = PADDING;
    let width = text.width + 2.0 * side;
    let height = above + text.height + below;
    let centered = anchor.left + (anchor.width - width) / 2.0;
    let left = centered.min(bounds.right() - width).max(bounds.left);
    let over = anchor.top - GAP - height;
    let top = if over >= bounds.top {
        over
    } else {
        anchor.bottom() + GAP
    };
    Area::new(left, top, width, height)
}
