//! The editor: a window showing a capture, where the user blurs parts of it, puts one
//! in focus, numbers the steps to follow and points things out with shapes. Each
//! change reaches the clipboard and, with autosave on, the capture's file.

mod canvas;
mod icons;
mod toolbar;

use std::{
    cell::{Cell, RefCell},
    mem,
    time::Instant,
};

use kata_editor::{
    Blur, Document, Edits, FOCUS_CORNER, Figure, Focus, Form, Image, Rect, Renderer, STEP_SIZES,
    Shape, Spot, Step, ZOOMS,
};
use kata_ui::{
    Graphics,
    access::{self, Access},
    controls::Action,
    motion::{self, Animation, Easing},
    paint::{Area, Fonts, Label, Painter, Point},
    theme::{Palette, faded},
    window::{self, WorkArea},
};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
        Graphics::{
            Direct2D::ID2D1Bitmap1,
            Gdi::{
                InvalidateRect, MONITOR_DEFAULTTONEAREST, MonitorFromRect, ScreenToClient,
                ValidateRect,
            },
        },
        UI::{
            Controls::{WM_MOUSEHOVER, WM_MOUSELEAVE},
            HiDpi::GetSystemMetricsForDpi,
            Input::KeyboardAndMouse::{
                ReleaseCapture, SetCapture, VIRTUAL_KEY, VK_0, VK_1, VK_2, VK_3, VK_4, VK_BACK,
                VK_C, VK_CONTROL, VK_DELETE, VK_ESCAPE, VK_NUMPAD0, VK_NUMPAD1, VK_NUMPAD2,
                VK_NUMPAD3, VK_NUMPAD4, VK_S, VK_SHIFT, VK_TAB, VK_Y, VK_Z,
            },
            WindowsAndMessaging::{
                DefWindowProcW, DestroyWindow, GWLP_USERDATA, GetCursorPos, HTCAPTION, HTCLIENT,
                IDC_ARROW, IDC_CROSS, IDC_SIZEALL, IDC_SIZENESW, IDC_SIZENWSE, LoadCursorW,
                PostMessageW, SIZE_MINIMIZED, SM_CXDRAG, SM_CYDRAG, SW_SHOW, SetCursor,
                SetForegroundWindow, SetWindowLongPtrW, ShowWindow, WHEEL_DELTA, WM_CAPTURECHANGED,
                WM_CLOSE, WM_DESTROY, WM_DPICHANGED, WM_DWMCOLORIZATIONCOLORCHANGED, WM_ERASEBKGND,
                WM_GETMINMAXINFO, WM_GETOBJECT, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP,
                WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCDESTROY, WM_NCHITTEST, WM_PAINT, WM_SETCURSOR,
                WM_SETTINGCHANGE, WM_SIZE,
            },
        },
    },
    core::{PCWSTR, Result, w},
};

use crate::{
    channel,
    export::{self, Output},
    report,
    settings::EditorSettings,
};
use canvas::{Frame, Glide, Gpu, HANDLE_REACH, View, WHITE};
use toolbar::{Part, Tool, Toolbar};

/// The title bar's height, in DIPs. Saving sits in it.
const TITLE_BAR: f32 = 48.0;
/// The least room around the capture.
const MARGIN: f32 = 24.0;
/// The least of the capture a resized window keeps showing.
const MIN_CANVAS: f32 = 100.0;
/// The most of the screen a new editor takes, in each direction.
const MAX_SHARE: f32 = 0.85;
/// How much each notch of the mouse wheel zooms.
const ZOOM_STEP: f32 = 1.25;
const CLASS: PCWSTR = w!("Kata.Editor");

/// A capture to edit.
pub struct Capture {
    pub image: Image,
    /// Where it was taken, on the virtual screen.
    pub area: Rect,
    /// The display scale it was taken at.
    pub scale: f32,
}

thread_local! {
    /// The editor's window, while one is open.
    static OPEN: Cell<Option<HWND>> = const { Cell::new(None) };
}

/// Edits `capture`, which is delivered from the start: on the clipboard, through
/// `owner`, and in a new file, with the tools named under their icons if
/// `tool_names`. The capture already open, if any, closes first.
pub fn open(graphics: &Graphics, owner: HWND, capture: Capture, tool_names: bool) {
    // Closing delivers the open capture's last changes, which mustn't follow this one.
    close();
    let output = Output::new(owner, capture.image.clone());
    match unsafe { create(graphics, capture, output, tool_names) } {
        Ok(hwnd) => OPEN.set(Some(hwnd)),
        Err(error) => report("editor", &error),
    }
}

pub fn close() {
    if let Some(hwnd) = OPEN.take() {
        let _ = unsafe { DestroyWindow(hwnd) };
    }
}

