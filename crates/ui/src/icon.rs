//! Kata's glyph — four rounded corners framing a selection — drawn at any size.

use std::{mem::size_of, ptr};

use windows::{
    Win32::{
        Graphics::{
            Direct2D::{
                Common::{
                    D2D_RECT_F, D2D_SIZE_F, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F,
                    D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_OPEN, D2D1_PIXEL_FORMAT,
                },
                D2D1_ANTIALIAS_MODE_ALIASED, D2D1_ARC_SEGMENT, D2D1_ARC_SIZE_SMALL,
                D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_RENDER_TARGET_PROPERTIES, D2D1_ROUNDED_RECT,
                D2D1_SWEEP_DIRECTION_CLOCKWISE, D2D1_SWEEP_DIRECTION_COUNTER_CLOCKWISE,
                D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE, ID2D1PathGeometry1,
            },
            DirectWrite::{
                DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER, IDWriteTextLayout,
            },
            Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
            Gdi::{
                BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection,
                DIB_RGB_COLORS, DeleteObject,
            },
            Imaging::{
                CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICImagingFactory,
                WICBitmapCacheOnLoad,
            },
        },
        System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance},
        UI::WindowsAndMessaging::{CreateIconIndirect, HICON, ICONINFO},
    },
    core::{Result, w},
};
use windows_numerics::Vector2;

use crate::Graphics;

/// The glyph as an icon of `size` × `size` pixels, white for dark surfaces or black
/// for light ones, with `tag` (such as "Dev") on a label across its foot. Needs COM
/// on the calling thread.
pub fn glyph_icon(
    graphics: &Graphics,
    size: u32,
    light_surface: bool,
    tag: Option<&str>,
) -> Result<HICON> {
    let ink = if light_surface { 0.0 } else { 1.0 };
    let gray = |value| D2D1_COLOR_F {
        r: value,
        g: value,
        b: value,
        a: 1.0,
    };
    unsafe {
        let wic: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let bitmap = wic.CreateBitmap(
            size,
            size,
            &GUID_WICPixelFormat32bppPBGRA,
            WICBitmapCacheOnLoad,
        )?;
        let target = graphics.d2d.CreateWicBitmapRenderTarget(
            &bitmap,
            &D2D1_RENDER_TARGET_PROPERTIES {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                ..Default::default()
            },
        )?;
        let brush = target.CreateSolidColorBrush(&gray(ink), None)?;
        let (glyph, stroke) = corners(graphics, size as f32)?;
        let tag = tag
            .map(|tag| Tag::new(graphics, size as f32, tag))
            .transpose()?;
        target.BeginDraw();
        target.Clear(None);
        target.DrawGeometry(&glyph, &brush, stroke, None);
        if let Some(tag) = &tag {
            // A clear gap keeps the label apart from the corners it covers.
            target.PushAxisAlignedClip(&tag.clearance, D2D1_ANTIALIAS_MODE_ALIASED);
            target.Clear(None);
            target.PopAxisAlignedClip();
            target.FillRoundedRectangle(&tag.label, &brush);
            brush.SetColor(&gray(1.0 - ink));
            target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            target.DrawTextLayout(
                Vector2::new(tag.label.rect.left, tag.label.rect.top),
                &tag.text,
                &brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
            );
        }
        target.EndDraw(None, None)?;

        let stride = size * 4;
        let mut pixels = vec![0u8; (stride * size) as usize];
        bitmap.CopyPixels(ptr::null(), stride, &mut pixels)?;
        // Icons expect straight alpha; Direct2D draws premultiplied.
        for pixel in pixels.as_chunks_mut::<4>().0 {
            let alpha = u32::from(pixel[3]);
            for channel in &mut pixel[..3] {
                if let Some(straight) = (u32::from(*channel) * 255).checked_div(alpha) {
                    *channel = straight as u8;
                }
            }
        }
        icon_from_pixels(size, &pixels)
    }
}

