//! The selection overlay. It shows the frozen screen and lets the user choose what
//! to capture: hovering highlights the window or monitor under the pointer, a click
//! takes it, and dragging selects any area. In a monitor's corners, the whole
//! monitor is highlighted. A window partly covered by others shows whole while
//! hovered, and is captured whole.

use std::{
    cell::RefCell,
    mem::{self, size_of},
    ptr,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant},
};

use kata_capture::{Monitor, Screenshot, Window};
use kata_editor::{Image, Rect};
use kata_ui::{
    Graphics,
    motion::{self, Animation, Easing},
    window,
};
use windows::{
    Win32::{
        Foundation::{D2DERR_RECREATE_TARGET, HWND, LPARAM, LRESULT, POINT, WPARAM},
        Graphics::{
            Direct2D::{
                Common::{
                    D2D_RECT_F, D2D_SIZE_U, D2D1_ALPHA_MODE_IGNORE, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
                },
                D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR, D2D1_BITMAP_PROPERTIES,
                D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_HWND_RENDER_TARGET_PROPERTIES,
                D2D1_PRESENT_OPTIONS_NONE, D2D1_RENDER_TARGET_PROPERTIES, D2D1_ROUNDED_RECT,
                ID2D1Bitmap, ID2D1HwndRenderTarget, ID2D1SolidColorBrush,
            },
            DirectWrite::{
                DWRITE_FONT_FEATURE, DWRITE_FONT_FEATURE_TAG_TABULAR_FIGURES,
                DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_TEXT_METRICS, DWRITE_TEXT_RANGE, IDWriteFactory, IDWriteTextFormat,
                IDWriteTextLayout, IDWriteTypography,
            },
            Dwm::{DWMWA_CLOAK, DWMWA_TRANSITIONS_FORCEDISABLED},
            Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
            Gdi::{InvalidateRect, UpdateWindow, ValidateRect},
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, VK_ESCAPE, VK_RETURN},
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetCursorPos,
                GetMessageW, GetSystemMetrics, IDC_CROSS, LoadCursorW, MSG, PostMessageW,
                PostQuitMessage, RegisterClassExW, SM_CXDRAG, SM_CYDRAG, SW_SHOW,
                SetForegroundWindow, ShowWindow, TranslateMessage, WA_INACTIVE, WM_ACTIVATE,
                WM_APP, WM_CLOSE, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_ERASEBKGND, WM_KEYDOWN,
                WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT, WM_RBUTTONUP, WNDCLASSEXW,
                WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
            },
        },
    },
    core::{BOOL, HSTRING, PCWSTR, Result, w},
};
use windows_numerics::Vector2;

use crate::{channel, report};

/// How dark the screen gets outside the selection.
const DIM: f32 = 0.45;
/// The size label's text size at 100 % scale.
const LABEL_SIZE: f32 = 12.0;
const CLASS: PCWSTR = w!("Kata.Overlay");
/// Posted to the overlay when Windows has drawn a covered window whole.
const DRAWN: u32 = WM_APP + 1;
/// How long a click on a covered window waits for Windows to draw it whole, before
/// settling for what the frozen screen shows.
const PATIENCE: Duration = Duration::from_millis(500);
/// How far into a monitor's corner, at 100 % scale, the pointer selects the whole
/// monitor.
const CORNER: f32 = 8.0;

