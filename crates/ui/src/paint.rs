//! Drawing in device-independent pixels (DIPs, 1/96 of an inch), lined up with
//! the screen's pixels.

use std::mem::ManuallyDrop;

use windows::{
    Win32::Graphics::{
        Direct2D::{
            Common::{D2D_RECT_F, D2D1_COLOR_F, D2D1_GRADIENT_STOP},
            D2D1_ANTIALIAS_MODE_ALIASED, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
            D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_ELLIPSE, D2D1_EXTEND_MODE_CLAMP, D2D1_GAMMA_2_2,
            D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC, D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
            D2D1_LAYER_PARAMETERS1, D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES, D2D1_ROUNDED_RECT,
            ID2D1Bitmap, ID2D1Brush, ID2D1DeviceContext, ID2D1Layer, ID2D1LinearGradientBrush,
            ID2D1RenderTarget, ID2D1SolidColorBrush,
        },
        DirectWrite::{
            DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_HIT_TEST_METRICS, DWRITE_TEXT_METRICS,
            DWRITE_TRIMMING, DWRITE_TRIMMING_GRANULARITY_CHARACTER, DWRITE_WORD_WRAPPING_NO_WRAP,
            IDWriteFactory, IDWriteTextFormat, IDWriteTextLayout,
        },
    },
    core::{BOOL, PCWSTR, Result, w},
};
use windows_numerics::{Matrix3x2, Vector2};

