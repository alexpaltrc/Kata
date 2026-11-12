//! The notice of a capture: the capture itself, as the card it's dragged as, comes
//! up in a corner of its screen for a moment. A click on it opens it in the
//! editor; left alone, it goes back down. It's Kata's own window, so it shows
//! whatever Windows is set to keep quiet, and takes the keyboard from no one.

use std::{
    cell::{Cell, RefCell},
    time::{Duration, Instant},
};

use kata_editor::{Image, Rect};
use kata_ui::{
    Graphics, Surface,
    motion::{self, Animation, Easing},
    paint::{Area, Point},
    theme::Palette,
    window::{self, WorkArea},
};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::{
            Direct2D::ID2D1Bitmap1,
            Gdi::{InvalidateRect, MONITOR_DEFAULTTONEAREST, MonitorFromRect, ValidateRect},
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Controls::WM_MOUSELEAVE,
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DestroyWindow, GWLP_USERDATA, HTCLIENT,
                HTTRANSPARENT, IDC_HAND, KillTimer, LoadCursorW, MA_NOACTIVATE, PostMessageW,
                RegisterClassExW, SW_SHOWNOACTIVATE, SetTimer, SetWindowLongPtrW, ShowWindow,
                WM_CLOSE, WM_ERASEBKGND, WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEMOVE,
                WM_NCDESTROY, WM_NCHITTEST, WM_PAINT, WM_TIMER, WNDCLASSEXW, WS_EX_NOACTIVATE,
                WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
            },
        },
    },
    core::{PCWSTR, Result, w},
};

use crate::{
    card::{self, Pose},
    channel, report,
};

const CLASS: PCWSTR = w!("Kata.Notice");
/// Between the card and the edges of its screen's corner, in DIPs.
const MARGIN: f32 = 16.0;
/// How far below its place the card starts, and how long it stays once up, and once
/// the pointer leaves it.
const RISE: f32 = 28.0;
const STAY: u32 = 5000;
const LINGER: u32 = 1500;
const LEAVE: usize = 1;
/// The glint that crosses it as it comes up: when, and for how long.
const GLINT_AFTER: Duration = Duration::from_millis(150);
const GLINT: Duration = Duration::from_millis(800);
/// How much it grows under the pointer.
const LIFT: f32 = 0.04;

thread_local! {
    /// The notice's window, while one shows.
    static OPEN: Cell<Option<HWND>> = const { Cell::new(None) };
}

/// Shows `image`, taken at `area` of the virtual screen, as a notice in the corner
/// of its screen. `owner` hears how it ends, as `crate::NOTICE`: with a nonzero
/// `WPARAM` if it was clicked. The notice already showing, if any, goes first.
pub fn show(graphics: &Graphics, owner: HWND, image: &Image, area: Rect) {
    close();
    match unsafe { create(graphics, owner, image, area) } {
        Ok(hwnd) => OPEN.set(Some(hwnd)),
        Err(error) => report("notice", &error),
    }
}

/// Puts the notice away at once, if one shows.
pub fn close() {
    if let Some(hwnd) = OPEN.take() {
        let _ = unsafe { DestroyWindow(hwnd) };
    }
}

unsafe fn create(graphics: &Graphics, owner: HWND, image: &Image, area: Rect) -> Result<HWND> {
    unsafe {
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
        // The card and the room its shadow takes, at the corner of the work area.
        let room = (
            card::CARD.0 + 2.0 * card::ROOM,
            card::CARD.1 + 2.0 * card::ROOM,
        );
        let size = (
            (room.0 * scale).ceil() as i32,
            (room.1 * scale).ceil() as i32,
        );
        let inset = ((MARGIN - card::ROOM) * scale).round() as i32;
        let instance = GetModuleHandleW(None)?.into();
        // Every notice after the first registers the class again, which fails
        // harmlessly.
        RegisterClassExW(&WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            hCursor: LoadCursorW(None, IDC_HAND)?,
            lpszClassName: CLASS,
            ..Default::default()
        });
        let hwnd = CreateWindowExW(
            WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            CLASS,
            &windows::core::HSTRING::from(channel::name()),
            WS_POPUP,
            work.rect.right - inset - size.0,
            work.rect.bottom - inset - size.1,
            size.0,
            size.1,
            None,
            None,
            Some(instance),
            None,
        )?;
        let made =
            Surface::new(graphics, hwnd, (size.0 as u32, size.1 as u32)).and_then(|surface| {
                let bitmap = card::bitmap(surface.context(), image)?;
                Ok((surface, bitmap))
            });
        let (surface, bitmap) = match made {
            Ok(made) => made,
            Err(error) => {
                let _ = DestroyWindow(hwnd);
                return Err(error);
            }
        };
        let shown = (image.width() as f32 / scale, image.height() as f32 / scale);
        let mut notice = Notice {
            hwnd,
            owner,
            palette: Palette::current(),
            scale,
            surface,
            bitmap,
            part: card::part(image.bounds()),
            pose: Pose::card(shown, (room.0 / 2.0, room.1 / 2.0)),
            up: Animation::new(0.0, 1.0, motion::NORMAL, Easing::SPRING),
            glint: Animation::new(0.0, 1.0, GLINT, Easing::SWEEP).delayed(GLINT_AFTER),
            lift: Animation::idle(0.0),
            hovered: false,
            clicked: false,
        };
        // The first frame is ready before the window shows.
        notice.paint();
        let state = Box::into_raw(Box::new(RefCell::new(notice)));
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        SetTimer(Some(hwnd), LEAVE, STAY, None);
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
        if message == WM_NCDESTROY {
            release(hwnd);
        }
        window::linked_state::<Notice>(hwnd, message, lparam)
            .and_then(|notice| notice.try_borrow_mut().ok())
            .and_then(|mut notice| notice.handle(message, wparam, lparam))
            .unwrap_or_else(|| DefWindowProcW(hwnd, message, wparam, lparam))
    }
}