/// Lets the user choose an area of `shot`, in image coordinates, and gives it back
/// with its pixels. `None` when they cancel.
pub fn select(shot: &Screenshot) -> Option<(Rect, Image)> {
    let mut pointer = POINT::default();
    let _ = unsafe { GetCursorPos(&mut pointer) };
    let pointer = (pointer.x - shot.origin.0, pointer.y - shot.origin.1);
    // Direct2D keeps a factory's screen-sized buffers until the factory goes, so the
    // overlay brings its own: Kata gives the memory back once the overlay closes.
    let graphics = match Graphics::new() {
        Ok(graphics) => graphics,
        Err(error) => {
            report("overlay", &error);
            return None;
        }
    };
    let overlay = RefCell::new(Overlay::new(&graphics, shot, pointer));
    let hwnd = match unsafe { create(&overlay) } {
        Ok(hwnd) => hwnd,
        Err(error) => {
            report("overlay", &error);
            return None;
        }
    };
    unsafe { show(hwnd) };
    overlay.borrow_mut().appear(hwnd);
    let chosen = run(&overlay);
    let whole = chosen.and_then(|_| overlay.borrow_mut().chosen_whole(hwnd));
    let _ = unsafe { DestroyWindow(hwnd) };
    let area = chosen?;
    Some((area, whole.or_else(|| shot.image.crop(area))?))
}

unsafe fn create(overlay: &RefCell<Overlay<'_>>) -> Result<HWND> {
    let shot = overlay.borrow().shot;
    unsafe {
        let instance = GetModuleHandleW(None)?.into();
        // Later captures register the class again, which fails harmlessly.
        RegisterClassExW(&WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            hCursor: LoadCursorW(None, IDC_CROSS)?,
            lpszClassName: CLASS,
            ..Default::default()
        });
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            CLASS,
            &HSTRING::from(channel::name()),
            WS_POPUP,
            shot.origin.0,
            shot.origin.1,
            shot.image.bounds().width(),
            shot.image.bounds().height(),
            None,
            None,
            Some(instance),
            Some(ptr::from_ref(overlay).cast()),
        )
    }
}

/// Shows the overlay with its first frame already drawn, so the switch from the live
/// screen to the frozen one can't be seen.
unsafe fn show(hwnd: HWND) {
    window::set_attribute(hwnd, DWMWA_TRANSITIONS_FORCEDISABLED, BOOL::from(true));
    window::set_attribute(hwnd, DWMWA_CLOAK, BOOL::from(true));
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);
    }
    window::set_attribute(hwnd, DWMWA_CLOAK, BOOL::from(false));
    let _ = unsafe { SetForegroundWindow(hwnd) };
}

/// Runs the overlay's message loop until the user has chosen.
fn run(overlay: &RefCell<Overlay<'_>>) -> Option<Rect> {
    let mut message = MSG::default();
    loop {
        if let Phase::Done(chosen) = overlay.borrow().phase {
            return chosen;
        }
        match unsafe { GetMessageW(&mut message, None, 0, 0) }.0 {
            // Kata is quitting: hand the request back to the main loop.
            0 => {
                unsafe { PostQuitMessage(message.wParam.0 as i32) };
                return None;
            }
            -1 => return None,
            _ => unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            },
        }
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let overlay = unsafe { window::linked_state::<Overlay<'_>>(hwnd, message, lparam) };
    overlay
        .and_then(|overlay| overlay.try_borrow_mut().ok())
        .and_then(|mut overlay| overlay.handle(hwnd, message, wparam, lparam))
        .unwrap_or_else(|| unsafe { DefWindowProcW(hwnd, message, wparam, lparam) })
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Choosing,
    /// Fading out after the user cancelled.
    Leaving,
    Done(Option<Rect>),
}

struct Overlay<'a> {
    graphics: &'a Graphics,
    shot: &'a Screenshot,
    canvas: Option<Canvas>,
    phase: Phase,
    /// 0 shows the plain screen; 1, the dimmed screen with the selection lit.
    presence: Animation,
    /// The selection glides from `from` to `to` as `glide` goes from 0 to 1.
    from: Rect,
    to: Rect,
    glide: Animation,
    /// Where the left button went down, while it's held.
    press: Option<(i32, i32)>,
    dragging: bool,
    /// Whether the overlay has been the active window: losing that cancels.
    active: bool,
    /// The window the selection is on, as an index into the shot's windows.
    window: Option<usize>,
    /// Each window drawn whole, for those covered by others, once asked for.
    wholes: Vec<Whole>,
    /// Where the threads drawing windows whole send them, as `(index, whole)`.
    sender: Sender<(usize, Image)>,
    receiver: Receiver<(usize, Image)>,
    /// The covered window shown whole, and how visible: it fades in once Windows
    /// has drawn it, and out when the pointer leaves it.
    reveal: Option<(usize, Animation)>,
}

