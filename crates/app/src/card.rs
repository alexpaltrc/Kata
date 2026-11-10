//! The card a capture becomes as it moves, Kata's signature: lifted off with its
//! shadows, its light and its edge, and crossed by a glint. The editor carries
//! captures away as one, and the welcome's Ready shows the first capture as one.

use kata_editor::{Image, Rect};
use kata_ui::{
    paint::{Area, Painter, Point},
    theme::color,
};
use windows::{
    Win32::Graphics::{
        Direct2D::{
            Common::{D2D_SIZE_U, D2D1_ALPHA_MODE_IGNORE, D2D1_PIXEL_FORMAT},
            D2D1_BITMAP_OPTIONS_NONE, D2D1_BITMAP_PROPERTIES1, ID2D1Bitmap1, ID2D1DeviceContext,
        },
        Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
    },
    core::Result,
};

/// The most the card measures, in DIPs; its corners, as Windows 11's overlays
/// have; how far it leans, in degrees clockwise; how far above its bottom edge the
/// pointer holds it, so what Windows says about a drop shows below; and the room
/// around it for its shadows.
pub const CARD: (f32, f32) = (200.0, 140.0);
const RADIUS: f32 = 8.0;
pub const LEAN: f32 = 3.0;
const HOLD: f32 = 12.0;
pub const ROOM: f32 = 36.0;
/// Its shadows, lifted off: a wide one far below and a close one just under it,
/// each blurred over so many DIPs, cast so far down and so dark.
const SHADOWS: [(f32, f32, f32); 2] = [(36.0, 16.0, 0.34), (6.0, 2.0, 0.22)];
/// Its edge in Kata's own colors, in DIPs; and the glint that crosses it, for its
/// width, its share of the card's, and its opacity.
const RIM: f32 = 2.5;
const GLINT_BAND: f32 = 0.6;
const GLINT_OPACITY: f32 = 0.5;
/// A thumbnail keeps at least this many pixels across and down, enough for the
/// card on a screen at 200 % and then some.
const THUMBNAIL: (u32, u32) = (2 * 2 * CARD.0 as u32, 2 * 2 * CARD.1 as u32);

/// Where the card is, and how lifted off.
#[derive(Clone, Copy)]
pub struct Pose {
    /// Upright, before it leans around its center.
    area: Area,
    /// In degrees clockwise.
    lean: f32,
    radius: f32,
    /// From 0, lying flat as the capture, to 1, lifted off as a card.
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
            radius: RADIUS,
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

    /// The same, lying flat and upright where it is.
    pub fn laid(self) -> Self {
        Self {
            lean: 0.0,
            lift: 0.0,
            ..self
        }
    }

    /// Moved `across` and `down`.
    pub fn moved(mut self, (across, down): (f32, f32)) -> Self {
        self.area.left += across;
        self.area.top += down;
        self
    }

    /// The pose `progress` of the way to `to`.
    pub fn toward(self, to: Self, progress: f32) -> Self {
        let mix = |from: f32, to: f32| from + (to - from) * progress;
        Self {
            area: self.area.toward(to.area, progress),
            lean: mix(self.lean, to.lean),
            // A spring past its mark leans the other way, but lifts no lower.
            radius: mix(self.radius, to.radius).max(0.0),
            lift: mix(self.lift, to.lift).max(0.0),
        }
    }

    pub fn area(&self) -> Area {
        self.area
    }

    pub fn center(&self) -> Point {
        (self.area.left + self.area.width / 2.0, self.area.middle())
    }
}

/// Draws the `part` of `image`, in its pixels, as the card posed `pose`: the more lifted off,
/// the farther its shadows fall and the more its light and edge show. A glint
/// crosses it `glint` of the way, if any.
pub fn draw(painter: &Painter, image: &ID2D1Bitmap1, part: Area, pose: Pose, glint: Option<f32>) {
    let Pose {
        area,
        lean,
        radius,
        lift,
    } = pose;
    painter.turned(lean, pose.center(), || {
        for (blur, down, dark) in SHADOWS {
            let shadow = color(0x000000, dark * lift);
            painter.cast(area, radius, blur * lift, down * lift, shadow);
        }
        painter.clip_rounded(area, radius, || painter.bitmap(image, part, area));
        painter.sheen(area, radius, lift);
        if let Some(glint) = glint {
            let band = GLINT_BAND * area.width;
            let x = area.left - band / 2.0 + (area.width + band) * glint;
            painter.glint_across(area, radius, x, band, GLINT_OPACITY);
        }
        painter.rim(area, radius, RIM, lift);
    });
}

/// The `rect` of a bitmap's pixels, as `Painter::bitmap` takes it.
pub fn part(rect: Rect) -> Area {
    Area::new(
        rect.left as f32,
        rect.top as f32,
        rect.width() as f32,
        rect.height() as f32,
    )
}

/// `image`, copied to the graphics card that `context` draws on.
pub fn bitmap(context: &ID2D1DeviceContext, image: &Image) -> Result<ID2D1Bitmap1> {
    unsafe {
        context.CreateBitmap(
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
        )
    }
}

/// A capture, shrunk as far as its card allows, and the size it had.
pub struct Thumbnail {
    pub image: Image,
    /// In pixels.
    pub size: (u32, u32),
}

impl Thumbnail {
    pub fn of(image: &Image) -> Self {
        let factor = (image.width() / THUMBNAIL.0).min(image.height() / THUMBNAIL.1);
        Self {
            image: image.shrunk(factor.max(1)),
            size: (image.width(), image.height()),
        }
    }
}
