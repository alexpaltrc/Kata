//! Small helpers shared by every Win32 window.

use std::{cell::RefCell, mem::size_of};

use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
        Graphics::{
            Dwm::{
                DWMSBT_MAINWINDOW, DWMWA_CAPTION_BUTTON_BOUNDS, DWMWA_SYSTEMBACKDROP_TYPE,
                DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWINDOWATTRIBUTE, DwmDefWindowProc,
                DwmExtendFrameIntoClientArea, DwmGetWindowAttribute, DwmSetWindowAttribute,
            },
            Gdi::{ClientToScreen, ScreenToClient},
        },
        UI::{
            Controls::MARGINS,
            HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi},
            WindowsAndMessaging::{
                CREATESTRUCTW, DefWindowProcW, GWLP_USERDATA, GetWindowLongPtrW, GetWindowRect,
                HTCLIENT, HTTOP, IsZoomed, NCCALCSIZE_PARAMS, SM_CXPADDEDBORDER, SM_CYSIZEFRAME,
                SetWindowLongPtrW, WM_NCCALCSIZE, WM_NCCREATE, WM_NCHITTEST,
            },
        },
    },
    core::BOOL,
};

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

/// The screen position carried by `WM_NCHITTEST`, in client pixels.
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

/// Lets the client area cover the title bar, while Windows still draws the
/// caption buttons and handles the resize borders. Call it first in the window
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
        // The caption buttons.
        let mut result = LRESULT(0);
        if DwmDefWindowProc(hwnd, message, wparam, lparam, &mut result).as_bool() {
            return Some(result);
        }
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

/// Where Windows draws the caption buttons, in client pixels.
pub fn caption_buttons(hwnd: HWND) -> Option<RECT> {
    let mut bounds = RECT::default();
    let mut window = RECT::default();
    let mut client = POINT::default();
    unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CAPTION_BUTTON_BOUNDS,
            (&raw mut bounds).cast(),
            size_of::<RECT>() as u32,
        )
        .ok()?;
        GetWindowRect(hwnd, &mut window).ok()?;
        ClientToScreen(hwnd, &mut client).ok().ok()?;
    }
    // The bounds are relative to the window's top-left corner.
    let (dx, dy) = (window.left - client.x, window.top - client.y);
    Some(RECT {
        left: bounds.left + dx,
        top: bounds.top + dy,
        right: bounds.right + dx,
        bottom: bounds.bottom + dy,
    })
}

/// How thick the resize border is, in pixels.
fn resize_border(hwnd: HWND) -> i32 {
    unsafe {
        let dpi = GetDpiForWindow(hwnd);
        GetSystemMetricsForDpi(SM_CYSIZEFRAME, dpi) + GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi)
    }
}