enum Whole {
    Unasked,
    Asked,
    Drawn(Image),
}

impl<'a> Overlay<'a> {
    fn new(graphics: &'a Graphics, shot: &'a Screenshot, pointer: (i32, i32)) -> Self {
        let (target, window) = target_at(shot, pointer);
        let (sender, receiver) = mpsc::channel();
        Self {
            graphics,
            shot,
            canvas: None,
            phase: Phase::Choosing,
            presence: Animation::idle(0.0),
            from: target,
            to: target,
            glide: Animation::idle(1.0),
            press: None,
            dragging: false,
            active: false,
            window,
            wholes: shot.windows.iter().map(|_| Whole::Unasked).collect(),
            sender,
            receiver,
            reveal: None,
        }
    }

    fn appear(&mut self, hwnd: HWND) {
        self.presence = Animation::new(0.0, 1.0, motion::FAST, Easing::DECELERATE);
        self.aim(hwnd, self.window);
        invalidate(hwnd);
    }

    /// Points the selection at `window`, or at none: a covered window is asked
    /// for whole, and shown whole as soon as it's drawn.
    fn aim(&mut self, hwnd: HWND, window: Option<usize>) {
        self.window = window;
        if let Some((shown, opacity)) = &mut self.reveal
            && Some(*shown) != window
        {
            *opacity = opacity.retarget(0.0, motion::FAST, Easing::ACCELERATE);
        }
        if let Some(index) = window.filter(|&index| self.shot.windows[index].covered) {
            self.ask(hwnd, index);
            self.reveal_drawn();
        }
    }

    /// Has Windows draw window `index` whole, off this thread: an app that doesn't
    /// respond can't stall the overlay.
    fn ask(&mut self, hwnd: HWND, index: usize) {
        if !matches!(self.wholes[index], Whole::Unasked) {
            return;
        }
        self.wholes[index] = Whole::Asked;
        let window: Window = self.shot.windows[index];
        let Some(under) = self.shot.image.crop(window.rect) else {
            return;
        };
        let sender = self.sender.clone();
        let overlay = hwnd.0 as usize;
        thread::spawn(move || {
            if sender.send((index, window.uncover(under))).is_ok() {
                let overlay = HWND(overlay as *mut _);
                let _ = unsafe { PostMessageW(Some(overlay), DRAWN, WPARAM(0), LPARAM(0)) };
            }
        });
    }

    /// Keeps the windows drawn so far, and fades in the aimed one if it's among them.
    fn take_drawn(&mut self, hwnd: HWND) {
        while let Ok((index, whole)) = self.receiver.try_recv() {
            self.wholes[index] = Whole::Drawn(whole);
        }
        self.reveal_drawn();
        invalidate(hwnd);
    }

    fn reveal_drawn(&mut self) {
        let Some(index) = self.window else {
            return;
        };
        if !matches!(self.wholes[index], Whole::Drawn(_)) {
            return;
        }
        let opacity = match self.reveal {
            Some((shown, opacity)) if shown == index => opacity,
            _ => Animation::idle(0.0),
        };
        self.reveal = Some((
            index,
            opacity.retarget(1.0, motion::FAST, Easing::DECELERATE),
        ));
    }

    /// The chosen window whole, if others covered it. Windows has usually drawn it
    /// while the pointer rested on it; if not, it gets a moment more.
    fn chosen_whole(&mut self, hwnd: HWND) -> Option<Image> {
        let index = self
            .window
            .filter(|&index| self.shot.windows[index].covered)?;
        self.ask(hwnd, index);
        let deadline = Instant::now() + PATIENCE;
        loop {
            if let Whole::Drawn(whole) = mem::replace(&mut self.wholes[index], Whole::Asked) {
                return Some(whole);
            }
            let wait = deadline.saturating_duration_since(Instant::now());
            let (drawn, whole) = self.receiver.recv_timeout(wait).ok()?;
            self.wholes[drawn] = Whole::Drawn(whole);
        }
    }

