//! Captures put together in one frame, as windows snap side by side: the frame keeps
//! the first capture's size, and each capture docked takes half of a space. Every
//! capture fills its space, zoomed in as far as it must and framed where it's asked,
//! with a thin white seam between them.

use std::sync::Arc;

use crate::{Image, Rect};

/// Which side of a space another capture docks against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Top,
    Right,
    Bottom,
}

impl Side {
    /// The side of `rect` that `x`, `y` is nearest to, as its diagonals split it.
    pub fn nearest(rect: Rect, x: f32, y: f32) -> Self {
        let half = (rect.width() as f32 / 2.0, rect.height() as f32 / 2.0);
        let across = (x - rect.left as f32 - half.0) / half.0.max(1.0);
        let down = (y - rect.top as f32 - half.1) / half.1.max(1.0);
        match (across.abs() >= down.abs(), across < 0.0, down < 0.0) {
            (true, true, _) => Self::Left,
            (true, false, _) => Self::Right,
            (false, _, true) => Self::Top,
            (false, _, false) => Self::Bottom,
        }
    }

    /// Whether it docks beside, rather than above or below.
    const fn across(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }

    /// Whether it goes first: to the left, or above.
    const fn first(self) -> bool {
        matches!(self, Self::Left | Self::Top)
    }
}

/// Captures docked together. Each piece's image is its own: no two share one, so a
/// piece is known by its image wherever it moves.
#[derive(Clone, Debug, PartialEq)]
pub struct Collage {
    /// Pixels across and down: the first capture's, or what was kept of it.
    size: (i32, i32),
    /// The white between two spaces, in pixels.
    gap: i32,
    tree: Tree,
}

#[derive(Clone, Debug, PartialEq)]
enum Tree {
    Piece(Piece),
    /// Two spaces side by side when `across`, stacked otherwise; the first takes
    /// `ratio` of the room the gap leaves.
    Split {
        across: bool,
        ratio: f32,
        parts: Box<[Tree; 2]>,
    },
}

#[derive(Clone, Debug, PartialEq)]
struct Piece {
    image: Arc<Image>,
    /// How far past filling its space it's zoomed in.
    zoom: f32,
    /// The point of the image framed at the middle of its space, as shares of its
    /// width and height.
    center: (f32, f32),
}

/// Where a piece shows in the collage.
#[derive(Clone, Debug)]
pub struct Slot {
    pub image: Arc<Image>,
    /// Its space.
    pub rect: Rect,
    /// The area around it that's its own, its half of each gap included: every
    /// pixel of the collage is in one piece's cell.
    pub cell: Rect,
    pub zoom: f32,
    /// The point of the image framed at the middle of its space, as near to where
    /// it was asked as filling the space allows.
    pub center: (f32, f32),
    pub placement: Placement,
}

impl Slot {
    /// Whether it shows `image`.
    pub fn shows(&self, image: &Arc<Image>) -> bool {
        Arc::ptr_eq(&self.image, image)
    }
}

/// Where an image shows: scaled by `scale`, its top-left corner at `origin`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub scale: f32,
    pub origin: (f32, f32),
}

impl Placement {
    /// How `image` fills `rect`: zoomed `zoom` past covering it, with the point at
    /// `center`, in shares of its size, in the middle, or as near as it can be while
    /// it still fills the space. Pixel for pixel, it lands on whole pixels.
    pub fn fill(rect: Rect, image: &Image, zoom: f32, center: (f32, f32)) -> Self {
        let (width, height) = (image.width() as f32, image.height() as f32);
        let room = (rect.width() as f32, rect.height() as f32);
        let mut scale = (room.0 / width).max(room.1 / height) * zoom.clamp(1.0, Collage::MOST_ZOOM);
        let snap = (scale - 1.0).abs() < 1e-4;
        if snap {
            scale = 1.0;
        }
        let axis = |start: i32, room: f32, size: f32, center: f32| {
            let start = start as f32;
            let lowest = (start + room - size * scale).min(start);
            let origin = (start + room / 2.0 - center * size * scale).clamp(lowest, start);
            if snap { origin.round() } else { origin }
        };
        Self {
            scale,
            origin: (
                axis(rect.left, room.0, width, center.0),
                axis(rect.top, room.1, height, center.1),
            ),
        }
    }

