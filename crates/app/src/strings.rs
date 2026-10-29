//! The words Kata shows, in the user's language.

use std::sync::OnceLock;

use kata_editor::BLUR_LEVELS;
use windows::Win32::Globalization::GetUserDefaultUILanguage;

pub struct Strings {
    pub new_capture: &'static str,
    pub open_captures: &'static str,
    pub settings: &'static str,
    pub back: &'static str,
    pub exit: &'static str,
    pub blur: &'static str,
    pub focus: &'static str,
    pub blur_levels: [&'static str; BLUR_LEVELS],
    pub autosave: &'static str,
    pub save: &'static str,
    pub shortcut: &'static str,
    pub open_editor: &'static str,
    pub sound: &'static str,
    pub folder: &'static str,
    pub change: &'static str,
    /// What assistive technologies call the button that changes the folder.
    pub change_folder: &'static str,
    pub screen_keys: &'static str,
    pub start_with_windows: &'static str,
    /// While waiting for a new shortcut.
    pub press_shortcut: &'static str,
    /// When a new shortcut needs a modifier.
    pub add_modifier: &'static str,
    /// When another app already uses a new shortcut.
    pub in_use: &'static str,
    /// The Shift key's name.
    pub shift: &'static str,
    /// What follows a number to make it a percentage.
    percent: &'static str,
}

impl Strings {
    pub fn percent(&self, value: f32) -> String {
        format!("{}{}", (value * 100.0).round(), self.percent)
    }
}

static SPANISH: Strings = Strings {
    new_capture: "Nuevo recorte",
    open_captures: "Abrir capturas",
    settings: "Ajustes",
    back: "Atrás",
    exit: "Salir",
    blur: "Desenfocar",
    focus: "Foco",
    blur_levels: ["Suave", "Medio", "Fuerte"],
    autosave: "Autoguardado",
    save: "Guardar",
    shortcut: "Atajo",
    open_editor: "Abrir el editor al capturar",
    sound: "Sonido",
    folder: "Carpeta de guardado",
    change: "Cambiar",
    change_folder: "Cambiar la carpeta de guardado",
    screen_keys: "Capturar con Impr Pant y Win + Mayús + S",
    start_with_windows: "Iniciar con Windows",
    press_shortcut: "Pulsa el nuevo atajo",
    add_modifier: "Añade Ctrl o Alt",
    in_use: "Ya está en uso",
    shift: "Mayús",
    percent: "\u{a0}%",
};

static ENGLISH: Strings = Strings {
    new_capture: "New capture",
    open_captures: "Open captures",
    settings: "Settings",
    back: "Back",
    exit: "Exit",
    blur: "Blur",
    focus: "Focus",
    blur_levels: ["Soft", "Medium", "Strong"],
    autosave: "Autosave",
    save: "Save",
    shortcut: "Shortcut",
    open_editor: "Open the editor after capturing",
    sound: "Sound",
    folder: "Save folder",
    change: "Change",
    change_folder: "Change the save folder",
    screen_keys: "Capture with Print Screen and Win + Shift + S",
    start_with_windows: "Start with Windows",
    press_shortcut: "Press the new shortcut",
    add_modifier: "Add Ctrl or Alt",
    in_use: "Already in use",
    shift: "Shift",
    percent: "%",
};

pub fn strings() -> &'static Strings {
    static CURRENT: OnceLock<&Strings> = OnceLock::new();
    CURRENT.get_or_init(|| {
        const SPANISH_ID: u16 = 0x0a;
        // The low ten bits of a language id are the language, without the region.
        let language = unsafe { GetUserDefaultUILanguage() } & 0x3ff;
        if language == SPANISH_ID {
            &SPANISH
        } else {
            &ENGLISH
        }
    })
}