    fn handle(
        &mut self,
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<LRESULT> {
        match message {
            WM_PAINT => self.paint(hwnd),
            // Every frame covers the whole window: erasing first would only flicker.
            WM_ERASEBKGND => return Some(LRESULT(1)),
            WM_MOUSEMOVE => self.pointer_moved(hwnd, window::mouse_position(lparam)),
            WM_LBUTTONDOWN => {
                self.press = Some(window::mouse_position(lparam));
                unsafe { SetCapture(hwnd) };
            }
            WM_LBUTTONUP => {
                let _ = unsafe { ReleaseCapture() };
                if self.press.take().is_some() {
                    self.choose();
                }
            }
            WM_KEYDOWN if wparam.0 == usize::from(VK_RETURN.0) => self.choose(),
            WM_KEYDOWN if wparam.0 == usize::from(VK_ESCAPE.0) => self.leave(hwnd),
            WM_RBUTTONUP | WM_CLOSE => self.leave(hwnd),
            WM_ACTIVATE if wparam.0 & 0xffff == WA_INACTIVE as usize => {
                if self.active {
                    self.cancel();
                }
            }
            WM_ACTIVATE => self.active = true,
            // The screen changed under the overlay: what it shows is no longer true.
            WM_DISPLAYCHANGE => self.cancel(),
            // The overlay must keep covering every monitor exactly, whatever their scale.
            WM_DPICHANGED => {}
            DRAWN => self.take_drawn(hwnd),
            _ => return None,
        }
        Some(LRESULT(0))
    }

    fn pointer_moved(&mut self, hwnd: HWND, point: (i32, i32)) {
        if self.phase != Phase::Choosing {
            return;
        }
        if let Some(start) = self.press {
            if !self.dragging {
                let threshold =
                    unsafe { (GetSystemMetrics(SM_CXDRAG), GetSystemMetrics(SM_CYDRAG)) };
                self.dragging = (point.0 - start.0).abs() > threshold.0
                    || (point.1 - start.1).abs() > threshold.1;
            }
            if self.dragging {
                // Both corners are pixels the user pointed at, so both are included.
                let area = Rect::from_corners(start, point);
                let area = Rect::new(area.left, area.top, area.right + 1, area.bottom + 1);
                if let Some(area) = area.intersect(&self.shot.image.bounds()) {
                    self.from = area;
                    self.to = area;
                    self.glide = Animation::idle(1.0);
                    self.aim(hwnd, None);
                    invalidate(hwnd);
                }
                return;
            }
        }
        let (target, window) = target_at(self.shot, point);
        if target != self.to || window != self.window {
            self.from = self.from.lerp(&self.to, self.glide.value(Instant::now()));
            self.to = target;
            self.glide = Animation::new(0.0, 1.0, motion::FAST, Easing::POINT_TO_POINT);
            self.aim(hwnd, window);
            invalidate(hwnd);
        }
    }

    fn choose(&mut self) {
        if self.phase == Phase::Choosing {
            self.phase = Phase::Done(Some(self.to));
        }
    }

    /// Fades out, then cancels.
    fn leave(&mut self, hwnd: HWND) {
        if self.phase == Phase::Choosing {
            self.phase = Phase::Leaving;
            let shown = self.presence.value(Instant::now());
            self.presence = Animation::new(shown, 0.0, motion::FAST, Easing::ACCELERATE);
            invalidate(hwnd);
        }
    }

    fn cancel(&mut self) {
        if !matches!(self.phase, Phase::Done(_)) {
            self.phase = Phase::Done(None);
        }
    }

    fn paint(&mut self, hwnd: HWND) {
        let _ = unsafe { ValidateRect(Some(hwnd), None) };
        let now = Instant::now();
        let canvas = self.canvas.take();
        let frame = self.frame(now);
        let drawn = match canvas {
            Some(canvas) => Ok(canvas),
            None => Canvas::new(self.graphics, hwnd, &self.shot.image),
        }
        .and_then(|mut canvas| {
            canvas.draw(&self.graphics.dwrite, &frame)?;
            Ok(canvas)
        });
        match drawn {
            Ok(canvas) => self.canvas = Some(canvas),
            // The graphics device went away (a driver update, say): start over.
            Err(error) if error.code() == D2DERR_RECREATE_TARGET => invalidate(hwnd),
            Err(error) => {
                report("overlay", &error);
                self.cancel();
            }
        }
        if self.phase == Phase::Leaving && !self.presence.is_running(now) {
            self.phase = Phase::Done(None);
        } else if self.presence.is_running(now)
            || self.glide.is_running(now)
            || self
                .reveal
                .is_some_and(|(_, opacity)| opacity.is_running(now))
        {
            invalidate(hwnd);
        }
    }

    fn frame(&self, now: Instant) -> Frame<'_> {
        let selection = self.from.lerp(&self.to, self.glide.value(now));
        let presence = self.presence.value(now);
        Frame {
            presence,
            whole: self.reveal.and_then(|(index, opacity)| {
                let Whole::Drawn(image) = &self.wholes[index] else {
                    return None;
                };
                Some(Reveal {
                    index,
                    image,
                    rect: self.shot.windows[index].rect,
                    opacity: opacity.value(now) * presence,
                })
            }),
            selection,
            size: (self.to.width(), self.to.height()),
            area: self
                .shot
                .monitors
                .iter()
                .map(|monitor| monitor.rect)
                .filter(|rect| rect.intersect(&selection).is_some())
                .reduce(|a, b| a.union(&b))
                .unwrap_or_else(|| self.shot.image.bounds()),
            scale: self.shot.scale_at(selection),
        }
    }
}

