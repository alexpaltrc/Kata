use windows::{
    Win32::Graphics::{
        Direct2D::{D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1CreateFactory, ID2D1Factory},
        DirectWrite::{DWRITE_FACTORY_TYPE_SHARED, DWriteCreateFactory, IDWriteFactory},
    },
    core::Result,
};

/// The device-independent factories every window draws with. Cheap to clone.
#[derive(Clone)]
pub struct Graphics {
    pub d2d: ID2D1Factory,
    pub dwrite: IDWriteFactory,
}

impl Graphics {
    /// Must be created and used on the UI thread.
    pub fn new() -> Result<Self> {
        unsafe {
            Ok(Self {
                d2d: D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?,
                dwrite: DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?,
            })
        }
    }
}