    /// Where point `(x, y)` of the image shows.
    pub fn show(&self, (x, y): (f32, f32)) -> (f32, f32) {
        (
            self.origin.0 + x * self.scale,
            self.origin.1 + y * self.scale,
        )
    }

    /// The point of the image that shows at `(x, y)`.
    pub fn source(&self, (x, y): (f32, f32)) -> (f32, f32) {
        (
            (x - self.origin.0) / self.scale,
            (y - self.origin.1) / self.scale,
        )
    }

    /// The point of `image` at the middle of `rect`, as shares of its size.
    fn center(&self, rect: Rect, image: &Image) -> (f32, f32) {
        let (x, y) = self.source((
            (rect.left + rect.right) as f32 / 2.0,
            (rect.top + rect.bottom) as f32 / 2.0,
        ));
        (x / image.width() as f32, y / image.height() as f32)
    }
}

/// The white between two spaces, which moves to share their room differently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Divider {
    /// Whether the spaces are side by side, so it runs down between them.
    pub across: bool,
    /// The white itself.
    pub strip: Rect,
    /// The room the two spaces share, the white included.
    pub room: Rect,
}

impl Divider {
    /// The share of the room the first space takes with the white's middle at
    /// `at`, along the way it moves, kept to what `Collage::divided` allows.
    pub fn ratio(&self, at: f32) -> f32 {
        let (start, length, gap) = if self.across {
            (self.room.left, self.room.width(), self.strip.width())
        } else {
            (self.room.top, self.room.height(), self.strip.height())
        };
        let room = (length - gap).max(1) as f32;
        let ratio = (at - start as f32 - gap as f32 / 2.0) / room;
        ratio.clamp(Collage::LEAST_SHARE, 1.0 - Collage::LEAST_SHARE)
    }
}

impl Collage {
    /// The most a piece zooms in past filling its space.
    pub const MOST_ZOOM: f32 = 5.0;
    /// The least share of its room a space keeps.
    pub const LEAST_SHARE: f32 = 0.15;

    /// Just `image`, whole.
    pub fn new(image: Arc<Image>) -> Self {
        Self {
            size: (image.width() as i32, image.height() as i32),
            gap: 0,
            tree: Tree::Piece(Piece::whole(image)),
        }
    }

    /// How many pixels across and down it takes.
    pub fn size(&self) -> (i32, i32) {
        self.size
    }

    pub fn bounds(&self) -> Rect {
        Rect::new(0, 0, self.size.0, self.size.1)
    }

    /// Where each piece shows, in order from the top-left.
    pub fn slots(&self) -> Vec<Slot> {
        self.layout().0
    }

    /// The white between the spaces, outermost first.
    pub fn dividers(&self) -> Vec<Divider> {
        self.layout().1
    }

    fn layout(&self) -> (Vec<Slot>, Vec<Divider>) {
        let (mut slots, mut dividers) = (Vec::new(), Vec::new());
        let bounds = self.bounds();
        (self.tree).lay(bounds, bounds, self.gap, &mut slots, &mut dividers);
        (slots, dividers)
    }

    /// The piece whose cell holds pixel `x`, `y`, or the nearest one.
    pub fn owner(&self, x: i32, y: i32) -> usize {
        let bounds = self.bounds();
        let (x, y) = (
            x.clamp(bounds.left, bounds.right - 1),
            y.clamp(bounds.top, bounds.bottom - 1),
        );
        self.slots()
            .iter()
            .position(|slot| slot.cell.contains(x, y))
            .unwrap_or_default()
    }

