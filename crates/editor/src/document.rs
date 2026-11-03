use crate::{Image, Rect};

/// How strongly each blur level hides what's under it: Soft, Medium, Strong.
pub const BLUR_LEVELS: usize = 3;
/// How much each focus level magnifies.
pub const ZOOMS: [f32; 4] = [1.0, 1.25, 1.5, 2.0];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Blur {
    pub rect: Rect,
    /// Below `BLUR_LEVELS`.
    pub level: usize,
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
}

impl Focus {
    pub fn spot(&self) -> Spot {
        Spot {
            rect: self.rect,
            zoom: ZOOMS[self.level],
            blur: self.blur,
        }
    }
}

/// A focus as drawn: the area it lights, how much it magnifies it and how it blurs
/// what's around. The zoom is free so it can animate between levels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spot {
    pub rect: Rect,
    pub zoom: f32,
    pub blur: Option<usize>,
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

/// A numbered marker that shows the order to follow. Steps count from 1, in the
/// order they were added.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    /// The square its disc fills.
    pub rect: Rect,
}

/// Everything the user added to the image.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Edits {
    pub blurs: Vec<Blur>,
    pub focus: Option<Focus>,
    pub steps: Vec<Step>,
}

/// One thing the user drew: a blur or a step, by its place in `Edits`, or the focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Blur(usize),
    Focus,
    Step(usize),
}

impl Edits {
    /// The area `shape` covers on an image of `bounds`, or `None` when it's gone.
    pub fn outline(&self, shape: Shape, bounds: Rect) -> Option<Rect> {
        match shape {
            Shape::Blur(index) => self.blurs.get(index).map(|blur| blur.rect),
            Shape::Focus => self.focus.map(|focus| focus.spot().lit(bounds)),
            Shape::Step(index) => self.steps.get(index).map(|step| step.rect),
        }
    }

    /// The topmost shape under a pixel. The steps are drawn over the focus, and the
    /// focus over the blurs; newer steps and blurs over older ones.
    pub fn shape_at(&self, x: i32, y: i32, bounds: Rect) -> Option<Shape> {
        let under = |shape| {
            self.outline(shape, bounds)
                .is_some_and(|rect| rect.contains(x, y))
        };
        if let Some(step) = (0..self.steps.len())
            .rev()
            .map(Shape::Step)
            .find(|&shape| under(shape))
        {
            return Some(step);
        }
        if under(Shape::Focus) {
            return Some(Shape::Focus);
        }
        (0..self.blurs.len())
            .rev()
            .map(Shape::Blur)
            .find(|&shape| under(shape))
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
            Shape::Step(index) if index < self.steps.len() => {
                self.steps.remove(index);
            }
            Shape::Blur(_) | Shape::Step(_) => {}
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
            }),
            steps: vec![Step {
                rect: Rect::new(44, 44, 60, 60),
            }],
        };
        assert_eq!(edits.shape_at(10, 10, bounds), Some(Shape::Blur(0)));
        assert_eq!(edits.shape_at(30, 30, bounds), Some(Shape::Blur(1)));
        // Doubled, the focus lights 35..55.
        assert_eq!(edits.shape_at(36, 36, bounds), Some(Shape::Focus));
        assert_eq!(edits.shape_at(50, 50, bounds), Some(Shape::Step(0)));
        assert_eq!(edits.shape_at(90, 90, bounds), None);
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
            }),
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
            edits.shape_at(45, 5, Rect::new(0, 0, 100, 100)),
            Some(Shape::Step(1))
        );
    }

    #[test]
    fn source_undoes_lit() {
        let rect = Rect::new(40, 30, 60, 70);
        let lit = Spot {
            rect,
            zoom: 1.5,
            blur: None,
        }
        .lit(Rect::new(0, 0, 200, 200));
        assert_eq!(Spot::source(lit, 1.5), rect);
    }
}
