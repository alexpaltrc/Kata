//! The selection overlay. It shows the frozen screen and lets the user choose what
//! to capture: hovering highlights the window or monitor under the pointer, a click
//! takes it, and dragging selects any area.

use std::{cell::RefCell, mem::size_of, ptr, time::Instant};

use kata_capture::Screenshot;
use kata_editor::{Image, Rect};
use kata_ui::{
    Graphics,
    motion::{self, Animation, Easing},
    window,
};
use windows::{
    Win32::{
        Foundation::{D2DERR_RECREATE_TARGET, HWND, LPARAM, LRESULT, POINT, WPARAM},
        Graphics::{
            Direct2D::{
                Common::{
                    D2D_RECT_F, D2D_SIZE_U, D2D1_ALPHA_MODE_IGNORE, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
                },
                D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR, D2D1_BITMAP_PROPERTIES,
                D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_HWND_RENDER_TARGET_PROPERTIES,
                D2D1_PRESENT_OPTIONS_NONE, D2D1_RENDER_TARGET_PROPERTIES, D2D1_ROUNDED_RECT,
                ID2D1Bitmap, ID2D1HwndRenderTarget, ID2D1SolidColorBrush,
            },
            DirectWrite::{
                DWRITE_FONT_FEATURE, DWRITE_FONT_FEATURE_TAG_TABULAR_FIGURES,
                DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_TEXT_METRICS, DWRITE_TEXT_RANGE, IDWriteFactory, IDWriteTextFormat,
                IDWriteTextLayout, IDWriteTypography,
            },
            Dwm::{
                DWMWA_CLOAK, DWMWA_TRANSITIONS_FORCEDISABLED, DWMWINDOWATTRIBUTE,
                DwmSetWindowAttribute,
            },
            Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
            Gdi::{InvalidateRect, UpdateWindow, ValidateRect},
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, VK_ESCAPE, VK_RETURN},
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetCursorPos,
                GetMessageW, GetSystemMetrics, IDC_CROSS, LoadCursorW, MSG, PostQuitMessage,
                RegisterClassExW, SM_CXDRAG, SM_CYDRAG, SW_SHOW, SetForegroundWindow, ShowWindow,
                TranslateMessage, WA_INACTIVE, WM_ACTIVATE, WM_CLOSE, WM_DISPLAYCHANGE,
                WM_DPICHANGED, WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP,
                WM_MOUSEMOVE, WM_PAINT, WM_RBUTTONUP, WNDCLASSEXW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
                WS_POPUP,
            },
        },
    },
    core::{BOOL, PCWSTR, Result, w},
};
use windows_numerics::Vector2;

use crate::report;

/// How dark the screen gets outside the selection.
const DIM: f32 = 0.45;
/// The size label's text size at 100 % scale.
const LABEL_SIZE: f32 = 12.0;
const CLASS: PCWSTR = w!("Kata.Overlay");

/// Lets the user choose an area of `shot`, in image coordinates. `None` when they
/// cancel.
pub fn select(graphics: &Graphics, shot: &Screenshot) -> Option<Rect> {
    let mut pointer = POINT::default();
    let _ = unsafe { GetCursorPos(&mut pointer) };
    let pointer = (pointer.x - shot.origin.0, pointer.y - shot.origin.1);
    let overlay = RefCell::new(Overlay::new(graphics, shot, pointer));
    let hwnd = match unsafe { create(&overlay) } {
        Ok(hwnd) => hwnd,
        Err(error) => {
            report("overlay", &error);
            return None;
        }
    };
    unsafe { show(hwnd) };
    overlay.borrow_mut().appear(hwnd);
    let chosen = run(&overlay);
    let _ = unsafe { DestroyWindow(hwnd) };
    chosen
}

unsafe fn create(overlay: &RefCell<Overlay<'_>>) -> Result<HWND> {
    let shot = overlay.borrow().shot;
    unsafe {
        let instance = GetModuleHandleW(None)?.into();
        // Later captures register the class again, which fails harmlessly.
        RegisterClassExW(&WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            hCursor: LoadCursorW(None, IDC_CROSS)?,
            lpszClassName: CLASS,
            ..Default::default()
        });
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            CLASS,
            w!("Kata"),
            WS_POPUP,
            shot.origin.0,
            shot.origin.1,
            shot.image.bounds().width(),
            shot.image.bounds().height(),
            None,
            None,
            Some(instance),
            Some(ptr::from_ref(overlay).cast()),
        )
    }
}

