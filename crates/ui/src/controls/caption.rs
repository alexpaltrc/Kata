//! A window's minimize, maximize and close buttons, drawn by Kata as tall as the
//! title bar they close, so what's in the bar lines up with them.
//!
//! Windows still knows them for what they are: each answers `WM_NCHITTEST` as the
//! button it is, so resting on Maximize brings up the snap layouts, and what they do
//! goes through `WM_SYSCOMMAND`, as with Windows' own.

use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    Graphics::Gdi::InvalidateRect,
    UI::WindowsAndMessaging::{
        HTCLOSE, HTMAXBUTTON, HTMINBUTTON, PostMessageW, SC_CLOSE, SC_MAXIMIZE, SC_MINIMIZE,
        SC_RESTORE, SIZE_MAXIMIZED, WA_INACTIVE, WM_ACTIVATE, WM_NCHITTEST, WM_NCLBUTTONDBLCLK,
        WM_NCLBUTTONDOWN, WM_NCLBUTTONUP, WM_NCMOUSELEAVE, WM_NCMOUSEMOVE, WM_SIZE, WM_SYSCOMMAND,
    },
};
use windows::core::Result;

use crate::{
    access::{self, Element, Role},
    paint::{Area, Fonts, Label, Painter, Point},
    theme::{color, faded},
    window,
};

/// Each button's width, as Windows' own.
const BUTTON: f32 = 46.0;
/// Segoe Fluent Icons draws these glyphs for this size.
const GLYPH: f32 = 10.0;
/// Behind Close under the pointer, as in every window.
const CLOSE: u32 = 0xC4_2B1C;
/// How much of its ink a window in the background keeps.
const INACTIVE: f32 = 0.5;
/// What assistive technologies number the first button; the others follow. No window
/// has this many controls of its own.
const FIRST: usize = usize::MAX - 2;

const MINIMIZE: usize = 0;
const MAXIMIZE: usize = 1;
const CLOSES: usize = 2;
/// What each button is to Windows.
const HITS: [u32; 3] = [HTMINBUTTON, HTMAXBUTTON, HTCLOSE];

pub struct Caption {
    /// Minimize, Maximize, Close, and Restore, which stands in for Maximize.
    glyphs: [Label; 4],
    names: [String; 4],
    area: Area,
    hovered: Option<usize>,
    pressed: Option<usize>,
    /// Whether Windows will say when the pointer leaves the buttons.
    tracking: bool,
    active: bool,
    maximized: bool,
}

impl Caption {
    /// How wide the three buttons are.
    pub const WIDTH: f32 = 3.0 * BUTTON;

    /// The buttons, called `names`: Minimize, Maximize, Close and Restore.
    pub fn new(fonts: &Fonts, names: [&str; 4]) -> Result<Self> {
        let glyph = |glyph: char| fonts.icon(glyph, GLYPH);
        Ok(Self {
            glyphs: [
                glyph('\u{E921}')?,
                glyph('\u{E922}')?,
                glyph('\u{E8BB}')?,
                glyph('\u{E923}')?,
            ],
            names: names.map(str::to_owned),
            area: Area::default(),
            hovered: None,
            pressed: None,
            tracking: false,
            active: true,
            maximized: false,
        })
    }

    /// Places the buttons at the end of a title bar `height` tall, in a client area
    /// `width` wide.
    pub fn layout(&mut self, width: f32, height: f32) {
        self.area = Area::new(width - Self::WIDTH, 0.0, Self::WIDTH, height);
    }

    /// Where the buttons start.
    pub fn left(&self) -> f32 {
        self.area.left
    }

    fn button(&self, index: usize) -> Area {
        let left = self.area.left + index as f32 * BUTTON;
        Area::new(left, self.area.top, BUTTON, self.area.height)
    }

    /// Which glyph and name `index` goes by now.
    fn face(&self, index: usize) -> usize {
        if index == MAXIMIZE && self.maximized {
            3
        } else {
            index
        }
    }

