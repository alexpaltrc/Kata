//! Drawing in device-independent pixels (DIPs, 1/96 of an inch), lined up with
//! the screen's pixels.

use windows::{
    Win32::Graphics::{
        Direct2D::{
            Common::{D2D_RECT_F, D2D1_COLOR_F},
            D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_ELLIPSE, D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC,
            D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR, D2D1_ROUNDED_RECT, ID2D1Bitmap,
            ID2D1DeviceContext, ID2D1SolidColorBrush,
        },
        DirectWrite::{
            DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_METRICS, IDWriteFactory, IDWriteTextFormat, IDWriteTextLayout,
        },
    },
    core::{PCWSTR, Result, w},
};
use windows_numerics::{Matrix3x2, Vector2};

use crate::{Graphics, theme::Palette};

/// A position in DIPs.
pub type Point = (f32, f32);

/// A rectangle in DIPs.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Area {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

impl Area {
    pub const fn new(left: f32, top: f32, width: f32, height: f32) -> Self {
        Self {
            left,
            top,
            width,
            height,
        }
    }

    pub fn right(&self) -> f32 {
        self.left + self.width
    }

    pub fn bottom(&self) -> f32 {
        self.top + self.height
    }

    pub fn middle(&self) -> f32 {
        self.top + self.height / 2.0
    }

    pub fn contains(&self, (x, y): Point) -> bool {
        x >= self.left && x < self.right() && y >= self.top && y < self.bottom()
    }

    /// Grows each edge outward by `by`, or inward when it's negative.
    pub fn inflate(&self, by: f32) -> Self {
        Self::new(
            self.left - by,
            self.top - by,
            self.width + 2.0 * by,
            self.height + 2.0 * by,
        )
    }

    fn rect(&self) -> D2D_RECT_F {
        D2D_RECT_F {
            left: self.left,
            top: self.top,
            right: self.right(),
            bottom: self.bottom(),
        }
    }
}

/// Draws shapes and text in DIPs on a Direct2D context that's drawing a frame.
pub struct Painter<'a> {
    context: &'a ID2D1DeviceContext,
    brush: &'a ID2D1SolidColorBrush,
    pub palette: &'a Palette,
    scale: f32,
}

impl<'a> Painter<'a> {
    /// `scale` is how many pixels make a DIP.
    pub fn new(
        context: &'a ID2D1DeviceContext,
        brush: &'a ID2D1SolidColorBrush,
        palette: &'a Palette,
        scale: f32,
    ) -> Self {
        unsafe { context.SetTransform(&Matrix3x2::scale(scale, scale)) };
        Self {
            context,
            brush,
            palette,
            scale,
        }
    }

    pub fn context(&self) -> &ID2D1DeviceContext {
        self.context
    }

    /// Rounds `value` to the nearest pixel edge.
    pub fn snap(&self, value: f32) -> f32 {
        (value * self.scale).round() / self.scale
    }

    /// One pixel, in DIPs.
    pub fn pixel(&self) -> f32 {
        1.0 / self.scale
    }

    pub fn fill(&self, area: Area, radius: f32, color: D2D1_COLOR_F) {
        if color.a <= 0.0 {
            return;
        }
        unsafe {
            self.brush.SetColor(&color);
            if radius > 0.0 {
                self.context
                    .FillRoundedRectangle(&rounded(area, radius), self.brush);
            } else {
                self.context.FillRectangle(&area.rect(), self.brush);
            }
        }
    }

    /// A line `width` DIPs thick, just inside the edges of `area`.
    pub fn stroke(&self, area: Area, radius: f32, width: f32, color: D2D1_COLOR_F) {
        if color.a <= 0.0 {
            return;
        }
        let half = width / 2.0;
        let center = area.inflate(-half);
        let radius = (radius - half).max(0.0);
        unsafe {
            self.brush.SetColor(&color);
            if radius > 0.0 {
                self.context.DrawRoundedRectangle(
                    &rounded(center, radius),
                    self.brush,
                    width,
                    None,
                );
            } else {
                self.context
                    .DrawRectangle(&center.rect(), self.brush, width, None);
            }
        }
    }

