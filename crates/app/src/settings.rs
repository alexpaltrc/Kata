//! What Kata remembers between runs, kept in the registry under HKCU\Software\Kata.

use std::{mem::size_of, ptr};

use kata_editor::{BLUR_LEVELS, ZOOMS};
use windows::{
    Win32::System::Registry::{
        HKEY_CURRENT_USER, REG_DWORD, RRF_RT_REG_DWORD, RegGetValueW, RegSetKeyValueW,
    },
    core::{PCWSTR, w},
};

use crate::report;

const KEY: PCWSTR = w!(r"Software\Kata");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// 0 for blur, 1 for focus.
    pub tool: usize,
    pub blur_level: usize,
    /// An index into `ZOOMS`.
    pub zoom_level: usize,
    /// Whether every change to a capture overwrites its file.
    pub autosave: bool,
}

impl Settings {
    pub fn load() -> Self {
        Self {
            tool: read(w!("Tool"), 0).min(1),
            blur_level: read(w!("BlurLevel"), 1).min(BLUR_LEVELS - 1),
            zoom_level: read(w!("ZoomLevel"), 0).min(ZOOMS.len() - 1),
            autosave: read(w!("Autosave"), 1) != 0,
        }
    }

    pub fn save(&self) {
        write(w!("Tool"), self.tool);
        write(w!("BlurLevel"), self.blur_level);
        write(w!("ZoomLevel"), self.zoom_level);
        write(w!("Autosave"), usize::from(self.autosave));
    }
}

fn read(name: PCWSTR, default: usize) -> usize {
    let mut value = 0u32;
    let mut size = size_of::<u32>() as u32;
    let found = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            KEY,
            name,
            RRF_RT_REG_DWORD,
            None,
            Some(ptr::from_mut(&mut value).cast()),
            Some(&mut size),
        )
    };
    if found.is_ok() {
        value as usize
    } else {
        default
    }
}

fn write(name: PCWSTR, value: usize) {
    let value = value as u32;
    let written = unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            KEY,
            name,
            REG_DWORD.0,
            Some(ptr::from_ref(&value).cast()),
            size_of::<u32>() as u32,
        )
    };
    if let Err(error) = written.ok() {
        report("settings", &error);
    }
}