/// What a click at `(x, y)` captures: the whole monitor from its corners, else
/// the window there, with its index, else its monitor.
fn target_at(shot: &Screenshot, (x, y): (i32, i32)) -> (Rect, Option<usize>) {
    let monitor = shot
        .monitors
        .iter()
        .find(|monitor| monitor.rect.contains(x, y));
    if let Some(monitor) = monitor.filter(|monitor| in_corner(monitor, (x, y))) {
        return (monitor.rect, None);
    }
    if let Some(index) = shot.windows.iter().position(|w| w.rect.contains(x, y)) {
        return (shot.windows[index].rect, Some(index));
    }
    let monitor = monitor.map(|monitor| monitor.rect);
    (monitor.unwrap_or_else(|| shot.image.bounds()), None)
}

/// Whether `(x, y)` lies in one of `monitor`'s corners, where the screen's edges
/// stop the pointer, so a flick of the mouse gets there.
fn in_corner(monitor: &Monitor, (x, y): (i32, i32)) -> bool {
    let reach = (CORNER * monitor.scale).round() as i32;
    let Rect {
        left,
        top,
        right,
        bottom,
    } = monitor.rect;
    let near = |at: i32, start: i32, end: i32| at < start + reach || at >= end - reach;
    near(x, left, right) && near(y, top, bottom)
}

fn invalidate(hwnd: HWND) {
    let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
}

/// Everything one frame shows, in image pixels.
struct Frame<'a> {
    /// 0 shows the plain screen; 1, the full overlay.
    presence: f32,
    /// A covered window shown whole, over the frozen screen.
    whole: Option<Reveal<'a>>,
    selection: Rect,
    /// The size the label reads: where the selection is heading.
    size: (i32, i32),
    /// The monitors the selection touches. Its ring and label stay inside them.
    area: Rect,
    scale: f32,
}