unsafe fn create(
    graphics: &Graphics,
    capture: Capture,
    output: Output,
    tool_names: bool,
) -> Result<HWND> {
    let fonts = Fonts::new(graphics)?;
    let settings = EditorSettings::load();
    let toolbar = Toolbar::new(graphics, &fonts, &settings, tool_names)?;
    let chip = fonts.icon('\u{E711}', 10.0)?;
    let palette = Palette::current();
    let min = min_size(&toolbar);
    let image = (capture.image.width() as f32, capture.image.height() as f32);
    let area = capture.area;
    unsafe {
        // On the capture's monitor, pixel for pixel when it fits.
        let monitor = MonitorFromRect(
            &RECT {
                left: area.left,
                top: area.top,
                right: area.right,
                bottom: area.bottom,
            },
            MONITOR_DEFAULTTONEAREST,
        );
        let work = WorkArea::of(monitor);
        let scale = work.scale;
        let client = (
            (image.0 + 2.0 * MARGIN * scale)
                .min(work.width() as f32 * MAX_SHARE)
                .max((min.0 * scale).ceil()),
            (image.1 + (TITLE_BAR + MARGIN + toolbar.bottom()) * scale)
                .min(work.height() as f32 * MAX_SHARE)
                .max((min.1 * scale).ceil()),
        );
        let client = (client.0 as i32, client.1 as i32);
        let title = channel::name();
        let hwnd = window::create(CLASS, title, window_proc, &work, client, !palette.light)?;
        let size = window::client_size(hwnd);
        let renderer = Renderer::new(&capture.image, capture.scale);
        let gpu = match Gpu::new(graphics, hwnd, size, renderer.output()) {
            Ok(gpu) => gpu,
            Err(error) => {
                let _ = DestroyWindow(hwnd);
                return Err(error);
            }
        };
        let mut editor = Editor {
            hwnd,
            graphics: graphics.clone(),
            palette,
            scale: window::scale(hwnd),
            size,
            document: Document::new(capture.image),
            edits: Edits::default(),
            renderer,
            capture_scale: capture.scale,
            stale: Vec::new(),
            output,
            settings,
            toolbar,
            chip,
            view: View::default(),
            glide: Glide::rest(View::default()),
            gpu: Some(gpu),
            selected: None,
            hovered: None,
            chip_hovered: false,
            gesture: None,
            zoom: Animation::idle(1.0),
            tracking: false,
            unpublished: false,
            unsaved: false,
            access: Access::new(hwnd),
        };
        // The first frame is ready before the window shows.
        editor.layout();
        editor.paint();
        let state = Box::into_raw(Box::new(RefCell::new(editor)));
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        Ok(hwnd)
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        if let Some(result) = window::frame_proc(hwnd, message, wparam, lparam) {
            return result;
        }
        if message == WM_NCDESTROY {
            release(hwnd);
        }
        window::linked_state::<Editor>(hwnd, message, lparam)
            .and_then(|editor| editor.try_borrow_mut().ok())
            .and_then(|mut editor| editor.handle(message, wparam, lparam))
            .unwrap_or_else(|| DefWindowProcW(hwnd, message, wparam, lparam))
    }
}

/// Frees the editor as its window goes away.
unsafe fn release(hwnd: HWND) {
    let state = unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) } as *mut RefCell<Editor>;
    // A window destroyed from inside its own handler keeps its editor, still in use.
    if !state.is_null() && unsafe { (*state).try_borrow_mut() }.is_ok() {
        drop(unsafe { Box::from_raw(state) });
    }
    if OPEN.get() == Some(hwnd) {
        OPEN.set(None);
    }
}

/// The smallest client area, in DIPs: the toolbar and a little of the capture.
fn min_size(toolbar: &Toolbar) -> (f32, f32) {
    (
        toolbar.min_width(),
        TITLE_BAR + MARGIN + MIN_CANVAS + toolbar.bottom(),
    )
}

/// Whether `shape` has handles to resize it: steps keep their size.
fn resizable(shape: Shape) -> bool {
    !matches!(shape, Shape::Step(_))
}

/// `rect`, at least a pixel wide and high.
fn solid(rect: Rect) -> Rect {
    Rect::new(
        rect.left,
        rect.top,
        rect.right.max(rect.left + 1),
        rect.bottom.max(rect.top + 1),
    )
}

/// What the left button is doing.
#[derive(Clone, Copy)]
enum Gesture {
    /// Pressing a toolbar control.
    Control(Part),
    /// Down on the capture: a click so far. `start` is the pixel edge nearest to
    /// `at`, and `on` the shape there.
    Press {
        at: Point,
        start: (i32, i32),
        on: Option<Shape>,
    },
    /// Drawing a new shape from `start`.
    Draw { shape: Shape, start: (i32, i32) },
    /// Moving a shape grabbed at `grab`, from where it was.
    Move {
        shape: Shape,
        grab: (i32, i32),
        outline: Rect,
    },
    /// Resizing a shape, whose `anchor` corner stays in place.
    Resize { shape: Shape, anchor: (i32, i32) },
    /// Moving an arrow's head, or its tail, from `end`, grabbed at `grab`.
    Aim {
        index: usize,
        head: bool,
        end: (i32, i32),
        grab: (i32, i32),
    },
    /// Pressing the selected shape's remove chip.
    Remove,
}

struct Editor {
    hwnd: HWND,
    graphics: Graphics,
    palette: Palette,
    /// Pixels per DIP.
    scale: f32,
    /// The client area, in pixels.
    size: (u32, u32),
    document: Document,
    /// The edits on show: the document's, plus the gesture in progress.
    edits: Edits,
    renderer: Renderer,
    /// The display scale the capture was taken at.
    capture_scale: f32,
    /// The areas of the rendering the graphics card doesn't have yet.
    stale: Vec<Rect>,
    output: Output,
    /// The settings as last saved.
    settings: EditorSettings,
    toolbar: Toolbar,
    /// The remove chip's glyph.
    chip: Label,
    /// Where the capture shows, and where it's heading as it zooms.
    view: View,
    glide: Glide,
    /// Made again after the graphics device goes away.
    gpu: Option<Gpu>,
    selected: Option<Shape>,
    hovered: Option<Shape>,
    chip_hovered: bool,
    gesture: Option<Gesture>,
    /// The focus's zoom on show, which glides between levels.
    zoom: Animation,
    /// Whether Windows will say when the pointer leaves.
    tracking: bool,
    /// The document changed since it was last put on the clipboard.
    unpublished: bool,
    /// The clipboard has a version the file doesn't.
    unsaved: bool,
    access: Access,
}

