//! The global shortcut that starts a capture.

use windows::{
    Win32::{
        Foundation::HWND,
        UI::Input::KeyboardAndMouse::{
            GetKeyNameTextW, HOT_KEY_MODIFIERS, MAPVK_VK_TO_CHAR, MAPVK_VK_TO_VSC_EX, MOD_ALT,
            MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, MapVirtualKeyW, RegisterHotKey,
            UnregisterHotKey, VIRTUAL_KEY, VK_CONTROL, VK_F1, VK_F24, VK_LCONTROL, VK_LMENU,
            VK_LSHIFT, VK_LWIN, VK_MENU, VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT,
            VK_SNAPSHOT,
        },
    },
    core::Result,
};

use kata_ui::window::held;

use crate::{channel, strings::strings};

const ID: i32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hotkey {
    /// Ctrl, Alt, Shift and Win, as `MOD_` flags.
    pub modifiers: HOT_KEY_MODIFIERS,
    /// A virtual-key code.
    pub key: u32,
}

/// What a key press makes of a new shortcut.
#[derive(Debug, PartialEq, Eq)]
pub enum Press {
    /// A modifier on its own: the shortcut isn't complete yet.
    Modifier,
    /// A key that needs Ctrl or Alt with it.
    Alone,
    Hotkey(Hotkey),
}

impl Default for Hotkey {
    /// Ctrl + Shift + S, or Ctrl + Shift + D in Kata Dev, so both can run at once.
    fn default() -> Self {
        let key = if channel::is_dev() { b'D' } else { b'S' };
        Self {
            modifiers: MOD_CONTROL | MOD_SHIFT,
            key: u32::from(key),
        }
    }
}

impl Hotkey {
    /// The shortcut `key` makes with the modifiers held down along with it.
    pub fn pressed(key: VIRTUAL_KEY) -> Press {
        let flags = [
            (held(VK_CONTROL), MOD_CONTROL),
            (held(VK_MENU), MOD_ALT),
            (held(VK_SHIFT), MOD_SHIFT),
            (held(VK_LWIN) || held(VK_RWIN), MOD_WIN),
        ];
        let modifiers = flags
            .into_iter()
            .filter(|&(held, _)| held)
            .fold(HOT_KEY_MODIFIERS(0), |all, (_, flag)| all | flag);
        let hotkey = Self {
            modifiers,
            key: u32::from(key.0),
        };
        if is_modifier(hotkey.key) {
            Press::Modifier
        } else if hotkey.is_valid() {
            Press::Hotkey(hotkey)
        } else {
            Press::Alone
        }
    }

    /// The shortcut as a registry value: the modifiers above the key.
    pub fn encode(self) -> u32 {
        self.modifiers.0 << 16 | self.key
    }

    /// Reads a registry value, which must hold a shortcut Kata would accept.
    pub fn decode(value: u32) -> Option<Self> {
        let hotkey = Self {
            modifiers: HOT_KEY_MODIFIERS(value >> 16),
            key: value & 0xffff,
        };
        hotkey.is_valid().then_some(hotkey)
    }

    /// Whether it's a key with Ctrl or Alt, or Print Screen, which may go alone.
    fn is_valid(self) -> bool {
        let all = (MOD_CONTROL | MOD_ALT | MOD_SHIFT | MOD_WIN).0;
        let anchored = self.modifiers.0 & (MOD_CONTROL | MOD_ALT).0 != 0;
        self.modifiers.0 & !all == 0
            && (1..0xff).contains(&self.key)
            && !is_modifier(self.key)
            && (anchored || self.key == u32::from(VK_SNAPSHOT.0))
    }

    /// The keys to press, as they read on the keyboard.
    pub fn keys(self) -> Vec<String> {
        let names = strings();
        let modifiers = [
            (MOD_WIN, "Win"),
            (MOD_CONTROL, "Ctrl"),
            (MOD_ALT, "Alt"),
            (MOD_SHIFT, names.shift),
        ];
        modifiers
            .into_iter()
            .filter(|&(flag, _)| self.modifiers.0 & flag.0 != 0)
            .map(|(_, name)| name.to_owned())
            .chain([key_name(self.key)])
            .collect()
    }