struct Reveal<'a> {
    /// Which window, among the shot's.
    index: usize,
    image: &'a Image,
    rect: Rect,
    opacity: f32,
}

/// The Direct2D resources the overlay draws with.
struct Canvas {
    target: ID2D1HwndRenderTarget,
    bounds: Rect,
    screen: ID2D1Bitmap,
    /// The last window shown whole, by index.
    whole: Option<(usize, ID2D1Bitmap)>,
    brush: ID2D1SolidColorBrush,
    typography: IDWriteTypography,
    /// The label's text format and the display scale it was made for.
    label_format: Option<(f32, IDWriteTextFormat)>,
}

impl Canvas {
    fn new(graphics: &Graphics, hwnd: HWND, image: &Image) -> Result<Self> {
        unsafe {
            // At 96 DPI, Direct2D's units are physical pixels, like the capture's.
            let target = graphics.d2d.CreateHwndRenderTarget(
                &D2D1_RENDER_TARGET_PROPERTIES {
                    pixelFormat: PIXEL_FORMAT,
                    dpiX: 96.0,
                    dpiY: 96.0,
                    ..Default::default()
                },
                &D2D1_HWND_RENDER_TARGET_PROPERTIES {
                    hwnd,
                    pixelSize: D2D_SIZE_U {
                        width: image.width(),
                        height: image.height(),
                    },
                    presentOptions: D2D1_PRESENT_OPTIONS_NONE,
                },
            )?;
            let screen = bitmap(&target, image)?;
            let brush = target.CreateSolidColorBrush(&rgba(0.0, 0.0, 0.0, 0.0), None)?;
            // Tabular figures keep the label from jittering as its numbers change.
            let typography = graphics.dwrite.CreateTypography()?;
            typography.AddFontFeature(DWRITE_FONT_FEATURE {
                nameTag: DWRITE_FONT_FEATURE_TAG_TABULAR_FIGURES,
                parameter: 1,
            })?;
            Ok(Self {
                target,
                bounds: image.bounds(),
                screen,
                whole: None,
                brush,
                typography,
                label_format: None,
            })
        }
    }

    fn draw(&mut self, dwrite: &IDWriteFactory, frame: &Frame) -> Result<()> {
        let presence = frame.presence;
        let selection = frame.selection;
        let ring = frame.scale.round().max(1.0) as i32;
        // The ring hugs the selection from outside, and slides inside at screen edges.
        let clamp = |rect: Rect| rect.intersect(&frame.area).unwrap_or(rect);
        let white = clamp(selection.inflate(ring));
        let shadow = clamp(white.inflate(ring));
        let label = if presence > 0.0 {
            Some(self.label(dwrite, frame, ring as f32)?)
        } else {
            None
        };
        let whole = match &frame.whole {
            Some(reveal) if reveal.opacity > 0.0 => {
                if self
                    .whole
                    .as_ref()
                    .is_none_or(|(index, _)| *index != reveal.index)
                {
                    self.whole = Some((reveal.index, bitmap(&self.target, reveal.image)?));
                }
                self.whole.as_ref().map(|(_, bitmap)| (bitmap, reveal))
            }
            _ => None,
        };
        let (target, brush) = (&self.target, &self.brush);
        unsafe {
            target.BeginDraw();
            target.DrawBitmap(
                &self.screen,
                None,
                1.0,
                D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
                None,
            );
            if let Some((bitmap, reveal)) = whole {
                target.DrawBitmap(
                    bitmap,
                    Some(&rect_f(reveal.rect)),
                    reveal.opacity,
                    D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
                    None,
                );
            }
            if let Some(label) = label {
                brush.SetColor(&rgba(0.0, 0.0, 0.0, DIM * presence));
                for band in around(self.bounds, selection) {
                    target.FillRectangle(&rect_f(band), brush);
                }
                outline(
                    target,
                    brush,
                    shadow,
                    ring as f32,
                    rgba(0.0, 0.0, 0.0, 0.3 * presence),
                );
                outline(
                    target,
                    brush,
                    white,
                    ring as f32,
                    rgba(1.0, 1.0, 1.0, 0.9 * presence),
                );
                label.draw(target, brush, presence);
            }
            target.EndDraw(None, None)
        }
    }

