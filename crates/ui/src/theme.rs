//! Following the system's light and dark themes and its accent color.

use std::{ffi::c_void, mem, ptr};

use windows::{
    Win32::{
        Graphics::{
            Direct2D::Common::D2D1_COLOR_F,
            Gdi::{
                COLOR_GRAYTEXT, COLOR_HIGHLIGHT, COLOR_HIGHLIGHTTEXT, COLOR_WINDOW,
                COLOR_WINDOWTEXT, GetSysColor, SYS_COLOR_INDEX,
            },
        },
        System::{
            LibraryLoader::{GetProcAddress, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW},
            Registry::{
                HKEY_CURRENT_USER, REG_ROUTINE_FLAGS, RRF_RT_REG_BINARY, RRF_RT_REG_DWORD,
                RegGetValueW,
            },
        },
        UI::{
            Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW},
            WindowsAndMessaging::{
                SPI_GETHIGHCONTRAST, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
            },
        },
    },
    core::{PCSTR, PCWSTR, w},
};

const PERSONALIZE: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");

/// Whether the taskbar and other shell surfaces use the light theme.
pub fn shell_is_light() -> bool {
    let mut value = 0u32;
    read(
        PERSONALIZE,
        w!("SystemUsesLightTheme"),
        RRF_RT_REG_DWORD,
        &mut value,
    ) && value != 0
}