impl Editor {
    fn handle(&mut self, message: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        match message {
            WM_PAINT => self.paint(),
            // Every frame covers the whole window: erasing first would only flicker.
            WM_ERASEBKGND => return Some(LRESULT(1)),
            WM_NCHITTEST => {
                let hit = self.hit(window::hit_position(self.hwnd, lparam));
                return Some(LRESULT(hit as isize));
            }
            WM_SETCURSOR if lparam.0 as u16 as u32 == HTCLIENT => {
                self.set_cursor();
                return Some(LRESULT(1));
            }
            WM_MOUSEMOVE => self.pointer_moved(self.point(lparam)),
            WM_MOUSELEAVE => {
                self.tracking = false;
                if self.gesture.is_none() && self.hover(None) {
                    self.invalidate();
                }
            }
            // The pointer rested: a tool's name may show.
            WM_MOUSEHOVER => {
                if self.gesture.is_none() && self.toolbar.rest(self.point(lparam)) {
                    self.invalidate();
                }
            }
            WM_MOUSEWHEEL => self.wheel(wparam, lparam),
            WM_LBUTTONDOWN => self.press(self.point(lparam)),
            WM_LBUTTONUP => self.release(self.point(lparam)),
            // Another window took the pointer.
            WM_CAPTURECHANGED => self.cancel(),
            WM_KEYDOWN => return self.key(VIRTUAL_KEY(wparam.0 as u16)).then_some(LRESULT(0)),
            WM_SIZE if wparam.0 != SIZE_MINIMIZED as usize => self.resized(),
            WM_DPICHANGED => self.dpi_changed(wparam, lparam),
            // Keeps the window from getting too small for the toolbar.
            WM_GETMINMAXINFO => unsafe {
                window::limit_size(lparam, min_size(&self.toolbar), self.scale);
            },
            WM_SETTINGCHANGE | WM_DWMCOLORIZATIONCOLORCHANGED => {
                self.follow_theme();
                return None;
            }
            WM_GETOBJECT => {
                let result = self.access.answer(self.hwnd, wparam, lparam)?;
                self.update_access();
                return Some(result);
            }
            access::ACT => {
                if let Some(part) = self.toolbar.part(wparam.0)
                    && self.gesture.is_none()
                {
                    self.act(part, access::action(lparam));
                    self.remember();
                    self.invalidate();
                }
            }
            // The last changes still reach the clipboard.
            WM_DESTROY => {
                if self.unpublished {
                    self.publish();
                }
                self.access.close(self.hwnd);
                return None;
            }
            _ => return None,
        }
        Some(LRESULT(0))
    }

    /// The pointer position carried by mouse messages, in DIPs.
    fn point(&self, lparam: LPARAM) -> Point {
        let (x, y) = window::mouse_position(lparam);
        (x as f32 / self.scale, y as f32 / self.scale)
    }

    /// What's at `(x, y)`, in client pixels, for Windows: the title bar moves the
    /// window, except where the toolbar has a control.
    fn hit(&self, (x, y): (i32, i32)) -> u32 {
        let point = (x as f32 / self.scale, y as f32 / self.scale);
        if point.1 < TITLE_BAR && self.toolbar.part_at(point).is_none() {
            HTCAPTION
        } else {
            HTCLIENT
        }
    }

    /// Where the pointer is now, in DIPs.
    fn pointer(&self) -> Point {
        let mut pointer = POINT::default();
        unsafe {
            let _ = GetCursorPos(&mut pointer);
            let _ = ScreenToClient(self.hwnd, &mut pointer);
        }
        (pointer.x as f32 / self.scale, pointer.y as f32 / self.scale)
    }

    /// Shows what pressing would do.
    fn set_cursor(&self) {
        let point = self.pointer();
        let cursor = if self.toolbar.covers(point) || self.chip_at(point) {
            IDC_ARROW
        } else if let Some(handle) = self.handle_at(point) {
            // An arrow's ends go anywhere, and the top-left and bottom-right corners
            // pull along one diagonal.
            if self.selected.and_then(|shape| self.arrow(shape)).is_some() {
                IDC_SIZEALL
            } else if handle % 2 == 0 {
                IDC_SIZENWSE
            } else {
                IDC_SIZENESW
            }
        } else if self.shape_at(point).is_some_and(|shape| !self.fills(shape)) {
            IDC_SIZEALL
        } else if point.1 >= TITLE_BAR {
            IDC_CROSS
        } else {
            IDC_ARROW
        };
        unsafe {
            if let Ok(cursor) = LoadCursorW(None, cursor) {
                SetCursor(Some(cursor));
            }
        }
    }

    /// Zooms the capture around the pointer, as `WM_MOUSEWHEEL` asks: in as the wheel
    /// turns away from the user, out as it turns back, down to fitting the window.
    fn wheel(&mut self, wparam: WPARAM, lparam: LPARAM) {
        if self.gesture.is_some() {
            return;
        }
        let (x, y) = window::hit_position(self.hwnd, lparam);
        let at = (x as f32 / self.scale, y as f32 / self.scale);
        let notches = f32::from((wparam.0 >> 16) as u16 as i16) / WHEEL_DELTA as f32;
        let to = self
            .frame()
            .zoom(self.glide.target(), ZOOM_STEP.powf(notches), at);
        self.glide_to(to);
    }