    /// The selection's size, in a pill below it, else above it, else inside.
    fn label(&mut self, dwrite: &IDWriteFactory, frame: &Frame, ring: f32) -> Result<Label> {
        let scale = frame.scale;
        let format = match &self.label_format {
            Some((made_for, format)) if *made_for == scale => format.clone(),
            _ => {
                let format = unsafe {
                    dwrite.CreateTextFormat(
                        w!("Segoe UI Variable Text"),
                        None,
                        DWRITE_FONT_WEIGHT_NORMAL,
                        DWRITE_FONT_STYLE_NORMAL,
                        DWRITE_FONT_STRETCH_NORMAL,
                        LABEL_SIZE * scale,
                        w!(""),
                    )?
                };
                self.label_format = Some((scale, format.clone()));
                format
            }
        };
        let text: Vec<u16> = format!("{} × {}", frame.size.0, frame.size.1)
            .encode_utf16()
            .collect();
        let mut metrics = DWRITE_TEXT_METRICS::default();
        let layout = unsafe {
            let layout = dwrite.CreateTextLayout(&text, &format, f32::MAX, f32::MAX)?;
            let all = DWRITE_TEXT_RANGE {
                startPosition: 0,
                length: text.len() as u32,
            };
            layout.SetTypography(&self.typography, all)?;
            layout.GetMetrics(&mut metrics)?;
            layout
        };

        let (padding_x, padding_y, margin) = (8.0 * scale, 4.0 * scale, 8.0 * scale);
        let gap = margin + ring;
        let width = (metrics.width + 2.0 * padding_x).round();
        let height = (metrics.height + 2.0 * padding_y).round();
        let (selection, area) = (frame.selection, frame.area);
        let center = (selection.left + selection.right) as f32 / 2.0;
        let x = (center - width / 2.0)
            .min(area.right as f32 - margin - width)
            .max(area.left as f32 + margin)
            .round();
        let below = selection.bottom as f32 + gap;
        let above = selection.top as f32 - gap - height;
        let y = if below + height <= area.bottom as f32 - margin {
            below
        } else if above >= area.top as f32 + margin {
            above
        } else {
            selection.bottom as f32 - gap - height
        }
        .round();
        Ok(Label {
            layout,
            pill: D2D_RECT_F {
                left: x,
                top: y,
                right: x + width,
                bottom: y + height,
            },
            text: Vector2::new((x + padding_x).round(), (y + padding_y).round()),
            radius: 4.0 * scale,
        })
    }
}

struct Label {
    layout: IDWriteTextLayout,
    pill: D2D_RECT_F,
    text: Vector2,
    radius: f32,
}

impl Label {
    unsafe fn draw(
        &self,
        target: &ID2D1HwndRenderTarget,
        brush: &ID2D1SolidColorBrush,
        presence: f32,
    ) {
        let rounded = |rect| D2D1_ROUNDED_RECT {
            rect,
            radiusX: self.radius,
            radiusY: self.radius,
        };
        let edge = D2D_RECT_F {
            left: self.pill.left + 0.5,
            top: self.pill.top + 0.5,
            right: self.pill.right - 0.5,
            bottom: self.pill.bottom - 0.5,
        };
        unsafe {
            brush.SetColor(&rgba(0.11, 0.11, 0.11, 0.9 * presence));
            target.FillRoundedRectangle(&rounded(self.pill), brush);
            brush.SetColor(&rgba(1.0, 1.0, 1.0, 0.08 * presence));
            target.DrawRoundedRectangle(&rounded(edge), brush, 1.0, None);
            brush.SetColor(&rgba(1.0, 1.0, 1.0, presence));
            target.DrawTextLayout(self.text, &self.layout, brush, D2D1_DRAW_TEXT_OPTIONS_NONE);
        }
    }
}

