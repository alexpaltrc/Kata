//! Brings a capture in: a picture dragged from File Explorer over a window, read as
//! Windows reads it. The window hears of each step of the drag as a message, and
//! says whether it takes what's dragged.

use std::{
    cell::Cell,
    path::{Path, PathBuf},
    ptr,
};

use kata_editor::Image;
use windows::{
    Win32::{
        Foundation::{E_INVALIDARG, GENERIC_READ, HWND, LPARAM, POINTL, WPARAM},
        Graphics::Imaging::{
            CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICImagingFactory,
            WICBitmapDitherTypeNone, WICBitmapPaletteTypeCustom, WICDecodeMetadataCacheOnDemand,
        },
        System::{
            Com::{
                CLSCTX_INPROC_SERVER, CoCreateInstance, DVASPECT_CONTENT, FORMATETC, IDataObject,
                TYMED_HGLOBAL,
            },
            Ole::{
                CF_HDROP, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE, IDropTarget,
                IDropTarget_Impl, RegisterDragDrop, ReleaseStgMedium, RevokeDragDrop,
            },
            SystemServices::MODIFIERKEYS_FLAGS,
        },
        UI::{
            Shell::{DragQueryFileW, HDROP},
            WindowsAndMessaging::SendMessageW,
        },
    },
    core::{Error, HSTRING, Ref, Result, implement},
};

use crate::report;

/// The most pixels a picture brought in takes across or down, as the graphics card
/// takes them.
pub const MOST_ACROSS: u32 = 16384;

/// A step of a drag over the window, in its message's `WPARAM`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// A picture came in. The `LPARAM` holds it, a `Box<Image>` the window takes
    /// over by answering nonzero.
    Enter,
    /// It moved, or rests: the window answers nonzero while it would take it there.
    Over,
    /// It left, or the drag was given up.
    Leave,
    /// It was let go: the window answers nonzero if it took it.
    Drop,
}

impl Phase {
    pub fn of(wparam: WPARAM) -> Option<Self> {
        [Self::Enter, Self::Over, Self::Leave, Self::Drop]
            .into_iter()
            .find(|phase| *phase as usize == wparam.0)
    }
}

/// Lets pictures be dropped on `hwnd`, which hears of each step as `message`.
pub fn accept(hwnd: HWND, message: u32) {
    let target: IDropTarget = Target {
        hwnd,
        message,
        taken: Cell::new(false),
    }
    .into();
    if let Err(error) = unsafe { RegisterDragDrop(hwnd, &target) } {
        report("drop", &error);
    }
}

/// No longer lets pictures be dropped on `hwnd`.
pub fn refuse(hwnd: HWND) {
    let _ = unsafe { RevokeDragDrop(hwnd) };
}

#[implement(IDropTarget)]
struct Target {
    hwnd: HWND,
    message: u32,
    /// Whether the window took the picture dragged over it.
    taken: Cell<bool>,
}

impl Target {
    fn tell(&self, phase: Phase, picture: LPARAM) -> bool {
        let answer = unsafe {
            SendMessageW(
                self.hwnd,
                self.message,
                Some(WPARAM(phase as usize)),
                Some(picture),
            )
        };
        answer.0 != 0
    }

    /// Copies what's dragged if the window takes it, as `allowed` lets it be.
    fn answer(&self, phase: Phase, effect: *mut DROPEFFECT) {
        let takes = self.taken.get() && self.tell(phase, LPARAM(0));
        unsafe {
            *effect = if takes && (*effect & DROPEFFECT_COPY) == DROPEFFECT_COPY {
                DROPEFFECT_COPY
            } else {
                DROPEFFECT_NONE
            };
        }
    }
}

