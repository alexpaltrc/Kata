//! Shapes made of lines and curves, such as icons: built once in DIPs, then filled
//! anywhere, sharp at any scale.

use windows::{
    Win32::Graphics::Direct2D::{
        Common::{
            D2D_SIZE_F, D2D1_BEZIER_SEGMENT, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED,
            D2D1_FIGURE_END_OPEN, D2D1_FILL_MODE_WINDING,
        },
        D2D1_ARC_SEGMENT, D2D1_ARC_SIZE_LARGE, D2D1_ARC_SIZE_SMALL, D2D1_CAP_STYLE_ROUND,
        D2D1_COMBINE_MODE, D2D1_COMBINE_MODE_EXCLUDE, D2D1_COMBINE_MODE_UNION,
        D2D1_LINE_JOIN_ROUND, D2D1_STROKE_STYLE_PROPERTIES1, D2D1_SWEEP_DIRECTION_CLOCKWISE,
        D2D1_SWEEP_DIRECTION_COUNTER_CLOCKWISE, ID2D1Geometry, ID2D1GeometrySink,
    },
    core::{Interface, Result},
};
use windows_numerics::Vector2;

use crate::{
    Graphics,
    paint::{Area, Point},
};

/// How closely traced curves follow the outline, in DIPs: finer than a pixel at
/// any scale.
const TOLERANCE: f32 = 0.01;

/// An area bounded by outlines, filled where they wind around.
#[derive(Clone)]
pub struct Figure {
    pub(crate) geometry: ID2D1Geometry,
    graphics: Graphics,
}

impl Figure {
    /// The outlines `draw` makes with a pen.
    pub fn new(graphics: &Graphics, draw: impl FnOnce(&mut Pen)) -> Result<Self> {
        Self::build(graphics, |sink| {
            unsafe { sink.SetFillMode(D2D1_FILL_MODE_WINDING) };
            let mut pen = Pen {
                sink,
                drawing: false,
            };
            draw(&mut pen);
            pen.lift();
            Ok(())
        })
    }

    /// What a line `width` DIPs thick covers along the outlines, with round ends and
    /// corners.
    pub fn traced(&self, width: f32) -> Result<Self> {
        let round = D2D1_STROKE_STYLE_PROPERTIES1 {
            startCap: D2D1_CAP_STYLE_ROUND,
            endCap: D2D1_CAP_STYLE_ROUND,
            dashCap: D2D1_CAP_STYLE_ROUND,
            lineJoin: D2D1_LINE_JOIN_ROUND,
            ..Default::default()
        };
        let style = unsafe { self.graphics.d2d.CreateStrokeStyle(&round, None)? };
        Self::build(&self.graphics, |sink| unsafe {
            self.geometry.Widen(width, &style, None, TOLERANCE, sink)
        })
    }

    /// This figure and `other` together.
    pub fn and(&self, other: &Self) -> Result<Self> {
        self.combine(other, D2D1_COMBINE_MODE_UNION)
    }

    /// This figure with `other` cut out of it.
    pub fn without(&self, other: &Self) -> Result<Self> {
        self.combine(other, D2D1_COMBINE_MODE_EXCLUDE)
    }

    fn combine(&self, other: &Self, mode: D2D1_COMBINE_MODE) -> Result<Self> {
        Self::build(&self.graphics, |sink| unsafe {
            self.geometry
                .CombineWithGeometry(&other.geometry, mode, None, TOLERANCE, sink)
        })
    }

    fn build(
        graphics: &Graphics,
        write: impl FnOnce(&ID2D1GeometrySink) -> Result<()>,
    ) -> Result<Self> {
        unsafe {
            let path = graphics.d2d.CreatePathGeometry()?;
            let sink = path.Open()?;
            write(&sink)?;
            sink.Close()?;
            Ok(Self {
                geometry: path.cast()?,
                graphics: graphics.clone(),
            })
        }
    }
}

/// Draws a figure's outlines, one after another.
pub struct Pen<'a> {
    sink: &'a ID2D1GeometrySink,
    drawing: bool,
}

impl Pen<'_> {
    /// Starts an outline at `at`, leaving the last one open.
    pub fn start(&mut self, at: Point) {
        self.lift();
        unsafe { self.sink.BeginFigure(vector(at), D2D1_FIGURE_BEGIN_FILLED) };
        self.drawing = true;
    }

    pub fn line(&mut self, to: Point) {
        unsafe { self.sink.AddLine(vector(to)) };
    }

    /// Along a circle of `radius` to `to`: clockwise or not, and the long way round
    /// or the short one.
    pub fn arc(&mut self, to: Point, radius: f32, clockwise: bool, long: bool) {
        self.oval_arc(to, (radius, radius), clockwise, long);
    }

    /// Along an ellipse of `radii`, across and down, to `to`, as `arc` goes.
    fn oval_arc(&mut self, to: Point, (across, down): (f32, f32), clockwise: bool, long: bool) {
        unsafe {
            self.sink.AddArc(&D2D1_ARC_SEGMENT {
                point: vector(to),
                size: D2D_SIZE_F {
                    width: across,
                    height: down,
                },
                rotationAngle: 0.0,
                sweepDirection: if clockwise {
                    D2D1_SWEEP_DIRECTION_CLOCKWISE
                } else {
                    D2D1_SWEEP_DIRECTION_COUNTER_CLOCKWISE
                },
                arcSize: if long {
                    D2D1_ARC_SIZE_LARGE
                } else {
                    D2D1_ARC_SIZE_SMALL
                },
            });
        }
    }

    /// A cubic Bézier curve to `to`, pulled toward `a` and then `b`.
    pub fn curve(&mut self, a: Point, b: Point, to: Point) {
        unsafe {
            self.sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: vector(a),
                point2: vector(b),
                point3: vector(to),
            });
        }
    }

    /// Joins the outline back to where it started.
    pub fn close(&mut self) {
        if self.drawing {
            unsafe { self.sink.EndFigure(D2D1_FIGURE_END_CLOSED) };
            self.drawing = false;
        }
    }

    pub fn circle(&mut self, center: Point, radius: f32) {
        self.ellipse(center, (radius, radius));
    }

    /// An ellipse with `radii` across and down.
    pub fn ellipse(&mut self, (x, y): Point, radii: (f32, f32)) {
        self.start((x - radii.0, y));
        self.oval_arc((x + radii.0, y), radii, true, false);
        self.oval_arc((x - radii.0, y), radii, true, false);
        self.close();
    }

    /// A rectangle with corners of `radius`.
    pub fn rounded(&mut self, area: Area, radius: f32) {
        let (left, top, right, bottom) = (area.left, area.top, area.right(), area.bottom());
        self.start((left + radius, top));
        self.line((right - radius, top));
        self.arc((right, top + radius), radius, true, false);
        self.line((right, bottom - radius));
        self.arc((right - radius, bottom), radius, true, false);
        self.line((left + radius, bottom));
        self.arc((left, bottom - radius), radius, true, false);
        self.line((left, top + radius));
        self.arc((left + radius, top), radius, true, false);
        self.close();
    }

    fn lift(&mut self) {
        if self.drawing {
            unsafe { self.sink.EndFigure(D2D1_FIGURE_END_OPEN) };
            self.drawing = false;
        }
    }
}

fn vector((x, y): Point) -> Vector2 {
    Vector2::new(x, y)
}