/// Frees the notice as its window goes away, and tells its owner if no click
/// ended it.
unsafe fn release(hwnd: HWND) {
    let state = unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) } as *mut RefCell<Notice>;
    if !state.is_null() {
        let notice = unsafe { Box::from_raw(state) };
        if !notice.borrow().clicked {
            let gone = crate::NOTICE;
            let _ =
                unsafe { PostMessageW(Some(notice.borrow().owner), gone, WPARAM(0), LPARAM(0)) };
        }
    }
    if OPEN.get() == Some(hwnd) {
        OPEN.set(None);
    }
}

struct Notice {
    hwnd: HWND,
    /// Kata's resident window, which holds the capture.
    owner: HWND,
    palette: Palette,
    /// Pixels per DIP.
    scale: f32,
    surface: Surface,
    bitmap: ID2D1Bitmap1,
    /// The capture's pixels, and the card it shows as, once up.
    part: Area,
    pose: Pose,
    /// From 0, below its place and unseen, to 1, up; and back as it leaves.
    up: Animation,
    glint: Animation,
    /// From 0 to 1 as the pointer comes over it.
    lift: Animation,
    hovered: bool,
    clicked: bool,
}

impl Notice {
    fn handle(&mut self, message: u32, _wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        match message {
            WM_PAINT => self.paint(),
            // Every frame covers the whole window: erasing first would only flicker.
            WM_ERASEBKGND => return Some(LRESULT(1)),
            // Only the card is there to the pointer: around it, clicks go through.
            WM_NCHITTEST => {
                let (x, y) = window::hit_position(self.hwnd, lparam);
                let on = self.covers((x as f32 / self.scale, y as f32 / self.scale));
                let hit = if on { HTCLIENT } else { HTTRANSPARENT as u32 };
                return Some(LRESULT(hit as i32 as isize));
            }
            // It never takes the keyboard from what the user is doing.
            WM_MOUSEACTIVATE => return Some(LRESULT(MA_NOACTIVATE as isize)),
            WM_MOUSEMOVE => self.hover(true),
            WM_MOUSELEAVE => self.hover(false),
            WM_LBUTTONUP => {
                self.clicked = true;
                let clicked = crate::NOTICE;
                let _ = unsafe { PostMessageW(Some(self.owner), clicked, WPARAM(1), LPARAM(0)) };
                self.end();
            }
            WM_TIMER => {
                unsafe {
                    let _ = KillTimer(Some(self.hwnd), LEAVE);
                }
                self.up = (self.up).retarget(0.0, motion::FAST, Easing::ACCELERATE);
                self.invalidate();
            }
            _ => return None,
        }
        Some(LRESULT(0))
    }

    fn covers(&self, point: Point) -> bool {
        self.pose.area().contains(point)
    }

    /// The pointer came over the card, or left it: under it, the card stays.
    fn hover(&mut self, hovered: bool) {
        if hovered == self.hovered {
            return;
        }
        self.hovered = hovered;
        let lift = if hovered { 1.0 } else { 0.0 };
        self.lift = (self.lift).retarget(lift, motion::FAST, Easing::DECELERATE);
        unsafe {
            if hovered {
                window::track_mouse_leave(self.hwnd);
                let _ = KillTimer(Some(self.hwnd), LEAVE);
                // On its way down, it comes back up.
                self.up = self.up.retarget(1.0, motion::FAST, Easing::DECELERATE);
            } else {
                SetTimer(Some(self.hwnd), LEAVE, LINGER, None);
            }
        }
        self.invalidate();
    }

    fn paint(&mut self) {
        let _ = unsafe { ValidateRect(Some(self.hwnd), None) };
        let now = Instant::now();
        let up = self.up.value(now);
        let lift = self.lift.value(now);
        let glint = (self.glint.is_running(now)).then(|| self.glint.value(now));
        let pose = self.pose.moved((0.0, RISE * (1.0 - up)));
        let (bitmap, part) = (&self.bitmap, self.part);
        let drawn = self.surface.draw(&self.palette, self.scale, |painter| {
            painter.grown(up.clamp(0.0, 1.0), 1.0 + LIFT * lift, pose.center(), || {
                card::draw(painter, bitmap, part, pose, glint);
            });
        });
        if let Err(error) = drawn {
            report("notice", &error);
        }
        let moving =
            self.up.is_running(now) || self.lift.is_running(now) || self.glint.is_running(now);
        if moving {
            self.invalidate();
        } else if self.up.target() == 0.0 {
            // Down and out of sight: it's gone.
            self.end();
        }
    }

    /// Closes the window once this message is done with: its state is in use until
    /// then.
    fn end(&self) {
        let _ = unsafe { PostMessageW(Some(self.hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) };
    }

    fn invalidate(&self) {
        let _ = unsafe { InvalidateRect(Some(self.hwnd), None, false) };
    }
}
