//! Windows' screenshot keys, taken over for Kata: Print Screen, with or without
//! modifiers, and Win + Shift + S. Windows keeps Alt and Win + Print Screen for
//! itself, so a low-level keyboard hook, on a thread of its own, claims them
//! before Windows sees them. It looks at nothing but those keys, and lets every
//! other key pass at once.

use std::{
    mem::size_of,
    sync::{
        atomic::{AtomicU32, AtomicUsize, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
};

use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    System::Threading::GetCurrentThreadId,
    UI::{
        Input::KeyboardAndMouse::{
            GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
            SendInput, VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT, VK_SNAPSHOT,
        },
        WindowsAndMessaging::{
            CallNextHookEx, DispatchMessageW, GetMessageW, HC_ACTION, KBDLLHOOKSTRUCT, MSG,
            PM_NOREMOVE, PeekMessageW, PostMessageW, PostThreadMessageW, SetWindowsHookExW,
            UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_QUIT, WM_SYSKEYDOWN,
        },
    },
};

use crate::{SCREEN_KEY, report};

/// An unassigned key. Pressed in place of a key Kata claims while Win or Alt is
/// held, it keeps Windows from taking their release alone as a tap, which would
/// open Start or a menu bar.
const MASK: VIRTUAL_KEY = VIRTUAL_KEY(0xE8);

/// Where the hook sends the keys it claims: Kata's resident window.
static OWNER: AtomicUsize = AtomicUsize::new(0);
/// The key claimed last, until it's released: its repeats and release are Kata's too.
static CLAIMED: AtomicU32 = AtomicU32::new(0);

/// The screenshot keys, while Kata holds them. Dropping it gives them back.
pub struct ScreenKeys {
    thread: Option<JoinHandle<()>>,
    thread_id: u32,
}

/// Claims the screenshot keys for `owner`, which gets `SCREEN_KEY` on each press.
pub fn take(owner: HWND) -> ScreenKeys {
    OWNER.store(owner.0 as usize, Ordering::Relaxed);
    let (started, thread_id) = mpsc::channel();
    let thread = thread::spawn(move || unsafe {
        // A message queue first, so the thread can be told to stop.
        let mut message = MSG::default();
        let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
        let _ = started.send(GetCurrentThreadId());
        match SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook), None, 0) {
            Ok(hook) => {
                while GetMessageW(&mut message, None, 0, 0).0 > 0 {
                    DispatchMessageW(&message);
                }
                let _ = UnhookWindowsHookEx(hook);
            }
            Err(error) => report("screen keys", &error),
        }
    });
    ScreenKeys {
        thread: Some(thread),
        thread_id: thread_id.recv().unwrap_or_default(),
    }
}

impl Drop for ScreenKeys {
    fn drop(&mut self) {
        let _ = unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

unsafe extern "system" fn hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let key = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) }.vkCode;
        let down = matches!(wparam.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
        if key == CLAIMED.load(Ordering::Relaxed) {
            if !down {
                CLAIMED.store(0, Ordering::Relaxed);
            }
            return LRESULT(1);
        }
        if down && is_screen_key(key) {
            CLAIMED.store(key, Ordering::Relaxed);
            if held(VK_LWIN) || held(VK_RWIN) || held(VK_MENU) {
                mask();
            }
            let owner = HWND(OWNER.load(Ordering::Relaxed) as *mut _);
            let _ = unsafe { PostMessageW(Some(owner), SCREEN_KEY, WPARAM(0), LPARAM(0)) };
            return LRESULT(1);
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// Print Screen, with any modifiers, or the S of Win + Shift + S.
fn is_screen_key(key: u32) -> bool {
    key == u32::from(VK_SNAPSHOT.0)
        || key == u32::from(b'S')
            && (held(VK_LWIN) || held(VK_RWIN))
            && held(VK_SHIFT)
            && !held(VK_CONTROL)
            && !held(VK_MENU)
}

/// Whether `key` is down now. A hook sees keys before any thread does, so it
/// can't ask its own thread's keyboard state.
fn held(key: VIRTUAL_KEY) -> bool {
    unsafe { GetAsyncKeyState(i32::from(key.0)) < 0 }
}

/// Taps `MASK`.
fn mask() {
    let tap = |flags| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: MASK,
                dwFlags: flags,
                ..Default::default()
            },
        },
    };
    let inputs = [tap(Default::default()), tap(KEYEVENTF_KEYUP)];
    unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
}