    pub fn circle(&self, center: Point, radius: f32, color: D2D1_COLOR_F) {
        if color.a <= 0.0 {
            return;
        }
        unsafe {
            self.brush.SetColor(&color);
            self.context
                .FillEllipse(&ellipse(center, radius), self.brush);
        }
    }

    /// A line `width` DIPs thick, just inside a circle.
    pub fn ring(&self, center: Point, radius: f32, width: f32, color: D2D1_COLOR_F) {
        if color.a <= 0.0 {
            return;
        }
        unsafe {
            self.brush.SetColor(&color);
            self.context.DrawEllipse(
                &ellipse(center, radius - width / 2.0),
                self.brush,
                width,
                None,
            );
        }
    }

    /// Stretches `bitmap` over `area`: pixel for pixel when they match, smoothly
    /// otherwise.
    pub fn bitmap(&self, bitmap: &ID2D1Bitmap, area: Area) {
        unsafe {
            let size = bitmap.GetPixelSize();
            let exact = (area.width * self.scale - size.width as f32).abs() < 0.01
                && (area.height * self.scale - size.height as f32).abs() < 0.01;
            let interpolation = if exact {
                D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR
            } else {
                D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC
            };
            self.context
                .DrawBitmap(bitmap, Some(&area.rect()), 1.0, interpolation, None, None);
        }
    }

    /// Draws `label` starting at `left`, centered vertically on `middle`.
    pub fn text(&self, label: &Label, left: f32, middle: f32, color: D2D1_COLOR_F) {
        let origin = Vector2::new(self.snap(left), self.snap(middle - label.height / 2.0));
        unsafe {
            self.brush.SetColor(&color);
            self.context.DrawTextLayout(
                origin,
                &label.layout,
                self.brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
            );
        }
    }
}

fn rounded(area: Area, radius: f32) -> D2D1_ROUNDED_RECT {
    D2D1_ROUNDED_RECT {
        rect: area.rect(),
        radiusX: radius,
        radiusY: radius,
    }
}

fn ellipse((x, y): Point, radius: f32) -> D2D1_ELLIPSE {
    D2D1_ELLIPSE {
        point: Vector2::new(x, y),
        radiusX: radius,
        radiusY: radius,
    }
}

/// The fonts Windows 11 apps use, at their standard sizes in DIPs.
pub struct Fonts {
    dwrite: IDWriteFactory,
    body: IDWriteTextFormat,
    icons: IDWriteTextFormat,
}

impl Fonts {
    pub fn new(graphics: &Graphics) -> Result<Self> {
        let format = |family: PCWSTR, size: f32| unsafe {
            graphics.dwrite.CreateTextFormat(
                family,
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                size,
                w!(""),
            )
        };
        Ok(Self {
            dwrite: graphics.dwrite.clone(),
            body: format(w!("Segoe UI Variable Text"), 14.0)?,
            icons: format(w!("Segoe Fluent Icons"), 10.0)?,
        })
    }

    pub fn label(&self, text: &str) -> Result<Label> {
        self.layout(text, &self.body)
    }

    /// One of Segoe Fluent Icons' glyphs.
    pub fn icon(&self, glyph: char) -> Result<Label> {
        self.layout(glyph.encode_utf8(&mut [0; 4]), &self.icons)
    }

    fn layout(&self, text: &str, format: &IDWriteTextFormat) -> Result<Label> {
        let text: Vec<u16> = text.encode_utf16().collect();
        let mut metrics = DWRITE_TEXT_METRICS::default();
        unsafe {
            let layout = self
                .dwrite
                .CreateTextLayout(&text, format, f32::MAX, f32::MAX)?;
            layout.GetMetrics(&mut metrics)?;
            Ok(Label {
                layout,
                width: metrics.width,
                height: metrics.height,
            })
        }
    }
}

/// A line of text ready to draw, and its size in DIPs.
#[derive(Clone)]
pub struct Label {
    layout: IDWriteTextLayout,
    pub width: f32,
    pub height: f32,
}
