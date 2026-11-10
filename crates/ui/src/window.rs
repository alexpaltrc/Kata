//! Small helpers shared by every Win32 window.

use std::{cell::RefCell, mem::size_of};

use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
        Graphics::{
            Dwm::{
                DWMSBT_MAINWINDOW, DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_IMMERSIVE_DARK_MODE,
                DWMWINDOWATTRIBUTE, DwmExtendFrameIntoClientArea, DwmSetWindowAttribute,
            },
            Gdi::{GetMonitorInfoW, HMONITOR, MONITORINFO, ScreenToClient},
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Controls::{HOVER_DEFAULT, MARGINS},
            HiDpi::{
                AdjustWindowRectExForDpi, GetDpiForMonitor, GetDpiForWindow,
                GetSystemMetricsForDpi, MDT_EFFECTIVE_DPI,
            },
            Input::KeyboardAndMouse::{
                GetKeyState, TME_HOVER, TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT,
                TRACKMOUSEEVENT_FLAGS, TrackMouseEvent, VIRTUAL_KEY,
            },
            WindowsAndMessaging::{
                CREATESTRUCTW, CreateWindowExW, DefWindowProcW, GWLP_USERDATA, GetClientRect,
                GetWindowLongPtrW, HTCLIENT, HTTOP, IDC_ARROW, IsZoomed, LoadCursorW, MINMAXINFO,
                NCCALCSIZE_PARAMS, RegisterClassExW, SM_CXPADDEDBORDER, SM_CYSIZEFRAME,
                SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
                SetWindowLongPtrW, SetWindowPos, WINDOW_STYLE, WM_NCCALCSIZE, WM_NCCREATE,
                WM_NCHITTEST, WNDCLASSEXW, WS_EX_NOREDIRECTIONBITMAP, WS_OVERLAPPEDWINDOW,
                WS_SYSMENU,
            },
        },
    },
    core::{BOOL, HSTRING, PCWSTR, Result},
};

/// A window with a frame to resize it by and a place in the taskbar, but no caption/// buttons of Windows' own: it draws its own (`controls::Caption`).const FRAME: WINDOW_STYLE =    WINDOW_STYLE(WS_OVERLAPPEDWINDOW.0 & !WS_SYSMENU.0);
/// The state linked to `hwnd`. Call it first thing in the window procedure: on
/// `WM_NCCREATE` it links the pointer passed as `CreateWindowExW`'s last argument.
///
/// # Safety
/// That pointer must point to a `RefCell<T>` that outlives the window.
pub unsafe fn linked_state<'a, T>(
    hwnd: HWND,
    message: u32,
    lparam: LPARAM,
) -> Option<&'a RefCell<T>> {
    unsafe {
        if message == WM_NCCREATE {
            let create = &*(lparam.0 as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        }
        (GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const RefCell<T>).as_ref()
    }
}

/// The pointer position carried by mouse messages, in client pixels.
pub fn mouse_position(lparam: LPARAM) -> (i32, i32) {
    let x = lparam.0 as u16 as i16;
    let y = (lparam.0 >> 16) as u16 as i16;
    (x.into(), y.into())
}

/// The screen position carried by `WM_NCHITTEST` and `WM_MOUSEWHEEL`, in client
/// pixels.
pub fn hit_position(hwnd: HWND, lparam: LPARAM) -> (i32, i32) {
    let (x, y) = mouse_position(lparam);
    let mut point = POINT { x, y };
    let _ = unsafe { ScreenToClient(hwnd, &mut point) };
    (point.x, point.y)
}

/// Sets one of the window's DWM attributes. A failure only loses the effect.
pub fn set_attribute<T: Copy>(hwnd: HWND, attribute: DWMWINDOWATTRIBUTE, value: T) {
    let size = size_of::<T>() as u32;
    let _ = unsafe { DwmSetWindowAttribute(hwnd, attribute, (&raw const value).cast(), size) };
}

