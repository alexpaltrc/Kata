//! Showing the capture: its place in the window, its copy on the graphics card, and
//! the outlines, handles and chip drawn over it.

use std::time::Instant;

use kata_editor::{Image, Rect};
use kata_ui::{
    Graphics, Surface,
    motion::{self, Animation, Easing},
    paint::{Area, Label, Painter, Point},
    theme::{Palette, color},
};
use windows::{
    Win32::{
        Foundation::HWND,
        Graphics::{
            Direct2D::{
                Common::{
                    D2D_RECT_U, D2D_SIZE_U, D2D1_ALPHA_MODE_IGNORE, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
                },
                D2D1_BITMAP_OPTIONS_NONE, D2D1_BITMAP_PROPERTIES1, ID2D1Bitmap1,
            },
            Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
        },
    },
    core::Result,
};

use super::{MARGIN, TITLE_BAR};
use crate::export::Picture;

/// The selected shape's outline width, in DIPs.
pub const SELECTED: f32 = 2.0;
/// The hovered shape's.
pub const HOVERED: f32 = 1.0;
/// The resize handles' radius, and how far from their center they can be grabbed.
const HANDLE: f32 = 6.0;
pub const HANDLE_REACH: f32 = 8.0;
/// The remove chip's radius, and where it sits from the shape's top-right corner.
const CHIP: f32 = 10.0;
const CHIP_OFFSET: f32 = 16.0;
/// The most the capture magnifies: each of its pixels this many screen pixels wide.
const MAX_ZOOM: f32 = 8.0;

/// The most the card that a dragged capture shrinks into measures, in DIPs; its
/// corners, as Windows 11's overlays have; how far it leans, in degrees clockwise;
/// how far above its bottom edge the pointer holds it, so what Windows says about
/// the drop shows below; and the room around it for its shadows.
const CARD: (f32, f32) = (200.0, 140.0);
const CARD_RADIUS: f32 = 8.0;
const LEAN: f32 = 3.0;
const HOLD: f32 = 12.0;
const CARD_ROOM: f32 = 36.0;
/// The card's shadows, lifted off: a wide one far below and a close one just under
/// it, each blurred over so many DIPs, cast so far down and so dark.
const CARD_SHADOWS: [(f32, f32, f32); 2] = [(36.0, 16.0, 0.34), (6.0, 2.0, 0.22)];
/// Its edge in Kata's own colors, in DIPs; and the glint that crosses it as it
/// lifts off, for its width, its share of the card's, and its opacity.
const RIM: f32 = 2.5;
const GLINT_BAND: f32 = 0.6;
const GLINT_OPACITY: f32 = 0.5;

const SHADOW: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.35,
};
pub const WHITE: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};

/// What the editor draws with on the graphics card.
pub struct Gpu {
    pub surface: Surface,
    /// The edited capture.
    pub image: ID2D1Bitmap1,
}

impl Gpu {
    /// Covers `hwnd`, whose client area measures `size` pixels, and copies `image`.
    pub fn new(graphics: &Graphics, hwnd: HWND, size: (u32, u32), image: &Image) -> Result<Self> {
        let surface = Surface::new(graphics, hwnd, size)?;
        let context = surface.context();
        unsafe {
            let image = context.CreateBitmap(
                D2D_SIZE_U {
                    width: image.width(),
                    height: image.height(),
                },
                Some(image.pixels().as_ptr().cast()),
                image.stride() as u32,
                &D2D1_BITMAP_PROPERTIES1 {
                    pixelFormat: D2D1_PIXEL_FORMAT {
                        format: DXGI_FORMAT_B8G8R8A8_UNORM,
                        alphaMode: D2D1_ALPHA_MODE_IGNORE,
                    },
                    dpiX: 96.0,
                    dpiY: 96.0,
                    bitmapOptions: D2D1_BITMAP_OPTIONS_NONE,
                    ..Default::default()
                },
            )?;
            Ok(Self { surface, image })
        }
    }

    /// The card the capture shrinks into to carry it away, as a picture `scale`
    /// pixels to the DIP.
    pub fn card(&self, palette: &Palette, scale: f32) -> Result<Picture> {
        let size = unsafe { self.image.GetPixelSize() };
        let size = (size.width as f32 / scale, size.height as f32 / scale);
        let held = Pose::held(size, (0.0, 0.0));
        let (width, height) = (held.area.width, held.area.height);
        let (sin, cos) = LEAN.to_radians().sin_cos();
        // Leaning, it takes a little more room.
        let room = |across: f32, down: f32| {
            ((across * cos + down * sin + 2.0 * CARD_ROOM) * scale).ceil() as u32
        };
        let pixels = (room(width, height), room(height, width));
        let center = held.center();
        let pointer = (
            pixels.0 as f32 / scale / 2.0 - center.0,
            pixels.1 as f32 / scale / 2.0 - center.1,
        );
        let drawn = self.surface.picture(palette, scale, pixels, |painter| {
            draw_card(painter, &self.image, Pose::held(size, pointer), None);
        })?;
        Ok(Picture {
            pixels: drawn,
            size: pixels,
            pointer: (
                (pointer.0 * scale).round() as i32,
                (pointer.1 * scale).round() as i32,
            ),
        })
    }

