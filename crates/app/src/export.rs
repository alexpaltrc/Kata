//! Delivers a capture: on the clipboard at once, and as a PNG file written in the
//! background. Each new version of an edited capture does the same, replacing the
//! last one, and the file takes the name the user gives it.

use std::{
    cell::Cell,
    ffi::OsString,
    fs::{self, File},
    io,
    mem::{self, ManuallyDrop, size_of},
    os::windows::ffi::OsStringExt,
    path::{Path, PathBuf},
    ptr,
    rc::Rc,
    slice,
    sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use kata_editor::Image;
use windows::{
    Win32::{
        Foundation::{COLORREF, GlobalFree, HANDLE, HGLOBAL, HWND, POINT, POINTL, SIZE},
        Graphics::Gdi::{
            BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateDIBSection, DIB_RGB_COLORS, DeleteObject,
        },
        Storage::FileSystem::{MOVE_FILE_FLAGS, MoveFileExW},
        System::{
            Com::{
                CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree, DVASPECT_CONTENT, FORMATETC,
                IDataObject, STGMEDIUM, STGMEDIUM_0, TYMED_HGLOBAL,
            },
            DataExchange::{
                CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard,
                RegisterClipboardFormatW, SetClipboardData,
            },
            Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock},
            Ole::{
                CF_DIB, CF_UNICODETEXT, CLIPBOARD_FORMAT, DROPEFFECT, DROPEFFECT_COPY,
                DROPEFFECT_NONE, IDropSource, IDropTarget, IDropTarget_Impl, RegisterDragDrop,
                RevokeDragDrop,
            },
            SystemInformation::GetLocalTime,
            SystemServices::MODIFIERKEYS_FLAGS,
        },
        UI::{
            Shell::{
                BHID_DataObject, CFSTR_DROPDESCRIPTION, CLSID_DragDropHelper, DROPDESCRIPTION,
                DROPIMAGE_INVALID, DROPIMAGE_NOIMAGE, DROPIMAGETYPE, DSH_ALLOWDROPDESCRIPTIONTEXT,
                FOLDERID_Pictures, IDragSourceHelper2, IDropTargetHelper, ILCreateFromPathW,
                ILFree, IShellItem, KF_FLAG_DEFAULT, SHCreateItemFromParsingName, SHDRAGIMAGE,
                SHDoDragDrop, SHGetKnownFolderPath, SHOpenFolderAndSelectItems, ShellExecuteW,
            },
            WindowsAndMessaging::SW_SHOWNORMAL,
        },
    },
    core::{Error, HSTRING, PCWSTR, Ref, Result, implement, w},
};

use crate::{channel, report, settings::Settings};

/// Savers still writing captures of closed editors.
static FINISHING: Mutex<Vec<JoinHandle<()>>> = Mutex::new(Vec::new());

/// A capture's file. A thread of its own writes each version handed to it, and
/// renames the file; when versions come faster than they're written, only the
/// newest is.
pub struct Output {
    saver: Option<Saver>,
}

struct Saver {
    /// When the capture was taken, as its file was first named.
    taken: String,
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
    /// Where the file is, and where it goes when that's elsewhere.
    at: PathBuf,
    to: PathBuf,
    /// A version is on its way to disk.
    writing: bool,
    /// No more versions will come.
    closed: bool,
}

impl Output {
    /// Puts `image` on the clipboard and starts saving it to a new file.
    pub fn new(owner: HWND, image: Image) -> Self {
        copy(owner, &image);
        // Claiming the file here, in capture order, keeps quick captures from
        // competing for the same name.
        let Ok((taken, path)) = reserve_file().inspect_err(|error| report("save", error)) else {
            return Self { saver: None };
        };
        let slot = Arc::new(Slot::default());
        *lock(&slot) = Pending {
            next: Some(image),
            at: path.clone(),
            to: path,
            ..Pending::default()
        };
        let thread = thread::spawn({
            let slot = Arc::clone(&slot);
            move || write_versions(&slot)
        });
        Self {
            saver: Some(Saver {
                taken,
                slot,
                thread,
            }),
        }
    }

