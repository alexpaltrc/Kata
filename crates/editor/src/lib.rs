//! Editing document model: tools (blur, focus), undo history and autosave.
//!
//! Platform-independent by design, so it can be built and tested anywhere.

mod geometry;
mod image;

pub use geometry::Rect;
pub use image::Image;
