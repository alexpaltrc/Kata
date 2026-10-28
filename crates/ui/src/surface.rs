//! A window's drawing surface. Direct2D draws into a swap chain that
//! DirectComposition places on the window, so what's left transparent shows the
//! window's backdrop, such as Mica.

use windows::{
    Win32::{
        Foundation::{E_POINTER, HMODULE, HWND},
        Graphics::{
            Direct2D::{
                Common::{D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT},
                D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_TARGET,
                D2D1_BITMAP_PROPERTIES1, D2D1_DEVICE_CONTEXT_OPTIONS_NONE,
                D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE, ID2D1Bitmap1, ID2D1DeviceContext, ID2D1Image,
                ID2D1SolidColorBrush,
            },
            Direct3D::{D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP},
            Direct3D11::{
                D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice,
                ID3D11Device,
            },
            DirectComposition::{
                DCompositionCreateDevice, IDCompositionDevice, IDCompositionTarget,
                IDCompositionVisual,
            },
            Dxgi::{
                Common::{
                    DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_UNKNOWN,
                    DXGI_SAMPLE_DESC,
                },
                CreateDXGIFactory2, DXGI_CREATE_FACTORY_FLAGS, DXGI_PRESENT, DXGI_SCALING_STRETCH,
                DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_CHAIN_FLAG, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIAdapter, IDXGIDevice, IDXGIFactory2,
                IDXGISurface, IDXGISwapChain1,
            },
        },
    },
    core::{Error, Interface, Result},
};

use crate::{Graphics, paint::Painter, theme::Palette};

pub struct Surface {
    context: ID2D1DeviceContext,
    brush: ID2D1SolidColorBrush,
    swap_chain: IDXGISwapChain1,
    size: (u32, u32),
    /// The back buffer as a Direct2D target, until the next resize.
    target: Option<ID2D1Bitmap1>,
    // The window shows the swap chain for as long as these live.
    _composition: IDCompositionDevice,
    _window: IDCompositionTarget,
    _visual: IDCompositionVisual,
}

impl Surface {
    /// Covers `hwnd`, whose client area measures `size` pixels. The window needs
    /// the `WS_EX_NOREDIRECTIONBITMAP` style.
    pub fn new(graphics: &Graphics, hwnd: HWND, size: (u32, u32)) -> Result<Self> {
        let size = at_least_one(size);
        unsafe {
            let dxgi: IDXGIDevice = d3d_device()?.cast()?;
            let factory: IDXGIFactory2 = CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0))?;
            let swap_chain = factory.CreateSwapChainForComposition(
                &dxgi,
                &DXGI_SWAP_CHAIN_DESC1 {
                    Width: size.0,
                    Height: size.1,
                    Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                    BufferCount: 2,
                    Scaling: DXGI_SCALING_STRETCH,
                    SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                    AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
                    ..Default::default()
                },
                None,
            )?;
            let context = graphics
                .d2d
                .CreateDevice(&dxgi)?
                .CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?;
            // ClearType needs an opaque background to blend with.
            context.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            let brush = context.CreateSolidColorBrush(&Default::default(), None)?;

            let composition: IDCompositionDevice = DCompositionCreateDevice(&dxgi)?;
            let window = composition.CreateTargetForHwnd(hwnd, true)?;
            let visual = composition.CreateVisual()?;
            visual.SetContent(&swap_chain)?;
            window.SetRoot(&visual)?;
            composition.Commit()?;
            Ok(Self {
                context,
                brush,
                swap_chain,
                size,
                target: None,
                _composition: composition,
                _window: window,
                _visual: visual,
            })
        }
    }

    /// For creating bitmaps and brushes to draw with.
    pub fn context(&self) -> &ID2D1DeviceContext {
        &self.context
    }

    /// Follows the window's new client size, in pixels.
    pub fn resize(&mut self, size: (u32, u32)) -> Result<()> {
        let size = at_least_one(size);
        if size == self.size {
            return Ok(());
        }
        // The swap chain only resizes once nothing refers to its buffers.
        self.target = None;
        unsafe {
            self.swap_chain.ResizeBuffers(
                0,
                size.0,
                size.1,
                DXGI_FORMAT_UNKNOWN,
                DXGI_SWAP_CHAIN_FLAG(0),
            )?;
        }
        self.size = size;
        Ok(())
    }

    /// Draws a frame on a transparent background, `scale` pixels to the DIP, and
    /// shows it. An error means the graphics device is gone: make a new surface.
    pub fn draw(
        &mut self,
        palette: &Palette,
        scale: f32,
        paint: impl FnOnce(&Painter),
    ) -> Result<()> {
        let target = match &self.target {
            Some(target) => target.clone(),
            None => self.target.insert(self.back_buffer()?).clone(),
        };
        let context = &self.context;
        unsafe {
            context.SetTarget(&target);
            context.BeginDraw();
            context.Clear(None);
            paint(&Painter::new(context, &self.brush, palette, scale));
            let drawn = context.EndDraw(None, None);
            context.SetTarget(None::<&ID2D1Image>);
            drawn?;
            self.swap_chain.Present(1, DXGI_PRESENT(0)).ok()
        }
    }

    fn back_buffer(&self) -> Result<ID2D1Bitmap1> {
        let properties = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
            ..Default::default()
        };
        unsafe {
            let buffer: IDXGISurface = self.swap_chain.GetBuffer(0)?;
            self.context
                .CreateBitmapFromDxgiSurface(&buffer, Some(&raw const properties))
        }
    }
}

/// The graphics card, or Windows' software renderer when there's no usable one.
fn d3d_device() -> Result<ID3D11Device> {
    let create = |driver: D3D_DRIVER_TYPE| {
        let mut device = None;
        unsafe {
            D3D11CreateDevice(
                None::<&IDXGIAdapter>,
                driver,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&raw mut device),
                None,
                None,
            )?;
        }
        device.ok_or_else(|| Error::from(E_POINTER))
    };
    create(D3D_DRIVER_TYPE_HARDWARE).or_else(|_| create(D3D_DRIVER_TYPE_WARP))
}

/// Swap chains can't be empty, but minimized windows are.
fn at_least_one((width, height): (u32, u32)) -> (u32, u32) {
    (width.max(1), height.max(1))
}
