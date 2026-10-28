//! The notification-area icon and its menu.

use std::mem::size_of;

use kata_ui::{Graphics, icon, theme};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        UI::{
            HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi},
            Shell::{
                NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
                NIM_SETVERSION, NIN_SELECT, NINF_KEY, NOTIFY_ICON_MESSAGE, NOTIFYICON_VERSION_4,
                NOTIFYICONDATAW, NOTIFYICONDATAW_0, Shell_NotifyIconW,
            },
            WindowsAndMessaging::{
                AppendMenuW, CreatePopupMenu, DestroyIcon, DestroyMenu, HICON, MF_SEPARATOR,
                MF_STRING, PostMessageW, SM_CXSMICON, SetForegroundWindow, TPM_BOTTOMALIGN,
                TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenuEx, WM_APP,
                WM_CONTEXTMENU, WM_NULL,
            },
        },
    },
    core::{HSTRING, PCWSTR, Result},
};

use crate::strings::strings;

/// The window message the icon reports its events with.
pub const MESSAGE: u32 = WM_APP + 1;
const ID: u32 = 1;
/// Enter or Space on the focused icon.
const NIN_KEYSELECT: u32 = NIN_SELECT | NINF_KEY;

pub enum Event {
    Activate,
    Menu { x: i32, y: i32 },
}

pub enum Command {
    Capture,
    OpenFolder,
    Settings,
    Exit,
}

pub fn event(wparam: WPARAM, lparam: LPARAM) -> Option<Event> {
    match lparam.0 as u32 & 0xffff {
        NIN_SELECT | NIN_KEYSELECT => Some(Event::Activate),
        WM_CONTEXTMENU => Some(Event::Menu {
            x: (wparam.0 as u16 as i16).into(),
            y: ((wparam.0 >> 16) as u16 as i16).into(),
        }),
        _ => None,
    }
}

pub struct Tray {
    hwnd: HWND,
    icon: HICON,
    /// Its tooltip: the app's name and the shortcut.
    tip: String,
    /// What the icon was drawn for: the shell's theme and the icon size.
    drawn_for: (bool, i32),
}

impl Tray {
    /// `shortcut` is the capture shortcut, as people read it.
    pub fn new(hwnd: HWND, graphics: &Graphics, shortcut: &str) -> Result<Self> {
        let mut tray = Self {
            hwnd,
            icon: HICON::default(),
            tip: tip(shortcut),
            drawn_for: (false, 0),
        };
        tray.redraw(graphics)?;
        // Right after sign-in the taskbar may not exist yet; `restore` adds the icon
        // once it does.
        let _ = tray.add();
        Ok(tray)
    }

    /// Adds the icon again after Explorer restarts.
    pub fn restore(&self) {
        let _ = self.add();
    }

    /// Redraws the icon when the theme or the display scale changed.
    pub fn refresh(&mut self, graphics: &Graphics) {
        if self.wanted() != self.drawn_for && self.redraw(graphics).is_ok() {
            let _ = notify(NIM_MODIFY, &self.data());
        }
    }

    /// Shows the new capture shortcut in the tooltip.
    pub fn set_shortcut(&mut self, shortcut: &str) {
        self.tip = tip(shortcut);
        let _ = notify(NIM_MODIFY, &self.data());
    }

    pub fn menu(&self, x: i32, y: i32) -> Option<Command> {
        const CAPTURE: usize = 1;
        const OPEN_FOLDER: usize = 2;
        const SETTINGS: usize = 3;
        const EXIT: usize = 4;
        let text = strings();
        unsafe {
            let menu = CreatePopupMenu().ok()?;
            let _ = AppendMenuW(menu, MF_STRING, CAPTURE, &HSTRING::from(text.new_capture));
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                OPEN_FOLDER,
                &HSTRING::from(text.open_captures),
            );
            let _ = AppendMenuW(menu, MF_STRING, SETTINGS, &HSTRING::from(text.settings));
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
            let _ = AppendMenuW(menu, MF_STRING, EXIT, &HSTRING::from(text.exit));
            // Without these two calls the menu wouldn't close when clicking elsewhere.
            let _ = SetForegroundWindow(self.hwnd);
            let chosen = TrackPopupMenuEx(
                menu,
                (TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN).0,
                x,
                y,
                self.hwnd,
                None,
            );
            let _ = PostMessageW(Some(self.hwnd), WM_NULL, WPARAM(0), LPARAM(0));
            let _ = DestroyMenu(menu);
            match chosen.0 as usize {
                CAPTURE => Some(Command::Capture),
                OPEN_FOLDER => Some(Command::OpenFolder),
                SETTINGS => Some(Command::Settings),
                EXIT => Some(Command::Exit),
                _ => None,
            }
        }
    }

    fn wanted(&self) -> (bool, i32) {
        let size = unsafe { GetSystemMetricsForDpi(SM_CXSMICON, GetDpiForWindow(self.hwnd)) };
        (theme::shell_is_light(), size)
    }

    fn redraw(&mut self, graphics: &Graphics) -> Result<()> {
        let (light, size) = self.wanted();
        let icon = icon::glyph_icon(graphics, size as u32, light)?;
        self.destroy_icon();
        self.icon = icon;
        self.drawn_for = (light, size);
        Ok(())
    }

    fn add(&self) -> Result<()> {
        notify(NIM_ADD, &self.data())?;
        notify(NIM_SETVERSION, &self.data())
    }

    fn data(&self) -> NOTIFYICONDATAW {
        let mut data = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: ID,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP,
            uCallbackMessage: MESSAGE,
            hIcon: self.icon,
            Anonymous: NOTIFYICONDATAW_0 {
                uVersion: NOTIFYICON_VERSION_4,
            },
            ..Default::default()
        };
        let tip: Vec<u16> = self.tip.encode_utf16().take(data.szTip.len() - 1).collect();
        data.szTip[..tip.len()].copy_from_slice(&tip);
        data
    }

    fn destroy_icon(&self) {
        if !self.icon.is_invalid() {
            let _ = unsafe { DestroyIcon(self.icon) };
        }
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        let _ = notify(NIM_DELETE, &self.data());
        self.destroy_icon();
    }
}

fn tip(shortcut: &str) -> String {
    format!("Kata · {shortcut}")
}

fn notify(message: NOTIFY_ICON_MESSAGE, data: &NOTIFYICONDATAW) -> Result<()> {
    unsafe { Shell_NotifyIconW(message, data) }.ok()
}