    /// Copies the `areas` of `image` that changed.
    pub fn upload(&self, image: &Image, areas: &[Rect]) -> Result<()> {
        for area in areas {
            let first = &image.pixels()[image.offset(area.left, area.top)..];
            let destination = D2D_RECT_U {
                left: area.left as u32,
                top: area.top as u32,
                right: area.right as u32,
                bottom: area.bottom as u32,
            };
            unsafe {
                self.image.CopyFromMemory(
                    Some(&destination),
                    first.as_ptr().cast(),
                    image.stride() as u32,
                )?;
            }
        }
        Ok(())
    }
}

/// Where the card a dragged capture shrinks into is, and how lifted off.
#[derive(Clone, Copy)]
pub struct Pose {
    /// Upright, before it leans around its center.
    area: Area,
    /// In degrees clockwise.
    lean: f32,
    radius: f32,
    /// From 0, lying in the window as the capture, to 1, lifted off as a card.
    lift: f32,
}

impl Pose {
    /// The capture, lying over `area`.
    pub fn flat(area: Area) -> Self {
        Self {
            area,
            lean: 0.0,
            radius: 0.0,
            lift: 0.0,
        }
    }

    /// The card that a capture `size` DIPs across and down, shown pixel for pixel,
    /// shrinks into, around `center`.
    pub fn card(size: (f32, f32), center: Point) -> Self {
        let fit = (CARD.0 / size.0).min(CARD.1 / size.1).min(1.0);
        let (width, height) = (size.0 * fit, size.1 * fit);
        Self {
            area: Area::new(
                center.0 - width / 2.0,
                center.1 - height / 2.0,
                width,
                height,
            ),
            lean: LEAN,
            radius: CARD_RADIUS,
            lift: 1.0,
        }
    }

    /// The card, as `pointer` holds it.
    pub fn held(size: (f32, f32), pointer: Point) -> Self {
        let card = Self::card(size, pointer);
        let (sin, cos) = LEAN.to_radians().sin_cos();
        let below = (card.area.height / 2.0 - HOLD).max(0.0);
        card.moved((below * sin, -below * cos))
    }

    /// The card, hanging from the middle of its top edge at `(x, top)`.
    pub fn hanging(size: (f32, f32), (x, top): Point) -> Self {
        let card = Self::card(size, (x, top));
        card.moved((0.0, card.area.height / 2.0))
    }

    /// Moved `across` and `down`.
    pub fn moved(mut self, (across, down): (f32, f32)) -> Self {
        self.area.left += across;
        self.area.top += down;
        self
    }

    /// Shrunk around its center to fit in `most`, settling closer as it shrinks.
    pub fn shrunk(self, most: (f32, f32)) -> Self {
        let area = self.area;
        let fit = (most.0 / area.width).min(most.1 / area.height).min(1.0);
        let (x, y) = self.center();
        let (width, height) = (area.width * fit, area.height * fit);
        Self {
            area: Area::new(x - width / 2.0, y - height / 2.0, width, height),
            lean: self.lean,
            radius: (self.radius * fit).max(2.0),
            lift: self.lift * fit.sqrt(),
        }
    }

    /// The pose `progress` of the way to `to`.
    pub fn toward(self, to: Self, progress: f32) -> Self {
        let mix = |from: f32, to: f32| from + (to - from) * progress;
        Self {
            area: Area::new(
                mix(self.area.left, to.area.left),
                mix(self.area.top, to.area.top),
                mix(self.area.width, to.area.width),
                mix(self.area.height, to.area.height),
            ),
            lean: mix(self.lean, to.lean),
            radius: mix(self.radius, to.radius),
            lift: mix(self.lift, to.lift),
        }
    }

    pub fn center(&self) -> Point {
        (self.area.left + self.area.width / 2.0, self.area.middle())
    }
}

