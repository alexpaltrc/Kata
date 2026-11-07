use crate::figure::{self, LINE, Trace};
use crate::geometry::{center, ellipse};
use crate::{Image, Rect, marker::STEP_SIZE};

/// How strongly each blur level hides what's under it: Soft, Medium, Strong.
pub const BLUR_LEVELS: usize = 3;
/// How much each focus level magnifies.
pub const ZOOMS: [f32; 4] = [1.0, 1.25, 1.5, 2.0];
/// How big each step size draws the markers, against the usual: Small, Medium, Large, Extra large.
pub const STEP_SIZES: [f32; 4] = [0.75, 1.0, 1.35, 1.8];
/// The least a usual marker spans of the capture's shorter side, so it still reads once
/// a big capture shrinks to fit a page.
const STEP_SHARE: f32 = 0.036;
/// A soft focus's corner radius at 100 %.
const FOCUS_CORNER: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Blur {
    pub rect: Rect,
    /// Below `BLUR_LEVELS`.
    pub level: usize,
}

/// How a focus's lit area is framed: softly, its corners rounded, or by a line of
/// Kata's ink around a rectangle or an ellipse.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Frame {
    #[default]
    Soft,
    Rectangle,
    Ellipse,
}

/// The one area that stays lit while the rest of the image darkens, and blurs if
/// asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Focus {
    pub rect: Rect,
    /// An index into `ZOOMS`.
    pub level: usize,
    /// The blur level around it, if it blurs what's around.
    pub blur: Option<usize>,
    pub frame: Frame,
}

impl Focus {
    pub fn spot(&self) -> Spot {
        Spot {
            rect: self.rect,
            zoom: ZOOMS[self.level],
            blur: self.blur,
            frame: self.frame,
        }
    }
}

/// A focus as drawn: the area it lights, how much it magnifies it, how it blurs
/// what's around and how it's framed. The zoom is free so it can animate between
/// levels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spot {
    pub rect: Rect,
    pub zoom: f32,
    pub blur: Option<usize>,
    pub frame: Frame,
}

impl Spot {
    /// Where the magnified area shows: centered on `rect`, moved inside `bounds`
    /// when it would stick out.
    pub fn lit(&self, bounds: Rect) -> Rect {
        let fit = |start: i32, len: i32, low: i32, high: i32| {
            let grown = ((len as f32 * self.zoom).round() as i32).min(high - low);
            let start = (start - (grown - len) / 2).clamp(low, high - grown);
            (start, start + grown)
        };
        let (left, right) = fit(self.rect.left, self.rect.width(), bounds.left, bounds.right);
        let (top, bottom) = fit(self.rect.top, self.rect.height(), bounds.top, bounds.bottom);
        Rect::new(left, top, right, bottom)
    }

    /// Whether pixel `x`, `y` is in the lit area: in its ellipse, when it's one.
    pub fn covers(&self, x: i32, y: i32, bounds: Rect) -> bool {
        let lit = self.lit(bounds);
        let point = (x as f32 + 0.5, y as f32 + 0.5);
        lit.contains(x, y)
            && (self.frame != Frame::Ellipse || ellipse(point, center(lit), radii(lit)) <= 0.0)
    }

    /// The line of Kata's ink around a framed focus, as thick as a figure's on a
    /// capture of `bounds` taken at display `scale`. Its outer edge is the lit area's,
    /// so that edge hides under it.
    pub fn border(&self, bounds: Rect, scale: f32) -> Option<Figure> {
        let form = match self.frame {
            Frame::Soft => return None,
            Frame::Rectangle => Form::Rectangle,
            Frame::Ellipse => Form::Ellipse,
        };
        let lit = self.lit(bounds);
        Some(Figure {
            form,
            from: (lit.left, lit.top),
            to: (lit.right - 1, lit.bottom - 1),
            line: Figure::line(bounds, scale),
        })
    }