    fn glide_to(&mut self, view: View) {
        self.glide = self.glide.retarget(view, Instant::now());
        self.invalidate();
    }

    fn pointer_moved(&mut self, point: Point) {
        if !self.tracking {
            self.tracking = window::track_mouse_leave(self.hwnd);
        }
        let changed = match self.gesture {
            Some(_) => self.drag(point),
            None => self.hover(Some(point)),
        };
        if changed {
            self.invalidate();
        }
        // A tool's name shows once the pointer stops moving.
        if self.gesture.is_none() && self.toolbar.awaits_rest() {
            window::track_mouse_rest(self.hwnd);
        }
    }

    /// Follows the pointer while no button is down. Returns whether the look changes.
    fn hover(&mut self, point: Option<Point>) -> bool {
        let toolbar = self.toolbar.hover(point);
        let chip = point.is_some_and(|point| !self.toolbar.covers(point) && self.chip_at(point));
        let shape = point
            .filter(|_| !chip)
            .and_then(|point| self.shape_at(point));
        let changed = toolbar || chip != self.chip_hovered || shape != self.hovered;
        (self.chip_hovered, self.hovered) = (chip, shape);
        changed
    }

    fn press(&mut self, point: Point) {
        self.toolbar.unfocus();
        let bounds = self.bounds();
        let gesture = if let Some(part) = self.toolbar.part_at(point) {
            if self.toolbar.control(part).press(point) {
                self.option_changed(part);
            }
            Gesture::Control(part)
        } else if self.toolbar.covers(point) {
            return;
        } else if self.chip_at(point) {
            Gesture::Remove
        } else if let Some(shape) = self.selected
            && let Some(handle) = self.handle_at(point)
        {
            if let Some((index, arrow)) = self.arrow(shape) {
                // The first handle is on the tail, the second on the head.
                let head = handle == 1;
                Gesture::Aim {
                    index,
                    head,
                    end: if head { arrow.to } else { arrow.from },
                    grab: self.view.pixel(point),
                }
            } else {
                let Some(outline) = self.edits.outline(shape, bounds) else {
                    return;
                };
                let corners =
                    canvas::corners(outline.left, outline.top, outline.right, outline.bottom);
                Gesture::Resize {
                    shape,
                    anchor: corners[(handle + 2) % 4],
                }
            }
        } else if self.places_step(point) {
            // A click places the next step there, and dragging carries it along.
            let grab = self.view.edge(point, bounds);
            let shape = self.add(point);
            let Some(outline) = self.edits.outline(shape, bounds) else {
                return;
            };
            Gesture::Move {
                shape,
                grab,
                outline,
            }
        } else if point.1 >= TITLE_BAR {
            let on = self.shape_at(point);
            if let Some(shape) = on.filter(|&shape| !self.fills(shape)) {
                self.select(Some(shape));
            }
            Gesture::Press {
                at: point,
                start: self.view.edge(point, bounds),
                on,
            }
        } else {
            return;
        };
        self.gesture = Some(gesture);
        unsafe { SetCapture(self.hwnd) };
        self.invalidate();
    }

    /// Follows the pointer while the button is down. Returns whether the look changes.
    fn drag(&mut self, point: Point) -> bool {
        let bounds = self.bounds();
        let edge = self.view.edge(point, bounds);
        let Some(gesture) = self.gesture else {
            return false;
        };
        match gesture {
            Gesture::Control(part) => {
                let changed = self.toolbar.control(part).drag(point);
                if changed {
                    self.option_changed(part);
                }
                changed
            }
            Gesture::Press { at, start, on } => {
                if !self.dragged(at, point) {
                    return false;
                }
                let grabbed = on
                    .filter(|&shape| !self.fills(shape))
                    .and_then(|shape| Some((shape, self.edits.outline(shape, bounds)?)));
                self.gesture = Some(match grabbed {
                    Some((shape, outline)) => Gesture::Move {
                        shape,
                        grab: start,
                        outline,
                    },
                    // Steps are placed with a click, not drawn.
                    None if self.toolbar.tool() == Tool::Steps => return false,
                    None => Gesture::Draw {
                        shape: self.add(at),
                        start,
                    },
                });
                self.drag(point)
            }
            Gesture::Draw { shape, start } => {
                let rect = solid(Rect::from_corners(start, edge)).moved_inside(&bounds);
                match shape {
                    // A focus is drawn at 100 %, and magnifies once let go.
                    Shape::Focus => {
                        if let Some(focus) = &mut self.edits.focus {
                            focus.rect = rect;
                        }
                    }
                    // A figure runs from pixel to pixel.
                    Shape::Figure(index) => {
                        let to = self.pixel(point);
                        if let Some(figure) = self.edits.figures.get_mut(index) {
                            figure.to = to;
                        }
                    }
                    Shape::Blur(_) | Shape::Step(_) => self.edits.reshape(shape, rect, bounds),
                }
                true
            }
            Gesture::Move {
                shape,
                grab,
                outline,
            } => {
                let moved = outline
                    .offset(edge.0 - grab.0, edge.1 - grab.1)
                    .moved_inside(&bounds);
                self.edits.reshape(shape, moved, bounds);
                true
            }
            Gesture::Resize { shape, anchor } => {
                let rect = solid(Rect::from_corners(anchor, edge)).moved_inside(&bounds);
                self.edits.reshape(shape, rect, bounds);
                true
            }
            Gesture::Aim {
                index,
                head,
                end,
                grab,
            } => {
                let (x, y) = self.view.pixel(point);
                let moved = (
                    (end.0 + x - grab.0).clamp(bounds.left, bounds.right - 1),
                    (end.1 + y - grab.1).clamp(bounds.top, bounds.bottom - 1),
                );
                if let Some(arrow) = self.edits.figures.get_mut(index) {
                    *if head { &mut arrow.to } else { &mut arrow.from } = moved;
                }
                true
            }
            Gesture::Remove => {
                let hovered = self.chip_at(point);
                mem::replace(&mut self.chip_hovered, hovered) != hovered
            }
        }
    }

