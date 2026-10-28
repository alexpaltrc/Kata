//! Values in the current user's registry.

use std::{
    ffi::{OsStr, OsString},
    mem::size_of,
    os::windows::ffi::{OsStrExt, OsStringExt},
    ptr,
};

use windows::{
    Win32::{
        Foundation::ERROR_FILE_NOT_FOUND,
        System::Registry::{
            HKEY_CURRENT_USER, REG_DWORD, REG_SZ, RRF_RT_REG_DWORD, RRF_RT_REG_SZ,
            RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
        },
    },
    core::PCWSTR,
};

use crate::report;

pub fn read_dword(key: PCWSTR, name: PCWSTR) -> Option<u32> {
    let mut value = 0u32;
    let mut size = size_of::<u32>() as u32;
    let found = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key,
            name,
            RRF_RT_REG_DWORD,
            None,
            Some(ptr::from_mut(&mut value).cast()),
            Some(&mut size),
        )
    };
    found.is_ok().then_some(value)
}

pub fn read_string(key: PCWSTR, name: PCWSTR) -> Option<OsString> {
    let read = |buffer: Option<&mut [u16]>, size: &mut u32| unsafe {
        let data = buffer.map(|buffer| buffer.as_mut_ptr().cast());
        RegGetValueW(
            HKEY_CURRENT_USER,
            key,
            name,
            RRF_RT_REG_SZ,
            None,
            data,
            Some(size),
        )
    };
    // The first read only measures, in bytes and with the terminating null.
    let mut size = 0;
    read(None, &mut size).ok().ok()?;
    let mut buffer = vec![0u16; size as usize / 2];
    read(Some(&mut buffer), &mut size).ok().ok()?;
    buffer.truncate(size as usize / 2);
    while buffer.last() == Some(&0) {
        buffer.pop();
    }
    Some(OsString::from_wide(&buffer))
}

pub fn write_dword(key: PCWSTR, name: PCWSTR, value: u32) {
    let written = unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            key,
            name,
            REG_DWORD.0,
            Some(ptr::from_ref(&value).cast()),
            size_of::<u32>() as u32,
        )
    };
    if let Err(error) = written.ok() {
        report("registry", &error);
    }
}

pub fn write_string(key: PCWSTR, name: PCWSTR, value: &OsStr) {
    let value: Vec<u16> = value.encode_wide().chain([0]).collect();
    let written = unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            key,
            name,
            REG_SZ.0,
            Some(value.as_ptr().cast()),
            (value.len() * size_of::<u16>()) as u32,
        )
    };
    if let Err(error) = written.ok() {
        report("registry", &error);
    }
}

/// Removes a value, if it's there.
pub fn delete(key: PCWSTR, name: PCWSTR) {
    let deleted = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key, name) };
    if deleted.is_err() && deleted != ERROR_FILE_NOT_FOUND {
        report("registry", &deleted.to_hresult());
    }
}
