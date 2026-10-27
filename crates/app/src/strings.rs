//! The words Kata shows, in the user's language.

use std::sync::OnceLock;

use windows::Win32::Globalization::GetUserDefaultUILanguage;

pub struct Strings {
    pub new_capture: &'static str,
    pub open_captures: &'static str,
    pub exit: &'static str,
}

static SPANISH: Strings = Strings {
    new_capture: "Nuevo recorte",
    open_captures: "Abrir capturas",
    exit: "Salir",
};

static ENGLISH: Strings = Strings {
    new_capture: "New capture",
    open_captures: "Open captures",
    exit: "Exit",
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