/// Puts the Mica backdrop behind the whole window, title bar included. The window
/// then draws its client area transparently wherever Mica should show.
pub fn use_mica(hwnd: HWND, dark: bool) {
    let everywhere = MARGINS {
        cxLeftWidth: -1,
        cxRightWidth: -1,
        cyTopHeight: -1,
        cyBottomHeight: -1,
    };
    let _ = unsafe { DwmExtendFrameIntoClientArea(hwnd, &everywhere) };
    set_dark(hwnd, dark);
    set_attribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, DWMSBT_MAINWINDOW);
}

/// Switches the window's frame and backdrop between the dark and light themes.
pub fn set_dark(hwnd: HWND, dark: bool) {
    set_attribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, BOOL::from(dark));
}

/// Lets the client area cover the title bar, while Windows still handles the
/// resize borders. Call it first in the window
/// procedure and return its result when there is one.
///
/// Over the client area, `WM_NCHITTEST` is left to the window: it answers
/// `HTCAPTION` where its title bar is empty and `HTCLIENT` elsewhere.
///
/// # Safety
/// Pass the window procedure's arguments unchanged.
pub unsafe fn frame_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    unsafe {
        match message {
            WM_NCCALCSIZE if wparam.0 != 0 => {
                // Keep the standard side and bottom borders, without the caption.
                let params = &mut *(lparam.0 as *mut NCCALCSIZE_PARAMS);
                let top = params.rgrc[0].top;
                DefWindowProcW(hwnd, message, wparam, lparam);
                params.rgrc[0].top = top;
                // Maximized windows hang their borders off the screen's edges.
                if IsZoomed(hwnd).as_bool() {
                    params.rgrc[0].top += resize_border(hwnd);
                }
                Some(LRESULT(0))
            }
            WM_NCHITTEST => {
                let hit = DefWindowProcW(hwnd, message, wparam, lparam);
                if hit.0 != HTCLIENT as isize {
                    return Some(hit);
                }
                let (_, y) = hit_position(hwnd, lparam);
                (y < resize_border(hwnd) && !IsZoomed(hwnd).as_bool())
                    .then_some(LRESULT(HTTOP as isize))
            }
            _ => None,
        }
    }
}

/// The part of a monitor that isn't under the taskbar.
pub struct WorkArea {
    /// In pixels, on the virtual screen.
    pub rect: RECT,
    /// The monitor's pixels per DIP.
    pub scale: f32,
}

impl WorkArea {
    pub fn of(monitor: HMONITOR) -> Self {
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let (mut dpi, mut dpi_y) = (96, 96);
        unsafe {
            let _ = GetMonitorInfoW(monitor, &mut info);
            let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi, &mut dpi_y);
        }
        Self {
            rect: info.rcWork,
            scale: dpi as f32 / 96.0,
        }
    }

    pub fn width(&self) -> i32 {
        self.rect.right - self.rect.left
    }

    pub fn height(&self) -> i32 {
        self.rect.bottom - self.rect.top
    }
}

/// Creates a hidden window of class `class`, named `title` in the taskbar, whose
/// client area of `client` pixels covers the title bar over Mica, centered in `work`.
pub fn create(
    class: PCWSTR,
    title: &str,
    proc: WindowProc,
    work: &WorkArea,
    client: (i32, i32),
    dark: bool,
) -> Result<HWND> {
    let dpi = (work.scale * 96.0).round() as u32;
    let (width, height) = frame_size(client, dpi);
    unsafe {
        let instance = GetModuleHandleW(None)?.into();
        // Every window after the first registers the class again, which fails
        // harmlessly.
        RegisterClassExW(&WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(proc),
            hInstance: instance,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            lpszClassName: class,
            ..Default::default()
        });
        let hwnd = CreateWindowExW(
            WS_EX_NOREDIRECTIONBITMAP,
            class,
            &HSTRING::from(title),
            FRAME,
            work.rect.left + (work.width() - width) / 2,
            work.rect.top + (work.height() - height) / 2,
            width,
            height,
            None,
            None,
            Some(instance),
            None,
        )?;
        use_mica(hwnd, dark);
        // Lets the client area take over the caption.
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
        Ok(hwnd)
    }
}

/// A window procedure.
pub type WindowProc = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;