    /// The radius of the lit area's corners, in pixels: as round as its border's, and
    /// none for an ellipse.
    pub fn corner(&self, bounds: Rect, scale: f32) -> f32 {
        match self.border(bounds, scale) {
            Some(border) => border.corner(),
            None => FOCUS_CORNER * scale,
        }
    }

    /// The area that, magnified by `zoom`, fills `lit`.
    pub fn source(lit: Rect, zoom: f32) -> Rect {
        let shrink = |start: i32, grown: i32| {
            let len = ((grown as f32 / zoom).round() as i32).max(1);
            let start = start + (grown - len) / 2;
            (start, start + len)
        };
        let (left, right) = shrink(lit.left, lit.width());
        let (top, bottom) = shrink(lit.top, lit.height());
        Rect::new(left, top, right, bottom)
    }
}

/// Half the width and height of `rect`.
pub(crate) fn radii(rect: Rect) -> (f32, f32) {
    (rect.width() as f32 / 2.0, rect.height() as f32 / 2.0)
}

/// A numbered marker that shows the order to follow. Steps count from 1, in the
/// order they were added.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    /// The square its disc fills.
    pub rect: Rect,
}

impl Step {
    /// A step `side` pixels wide, centered on `center` and moved inside `bounds`.
    pub fn new(center: (i32, i32), side: i32, bounds: Rect) -> Self {
        let (left, top) = (center.0 - side / 2, center.1 - side / 2);
        Self {
            rect: Rect::new(left, top, left + side, top + side).moved_inside(&bounds),
        }
    }

    /// How wide a step at `size`, an index into `STEP_SIZES`, is on a capture of
    /// `bounds` taken at display `scale`: as big as it'd look on that display, or
    /// bigger on a big capture.
    pub fn side(bounds: Rect, scale: f32, size: usize) -> i32 {
        let short = bounds.width().min(bounds.height());
        let usual = (STEP_SIZE * scale).max(short as f32 * STEP_SHARE);
        ((usual * STEP_SIZES[size]).round() as i32).clamp(1, short.max(1))
    }

    fn center(&self) -> (i32, i32) {
        (
            (self.rect.left + self.rect.right) / 2,
            (self.rect.top + self.rect.bottom) / 2,
        )
    }
}

/// What a figure draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    Rectangle,
    Ellipse,
    Arrow,
}

/// A rectangle, an ellipse or an arrow, drawn as a line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Figure {
    pub form: Form,
    /// The pixels the drag went from and to: an arrow's tail and head, or a
    /// rectangle's or an ellipse's opposite corners.
    pub from: (i32, i32),
    pub to: (i32, i32),
    /// How wide its line is, in pixels.
    pub line: i32,
}

impl Figure {
    /// How wide a figure's line is on a capture of `bounds` taken at display `scale`:
    /// in step with the usual marker.
    pub fn line(bounds: Rect, scale: f32) -> i32 {
        let usual = Step::side(bounds, scale, 1) as f32;
        (usual * LINE / STEP_SIZE).round().max(2.0) as i32
    }

    /// The pixels from one end to the other.
    pub fn rect(&self) -> Rect {
        let (left, top) = (self.from.0.min(self.to.0), self.from.1.min(self.to.1));
        let (right, bottom) = (self.from.0.max(self.to.0), self.from.1.max(self.to.1));
        Rect::new(left, top, right + 1, bottom + 1)
    }

    /// Moves the ends onto the corners of `rect` they were at, so an arrow keeps
    /// pointing the same way.
    pub fn fit(&mut self, rect: Rect) {
        let ends = |from: i32, to: i32, low: i32, high: i32| {
            if from <= to { (low, high) } else { (high, low) }
        };
        let (from_x, to_x) = ends(self.from.0, self.to.0, rect.left, rect.right - 1);
        let (from_y, to_y) = ends(self.from.1, self.to.1, rect.top, rect.bottom - 1);
        self.from = (from_x, from_y);
        self.to = (to_x, to_y);
    }

    /// The radius of its outer corners, in pixels: only a rectangle's are round.
    pub fn corner(&self) -> f32 {
        match self.form {
            Form::Rectangle => figure::outer(self).radius,
            Form::Ellipse | Form::Arrow => 0.0,
        }
    }