/// Shows the overlay with its first frame already drawn, so the switch from the live
/// screen to the frozen one can't be seen.
unsafe fn show(hwnd: HWND) {
    unsafe {
        set_flag(hwnd, DWMWA_TRANSITIONS_FORCEDISABLED, true);
        set_flag(hwnd, DWMWA_CLOAK, true);
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);
        set_flag(hwnd, DWMWA_CLOAK, false);
        let _ = SetForegroundWindow(hwnd);
    }
}

unsafe fn set_flag(hwnd: HWND, attribute: DWMWINDOWATTRIBUTE, on: bool) {
    let value = BOOL::from(on);
    let size = size_of::<BOOL>() as u32;
    let _ = unsafe { DwmSetWindowAttribute(hwnd, attribute, (&raw const value).cast(), size) };
}

/// Runs the overlay's message loop until the user has chosen.
fn run(overlay: &RefCell<Overlay<'_>>) -> Option<Rect> {
    let mut message = MSG::default();
    loop {
        if let Phase::Done(chosen) = overlay.borrow().phase {
            return chosen;
        }
        match unsafe { GetMessageW(&mut message, None, 0, 0) }.0 {
            // Kata is quitting: hand the request back to the main loop.
            0 => {
                unsafe { PostQuitMessage(message.wParam.0 as i32) };
                return None;
            }
            -1 => return None,
            _ => unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            },
        }
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let overlay = unsafe { window::linked_state::<Overlay<'_>>(hwnd, message, lparam) };
    overlay
        .and_then(|overlay| overlay.try_borrow_mut().ok())
        .and_then(|mut overlay| overlay.handle(hwnd, message, wparam, lparam))
        .unwrap_or_else(|| unsafe { DefWindowProcW(hwnd, message, wparam, lparam) })
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Choosing,
    /// Fading out after the user cancelled.
    Leaving,
    Done(Option<Rect>),
}

struct Overlay<'a> {
    graphics: &'a Graphics,
    shot: &'a Screenshot,
    canvas: Option<Canvas>,
    phase: Phase,
    /// 0 shows the plain screen; 1, the dimmed screen with the selection lit.
    presence: Animation,
    /// The selection glides from `from` to `to` as `glide` goes from 0 to 1.
    from: Rect,
    to: Rect,
    glide: Animation,
    /// Where the left button went down, while it's held.
    press: Option<(i32, i32)>,
    dragging: bool,
    /// Whether the overlay has been the active window: losing that cancels.
    active: bool,
}

impl<'a> Overlay<'a> {
    fn new(graphics: &'a Graphics, shot: &'a Screenshot, pointer: (i32, i32)) -> Self {
        let target = target_at(shot, pointer);
        Self {
            graphics,
            shot,
            canvas: None,
            phase: Phase::Choosing,
            presence: Animation::idle(0.0),
            from: target,
            to: target,
            glide: Animation::idle(1.0),
            press: None,
            dragging: false,
            active: false,
        }
    }

    fn appear(&mut self, hwnd: HWND) {
        self.presence = Animation::new(0.0, 1.0, motion::FAST, Easing::DECELERATE);
        invalidate(hwnd);
    }

