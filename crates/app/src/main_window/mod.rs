//! The main window: a button for a new capture and, a page away, the settings.

mod pages;

use std::{
    cell::{Cell, RefCell},
    ffi::OsString,
    os::windows::ffi::OsStringExt,
    path::{Path, PathBuf},
    time::Instant,
};

use kata_ui::{
    Graphics, Surface,
    motion::{self, Animation, Easing},
    paint::Point,
    theme::Palette,
    window::{self, WorkArea},
};
use windows::{
    Win32::{
        Foundation::{ERROR_CANCELLED, HWND, LPARAM, LRESULT, POINT, WPARAM},
        Graphics::Gdi::{InvalidateRect, MONITOR_DEFAULTTONEAREST, MonitorFromPoint, ValidateRect},
        System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree, IBindCtx},
        UI::{
            Controls::WM_MOUSELEAVE,
            Input::KeyboardAndMouse::{
                ReleaseCapture, SetCapture, VIRTUAL_KEY, VK_ESCAPE, VK_SNAPSHOT,
            },
            Shell::{
                FOS_FORCEFILESYSTEM, FOS_PICKFOLDERS, FileOpenDialog, IFileOpenDialog, IShellItem,
                SHCreateItemFromParsingName, SIGDN_FILESYSPATH,
            },
            WindowsAndMessaging::{
                DefWindowProcW, DestroyWindow, GWLP_USERDATA, GetCursorPos, HTCAPTION, HTCLIENT,
                IsIconic, PostMessageW, SIZE_MINIMIZED, SW_RESTORE, SW_SHOW, SetForegroundWindow,
                SetWindowLongPtrW, ShowWindow, WA_INACTIVE, WM_ACTIVATE, WM_APP, WM_CAPTURECHANGED,
                WM_CLOSE, WM_DESTROY, WM_DPICHANGED, WM_DWMCOLORIZATIONCOLORCHANGED, WM_ERASEBKGND,
                WM_GETMINMAXINFO, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
                WM_NCDESTROY, WM_NCHITTEST, WM_PAINT, WM_SETTINGCHANGE, WM_SIZE, WM_SYSCHAR,
                WM_SYSKEYDOWN, WM_SYSKEYUP,
            },
        },
    },
    core::{HSTRING, PCWSTR, Result, w},
};

use crate::{
    export,
    hotkey::{self, Hotkey, Press},
    report,
    settings::Settings,
    sound, startup,
};
pub use pages::Page;
use pages::{Pages, Part, Prompt, TITLE_BAR};

const CLASS: PCWSTR = w!("Kata.Main");
/// The client area a new window gets, in DIPs.
const SIZE: (f32, f32) = (480.0, 408.0);
/// How far a page slides as it comes in, in DIPs.
const SLIDE: f32 = 40.0;
/// Goes to the page in `wparam`.
const GO: u32 = WM_APP + 1;

thread_local! {
    /// The main window, while it's open.
    static OPEN: Cell<Option<HWND>> = const { Cell::new(None) };
}

/// Shows `page` in the main window, opening it if needed. `owner` is Kata's
/// resident window, which holds the capture shortcut.
pub fn show(graphics: &Graphics, owner: HWND, page: Page) {
    if let Some(hwnd) = OPEN.get() {
        unsafe {
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            let _ = SetForegroundWindow(hwnd);
            let _ = PostMessageW(Some(hwnd), GO, WPARAM(page as usize), LPARAM(0));
        }
        return;
    }
    match unsafe { create(graphics, owner, page) } {
        Ok(hwnd) => OPEN.set(Some(hwnd)),
        Err(error) => report("window", &error),
    }
}

pub fn close() {
    if let Some(hwnd) = OPEN.take() {
        let _ = unsafe { DestroyWindow(hwnd) };
    }
}