    fn release(&mut self, point: Point) {
        let Some(gesture) = self.gesture.take() else {
            return;
        };
        let _ = unsafe { ReleaseCapture() };
        match gesture {
            Gesture::Control(part) => {
                if self.toolbar.control(part).release(point) {
                    self.activated(part);
                }
            }
            // A click selects the shape under it, and away from the shapes clears the
            // selection.
            Gesture::Press { on, .. } => self.select(on),
            Gesture::Draw {
                shape: Shape::Focus,
                ..
            } => {
                if let Some(focus) = self.edits.focus {
                    let zoom = ZOOMS[focus.level];
                    self.zoom = Animation::new(1.0, zoom, motion::NORMAL, Easing::DECELERATE);
                }
            }
            // A figure needs its ends apart.
            Gesture::Draw {
                shape: Shape::Figure(index),
                ..
            } if self
                .edits
                .figures
                .get(index)
                .is_some_and(|figure| figure.from == figure.to) =>
            {
                self.remove_selected();
            }
            Gesture::Remove if self.chip_at(point) => self.remove_selected(),
            _ => {}
        }
        self.commit();
        self.remember();
        self.hover(Some(point));
        self.invalidate();
    }

    /// Drops the gesture in progress and what it changed.
    fn cancel(&mut self) {
        let Some(gesture) = self.gesture.take() else {
            return;
        };
        if let Gesture::Control(part) = gesture {
            self.toolbar.control(part).cancel();
        }
        self.sync();
        self.invalidate();
    }

    /// Returns whether `key` did something.
    fn key(&mut self, key: VIRTUAL_KEY) -> bool {
        if self.gesture.is_some() {
            if key != VK_ESCAPE {
                return false;
            }
            self.cancel();
            let _ = unsafe { ReleaseCapture() };
            return true;
        }
        let (ctrl, shift) = (window::held(VK_CONTROL), window::held(VK_SHIFT));
        match key {
            VK_TAB => self.toolbar.tab(shift),
            VK_1 | VK_NUMPAD1 if !ctrl => self.choose_tool(Tool::Blur),
            VK_2 | VK_NUMPAD2 if !ctrl => self.choose_tool(Tool::Focus),
            VK_3 | VK_NUMPAD3 if !ctrl => self.choose_tool(Tool::Steps),
            VK_4 | VK_NUMPAD4 if !ctrl => self.choose_tool(Tool::Shapes),
            VK_Z if ctrl && shift => self.step(Document::redo),
            VK_Y if ctrl => self.step(Document::redo),
            VK_Z if ctrl => self.step(Document::undo),
            VK_DELETE | VK_BACK => self.remove_selected(),
            VK_ESCAPE => self.select(None),
            VK_0 | VK_NUMPAD0 if ctrl => self.glide_to(self.frame().fit()),
            VK_S if ctrl => self.save(),
            VK_C if ctrl => {
                self.render_document();
                export::copy(self.hwnd, self.renderer.output());
            }
            // The rest go to the control with the keyboard focus.
            _ => {
                let Some((part, action)) = self.toolbar.focused().zip(Action::of(key)) else {
                    return false;
                };
                self.act(part, action);
            }
        }
        self.remember();
        self.invalidate();
        true
    }

    /// Whether the pointer went far enough from `from` to drag rather than click.
    fn dragged(&self, from: Point, to: Point) -> bool {
        let dpi = self.dpi();
        let (x, y) = unsafe {
            (
                GetSystemMetricsForDpi(SM_CXDRAG, dpi),
                GetSystemMetricsForDpi(SM_CYDRAG, dpi),
            )
        };
        ((to.0 - from.0) * self.scale).abs() > x as f32
            || ((to.1 - from.1) * self.scale).abs() > y as f32
    }

    /// Starts a shape of the current tool at `at`, selected: a step centered on it.
    fn add(&mut self, at: Point) -> Shape {
        let bounds = self.bounds();
        let start = self.view.edge(at, bounds);
        let rect = solid(Rect::new(start.0, start.1, start.0, start.1)).moved_inside(&bounds);
        let shape = match self.toolbar.tool() {
            Tool::Blur => {
                let level = self.toolbar.blur_level();
                self.edits.blurs.push(Blur { rect, level });
                Shape::Blur(self.edits.blurs.len() - 1)
            }
            Tool::Focus => {
                // There's one focus at most: a new one replaces it.
                let level = self.toolbar.zoom_level();
                self.edits.focus = Some(Focus {
                    rect,
                    level,
                    blur: self.toolbar.around(),
                });
                self.zoom = Animation::idle(1.0);
                Shape::Focus
            }
            Tool::Steps => {
                let side = self.step_side(self.toolbar.step_size());
                self.edits.steps.push(Step::new(start, side, bounds));
                Shape::Step(self.edits.steps.len() - 1)
            }
            Tool::Shapes => {
                let end = self.pixel(at);
                self.edits.figures.push(Figure {
                    form: self.toolbar.form(),
                    from: end,
                    to: end,
                    line: Figure::line(bounds, self.capture_scale),
                });
                Shape::Figure(self.edits.figures.len() - 1)
            }
        };
        self.selected = Some(shape);
        shape
    }

