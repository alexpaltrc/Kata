//! What Kata remembers between runs, kept in the registry under HKCU\Software\Kata.

use std::path::PathBuf;

use kata_editor::STEP_SIZES;
use windows::core::{PCWSTR, w};

use crate::{channel, hotkey::Hotkey, registry};

const KEY: PCWSTR = w!(r"Software\Kata");

/// The app's settings, chosen in its Settings page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub hotkey: Hotkey,
    /// Whether Print Screen and Win + Shift + S start Kata's captures instead of
    /// Windows' own.
    pub screen_keys: bool,
    /// What follows each new capture.
    pub after: After,
    /// Whether the editor names its tools under their icons.
    pub tool_names: bool,
    /// Whether a sound confirms each capture.
    pub sound: bool,
    /// Where captures go, when it isn't the Pictures folder named after Kata.
    pub folder: Option<PathBuf>,
    /// The color of the shapes and the steps: one of `INKS`, or past them, a hue of
    /// the user's own.
    pub ink: usize,
    /// That hue, in degrees.
    pub hue: usize,
}

/// What follows a capture, which is always copied and saved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum After {
    /// Nothing more: it's ready to paste.
    Nothing,
    /// The capture shows as a card in a corner for a moment, and a click on it
    /// opens it in the editor.
    Notice,
    /// It opens in the editor.
    Editor,
}

impl After {
    pub const ALL: [Self; 3] = [Self::Nothing, Self::Notice, Self::Editor];

    /// As it was last chosen. Before there was a notice, the editor opened or it
    /// didn't: that choice carries over.
    fn load() -> Self {
        let editor = read(w!("OpenEditor"), 1) != 0;
        let before = if editor { Self::Editor } else { Self::Nothing };
        Self::ALL[read(w!("AfterCapture"), before as usize).min(2)]
    }
}

/// The colors the shapes and the steps come in for free, as `0xRRGGBB`: Kata's
/// orange first, and four as vivid and as easy to read white on.
pub const INKS: [u32; 5] = [0xEC_5A22, 0xE0_3A44, 0xD0_3E96, 0x2F_6FEB, 0x1C_9A5A];

/// The color of `hue`, in degrees, as Kata's ink wears it: vivid, and never so light
/// that the white of a step's number gets lost on it.
pub fn tinted(hue: f32) -> u32 {
    const SATURATION: f32 = 0.86;
    const LIGHTEST: f32 = 0.30;
    let sector = (hue.rem_euclid(360.0)) / 60.0;
    let share = 1.0 - (sector % 2.0 - 1.0).abs();
    let (red, green, blue) = match sector as u32 {
        0 => (1.0, share, 0.0),
        1 => (share, 1.0, 0.0),
        2 => (0.0, 1.0, share),
        3 => (0.0, share, 1.0),
        4 => (share, 0.0, 1.0),
        _ => (1.0, 0.0, share),
    };
    let tone = |channel: f32, value: f32| value * (1.0 - SATURATION * (1.0 - channel));
    let light = |value: f32| {
        let linear = |channel: f32| tone(channel, value).powf(2.2);
        0.2126 * linear(red) + 0.7152 * linear(green) + 0.0722 * linear(blue)
    };
    let mut value = 0.94_f32;
    while light(value) > LIGHTEST {
        value *= 0.98;
    }
    let byte = |channel: f32| (tone(channel, value) * 255.0).round() as u32;
    byte(red) << 16 | byte(green) << 8 | byte(blue)
}

/// The color the shapes and the steps are drawn in, as `0xRRGGBB`.
pub fn ink() -> u32 {
    let settings = Settings::load();
    match INKS.get(settings.ink) {
        Some(&ink) => ink,
        None => tinted(settings.hue as f32),
    }
}

/// What Premium adds, each of which can be tried a few times for free.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Perk {
    Shortcut,
    Color,
    Joining,
}

impl Perk {
    const ALL: [Self; 3] = [Self::Shortcut, Self::Color, Self::Joining];
    /// How many times each can be tried without Premium.
    const TRIES: usize = 3;

    fn value(self) -> PCWSTR {
        match self {
            Self::Shortcut => w!("TriedShortcut"),
            Self::Color => w!("TriedColor"),
            Self::Joining => w!("TriedJoining"),
        }
    }

