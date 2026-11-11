//! Another capture, taken from the editor. The button beside the dock starts one, as
//! the shortcut does. Dragged over the capture instead, it lights up where the new
//! capture would land, as one dragged from File Explorer does; let go there, the
//! editor gets out of the way, and the capture taken next lands in that place.

use std::{cell::Cell, sync::Arc};

use kata_editor::Image;
use kata_ui::paint::Point;
use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    UI::WindowsAndMessaging::{
        IsWindow, PostMessageW, SW_HIDE, SW_SHOW, SendMessageW, SetForegroundWindow, ShowWindow,
        WM_APP,
    },
};

use super::{Editor, toolbar::Part};
use crate::settings::Perk;

/// A capture was taken for the editor that asked for one to dock. Its `LPARAM`
/// points to the `Image`, which the editor copies if it takes it, answering
/// nonzero.
pub const ADD: u32 = WM_APP + 3;

thread_local! {
    /// The editor that got out of the way of a capture it asked for.
    static AWAITING: Cell<Option<HWND>> = const { Cell::new(None) };
}

/// Gives `image`, just captured, to the editor that asked for a capture to dock, if
/// one did. Returns whether it docked there.
pub fn take(image: &Image) -> bool {
    let Some(hwnd) = AWAITING.get() else {
        return false;
    };
    let image = LPARAM(std::ptr::from_ref(image) as isize);
    let taken = unsafe { SendMessageW(hwnd, ADD, Some(WPARAM(0)), Some(image)) }.0 != 0;
    if taken {
        AWAITING.set(None);
    }
    taken
}

/// Forgets that `hwnd`'s editor asked for a capture, as it closes: its handle may
/// go to another window.
pub fn forget(hwnd: HWND) {
    if AWAITING.get() == Some(hwnd) {
        AWAITING.set(None);
    }
}

/// Shows again the editor that got out of the way of a capture, when none came of
/// it or it went elsewhere.
pub fn resume() {
    if let Some(hwnd) = AWAITING.take()
        && unsafe { IsWindow(Some(hwnd)) }.as_bool()
    {
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

impl Editor {
    /// The button moved under the pointer that pressed it. Off the button, it shows
    /// where a new capture would land. Returns whether the editor's look changes.
    pub(super) fn adding_moved(&mut self, point: Point) -> bool {
        if self.placing {
            self.expected_at(point);
        } else if self.toolbar.control(Part::Add).contains(point) {
            return self.toolbar.control(Part::Add).drag(point);
        } else {
            self.placing = true;
            self.toolbar.control(Part::Add).cancel();
            self.expect(point);
        }
        true
    }

    /// The button was let go at `point`, off its place: over a place to land, a
    /// capture is taken to dock there.
    pub(super) fn adding_released(&mut self, point: Point) {
        self.placing = false;
        let docks = self.expected_at(point);
        self.unexpect();
        if !docks {
            return;
        }
        // Joining captures is Premium's, past its free tries.
        if Perk::Joining.allowed() {
            self.add_capture(Some(point));
        } else {
            self.offer_premium();
        }
    }

    /// Gives up placing a new capture.
    pub(super) fn adding_canceled(&mut self) {
        if std::mem::take(&mut self.placing) {
            self.unexpect();
        }
    }

    /// Starts a capture, the editor out of its way: to dock at `at` once taken, or
    /// to open on its own.
    pub(super) fn add_capture(&mut self, at: Option<Point>) {
        self.adding = at;
        AWAITING.set(Some(self.hwnd));
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
            let _ = PostMessageW(Some(self.owner), crate::CAPTURE, WPARAM(0), LPARAM(0));
        }
    }

    /// The capture asked for was taken, as `ADD` says. Returns whether it docked.
    pub(super) fn added(&mut self, lparam: LPARAM) -> bool {
        let Some(at) = self.adding.take() else {
            return false;
        };
        let image = unsafe { &*(lparam.0 as *const Image) }.clone();
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOW);
            let _ = SetForegroundWindow(self.hwnd);
        }
        self.dock_at(Arc::new(image), at)
    }

    /// Shows Premium, for something of its own that has no free tries left.
    pub(super) fn offer_premium(&self) {
        let _ = unsafe { PostMessageW(Some(self.owner), crate::OFFER, WPARAM(0), LPARAM(0)) };
    }
}
