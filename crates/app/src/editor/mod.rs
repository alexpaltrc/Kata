//! The editor: a window showing a capture, where the user blurs parts of it or puts
//! one in focus. Each change reaches the clipboard and, with autosave on, the
//! capture's file.

mod canvas;
mod toolbar;

use std::{
    cell::{Cell, RefCell},
    mem,
    time::Instant,
};

use kata_editor::{
    Blur, Document, Edits, FOCUS_CORNER, Focus, Image, Rect, Renderer, Shape, Spot, ZOOMS,
};
use kata_ui::{
    Graphics,
    motion::{self, Animation, Easing},
    paint::{Area, Fonts, Label, Point},
    theme::{Palette, faded},
    window::{self, WorkArea},
};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
        Graphics::Gdi::{
            InvalidateRect, MONITOR_DEFAULTTONEAREST, MonitorFromRect, ScreenToClient, ValidateRect,
        },
        UI::{
            Controls::WM_MOUSELEAVE,
            HiDpi::GetSystemMetricsForDpi,
            Input::KeyboardAndMouse::{
                GetKeyState, ReleaseCapture, SetCapture, VIRTUAL_KEY, VK_1, VK_2, VK_BACK, VK_C,
                VK_CONTROL, VK_DELETE, VK_ESCAPE, VK_NUMPAD1, VK_NUMPAD2, VK_S, VK_SHIFT, VK_Y,
                VK_Z,
            },
            WindowsAndMessaging::{
                DefWindowProcW, DestroyWindow, GWLP_USERDATA, GetCursorPos, HTCAPTION, HTCLIENT,
                IDC_ARROW, IDC_CROSS, IDC_SIZEALL, IDC_SIZENESW, IDC_SIZENWSE, LoadCursorW,
                PostMessageW, SIZE_MINIMIZED, SM_CXDRAG, SM_CYDRAG, SW_SHOW, SetCursor,
                SetForegroundWindow, SetWindowLongPtrW, ShowWindow, WM_CAPTURECHANGED, WM_CLOSE,
                WM_DESTROY, WM_DPICHANGED, WM_DWMCOLORIZATIONCOLORCHANGED, WM_ERASEBKGND,
                WM_GETMINMAXINFO, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
                WM_NCDESTROY, WM_NCHITTEST, WM_PAINT, WM_SETCURSOR, WM_SETTINGCHANGE, WM_SIZE,
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
use canvas::{Gpu, HANDLE_REACH, View, WHITE};
use toolbar::{Part, Tool, Toolbar};

/// The title bar's height, in DIPs. The toolbar sits in it.
const TITLE_BAR: f32 = 48.0;
/// The least room around the capture.
const MARGIN: f32 = 24.0;
/// The least of the capture a resized window keeps showing.
const MIN_CANVAS: f32 = 100.0;
/// The most of the screen a new editor takes, in each direction.
const MAX_SHARE: f32 = 0.85;
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
/// `owner`, and in a new file. The capture already open, if any, closes first.
pub fn open(graphics: &Graphics, owner: HWND, capture: Capture) {
    // Closing delivers the open capture's last changes, which mustn't follow this one.
    close();
    let output = Output::new(owner, capture.image.clone());
    match unsafe { create(graphics, capture, output) } {
        Ok(hwnd) => OPEN.set(Some(hwnd)),
        Err(error) => report("editor", &error),
    }
}

pub fn close() {
    if let Some(hwnd) = OPEN.take() {
        let _ = unsafe { DestroyWindow(hwnd) };
    }
}

unsafe fn create(graphics: &Graphics, capture: Capture, output: Output) -> Result<HWND> {
    let fonts = Fonts::new(graphics)?;
    let settings = EditorSettings::load();
    let toolbar = Toolbar::new(&fonts, &settings)?;
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
            (image.1 + (TITLE_BAR + 2.0 * MARGIN) * scale)
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
            gpu: Some(gpu),
            selected: None,
            hovered: None,
            chip_hovered: false,
            gesture: None,
            zoom: Animation::idle(1.0),
            tracking: false,
            unpublished: false,
            unsaved: false,
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
        toolbar.min_width() + window::CAPTION_BUTTONS,
        TITLE_BAR + 2.0 * MARGIN + MIN_CANVAS,
    )
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
    view: View,
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
            // The last changes still reach the clipboard.
            WM_DESTROY => {
                if self.unpublished {
                    self.publish();
                }
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

    /// Shows what pressing would do.
    fn set_cursor(&self) {
        let mut pointer = POINT::default();
        unsafe {
            let _ = GetCursorPos(&mut pointer);
            let _ = ScreenToClient(self.hwnd, &mut pointer);
        }
        let point = (pointer.x as f32 / self.scale, pointer.y as f32 / self.scale);
        let cursor = if self.toolbar.part_at(point).is_some() || self.chip_at(point) {
            IDC_ARROW
        } else if let Some(corner) = self.handle_at(point) {
            // The top-left and bottom-right corners pull along one diagonal.
            if corner % 2 == 0 {
                IDC_SIZENWSE
            } else {
                IDC_SIZENESW
            }
        } else if self.shape_at(point).is_some() {
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
    }

    /// Follows the pointer while no button is down. Returns whether the look changes.
    fn hover(&mut self, point: Option<Point>) -> bool {
        let toolbar = self.toolbar.hover(point);
        let chip = point.is_some_and(|point| self.chip_at(point));
        let shape = point
            .filter(|_| !chip)
            .and_then(|point| self.shape_at(point));
        let changed = toolbar || chip != self.chip_hovered || shape != self.hovered;
        (self.chip_hovered, self.hovered) = (chip, shape);
        changed
    }

    fn press(&mut self, point: Point) {
        let bounds = self.bounds();
        let gesture = if let Some(part) = self.toolbar.part_at(point) {
            if self.toolbar.control(part).press(point) {
                self.slider_changed(part);
            }
            Gesture::Control(part)
        } else if self.chip_at(point) {
            Gesture::Remove
        } else if let Some(shape) = self.selected
            && let Some(corner) = self.handle_at(point)
            && let Some(outline) = self.edits.outline(shape, bounds)
        {
            let corners = canvas::corners(outline.left, outline.top, outline.right, outline.bottom);
            Gesture::Resize {
                shape,
                anchor: corners[(corner + 2) % 4],
            }
        } else if point.1 >= TITLE_BAR {
            let on = self.shape_at(point);
            if on.is_some() {
                self.select(on);
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
                    self.slider_changed(part);
                }
                changed
            }
            Gesture::Press { at, start, on } => {
                if !self.dragged(at, point) {
                    return false;
                }
                let grabbed =
                    on.and_then(|shape| Some((shape, self.edits.outline(shape, bounds)?)));
                self.gesture = Some(match grabbed {
                    Some((shape, outline)) => Gesture::Move {
                        shape,
                        grab: start,
                        outline,
                    },
                    None => Gesture::Draw {
                        shape: self.add(start),
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
                    Shape::Blur(_) => self.edits.reshape(shape, rect, bounds),
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
            // A click away from the shapes clears the selection.
            Gesture::Press { on: None, .. } => self.select(None),
            Gesture::Draw {
                shape: Shape::Focus,
                ..
            } => {
                if let Some(focus) = self.edits.focus {
                    let zoom = ZOOMS[focus.level];
                    self.zoom = Animation::new(1.0, zoom, motion::NORMAL, Easing::DECELERATE);
                }
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
        let held = |key: VIRTUAL_KEY| unsafe { GetKeyState(i32::from(key.0)) } < 0;
        let (ctrl, shift) = (held(VK_CONTROL), held(VK_SHIFT));
        match key {
            VK_1 | VK_NUMPAD1 if !ctrl => self.choose_tool(Tool::Blur),
            VK_2 | VK_NUMPAD2 if !ctrl => self.choose_tool(Tool::Focus),
            VK_Z if ctrl && shift => self.step(Document::redo),
            VK_Y if ctrl => self.step(Document::redo),
            VK_Z if ctrl => self.step(Document::undo),
            VK_DELETE | VK_BACK => self.remove_selected(),
            VK_ESCAPE => self.select(None),
            VK_S if ctrl => self.save(),
            VK_C if ctrl => {
                self.render_document();
                export::copy(self.hwnd, self.renderer.output());
            }
            _ => return false,
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

    /// Starts a shape of the current tool at `start`, selected.
    fn add(&mut self, start: (i32, i32)) -> Shape {
        let rect =
            solid(Rect::new(start.0, start.1, start.0, start.1)).moved_inside(&self.bounds());
        let shape = match self.toolbar.tool() {
            Tool::Blur => {
                let level = self.toolbar.blur_level();
                self.edits.blurs.push(Blur { rect, level });
                Shape::Blur(self.edits.blurs.len() - 1)
            }
            Tool::Focus => {
                // There's one focus at most: a new one replaces it.
                let level = self.toolbar.zoom_level();
                self.edits.focus = Some(Focus { rect, level });
                self.zoom = Animation::idle(1.0);
                Shape::Focus
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
        let tool = self.toolbar.tool();
        self.selected = self.selected.filter(|&shape| Tool::of(shape) == tool);
    }

    /// Gives the selected shape a slider's new level.
    fn slider_changed(&mut self, part: Part) {
        match (part, self.selected) {
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
            _ => {}
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
            Part::Blur | Part::Zoom => {}
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
        let changed = self
            .renderer
            .update(self.document.image(), &edits.blurs, spot);
        if !changed.is_empty() {
            // The window was showing something else, such as a zoom halfway.
            self.stale.extend(changed);
            self.invalidate();
        }
    }

    fn paint(&mut self) {
        let _ = unsafe { ValidateRect(Some(self.hwnd), None) };
        let now = Instant::now();
        let zoom = self.zoom.value(now);
        let spot = self.edits.focus.map(|focus| Spot {
            rect: focus.rect,
            zoom,
        });
        let changed = self
            .renderer
            .update(self.document.image(), &self.edits.blurs, spot);
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
        if self.toolbar.is_animating(now) || self.zoom.is_running(now) {
            self.invalidate();
        }
        if self.unpublished && self.gesture.is_none() && !self.zoom.is_running(now) {
            self.publish();
        }
    }

    fn draw(&self, gpu: &mut Gpu, now: Instant) -> Result<()> {
        let Gpu { surface, image } = gpu;
        surface.draw(&self.palette, self.scale, |painter| {
            let capture = self.view.area(self.bounds());
            painter.bitmap(image, capture);
            let border = painter.pixel();
            painter.stroke(
                capture.inflate(border),
                0.0,
                border,
                self.palette.card_stroke,
            );
            if self.gesture.is_none()
                && let Some(shape) = self.hovered.filter(|&shape| Some(shape) != self.selected)
                && let Some(area) = self.shape_area(shape, now)
            {
                let color = faded(WHITE, 0.9);
                canvas::draw_outline(painter, area, self.radius(shape), canvas::HOVERED, color);
            }
            if let Some(shape) = self.selected
                && let Some(area) = self.shape_area(shape, now)
            {
                let accent = self.palette.accent;
                canvas::draw_outline(painter, area, self.radius(shape), canvas::SELECTED, accent);
                canvas::draw_handles(painter, area);
                if matches!(self.gesture, None | Some(Gesture::Remove)) {
                    let center = canvas::chip(area, self.width());
                    canvas::draw_chip(painter, center, &self.chip, self.chip_hovered);
                }
            }
            self.toolbar.draw(painter, now);
        })
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

    fn layout(&mut self) {
        let room = (
            self.size.0 as f32 / self.scale,
            self.size.1 as f32 / self.scale,
        );
        let image = self.document.image();
        self.view = View::fit(room, (image.width(), image.height()), self.scale);
        self.toolbar
            .layout(window::caption_buttons(self.hwnd, self.scale));
    }

    /// Where `shape` shows, in capture pixels: a focus follows its zoom as it glides.
    fn outline(&self, shape: Shape, now: Instant) -> Option<Rect> {
        match (shape, self.edits.focus) {
            (Shape::Focus, Some(focus)) => {
                let zoom = self.zoom.value(now);
                Some(
                    Spot {
                        rect: focus.rect,
                        zoom,
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
        }
    }

    /// The topmost shape under `point`.
    fn shape_at(&self, point: Point) -> Option<Shape> {
        if point.1 < TITLE_BAR {
            return None;
        }
        let (x, y) = self.view.pixel(point);
        self.edits.shape_at(x, y, self.bounds())
    }

    /// The selected shape's resize handle under `point`, numbered clockwise from the
    /// top-left.
    fn handle_at(&self, point: Point) -> Option<usize> {
        let area = self.shape_area(self.selected?, Instant::now())?;
        canvas::handles(area)
            .iter()
            .position(|&handle| canvas::distance(handle, point) <= HANDLE_REACH)
    }

    /// Whether `point` is on the selected shape's remove chip.
    fn chip_at(&self, point: Point) -> bool {
        self.selected
            .and_then(|shape| self.shape_area(shape, Instant::now()))
            .is_some_and(|area| canvas::chip_contains(canvas::chip(area, self.width()), point))
    }

    fn bounds(&self) -> Rect {
        self.document.image().bounds()
    }

    /// The window's width, in DIPs.
    fn width(&self) -> f32 {
        self.size.0 as f32 / self.scale
    }

    fn dpi(&self) -> u32 {
        (self.scale * 96.0).round() as u32
    }

    fn invalidate(&self) {
        let _ = unsafe { InvalidateRect(Some(self.hwnd), None, false) };
    }
}
