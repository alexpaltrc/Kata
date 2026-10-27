//! Following the system's light and dark themes.

use std::{ffi::c_void, mem, ptr};

use windows::{
    Win32::System::{
        LibraryLoader::{GetProcAddress, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW},
        Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW},
    },
    core::{PCSTR, w},
};

/// Whether the taskbar and other shell surfaces use the light theme.
pub fn shell_is_light() -> bool {
    let mut value = 0u32;
    let mut size = mem::size_of::<u32>() as u32;
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&raw mut value).cast::<c_void>()),
            Some(&mut size),
        )
    };
    result.is_ok() && value != 0
}

/// Lets native context menus follow the system's dark theme.
pub fn follow_system_theme_in_menus() {
    // uxtheme's SetPreferredAppMode (ordinal 135) and FlushMenuThemes (136) are
    // undocumented, but they're the only way for Win32 menus to turn dark.
    type SetPreferredAppMode = unsafe extern "system" fn(i32) -> i32;
    type FlushMenuThemes = unsafe extern "system" fn();
    type Export = unsafe extern "system" fn() -> isize;
    const ALLOW_DARK: i32 = 1;
    unsafe {
        let Ok(uxtheme) = LoadLibraryExW(w!("uxtheme.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32)
        else {
            return;
        };
        let ordinal = |n: usize| GetProcAddress(uxtheme, PCSTR(ptr::without_provenance(n)));
        if let Some(set) = ordinal(135) {
            mem::transmute::<Export, SetPreferredAppMode>(set)(ALLOW_DARK);
        }
        if let Some(flush) = ordinal(136) {
            mem::transmute::<Export, FlushMenuThemes>(flush)();
        }
    }
}