    fn remove_selected(&mut self) {
        if let Some(shape) = self.selected.take() {
            self.edits.remove(shape);
            self.hovered = None;
            self.commit();
        }
    }

    /// Selects `shape`, showing its tool and level in the toolbar.
    fn select(&mut self, shape: Option<Shape>) {
        self.selected = shape;
        match shape {
            Some(Shape::Blur(index)) => {
                self.toolbar.select_tool(Tool::Blur);
                if let Some(blur) = self.edits.blurs.get(index) {
                    self.toolbar.set_blur_level(blur.level);
                }
            }
            Some(Shape::Focus) => {
                self.toolbar.select_tool(Tool::Focus);
                if let Some(focus) = self.edits.focus {
                    self.toolbar.set_zoom_level(focus.level);
                    self.toolbar.set_around(focus.blur);
                }
            }
            Some(Shape::Step(_)) => self.toolbar.select_tool(Tool::Steps),
            Some(Shape::Figure(index)) => {
                self.toolbar.select_tool(Tool::Shapes);
                if let Some(figure) = self.edits.figures.get(index) {
                    self.toolbar.set_form(figure.form);
                }
            }
            None => {}
        }
    }

    fn choose_tool(&mut self, tool: Tool) {
        self.toolbar.select_tool(tool);
        self.tool_changed();
    }

    /// Only a shape of the current tool stays selected.
    fn tool_changed(&mut self) {
        self.toolbar.follow_tool();
        let tool = self.toolbar.tool();
        self.selected = self.selected.filter(|&shape| Tool::of(shape) == tool);
    }

    /// How wide the steps are at `size`, an index into `STEP_SIZES`.
    fn step_side(&self, size: usize) -> i32 {
        Step::side(self.bounds(), self.capture_scale, size)
    }

    /// Gives the selected shape an option's new value. The steps all take the new size.
    fn option_changed(&mut self, part: Part) {
        match (part, self.selected) {
            (Part::StepSize, _) => {
                let side = self.step_side(self.toolbar.step_size());
                self.edits.resize_steps(side, self.bounds());
            }
            (Part::Blur, Some(Shape::Blur(index))) => {
                if let Some(blur) = self.edits.blurs.get_mut(index) {
                    blur.level = self.toolbar.blur_level();
                }
            }
            (Part::Zoom, Some(Shape::Focus)) => {
                if let Some(focus) = &mut self.edits.focus {
                    focus.level = self.toolbar.zoom_level();
                    let zoom = ZOOMS[focus.level];
                    self.zoom = self
                        .zoom
                        .retarget(zoom, motion::NORMAL, Easing::POINT_TO_POINT);
                }
            }
            (Part::Around, Some(Shape::Focus)) => {
                if let Some(focus) = &mut self.edits.focus {
                    focus.blur = self.toolbar.around();
                }
            }
            (Part::Form, Some(Shape::Figure(index))) => {
                if let Some(figure) = self.edits.figures.get_mut(index) {
                    figure.form = self.toolbar.form();
                }
            }
            _ => {}
        }
    }

    /// Carries out `action`, asked by the keyboard or an assistive technology, on
    /// `part`.
    fn act(&mut self, part: Part, action: Action) {
        if self.toolbar.control(part).act(action) {
            self.activated(part);
            self.commit();
        }
    }

    /// A toolbar control was clicked.
    fn activated(&mut self, part: Part) {
        match part {
            Part::Tools => self.tool_changed(),
            Part::Save => self.save(),
            Part::Autosave => {
                let on = self.toolbar.autosave();
                self.toolbar.show_save(!on);
                if on {
                    self.save();
                }
            }
            Part::Blur | Part::Zoom | Part::Around | Part::StepSize | Part::Form => {
                self.option_changed(part)
            }
            Part::Folder => self.output.show(),
        }
    }

    /// Makes the edits on show the document's, when they changed.
    fn commit(&mut self) {
        if self.document.commit(self.edits.clone()) {
            self.unpublished = true;
        }
    }

    /// Moves through the document's history.
    fn step(&mut self, step: fn(&mut Document) -> bool) {
        if step(&mut self.document) {
            self.selected = None;
            self.sync();
            self.unpublished = true;
        }
    }

    /// Shows the document's edits again, and nothing more.
    fn sync(&mut self) {
        self.edits = self.document.edits().clone();
        let bounds = self.bounds();
        let selected = self
            .selected
            .filter(|&shape| self.edits.outline(shape, bounds).is_some());
        self.select(selected);
        // The steps all share a size, which the slider shows.
        if let Some(step) = self.edits.steps.first() {
            let side = step.rect.width();
            let size = (0..STEP_SIZES.len())
                .min_by_key(|&size| (self.step_side(size) - side).abs())
                .unwrap_or_default();
            self.toolbar.set_step_size(size);
        }
        let zoom = self.edits.focus.map_or(1.0, |focus| ZOOMS[focus.level]);
        self.zoom = Animation::idle(zoom);
        self.hovered = None;
    }

    /// Saves the toolbar's settings for the next capture, when they changed.
    fn remember(&mut self) {
        let settings = self.toolbar.settings();
        if settings != self.settings {
            settings.save();
            self.settings = settings;
        }
    }

    /// Puts the document on the clipboard and, with autosave on, in its file.
    fn publish(&mut self) {
        self.render_document();
        export::copy(self.hwnd, self.renderer.output());
        self.unpublished = false;
        self.unsaved = true;
        if self.toolbar.autosave() {
            self.write();
        } else {
            self.toolbar.enable_save(true);
        }
    }

