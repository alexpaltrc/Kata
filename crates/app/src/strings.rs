//! The words Kata shows, in the user's language.

use std::sync::OnceLock;

use kata_editor::BLUR_LEVELS;
use windows::Win32::Globalization::GetUserDefaultUILanguage;

pub struct Strings {
    pub new_capture: &'static str,
    pub open_captures: &'static str,
    pub exit: &'static str,
    pub blur: &'static str,
    pub focus: &'static str,
    pub blur_levels: [&'static str; BLUR_LEVELS],
    pub autosave: &'static str,
    pub save: &'static str,
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
    exit: "Salir",
    blur: "Desenfocar",
    focus: "Foco",
    blur_levels: ["Suave", "Medio", "Fuerte"],
    autosave: "Autoguardado",
    save: "Guardar",
    percent: "\u{a0}%",
};

static ENGLISH: Strings = Strings {
    new_capture: "New capture",
    open_captures: "Open captures",
    exit: "Exit",
    blur: "Blur",
    focus: "Focus",
    blur_levels: ["Soft", "Medium", "Strong"],
    autosave: "Autosave",
    save: "Save",
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