    /// All in one image: white, with each piece filling its space. A single piece
    /// shown whole, pixel for pixel, is its own image.
    pub fn compose(&self) -> Arc<Image> {
        let slots = self.slots();
        if let [slot] = slots.as_slice()
            && slot.rect == slot.image.bounds()
            && slot.placement.scale == 1.0
            && slot.placement.origin == (0.0, 0.0)
        {
            return slot.image.clone();
        }
        let (width, height) = self.size;
        let mut composed = Image::new(
            width as u32,
            height as u32,
            vec![255; width as usize * height as usize * Image::BYTES_PER_PIXEL],
        );
        for slot in &slots {
            let Placement { scale, origin } = slot.placement;
            if scale == 1.0 {
                let at = (origin.0 as i32, origin.1 as i32);
                composed.paste(&slot.image, at, slot.rect);
            } else {
                composed.paste_scaled(&slot.image, scale, origin, slot.rect);
            }
        }
        Arc::new(composed)
    }

    /// With `image` docked against the `side` of the space of the `target`th piece,
    /// or of them all with none, taking half of it, `gap` pixels between spaces.
    pub fn docked(&self, image: Arc<Image>, target: Option<usize>, side: Side, gap: i32) -> Self {
        let piece = Tree::Piece(Piece::whole(image));
        let mut tree = self.tree.clone();
        match target {
            Some(target) => {
                if let Some(leaf) = tree.leaf(&mut { target }) {
                    *leaf = leaf.clone().beside(piece, side);
                }
            }
            None => tree = tree.beside(piece, side),
        }
        Self {
            size: self.size,
            gap,
            tree,
        }
    }

    /// With the `index`th divider giving its first space `ratio` of their room, kept
    /// between `LEAST_SHARE` and what's left of it.
    pub fn divided(&self, index: usize, ratio: f32) -> Self {
        let mut collage = self.clone();
        if let Some(share) = collage.tree.split(&mut { index }) {
            *share = ratio.clamp(Self::LEAST_SHARE, 1.0 - Self::LEAST_SHARE);
        }
        collage
    }

    /// With the `index`th piece zoomed `zoom` past filling its space, from 1 to
    /// `MOST_ZOOM`, and the point at `center` framed in the middle, or as near as it
    /// can be.
    pub fn reframed(&self, index: usize, zoom: f32, center: (f32, f32)) -> Self {
        let Some(slot) = self.slots().into_iter().nth(index) else {
            return self.clone();
        };
        let zoom = zoom.clamp(1.0, Self::MOST_ZOOM);
        let placement = Placement::fill(slot.rect, &slot.image, zoom, center);
        let center = placement.center(slot.rect, &slot.image);
        let mut collage = self.clone();
        if let Some(Tree::Piece(piece)) = collage.tree.leaf(&mut { index }) {
            piece.zoom = zoom;
            piece.center = center;
        }
        collage
    }

    /// With the `index`th piece moved the least it must to show `area` of its image,
    /// in its pixels as left, top, right and bottom, or centered on it when it
    /// can't show it all.
    pub fn showing(&self, index: usize, area: [f32; 4]) -> Self {
        let Some(slot) = self.slots().into_iter().nth(index) else {
            return self.clone();
        };
        let scale = slot.placement.scale;
        let axis = |center: f32, room: i32, size: u32, low: f32, high: f32| {
            let size = size as f32;
            let half = room as f32 / scale / size / 2.0;
            let (low, high) = (low / size, high / size);
            let (from, to) = (high - half, low + half);
            if from <= to {
                center.clamp(from, to)
            } else {
                (low + high) / 2.0
            }
        };
        let [left, top, right, bottom] = area;
        let center = (
            axis(
                slot.center.0,
                slot.rect.width(),
                slot.image.width(),
                left,
                right,
            ),
            axis(
                slot.center.1,
                slot.rect.height(),
                slot.image.height(),
                top,
                bottom,
            ),
        );
        self.reframed(index, slot.zoom, center)
    }