    /// Whether pixel `x`, `y` lies on the line, or up to `reach` pixels from it.
    pub fn touches(&self, x: i32, y: i32, reach: f32) -> bool {
        Trace::new(self).outside((x as f32 + 0.5, y as f32 + 0.5)) <= reach
    }
}

/// Everything the user added to the image.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Edits {
    pub blurs: Vec<Blur>,
    pub focus: Option<Focus>,
    pub figures: Vec<Figure>,
    pub steps: Vec<Step>,
}

/// One thing the user drew: a blur, a figure or a step, by its place in `Edits`, or
/// the focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Blur(usize),
    Focus,
    Figure(usize),
    Step(usize),
}

impl Edits {
    /// The area `shape` covers on an image of `bounds`, or `None` when it's gone.
    pub fn outline(&self, shape: Shape, bounds: Rect) -> Option<Rect> {
        match shape {
            Shape::Blur(index) => self.blurs.get(index).map(|blur| blur.rect),
            Shape::Focus => self.focus.map(|focus| focus.spot().lit(bounds)),
            Shape::Figure(index) => self.figures.get(index).map(Figure::rect),
            Shape::Step(index) => self.steps.get(index).map(|step| step.rect),
        }
    }

    /// The topmost shape under a pixel. The steps are drawn over the figures, the
    /// figures over the focus and the focus over the blurs; newer ones over older
    /// ones. A figure is under the pixel only along its line, up to `reach` pixels
    /// away, so what it surrounds stays reachable.
    pub fn shape_at(&self, x: i32, y: i32, reach: f32, bounds: Rect) -> Option<Shape> {
        let under = |shape: &Shape| {
            self.outline(*shape, bounds)
                .is_some_and(|rect| rect.contains(x, y))
        };
        let steps = (0..self.steps.len()).rev().map(Shape::Step).filter(under);
        let figures = (0..self.figures.len())
            .rev()
            .filter(|&index| self.figures[index].touches(x, y, reach))
            .map(Shape::Figure);
        let focus = self
            .focus
            .filter(|focus| focus.spot().covers(x, y, bounds))
            .map(|_| Shape::Focus);
        let blurs = (0..self.blurs.len()).rev().map(Shape::Blur).filter(under);
        steps.chain(figures).chain(focus).chain(blurs).next()
    }

    /// Makes `shape` cover `outline`, clipped to `bounds`.
    pub fn reshape(&mut self, shape: Shape, outline: Rect, bounds: Rect) {
        let Some(outline) = outline.intersect(&bounds) else {
            return;
        };
        match shape {
            Shape::Blur(index) => {
                if let Some(blur) = self.blurs.get_mut(index) {
                    blur.rect = outline;
                }
            }
            Shape::Focus => {
                if let Some(focus) = &mut self.focus {
                    focus.rect = Spot::source(outline, ZOOMS[focus.level]);
                }
            }
            Shape::Figure(index) => {
                if let Some(figure) = self.figures.get_mut(index) {
                    figure.fit(outline);
                }
            }
            Shape::Step(index) => {
                if let Some(step) = self.steps.get_mut(index) {
                    step.rect = outline;
                }
            }
        }
    }

    pub fn remove(&mut self, shape: Shape) {
        match shape {
            Shape::Blur(index) if index < self.blurs.len() => {
                self.blurs.remove(index);
            }
            Shape::Focus => self.focus = None,
            Shape::Figure(index) if index < self.figures.len() => {
                self.figures.remove(index);
            }
            Shape::Step(index) if index < self.steps.len() => {
                self.steps.remove(index);
            }
            Shape::Blur(_) | Shape::Figure(_) | Shape::Step(_) => {}
        }
    }

    /// Makes every step `side` pixels wide, about its center, inside `bounds`.
    pub fn resize_steps(&mut self, side: i32, bounds: Rect) {
        for step in &mut self.steps {
            *step = Step::new(step.center(), side, bounds);
        }
    }
}