    /// The shortcut as people read it, such as "Ctrl + Shift + S".
    pub fn label(self) -> String {
        self.keys().join(" + ")
    }
}

fn is_modifier(key: u32) -> bool {
    [
        VK_SHIFT,
        VK_CONTROL,
        VK_MENU,
        VK_LSHIFT,
        VK_RSHIFT,
        VK_LCONTROL,
        VK_RCONTROL,
        VK_LMENU,
        VK_RMENU,
        VK_LWIN,
        VK_RWIN,
    ]
    .iter()
    .any(|modifier| u32::from(modifier.0) == key)
}

/// A key's name in the keyboard's layout and the system's language.
fn key_name(key: u32) -> String {
    let (first, last) = (u32::from(VK_F1.0), u32::from(VK_F24.0));
    if (first..=last).contains(&key) {
        return format!("F{}", key - first + 1);
    }
    // Keys that type a character are named after it. Dead keys set the top bit.
    let character = unsafe { MapVirtualKeyW(key, MAPVK_VK_TO_CHAR) };
    if character & 0x8000_0000 == 0
        && let Some(character) = char::from_u32(character)
        && !character.is_whitespace()
        && !character.is_control()
    {
        return character.to_uppercase().collect();
    }
    let scan = unsafe { MapVirtualKeyW(key, MAPVK_VK_TO_VSC_EX) };
    let extended = scan >> 8 != 0;
    let lparam = ((scan & 0xff) << 16 | u32::from(extended) << 24) as i32;
    let mut name = [0u16; 64];
    let length = unsafe { GetKeyNameTextW(lparam, &mut name) };
    let name = String::from_utf16_lossy(&name[..length.max(0) as usize]);
    if name.is_empty() {
        format!("0x{key:02X}")
    } else {
        title_case(&name)
    }
}

/// "PAGE UP" and "page up" become "Page Up".
fn title_case(name: &str) -> String {
    name.split(' ')
        .map(|word| {
            let mut letters = word.chars();
            letters.next().map_or_else(String::new, |first| {
                first
                    .to_uppercase()
                    .chain(letters.flat_map(char::to_lowercase))
                    .collect()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Has `owner` receive `WM_HOTKEY` when `hotkey` is pressed. Fails when another app
/// already uses it.
pub fn register(owner: HWND, hotkey: Hotkey) -> Result<()> {
    unsafe { RegisterHotKey(Some(owner), ID, hotkey.modifiers | MOD_NOREPEAT, hotkey.key) }
}

pub fn unregister(owner: HWND) {
    let _ = unsafe { UnregisterHotKey(Some(owner), ID) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoding_round_trips() {
        // Tests run outside any package, as Kata.
        let encoded = Hotkey::default().encode();
        assert_eq!(encoded, 0x0006_0053);
        assert_eq!(Hotkey::decode(encoded), Some(Hotkey::default()));
    }

    #[test]
    fn needs_ctrl_or_alt() {
        let with = |modifiers: HOT_KEY_MODIFIERS, key: u8| Hotkey {
            modifiers,
            key: u32::from(key),
        };
        assert!(with(MOD_ALT, b'A').is_valid());
        assert!(with(MOD_CONTROL | MOD_WIN, b'A').is_valid());
        assert!(!with(MOD_SHIFT, b'A').is_valid());
        assert!(!with(MOD_WIN, b'A').is_valid());
        assert!(!with(HOT_KEY_MODIFIERS(0), b'A').is_valid());
        assert!(with(HOT_KEY_MODIFIERS(0), VK_SNAPSHOT.0 as u8).is_valid());
    }

    #[test]
    fn rejects_what_kata_never_saves() {
        assert_eq!(Hotkey::decode(0), None);
        // A modifier as the key.
        assert_eq!(Hotkey::decode(0x0002_0000 | u32::from(VK_SHIFT.0)), None);
        // Flags that aren't modifiers, such as MOD_NOREPEAT.
        assert_eq!(Hotkey::decode(0x4002_0053), None);
    }

    #[test]
    fn title_cases_key_names() {
        assert_eq!(title_case("PAGE UP"), "Page Up");
        assert_eq!(title_case("espacio"), "Espacio");
    }
}