    fn handle(
        &mut self,
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<LRESULT> {
        match message {
            WM_PAINT => self.paint(hwnd),
            // Every frame covers the whole window: erasing first would only flicker.
            WM_ERASEBKGND => return Some(LRESULT(1)),
            WM_MOUSEMOVE => self.pointer_moved(hwnd, window::mouse_position(lparam)),
            WM_LBUTTONDOWN => {
                self.press = Some(window::mouse_position(lparam));
                unsafe { SetCapture(hwnd) };
            }
            WM_LBUTTONUP => {
                let _ = unsafe { ReleaseCapture() };
                if self.press.take().is_some() {
                    self.choose();
                }
            }
            WM_KEYDOWN if wparam.0 == usize::from(VK_RETURN.0) => self.choose(),
            WM_KEYDOWN if wparam.0 == usize::from(VK_ESCAPE.0) => self.leave(hwnd),
            WM_RBUTTONUP | WM_CLOSE => self.leave(hwnd),
            WM_ACTIVATE if wparam.0 & 0xffff == WA_INACTIVE as usize => {
                if self.active {
                    self.cancel();
                }
            }
            WM_ACTIVATE => self.active = true,
            // The screen changed under the overlay: what it shows is no longer true.
            WM_DISPLAYCHANGE => self.cancel(),
            // The overlay must keep covering every monitor exactly, whatever their scale.
            WM_DPICHANGED => {}
            _ => return None,
        }
        Some(LRESULT(0))
    }

    fn pointer_moved(&mut self, hwnd: HWND, point: (i32, i32)) {
        if self.phase != Phase::Choosing {
            return;
        }
        if let Some(start) = self.press {
            if !self.dragging {
                let threshold =
                    unsafe { (GetSystemMetrics(SM_CXDRAG), GetSystemMetrics(SM_CYDRAG)) };
                self.dragging = (point.0 - start.0).abs() > threshold.0
                    || (point.1 - start.1).abs() > threshold.1;
            }
            if self.dragging {
                // Both corners are pixels the user pointed at, so both are included.
                let area = Rect::from_corners(start, point);
                let area = Rect::new(area.left, area.top, area.right + 1, area.bottom + 1);
                if let Some(area) = area.intersect(&self.shot.image.bounds()) {
                    self.from = area;
                    self.to = area;
                    self.glide = Animation::idle(1.0);
                    invalidate(hwnd);
                }
                return;
            }
        }
        let target = target_at(self.shot, point);
        if target != self.to {
            self.from = self.from.lerp(&self.to, self.glide.value(Instant::now()));
            self.to = target;
            self.glide = Animation::new(0.0, 1.0, motion::FAST, Easing::POINT_TO_POINT);
            invalidate(hwnd);
        }
    }

    fn choose(&mut self) {
        if self.phase == Phase::Choosing {
            self.phase = Phase::Done(Some(self.to));
        }
    }

    /// Fades out, then cancels.
    fn leave(&mut self, hwnd: HWND) {
        if self.phase == Phase::Choosing {
            self.phase = Phase::Leaving;
            let shown = self.presence.value(Instant::now());
            self.presence = Animation::new(shown, 0.0, motion::FAST, Easing::ACCELERATE);
            invalidate(hwnd);
        }
    }

    fn cancel(&mut self) {
        if !matches!(self.phase, Phase::Done(_)) {
            self.phase = Phase::Done(None);
        }
    }

    fn paint(&mut self, hwnd: HWND) {
        let _ = unsafe { ValidateRect(Some(hwnd), None) };
        let now = Instant::now();
        let frame = self.frame(now);
        let drawn = match self.canvas.take() {
            Some(canvas) => Ok(canvas),
            None => Canvas::new(self.graphics, hwnd, &self.shot.image),
        }
        .and_then(|mut canvas| {
            canvas.draw(&self.graphics.dwrite, &frame)?;
            Ok(canvas)
        });
        match drawn {
            Ok(canvas) => self.canvas = Some(canvas),
            // The graphics device went away (a driver update, say): start over.
            Err(error) if error.code() == D2DERR_RECREATE_TARGET => invalidate(hwnd),
            Err(error) => {
                report("overlay", &error);
                self.cancel();
            }
        }
        if self.phase == Phase::Leaving && !self.presence.is_running(now) {
            self.phase = Phase::Done(None);
        } else if self.presence.is_running(now) || self.glide.is_running(now) {
            invalidate(hwnd);
        }
    }

    fn frame(&self, now: Instant) -> Frame {
        let selection = self.from.lerp(&self.to, self.glide.value(now));
        let touched = || {
            self.shot
                .monitors
                .iter()
                .filter(move |monitor| monitor.rect.intersect(&selection).is_some())
        };
        Frame {
            presence: self.presence.value(now),
            selection,
            size: (self.to.width(), self.to.height()),
            area: touched()
                .map(|monitor| monitor.rect)
                .reduce(|a, b| a.union(&b))
                .unwrap_or_else(|| self.shot.image.bounds()),
            scale: touched()
                .map(|monitor| monitor.scale)
                .reduce(f32::max)
                .unwrap_or(1.0),
        }
    }
}

/// What a click at `(x, y)` captures: the window there, else its monitor.
fn target_at(shot: &Screenshot, (x, y): (i32, i32)) -> Rect {
    shot.windows
        .iter()
        .chain(shot.monitors.iter().map(|monitor| &monitor.rect))
        .find(|rect| rect.contains(x, y))
        .copied()
        .unwrap_or_else(|| shot.image.bounds())
}

fn invalidate(hwnd: HWND) {
    let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
}

/// Everything one frame shows, in image pixels.
struct Frame {
    /// 0 shows the plain screen; 1, the full overlay.
    presence: f32,
    selection: Rect,
    /// The size the label reads: where the selection is heading.
    size: (i32, i32),
    /// The monitors the selection touches. Its ring and label stay inside them.
    area: Rect,
    scale: f32,
}

/// The Direct2D resources the overlay draws with.
struct Canvas {
    target: ID2D1HwndRenderTarget,
    bounds: Rect,
    screen: ID2D1Bitmap,
    brush: ID2D1SolidColorBrush,
    typography: IDWriteTypography,
    /// The label's text format and the display scale it was made for.
    label_format: Option<(f32, IDWriteTextFormat)>,
}

impl Canvas {
    fn new(graphics: &Graphics, hwnd: HWND, image: &Image) -> Result<Self> {
        let size = D2D_SIZE_U {
            width: image.width(),
            height: image.height(),
        };
        let pixel_format = D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_IGNORE,
        };
        unsafe {
            // At 96 DPI, Direct2D's units are physical pixels, like the capture's.
            let target = graphics.d2d.CreateHwndRenderTarget(
                &D2D1_RENDER_TARGET_PROPERTIES {
                    pixelFormat: pixel_format,
                    dpiX: 96.0,
                    dpiY: 96.0,
                    ..Default::default()
                },
                &D2D1_HWND_RENDER_TARGET_PROPERTIES {
                    hwnd,
                    pixelSize: size,
                    presentOptions: D2D1_PRESENT_OPTIONS_NONE,
                },
            )?;
            let screen = target.CreateBitmap(
                size,
                Some(image.pixels().as_ptr().cast()),
                image.stride() as u32,
                &D2D1_BITMAP_PROPERTIES {
                    pixelFormat: pixel_format,
                    dpiX: 96.0,
                    dpiY: 96.0,
                },
            )?;
            let brush = target.CreateSolidColorBrush(&rgba(0.0, 0.0, 0.0, 0.0), None)?;
            // Tabular figures keep the label from jittering as its numbers change.
            let typography = graphics.dwrite.CreateTypography()?;
            typography.AddFontFeature(DWRITE_FONT_FEATURE {
                nameTag: DWRITE_FONT_FEATURE_TAG_TABULAR_FIGURES,
                parameter: 1,
            })?;
            Ok(Self {
                target,
                bounds: image.bounds(),
                screen,
                brush,
                typography,
                label_format: None,
            })
        }
    }

