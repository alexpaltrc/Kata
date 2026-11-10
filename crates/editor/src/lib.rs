//! Editing document model: tools (blur, focus, shapes, steps), undo history, rendering, PNG
//! files and their names.
//!
//! Platform-independent by design, so it can be built and tested anywhere.

mod collage;
mod deflate;
mod document;
mod figure;
mod geometry;
mod image;
mod ink;
mod marker;
pub mod name;
mod parallel;
mod png;
mod render;

pub use collage::{Collage, Divider, Placement, Side, Slot};
pub use document::{
    BLUR_LEVELS, Blur, Document, Edits, Figure, Focus, Form, Frame, STEP_SIZES, Shape, Spot, Step,
    ZOOMS,
};
pub use geometry::Rect;
pub use image::Image;
pub use render::Renderer;