    /// Answers the messages that are the buttons' own. Call it first in the window's
    /// handler, and return its result when there is one.
    pub fn handle(
        &mut self,
        hwnd: HWND,
        scale: f32,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<LRESULT> {
        let button = || HITS.iter().position(|&hit| hit as usize == wparam.0);
        match message {
            WM_NCHITTEST => {
                let (x, y) = window::hit_position(hwnd, lparam);
                let point: Point = (x as f32 / scale, y as f32 / scale);
                let button = (0..3).find(|&index| self.button(index).contains(point))?;
                return Some(LRESULT(HITS[button] as isize));
            }
            WM_NCMOUSEMOVE => {
                let button = button();
                if button.is_some() && !self.tracking {
                    self.tracking = window::track_frame_leave(hwnd);
                }
                self.point_at(hwnd, button);
                button?;
            }
            WM_NCMOUSELEAVE => {
                self.tracking = false;
                self.pressed = None;
                self.point_at(hwnd, None);
                return None;
            }
            // Windows' own handling would draw its buttons of old.
            WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK => {
                self.pressed = Some(button()?);
                invalidate(hwnd);
            }
            WM_NCLBUTTONUP => {
                let button = button()?;
                if self.pressed.take() == Some(button) {
                    self.act(hwnd, button);
                }
                invalidate(hwnd);
            }
            WM_ACTIVATE => {
                self.active = wparam.0 as u32 & 0xffff != WA_INACTIVE;
                invalidate(hwnd);
                return None;
            }
            WM_SIZE => {
                self.maximized = wparam.0 == SIZE_MAXIMIZED as usize;
                return None;
            }
            access::ACT => {
                let button = wparam.0.checked_sub(FIRST)?;
                self.act(hwnd, button);
            }
            _ => return None,
        }
        Some(LRESULT(0))
    }

    /// The pointer is over `button`, or over none.
    fn point_at(&mut self, hwnd: HWND, button: Option<usize>) {
        if std::mem::replace(&mut self.hovered, button) != button {
            invalidate(hwnd);
        }
    }

    /// Does what `button` is for, once the window is done with this message: a
    /// window resized from inside its own handler would miss the news.
    fn act(&self, hwnd: HWND, button: usize) {
        let command = match button {
            MINIMIZE => SC_MINIMIZE,
            MAXIMIZE if self.maximized => SC_RESTORE,
            MAXIMIZE => SC_MAXIMIZE,
            _ => SC_CLOSE,
        };
        let command = WPARAM(command as usize);
        let _ = unsafe { PostMessageW(Some(hwnd), WM_SYSCOMMAND, command, LPARAM(0)) };
    }

    /// The buttons, as assistive technologies know them, after the window's own.
    pub fn describe(&self, elements: &mut Vec<Element>) {
        elements.extend((0..3).map(|index| Element {
            control: FIRST + index,
            name: self.names[self.face(index)].clone(),
            role: Role::Button,
            area: self.button(index),
            help: "",
        }));
    }

    pub fn draw(&self, painter: &Painter) {
        let palette = painter.palette;
        for index in 0..3 {
            let hovered = self.hovered == Some(index);
            let pressed = hovered && self.pressed == Some(index);
            let (fill, ink) = match (hovered, pressed) {
                (false, _) if self.active => (Default::default(), palette.text),
                (false, _) => (Default::default(), faded(palette.text, INACTIVE)),
                // A contrast theme marks what's under the pointer in its own colors.
                (true, _) if palette.contrast => (palette.accent, palette.on_accent),
                (true, false) if index == CLOSES => (color(CLOSE, 1.0), color(0xFF_FFFF, 1.0)),
                (true, true) if index == CLOSES => (color(CLOSE, 0.9), color(0xFF_FFFF, 0.7)),
                (true, false) => (palette.subtle_hover, palette.text),
                (true, true) => (palette.subtle_pressed, palette.text_secondary),
            };
            let area = painter.align(self.button(index));
            painter.fill(area, 0.0, fill);
            let glyph = &self.glyphs[self.face(index)];
            let left = area.left + (area.width - glyph.width) / 2.0;
            painter.text(glyph, left, area.middle(), ink);
        }
    }
}

fn invalidate(hwnd: HWND) {
    let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
}