/// Whether apps use the light theme, which is Windows' default.
pub fn apps_are_light() -> bool {
    let mut value = 1u32;
    !read(
        PERSONALIZE,
        w!("AppsUseLightTheme"),
        RRF_RT_REG_DWORD,
        &mut value,
    ) || value != 0
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

/// Whether one of Windows' contrast themes is on.
pub fn high_contrast() -> bool {
    let mut contrast = HIGHCONTRASTW {
        cbSize: mem::size_of::<HIGHCONTRASTW>() as u32,
        ..Default::default()
    };
    let read = unsafe {
        SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            contrast.cbSize,
            Some(ptr::from_mut(&mut contrast).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    read.is_ok() && contrast.dwFlags.contains(HCF_HIGHCONTRASTON)
}

/// Fluent's colors for the apps' current theme, or the contrast theme's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub light: bool,
    /// Behind everything: transparent, so Mica shows, unless a contrast theme is on.
    pub background: D2D1_COLOR_F,
    pub text: D2D1_COLOR_F,
    pub text_secondary: D2D1_COLOR_F,
    /// Behind subtle controls, such as list items, while hovered and pressed.
    pub subtle_hover: D2D1_COLOR_F,
    pub subtle_pressed: D2D1_COLOR_F,
    /// Solid control surfaces, such as a slider's thumb.
    pub control: D2D1_COLOR_F,
    pub control_stroke: D2D1_COLOR_F,
    /// Borders around content, such as the capture.
    pub card_stroke: D2D1_COLOR_F,
    /// Rails, outlines and other controls that must stand out.
    pub strong: D2D1_COLOR_F,
    /// The inside of switches that are off.
    pub alt: D2D1_COLOR_F,
    /// Cards grouping related content, such as a setting.
    pub card: D2D1_COLOR_F,
    /// Standard buttons and keycaps.
    pub button: D2D1_COLOR_F,
    pub accent: D2D1_COLOR_F,
    pub on_accent: D2D1_COLOR_F,
    pub accent_disabled: D2D1_COLOR_F,
    pub on_accent_disabled: D2D1_COLOR_F,
    /// The inner ring of the keyboard focus visual.
    pub focus_inner: D2D1_COLOR_F,
}

impl Palette {
    /// Reads the theme and the accent color from the system.
    pub fn current() -> Self {
        if high_contrast() {
            return Self::contrast();
        }
        let light = apps_are_light();
        let accent = accent(light);
        if light {
            Self {
                light,
                background: color(0x000000, 0.0),
                text: color(0x000000, 0.8956),
                text_secondary: color(0x000000, 0.6063),
                subtle_hover: color(0x000000, 0.0373),
                subtle_pressed: color(0x000000, 0.0241),
                control: color(0xFFFFFF, 1.0),
                control_stroke: color(0x000000, 0.0578),
                card_stroke: color(0x000000, 0.1622),
                strong: color(0x000000, 0.4458),
                alt: color(0x000000, 0.0241),
                card: color(0xFFFFFF, 0.7),
                button: color(0xFFFFFF, 0.7),
                accent,
                on_accent: color(0xFFFFFF, 1.0),
                accent_disabled: color(0x000000, 0.2169),
                on_accent_disabled: color(0xFFFFFF, 1.0),
                focus_inner: color(0xFFFFFF, 0.7),
            }
        } else {
            Self {
                light,
                background: color(0x000000, 0.0),
                text: color(0xFFFFFF, 1.0),
                text_secondary: color(0xFFFFFF, 0.786),
                subtle_hover: color(0xFFFFFF, 0.0605),
                subtle_pressed: color(0xFFFFFF, 0.0419),
                control: color(0x454545, 1.0),
                control_stroke: color(0xFFFFFF, 0.0698),
                card_stroke: color(0xFFFFFF, 0.093),
                strong: color(0xFFFFFF, 0.5442),
                alt: color(0x000000, 0.1),
                card: color(0xFFFFFF, 0.0512),
                button: color(0xFFFFFF, 0.0605),
                accent,
                on_accent: color(0x000000, 1.0),
                accent_disabled: color(0xFFFFFF, 0.1581),
                on_accent_disabled: color(0xFFFFFF, 0.5302),
                focus_inner: color(0x000000, 0.7),
            }
        }
    }

    /// The contrast theme's system colors, solid: every line and surface stands
    /// out, hovering shades nothing, and the accent becomes the highlight.
    fn contrast() -> Self {
        let system = |index: SYS_COLOR_INDEX| {
            let [r, g, b, _] = unsafe { GetSysColor(index) }.to_le_bytes();
            color(u32::from_be_bytes([0, r, g, b]), 1.0)
        };
        let (window, text) = (system(COLOR_WINDOW), system(COLOR_WINDOWTEXT));
        let none = faded(window, 0.0);
        Self {
            light: 0.2126 * window.r + 0.7152 * window.g + 0.0722 * window.b > 0.5,
            background: window,
            text,
            text_secondary: text,
            subtle_hover: none,
            subtle_pressed: none,
            control: window,
            control_stroke: text,
            card_stroke: text,
            strong: text,
            alt: window,
            card: window,
            button: window,
            accent: system(COLOR_HIGHLIGHT),
            on_accent: system(COLOR_HIGHLIGHTTEXT),
            accent_disabled: system(COLOR_GRAYTEXT),
            on_accent_disabled: window,
            focus_inner: window,
        }
    }
}

/// A color written as `0xRRGGBB`.
pub fn color(rgb: u32, alpha: f32) -> D2D1_COLOR_F {
    let [_, r, g, b] = rgb.to_be_bytes().map(|channel| f32::from(channel) / 255.0);
    D2D1_COLOR_F { r, g, b, a: alpha }
}

/// `color` with its opacity multiplied by `opacity`.
pub const fn faded(color: D2D1_COLOR_F, opacity: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        a: color.a * opacity,
        ..color
    }
}

/// The color `t` of the way from `a` to `b`.
pub fn mix(a: D2D1_COLOR_F, b: D2D1_COLOR_F, t: f32) -> D2D1_COLOR_F {
    let lerp = |from: f32, to: f32| from + (to - from) * t;
    D2D1_COLOR_F {
        r: lerp(a.r, b.r),
        g: lerp(a.g, b.g),
        b: lerp(a.b, b.b),
        a: lerp(a.a, b.a),
    }
}

/// The accent color in the shade Fluent uses on light or dark surfaces.
fn accent(light: bool) -> D2D1_COLOR_F {
    // Eight shades from lightest to darkest, as RGBA bytes. Fluent uses the
    // second-lightest on dark surfaces and the first dark one on light surfaces.
    let mut shades = [0u8; 32];
    let found = read(
        w!(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Accent"),
        w!("AccentPalette"),
        RRF_RT_REG_BINARY,
        &mut shades,
    );
    let shade = if light { 4 } else { 1 };
    match shades.as_chunks::<4>().0.get(shade) {
        Some(&[r, g, b, _]) if found => color(u32::from_be_bytes([0, r, g, b]), 1.0),
        _ if light => color(0x005FB8, 1.0),
        _ => color(0x60CDFF, 1.0),
    }
}

/// Reads a value of the current user's registry into `value`, which must have its
/// exact size. Returns whether it did.
fn read<T: Copy>(key: PCWSTR, name: PCWSTR, kind: REG_ROUTINE_FLAGS, value: &mut T) -> bool {
    let expected = mem::size_of::<T>() as u32;
    let mut size = expected;
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key,
            name,
            kind,
            None,
            Some(ptr::from_mut(value).cast::<c_void>()),
            Some(&mut size),
        )
    };
    result.is_ok() && size == expected
}