impl IDropTarget_Impl for Target_Impl {
    fn DragEnter(
        &self,
        data: Ref<IDataObject>,
        _: MODIFIERKEYS_FLAGS,
        _: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> Result<()> {
        let picture = data.ok().and_then(file).and_then(|path| decode(&path));
        let taken = match picture {
            Ok(image) => {
                let picture = Box::into_raw(Box::new(image));
                let taken = self.tell(Phase::Enter, LPARAM(picture as isize));
                if !taken {
                    drop(unsafe { Box::from_raw(picture) });
                }
                taken
            }
            // Not a picture, or not one Windows reads.
            Err(_) => false,
        };
        self.taken.set(taken);
        self.answer(Phase::Over, effect);
        Ok(())
    }

    fn DragOver(&self, _: MODIFIERKEYS_FLAGS, _: &POINTL, effect: *mut DROPEFFECT) -> Result<()> {
        self.answer(Phase::Over, effect);
        Ok(())
    }

    fn DragLeave(&self) -> Result<()> {
        if self.taken.replace(false) {
            self.tell(Phase::Leave, LPARAM(0));
        }
        Ok(())
    }

    fn Drop(
        &self,
        _: Ref<IDataObject>,
        _: MODIFIERKEYS_FLAGS,
        _: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> Result<()> {
        self.answer(Phase::Drop, effect);
        self.taken.set(false);
        Ok(())
    }
}

/// The one file `data` holds, as File Explorer drags files.
fn file(data: &IDataObject) -> Result<PathBuf> {
    let format = FORMATETC {
        cfFormat: CF_HDROP.0,
        ptd: ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    };
    unsafe {
        let mut medium = data.GetData(&format)?;
        let drop = HDROP(medium.u.hGlobal.0);
        let path = (DragQueryFileW(drop, u32::MAX, None) == 1).then(|| {
            let mut path = vec![0; DragQueryFileW(drop, 0, None) as usize + 1];
            let len = DragQueryFileW(drop, 0, Some(&mut path)) as usize;
            PathBuf::from(String::from_utf16_lossy(&path[..len]))
        });
        ReleaseStgMedium(&mut medium);
        path.ok_or_else(|| E_INVALIDARG.into())
    }
}

/// The picture at `path`, as Windows reads it: its first frame, opaque, over white
/// where it's see-through.
pub fn decode(path: &Path) -> Result<Image> {
    unsafe {
        let wic: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let decoder = wic.CreateDecoderFromFilename(
            &HSTRING::from(path),
            None,
            GENERIC_READ,
            WICDecodeMetadataCacheOnDemand,
        )?;
        let frame = decoder.GetFrame(0)?;
        let (mut width, mut height) = (0, 0);
        frame.GetSize(&mut width, &mut height)?;
        if width == 0 || height == 0 || width > MOST_ACROSS || height > MOST_ACROSS {
            return Err(Error::from(E_INVALIDARG));
        }
        let pixels = wic.CreateFormatConverter()?;
        pixels.Initialize(
            &frame,
            &GUID_WICPixelFormat32bppPBGRA,
            WICBitmapDitherTypeNone,
            None,
            0.0,
            WICBitmapPaletteTypeCustom,
        )?;
        let mut bytes = vec![0; 4 * width as usize * height as usize];
        pixels.CopyPixels(ptr::null(), 4 * width, &mut bytes)?;
        // Premultiplied, so white shows through as much as each pixel lets it.
        for pixel in bytes.as_chunks_mut::<4>().0 {
            let through = 255 - pixel[3];
            for channel in &mut pixel[..3] {
                *channel = channel.saturating_add(through);
            }
            pixel[3] = 255;
        }
        Ok(Image::new(width, height, bytes))
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, time::Instant};

    use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};

    use super::*;

    #[test]
    fn a_picture_reads_back_opaque() {
        let pixels = (0..48 * 32)
            .flat_map(|i: u32| [(i % 256) as u8, (i / 7 % 256) as u8, 40, 255])
            .collect();
        let image = Image::new(48, 32, pixels);
        let path = std::env::temp_dir().join(format!("kata-import-{}.png", std::process::id()));
        fs::write(&path, image.to_png()).unwrap();
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok().unwrap() };
        let read = decode(&path);
        let _ = fs::remove_file(&path);
        assert!(read.unwrap() == image);
    }

    /// How long a screenful takes to read, the editor waiting meanwhile.
    #[test]
    #[ignore = "measures"]
    fn reading_a_screenful() {
        let (width, height) = (2560, 1440);
        let pixels = (0..width * height)
            .flat_map(|i: u32| {
                let (x, y) = (i % width, i / width);
                [
                    (x / 40 % 2 * 200) as u8,
                    (y % 256) as u8,
                    (x ^ y) as u8,
                    255,
                ]
            })
            .collect();
        let image = Image::new(width, height, pixels);
        let path = std::env::temp_dir().join(format!("kata-screen-{}.png", std::process::id()));
        fs::write(&path, image.to_png()).unwrap();
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok().unwrap() };
        let mut times: Vec<_> = (0..9)
            .map(|_| {
                let start = Instant::now();
                decode(&path).unwrap();
                start.elapsed()
            })
            .collect();
        let _ = fs::remove_file(&path);
        times.sort();
        println!("2560x1440: {:?} (median of 9)", times[4]);
    }
}
