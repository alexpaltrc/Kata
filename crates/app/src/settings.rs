//! What Kata remembers between runs, kept in the registry under HKCU\Software\Kata.

use std::path::PathBuf;

use kata_editor::{BLUR_LEVELS, STEP_SIZES, ZOOMS};
use windows::core::{PCWSTR, w};

use crate::{hotkey::Hotkey, registry};

const KEY: PCWSTR = w!(r"Software\Kata");

/// The app's settings, chosen in its Settings page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub hotkey: Hotkey,
    /// Whether Print Screen and Win + Shift + S start Kata's captures instead of
    /// Windows' own.
    pub screen_keys: bool,
    /// Whether each new capture opens in the editor.
    pub open_editor: bool,
    /// Whether the editor names its tools under their icons.
    pub tool_names: bool,
    /// Whether a sound confirms each capture.
    pub sound: bool,
    /// Where captures go, when it isn't the Pictures folder named after Kata.
    pub folder: Option<PathBuf>,
}

impl Settings {
    pub fn load() -> Self {
        Self {
            hotkey: registry::read_dword(KEY, w!("Hotkey"))
                .and_then(Hotkey::decode)
                .unwrap_or_default(),
            screen_keys: read(w!("ScreenKeys"), 0) != 0,
            open_editor: read(w!("OpenEditor"), 1) != 0,
            tool_names: read(w!("ToolNames"), 0) != 0,
            sound: read(w!("Sound"), 1) != 0,
            folder: registry::read_string(KEY, w!("Folder")).map(PathBuf::from),
        }
    }

    pub fn save(&self) {
        registry::write_dword(KEY, w!("Hotkey"), self.hotkey.encode());
        write(w!("ScreenKeys"), usize::from(self.screen_keys));
        write(w!("OpenEditor"), usize::from(self.open_editor));
        write(w!("ToolNames"), usize::from(self.tool_names));
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
    /// The tool's place in the dock: blur, focus, steps, shapes.
    pub tool: usize,
    pub blur_level: usize,
    /// An index into `ZOOMS`.
    pub zoom_level: usize,
    /// How the focus blurs around it: 0 for not at all, or the blur level plus 1.
    pub around: usize,
    /// An index into `STEP_SIZES`.
    pub step_size: usize,
    /// The shapes' form: rectangle, ellipse, arrow.
    pub form: usize,
    /// The focus's frame: soft, rectangle, ellipse.
    pub frame: usize,
}

impl EditorSettings {
    pub fn load() -> Self {
        Self {
            tool: read(w!("Tool"), 0).min(3),
            blur_level: read(w!("BlurLevel"), 1).min(BLUR_LEVELS - 1),
            zoom_level: read(w!("ZoomLevel"), 0).min(ZOOMS.len() - 1),
            around: read(w!("Around"), 0).min(BLUR_LEVELS),
            step_size: read(w!("StepSize"), 1).min(STEP_SIZES.len() - 1),
            form: read(w!("Form"), 0).min(2),
            frame: read(w!("FocusFrame"), 0).min(2),
        }
    }

    pub fn save(&self) {
        write(w!("Tool"), self.tool);
        write(w!("BlurLevel"), self.blur_level);
        write(w!("ZoomLevel"), self.zoom_level);
        write(w!("Around"), self.around);
        write(w!("StepSize"), self.step_size);
        write(w!("Form"), self.form);
        write(w!("FocusFrame"), self.frame);
    }
}

/// Whether Kata has taken its first capture: until then, it opens with a welcome.
pub fn welcomed() -> bool {
    read(w!("Welcomed"), 0) != 0
}

/// Remembers that Kata has taken its first capture. Returns whether this was it.
pub fn welcome_done() -> bool {
    let first = !welcomed();
    if first {
        write(w!("Welcomed"), 1);
    }
    first
}

/// Forgets the welcome and its hints, so Kata greets as on its first day. Only
/// Kata Dev offers it, to try the welcome again.
pub fn forget_welcome() {
    registry::delete(KEY, w!("Welcomed"));
    for hint in Hint::ALL {
        registry::delete(KEY, hint.value());
    }
}

/// A hint of the welcome, shown until the user does what it suggests, and a few
/// times at most.
#[derive(Clone, Copy)]
pub enum Hint {
    /// In the editor, to name the capture.
    Name,
    /// In the editor, that the capture saves itself, and where.
    Saved,
    /// In the editor, that the capture drags away from its name.
    Drag,
}

impl Hint {
    const ALL: [Self; 3] = [Self::Name, Self::Saved, Self::Drag];

    /// How many times it shows, at most.
    const MOST: usize = 3;

    /// The registry value counting the times it showed.
    fn value(self) -> PCWSTR {
        match self {
            Self::Name => w!("NameHints"),
            Self::Saved => w!("SaveHints"),
            Self::Drag => w!("DragHints"),
        }
    }
}

/// Whether `hint` still shows.
pub fn hinting(hint: Hint) -> bool {
    read(hint.value(), 0) < Hint::MOST
}

/// Remembers that `hint` showed once more, or that the user followed it.
pub fn hinted(hint: Hint, followed: bool) {
    let times = read(hint.value(), 0);
    if times < Hint::MOST {
        write(hint.value(), if followed { Hint::MOST } else { times + 1 });
    }
}

fn read(name: PCWSTR, default: usize) -> usize {
    registry::read_dword(KEY, name).map_or(default, |value| value as usize)
}

fn write(name: PCWSTR, value: usize) {
    registry::write_dword(KEY, name, value as u32);
}
