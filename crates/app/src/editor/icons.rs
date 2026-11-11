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

/// Pasos's icon: a step marker, a circle with a 1 in it, `size` DIPs square. Filled
/// in, the 1 is cut out of the disc.
pub fn steps(graphics: &Graphics, size: f32) -> Result<(Figure, Figure)> {
    let unit = size / GRID;
    let at = |x: f32, y: f32| (x * unit, y * unit);
    let disc = Figure::new(graphics, |pen| pen.circle(at(10.0, 10.0), 7.5 * unit))?;
    let ring = disc.traced(LINE * unit)?;
    let one = Figure::new(graphics, |pen| {
        pen.start(at(8.875, 8.0));
        pen.line(at(11.125, 6.25));
        pen.line(at(11.125, 13.75));
    })?
    .traced(LINE * unit)?;
    Ok((ring.and(&one)?, disc.and(&ring)?.without(&one)?))
}

/// Formas's icon: a circle behind a square, `size` DIPs square. Filled in, both are,
/// a little apart.
pub fn shapes(graphics: &Graphics, size: f32) -> Result<(Figure, Figure)> {
    let unit = size / GRID;
    let disc = Figure::new(graphics, |pen| {
        pen.circle((7.5 * unit, 7.5 * unit), 5.0 * unit)
    })?;
    let square = Figure::new(graphics, |pen| {
        let side = 10.0 * unit;
        pen.rounded(Area::new(7.5 * unit, 7.5 * unit, side, side), 2.0 * unit);
    })?;
    let ring = disc.traced(LINE * unit)?;
    let frame = square.traced(LINE * unit)?;
    let outlined = frame.and(&ring.without(&square)?)?;
    // A line's width clear of the square's outline.
    let clear = square.and(&square.traced(3.0 * LINE * unit)?)?;
    let filled = square
        .and(&frame)?
        .and(&disc.and(&ring)?.without(&clear)?)?;
    Ok((outlined, filled))
}

/// A rectangle figure's icon, `size` DIPs square.
pub fn rectangle(graphics: &Graphics, size: f32) -> Result<(Figure, Figure)> {
    let unit = size / GRID;
    let area = Area::new(2.5 * unit, 4.5 * unit, 15.0 * unit, 11.0 * unit);
    let rectangle = Figure::new(graphics, |pen| pen.rounded(area, 2.0 * unit))?;
    let outline = rectangle.traced(LINE * unit)?;
    let filled = rectangle.and(&outline)?;
    Ok((outline, filled))
}

/// An ellipse figure's icon, `size` DIPs square.
pub fn ellipse(graphics: &Graphics, size: f32) -> Result<(Figure, Figure)> {
    let unit = size / GRID;
    let ellipse = Figure::new(graphics, |pen| {
        pen.ellipse((10.0 * unit, 10.0 * unit), (7.5 * unit, 5.5 * unit))
    })?;
    let outline = ellipse.traced(LINE * unit)?;
    let filled = ellipse.and(&outline)?;
    Ok((outline, filled))
}

/// An arrow figure's icon, pointing up and right, `size` DIPs square. It has no
/// inside, so filled in, its line is bolder.
pub fn arrow(graphics: &Graphics, size: f32) -> Result<(Figure, Figure)> {
    let unit = size / GRID;
    let at = |x: f32, y: f32| (x * unit, y * unit);
    let arrow = Figure::new(graphics, |pen| {
        pen.start(at(4.5, 15.5));
        pen.line(at(15.5, 4.5));
        pen.start(at(8.5, 4.5));
        pen.line(at(15.5, 4.5));
        pen.line(at(15.5, 11.5));
    })?;
    Ok((arrow.traced(LINE * unit)?, arrow.traced(1.6 * LINE * unit)?))
}