    fn draw(&mut self, dwrite: &IDWriteFactory, frame: &Frame) -> Result<()> {
        let presence = frame.presence;
        let selection = frame.selection;
        let ring = frame.scale.round().max(1.0) as i32;
        // The ring hugs the selection from outside, and slides inside at screen edges.
        let clamp = |rect: Rect| rect.intersect(&frame.area).unwrap_or(rect);
        let white = clamp(selection.inflate(ring));
        let shadow = clamp(white.inflate(ring));
        let label = if presence > 0.0 {
            Some(self.label(dwrite, frame, ring as f32)?)
        } else {
            None
        };
        let (target, brush) = (&self.target, &self.brush);
        unsafe {
            target.BeginDraw();
            target.DrawBitmap(
                &self.screen,
                None,
                1.0,
                D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
                None,
            );
            if let Some(label) = label {
                brush.SetColor(&rgba(0.0, 0.0, 0.0, DIM * presence));
                for band in around(self.bounds, selection) {
                    target.FillRectangle(&rect_f(band), brush);
                }
                outline(
                    target,
                    brush,
                    shadow,
                    ring as f32,
                    rgba(0.0, 0.0, 0.0, 0.3 * presence),
                );
                outline(
                    target,
                    brush,
                    white,
                    ring as f32,
                    rgba(1.0, 1.0, 1.0, 0.9 * presence),
                );
                label.draw(target, brush, presence);
            }
            target.EndDraw(None, None)
        }
    }

