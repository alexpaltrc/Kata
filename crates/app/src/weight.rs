//! How much lighter a capture's PNG is with Kata than with Windows' own encoder,
//! measured on the capture itself, in the background, for the welcome's Ready.

use std::{ptr, thread};

use kata_editor::Image;
use windows::{
    Win32::{
        Foundation::{E_POINTER, HWND, LPARAM, WPARAM},
        Graphics::Imaging::{
            CLSID_WICImagingFactory, GUID_ContainerFormatPng, GUID_WICPixelFormat24bppBGR,
            GUID_WICPixelFormat32bppBGR, IWICImagingFactory, WICBitmapDitherTypeNone,
            WICBitmapEncoderNoCache, WICBitmapPaletteTypeCustom,
        },
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
            CoUninitialize, STATFLAG_NONAME, StructuredStorage::CreateStreamOnHGlobal,
        },
        UI::WindowsAndMessaging::PostMessageW,
    },
    core::{Error, Result},
};

use crate::{WEIGHED, report};

/// Weighs `image` both ways, then posts `WEIGHED` to `owner`, its `WPARAM` holding
/// the share Kata saves as the bits of an `f32`: 0.3 for a file 30 % lighter.
pub fn compare(owner: HWND, image: Image) {
    // A window handle is only a number, the same on any thread.
    let owner = owner.0 as isize;
    thread::spawn(move || {
        let windows = match unsafe { windows_png(&image) } {
            Ok(size) => size as f32,
            Err(error) => return report("weight", &error),
        };
        let kata = image.to_png().len() as f32;
        let saved = 1.0 - kata / windows;
        let owner = HWND(owner as *mut _);
        let saved = WPARAM(saved.to_bits() as usize);
        let _ = unsafe { PostMessageW(Some(owner), WEIGHED, saved, LPARAM(0)) };
    });
}

/// The size in bytes of `image` as a PNG from Windows' encoder, as Kata used to save
/// it: 24 bits a pixel, with its default settings.
unsafe fn windows_png(image: &Image) -> Result<u64> {
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        let size = encode(image);
        CoUninitialize();
        size
    }
}

unsafe fn encode(image: &Image) -> Result<u64> {
    unsafe {
        let wic: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let (width, height) = (image.width(), image.height());
        let pixels = wic.CreateBitmapFromMemory(
            width,
            height,
            &GUID_WICPixelFormat32bppBGR,
            image.stride() as u32,
            image.pixels(),
        )?;
        let opaque = wic.CreateFormatConverter()?;
        opaque.Initialize(
            &pixels,
            &GUID_WICPixelFormat24bppBGR,
            WICBitmapDitherTypeNone,
            None,
            0.0,
            WICBitmapPaletteTypeCustom,
        )?;
        let stream = CreateStreamOnHGlobal(Default::default(), true)?;
        let encoder = wic.CreateEncoder(&GUID_ContainerFormatPng, ptr::null())?;
        encoder.Initialize(&stream, WICBitmapEncoderNoCache)?;
        let (mut frame, mut options) = (None, None);
        encoder.CreateNewFrame(&mut frame, &mut options)?;
        let frame = frame.ok_or(Error::from(E_POINTER))?;
        frame.Initialize(options.as_ref())?;
        frame.SetSize(width, height)?;
        let mut format = GUID_WICPixelFormat24bppBGR;
        frame.SetPixelFormat(&mut format)?;
        frame.WriteSource(&opaque, ptr::null())?;
        frame.Commit()?;
        encoder.Commit()?;
        let mut stat = Default::default();
        stream.Stat(&mut stat, STATFLAG_NONAME)?;
        Ok(stat.cbSize)
    }
}
