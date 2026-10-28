//! Delivers a capture: on the clipboard at once, and as a PNG file written in the
//! background. Each new version of an edited capture does the same, replacing the
//! last one.

use std::{
    ffi::OsString,
    fs::{self, File},
    io,
    mem::{self, size_of},
    os::windows::ffi::OsStringExt,
    path::{Path, PathBuf},
    ptr,
    sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError},
    thread::{self, JoinHandle},
    time::Duration,
};

use kata_editor::Image;
use windows::{
    Win32::{
        Foundation::{E_POINTER, GENERIC_WRITE, GlobalFree, HANDLE, HWND},
        Graphics::{
            Gdi::{BI_RGB, BITMAPINFOHEADER},
            Imaging::{
                CLSID_WICImagingFactory, GUID_ContainerFormatPng, GUID_WICPixelFormat24bppBGR,
                GUID_WICPixelFormat32bppBGR, IWICImagingFactory, WICBitmapDitherTypeNone,
                WICBitmapEncoderNoCache, WICBitmapPaletteTypeCustom,
            },
        },
        System::{
            Com::{
                CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
                CoTaskMemFree, CoUninitialize,
            },
            DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData},
            Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock},
            Ole::CF_DIB,
            SystemInformation::GetLocalTime,
        },
        UI::{
            Shell::{FOLDERID_Pictures, KF_FLAG_DEFAULT, SHGetKnownFolderPath, ShellExecuteW},
            WindowsAndMessaging::SW_SHOWNORMAL,
        },
    },
    core::{Error, HSTRING, PCWSTR, Result, w},
};

use crate::{channel, report, settings::Settings};

/// Savers still writing captures of closed editors.
static FINISHING: Mutex<Vec<JoinHandle<()>>> = Mutex::new(Vec::new());

/// A capture's file. A thread of its own writes each version handed to it; when
/// versions come faster than they're written, only the newest is.
pub struct Output {
    saver: Option<Saver>,
}

struct Saver {
    slot: Arc<Slot>,
    thread: JoinHandle<()>,
}

/// What the saver is waiting for.
#[derive(Default)]
struct Slot {
    pending: Mutex<Pending>,
    changed: Condvar,
}

#[derive(Default)]
struct Pending {
    next: Option<Image>,
    /// No more versions will come.
    closed: bool,
}

impl Output {
    /// Puts `image` on the clipboard and starts saving it to a new file.
    pub fn new(owner: HWND, image: Image) -> Self {
        copy(owner, &image);
        // Claiming the file here, in capture order, keeps quick captures from
        // competing for the same name.
        let Ok(path) = reserve_file().inspect_err(|error| report("save", error)) else {
            return Self { saver: None };
        };
        let slot = Arc::new(Slot::default());
        lock(&slot).next = Some(image);
        let thread = thread::spawn({
            let slot = Arc::clone(&slot);
            move || write_versions(&path, &slot)
        });
        Self {
            saver: Some(Saver { slot, thread }),
        }
    }

    /// Replaces the file's contents with `image`, in the background.
    pub fn save(&self, image: Image) {
        if let Some(saver) = &self.saver {
            lock(&saver.slot).next = Some(image);
            saver.slot.changed.notify_one();
        }
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        if let Some(Saver { slot, thread }) = self.saver.take() {
            lock(&slot).closed = true;
            slot.changed.notify_one();
            let mut finishing = FINISHING.lock().unwrap_or_else(PoisonError::into_inner);
            finishing.retain(|thread| !thread.is_finished());
            finishing.push(thread);
        }
    }
}

/// Waits until every capture is on disk.
pub fn wait_for_saves() {
    let finishing = mem::take(&mut *FINISHING.lock().unwrap_or_else(PoisonError::into_inner));
    for thread in finishing {
        let _ = thread.join();
    }
}

fn lock(slot: &Slot) -> MutexGuard<'_, Pending> {
    slot.pending.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Writes each version `slot` receives to `path` until it closes.
fn write_versions(path: &Path, slot: &Slot) {
    let mut written = false;
    loop {
        let image = {
            let mut pending = lock(slot);
            loop {
                if let Some(image) = pending.next.take() {
                    break image;
                }
                if pending.closed {
                    // Never leave the empty file behind.
                    if !written {
                        let _ = fs::remove_file(path);
                    }
                    return;
                }
                pending = slot
                    .changed
                    .wait(pending)
                    .unwrap_or_else(PoisonError::into_inner);
            }
        };
        match save_png(path, &image) {
            Ok(()) => written = true,
            Err(error) => report("save", &error),
        }
    }
}

/// Puts `image` on the clipboard.
pub fn copy(owner: HWND, image: &Image) {
    if let Err(error) = copy_to_clipboard(owner, image) {
        report("clipboard", &error);
    }
}

/// Opens the captures folder in File Explorer.
pub fn open_folder() {
    let opened = folder().and_then(|folder| {
        fs::create_dir_all(&folder)?;
        let folder = HSTRING::from(folder.as_path());
        let none = PCWSTR::null();
        unsafe { ShellExecuteW(None, w!("open"), &folder, none, none, SW_SHOWNORMAL) };
        Ok(())
    });
    if let Err(error) = opened {
        report("folder", &error);
    }
}

