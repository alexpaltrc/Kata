//! Showing the capture: its place in the window, its copy on the graphics card, and
//! the outlines, handles and chip drawn over it.

use kata_editor::{Image, Rect};
use kata_ui::{
    Graphics, Surface,
    paint::{Area, Label, Painter, Point},
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
                ID2D1SolidColorBrush,
            },
            Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
        },
    },
    core::Result,
};

use super::{MARGIN, TITLE_BAR};

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
    pub brush: ID2D1SolidColorBrush,
    /// The edited capture.
    pub image: ID2D1Bitmap1,
}

impl Gpu {
    /// Covers `hwnd`, whose client area measures `size` pixels, and copies `image`.
    pub fn new(graphics: &Graphics, hwnd: HWND, size: (u32, u32), image: &Image) -> Result<Self> {
        let surface = Surface::new(graphics, hwnd, size)?;
        let context = surface.context();
        unsafe {
            let brush = context.CreateSolidColorBrush(&WHITE, None)?;
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
            Ok(Self {
                surface,
                brush,
                image,
            })
        }
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

/// Where the capture sits in the window.
#[derive(Clone, Copy, Debug, Default)]
pub struct View {
    /// The capture's top-left corner, in DIPs.
    left: f32,
    top: f32,
    /// DIPs per capture pixel.
    pub zoom: f32,
}

impl View {
    /// Centers an image of `image` pixels below the title bar of a window of `window`
    /// DIPs, each `scale` pixels wide. It shows pixel for pixel when it fits, and
    /// shrinks to fit otherwise.
    pub fn fit(window: (f32, f32), image: (u32, u32), scale: f32) -> Self {
        let room = (window.0 - 2.0 * MARGIN, window.1 - TITLE_BAR - 2.0 * MARGIN);
        let zoom = (1.0 / scale)
            .min(room.0 / image.0 as f32)
            .min(room.1 / image.1 as f32)
            .max(1e-3);
        let snap = |value: f32| (value * scale).round() / scale;
        let (width, height) = (image.0 as f32 * zoom, image.1 as f32 * zoom);
        Self {
            left: snap((window.0 - width) / 2.0),
            top: snap(TITLE_BAR + (window.1 - TITLE_BAR - height) / 2.0),
            zoom,
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

/// A rectangle's corners, clockwise from the top-left.
pub fn corners<T: Copy>(left: T, top: T, right: T, bottom: T) -> [(T, T); 4] {
    [(left, top), (right, top), (right, bottom), (left, bottom)]
}

/// Where a shape's resize handles are: on the corners of its selected outline.
pub fn handles(shape: Area) -> [Point; 4] {
    let line = shape.inflate(SELECTED / 2.0);
    corners(line.left, line.top, line.right(), line.bottom())
}

/// Where a shape's remove chip is, in a window `width` DIPs wide.
pub fn chip(shape: Area, width: f32) -> Point {
    (
        (shape.right() + CHIP_OFFSET).min(width - CHIP - 4.0),
        (shape.top - CHIP_OFFSET).max(TITLE_BAR + CHIP + 2.0),
    )
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
    let area = snapped(painter, area);
    let width = painter.snap(width).max(painter.pixel());
    let shadow = painter.snap(1.0).max(painter.pixel());
    let edge = width + shadow;
    painter.stroke(
        area.inflate(edge),
        radius + edge,
        width + 2.0 * shadow,
        SHADOW,
    );
    painter.stroke(area.inflate(width), radius + width, width, color);
}

pub fn draw_handles(painter: &Painter, shape: Area) {
    for center in handles(shape) {
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

/// `area` with its edges on the nearest pixel edges.
fn snapped(painter: &Painter, area: Area) -> Area {
    let (left, top) = (painter.snap(area.left), painter.snap(area.top));
    Area::new(
        left,
        top,
        painter.snap(area.right()) - left,
        painter.snap(area.bottom()) - top,
    )
}