    /// Only what shows inside `kept`, as a frame of its own: each piece keeps
    /// showing as it did, and the spaces wholly outside go. `None` when none is left.
    pub fn cropped(&self, kept: Rect) -> Option<Self> {
        let kept = kept.intersect(&self.bounds())?;
        let slots = self.slots();
        let tree = (self.tree).cropped(self.bounds(), self.gap, kept, &slots, &mut 0)?;
        Some(Self {
            size: (kept.width(), kept.height()),
            gap: self.gap,
            tree,
        })
    }
}

impl Piece {
    fn whole(image: Arc<Image>) -> Self {
        Self {
            image,
            zoom: 1.0,
            center: (0.5, 0.5),
        }
    }
}

/// `room` split in two with `gap` pixels between them, the first taking `ratio` of
/// what the gap leaves: the two spaces and the gap.
fn split(room: Rect, across: bool, ratio: f32, gap: i32) -> (Rect, Rect, Rect) {
    let (start, length) = if across {
        (room.left, room.width())
    } else {
        (room.top, room.height())
    };
    let left = (length - gap).max(0);
    let first = ((left as f32 * ratio).round() as i32).clamp(left.min(1), (left - 1).max(0));
    let (end, next) = (start + first, start + first + gap);
    if across {
        (
            Rect::new(room.left, room.top, end, room.bottom),
            Rect::new(next, room.top, room.right, room.bottom),
            Rect::new(end, room.top, next, room.bottom),
        )
    } else {
        (
            Rect::new(room.left, room.top, room.right, end),
            Rect::new(room.left, next, room.right, room.bottom),
            Rect::new(room.left, end, room.right, next),
        )
    }
}

impl Tree {
    /// Lays the pieces out in `room`, each owning its share of `cell`.
    fn lay(
        &self,
        room: Rect,
        cell: Rect,
        gap: i32,
        slots: &mut Vec<Slot>,
        dividers: &mut Vec<Divider>,
    ) {
        match self {
            Self::Piece(piece) => {
                let placement = Placement::fill(room, &piece.image, piece.zoom, piece.center);
                slots.push(Slot {
                    image: piece.image.clone(),
                    rect: room,
                    cell,
                    zoom: piece.zoom,
                    center: placement.center(room, &piece.image),
                    placement,
                });
            }
            Self::Split {
                across,
                ratio,
                parts,
            } => {
                let (first, second, strip) = split(room, *across, *ratio, gap);
                dividers.push(Divider {
                    across: *across,
                    strip,
                    room,
                });
                // The gap splits down the middle.
                let (cells, middle) = if *across {
                    let middle = (strip.left + strip.right) / 2;
                    let first = Rect::new(cell.left, cell.top, middle, cell.bottom);
                    (first, Rect::new(middle, cell.top, cell.right, cell.bottom))
                } else {
                    let middle = (strip.top + strip.bottom) / 2;
                    let first = Rect::new(cell.left, cell.top, cell.right, middle);
                    (first, Rect::new(cell.left, middle, cell.right, cell.bottom))
                };
                parts[0].lay(first, cells, gap, slots, dividers);
                parts[1].lay(second, middle, gap, slots, dividers);
            }
        }
    }

    /// The `index`th piece, counting down as pieces go by.
    fn leaf(&mut self, index: &mut usize) -> Option<&mut Self> {
        match self {
            Self::Piece(_) => {
                if *index == 0 {
                    return Some(self);
                }
                *index -= 1;
                None
            }
            Self::Split { parts, .. } => {
                let [first, second] = &mut **parts;
                first.leaf(index).or_else(|| second.leaf(index))
            }
        }
    }

    /// The share of the `index`th split, outermost first, counting down as splits go
    /// by.
    fn split(&mut self, index: &mut usize) -> Option<&mut f32> {
        let Self::Split { ratio, parts, .. } = self else {
            return None;
        };
        if *index == 0 {
            return Some(ratio);
        }
        *index -= 1;
        let [first, second] = &mut **parts;
        first.split(index).or_else(|| second.split(index))
    }

    /// This and `piece` sharing its space, `piece` on its `side`.
    fn beside(self, piece: Self, side: Side) -> Self {
        let parts = if side.first() {
            [piece, self]
        } else {
            [self, piece]
        };
        Self::Split {
            across: side.across(),
            ratio: 0.5,
            parts: Box::new(parts),
        }
    }

