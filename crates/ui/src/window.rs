//! Small helpers shared by every Win32 window.

use std::cell::RefCell;

use windows::Win32::{
    Foundation::{HWND, LPARAM},
    UI::WindowsAndMessaging::{
        CREATESTRUCTW, GWLP_USERDATA, GetWindowLongPtrW, SetWindowLongPtrW, WM_NCCREATE,
    },
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