/// Draws `image` as the card posed `pose`: the more lifted off, the farther its
/// shadows fall and the more its light and edge show. A glint crosses it `glint` of
/// the way, if any.
pub fn draw_card(painter: &Painter, image: &ID2D1Bitmap1, pose: Pose, glint: Option<f32>) {
    let Pose {
        area,
        lean,
        radius,
        lift,
    } = pose;
    painter.turned(lean, pose.center(), || {
        for (blur, down, dark) in CARD_SHADOWS {
            let shadow = color(0x000000, dark * lift);
            painter.cast(area, radius, blur * lift, down * lift, shadow);
        }
        painter.clip_rounded(area, radius, || painter.bitmap(image, area));
        painter.sheen(area, radius, lift);
        if let Some(glint) = glint {
            let band = GLINT_BAND * area.width;
            let x = area.left - band / 2.0 + (area.width + band) * glint;
            painter.glint_across(area, radius, x, band, GLINT_OPACITY);
        }
        painter.rim(area, radius, RIM, lift);
    });
}

/// What the capture shows in: a window of `window` DIPs, each `scale` pixels wide,
/// for an image of `image` pixels, with the toolbar taking `bottom` DIPs at the
/// bottom.
#[derive(Clone, Copy)]
pub struct Frame {
    pub window: (f32, f32),
    pub image: (u32, u32),
    pub scale: f32,
    pub bottom: f32,
}

impl Frame {
    /// The capture centered below the title bar: pixel for pixel when it fits, and
    /// shrunk to fit otherwise.
    pub fn fit(&self) -> View {
        let room = self.room();
        let zoom = (1.0 / self.scale)
            .min(room.0 / self.image.0 as f32)
            .min(room.1 / self.image.1 as f32)
            .max(1e-3);
        self.place(View {
            left: 0.0,
            top: 0.0,
            zoom,
        })
    }

    /// `view` magnified `factor` times around `at`, no less than fitting and no more
    /// than `MAX_ZOOM`.
    pub fn zoom(&self, view: View, factor: f32, at: Point) -> View {
        let zoom = (view.zoom * factor).clamp(self.fit().zoom, MAX_ZOOM / self.scale);
        let grow = zoom / view.zoom;
        self.place(View {
            left: at.0 - (at.0 - view.left) * grow,
            top: at.1 - (at.1 - view.top) * grow,
            zoom,
        })
    }

    /// The room for the capture, inside the margins and above the toolbar.
    fn room(&self) -> (f32, f32) {
        (
            self.window.0 - 2.0 * MARGIN,
            self.window.1 - TITLE_BAR - MARGIN - self.bottom,
        )
    }

    /// `view`, centered along each side the capture fits, and filling the room along
    /// each side it doesn't.
    fn place(&self, view: View) -> View {
        let room = self.room();
        let snap = |value: f32| (value * self.scale).round() / self.scale;
        let place = |at: f32, start: f32, room: f32, pixels: u32| {
            let size = pixels as f32 * view.zoom;
            snap(if size <= room {
                start + (room - size) / 2.0
            } else {
                at.clamp(start + room - size, start)
            })
        };
        View {
            left: place(view.left, MARGIN, room.0, self.image.0),
            top: place(view.top, TITLE_BAR + MARGIN, room.1, self.image.1),
            zoom: view.zoom,
        }
    }
}

/// Where the capture sits in the window.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct View {
    /// The capture's top-left corner, in DIPs.
    left: f32,
    top: f32,
    /// DIPs per capture pixel.
    pub zoom: f32,
}

impl View {
    /// The view `progress` of the way to `to`. A capture pixel both show in the same
    /// place stays there all the way.
    fn toward(self, to: Self, progress: f32) -> Self {
        let mix = |from: f32, to: f32| from + (to - from) * progress;
        Self {
            left: mix(self.left, to.left),
            top: mix(self.top, to.top),
            zoom: mix(self.zoom, to.zoom),
        }
    }

    /// Where `rect`, in capture pixels, shows.
    pub fn area(&self, rect: Rect) -> Area {
        Area::new(
            self.left + rect.left as f32 * self.zoom,
            self.top + rect.top as f32 * self.zoom,
            rect.width() as f32 * self.zoom,
            rect.height() as f32 * self.zoom,
        )
    }

    /// Where the middle of capture pixel `x`, `y` shows.
    pub fn center(&self, (x, y): (i32, i32)) -> Point {
        (
            self.left + (x as f32 + 0.5) * self.zoom,
            self.top + (y as f32 + 0.5) * self.zoom,
        )
    }

    /// The capture pixel under `point`.
    pub fn pixel(&self, (x, y): Point) -> (i32, i32) {
        (
            ((x - self.left) / self.zoom).floor() as i32,
            ((y - self.top) / self.zoom).floor() as i32,
        )
    }

    /// The edge between capture pixels nearest to `point`, inside `bounds`.
    pub fn edge(&self, (x, y): Point, bounds: Rect) -> (i32, i32) {
        (
            (((x - self.left) / self.zoom).round() as i32).clamp(bounds.left, bounds.right),
            (((y - self.top) / self.zoom).round() as i32).clamp(bounds.top, bounds.bottom),
        )
    }
}

