//! Which copy of Kata is running: Kata, or Kata Dev, where every change is tried
//! first. Kata Dev says so wherever it shows its name or icon, and keeps its own
//! shortcut and folder, so the two run side by side without getting mixed up.

use std::sync::OnceLock;

use windows::ApplicationModel::Package;

/// Whether this is Kata Dev, whose package name ends in ".Dev". Run from the
/// build, it's Kata.
pub fn is_dev() -> bool {
    static DEV: OnceLock<bool> = OnceLock::new();
    *DEV.get_or_init(|| {
        Package::Current()
            .and_then(|package| package.Id()?.Name())
            .is_ok_and(|name| name.to_string_lossy().ends_with(".Dev"))
    })
}

/// The name Kata shows in its windows and tray: "Kata" or "Kata Dev".
pub fn name() -> &'static str {
    if is_dev() { "Kata Dev" } else { "Kata" }
}