/// The glyph's outline on a 16-unit grid scaled to `size`, and its stroke width.
/// Straight strokes sit on pixel centers so they stay crisp at every size.
fn corners(graphics: &Graphics, size: f32) -> Result<(ID2D1PathGeometry1, f32)> {
    let unit = size / 16.0;
    let stroke = unit.round().max(1.0);
    let near = (2.0 * unit).round() + stroke / 2.0;
    let far = size - near;
    let arm = 4.5 * unit;
    let radius = 2.0 * unit;
    let point = Vector2::new;
    unsafe {
        let path = graphics.d2d.CreatePathGeometry()?;
        let sink = path.Open()?;
        for (x, y, dx, dy) in [
            (near, near, 1.0, 1.0),
            (far, near, -1.0, 1.0),
            (far, far, -1.0, -1.0),
            (near, far, 1.0, -1.0),
        ] {
            sink.BeginFigure(point(x, y + dy * arm), D2D1_FIGURE_BEGIN_HOLLOW);
            sink.AddLine(point(x, y + dy * radius));
            sink.AddArc(&D2D1_ARC_SEGMENT {
                point: point(x + dx * radius, y),
                size: D2D_SIZE_F {
                    width: radius,
                    height: radius,
                },
                rotationAngle: 0.0,
                sweepDirection: if dx * dy > 0.0 {
                    D2D1_SWEEP_DIRECTION_CLOCKWISE
                } else {
                    D2D1_SWEEP_DIRECTION_COUNTER_CLOCKWISE
                },
                arcSize: D2D1_ARC_SIZE_SMALL,
            });
            sink.AddLine(point(x + dx * arm, y));
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
        }
        sink.Close()?;
        Ok((path, stroke))
    }
}

/// A word on a rounded label as wide as the icon, across the bottom of its 16-unit
/// grid: seven units tall, so it stays legible at the smallest icon size.
struct Tag {
    label: D2D1_ROUNDED_RECT,
    /// The label and the gap above it.
    clearance: D2D_RECT_F,
    /// The word, centered on the label.
    text: IDWriteTextLayout,
}

impl Tag {
    fn new(graphics: &Graphics, size: f32, word: &str) -> Result<Self> {
        let unit = size / 16.0;
        let height = (7.0 * unit).round();
        let top = size - height;
        let word: Vec<u16> = word.encode_utf16().collect();
        unsafe {
            let format = graphics.dwrite.CreateTextFormat(
                w!("Segoe UI Variable Small"),
                None,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                0.9 * height,
                w!(""),
            )?;
            format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            Ok(Self {
                label: D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: 0.0,
                        top,
                        right: size,
                        bottom: size,
                    },
                    radiusX: 2.0 * unit,
                    radiusY: 2.0 * unit,
                },
                clearance: D2D_RECT_F {
                    left: 0.0,
                    top: top - unit.round().max(1.0),
                    right: size,
                    bottom: size,
                },
                text: graphics
                    .dwrite
                    .CreateTextLayout(&word, &format, size, height)?,
            })
        }
    }
}

/// Builds an icon from straight-alpha BGRA pixels.
unsafe fn icon_from_pixels(size: u32, pixels: &[u8]) -> Result<HICON> {
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size as i32,
            biHeight: -(size as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    unsafe {
        let mut bits = ptr::null_mut();
        let color = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0)?;
        ptr::copy_nonoverlapping(pixels.as_ptr(), bits.cast(), pixels.len());
        // Alpha icons ignore the mask, but Windows still requires one.
        let mask_rows = vec![0u8; (size.div_ceil(16) * 2 * size) as usize];
        let mask = CreateBitmap(
            size as i32,
            size as i32,
            1,
            1,
            Some(mask_rows.as_ptr().cast()),
        );
        let icon = CreateIconIndirect(&ICONINFO {
            fIcon: true.into(),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        });
        let _ = DeleteObject(color.into());
        let _ = DeleteObject(mask.into());
        icon
    }
}