/// A view gliding to another as the capture zooms.
#[derive(Clone, Copy)]
pub struct Glide {
    from: View,
    to: View,
    progress: Animation,
}

impl Glide {
    pub fn rest(view: View) -> Self {
        Self {
            from: view,
            to: view,
            progress: Animation::idle(1.0),
        }
    }

    /// Heads for `to` from where the view is now.
    pub fn retarget(&self, to: View, now: Instant) -> Self {
        Self {
            from: self.at(now),
            to,
            progress: Animation::new(0.0, 1.0, motion::FAST, Easing::DECELERATE),
        }
    }

    pub fn target(&self) -> View {
        self.to
    }

    pub fn at(&self, now: Instant) -> View {
        self.from.toward(self.to, self.progress.value(now))
    }

    pub fn is_running(&self, now: Instant) -> bool {
        self.progress.is_running(now)
    }
}

/// A rectangle's corners, clockwise from the top-left.
pub fn corners<T: Copy>(left: T, top: T, right: T, bottom: T) -> [(T, T); 4] {
    [(left, top), (right, top), (right, bottom), (left, bottom)]
}

/// Where a shape's resize handles are: on the corners of its selected outline.
pub fn handles(shape: Area) -> [Point; 4] {
    let line = shape.inflate(SELECTED / 2.0);
    corners(line.left, line.top, line.right(), line.bottom())
}

/// Where a shape's remove chip is, in a window of `window` DIPs, while the shape
/// shows in it.
pub fn chip(shape: Area, window: (f32, f32)) -> Option<Point> {
    let shows = shape.right() > 0.0
        && shape.left < window.0
        && shape.bottom() > TITLE_BAR
        && shape.top < window.1;
    shows.then(|| {
        (
            (shape.right() + CHIP_OFFSET).min(window.0 - CHIP - 4.0),
            (shape.top - CHIP_OFFSET).max(TITLE_BAR + CHIP + 2.0),
        )
    })
}

pub fn chip_contains(center: Point, point: Point) -> bool {
    distance(center, point) <= CHIP
}

pub fn distance(a: Point, b: Point) -> f32 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

/// Rings `area` from outside with a line `width` DIPs thick, edged with a faint
/// shadow so it shows on any image.
pub fn draw_outline(painter: &Painter, area: Area, radius: f32, width: f32, color: D2D1_COLOR_F) {
    let area = painter.align(area);
    let width = painter.snap(width).max(painter.pixel());
    let shadow = painter.border();
    let edge = width + shadow;
    painter.stroke(
        area.inflate(edge),
        radius + edge,
        width + 2.0 * shadow,
        SHADOW,
    );
    painter.stroke(area.inflate(width), radius + width, width, color);
}

pub fn draw_handles(painter: &Painter, handles: &[Point]) {
    for &center in handles {
        painter.circle(center, HANDLE, WHITE);
        painter.ring(center, HANDLE, SELECTED, painter.palette.accent);
    }
}

/// The button that removes the selected shape.
pub fn draw_chip(painter: &Painter, center: Point, glyph: &Label, hovered: bool) {
    let palette = painter.palette;
    painter.circle(center, CHIP, palette.control);
    if hovered {
        painter.circle(center, CHIP, palette.subtle_hover);
    }
    painter.ring(center, CHIP, painter.pixel(), palette.control_stroke);
    painter.text(glyph, center.0 - glyph.width / 2.0, center.1, palette.text);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 1000 x 500 window at 100 %, for a capture twice as big.
    const FRAME: Frame = Frame {
        window: (1000.0, 500.0),
        image: (1904, 856),
        scale: 1.0,
        bottom: 124.0,
    };

    #[test]
    fn zooming_keeps_the_pixel_under_the_pointer() {
        let at = (600.0, 300.0);
        let fit = FRAME.fit();
        let zoomed = FRAME.zoom(fit, 2.0, at);
        assert_eq!(zoomed.zoom, 2.0 * fit.zoom);
        assert_eq!(fit.pixel(at), zoomed.pixel(at));
    }

    #[test]
    fn zoom_stays_between_fitting_and_the_most() {
        let fit = FRAME.fit();
        assert_eq!(FRAME.zoom(fit, 0.5, (0.0, 0.0)), fit);
        let most = FRAME.zoom(fit, 1000.0, (500.0, 300.0));
        assert_eq!(most.zoom, MAX_ZOOM);
    }

    #[test]
    fn a_zoomed_capture_leaves_no_gap_at_its_edges() {
        let zoomed = FRAME.zoom(FRAME.fit(), 4.0, (0.0, TITLE_BAR));
        let capture = zoomed.area(Rect::new(0, 0, 1904, 856));
        assert_eq!((capture.left, capture.top), (MARGIN, TITLE_BAR + MARGIN));
    }
}