    /// What's left inside `kept` of this, laid out in `room`, the pieces counted by
    /// `index` among `slots`, in the frame `kept` makes.
    fn cropped(
        &self,
        room: Rect,
        gap: i32,
        kept: Rect,
        slots: &[Slot],
        index: &mut usize,
    ) -> Option<Self> {
        match self {
            Self::Piece(piece) => {
                let slot = &slots[*index];
                *index += 1;
                let shown = slot.rect.intersect(&kept)?;
                let rect = shown.offset(-kept.left, -kept.top);
                let image = &piece.image;
                let cover = (rect.width() as f32 / image.width() as f32)
                    .max(rect.height() as f32 / image.height() as f32);
                Some(Self::Piece(Piece {
                    image: image.clone(),
                    zoom: (slot.placement.scale / cover).max(1.0),
                    center: slot.placement.center(shown, image),
                }))
            }
            Self::Split {
                across,
                ratio,
                parts,
            } => {
                let (first, second, _) = split(room, *across, *ratio, gap);
                let kept_first = parts[0].cropped(first, gap, kept, slots, index);
                let kept_second = parts[1].cropped(second, gap, kept, slots, index);
                let (kept_first, kept_second) = match (kept_first, kept_second) {
                    (Some(a), Some(b)) => (a, b),
                    (a, b) => return a.or(b),
                };
                let length = |rect: Option<Rect>| {
                    rect.map_or(0, |rect| if *across { rect.width() } else { rect.height() })
                };
                let shared = (length(room.intersect(&kept)) - gap).max(1);
                Some(Self::Split {
                    across: *across,
                    ratio: length(first.intersect(&kept)) as f32 / shared as f32,
                    parts: Box::new([kept_first, kept_second]),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An image `width` by `height`, every pixel `shade`.
    fn image(width: u32, height: u32, shade: u8) -> Arc<Image> {
        let pixels = [shade, shade, shade, 255].repeat((width * height) as usize);
        Arc::new(Image::new(width, height, pixels))
    }

    /// An image `width` by `height` whose blue channel is each pixel's column.
    fn columns(width: u32, height: u32) -> Arc<Image> {
        let row: Vec<u8> = (0..width).flat_map(|x| [x as u8, 0, 0, 255]).collect();
        Arc::new(Image::new(width, height, row.repeat(height as usize)))
    }

    fn rects(collage: &Collage) -> Vec<Rect> {
        collage.slots().iter().map(|slot| slot.rect).collect()
    }

    #[test]
    fn one_capture_is_its_own_image() {
        let alone = image(30, 20, 9);
        let collage = Collage::new(alone.clone());
        assert!(Arc::ptr_eq(&collage.compose(), &alone));
        assert_eq!(collage.bounds(), Rect::new(0, 0, 30, 20));
        assert!(collage.dividers().is_empty());
    }

    #[test]
    fn docking_takes_half_of_the_space_and_the_frame_stays() {
        let collage =
            Collage::new(image(100, 60, 10)).docked(image(50, 30, 20), Some(0), Side::Right, 4);
        assert_eq!(collage.size(), (100, 60));
        assert_eq!(
            rects(&collage),
            [Rect::new(0, 0, 48, 60), Rect::new(52, 0, 100, 60)]
        );
        let composed = collage.compose();
        let shade = |x, y| composed.pixels()[composed.offset(x, y)];
        // Each fills its space, and the gap is white.
        assert_eq!((shade(0, 0), shade(47, 59)), (10, 10));
        assert_eq!((shade(52, 0), shade(99, 59)), (20, 20));
        assert_eq!((shade(48, 30), shade(51, 30)), (255, 255));
    }

    #[test]
    fn docking_on_the_left_or_above_goes_first() {
        let left =
            Collage::new(image(40, 30, 10)).docked(image(20, 30, 20), Some(0), Side::Left, 0);
        assert_eq!(
            rects(&left),
            [Rect::new(0, 0, 20, 30), Rect::new(20, 0, 40, 30)]
        );
        let above =
            Collage::new(image(40, 30, 10)).docked(image(40, 10, 20), Some(0), Side::Top, 2);
        assert_eq!(
            rects(&above),
            [Rect::new(0, 0, 40, 14), Rect::new(0, 16, 40, 30)]
        );
    }

    #[test]
    fn a_capture_fills_its_space_from_its_middle() {
        let wide = columns(100, 60);
        let collage = Collage::new(wide.clone()).docked(image(50, 30, 20), Some(0), Side::Right, 4);
        let slots = collage.slots();
        // As tall as its space, it keeps its size and shows its middle columns.
        assert_eq!(
            slots[0].placement,
            Placement {
                scale: 1.0,
                origin: (-26.0, 0.0)
            }
        );
        let composed = collage.compose();
        assert_eq!(composed.pixels()[composed.offset(0, 0)], 26);
        assert_eq!(composed.pixels()[composed.offset(47, 0)], 73);
        // Smaller than its space, it grows to fill it.
        assert_eq!(slots[1].placement.scale, 2.0);
        assert_eq!(slots[1].placement.origin, (26.0, 0.0));
    }

    #[test]
    fn docked_against_them_all_it_spans_them() {
        let row =
            Collage::new(image(40, 40, 10)).docked(image(20, 10, 20), Some(0), Side::Right, 0);
        let under = row.docked(image(30, 10, 30), None, Side::Bottom, 0);
        assert_eq!(
            rects(&under),
            [
                Rect::new(0, 0, 20, 20),
                Rect::new(20, 0, 40, 20),
                Rect::new(0, 20, 40, 40)
            ]
        );
    }

    #[test]
    fn four_make_a_grid() {
        let collage = Collage::new(image(40, 20, 10))
            .docked(image(20, 10, 20), Some(0), Side::Right, 0)
            .docked(image(20, 10, 30), Some(0), Side::Bottom, 0)
            .docked(image(20, 10, 40), Some(2), Side::Bottom, 0);
        let mut shown = rects(&collage);
        shown.sort_by_key(|rect| (rect.top, rect.left));
        assert_eq!(
            shown,
            [
                Rect::new(0, 0, 20, 10),
                Rect::new(20, 0, 40, 10),
                Rect::new(0, 10, 20, 20),
                Rect::new(20, 10, 40, 20)
            ]
        );
        assert_eq!(collage.dividers().len(), 3);
    }

    #[test]
    fn a_divider_shares_the_room_differently_within_limits() {
        let collage =
            Collage::new(image(104, 60, 10)).docked(image(50, 30, 20), Some(0), Side::Right, 4);
        let [divider] = collage.dividers()[..] else {
            panic!("two spaces have one divider");
        };
        assert_eq!(divider.strip, Rect::new(50, 0, 54, 60));
        assert_eq!(divider.room, collage.bounds());
        // Its middle at 77 gives the first three quarters.
        assert_eq!(divider.ratio(77.0), 0.75);
        let moved = collage.divided(0, 0.75);
        assert_eq!(
            rects(&moved),
            [Rect::new(0, 0, 75, 60), Rect::new(79, 0, 104, 60)]
        );
        assert_eq!(divider.ratio(500.0), 1.0 - Collage::LEAST_SHARE);
        assert_eq!(
            collage.divided(0, 0.0).dividers()[0].strip,
            Rect::new(15, 0, 19, 60)
        );
    }

    #[test]
    fn reframing_zooms_and_moves_as_far_as_the_space_stays_filled() {
        let collage =
            Collage::new(columns(100, 60)).docked(image(50, 30, 20), Some(0), Side::Right, 4);
        // Twice as close, around a point on the left.
        let closer = collage.reframed(0, 2.0, (0.3, 0.5));
        let slot = &closer.slots()[0];
        assert_eq!(slot.zoom, 2.0);
        assert_eq!(slot.placement.scale, 2.0);
        assert!((slot.center.0 - 0.3).abs() < 1e-5);
        // Pushed past an edge, it stops where the image ends.
        let edge = collage.reframed(0, 1.0, (0.0, 0.0));
        let slot = &edge.slots()[0];
        assert_eq!(slot.placement.origin, (0.0, 0.0));
        assert_eq!(slot.center, (0.24, 0.5));
        assert_eq!(edge.compose().pixels()[0], 0);
        // Never closer than the most, nor farther than filling.
        assert_eq!(collage.reframed(0, 9.0, (0.5, 0.5)).slots()[0].zoom, 5.0);
        assert_eq!(collage.reframed(0, 0.2, (0.5, 0.5)).slots()[0].zoom, 1.0);
    }

    #[test]
    fn showing_moves_the_least_it_must() {
        let collage =
            Collage::new(columns(100, 60)).docked(image(50, 30, 20), Some(0), Side::Right, 4);
        // Columns 26 to 74 show; showing 80 to 90 brings the right edge to 90.
        let shown = collage.showing(0, [80.0, 10.0, 90.0, 20.0]);
        let slot = &shown.slots()[0];
        assert_eq!(slot.placement.origin, (-42.0, 0.0));
        // What already shows stays put.
        let still = collage.showing(0, [30.0, 10.0, 40.0, 20.0]);
        assert_eq!(still.slots()[0].placement, collage.slots()[0].placement);
        // Too wide to show, it centers on it.
        let wide = collage.showing(0, [0.0, 0.0, 80.0, 60.0]);
        assert_eq!(wide.slots()[0].center.0, 0.4);
    }

    #[test]
    fn every_pixel_has_an_owner() {
        let collage =
            Collage::new(image(100, 60, 10)).docked(image(20, 10, 20), Some(0), Side::Right, 4);
        // Left of the gap's middle, the first; right of it, the second.
        assert_eq!(collage.owner(49, 15), 0);
        assert_eq!(collage.owner(50, 2), 1);
        // Off the collage, the nearest.
        assert_eq!(collage.owner(-5, 15), 0);
        assert_eq!(collage.owner(500, 15), 1);
    }

    #[test]
    fn cropping_keeps_each_piece_as_it_shows() {
        let collage =
            Collage::new(columns(100, 60)).docked(image(50, 30, 20), Some(0), Side::Right, 4);
        // The right half of the first space, and the second one.
        let cropped = collage.cropped(Rect::new(24, 10, 100, 50)).unwrap();
        assert_eq!(cropped.size(), (76, 40));
        assert_eq!(
            rects(&cropped),
            [Rect::new(0, 0, 24, 40), Rect::new(28, 0, 76, 40)]
        );
        let near = |a: f32, b: f32| (a - b).abs() < 1e-3;
        let (before, after) = (collage.slots(), cropped.slots());
        for (before, after) in before.iter().zip(&after) {
            assert!(near(after.placement.scale, before.placement.scale));
            let ((x, y), (to_x, to_y)) = (before.placement.origin, after.placement.origin);
            assert!(near(to_x, x - 24.0) && near(to_y, y - 10.0));
        }
        // A crop of one space leaves just it.
        let one = collage.cropped(Rect::new(60, 0, 90, 60)).unwrap();
        assert!(matches!(one.tree, Tree::Piece(_)));
        assert_eq!(one.slots()[0].placement.scale, 2.0);
        assert!(collage.cropped(Rect::new(200, 0, 210, 10)).is_none());
    }

    #[test]
    fn the_side_nearest_follows_the_diagonals() {
        let rect = Rect::new(0, 0, 200, 100);
        assert_eq!(Side::nearest(rect, 20.0, 50.0), Side::Left);
        assert_eq!(Side::nearest(rect, 180.0, 40.0), Side::Right);
        assert_eq!(Side::nearest(rect, 100.0, 10.0), Side::Top);
        assert_eq!(Side::nearest(rect, 110.0, 95.0), Side::Bottom);
    }
}