unsafe fn create(graphics: &Graphics, owner: HWND, page: Page) -> Result<HWND> {
    let settings = Settings::load();
    let folder = export::folder().unwrap_or_default();
    let pages = Pages::new(graphics, &settings, &folder, startup::is_on())?;
    let palette = Palette::current();
    let min = pages.min_size();
    unsafe {
        // Where the user is looking: the monitor with the pointer.
        let mut pointer = POINT::default();
        let _ = GetCursorPos(&mut pointer);
        let work = WorkArea::of(MonitorFromPoint(pointer, MONITOR_DEFAULTTONEAREST));
        let client = (
            (SIZE.0.max(min.0) * work.scale).ceil() as i32,
            (SIZE.1.max(min.1) * work.scale).ceil() as i32,
        );
        let hwnd = window::create(CLASS, window_proc, &work, client, !palette.light)?;
        let size = window::client_size(hwnd);
        let surface = match Surface::new(graphics, hwnd, size) {
            Ok(surface) => surface,
            Err(error) => {
                let _ = DestroyWindow(hwnd);
                return Err(error);
            }
        };
        let mut main = Main {
            hwnd,
            owner,
            graphics: graphics.clone(),
            palette,
            scale: window::scale(hwnd),
            size,
            surface: Some(surface),
            pages,
            settings,
            page,
            transition: Animation::idle(1.0),
            direction: 1.0,
            pressed: None,
            tracking: false,
        };
        // The first frame is ready before the window shows.
        main.layout();
        main.paint();
        let state = Box::into_raw(Box::new(RefCell::new(main)));
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        Ok(hwnd)
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        if let Some(result) = window::frame_proc(hwnd, message, wparam, lparam) {
            return result;
        }
        if message == WM_NCDESTROY {
            release(hwnd);
        }
        window::linked_state::<Main>(hwnd, message, lparam)
            .and_then(|main| main.try_borrow_mut().ok())
            .and_then(|mut main| main.handle(message, wparam, lparam))
            .unwrap_or_else(|| DefWindowProcW(hwnd, message, wparam, lparam))
    }
}

/// Frees the window's state as it goes away.
unsafe fn release(hwnd: HWND) {
    let state = unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) } as *mut RefCell<Main>;
    // A window destroyed from inside its own handler keeps its state, still in use.
    if !state.is_null() && unsafe { (*state).try_borrow_mut() }.is_ok() {
        drop(unsafe { Box::from_raw(state) });
    }
    if OPEN.get() == Some(hwnd) {
        OPEN.set(None);
    }
}

struct Main {
    hwnd: HWND,
    /// Kata's resident window.
    owner: HWND,
    graphics: Graphics,
    palette: Palette,
    /// Pixels per DIP.
    scale: f32,
    /// The client area, in pixels.
    size: (u32, u32),
    /// Made again after the graphics device goes away.
    surface: Option<Surface>,
    pages: Pages,
    /// The settings as last saved.
    settings: Settings,
    page: Page,
    /// How far the page on show has come in, from 0 to 1.
    transition: Animation,
    /// 1 when the page comes in from the right, -1 from the left.
    direction: f32,
    /// The control the left button went down on.
    pressed: Option<Part>,
    /// Whether Windows will say when the pointer leaves.
    tracking: bool,
}

impl Main {
    fn handle(&mut self, message: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        let key = VIRTUAL_KEY(wparam.0 as u16);
        match message {
            WM_PAINT => self.paint(),
            // Every frame covers the whole window: erasing first would only flicker.
            WM_ERASEBKGND => return Some(LRESULT(1)),
            WM_NCHITTEST => {
                let hit = self.hit(window::hit_position(self.hwnd, lparam));
                return Some(LRESULT(hit as isize));
            }
            WM_MOUSEMOVE => self.pointer_moved(self.point(lparam)),
            WM_MOUSELEAVE => {
                self.tracking = false;
                if self.pressed.is_none() && self.pages.hover(self.page, None) {
                    self.invalidate();
                }
            }
            WM_LBUTTONDOWN => self.press(self.point(lparam)),
            WM_LBUTTONUP => self.release(self.point(lparam)),
            // Another window took the pointer.
            WM_CAPTURECHANGED => self.cancel(),
            // While the shortcut waits for a new one, every key goes to it.
            WM_KEYDOWN | WM_SYSKEYDOWN if self.pages.is_listening() => self.shortcut_key(key),
            // Print Screen only reports going up.
            WM_KEYUP | WM_SYSKEYUP if self.pages.is_listening() => {
                if key == VK_SNAPSHOT {
                    self.shortcut_key(key);
                }
            }
            WM_KEYDOWN => return self.key(key).then_some(LRESULT(0)),
            // Alt with a letter would only beep. Alt + Space still opens the window menu.
            WM_SYSCHAR if wparam.0 != usize::from(b' ') => {}
            WM_ACTIVATE => {
                if wparam.0 as u32 & 0xffff == WA_INACTIVE {
                    self.stop_listening();
                    self.invalidate();
                }
                return None;
            }
            WM_SIZE if wparam.0 != SIZE_MINIMIZED as usize => self.resized(),
            WM_DPICHANGED => {
                self.scale = unsafe { window::follow_dpi(self.hwnd, wparam, lparam) };
                self.resized();
            }
            // Keeps the window from getting too small for its pages.
            WM_GETMINMAXINFO => unsafe {
                window::limit_size(lparam, self.pages.min_size(), self.scale);
            },
            WM_SETTINGCHANGE | WM_DWMCOLORIZATIONCOLORCHANGED => {
                self.follow_theme();
                return None;
            }
            GO if wparam.0 == Page::Settings as usize => self.go(Page::Settings),
            GO => self.go(Page::Home),
            // The shortcut works again once the window is gone.
            WM_DESTROY => {
                self.stop_listening();
                return None;
            }
            _ => return None,
        }
        Some(LRESULT(0))
    }

