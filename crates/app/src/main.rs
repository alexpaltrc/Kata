//! Kata: startup, tray icon, global hotkey and window orchestration.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod channel;
mod editor;
mod export;
mod hotkey;
mod main_window;
mod overlay;
mod registry;
mod screen_keys;
mod settings;
mod sound;
mod startup;
mod strings;
mod tray;

use std::{cell::RefCell, fmt::Display, mem::size_of, ptr};

use kata_ui::{Graphics, theme, window};
use main_window::Page;
use screen_keys::ScreenKeys;
use settings::Settings;
use tray::Tray;
use windows::{
    ApplicationModel::Package,
    Win32::{
        Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HWND, LPARAM, LRESULT, WPARAM},
        System::{
            Com::{COINIT_APARTMENTTHREADED, CoInitializeEx},
            LibraryLoader::GetModuleHandleW,
            Threading::CreateMutexW,
        },
        UI::{
            HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext},
            WindowsAndMessaging::{
                AllowSetForegroundWindow, CreateWindowExW, DefWindowProcW, DestroyWindow,
                DispatchMessageW, FindWindowW, GetMessageW, GetWindowThreadProcessId, KillTimer,
                MSG, PostMessageW, PostQuitMessage, RegisterClassExW, RegisterWindowMessageW,
                SetTimer, TranslateMessage, WM_APP, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_HOTKEY,
                WM_SETTINGCHANGE, WM_TIMER, WNDCLASSEXW, WS_EX_TOOLWINDOW, WS_OVERLAPPED,
            },
        },
    },
    core::{HSTRING, Owned, PCWSTR, Result, h, w},
};

/// Shows the main window. Starting Kata again sends it to the Kata running.
pub const SHOW: u32 = WM_APP + 2;
/// Starts a capture, from the main window.
pub const CAPTURE: u32 = WM_APP + 3;
/// The capture shortcut, or whether Kata takes the screenshot keys, changed.
pub const KEYS_CHANGED: u32 = WM_APP + 4;
/// One of Windows' screenshot keys was pressed, while Kata takes them.
pub const SCREEN_KEY: u32 = WM_APP + 5;
const CLASS: PCWSTR = w!("Kata");
/// Captures started from the tray or the main window wait for it to fade away first.
const CAPTURE_TIMER: usize = 1;
const CAPTURE_DELAY_MS: u32 = 200;

fn main() -> Result<()> {
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)?;
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
    }
    // Started at sign-in, Kata waits in the tray.
    let quiet = startup::at_sign_in();
    let name = instance_name();
    let Some(_instance) = claim_single_instance(&name)? else {
        if !quiet {
            show_running(&name);
        }
        return Ok(());
    };
    theme::follow_system_theme_in_menus();
    sound::prepare();

    let app = RefCell::new(None);
    let hwnd = unsafe { create_window(&app, &name)? };
    *app.borrow_mut() = Some(App::new(hwnd, Graphics::new()?)?);
    unsafe {
        if !quiet {
            PostMessageW(Some(hwnd), SHOW, WPARAM(0), LPARAM(0))?;
        }
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    drop(app.take());
    unsafe { DestroyWindow(hwnd) }
}

/// Names the single-instance lock and the resident window after Kata's package, so
/// Kata and Kata Dev each run on their own. Run from the build, it's plain "Kata".
fn instance_name() -> HSTRING {
    Package::Current()
        .and_then(|package| package.Id()?.FamilyName())
        .unwrap_or_else(|_| h!("Kata").clone())
}

/// Holds the single-instance lock, or `None` when Kata is already running.
fn claim_single_instance(name: &HSTRING) -> Result<Option<Owned<HANDLE>>> {
    let lock = HSTRING::from(format!(r"Local\{name}"));
    unsafe {
        let lock = Owned::new(CreateMutexW(None, false, &lock)?);
        Ok((GetLastError() != ERROR_ALREADY_EXISTS).then_some(lock))
    }
}