/// A capture and its edits, with their undo history.
pub struct Document {
    image: Image,
    edits: Edits,
    undo: Vec<Edits>,
    redo: Vec<Edits>,
}

impl Document {
    pub fn new(image: Image) -> Self {
        Self {
            image,
            edits: Edits::default(),
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    /// The capture as taken.
    pub fn image(&self) -> &Image {
        &self.image
    }

    pub fn edits(&self) -> &Edits {
        &self.edits
    }

    /// Makes `edits` the current version. Returns whether anything changed.
    pub fn commit(&mut self, edits: Edits) -> bool {
        if edits == self.edits {
            return false;
        }
        self.undo.push(std::mem::replace(&mut self.edits, edits));
        self.redo.clear();
        true
    }

    /// Returns whether there was something to undo.
    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        self.redo.push(std::mem::replace(&mut self.edits, previous));
        true
    }

    /// Returns whether there was something to redo.
    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(std::mem::replace(&mut self.edits, next));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document() -> Document {
        Document::new(Image::new(1, 1, vec![0, 0, 0, 255]))
    }

    fn blurred(left: i32) -> Edits {
        Edits {
            blurs: vec![Blur {
                rect: Rect::new(left, 0, left + 10, 10),
                level: 1,
            }],
            focus: None,
            figures: Vec::new(),
            steps: Vec::new(),
        }
    }

    #[test]
    fn undo_and_redo_walk_the_history() {
        let mut document = document();
        assert!(document.commit(blurred(0)));
        assert!(document.commit(blurred(5)));
        assert!(document.undo());
        assert_eq!(document.edits(), &blurred(0));
        assert!(document.undo());
        assert_eq!(document.edits(), &Edits::default());
        assert!(!document.undo());
        assert!(document.redo());
        assert!(document.redo());
        assert_eq!(document.edits(), &blurred(5));
        assert!(!document.redo());
    }

    #[test]
    fn a_new_edit_drops_what_was_undone() {
        let mut document = document();
        document.commit(blurred(0));
        document.undo();
        document.commit(blurred(5));
        assert!(!document.redo());
    }

    #[test]
    fn unchanged_edits_add_no_history() {
        let mut document = document();
        assert!(!document.commit(Edits::default()));
        assert!(!document.undo());
    }

    #[test]
    fn lit_grows_around_the_center_and_stays_inside() {
        let bounds = Rect::new(0, 0, 100, 100);
        let spot = |rect, zoom| Spot {
            rect,
            zoom,
            blur: None,
            frame: Frame::Soft,
        };
        assert_eq!(
            spot(Rect::new(40, 40, 60, 60), 2.0).lit(bounds),
            Rect::new(30, 30, 70, 70)
        );
        assert_eq!(
            spot(Rect::new(0, 80, 20, 100), 2.0).lit(bounds),
            Rect::new(0, 60, 40, 100)
        );
        assert_eq!(spot(Rect::new(10, 10, 90, 90), 2.0).lit(bounds), bounds);
    }

    #[test]
    fn steps_are_on_top_then_the_focus_then_the_newest_blur() {
        let bounds = Rect::new(0, 0, 100, 100);
        let edits = Edits {
            blurs: vec![
                Blur {
                    rect: Rect::new(0, 0, 50, 50),
                    level: 0,
                },
                Blur {
                    rect: Rect::new(20, 20, 70, 70),
                    level: 0,
                },
            ],
            focus: Some(Focus {
                rect: Rect::new(40, 40, 50, 50),
                level: 3,
                blur: None,
                frame: Frame::Soft,
            }),
            figures: Vec::new(),
            steps: vec![Step {
                rect: Rect::new(44, 44, 60, 60),
            }],
        };
        assert_eq!(edits.shape_at(10, 10, 0.0, bounds), Some(Shape::Blur(0)));
        assert_eq!(edits.shape_at(30, 30, 0.0, bounds), Some(Shape::Blur(1)));
        // Doubled, the focus lights 35..55.
        assert_eq!(edits.shape_at(36, 36, 0.0, bounds), Some(Shape::Focus));
        assert_eq!(edits.shape_at(50, 50, 0.0, bounds), Some(Shape::Step(0)));
        assert_eq!(edits.shape_at(90, 90, 0.0, bounds), None);
    }

    #[test]
    fn an_ellipse_focus_is_reached_only_inside_its_ellipse() {
        let bounds = Rect::new(0, 0, 100, 100);
        let mut edits = Edits {
            focus: Some(Focus {
                rect: Rect::new(20, 20, 80, 60),
                level: 0,
                blur: None,
                frame: Frame::Ellipse,
            }),
            ..Edits::default()
        };
        assert_eq!(edits.shape_at(50, 21, 0.0, bounds), Some(Shape::Focus));
        assert_eq!(edits.shape_at(21, 21, 0.0, bounds), None);
        if let Some(focus) = &mut edits.focus {
            focus.frame = Frame::Rectangle;
        }
        assert_eq!(edits.shape_at(21, 21, 0.0, bounds), Some(Shape::Focus));
    }

    #[test]
    fn a_framed_focus_is_bordered_along_its_edge() {
        let bounds = Rect::new(0, 0, 400, 300);
        let spot = |frame| Spot {
            rect: Rect::new(40, 30, 200, 130),
            zoom: 1.0,
            blur: None,
            frame,
        };
        assert_eq!(spot(Frame::Soft).border(bounds, 1.0), None);
        for frame in [Frame::Rectangle, Frame::Ellipse] {
            let border = spot(frame).border(bounds, 1.0).unwrap();
            assert_eq!(border.rect(), spot(frame).lit(bounds));
            assert_eq!(border.line, Figure::line(bounds, 1.0));
        }
        // Its corners are as round as its border's, and an ellipse has none.
        let rectangle = spot(Frame::Rectangle);
        let border = rectangle.border(bounds, 1.0).unwrap();
        assert_eq!(rectangle.corner(bounds, 1.0), border.corner());
        assert_eq!(spot(Frame::Ellipse).corner(bounds, 1.0), 0.0);
        assert_eq!(spot(Frame::Soft).corner(bounds, 2.0), FOCUS_CORNER * 2.0);
    }

    #[test]
    fn reshaping_a_focus_lights_the_new_outline() {
        let bounds = Rect::new(0, 0, 200, 200);
        let mut edits = Edits {
            blurs: Vec::new(),
            focus: Some(Focus {
                rect: Rect::new(10, 10, 30, 30),
                level: 2,
                blur: Some(1),
                frame: Frame::Soft,
            }),
            figures: Vec::new(),
            steps: Vec::new(),
        };
        let outline = Rect::new(100, 60, 160, 120);
        edits.reshape(Shape::Focus, outline, bounds);
        assert_eq!(edits.outline(Shape::Focus, bounds), Some(outline));
        let focus = edits.focus.map(|focus| (focus.level, focus.blur));
        assert_eq!(focus, Some((2, Some(1))));
    }

    #[test]
    fn reshaping_clips_to_the_image() {
        let bounds = Rect::new(0, 0, 100, 100);
        let mut edits = blurred(0);
        edits.reshape(Shape::Blur(0), Rect::new(80, -10, 120, 30), bounds);
        assert_eq!(edits.blurs[0].rect, Rect::new(80, 0, 100, 30));
        edits.reshape(Shape::Blur(0), Rect::new(200, 0, 220, 30), bounds);
        assert_eq!(edits.blurs[0].rect, Rect::new(80, 0, 100, 30));
    }

    #[test]
    fn removing_drops_only_that_shape() {
        let mut edits = blurred(0);
        edits.focus = Some(Focus {
            rect: Rect::new(0, 0, 10, 10),
            level: 0,
            blur: None,
            frame: Frame::Soft,
        });
        edits.remove(Shape::Blur(3));
        edits.remove(Shape::Focus);
        assert_eq!(edits, blurred(0));
        edits.remove(Shape::Blur(0));
        assert_eq!(edits, Edits::default());
    }

    #[test]
    fn removing_a_step_renumbers_the_ones_after_it() {
        let step = |left| Step {
            rect: Rect::new(left, 0, left + 10, 10),
        };
        let mut edits = Edits {
            steps: vec![step(0), step(20), step(40)],
            ..Edits::default()
        };
        edits.remove(Shape::Step(1));
        assert_eq!(edits.steps, [step(0), step(40)]);
        assert_eq!(
            edits.shape_at(45, 5, 0.0, Rect::new(0, 0, 100, 100)),
            Some(Shape::Step(1))
        );
    }

    #[test]
    fn a_figure_is_under_its_line_but_not_what_it_surrounds() {
        let bounds = Rect::new(0, 0, 100, 100);
        let edits = Edits {
            blurs: vec![Blur {
                rect: bounds,
                level: 0,
            }],
            figures: vec![Figure {
                form: Form::Rectangle,
                from: (10, 10),
                to: (89, 89),
                line: 6,
            }],
            ..Edits::default()
        };
        assert_eq!(edits.shape_at(12, 50, 0.0, bounds), Some(Shape::Figure(0)));
        assert_eq!(edits.shape_at(50, 50, 0.0, bounds), Some(Shape::Blur(0)));
        assert_eq!(edits.shape_at(20, 50, 0.0, bounds), Some(Shape::Blur(0)));
        assert_eq!(edits.shape_at(20, 50, 5.0, bounds), Some(Shape::Figure(0)));
    }

    #[test]
    fn a_moved_arrow_keeps_pointing_the_same_way() {
        let mut arrow = Figure {
            form: Form::Arrow,
            from: (50, 10),
            to: (10, 40),
            line: 6,
        };
        assert_eq!(arrow.rect(), Rect::new(10, 10, 51, 41));
        arrow.fit(arrow.rect().offset(5, 20));
        assert_eq!((arrow.from, arrow.to), ((55, 30), (15, 60)));
    }

    #[test]
    fn figure_lines_grow_like_the_steps() {
        let window = Rect::new(0, 0, 640, 480);
        assert_eq!(Figure::line(window, 1.0), 6);
        assert_eq!(Figure::line(window, 1.5), 9);
        assert_eq!(Figure::line(Rect::new(0, 0, 3840, 2160), 1.5), 17);
    }

    #[test]
    fn steps_grow_with_the_display_and_with_big_captures() {
        let window = Rect::new(0, 0, 640, 480);
        assert_eq!(Step::side(window, 1.0, 1), 28);
        assert_eq!(Step::side(window, 1.5, 1), 42);
        assert_eq!(Step::side(window, 1.0, 0), 21);
        let screen = Rect::new(0, 0, 3840, 2160);
        assert_eq!(Step::side(screen, 1.5, 1), 78);
        assert_eq!(Step::side(screen, 1.5, 2), 105);
        assert_eq!(Step::side(Rect::new(0, 0, 400, 12), 1.0, 2), 12);
    }

    #[test]
    fn resizing_the_steps_keeps_their_centers_inside() {
        let bounds = Rect::new(0, 0, 100, 100);
        let mut edits = Edits {
            steps: vec![
                Step::new((50, 50), 20, bounds),
                Step::new((5, 5), 10, bounds),
            ],
            ..Edits::default()
        };
        edits.resize_steps(30, bounds);
        assert_eq!(
            edits.steps,
            [
                Step::new((50, 50), 30, bounds),
                Step {
                    rect: Rect::new(0, 0, 30, 30)
                },
            ]
        );
    }

    #[test]
    fn source_undoes_lit() {
        let rect = Rect::new(40, 30, 60, 70);
        let lit = Spot {
            rect,
            zoom: 1.5,
            blur: None,
            frame: Frame::Soft,
        }
        .lit(Rect::new(0, 0, 200, 200));
        assert_eq!(Spot::source(lit, 1.5), rect);
    }
}
