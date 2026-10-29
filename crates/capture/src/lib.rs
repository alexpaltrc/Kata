//! Screen capture: freezes every monitor at the moment the capture starts,
//! handling mixed DPI and HDR displays.

use std::{ffi::c_void, mem::size_of, ptr, slice};

use kata_editor::{Image, Rect};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, RECT},
        Graphics::{
            Dwm::{
                DWM_WINDOW_CORNER_PREFERENCE, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS,
                DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND, DWMWCP_ROUNDSMALL,
                DwmGetWindowAttribute,
            },
            Gdi::{
                BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CAPTUREBLT, CreateCompatibleDC,
                CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, EnumDisplayMonitors,
                GetDC, HDC, HMONITOR, ROP_CODE, ReleaseDC, SRCCOPY, SelectObject,
            },
        },
        Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow},
        UI::{
            HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
            WindowsAndMessaging::{
                EnumWindows, GWL_EXSTYLE, GetClassNameW, GetSystemMetrics, GetWindowLongW,
                GetWindowRect, IsIconic, IsWindowVisible, IsZoomed, SM_CXVIRTUALSCREEN,
                SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, WS_EX_TRANSPARENT,
            },
        },
    },
    core::{BOOL, Error, Result},
};

/// Has `PrintWindow` ask the compositor for the window's pixels, which works for
/// apps drawn with DirectComposition, such as browsers. Undocumented, but Chromium
/// and WebRTC rely on it.
const PW_RENDERFULLCONTENT: PRINT_WINDOW_FLAGS = PRINT_WINDOW_FLAGS(2);

/// A display, in image coordinates.
#[derive(Clone, Copy, Debug)]
pub struct Monitor {
    pub rect: Rect,
    /// Display scale: 1.0 at 100 %, 1.5 at 150 %…
    pub scale: f32,
}

/// A top-level window, as it was when the screen froze.
#[derive(Clone, Copy, Debug)]
pub struct Window {
    /// The part of it on screen, in image coordinates.
    pub rect: Rect,
    /// Whether a window in front of it hides part of it.
    pub covered: bool,
    /// Its `HWND`, kept as a number so a `Window` can go to another thread.
    handle: usize,
    /// Its frame on the virtual screen.
    frame: Rect,
    /// Where its frame's top-left corner lies, from `rect`'s.
    at: (i32, i32),
    /// How round its corners are, in pixels.
    radius: f32,
}

impl Window {
    /// The window whole, as if nothing covered it, laid over `under`: the frozen
    /// screen inside `rect`. Windows draws it on request, in some 15 to 25 ms, or
    /// longer for an app that doesn't respond, so it's best asked for off the UI
    /// thread. When Windows can't, `under` comes back as it was.
    pub fn uncover(&self, mut under: Image) -> Image {
        if let Some(window) = self.draw() {
            under.paste_rounded(&window, self.at, self.radius);
        }
        under
    }

    /// The window's own pixels, inside its frame.
    fn draw(&self) -> Option<Image> {
        let hwnd = HWND(self.handle as *mut c_void);
        // Moved or resized since, it no longer matches the frozen screen.
        if unsafe { frame_bounds(hwnd) } != Some(self.frame) {
            return None;
        }
        let mut whole = RECT::default();
        unsafe { GetWindowRect(hwnd, &mut whole) }.ok()?;
        let whole = Rect::new(whole.left, whole.top, whole.right, whole.bottom);
        let image = render(whole.width(), whole.height(), |memory, _| unsafe {
            PrintWindow(hwnd, memory, PW_RENDERFULLCONTENT)
                .ok()
                .map_err(|_| Error::from_thread())
        })
        .ok()?;
        let frame = image.crop(self.frame.offset(-whole.left, -whole.top))?;
        // Some apps, such as games, give back nothing but black.
        let drawn = frame
            .pixels()
            .as_chunks::<{ Image::BYTES_PER_PIXEL }>()
            .0
            .iter()
            .any(|pixel| pixel[..3] != [0, 0, 0]);
        drawn.then_some(frame)
    }
}

/// Everything on screen at one instant. All rectangles are in image coordinates.
pub struct Screenshot {
    /// Where the image's top-left corner sits on the virtual screen.
    pub origin: (i32, i32),
    pub image: Image,
    /// Visible top-level windows, front to back.
    pub windows: Vec<Window>,
    pub monitors: Vec<Monitor>,
}

impl Screenshot {
    /// The highest display scale among the monitors `area` touches.
    pub fn scale_at(&self, area: Rect) -> f32 {
        highest_scale(&self.monitors, area)
    }
}

