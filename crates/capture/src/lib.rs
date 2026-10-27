//! Screen capture: freezes every monitor at the moment the capture starts,
//! handling mixed DPI and HDR displays.

use std::{ffi::c_void, mem::size_of, ptr, slice};

use kata_editor::{Image, Rect};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, RECT},
        Graphics::{
            Dwm::{DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute},
            Gdi::{
                BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CAPTUREBLT, CreateCompatibleDC,
                CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, EnumDisplayMonitors,
                GetDC, HDC, HMONITOR, ROP_CODE, ReleaseDC, SRCCOPY, SelectObject,
            },
        },
        UI::{
            HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
            WindowsAndMessaging::{
                EnumWindows, GWL_EXSTYLE, GetClassNameW, GetSystemMetrics, GetWindowLongW,
                IsIconic, IsWindowVisible, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
                SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, WS_EX_TRANSPARENT,
            },
        },
    },
    core::{BOOL, Result},
};

/// A display, in image coordinates.
#[derive(Clone, Copy, Debug)]
pub struct Monitor {
    pub rect: Rect,
    /// Display scale: 1.0 at 100 %, 1.5 at 150 %…
    pub scale: f32,
}

/// Everything on screen at one instant. All rectangles are in image coordinates.
pub struct Screenshot {
    /// Where the image's top-left corner sits on the virtual screen.
    pub origin: (i32, i32),
    pub image: Image,
    /// Visible top-level windows, front to back.
    pub windows: Vec<Rect>,
    pub monitors: Vec<Monitor>,
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
    let windows = visible_windows();
    let image = grab(area)?;
    let bounds = image.bounds();
    let to_image = |rect: Rect| rect.offset(-area.left, -area.top).intersect(&bounds);
    Ok(Screenshot {
        origin: (area.left, area.top),
        windows: windows.into_iter().filter_map(to_image).collect(),
        monitors: monitors()
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

fn grab(area: Rect) -> Result<Image> {
    let (width, height) = (area.width(), area.height());
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
                let copied = BitBlt(
                    memory,
                    0,
                    0,
                    width,
                    height,
                    Some(screen),
                    area.left,
                    area.top,
                    ROP_CODE(SRCCOPY.0 | CAPTUREBLT.0),
                );
                let image = copied.map(|()| {
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

fn visible_windows() -> Vec<Rect> {
    let mut rects: Vec<Rect> = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(collect_window), LPARAM(&raw mut rects as isize));
    }
    rects
}

unsafe extern "system" fn collect_window(hwnd: HWND, rects: LPARAM) -> BOOL {
    let rects = unsafe { &mut *(rects.0 as *mut Vec<Rect>) };
    if let Some(rect) = unsafe { window_rect(hwnd) } {
        rects.push(rect);
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
        let mut frame = RECT::default();
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&raw mut frame).cast(),
            size_of::<RECT>() as u32,
        )
        .ok()?;
        let rect = Rect::new(frame.left, frame.top, frame.right, frame.bottom);
        (!rect.is_empty()).then_some(rect)
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