    /// Brings the clipboard and the file up to date, autosave or not.
    fn save(&mut self) {
        if self.unpublished {
            self.publish();
        }
        if self.unsaved {
            self.write();
        }
    }

    fn write(&mut self) {
        self.render_document();
        self.output.save(self.renderer.output().clone());
        self.unsaved = false;
        self.toolbar.enable_save(false);
    }

    /// Renders the document as it's delivered: its edits, at rest.
    fn render_document(&mut self) {
        let edits = self.document.edits();
        let spot = edits.focus.map(|focus| focus.spot());
        let changed = self.renderer.update(
            self.document.image(),
            &edits.blurs,
            spot,
            &edits.figures,
            &edits.steps,
        );
        if !changed.is_empty() {
            // The window was showing something else, such as a zoom halfway.
            self.stale.extend(changed);
            self.invalidate();
        }
    }

    fn paint(&mut self) {
        let _ = unsafe { ValidateRect(Some(self.hwnd), None) };
        let now = Instant::now();
        let view = self.glide.at(now);
        if view != self.view {
            self.view = view;
            // The capture moved under the pointer.
            if self.tracking && self.gesture.is_none() {
                self.hover(Some(self.pointer()));
                self.set_cursor();
            }
        }
        let zoom = self.zoom.value(now);
        let spot = self.edits.focus.map(|focus| Spot {
            zoom,
            ..focus.spot()
        });
        let changed = self.renderer.update(
            self.document.image(),
            &self.edits.blurs,
            spot,
            &self.edits.figures,
            &self.edits.steps,
        );
        self.stale.extend(changed);
        let fresh = self.gpu.is_none();
        let output = self.renderer.output();
        let gpu = match self.gpu.take() {
            Some(gpu) => gpu.upload(output, &self.stale).map(|()| gpu),
            None => Gpu::new(&self.graphics, self.hwnd, self.size, output),
        };
        self.stale.clear();
        match gpu.and_then(|mut gpu| self.draw(&mut gpu, now).map(|()| gpu)) {
            Ok(gpu) => self.gpu = Some(gpu),
            Err(error) if fresh => {
                report("editor", &error);
                let _ = unsafe { PostMessageW(Some(self.hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) };
            }
            // The graphics device went away (a driver update, say): start over.
            Err(_) => self.invalidate(),
        }
        if self.toolbar.is_animating(now) || self.zoom.is_running(now) || self.glide.is_running(now)
        {
            self.invalidate();
        }
        if self.unpublished && self.gesture.is_none() && !self.zoom.is_running(now) {
            self.publish();
        }
        self.update_access();
    }

    /// Tells assistive technologies, if any follow the window, how the toolbar is.
    fn update_access(&mut self) {
        if self.access.is_wanted() {
            let focus = self.toolbar.focused().map(|part| part as usize);
            self.access
                .update(self.toolbar.describe(), focus, self.scale);
        }
    }

    fn draw(&self, gpu: &mut Gpu, now: Instant) -> Result<()> {
        let Gpu { surface, image } = gpu;
        surface.draw(&self.palette, self.scale, |painter| {
            let (width, height) = self.window();
            // A zoomed capture slides under the title bar, out of sight.
            let canvas = Area::new(0.0, TITLE_BAR, width, height - TITLE_BAR);
            painter.clip(canvas, || self.draw_capture(painter, image, now));
            self.toolbar.draw(painter, now);
        })
    }

    /// The capture with its frame, and the outlines, handles and chip over it.
    fn draw_capture(&self, painter: &Painter, image: &ID2D1Bitmap1, now: Instant) {
        let capture = self.view.area(self.bounds());
        painter.bitmap(image, capture);
        let border = painter.pixel();
        painter.stroke(
            capture.inflate(border),
            0.0,
            border,
            self.palette.card_stroke,
        );
        // An arrow shows no outline: its ends' handles say enough.
        let ringed = |shape: Shape| self.arrow(shape).is_none();
        if self.gesture.is_none()
            && let Some(shape) = self.hovered.filter(|&shape| Some(shape) != self.selected)
            && ringed(shape)
            && let Some(area) = self.shape_area(shape, now)
        {
            let color = faded(WHITE, 0.9);
            canvas::draw_outline(painter, area, self.radius(shape), canvas::HOVERED, color);
        }
        if let Some(shape) = self.selected
            && let Some(area) = self.shape_area(shape, now)
        {
            if ringed(shape) {
                let accent = self.palette.accent;
                canvas::draw_outline(painter, area, self.radius(shape), canvas::SELECTED, accent);
            }
            canvas::draw_handles(painter, &self.handles(shape, area));
            if matches!(self.gesture, None | Some(Gesture::Remove))
                && let Some(center) = canvas::chip(area, self.window())
            {
                canvas::draw_chip(painter, center, &self.chip, self.chip_hovered);
            }
        }
    }

    /// Follows the window's new size.
    fn resized(&mut self) {
        self.size = window::client_size(self.hwnd);
        let size = self.size;
        if self
            .gpu
            .as_mut()
            .is_some_and(|gpu| gpu.surface.resize(size).is_err())
        {
            self.gpu = None;
        }
        self.layout();
        self.paint();
    }

    /// The window moved to a monitor with another scale, and Windows suggests where
    /// it goes.
    fn dpi_changed(&mut self, wparam: WPARAM, lparam: LPARAM) {
        self.scale = unsafe { window::follow_dpi(self.hwnd, wparam, lparam) };
        self.resized();
    }

    fn follow_theme(&mut self) {
        self.palette = Palette::current();
        window::set_dark(self.hwnd, !self.palette.light);
        self.invalidate();
    }

    /// Fits the capture in the window again, unzoomed, and places the toolbar.
    fn layout(&mut self) {
        self.view = self.frame().fit();
        self.glide = Glide::rest(self.view);
        let caption_buttons = window::caption_buttons(self.hwnd, self.scale).left;
        self.toolbar.layout(self.window(), caption_buttons);
    }

    /// Where `shape` shows, in capture pixels: a focus follows its zoom as it glides.
    fn outline(&self, shape: Shape, now: Instant) -> Option<Rect> {
        match (shape, self.edits.focus) {
            (Shape::Focus, Some(focus)) => {
                let zoom = self.zoom.value(now);
                Some(
                    Spot {
                        zoom,
                        ..focus.spot()
                    }
                    .lit(self.bounds()),
                )
            }
            _ => self.edits.outline(shape, self.bounds()),
        }
    }

    /// Where `shape` shows in the window.
    fn shape_area(&self, shape: Shape, now: Instant) -> Option<Area> {
        self.outline(shape, now).map(|rect| self.view.area(rect))
    }

    /// The corner radius `shape` shows with, in DIPs.
    fn radius(&self, shape: Shape) -> f32 {
        match shape {
            Shape::Blur(_) => 0.0,
            Shape::Focus => FOCUS_CORNER * self.capture_scale * self.view.zoom,
            Shape::Figure(index) => self
                .edits
                .figures
                .get(index)
                .map_or(0.0, |figure| figure.corner() * self.view.zoom),
            Shape::Step(index) => self
                .edits
                .steps
                .get(index)
                .map_or(0.0, |step| step.rect.width() as f32 * self.view.zoom / 2.0),
        }
    }

    /// The topmost shape under `point`, or with a figure's line within reach. The
    /// steps' tool reaches only steps, so new ones go over the other shapes.
    fn shape_at(&self, point: Point) -> Option<Shape> {
        if point.1 < TITLE_BAR || self.toolbar.covers(point) {
            return None;
        }
        let (x, y) = self.view.pixel(point);
        let tool = self.toolbar.tool();
        let reach = HANDLE_REACH / self.view.zoom;
        self.edits
            .shape_at(x, y, reach, self.bounds())
            .filter(|&shape| tool != Tool::Steps || Tool::of(shape) == tool)
    }

    /// The arrow `shape` is, if it's one, and its place among the figures.
    fn arrow(&self, shape: Shape) -> Option<(usize, Figure)> {
        let Shape::Figure(index) = shape else {
            return None;
        };
        let figure = *self.edits.figures.get(index)?;
        (figure.form == Form::Arrow).then_some((index, figure))
    }

    /// Where the handles of `shape`, showing in `area`, are: on an arrow's tail and
    /// head, or on the corners of the other shapes that resize.
    fn handles(&self, shape: Shape, area: Area) -> Vec<Point> {
        if let Some((_, arrow)) = self.arrow(shape) {
            vec![self.view.center(arrow.from), self.view.center(arrow.to)]
        } else if resizable(shape) {
            canvas::handles(area).to_vec()
        } else {
            Vec::new()
        }
    }

    /// The capture pixel under `point`, or the nearest one off the capture.
    fn pixel(&self, point: Point) -> (i32, i32) {
        let bounds = self.bounds();
        let (x, y) = self.view.pixel(point);
        (
            x.clamp(bounds.left, bounds.right - 1),
            y.clamp(bounds.top, bounds.bottom - 1),
        )
    }

    /// Whether pressing at `point` places a step: with the steps' tool, on the capture,
    /// away from the other steps.
    fn places_step(&self, point: Point) -> bool {
        self.toolbar.tool() == Tool::Steps
            && point.1 >= TITLE_BAR
            && self.view.area(self.bounds()).contains(point)
            && self.shape_at(point).is_none()
    }

    /// The selected shape's handle under `point`, numbered as `handles` lists them.
    fn handle_at(&self, point: Point) -> Option<usize> {
        let shape = self.selected?;
        let area = self.shape_area(shape, Instant::now())?;
        self.handles(shape, area)
            .iter()
            .position(|&handle| canvas::distance(handle, point) <= HANDLE_REACH)
    }

    /// Whether `point` is on the selected shape's remove chip.
    fn chip_at(&self, point: Point) -> bool {
        self.selected
            .and_then(|shape| self.shape_area(shape, Instant::now()))
            .and_then(|area| canvas::chip(area, self.window()))
            .is_some_and(|center| canvas::chip_contains(center, point))
    }

    /// Whether `shape` covers the whole capture. It has nowhere to move, so dragging
    /// on it draws a new shape instead.
    fn fills(&self, shape: Shape) -> bool {
        let bounds = self.bounds();
        self.edits.outline(shape, bounds) == Some(bounds)
    }

    fn bounds(&self) -> Rect {
        self.document.image().bounds()
    }

    /// The window's size, in DIPs.
    fn window(&self) -> (f32, f32) {
        (
            self.size.0 as f32 / self.scale,
            self.size.1 as f32 / self.scale,
        )
    }

    fn frame(&self) -> Frame {
        let image = self.document.image();
        Frame {
            window: self.window(),
            image: (image.width(), image.height()),
            scale: self.scale,
            bottom: self.toolbar.bottom(),
        }
    }

    fn dpi(&self) -> u32 {
        (self.scale * 96.0).round() as u32
    }

    fn invalidate(&self) {
        let _ = unsafe { InvalidateRect(Some(self.hwnd), None, false) };
    }
}