/// Kata's images, as Direct2D sees them: opaque BGRA.
const PIXEL_FORMAT: D2D1_PIXEL_FORMAT = D2D1_PIXEL_FORMAT {
    format: DXGI_FORMAT_B8G8R8A8_UNORM,
    alphaMode: D2D1_ALPHA_MODE_IGNORE,
};

/// `image` as a bitmap `target` can draw, pixel for pixel.
fn bitmap(target: &ID2D1HwndRenderTarget, image: &Image) -> Result<ID2D1Bitmap> {
    unsafe {
        target.CreateBitmap(
            D2D_SIZE_U {
                width: image.width(),
                height: image.height(),
            },
            Some(image.pixels().as_ptr().cast()),
            image.stride() as u32,
            &D2D1_BITMAP_PROPERTIES {
                pixelFormat: PIXEL_FORMAT,
                dpiX: 96.0,
                dpiY: 96.0,
            },
        )
    }
}

/// The parts of `bounds` around `hole`, which lies inside it.
fn around(bounds: Rect, hole: Rect) -> impl Iterator<Item = Rect> {
    [
        Rect::new(bounds.left, bounds.top, bounds.right, hole.top),
        Rect::new(bounds.left, hole.bottom, bounds.right, bounds.bottom),
        Rect::new(bounds.left, hole.top, hole.left, hole.bottom),
        Rect::new(hole.right, hole.top, bounds.right, hole.bottom),
    ]
    .into_iter()
    .filter(|band| !band.is_empty())
}

/// Paints a band `thickness` pixels wide just inside the edges of `outer`.
unsafe fn outline(
    target: &ID2D1HwndRenderTarget,
    brush: &ID2D1SolidColorBrush,
    outer: Rect,
    thickness: f32,
    color: D2D1_COLOR_F,
) {
    let half = thickness / 2.0;
    let center = D2D_RECT_F {
        left: outer.left as f32 + half,
        top: outer.top as f32 + half,
        right: outer.right as f32 - half,
        bottom: outer.bottom as f32 - half,
    };
    unsafe {
        brush.SetColor(&color);
        target.DrawRectangle(&center, brush, thickness, None);
    }
}

fn rect_f(rect: Rect) -> D2D_RECT_F {
    D2D_RECT_F {
        left: rect.left as f32,
        top: rect.top as f32,
        right: rect.right as f32,
        bottom: rect.bottom as f32,
    }
}

const fn rgba(r: f32, g: f32, b: f32, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F { r, g, b, a }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corners_reach_grows_with_scale() {
        // At 150 %, 12 pixels into each corner.
        let monitor = Monitor {
            rect: Rect::new(0, 0, 1920, 1080),
            scale: 1.5,
        };
        for corner in [
            (0, 0),
            (1919, 0),
            (0, 1079),
            (1919, 1079),
            (11, 11),
            (1908, 1068),
        ] {
            assert!(in_corner(&monitor, corner), "{corner:?}");
        }
        for elsewhere in [
            (12, 0),
            (0, 12),
            (960, 0),
            (0, 540),
            (960, 540),
            (1907, 1079),
        ] {
            assert!(!in_corner(&monitor, elsewhere), "{elsewhere:?}");
        }
    }

    #[test]
    fn each_monitor_has_its_own_corners() {
        // A second monitor, to the right of a 1920-pixel one, at 100 %.
        let monitor = Monitor {
            rect: Rect::new(1920, 0, 4480, 1440),
            scale: 1.0,
        };
        for corner in [(1920, 0), (1927, 1439), (4479, 7), (4472, 1432)] {
            assert!(in_corner(&monitor, corner), "{corner:?}");
        }
        for elsewhere in [(1928, 0), (1920, 8), (4471, 1439), (3200, 720)] {
            assert!(!in_corner(&monitor, elsewhere), "{elsewhere:?}");
        }
    }
}
