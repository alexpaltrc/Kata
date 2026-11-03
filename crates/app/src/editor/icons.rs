//! The editor's icons, drawn on a grid of 20 units with lines of 1.25, as Fluent's
//! are. Each comes outlined and filled in: the filled one is the outlined one with
//! its inside filled, so both are the same size.

use kata_ui::{Graphics, figure::Figure, paint::Area};
use windows::core::Result;

/// The grid's size, in units.
const GRID: f32 = 20.0;
/// The lines' width, in units.
const LINE: f32 = 1.25;

/// Desenfocar's droplet, `size` DIPs square: outlined and filled in.
pub fn blur(graphics: &Graphics, size: f32) -> Result<(Figure, Figure)> {
    let unit = size / GRID;
    let at = |x: f32, y: f32| (x * unit, y * unit);
    let drop = Figure::new(graphics, |pen| {
        pen.start(at(10.0, 1.75));
        pen.curve(at(7.5, 5.0), at(4.0, 8.5), at(4.0, 12.0));
        pen.arc(at(16.0, 12.0), 6.0 * unit, false, false);
        pen.curve(at(16.0, 8.5), at(12.5, 5.0), at(10.0, 1.75));
        pen.close();
    })?;
    let outline = drop.traced(LINE * unit)?;
    let filled = drop.and(&outline)?;
    Ok((outline, filled))
}

/// Foco's icon, Kata's own: four corners around an area in focus, `size` DIPs
/// square. Filled in, the area lights up.
pub fn focus(graphics: &Graphics, size: f32) -> Result<(Figure, Figure)> {
    const NEAR: f32 = 3.125;
    const FAR: f32 = GRID - NEAR;
    const ARM: f32 = 4.375;
    const CORNER: f32 = 2.5;
    let unit = size / GRID;
    let at = |x: f32, y: f32| (x * unit, y * unit);
    let corners = Figure::new(graphics, |pen| {
        let radius = CORNER * unit;
        for (x, y, across, down) in [
            (NEAR, NEAR, 1.0, 1.0),
            (FAR, NEAR, -1.0, 1.0),
            (FAR, FAR, -1.0, -1.0),
            (NEAR, FAR, 1.0, -1.0),
        ] {
            pen.start(at(x, y + down * ARM));
            pen.line(at(x, y + down * CORNER));
            pen.arc(
                at(x + across * CORNER, y),
                radius,
                across * down > 0.0,
                false,
            );
            pen.line(at(x + across * ARM, y));
        }
    })?
    .traced(LINE * unit)?;
    let area = Figure::new(graphics, |pen| {
        let side = 6.0 * unit;
        pen.rounded(Area::new(7.0 * unit, 7.0 * unit, side, side), 1.5 * unit);
    })?;
    let outline = area.traced(LINE * unit)?;
    Ok((corners.and(&outline)?, corners.and(&area.and(&outline)?)?))
}

/// The magnifier that leads the focus's zoom, `size` DIPs square.
pub fn zoom(graphics: &Graphics, size: f32) -> Result<Figure> {
    let unit = size / GRID;
    let at = |x: f32, y: f32| (x * unit, y * unit);
    Figure::new(graphics, |pen| {
        pen.circle(at(8.75, 8.75), 5.625 * unit);
        pen.start(at(12.75, 12.75));
        pen.line(at(16.875, 16.875));
    })?
    .traced(LINE * unit)
}