    /// The pointer position carried by mouse messages, in DIPs.
    fn point(&self, lparam: LPARAM) -> Point {
        let (x, y) = window::mouse_position(lparam);
        (x as f32 / self.scale, y as f32 / self.scale)
    }

    /// What's at `(x, y)`, in client pixels, for Windows: the title bar moves the
    /// window, except where there's a button.
    fn hit(&self, (x, y): (i32, i32)) -> u32 {
        let point = (x as f32 / self.scale, y as f32 / self.scale);
        if point.1 < TITLE_BAR && self.pages.part_at(self.page, point).is_none() {
            HTCAPTION
        } else {
            HTCLIENT
        }
    }

    fn pointer_moved(&mut self, point: Point) {
        if !self.tracking {
            self.tracking = window::track_mouse_leave(self.hwnd);
        }
        let changed = match self.pressed {
            Some(part) => self.pages.control(part).drag(point),
            None => self.pages.hover(self.page, Some(point)),
        };
        if changed {
            self.invalidate();
        }
    }

    fn press(&mut self, point: Point) {
        let part = self.pages.part_at(self.page, point);
        // A click anywhere else gives up on a new shortcut.
        if part != Some(Part::Shortcut) {
            self.stop_listening();
        }
        if let Some(part) = part {
            self.pages.control(part).press(point);
            self.pressed = Some(part);
            unsafe { SetCapture(self.hwnd) };
        }
        self.invalidate();
    }

    fn release(&mut self, point: Point) {
        let Some(part) = self.pressed.take() else {
            return;
        };
        let _ = unsafe { ReleaseCapture() };
        if self.pages.control(part).release(point) {
            self.activated(part);
        }
        self.pages.hover(self.page, Some(point));
        self.invalidate();
    }

    /// Drops the press in progress.
    fn cancel(&mut self) {
        if let Some(part) = self.pressed.take() {
            self.pages.control(part).cancel();
            self.invalidate();
        }
    }

    /// Returns whether `key` did something.
    fn key(&mut self, key: VIRTUAL_KEY) -> bool {
        if key != VK_ESCAPE {
            return false;
        }
        if self.pressed.is_some() {
            self.cancel();
            let _ = unsafe { ReleaseCapture() };
        } else if self.page == Page::Settings {
            self.go(Page::Home);
        } else {
            return false;
        }
        true
    }