/// Captures every monitor at once. The process must be per-monitor DPI aware.
pub fn capture() -> Result<Screenshot> {
    let area = unsafe {
        let left = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let top = GetSystemMetrics(SM_YVIRTUALSCREEN);
        Rect::new(
            left,
            top,
            left + GetSystemMetrics(SM_CXVIRTUALSCREEN),
            top + GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    };
    // Windows first: listing them is instant, and the pixels must match what was listed.
    let listed = visible_windows();
    let image = grab(area)?;
    let bounds = image.bounds();
    let to_image = |rect: Rect| rect.offset(-area.left, -area.top).intersect(&bounds);
    let monitors = monitors();
    let windows = listed
        .iter()
        .enumerate()
        .filter_map(|(index, &(hwnd, frame))| {
            let rect = to_image(frame)?;
            Some(Window {
                rect,
                covered: listed[..index]
                    .iter()
                    .any(|(_, front)| front.intersect(&frame).is_some()),
                handle: hwnd.0 as usize,
                frame,
                at: (
                    frame.left - area.left - rect.left,
                    frame.top - area.top - rect.top,
                ),
                radius: unsafe { corner_radius(hwnd) } * highest_scale(&monitors, frame),
            })
        })
        .collect();
    Ok(Screenshot {
        origin: (area.left, area.top),
        windows,
        monitors: monitors
            .into_iter()
            .filter_map(|monitor| {
                Some(Monitor {
                    rect: to_image(monitor.rect)?,
                    ..monitor
                })
            })
            .collect(),
        image,
    })
}

/// The highest display scale among the `monitors` that `area` touches.
fn highest_scale(monitors: &[Monitor], area: Rect) -> f32 {
    monitors
        .iter()
        .filter(|monitor| monitor.rect.intersect(&area).is_some())
        .map(|monitor| monitor.scale)
        .reduce(f32::max)
        .unwrap_or(1.0)
}

fn grab(area: Rect) -> Result<Image> {
    render(area.width(), area.height(), |memory, screen| unsafe {
        BitBlt(
            memory,
            0,
            0,
            area.width(),
            area.height(),
            Some(screen),
            area.left,
            area.top,
            ROP_CODE(SRCCOPY.0 | CAPTUREBLT.0),
        )
    })
}

/// An image `width` by `height` pixels, drawn by `draw` into a memory DC, with
/// the screen's DC at hand.
fn render(width: i32, height: i32, draw: impl FnOnce(HDC, HDC) -> Result<()>) -> Result<Image> {
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    unsafe {
        let screen = GetDC(None);
        let memory = CreateCompatibleDC(Some(screen));
        let mut bits: *mut c_void = ptr::null_mut();
        let image = CreateDIBSection(Some(screen), &info, DIB_RGB_COLORS, &mut bits, None, 0)
            .and_then(|bitmap| {
                let previous = SelectObject(memory, bitmap.into());
                let image = draw(memory, screen).map(|()| {
                    let len = width as usize * height as usize * Image::BYTES_PER_PIXEL;
                    let mut pixels = slice::from_raw_parts(bits.cast::<u8>(), len).to_vec();
                    // GDI leaves the alpha byte undefined; Kata's images are always opaque.
                    for pixel in pixels.as_chunks_mut::<{ Image::BYTES_PER_PIXEL }>().0 {
                        pixel[3] = u8::MAX;
                    }
                    Image::new(width as u32, height as u32, pixels)
                });
                SelectObject(memory, previous);
                let _ = DeleteObject(bitmap.into());
                image
            });
        let _ = DeleteDC(memory);
        ReleaseDC(None, screen);
        image
    }
}

/// The windows the user can see, front to back, with their frames.
fn visible_windows() -> Vec<(HWND, Rect)> {
    let mut windows: Vec<(HWND, Rect)> = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(collect_window), LPARAM(&raw mut windows as isize));
    }
    windows
}

unsafe extern "system" fn collect_window(hwnd: HWND, windows: LPARAM) -> BOOL {
    let windows = unsafe { &mut *(windows.0 as *mut Vec<(HWND, Rect)>) };
    if let Some(rect) = unsafe { window_rect(hwnd) } {
        windows.push((hwnd, rect));
    }
    true.into()
}

/// The visible frame of a window the user can see and click, if it is one.
unsafe fn window_rect(hwnd: HWND) -> Option<Rect> {
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() || is_desktop(hwnd) {
            return None;
        }
        // Click-through overlays are invisible to the user.
        if GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TRANSPARENT.0 != 0 {
            return None;
        }
        let mut cloaked = 0u32;
        let _ = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            (&raw mut cloaked).cast(),
            size_of::<u32>() as u32,
        );
        if cloaked != 0 {
            return None;
        }
        frame_bounds(hwnd).filter(|rect| !rect.is_empty())
    }
}

/// A window's visible frame, without the invisible borders it's resized by.
unsafe fn frame_bounds(hwnd: HWND) -> Option<Rect> {
    let mut frame = RECT::default();
    unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&raw mut frame).cast(),
            size_of::<RECT>() as u32,
        )
    }
    .ok()?;
    Some(Rect::new(frame.left, frame.top, frame.right, frame.bottom))
}

/// How round Windows 11 draws a window's corners, at 100 % scale.
unsafe fn corner_radius(hwnd: HWND) -> f32 {
    if unsafe { IsZoomed(hwnd) }.as_bool() {
        return 0.0;
    }
    let mut preference = DWM_WINDOW_CORNER_PREFERENCE::default();
    let _ = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&raw mut preference).cast(),
            size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        )
    };
    match preference {
        DWMWCP_DONOTROUND => 0.0,
        DWMWCP_ROUNDSMALL => 4.0,
        _ => 8.0,
    }
}

/// The desktop's own windows: hovering the wallpaper selects the monitor instead.
unsafe fn is_desktop(hwnd: HWND) -> bool {
    let mut name = [0u16; 16];
    let len = unsafe { GetClassNameW(hwnd, &mut name) } as usize;
    matches!(
        String::from_utf16_lossy(&name[..len]).as_str(),
        "Progman" | "WorkerW"
    )
}

fn monitors() -> Vec<Monitor> {
    let mut monitors: Vec<Monitor> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(collect_monitor),
            LPARAM(&raw mut monitors as isize),
        );
    }
    monitors
}

unsafe extern "system" fn collect_monitor(
    monitor: HMONITOR,
    _: HDC,
    rect: *mut RECT,
    monitors: LPARAM,
) -> BOOL {
    let monitors = unsafe { &mut *(monitors.0 as *mut Vec<Monitor>) };
    let rect = unsafe { *rect };
    let mut dpi = (96, 96);
    let _ = unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi.0, &mut dpi.1) };
    monitors.push(Monitor {
        rect: Rect::new(rect.left, rect.top, rect.right, rect.bottom),
        scale: dpi.0 as f32 / 96.0,
    });
    true.into()
}
