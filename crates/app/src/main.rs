//! Kata: startup, tray icon, global hotkey and window orchestration.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod editor;
mod export;
mod overlay;
mod settings;
mod sound;
mod strings;
mod tray;

use std::{cell::RefCell, fmt::Display, mem::size_of, ptr};

use kata_ui::{Graphics, theme, window};
use tray::Tray;
use windows::{
    Win32::{
        Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HWND, LPARAM, LRESULT, WPARAM},
        System::{
            Com::{COINIT_APARTMENTTHREADED, CoInitializeEx},
            LibraryLoader::GetModuleHandleW,
            Threading::CreateMutexW,
        },
        UI::{
            HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext},
            Input::KeyboardAndMouse::{
                MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, RegisterHotKey, UnregisterHotKey,
            },
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
                KillTimer, MSG, PostQuitMessage, RegisterClassExW, RegisterWindowMessageW,
                SetTimer, TranslateMessage, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_HOTKEY,
                WM_SETTINGCHANGE, WM_TIMER, WNDCLASSEXW, WS_EX_TOOLWINDOW, WS_OVERLAPPED,
            },
        },
    },
    core::{Owned, Result, w},
};

/// The shortcut that starts a capture, as people read it.
pub const HOTKEY_LABEL: &str = "Ctrl + Shift + S";
const HOTKEY_ID: i32 = 1;
/// Captures started from the tray wait for its menu or flyout to fade away first.
const TRAY_CAPTURE_TIMER: usize = 1;
const TRAY_CAPTURE_DELAY_MS: u32 = 200;

fn main() -> Result<()> {
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)?;
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
    }
    let Some(_instance) = claim_single_instance()? else {
        return Ok(());
    };
    theme::follow_system_theme_in_menus();
    sound::prepare();

    let app = RefCell::new(None);
    let hwnd = unsafe { create_window(&app)? };
    *app.borrow_mut() = Some(App::new(hwnd, Graphics::new()?)?);
    unsafe {
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    drop(app.take());
    unsafe { DestroyWindow(hwnd) }
}

/// Holds the single-instance lock, or `None` when Kata is already running.
fn claim_single_instance() -> Result<Option<Owned<HANDLE>>> {
    unsafe {
        let lock = Owned::new(CreateMutexW(None, false, w!(r"Local\Kata"))?);
        Ok((GetLastError() != ERROR_ALREADY_EXISTS).then_some(lock))
    }
}

/// Kata's resident window. It's never shown: it receives the hotkey, tray and
/// system messages.
unsafe fn create_window(app: &RefCell<Option<App>>) -> Result<HWND> {
    unsafe {
        let instance = GetModuleHandleW(None)?.into();
        RegisterClassExW(&WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: w!("Kata"),
            ..Default::default()
        });
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            w!("Kata"),
            w!("Kata"),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance),
            Some(ptr::from_ref(app).cast()),
        )
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // While a capture is in progress the app is busy, and its messages get default
    // handling: a second hotkey press, for instance, is ignored.
    let state = unsafe { window::linked_state::<Option<App>>(hwnd, message, lparam) };
    let handled = state
        .and_then(|state| state.try_borrow_mut().ok())
        .is_some_and(|mut app| {
            app.as_mut()
                .is_some_and(|app| app.handle(message, wparam, lparam))
        });
    if handled {
        LRESULT(0)
    } else {
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }
}

struct App {
    hwnd: HWND,
    graphics: Graphics,
    tray: Tray,
    /// Explorer broadcasts it after restarting; the tray icon must be added again.
    taskbar_created: u32,
}

impl App {
    fn new(hwnd: HWND, graphics: Graphics) -> Result<Self> {
        let tray = Tray::new(hwnd, &graphics)?;
        let hotkey = unsafe {
            RegisterHotKey(
                Some(hwnd),
                HOTKEY_ID,
                MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT,
                u32::from(b'S'),
            )
        };
        if let Err(error) = hotkey {
            report("hotkey", &error);
        }
        Ok(Self {
            hwnd,
            graphics,
            tray,
            taskbar_created: unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) },
        })
    }

    fn handle(&mut self, message: u32, wparam: WPARAM, lparam: LPARAM) -> bool {
        match message {
            WM_HOTKEY => self.capture(),
            tray::MESSAGE => match tray::event(wparam, lparam) {
                Some(tray::Event::Activate) => self.capture_after_tray(),
                Some(tray::Event::Menu { x, y }) => match self.tray.menu(x, y) {
                    Some(tray::Command::Capture) => self.capture_after_tray(),
                    Some(tray::Command::OpenFolder) => export::open_folder(),
                    Some(tray::Command::Exit) => unsafe { PostQuitMessage(0) },
                    None => {}
                },
                None => return false,
            },
            WM_TIMER if wparam.0 == TRAY_CAPTURE_TIMER => {
                unsafe {
                    let _ = KillTimer(Some(self.hwnd), TRAY_CAPTURE_TIMER);
                }
                self.capture();
            }
            WM_SETTINGCHANGE | WM_DISPLAYCHANGE | WM_DPICHANGED => {
                self.tray.refresh(&self.graphics);
            }
            _ if message == self.taskbar_created => self.tray.restore(),
            _ => return false,
        }
        true
    }

    /// Freezes the screen, lets the user choose an area and opens it in the editor,
    /// which delivers it.
    fn capture(&mut self) {
        let shot = match kata_capture::capture() {
            Ok(shot) => shot,
            Err(error) => {
                report("capture", &error);
                return;
            }
        };
        let Some(area) = overlay::select(&self.graphics, &shot) else {
            return;
        };
        let Some(image) = shot.image.crop(area) else {
            return;
        };
        let capture = editor::Capture {
            image,
            area: area.offset(shot.origin.0, shot.origin.1),
            scale: shot.scale_at(area),
        };
        drop(shot);
        sound::play();
        editor::open(&self.graphics, self.hwnd, capture);
    }

    fn capture_after_tray(&self) {
        unsafe {
            SetTimer(
                Some(self.hwnd),
                TRAY_CAPTURE_TIMER,
                TRAY_CAPTURE_DELAY_MS,
                None,
            );
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        unsafe {
            let _ = UnregisterHotKey(Some(self.hwnd), HOTKEY_ID);
        }
        // The editor's last changes are written before Kata exits.
        editor::close();
        export::wait_for_saves();
    }
}

/// Notes a failure Kata recovers from. Release builds stay silent for now.
fn report(context: &str, error: &dyn Display) {
    if cfg!(debug_assertions) {
        eprintln!("kata: {context}: {error}");
    }
}
