//! Editing document model: tools (blur, focus), undo history and rendering.
//!
//! Platform-independent by design, so it can be built and tested anywhere.

mod document;
mod geometry;
mod image;
mod render;

pub use document::{BLUR_LEVELS, Blur, Document, Edits, Focus, Shape, Spot, ZOOMS};
pub use geometry::Rect;
pub use image::Image;
pub use render::{FOCUS_CORNER, Renderer};
