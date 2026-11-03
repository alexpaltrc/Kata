//! Editing document model: tools (blur, focus, steps), undo history, rendering and PNG
//! files.
//!
//! Platform-independent by design, so it can be built and tested anywhere.

mod deflate;
mod document;
mod geometry;
mod image;
mod marker;
mod png;
mod render;

pub use document::{BLUR_LEVELS, Blur, Document, Edits, Focus, Shape, Spot, Step, ZOOMS};
pub use geometry::Rect;
pub use image::Image;
pub use marker::STEP_SIZE;
pub use render::{FOCUS_CORNER, Renderer};