/// The size of a window whose client area, which covers the caption, measures
/// `client` pixels.
fn frame_size(client: (i32, i32), dpi: u32) -> (i32, i32) {
    let mut frame = RECT {
        left: 0,
        top: 0,
        right: client.0,
        bottom: client.1,
    };
    let _ = unsafe {
        AdjustWindowRectExForDpi(&mut frame, FRAME, false, WS_EX_NOREDIRECTIONBITMAP, dpi)
    };
    // Only the side and bottom borders add.
    (frame.right - frame.left, frame.bottom)
}

/// The client area's size, in pixels.
pub fn client_size(hwnd: HWND) -> (u32, u32) {
    let mut client = RECT::default();
    let _ = unsafe { GetClientRect(hwnd, &mut client) };
    (client.right.max(0) as u32, client.bottom.max(0) as u32)
}

/// The window's pixels per DIP.
pub fn scale(hwnd: HWND) -> f32 {
    unsafe { GetDpiForWindow(hwnd) as f32 / 96.0 }
}

/// Moves the window where Windows suggests on `WM_DPICHANGED`, and returns its new
/// pixels per DIP.
///
/// # Safety
/// Pass the message's arguments unchanged.
pub unsafe fn follow_dpi(hwnd: HWND, wparam: WPARAM, lparam: LPARAM) -> f32 {
    unsafe {
        let suggested = *(lparam.0 as *const RECT);
        let _ = SetWindowPos(
            hwnd,
            None,
            suggested.left,
            suggested.top,
            suggested.right - suggested.left,
            suggested.bottom - suggested.top,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
    f32::from(wparam.0 as u16) / 96.0
}

/// Keeps the client area at least `min` DIPs on `WM_GETMINMAXINFO`.
///
/// # Safety
/// Pass the message's `lparam` unchanged.
pub unsafe fn limit_size(lparam: LPARAM, min: (f32, f32), scale: f32) {
    let client = ((min.0 * scale).ceil() as i32, (min.1 * scale).ceil() as i32);
    let (x, y) = frame_size(client, (scale * 96.0).round() as u32);
    let info = unsafe { &mut *(lparam.0 as *mut MINMAXINFO) };
    info.ptMinTrackSize = POINT { x, y };
}

/// Asks Windows to send `WM_MOUSELEAVE` once the pointer leaves the window, and
/// returns whether it will.
pub fn track_mouse_leave(hwnd: HWND) -> bool {
    track_mouse(hwnd, TME_LEAVE)
}

/// Asks Windows to send `WM_NCMOUSELEAVE` once the pointer leaves the parts of the
/// window that aren't its client area, such as its caption buttons, and returns
/// whether it will.
pub fn track_frame_leave(hwnd: HWND) -> bool {
    track_mouse(hwnd, TME_LEAVE | TME_NONCLIENT)
}

/// Asks Windows to send `WM_MOUSEHOVER` once the pointer rests on the window as
/// long as a tooltip waits, starting over now, and returns whether it will.
pub fn track_mouse_rest(hwnd: HWND) -> bool {
    track_mouse(hwnd, TME_HOVER | TME_LEAVE)
}

fn track_mouse(hwnd: HWND, flags: TRACKMOUSEEVENT_FLAGS) -> bool {
    let mut track = TRACKMOUSEEVENT {
        cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
        dwFlags: flags,
        hwndTrack: hwnd,
        dwHoverTime: HOVER_DEFAULT,
    };
    unsafe { TrackMouseEvent(&mut track) }.is_ok()
}

/// Whether `key` was down when the message being handled was sent.
pub fn held(key: VIRTUAL_KEY) -> bool {
    unsafe { GetKeyState(i32::from(key.0)) < 0 }
}

/// How thick the resize border is, in pixels.
fn resize_border(hwnd: HWND) -> i32 {
    unsafe {
        let dpi = GetDpiForWindow(hwnd);
        GetSystemMetricsForDpi(SM_CYSIZEFRAME, dpi) + GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi)
    }
}

/// A window with a frame to resize it by and a place in the taskbar, but none of
/// Windows' caption buttons: it draws its own (`controls::Caption`).
const FRAME: WINDOW_STYLE = WINDOW_STYLE(WS_OVERLAPPEDWINDOW.0 & !WS_SYSMENU.0);
