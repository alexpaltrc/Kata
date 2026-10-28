//! Starting Kata when the user signs in to Windows, through the user's Run key.

use std::{env, ffi::OsString};

use windows::core::{PCWSTR, w};

use crate::{registry, report};

const RUN: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");
const NAME: PCWSTR = w!("Kata");
/// Marks a start at sign-in, which shows no window.
pub const ARGUMENT: &str = "--startup";

pub fn is_on() -> bool {
    registry::read_string(RUN, NAME).is_some()
}

pub fn set(on: bool) {
    if !on {
        registry::delete(RUN, NAME);
        return;
    }
    match env::current_exe() {
        Ok(exe) => {
            let mut command = OsString::from("\"");
            command.push(exe);
            command.push(format!("\" {ARGUMENT}"));
            registry::write_string(RUN, NAME, &command);
        }
        Err(error) => report("startup", &error),
    }
}