    /// When the capture was taken, as its file was first named, such as
    /// "2026-11-03 14.32.05", and the file's name now, without its extension. None
    /// without a file.
    pub fn names(&self) -> Option<(String, String)> {
        let saver = self.saver.as_ref()?;
        let stem = lock(&saver.slot)
            .to
            .file_stem()?
            .to_string_lossy()
            .into_owned();
        Some((saver.taken.clone(), stem))
    }

    /// The name of the folder the file is in, such as "Kata". None without a file.
    pub fn folder_name(&self) -> Option<String> {
        let pending = lock(&self.saver.as_ref()?.slot);
        let folder = pending.at.parent()?.file_name()?;
        Some(folder.to_string_lossy().into_owned())
    }

    /// Replaces the file's contents with `image`, in the background.
    pub fn save(&self, image: Image) {
        if let Some(saver) = &self.saver {
            lock(&saver.slot).next = Some(image);
            saver.slot.changed.notify_one();
        }
    }

    /// Names the file `stem`, or `stem (2)` and so on when another file has that
    /// name, in the background. Returns the name it takes.
    pub fn rename(&self, stem: &str) -> Option<String> {
        let saver = self.saver.as_ref()?;
        let mut pending = lock(&saver.slot);
        let folder = pending.at.parent()?.to_owned();
        let to =
            candidates(&folder, stem).find(|path| same(path, &pending.at) || !path.exists())?;
        let named = to.file_stem()?.to_string_lossy().into_owned();
        pending.to = to;
        saver.slot.changed.notify_one();
        Some(named)
    }

    /// The file, once every version and name handed to it is on disk.
    pub fn file(&self) -> Option<PathBuf> {
        let pending = lock(&self.saver.as_ref()?.slot);
        let settled = pending.next.is_none() && !pending.writing && pending.at == pending.to;
        settled.then(|| pending.at.clone())
    }