/// Has the Kata already running show its main window.
fn show_running(name: &HSTRING) {
    unsafe {
        let Ok(hwnd) = FindWindowW(CLASS, name) else {
            return;
        };
        // Its window may only come to the front with this process's leave.
        let mut process = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut process));
        let _ = AllowSetForegroundWindow(process);
        let _ = PostMessageW(Some(hwnd), SHOW, WPARAM(0), LPARAM(0));
    }
}

/// Kata's resident window. It's never shown: it receives the hotkey, tray and
/// system messages.
unsafe fn create_window(app: &RefCell<Option<App>>, name: &HSTRING) -> Result<HWND> {
    unsafe {
        let instance = GetModuleHandleW(None)?.into();
        RegisterClassExW(&WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: CLASS,
            ..Default::default()
        });
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            CLASS,
            name,
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
    screen_keys: Option<ScreenKeys>,
    /// Explorer broadcasts it after restarting; the tray icon must be added again.
    taskbar_created: u32,
}

impl App {
    fn new(hwnd: HWND, graphics: Graphics) -> Result<Self> {
        let settings = Settings::load();
        let tray = Tray::new(hwnd, &graphics, &settings.hotkey.label())?;
        if let Err(error) = hotkey::register(hwnd, settings.hotkey) {
            report("hotkey", &error);
        }
        Ok(Self {
            hwnd,
            graphics,
            tray,
            screen_keys: settings.screen_keys.then(|| screen_keys::take(hwnd)),
            taskbar_created: unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) },
        })
    }

    fn handle(&mut self, message: u32, wparam: WPARAM, lparam: LPARAM) -> bool {
        match message {
            WM_HOTKEY | SCREEN_KEY => self.capture(),
            tray::MESSAGE => match tray::event(wparam, lparam) {
                Some(tray::Event::Activate) => self.capture_soon(),
                Some(tray::Event::Menu { x, y }) => match self.tray.menu(x, y) {
                    Some(tray::Command::Capture) => self.capture_soon(),
                    Some(tray::Command::OpenFolder) => export::open_folder(),
                    Some(tray::Command::Settings) => self.show(Page::Settings),
                    Some(tray::Command::Exit) => unsafe { PostQuitMessage(0) },
                    None => {}
                },
                None => return false,
            },
            SHOW => self.show(Page::Home),
            CAPTURE => self.capture_soon(),
            KEYS_CHANGED => {
                let settings = Settings::load();
                self.tray.set_shortcut(&settings.hotkey.label());
                if settings.screen_keys != self.screen_keys.is_some() {
                    self.screen_keys = settings.screen_keys.then(|| screen_keys::take(self.hwnd));
                }
            }
            WM_TIMER if wparam.0 == CAPTURE_TIMER => {
                unsafe {
                    let _ = KillTimer(Some(self.hwnd), CAPTURE_TIMER);
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

    fn show(&self, page: Page) {
        main_window::show(&self.graphics, self.hwnd, page);
    }

    /// Freezes the screen and lets the user choose an area, which is delivered as
    /// it is or opened in the editor, as the settings say.
    fn capture(&mut self) {
        let shot = match kata_capture::capture() {
            Ok(shot) => shot,
            Err(error) => {
                report("capture", &error);
                return;
            }
        };
        let Some((area, image)) = overlay::select(&shot) else {
            return;
        };
        let capture = editor::Capture {
            image,
            area: area.offset(shot.origin.0, shot.origin.1),
            scale: shot.scale_at(area),
        };
        drop(shot);
        let settings = Settings::load();
        if settings.sound {
            sound::play();
        }
        if settings.open_editor {
            editor::open(&self.graphics, self.hwnd, capture);
        } else {
            drop(export::Output::new(self.hwnd, capture.image));
        }
    }

    fn capture_soon(&self) {
        unsafe {
            SetTimer(Some(self.hwnd), CAPTURE_TIMER, CAPTURE_DELAY_MS, None);
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        // Closing the main window may give the hotkey back, so it goes first.
        main_window::close();
        hotkey::unregister(self.hwnd);
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