    /// The selection's size, in a pill below it, else above it, else inside.
    fn label(&mut self, dwrite: &IDWriteFactory, frame: &Frame, ring: f32) -> Result<Label> {
        let scale = frame.scale;
        let format = match &self.label_format {
            Some((made_for, format)) if *made_for == scale => format.clone(),
            _ => {
                let format = unsafe {
                    dwrite.CreateTextFormat(
                        w!("Segoe UI Variable Text"),
                        None,
                        DWRITE_FONT_WEIGHT_NORMAL,
                        DWRITE_FONT_STYLE_NORMAL,
                        DWRITE_FONT_STRETCH_NORMAL,
                        LABEL_SIZE * scale,
                        w!(""),
                    )?
                };
                self.label_format = Some((scale, format.clone()));
                format
            }
        };
        let text: Vec<u16> = format!("{} × {}", frame.size.0, frame.size.1)
            .encode_utf16()
            .collect();
        let mut metrics = DWRITE_TEXT_METRICS::default();
        let layout = unsafe {
            let layout = dwrite.CreateTextLayout(&text, &format, f32::MAX, f32::MAX)?;
            let all = DWRITE_TEXT_RANGE {
                startPosition: 0,
                length: text.len() as u32,
            };
            layout.SetTypography(&self.typography, all)?;
            layout.GetMetrics(&mut metrics)?;
            layout
        };

        let (padding_x, padding_y, margin) = (8.0 * scale, 4.0 * scale, 8.0 * scale);
        let gap = margin + ring;
        let width = (metrics.width + 2.0 * padding_x).round();
        let height = (metrics.height + 2.0 * padding_y).round();
        let (selection, area) = (frame.selection, frame.area);
        let center = (selection.left + selection.right) as f32 / 2.0;
        let x = (center - width / 2.0)
            .min(area.right as f32 - margin - width)
            .max(area.left as f32 + margin)
            .round();
        let below = selection.bottom as f32 + gap;
        let above = selection.top as f32 - gap - height;
        let y = if below + height <= area.bottom as f32 - margin {
            below
        } else if above >= area.top as f32 + margin {
            above
        } else {
            selection.bottom as f32 - gap - height
        }
        .round();
        Ok(Label {
            layout,
            pill: D2D_RECT_F {
                left: x,
                top: y,
                right: x + width,
                bottom: y + height,
            },
            text: Vector2::new((x + padding_x).round(), (y + padding_y).round()),
            radius: 4.0 * scale,
        })
    }
}

struct Label {
    layout: IDWriteTextLayout,
    pill: D2D_RECT_F,
    text: Vector2,
    radius: f32,
}

impl Label {
    unsafe fn draw(
        &self,
        target: &ID2D1HwndRenderTarget,
        brush: &ID2D1SolidColorBrush,
        presence: f32,
    ) {
        let rounded = |rect| D2D1_ROUNDED_RECT {
            rect,
            radiusX: self.radius,
            radiusY: self.radius,
        };
        let edge = D2D_RECT_F {
            left: self.pill.left + 0.5,
            top: self.pill.top + 0.5,
            right: self.pill.right - 0.5,
            bottom: self.pill.bottom - 0.5,
        };
        unsafe {
            brush.SetColor(&rgba(0.11, 0.11, 0.11, 0.9 * presence));
            target.FillRoundedRectangle(&rounded(self.pill), brush);
            brush.SetColor(&rgba(1.0, 1.0, 1.0, 0.08 * presence));
            target.DrawRoundedRectangle(&rounded(edge), brush, 1.0, None);
            brush.SetColor(&rgba(1.0, 1.0, 1.0, presence));
            target.DrawTextLayout(self.text, &self.layout, brush, D2D1_DRAW_TEXT_OPTIONS_NONE);
        }
    }
}

/// The parts of `bounds` around `hole`, which lies inside it.
fn around(bounds: Rect, hole: Rect) -> impl Iterator<Item = Rect> {
    [
        Rect::new(bounds.left, bounds.top, bounds.right, hole.top),
        Rect::new(bounds.left, hole.bottom, bounds.right, bounds.bottom),
        Rect::new(bounds.left, hole.top, hole.left, hole.bottom),
        Rect::new(hole.right, hole.top, bounds.right, hole.bottom),
    ]
    .into_iter()
    .filter(|band| !band.is_empty())
}

/// Paints a band `thickness` pixels wide just inside the edges of `outer`.
unsafe fn outline(
    target: &ID2D1HwndRenderTarget,
    brush: &ID2D1SolidColorBrush,
    outer: Rect,
    thickness: f32,
    color: D2D1_COLOR_F,
) {
    let half = thickness / 2.0;
    let center = D2D_RECT_F {
        left: outer.left as f32 + half,
        top: outer.top as f32 + half,
        right: outer.right as f32 - half,
        bottom: outer.bottom as f32 - half,
    };
    unsafe {
        brush.SetColor(&color);
        target.DrawRectangle(&center, brush, thickness, None);
    }
}

fn rect_f(rect: Rect) -> D2D_RECT_F {
    D2D_RECT_F {
        left: rect.left as f32,
        top: rect.top as f32,
        right: rect.right as f32,
        bottom: rect.bottom as f32,
    }
}

const fn rgba(r: f32, g: f32, b: f32, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F { r, g, b, a }
}
