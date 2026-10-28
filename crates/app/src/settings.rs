//! What Kata remembers between runs, kept in the registry under HKCU\Software\Kata.

use std::path::PathBuf;

use kata_editor::{BLUR_LEVELS, ZOOMS};
use windows::core::{PCWSTR, w};

use crate::{hotkey::Hotkey, registry};

const KEY: PCWSTR = w!(r"Software\Kata");

/// The app's settings, chosen in its Settings page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub hotkey: Hotkey,
    /// Whether each new capture opens in the editor.
    pub open_editor: bool,
    /// Whether a sound confirms each capture.
    pub sound: bool,
    /// Where captures go, when it isn't Pictures\Kata.
    pub folder: Option<PathBuf>,
}

impl Settings {
    pub fn load() -> Self {
        Self {
            hotkey: registry::read_dword(KEY, w!("Hotkey"))
                .and_then(Hotkey::decode)
                .unwrap_or(Hotkey::DEFAULT),
            open_editor: read(w!("OpenEditor"), 1) != 0,
            sound: read(w!("Sound"), 1) != 0,
            folder: registry::read_string(KEY, w!("Folder")).map(PathBuf::from),
        }
    }

    pub fn save(&self) {
        registry::write_dword(KEY, w!("Hotkey"), self.hotkey.encode());
        write(w!("OpenEditor"), usize::from(self.open_editor));
        write(w!("Sound"), usize::from(self.sound));
        match &self.folder {
            Some(folder) => registry::write_string(KEY, w!("Folder"), folder.as_os_str()),
            None => registry::delete(KEY, w!("Folder")),
        }
    }
}

/// The editor's settings, which it remembers as they change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorSettings {
    /// 0 for blur, 1 for focus.
    pub tool: usize,
    pub blur_level: usize,
    /// An index into `ZOOMS`.
    pub zoom_level: usize,
    /// Whether every change to a capture overwrites its file.
    pub autosave: bool,
}

impl EditorSettings {
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
    registry::read_dword(KEY, name).map_or(default, |value| value as usize)
}

fn write(name: PCWSTR, value: usize) {
    registry::write_dword(KEY, name, value as u32);
}