    /// Opens the captures folder in File Explorer, with this capture's file selected.
    pub fn show(&self) {
        let Some(saver) = &self.saver else {
            return open_folder();
        };
        let path = lock(&saver.slot).at.clone();
        unsafe {
            let file = ILCreateFromPathW(&HSTRING::from(path.as_path()));
            if file.is_null() {
                return open_folder();
            }
            if let Err(error) = SHOpenFolderAndSelectItems(file, None, 0) {
                report("folder", &error);
            }
            ILFree(Some(file));
        }
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        if let Some(Saver { slot, thread, .. }) = self.saver.take() {
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

/// Renames the file and writes each version `slot` receives until it closes.
fn write_versions(slot: &Slot) {
    let mut written = false;
    loop {
        let mut pending = lock(slot);
        while pending.next.is_none() && pending.at == pending.to && !pending.closed {
            pending = slot
                .changed
                .wait(pending)
                .unwrap_or_else(PoisonError::into_inner);
        }
        let at = pending.at.clone();
        if pending.at != pending.to {
            let to = pending.to.clone();
            drop(pending);
            let moved = move_file(&at, &to);
            let mut pending = lock(slot);
            match moved {
                Ok(()) => pending.at = to,
                // The file keeps its name, unless it was given another meanwhile.
                Err(error) => {
                    report("rename", &error);
                    if pending.to == to {
                        pending.to = at;
                    }
                }
            }
            continue;
        }
        let Some(image) = pending.next.take() else {
            // Never leave the empty file behind.
            if !written {
                let _ = fs::remove_file(&at);
            }
            return;
        };
        pending.writing = true;
        drop(pending);
        let saved = save_png(&at, &image);
        lock(slot).writing = false;
        match saved {
            Ok(()) => written = true,
            Err(error) => report("save", &error),
        }
    }
}

/// Moves the file at `from` to `to`, unless another file is there.
fn move_file(from: &Path, to: &Path) -> Result<()> {
    unsafe {
        MoveFileExW(
            &HSTRING::from(from),
            &HSTRING::from(to),
            MOVE_FILE_FLAGS::default(),
        )
    }
}

/// Whether `a` and `b` name the same file, as Windows compares names: ignoring case.
fn same(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
}

/// What follows the pointer while a file is dragged: its pixels, premultiplied BGRA
/// row by row from the top, how many there are across and down, and where among
/// them the pointer is.
pub struct Picture {
    pub pixels: Vec<u8>,
    pub size: (u32, u32),
    pub pointer: (i32, i32),
}

/// How what's dragged gets to the pointer: `owner`, the window it comes from, flies
/// it there itself, and Windows takes it over once it lands.
pub struct Handover {
    /// When it lands.
    pub landing: Instant,
    /// Whether Windows shows it now, there or anywhere else.
    pub taken: Cell<bool>,
}

/// Drags the file at `path` out of `owner`, as File Explorer would, until it's
/// dropped: into a folder, a document or a message. It's copied wherever it goes,
/// so the capture keeps its file. `picture` follows the pointer, or else Windows'
/// own, as `handover` says. Returns whether it went anywhere.
pub fn drag(owner: HWND, path: &Path, picture: Option<Picture>, handover: Rc<Handover>) -> bool {
    let dragged = unsafe {
        SHCreateItemFromParsingName::<_, _, IShellItem>(&HSTRING::from(path), None)
            .and_then(|item| item.BindToHandler::<_, IDataObject>(None, &BHID_DataObject))
            .and_then(|data| {
                if let Some(picture) = picture
                    && let Err(error) = carry(&data, &picture)
                {
                    report("drag", &error);
                }
                // Windows shows what's dragged only over windows that take drops.
                let shown = show_over(owner, &data, handover)
                    .inspect_err(|error| report("drag", error))
                    .is_ok();
                let dragged =
                    SHDoDragDrop(Some(owner), &data, None::<&IDropSource>, DROPEFFECT_COPY);
                if shown {
                    let _ = RevokeDragDrop(owner);
                }
                dragged
            })
    };
    dragged
        .inspect_err(|error| report("drag", error))
        .is_ok_and(|effect| effect != DROPEFFECT_NONE)
}

/// Makes `picture` follow the pointer while `data` is dragged.
fn carry(data: &IDataObject, picture: &Picture) -> Result<()> {
    let (width, height) = picture.size;
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            // Rows from the top.
            biHeight: -(height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    unsafe {
        let helper: IDragSourceHelper2 =
            CoCreateInstance(&CLSID_DragDropHelper, None, CLSCTX_INPROC_SERVER)?;
        // Windows still says where the file goes, as it does for its own pictures.
        helper.SetFlags(DSH_ALLOWDROPDESCRIPTIONTEXT.0 as u32)?;
        let mut bits = ptr::null_mut();
        let bitmap = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0)?;
        let length = picture
            .pixels
            .len()
            .min(4 * width as usize * height as usize);
        ptr::copy_nonoverlapping(picture.pixels.as_ptr(), bits.cast(), length);
        let image = SHDRAGIMAGE {
            sizeDragImage: SIZE {
                cx: width as i32,
                cy: height as i32,
            },
            ptOffset: POINT {
                x: picture.pointer.0,
                y: picture.pointer.1,
            },
            hbmpDragImage: bitmap,
            // No color stands for transparent: the pixels' own alpha does.
            crColorKey: COLORREF(u32::MAX),
        };
        // Once it takes the bitmap, it frees it.
        helper.InitializeFromBitmap(&image, data).inspect_err(|_| {
            let _ = DeleteObject(bitmap.into());
        })
    }
}

/// Shows `data` over `owner` too, the window it's dragged out of, as it shows
/// everywhere else, though it can't be dropped there: once `owner`'s own picture
/// lands, as `handover` says.
fn show_over(owner: HWND, data: &IDataObject, handover: Rc<Handover>) -> Result<()> {
    unsafe {
        let helper = CoCreateInstance(&CLSID_DragDropHelper, None, CLSCTX_INPROC_SERVER)?;
        let origin: IDropTarget = Origin {
            owner,
            data: data.clone(),
            helper,
            handover,
            shows: Cell::new(false),
        }
        .into();
        RegisterDragDrop(owner, &origin)
    }
}

/// The window a drag comes from, which only shows what's dragged.
#[implement(IDropTarget)]
struct Origin {
    owner: HWND,
    data: IDataObject,
    helper: IDropTargetHelper,
    handover: Rc<Handover>,
    /// Whether Windows shows what's dragged over the window.
    shows: Cell<bool>,
}

impl Origin {
    /// Has Windows show what's dragged at `point`, once the window's own picture
    /// lands there.
    fn show(&self, point: &POINTL) -> Result<()> {
        if Instant::now() < self.handover.landing {
            return Ok(());
        }
        self.shows.set(true);
        self.handover.taken.set(true);
        unsafe {
            self.helper
                .DragEnter(self.owner, &self.data, &on_screen(point), DROPEFFECT_NONE)
        }
    }
}

impl IDropTarget_Impl for Origin_Impl {
    fn DragEnter(
        &self,
        _: Ref<IDataObject>,
        _: MODIFIERKEYS_FLAGS,
        point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> Result<()> {
        // Just the picture, without the sign that says it can't go here.
        let _ = describe(&self.data, DROPIMAGE_NOIMAGE);
        unsafe { *effect = DROPEFFECT_NONE };
        self.show(point)
    }

    fn DragOver(
        &self,
        _: MODIFIERKEYS_FLAGS,
        point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> Result<()> {
        unsafe { *effect = DROPEFFECT_NONE };
        if self.shows.get() {
            unsafe { self.helper.DragOver(&on_screen(point), DROPEFFECT_NONE) }
        } else {
            self.show(point)
        }
    }

    fn DragLeave(&self) -> Result<()> {
        // Elsewhere, Windows shows it and says again where it goes.
        self.handover.taken.set(true);
        let _ = describe(&self.data, DROPIMAGE_INVALID);
        if self.shows.replace(false) {
            unsafe { self.helper.DragLeave() }
        } else {
            Ok(())
        }
    }

    fn Drop(
        &self,
        _: Ref<IDataObject>,
        _: MODIFIERKEYS_FLAGS,
        point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> Result<()> {
        unsafe { *effect = DROPEFFECT_NONE };
        if self.shows.replace(false) {
            unsafe {
                self.helper
                    .Drop(&self.data, &on_screen(point), DROPEFFECT_NONE)
            }
        } else {
            Ok(())
        }
    }
}

fn on_screen(point: &POINTL) -> POINT {
    POINT {
        x: point.x,
        y: point.y,
    }
}

/// Asks Windows to show `image` beside what `data` drags, as drop targets do.
fn describe(data: &IDataObject, image: DROPIMAGETYPE) -> Result<()> {
    let description = DROPDESCRIPTION {
        r#type: image,
        ..Default::default()
    };
    let len = size_of::<DROPDESCRIPTION>();
    let memory = global(len, |bytes| unsafe {
        bytes.copy_from_slice(slice::from_raw_parts(
            ptr::from_ref(&description).cast(),
            len,
        ));
    })?;
    let format = FORMATETC {
        cfFormat: unsafe { RegisterClipboardFormatW(CFSTR_DROPDESCRIPTION) } as u16,
        ptd: ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    };
    let medium = STGMEDIUM {
        tymed: TYMED_HGLOBAL.0 as u32,
        u: STGMEDIUM_0 { hGlobal: memory },
        pUnkForRelease: ManuallyDrop::new(None),
    };
    // Once it takes the memory, it frees it.
    unsafe {
        data.SetData(&format, &medium, true).inspect_err(|_| {
            let _ = GlobalFree(Some(memory));
        })
    }
}

/// Puts `image` on the clipboard.
pub fn copy(owner: HWND, image: &Image) {
    if let Err(error) = copy_to_clipboard(owner, image) {
        report("clipboard", &error);
    }
}

/// Puts `text` on the clipboard.
pub fn copy_text(owner: HWND, text: &str) {
    let wide: Vec<u16> = text.encode_utf16().chain([0]).collect();
    let placed = place_on_clipboard(owner, CF_UNICODETEXT, 2 * wide.len(), |bytes| {
        for (to, unit) in bytes.as_chunks_mut::<2>().0.iter_mut().zip(&wide) {
            *to = unit.to_ne_bytes();
        }
    });
    if let Err(error) = placed {
        report("clipboard", &error);
    }
}

/// The text on the clipboard, if there's any.
pub fn paste_text(owner: HWND) -> Option<String> {
    unsafe {
        open_clipboard(owner).ok()?;
        let text = GetClipboardData(CF_UNICODETEXT.0.into())
            .ok()
            .and_then(|data| {
                let memory = HGLOBAL(data.0);
                let units = GlobalLock(memory).cast::<u16>();
                if units.is_null() {
                    return None;
                }
                let wide = slice::from_raw_parts(units, GlobalSize(memory) / 2);
                let end = wide
                    .iter()
                    .position(|&unit| unit == 0)
                    .unwrap_or(wide.len());
                let text = String::from_utf16_lossy(&wide[..end]);
                let _ = GlobalUnlock(memory);
                Some(text)
            });
        let _ = CloseClipboard();
        text
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

/// Creates an empty file named after the current time, the date first, such as
/// "2026-11-03 14.32.05.png". Returns that time and the file.
fn reserve_file() -> io::Result<(String, PathBuf)> {
    let folder = folder()?;
    fs::create_dir_all(&folder)?;
    let now = unsafe { GetLocalTime() };
    let taken = format!(
        "{:04}-{:02}-{:02} {:02}.{:02}.{:02}",
        now.wYear, now.wMonth, now.wDay, now.wHour, now.wMinute, now.wSecond
    );
    for path in candidates(&folder, &taken) {
        match File::create_new(&path) {
            Ok(_) => return Ok((taken, path)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::ErrorKind::AlreadyExists.into())
}

/// The files in `folder` a capture called `stem` may take, in order: "stem.png",
/// then "stem (2).png" and so on.
fn candidates(folder: &Path, stem: &str) -> impl Iterator<Item = PathBuf> + use<> {
    let (folder, stem) = (folder.to_owned(), stem.to_owned());
    (1..=u32::MAX).map(move |copy| {
        folder.join(match copy {
            1 => format!("{stem}.png"),
            _ => format!("{stem} ({copy}).png"),
        })
    })
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
    let header = unsafe { slice::from_raw_parts(ptr::from_ref(&header).cast(), header_len) };
    let len = header_len + image.pixels().len();
    place_on_clipboard(owner, CF_DIB, len, |bytes| {
        let (head, rows) = bytes.split_at_mut(header_len);
        head.copy_from_slice(header);
        let from = image.pixels().chunks_exact(image.stride()).rev();
        for (row, from) in rows.chunks_exact_mut(image.stride()).zip(from) {
            row.copy_from_slice(from);
        }
    })
}

/// Puts `len` bytes, which `write` fills, on the clipboard as `format`.
fn place_on_clipboard(
    owner: HWND,
    format: CLIPBOARD_FORMAT,
    len: usize,
    write: impl FnOnce(&mut [u8]),
) -> Result<()> {
    let memory = global(len, write)?;
    unsafe {
        let placed = open_clipboard(owner).and_then(|()| {
            let placed = EmptyClipboard()
                .and_then(|()| SetClipboardData(format.0.into(), Some(HANDLE(memory.0))));
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

/// Memory `len` bytes long, which `write` fills, for Windows to take.
fn global(len: usize, write: impl FnOnce(&mut [u8])) -> Result<HGLOBAL> {
    unsafe {
        let memory = GlobalAlloc(GMEM_MOVEABLE, len)?;
        let bytes = GlobalLock(memory).cast::<u8>();
        if bytes.is_null() {
            let error = Error::from_thread();
            let _ = GlobalFree(Some(memory));
            return Err(error);
        }
        write(slice::from_raw_parts_mut(bytes, len));
        let _ = GlobalUnlock(memory);
        Ok(memory)
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
    let saved = fs::write(&partial, image.to_png()).and_then(|()| fs::rename(&partial, path));
    if saved.is_err() {
        let _ = fs::remove_file(&partial);
    }
    saved
}

#[cfg(test)]
mod tests {
    use windows::Win32::{
        Graphics::Imaging::{
            CLSID_WICImagingFactory, GUID_WICPixelFormat32bppBGR, IWICImagingFactory,
            WICBitmapDitherTypeNone, WICBitmapPaletteTypeCustom, WICDecodeMetadataCacheOnDemand,
        },
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
        },
    };

    use super::*;

    /// `png` as Windows' own decoder reads it.
    fn decode(png: &[u8]) -> Result<Image> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
            let wic: IWICImagingFactory =
                CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
            let stream = wic.CreateStream()?;
            stream.InitializeFromMemory(png)?;
            let decoder =
                wic.CreateDecoderFromStream(&stream, ptr::null(), WICDecodeMetadataCacheOnDemand)?;
            let pixels = wic.CreateFormatConverter()?;
            pixels.Initialize(
                &decoder.GetFrame(0)?,
                &GUID_WICPixelFormat32bppBGR,
                WICBitmapDitherTypeNone,
                None,
                0.0,
                WICBitmapPaletteTypeCustom,
            )?;
            let (mut width, mut height) = (0, 0);
            pixels.GetSize(&mut width, &mut height)?;
            let mut bytes = vec![0; (4 * width * height) as usize];
            pixels.CopyPixels(ptr::null(), 4 * width, &mut bytes)?;
            // This format leaves the fourth byte undefined; captures are opaque.
            for pixel in bytes.as_chunks_mut::<4>().0 {
                pixel[3] = 255;
            }
            Ok(Image::new(width, height, bytes))
        }
    }

    /// An image whose pixel at `(x, y)` is `color(x, y)`, as `0xRRGGBB`.
    fn image(width: u32, height: u32, color: impl Fn(u32, u32) -> u32) -> Image {
        let pixels = (0..height)
            .flat_map(|y| (0..width).map(move |x| (x, y)))
            .flat_map(|(x, y)| (0xFF00_0000 | color(x, y)).to_le_bytes())
            .collect();
        Image::new(width, height, pixels)
    }

    fn noise(x: u32, y: u32) -> u32 {
        (x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA77)).wrapping_mul(0xC2B2_AE3D) >> 8
    }

    #[test]
    fn windows_reads_back_every_pixel() {
        let cases = [
            image(1, 1, |_, _| 0x33_6699),
            // Indices one, two, four and eight bits wide, with rows ending mid-byte.
            image(13, 7, |x, y| [0, 0xFF_FFFF][((x + y) % 2) as usize]),
            image(9, 5, |x, y| (x * 3 + y) % 3 * 0x40_4040),
            image(31, 17, |x, y| (x ^ y) % 16 * 0x10_0000),
            image(50, 40, |x, y| (x * 40 + y) % 200 * 0x01_0101),
            image(64, 48, |x, y| (x * 4) << 16 | (y * 5) << 8 | (x + y)),
            // Long repeats near and far, then noise that fills several blocks.
            image(700, 120, |x, y| noise(x % 97, y % 5)),
            image(300, 200, noise),
        ];
        for image in cases {
            let (width, height) = (image.width(), image.height());
            assert!(
                decode(&image.to_png()).unwrap() == image,
                "{width}x{height}"
            );
        }
    }
}