    /// How many free tries are left, where they count: `None` with Premium, and
    /// where Premium doesn't show yet, as everything is free there.
    pub fn tries(self) -> Option<usize> {
        let counted = channel::is_dev() && !premium();
        counted.then(|| Self::TRIES - read(self.value(), 0).min(Self::TRIES))
    }

    /// Whether it can be used now.
    pub fn allowed(self) -> bool {
        self.tries() != Some(0)
    }

    /// Counts a use against the free tries, where they count.
    pub fn used(self) {
        if self.tries().is_some() {
            write(self.value(), read(self.value(), 0) + 1);
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        Self {
            hotkey: registry::read_dword(KEY, w!("Hotkey"))
                .and_then(Hotkey::decode)
                .unwrap_or_default(),
            screen_keys: read(w!("ScreenKeys"), 0) != 0,
            after: After::load(),
            tool_names: read(w!("ToolNames"), 0) != 0,
            sound: read(w!("Sound"), 1) != 0,
            folder: registry::read_string(KEY, w!("Folder")).map(PathBuf::from),
            ink: read(w!("Ink"), 0).min(INKS.len()),
            hue: read(w!("InkHue"), 265) % 360,
        }
    }

    pub fn save(&self) {
        registry::write_dword(KEY, w!("Hotkey"), self.hotkey.encode());
        write(w!("ScreenKeys"), usize::from(self.screen_keys));
        write(w!("AfterCapture"), self.after as usize);
        write(w!("ToolNames"), usize::from(self.tool_names));
        write(w!("Sound"), usize::from(self.sound));
        write(w!("Ink"), self.ink);
        write(w!("InkHue"), self.hue);
        match &self.folder {
            Some(folder) => registry::write_string(KEY, w!("Folder"), folder.as_os_str()),
            None => registry::delete(KEY, w!("Folder")),
        }
    }
}

/// The editor's settings, which it remembers as they change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorSettings {
    /// The tool's place in the dock: shapes, steps, focus, blur.
    pub tool: usize,
    /// An index into `STEP_SIZES`.
    pub step_size: usize,
    /// The shapes' form: rectangle, ellipse, arrow.
    pub form: usize,
}

impl EditorSettings {
    pub fn load() -> Self {
        Self {
            tool: read(w!("Tool"), 0).min(3),
            step_size: read(w!("StepSize"), 1).min(STEP_SIZES.len() - 1),
            form: read(w!("Form"), 0).min(2),
        }
    }

    pub fn save(&self) {
        write(w!("Tool"), self.tool);
        write(w!("StepSize"), self.step_size);
        write(w!("Form"), self.form);
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

/// Whether Premium is the user's. Until the Store sells it, only Kata Dev can say
/// so, to try how it shows.
pub fn premium() -> bool {
    channel::is_dev() && read(w!("Premium"), 0) != 0
}

/// What the codes that give Premium come to, digested: the codes themselves are
/// kept nowhere.
const CODES: [u64; 1] = [0x35c4_19aa_8d2f_e325];
/// How many times over a code is digested.
const ROUNDS: usize = 4096;

/// Whether `code` is one that gives Premium.
pub fn redeems(code: &str) -> bool {
    CODES.contains(&digest(code))
}

/// FNV-1a over `code`, again and again.
fn digest(code: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for _ in 0..ROUNDS {
        for byte in code.bytes() {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
        }
    }
    hash
}

/// Remembers whether Premium is the user's.
pub fn set_premium(premium: bool) {
    write(w!("Premium"), usize::from(premium));
}

/// Forgets the welcome, its hints and Premium, so Kata greets as on its first day.
/// Only Kata Dev offers it, to try the welcome again.
pub fn forget_welcome() {
    registry::delete(KEY, w!("Premium"));
    // A color of the user's own goes with Premium.
    if read(w!("Ink"), 0) >= INKS.len() {
        registry::delete(KEY, w!("Ink"));
    }
    for perk in Perk::ALL {
        registry::delete(KEY, perk.value());
    }
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

#[cfg(test)]
mod tests {
    use super::redeems;

    #[test]
    fn what_is_not_a_code_gives_nothing() {
        assert!(!redeems(""));
        assert!(!redeems("KATA"));
    }
}