/// Where captures go: the folder chosen in Settings, or Pictures\Kata (Pictures\Kata
/// Dev for Kata Dev, whose test captures stay apart).
pub fn folder() -> io::Result<PathBuf> {
    if let Some(folder) = Settings::load().folder {
        return Ok(folder);
    }
    unsafe {
        let path = SHGetKnownFolderPath(&FOLDERID_Pictures, KF_FLAG_DEFAULT, None)?;
        let pictures = PathBuf::from(OsString::from_wide(path.as_wide()));
        CoTaskMemFree(Some(path.0.cast()));
        Ok(pictures.join(channel::name()))
    }
}

/// Creates an empty file named after the current time, such as
/// "Kata 2026-10-27 14.32.05.png".
fn reserve_file() -> io::Result<PathBuf> {
    let folder = folder()?;
    fs::create_dir_all(&folder)?;
    let now = unsafe { GetLocalTime() };
    let stem = format!(
        "Kata {:04}-{:02}-{:02} {:02}.{:02}.{:02}",
        now.wYear, now.wMonth, now.wDay, now.wHour, now.wMinute, now.wSecond
    );
    let mut copy = 1;
    loop {
        let name = match copy {
            1 => format!("{stem}.png"),
            _ => format!("{stem} ({copy}).png"),
        };
        let path = folder.join(name);
        match File::create_new(&path) {
            Ok(_) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => copy += 1,
            Err(error) => return Err(error),
        }
    }
}

/// Clipboard bitmaps are DIBs: a header, then the rows from bottom to top.
fn copy_to_clipboard(owner: HWND, image: &Image) -> Result<()> {
    let header_len = size_of::<BITMAPINFOHEADER>();
    let header = BITMAPINFOHEADER {
        biSize: header_len as u32,
        biWidth: image.width() as i32,
        biHeight: image.height() as i32,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        biSizeImage: image.pixels().len() as u32,
        ..Default::default()
    };
    unsafe {
        let memory = GlobalAlloc(GMEM_MOVEABLE, header_len + image.pixels().len())?;
        let bytes = GlobalLock(memory).cast::<u8>();
        if bytes.is_null() {
            let error = Error::from_thread();
            let _ = GlobalFree(Some(memory));
            return Err(error);
        }
        ptr::copy_nonoverlapping(ptr::from_ref(&header).cast(), bytes, header_len);
        let rows = bytes.add(header_len);
        for (i, row) in image
            .pixels()
            .chunks_exact(image.stride())
            .rev()
            .enumerate()
        {
            ptr::copy_nonoverlapping(row.as_ptr(), rows.add(i * row.len()), row.len());
        }
        let _ = GlobalUnlock(memory);

        let placed = open_clipboard(owner).and_then(|()| {
            let placed = EmptyClipboard()
                .and_then(|()| SetClipboardData(CF_DIB.0.into(), Some(HANDLE(memory.0))));
            let _ = CloseClipboard();
            placed
        });
        // The clipboard owns the memory only once it accepted it.
        if placed.is_err() {
            let _ = GlobalFree(Some(memory));
        }
        placed.map(drop)
    }
}

/// Another app may be holding the clipboard for a moment; wait for it briefly.
fn open_clipboard(owner: HWND) -> Result<()> {
    for _ in 0..4 {
        if unsafe { OpenClipboard(Some(owner)) }.is_ok() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(10));
    }
    unsafe { OpenClipboard(Some(owner)) }
}

/// Writes the PNG beside `path`, then swaps it in: `path` never holds half an image.
fn save_png(path: &Path, image: &Image) -> io::Result<()> {
    let partial = path.with_extension("png.partial");
    let saved = unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED)
            .ok()
            .and_then(|()| {
                let encoded = encode_png(&partial, image);
                CoUninitialize();
                encoded
            })
    }
    .map_err(io::Error::from)
    .and_then(|()| fs::rename(&partial, path));
    if saved.is_err() {
        let _ = fs::remove_file(&partial);
    }
    saved
}

unsafe fn encode_png(path: &Path, image: &Image) -> Result<()> {
    let (width, height) = (image.width(), image.height());
    unsafe {
        let wic: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let pixels = wic.CreateBitmapFromMemory(
            width,
            height,
            &GUID_WICPixelFormat32bppBGR,
            image.stride() as u32,
            image.pixels(),
        )?;
        // Captures are opaque: an alpha channel would only make files bigger.
        let opaque = wic.CreateFormatConverter()?;
        opaque.Initialize(
            &pixels,
            &GUID_WICPixelFormat24bppBGR,
            WICBitmapDitherTypeNone,
            None,
            0.0,
            WICBitmapPaletteTypeCustom,
        )?;

        let stream = wic.CreateStream()?;
        stream.InitializeFromFilename(&HSTRING::from(path), GENERIC_WRITE.0)?;
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
        encoder.Commit()
    }
}