use crate::{
    Graphics,
    figure::Figure,
    theme::{self, Palette, color, faded},
};

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

    /// The area `progress` of the way to `to`.
    pub fn toward(self, to: Self, progress: f32) -> Self {
        let mix = |from: f32, to: f32| from + (to - from) * progress;
        Self::new(
            mix(self.left, to.left),
            mix(self.top, to.top),
            mix(self.width, to.width),
            mix(self.height, to.height),
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
    pub(crate) context: &'a ID2D1DeviceContext,
    pub(crate) brush: &'a ID2D1SolidColorBrush,
    pub palette: &'a Palette,
    scale: f32,
}

impl<'a> Painter<'a> {
    /// `scale` is how many pixels make a DIP.
    pub(crate) fn new(
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

    /// Rounds `value` to the nearest pixel edge.
    pub fn snap(&self, value: f32) -> f32 {
        (value * self.scale).round() / self.scale
    }

    /// `area` with its edges on the nearest pixel edges.
    pub fn align(&self, area: Area) -> Area {
        let (left, top) = (self.snap(area.left), self.snap(area.top));
        Area::new(
            left,
            top,
            self.snap(area.right()) - left,
            self.snap(area.bottom()) - top,
        )
    }

    /// One pixel, in DIPs.
    pub fn pixel(&self) -> f32 {
        1.0 / self.scale
    }

    /// A hairline's width: one DIP, in whole pixels.
    pub fn border(&self) -> f32 {
        self.snap(1.0).max(self.pixel())
    }

    /// Draws what `draw` paints at `opacity`, moved `offset` DIPs to the right, even
    /// inside another layer.
    pub fn layer(&self, opacity: f32, offset: f32, draw: impl FnOnce()) {
        let moved = Matrix3x2::translation(self.snap(offset), 0.0);
        self.transformed(opacity, moved, draw);
    }

    /// Draws what `draw` paints at `opacity`, moved `offset` DIPs down, even inside
    /// another layer.
    pub fn lowered(&self, opacity: f32, offset: f32, draw: impl FnOnce()) {
        let moved = Matrix3x2::translation(0.0, self.snap(offset));
        self.transformed(opacity, moved, draw);
    }

    /// Draws what `draw` paints at `opacity`, grown `scale` times around `center`,
    /// even inside another layer.
    pub fn grown(&self, opacity: f32, scale: f32, (x, y): Point, draw: impl FnOnce()) {
        let grown = Matrix3x2::scale_around(scale, scale, Vector2::new(x, y));
        self.transformed(opacity, grown, draw);
    }

    /// Draws what `draw` paints turned `degrees` clockwise around `center`.
    pub fn turned(&self, degrees: f32, (x, y): Point, draw: impl FnOnce()) {
        let turned = Matrix3x2::rotation_around(degrees, Vector2::new(x, y));
        self.transformed(1.0, turned, draw);
    }

    fn transformed(&self, opacity: f32, transform: Matrix3x2, draw: impl FnOnce()) {
        if opacity <= 0.0 {
            return;
        }
        let mut outer = Matrix3x2::default();
        unsafe {
            self.context.GetTransform(&mut outer);
            self.context.SetTransform(&(transform * outer));
        }
        if opacity < 1.0 {
            self.through(D2D1_LAYER_PARAMETERS1 { opacity, ..layer() }, draw);
        } else {
            draw();
        }
        unsafe { self.context.SetTransform(&outer) };
    }

    /// Draws what `draw` paints, only inside `area` with corners of `radius`.
    pub fn clip_rounded(&self, area: Area, radius: f32, draw: impl FnOnce()) {
        let corners = unsafe {
            self.context
                .GetFactory()
                .and_then(|factory| factory.CreateRoundedRectangleGeometry(&rounded(area, radius)))
        };
        match corners {
            Ok(corners) => self.through(
                D2D1_LAYER_PARAMETERS1 {
                    contentBounds: area.rect(),
                    geometricMask: ManuallyDrop::new(Some(corners.into())),
                    ..layer()
                },
                draw,
            ),
            Err(_) => draw(),
        }
    }

    /// Draws `labels`, each from its left edge and centered on `middle`, only where
    /// a glint `width` DIPs wide, centered on `x`, crosses them.
    pub fn glint(&self, labels: &[(&Label, f32)], middle: f32, x: f32, width: f32) {
        let Some(light) = self.band(x, middle, width, 1.0) else {
            return;
        };
        for &(label, left) in labels {
            unsafe {
                self.context.DrawTextLayout(
                    self.origin(label, left, middle),
                    &label.layout,
                    &light,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                );
            }
        }
    }

    /// Lights `area`, with corners of `radius`, where a glint `width` DIPs wide,
    /// centered on `x`, crosses it at `opacity`.
    pub fn glint_across(&self, area: Area, radius: f32, x: f32, width: f32, opacity: f32) {
        if let Some(light) = self.band(x, area.middle(), width, opacity) {
            unsafe {
                self.context
                    .FillRoundedRectangle(&rounded(area, radius), &light);
            }
        }
    }

    /// A band of colored light `width` DIPs wide, centered on `x` and `middle`,
    /// leaning as a reflection does, in Kata's own colors at `opacity`.
    fn band(
        &self,
        x: f32,
        middle: f32,
        width: f32,
        opacity: f32,
    ) -> Option<ID2D1LinearGradientBrush> {
        /// How far the band leans from upright, in radians.
        const LEAN: f32 = 0.35;
        let (across, down) = (LEAN.cos() * width / 2.0, LEAN.sin() * width / 2.0);
        let stop = |position, rgb, alpha: f32| D2D1_GRADIENT_STOP {
            position,
            color: color(rgb, alpha * opacity),
        };
        // From its trailing edge to its leading one, fading in and out at both.
        let [blue, violet, pink, orange] = theme::signature(self.palette.light);
        let stops = [
            stop(0.0, blue, 0.0),
            stop(0.2, blue, 1.0),
            stop(0.4, violet, 1.0),
            stop(0.6, pink, 1.0),
            stop(0.8, orange, 1.0),
            stop(1.0, orange, 0.0),
        ];
        let from = Vector2::new(x - across, middle - down);
        self.gradient(&stops, from, Vector2::new(x + across, middle + down))
            .ok()
    }

    /// Light falling from the top-left corner on a surface raised over `area`, with
    /// corners of `radius`, at `opacity`.
    pub fn sheen(&self, area: Area, radius: f32, opacity: f32) {
        /// How bright it is at the corner, and how far across it reaches.
        const LIGHT: f32 = 0.28;
        const REACH: f32 = 0.38;
        let white = |position, alpha| D2D1_GRADIENT_STOP {
            position,
            color: color(0xFFFFFF, alpha),
        };
        let stops = [white(0.0, LIGHT * opacity), white(REACH, 0.0)];
        let (from, to) = diagonal(area);
        if let Ok(light) = self.gradient(&stops, from, to) {
            unsafe {
                self.context
                    .FillRoundedRectangle(&rounded(area, radius), &light);
            }
        }
    }

    /// A line `width` DIPs thick, just inside the edges of `area`, in Kata's own
    /// colors from its top-left corner to its bottom-right, at `opacity`.
    pub fn rim(&self, area: Area, radius: f32, width: f32, opacity: f32) {
        let [blue, violet, pink, orange] = theme::signature(self.palette.light);
        let stop = |position, rgb| D2D1_GRADIENT_STOP {
            position,
            color: color(rgb, opacity),
        };
        let stops = [
            stop(0.0, blue),
            stop(0.35, violet),
            stop(0.7, pink),
            stop(1.0, orange),
        ];
        let (from, to) = diagonal(area);
        let half = width / 2.0;
        let center = area.inflate(-half);
        if let Ok(colors) = self.gradient(&stops, from, to) {
            unsafe {
                self.context.DrawRoundedRectangle(
                    &rounded(center, (radius - half).max(0.0)),
                    &colors,
                    width,
                    None,
                );
            }
        }
    }

    /// Fills `area`, rounded by `radius`, with `colors` evenly from its left edge to
    /// its right.
    pub fn spectrum(&self, area: Area, radius: f32, colors: &[D2D1_COLOR_F]) {
        let last = colors.len().saturating_sub(1).max(1) as f32;
        let stops: Vec<_> = (colors.iter().enumerate())
            .map(|(index, &color)| D2D1_GRADIENT_STOP {
                position: index as f32 / last,
                color,
            })
            .collect();
        let (from, to) = (
            Vector2::new(area.left, area.middle()),
            Vector2::new(area.right(), area.middle()),
        );
        if let Ok(brush) = self.gradient(&stops, from, to) {
            unsafe {
                self.context
                    .FillRoundedRectangle(&rounded(area, radius), &brush);
            }
        }
    }

    /// Paints `stops` evenly from `from` to `to`, and the ends' colors beyond.
    pub(crate) fn gradient(
        &self,
        stops: &[D2D1_GRADIENT_STOP],
        from: Vector2,
        to: Vector2,
    ) -> Result<ID2D1LinearGradientBrush> {
        unsafe {
            let stops = ID2D1RenderTarget::CreateGradientStopCollection(
                self.context,
                stops,
                D2D1_GAMMA_2_2,
                D2D1_EXTEND_MODE_CLAMP,
            )?;
            self.context.CreateLinearGradientBrush(
                &D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
                    startPoint: from,
                    endPoint: to,
                },
                None,
                &stops,
            )
        }
    }

    /// Draws what `draw` paints through a layer that `parameters` describe.
    fn through(&self, mut parameters: D2D1_LAYER_PARAMETERS1, draw: impl FnOnce()) {
        unsafe {
            self.context.PushLayer(&parameters, None::<&ID2D1Layer>);
            draw();
            self.context.PopLayer();
            ManuallyDrop::drop(&mut parameters.geometricMask);
            ManuallyDrop::drop(&mut parameters.opacityBrush);
        }
    }

    /// Draws what `draw` paints, only inside `area`.
    pub fn clip(&self, area: Area, draw: impl FnOnce()) {
        unsafe {
            self.context
                .PushAxisAlignedClip(&area.rect(), D2D1_ANTIALIAS_MODE_ALIASED);
            draw();
            self.context.PopAxisAlignedClip();
        }
    }

    /// The soft shadow that a surface raised over `area`, with corners of `radius`,
    /// casts below it: Fluent's, 4 DIPs down and blurred over 8.
    pub fn shadow(&self, area: Area, radius: f32, color: D2D1_COLOR_F) {
        self.cast(area, radius, 8.0, 4.0, color);
    }

    /// The shadow that a surface raised over `area`, with corners of `radius`, casts
    /// `down` DIPs below it, blurred over `blur`.
    pub fn cast(&self, area: Area, radius: f32, blur: f32, down: f32, color: D2D1_COLOR_F) {
        // Each step a little wider and fainter, so together they fade out: about one
        // a DIP, so none stands out.
        let steps = blur.round().clamp(8.0, 48.0);
        let step = faded(color, 1.0 / steps);
        let below = Area {
            top: area.top + down,
            ..area
        };
        for n in 0..steps as u8 {
            let by = blur * ((f32::from(n) + 0.5) / steps - 0.5);
            self.fill(below.inflate(by), (radius + by).max(0.0), step);
        }
    }

    /// A surface raised over the window, such as a tooltip: its shadow, its fill and
    /// a hairline around it.
    pub fn flyout(&self, area: Area, radius: f32) {
        let area = self.align(area);
        self.shadow(area, radius, self.palette.shadow);
        self.fill(area, radius, self.palette.flyout);
        self.stroke(area, radius, self.border(), self.palette.flyout_stroke);
    }

    /// Fills `figure`, drawn with its origin at `origin` on a pixel edge, so the
    /// same figure looks the same anywhere.
    pub fn figure(&self, figure: &Figure, (x, y): Point, color: D2D1_COLOR_F) {
        if color.a <= 0.0 {
            return;
        }
        let mut outer = Matrix3x2::default();
        unsafe {
            self.context.GetTransform(&mut outer);
            let at = Matrix3x2::translation(self.snap(x), self.snap(y));
            self.context.SetTransform(&(at * outer));
            self.brush.SetColor(&color);
            self.context
                .FillGeometry(&figure.geometry, self.brush, None::<&ID2D1Brush>);
            self.context.SetTransform(&outer);
        }
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

    /// Fluent's keyboard focus visual around `area`, whose corners have `radius`: a
    /// ring in the text color, with a thin one inside that sets it apart from
    /// whatever is behind.
    pub fn focus(&self, area: Area, radius: f32) {
        const MARGIN: f32 = 3.0;
        let outer = self.align(area.inflate(MARGIN));
        let width = self.snap(2.0).max(self.pixel());
        self.stroke(outer, radius + MARGIN, width, self.palette.text);
        let inner = outer.inflate(-width);
        let color = self.palette.focus_inner;
        self.stroke(inner, radius + MARGIN - width, self.border(), color);
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

    /// Stretches the `part` of `bitmap`, in its pixels, over `area`: pixel for pixel
    /// when they match, upright, smoothly otherwise.
    pub fn bitmap(&self, bitmap: &ID2D1Bitmap, part: Area, area: Area) {
        self.faded_bitmap(bitmap, part, area, 1.0);
    }

    /// The same, showing `opacity` as much.
    pub fn faded_bitmap(&self, bitmap: &ID2D1Bitmap, part: Area, area: Area, opacity: f32) {
        if opacity <= 0.0 {
            return;
        }
        let mut on_screen = Matrix3x2::default();
        unsafe {
            self.context.GetTransform(&mut on_screen);
            let exact = on_screen.M12 == 0.0
                && on_screen.M21 == 0.0
                && (area.width * on_screen.M11 - part.width).abs() < 0.01
                && (area.height * on_screen.M22 - part.height).abs() < 0.01;
            let interpolation = if exact {
                D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR
            } else {
                D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC
            };
            self.context.DrawBitmap(
                bitmap,
                Some(&area.rect()),
                opacity.min(1.0),
                interpolation,
                Some(&part.rect()),
                None,
            );
        }
    }

    /// Draws `label` starting at `left`, centered vertically on `middle`.
    pub fn text(&self, label: &Label, left: f32, middle: f32, color: D2D1_COLOR_F) {
        unsafe {
            self.brush.SetColor(&color);
            self.context.DrawTextLayout(
                self.origin(label, left, middle),
                &label.layout,
                self.brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
            );
        }
    }

    /// Where `label` starts, from its left edge and centered on `middle`, on whole
    /// pixels.
    fn origin(&self, label: &Label, left: f32, middle: f32) -> Vector2 {
        Vector2::new(self.snap(left), self.snap(middle - label.height / 2.0))
    }
}

/// A layer over everything, at full opacity.
fn layer() -> D2D1_LAYER_PARAMETERS1 {
    D2D1_LAYER_PARAMETERS1 {
        contentBounds: D2D_RECT_F {
            left: f32::MIN,
            top: f32::MIN,
            right: f32::MAX,
            bottom: f32::MAX,
        },
        maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
        maskTransform: Matrix3x2::identity(),
        opacity: 1.0,
        ..Default::default()
    }
}

pub(crate) fn rounded(area: Area, radius: f32) -> D2D1_ROUNDED_RECT {
    D2D1_ROUNDED_RECT {
        rect: area.rect(),
        radiusX: radius,
        radiusY: radius,
    }
}

/// From `area`'s top-left corner to its bottom-right.
pub(crate) fn diagonal(area: Area) -> (Vector2, Vector2) {
    (
        Vector2::new(area.left, area.top),
        Vector2::new(area.right(), area.bottom()),
    )
}

fn ellipse((x, y): Point, radius: f32) -> D2D1_ELLIPSE {
    D2D1_ELLIPSE {
        point: Vector2::new(x, y),
        radiusX: radius,
        radiusY: radius,
    }
}

/// The fonts Windows 11 apps use, at their standard sizes in DIPs.
#[derive(Clone)]
pub struct Fonts {
    dwrite: IDWriteFactory,
    body: IDWriteTextFormat,
    caption: IDWriteTextFormat,
}

impl Fonts {
    pub fn new(graphics: &Graphics) -> Result<Self> {
        let dwrite = graphics.dwrite.clone();
        Ok(Self {
            body: format(&dwrite, w!("Segoe UI Variable Text"), 14.0)?,
            caption: format(&dwrite, w!("Segoe UI Variable Small"), 12.0)?,
            dwrite,
        })
    }

    /// Text in controls and settings.
    pub fn label(&self, text: &str) -> Result<Label> {
        self.layout(text, &self.body, f32::MAX)
    }

    /// A page's title, as large and bold as Windows' own.
    pub fn title(&self, text: &str) -> Result<Label> {
        let title = unsafe {
            self.dwrite.CreateTextFormat(
                w!("Segoe UI Variable Display"),
                None,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                28.0,
                w!(""),
            )?
        };
        self.layout(text, &title, f32::MAX)
    }

    /// Small text, as in title bars and hints.
    pub fn caption(&self, text: &str) -> Result<Label> {
        self.layout(text, &self.caption, f32::MAX)
    }

    /// Small text in lines at most `width` DIPs wide, as in tooltips.
    pub fn wrapped(&self, text: &str, width: f32) -> Result<Label> {
        self.layout(text, &self.caption, width)
    }

    /// A line of text in controls, at most `width` DIPs wide: what doesn't fit gives
    /// way to an ellipsis.
    pub fn line(&self, text: &str, width: f32) -> Result<Label> {
        self.trimmed(text, &self.body, width, None)
    }

    /// A folder's path in small text, at most `width` DIPs wide: what doesn't fit
    /// gives way to an ellipsis before the last folder.
    pub fn path(&self, path: &str, width: f32) -> Result<Label> {
        self.trimmed(path, &self.caption, width, Some('\\'))
    }

    /// One line at most `width` DIPs wide, ending in an ellipsis where it doesn't
    /// fit, or putting it before what follows the last `delimiter`.
    fn trimmed(
        &self,
        text: &str,
        format: &IDWriteTextFormat,
        width: f32,
        delimiter: Option<char>,
    ) -> Result<Label> {
        let label = self.layout(text, format, width.max(0.0))?;
        let trimming = DWRITE_TRIMMING {
            granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
            delimiter: delimiter.map_or(0, u32::from),
            delimiterCount: u32::from(delimiter.is_some()),
        };
        unsafe {
            let layout = &label.layout;
            layout.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            let ellipsis = self.dwrite.CreateEllipsisTrimmingSign(layout)?;
            layout.SetTrimming(&trimming, &ellipsis)?;
        }
        Label::new(label.layout, label.text)
    }

    /// One of Segoe Fluent Icons' glyphs, `size` DIPs tall.
    pub fn icon(&self, glyph: char, size: f32) -> Result<Label> {
        let icons = format(&self.dwrite, w!("Segoe Fluent Icons"), size)?;
        self.layout(glyph.encode_utf8(&mut [0; 4]), &icons, f32::MAX)
    }

    fn layout(&self, text: &str, format: &IDWriteTextFormat, width: f32) -> Result<Label> {
        let wide: Vec<u16> = text.encode_utf16().collect();
        let layout = unsafe {
            self.dwrite
                .CreateTextLayout(&wide, format, width, f32::MAX)?
        };
        Label::new(layout, text.to_owned())
    }
}

fn format(dwrite: &IDWriteFactory, family: PCWSTR, size: f32) -> Result<IDWriteTextFormat> {
    unsafe {
        dwrite.CreateTextFormat(
            family,
            None,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            size,
            w!(""),
        )
    }
}

/// A line of text ready to draw, and its size in DIPs.
#[derive(Clone)]
pub struct Label {
    layout: IDWriteTextLayout,
    /// What it says, for assistive technologies such as Narrator.
    pub text: String,
    pub width: f32,
    pub height: f32,
}

impl Label {
    fn new(layout: IDWriteTextLayout, text: String) -> Result<Self> {
        let mut metrics = DWRITE_TEXT_METRICS::default();
        unsafe { layout.GetMetrics(&mut metrics)? };
        Ok(Self {
            layout,
            text,
            width: metrics.width,
            height: metrics.height,
        })
    }

    /// How far from its left edge the text is before the UTF-16 unit `index`, or
    /// after all of it, trailing spaces included.
    pub fn offset(&self, index: usize) -> f32 {
        let (mut x, mut y) = (0.0, 0.0);
        let mut metrics = DWRITE_HIT_TEST_METRICS::default();
        let index = u32::try_from(index).unwrap_or(u32::MAX);
        let hit = unsafe {
            self.layout
                .HitTestTextPosition(index, false, &mut x, &mut y, &mut metrics)
        };
        if hit.is_ok() { x } else { self.width }
    }

    /// The UTF-16 unit nearest `x` DIPs from the text's left edge, where a caret
    /// clicked there goes.
    pub fn position(&self, x: f32) -> usize {
        let (mut trailing, mut inside) = (BOOL(0), BOOL(0));
        let mut metrics = DWRITE_HIT_TEST_METRICS::default();
        let y = self.height / 2.0;
        let hit = unsafe {
            self.layout
                .HitTestPoint(x, y, &mut trailing, &mut inside, &mut metrics)
        };
        if hit.is_err() {
            return 0;
        }
        let after = if trailing.as_bool() {
            metrics.length
        } else {
            0
        };
        (metrics.textPosition + after) as usize
    }
}