    /// A control was clicked.
    fn activated(&mut self, part: Part) {
        match part {
            // The window gets out of the way of the capture.
            Part::Capture => unsafe {
                let _ = PostMessageW(Some(self.owner), crate::CAPTURE, WPARAM(0), LPARAM(0));
                let _ = PostMessageW(Some(self.hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
            },
            Part::OpenSettings => self.go(Page::Settings),
            Part::Back => self.go(Page::Home),
            // Pressing the current shortcut mustn't start a capture.
            Part::Shortcut => {
                hotkey::unregister(self.owner);
                self.pages.listen(Prompt::Press);
            }
            Part::OpenEditor | Part::Sound => {
                self.settings.open_editor = self.pages.open_editor();
                self.settings.sound = self.pages.sound();
                self.settings.save();
                if part == Part::Sound && self.settings.sound {
                    sound::play();
                }
            }
            Part::Folder => self.change_folder(),
            Part::Startup => {
                let on = startup::set(self.pages.startup());
                self.pages.set_startup(on);
            }
        }
    }

    /// Takes `key` for the new shortcut.
    fn shortcut_key(&mut self, key: VIRTUAL_KEY) {
        if key == VK_ESCAPE {
            self.stop_listening();
        } else {
            match Hotkey::pressed(key) {
                Press::Modifier => return,
                Press::Alone => self.pages.listen(Prompt::AddModifier),
                Press::Hotkey(hotkey) if hotkey::register(self.owner, hotkey).is_err() => {
                    self.pages.listen(Prompt::InUse);
                }
                Press::Hotkey(hotkey) => {
                    self.settings.hotkey = hotkey;
                    self.settings.save();
                    if let Err(error) = self.pages.set_hotkey(hotkey) {
                        report("window", &error);
                    }
                    self.pages.stop_listening();
                    let changed = crate::HOTKEY_CHANGED;
                    let _ =
                        unsafe { PostMessageW(Some(self.owner), changed, WPARAM(0), LPARAM(0)) };
                }
            }
        }
        self.invalidate();
    }

    /// Stops waiting for a new shortcut and brings back the saved one.
    fn stop_listening(&mut self) {
        if self.pages.is_listening() {
            self.pages.stop_listening();
            if let Err(error) = hotkey::register(self.owner, self.settings.hotkey) {
                report("hotkey", &error);
            }
        }
    }

    fn change_folder(&mut self) {
        let current = export::folder().unwrap_or_default();
        match unsafe { pick_folder(self.hwnd, &current) } {
            Ok(Some(folder)) => {
                if let Err(error) = self.pages.set_folder(&folder) {
                    report("window", &error);
                }
                self.settings.folder = Some(folder);
                self.settings.save();
            }
            Ok(None) => {}
            Err(error) => report("folder", &error),
        }
    }

    /// Slides `page` in.
    fn go(&mut self, page: Page) {
        if page == self.page {
            return;
        }
        self.stop_listening();
        self.pages.hover(self.page, None);
        self.direction = if page > self.page { 1.0 } else { -1.0 };
        self.page = page;
        self.transition = Animation::new(0.0, 1.0, motion::NORMAL, Easing::DECELERATE);
        self.invalidate();
    }

    fn paint(&mut self) {
        let _ = unsafe { ValidateRect(Some(self.hwnd), None) };
        let now = Instant::now();
        let fresh = self.surface.is_none();
        let surface = match self.surface.take() {
            Some(surface) => Ok(surface),
            None => Surface::new(&self.graphics, self.hwnd, self.size),
        };
        match surface.and_then(|mut surface| self.draw(&mut surface, now).map(|()| surface)) {
            Ok(surface) => self.surface = Some(surface),
            Err(error) if fresh => {
                report("window", &error);
                let _ = unsafe { PostMessageW(Some(self.hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) };
            }
            // The graphics device went away (a driver update, say): start over.
            Err(_) => self.invalidate(),
        }
        if self.transition.is_running(now) || self.pages.is_animating(self.page, now) {
            self.invalidate();
        }
    }

    fn draw(&self, surface: &mut Surface, now: Instant) -> Result<()> {
        let shown = self.transition.value(now);
        let offset = (1.0 - shown) * SLIDE * self.direction;
        surface.draw(&self.palette, self.scale, |painter| {
            painter.layer(shown, offset, || self.pages.draw(self.page, painter, now));
        })
    }

    /// Follows the window's new size.
    fn resized(&mut self) {
        self.size = window::client_size(self.hwnd);
        let size = self.size;
        if self
            .surface
            .as_mut()
            .is_some_and(|surface| surface.resize(size).is_err())
        {
            self.surface = None;
        }
        self.layout();
        self.paint();
    }

    fn follow_theme(&mut self) {
        self.palette = Palette::current();
        window::set_dark(self.hwnd, !self.palette.light);
        self.invalidate();
    }

    fn layout(&mut self) {
        let room = (
            self.size.0 as f32 / self.scale,
            self.size.1 as f32 / self.scale,
        );
        let caption_buttons = window::caption_buttons(self.hwnd, self.scale);
        if let Err(error) = self.pages.layout(room, caption_buttons) {
            report("window", &error);
        }
    }

    fn invalidate(&self) {
        let _ = unsafe { InvalidateRect(Some(self.hwnd), None, false) };
    }
}

/// Asks for a folder, starting at `current`. `None` when the user cancels.
unsafe fn pick_folder(owner: HWND, current: &Path) -> Result<Option<PathBuf>> {
    unsafe {
        let dialog: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
        dialog.SetOptions(dialog.GetOptions()? | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM)?;
        // A folder that doesn't exist yet leaves the dialog where it would start.
        let path = HSTRING::from(current);
        if let Ok(folder) =
            SHCreateItemFromParsingName::<_, _, IShellItem>(&path, None::<&IBindCtx>)
        {
            dialog.SetFolder(&folder)?;
        }
        if let Err(error) = dialog.Show(Some(owner)) {
            return if error.code() == ERROR_CANCELLED.to_hresult() {
                Ok(None)
            } else {
                Err(error)
            };
        }
        let path = dialog.GetResult()?.GetDisplayName(SIGDN_FILESYSPATH)?;
        let folder = PathBuf::from(OsString::from_wide(path.as_wide()));
        CoTaskMemFree(Some(path.0.cast()));
        Ok(Some(folder))
    }
}
