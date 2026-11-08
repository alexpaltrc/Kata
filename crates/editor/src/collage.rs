//! Captures put together into one, as windows snap side by side: each at its own
//! size, beside or above another as it was docked, with white around the smaller
//! ones and between them all.

use std::sync::Arc;

use crate::{Image, Rect};

/// Which side of a capture another docks against.
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

    /// The half of `rect` on this side.
    pub fn half(self, rect: Rect) -> Rect {
        let (middle, center) = (rect.left + rect.width() / 2, rect.top + rect.height() / 2);
        match self {
            Self::Left => Rect::new(rect.left, rect.top, middle, rect.bottom),
            Self::Top => Rect::new(rect.left, rect.top, rect.right, center),
            Self::Right => Rect::new(middle, rect.top, rect.right, rect.bottom),
            Self::Bottom => Rect::new(rect.left, center, rect.right, rect.bottom),
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Collage {
    tree: Tree,
    /// The white between two pieces, in pixels.
    gap: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Tree {
    /// The `part` of a capture it keeps.
    Piece { image: Arc<Image>, part: Rect },
    /// Side by side when `across`, stacked otherwise.
    Group { across: bool, parts: Vec<Tree> },
}

/// Where a piece shows in the collage.
#[derive(Clone, Debug)]
pub struct Slot {
    pub image: Arc<Image>,
    /// What it keeps of its image.
    pub part: Rect,
    /// Where that shows.
    pub rect: Rect,
    /// The area around it that's its own, white included: every pixel of the
    /// collage is in one piece's cell.
    pub cell: Rect,
}

impl Slot {
    /// Where the image's top-left pixel would show.
    pub fn origin(&self) -> (i32, i32) {
        (
            self.rect.left - self.part.left,
            self.rect.top - self.part.top,
        )
    }

    /// Whether it shows `image`.
    pub fn shows(&self, image: &Arc<Image>) -> bool {
        Arc::ptr_eq(&self.image, image)
    }
}

impl Collage {
    /// Just `image`, whole.
    pub fn new(image: Arc<Image>) -> Self {
        let part = image.bounds();
        Self {
            tree: Tree::Piece { image, part },
            gap: 0,
        }
    }

    /// How many pixels across and down it takes.
    pub fn size(&self) -> (i32, i32) {
        self.tree.size(self.gap)
    }

    pub fn bounds(&self) -> Rect {
        let (width, height) = self.size();
        Rect::new(0, 0, width, height)
    }

    /// Where each piece shows, in order from the top-left.
    pub fn slots(&self) -> Vec<Slot> {
        let mut slots = Vec::new();
        let bounds = self.bounds();
        self.tree.place(bounds, bounds, self.gap, &mut slots);
        slots
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

    /// All in one image: white, with each piece where it shows. A single piece kept
    /// whole is its own image.
    pub fn compose(&self) -> Arc<Image> {
        if let Tree::Piece { image, part } = &self.tree
            && *part == image.bounds()
        {
            return image.clone();
        }
        let (width, height) = self.size();
        let mut composed = Image::new(
            width as u32,
            height as u32,
            vec![255; width as usize * height as usize * Image::BYTES_PER_PIXEL],
        );
        for slot in self.slots() {
            composed.paste(&slot.image, slot.origin(), slot.rect);
        }
        Arc::new(composed)
    }

    /// With `image` docked against the `side` of the piece at `target`, or of them
    /// all with none, `gap` pixels apart.
    pub fn docked(&self, image: Arc<Image>, target: Option<usize>, side: Side, gap: i32) -> Self {
        let part = image.bounds();
        let piece = Tree::Piece { image, part };
        let mut tree = self.tree.clone();
        match target {
            Some(target) => {
                tree.dock(&mut { target }, piece, side);
            }
            None => tree.beside(piece, side),
        }
        Self { tree, gap }
    }

    /// Only what shows inside `kept`: each piece keeps the part of it there, and
    /// those wholly outside go. `None` when none is left.
    pub fn cropped(&self, kept: Rect) -> Option<Self> {
        let slots = self.slots();
        let mut index = 0;
        let tree = self.tree.cropped(&slots, &mut index, kept)?;
        Some(Self {
            tree,
            gap: self.gap,
        })
    }
}

impl Tree {
    fn size(&self, gap: i32) -> (i32, i32) {
        match self {
            Self::Piece { part, .. } => (part.width(), part.height()),
            Self::Group { across, parts } => {
                let gaps = gap * (parts.len() as i32 - 1);
                let sizes = parts.iter().map(|part| part.size(gap));
                if *across {
                    sizes.fold((gaps, 0), |(w, h), (pw, ph)| (w + pw, h.max(ph)))
                } else {
                    sizes.fold((0, gaps), |(w, h), (pw, ph)| (w.max(pw), h + ph))
                }
            }
        }
    }

    /// Places the pieces centered in `room`, each owning its share of `cell`.
    fn place(&self, room: Rect, cell: Rect, gap: i32, slots: &mut Vec<Slot>) {
        let (width, height) = self.size(gap);
        let left = room.left + (room.width() - width) / 2;
        let top = room.top + (room.height() - height) / 2;
        let shown = Rect::new(left, top, left + width, top + height);
        match self {
            Self::Piece { image, part } => slots.push(Slot {
                image: image.clone(),
                part: *part,
                rect: shown,
                cell,
            }),
            Self::Group { across, parts } => {
                let last = parts.len() - 1;
                let mut start = if *across { shown.left } else { shown.top };
                for (index, part) in parts.iter().enumerate() {
                    let (pw, ph) = part.size(gap);
                    let len = if *across { pw } else { ph };
                    let end = start + len;
                    // The gaps split down the middle.
                    let low = if index == 0 {
                        None
                    } else {
                        Some(start - gap / 2)
                    };
                    let high = (index < last).then_some(end + (gap + 1) / 2);
                    let (room, cell) = if *across {
                        (
                            Rect::new(start, shown.top, end, shown.bottom),
                            Rect::new(
                                low.unwrap_or(cell.left),
                                cell.top,
                                high.unwrap_or(cell.right),
                                cell.bottom,
                            ),
                        )
                    } else {
                        (
                            Rect::new(shown.left, start, shown.right, end),
                            Rect::new(
                                cell.left,
                                low.unwrap_or(cell.top),
                                cell.right,
                                high.unwrap_or(cell.bottom),
                            ),
                        )
                    };
                    part.place(room, cell, gap, slots);
                    start = end + gap;
                }
            }
        }
    }

    /// Puts `piece` on the `side` of all of this: among the parts of a group going
    /// its way, or in a new group with it.
    fn beside(&mut self, piece: Self, side: Side) {
        if let Self::Group { across, parts } = self
            && *across == side.across()
        {
            let at = if side.first() { 0 } else { parts.len() };
            parts.insert(at, piece);
            return;
        }
        let empty = Self::Group {
            across: side.across(),
            parts: Vec::new(),
        };
        let this = std::mem::replace(self, empty);
        let parts = if side.first() {
            vec![piece, this]
        } else {
            vec![this, piece]
        };
        *self = Self::Group {
            across: side.across(),
            parts,
        };
    }

    /// Docks `piece` against the `side` of the `target`th piece, counting down as
    /// pieces go by. Returns whether it did.
    fn dock(&mut self, target: &mut usize, piece: Self, side: Side) -> bool {
        match self {
            Self::Piece { .. } => {
                if *target > 0 {
                    *target -= 1;
                    return false;
                }
                self.beside(piece, side);
                true
            }
            Self::Group { across, parts } => {
                for index in 0..parts.len() {
                    // Beside a piece in a row, or above one in a column, it joins in.
                    if *across == side.across()
                        && matches!(parts[index], Self::Piece { .. })
                        && *target == 0
                    {
                        let at = if side.first() { index } else { index + 1 };
                        parts.insert(at, piece);
                        return true;
                    }
                    if parts[index].dock(target, piece.clone(), side) {
                        return true;
                    }
                }
                false
            }
        }
    }

    /// What's left inside `kept`, the pieces counted by `index` among `slots`.
    fn cropped(&self, slots: &[Slot], index: &mut usize, kept: Rect) -> Option<Self> {
        match self {
            Self::Piece { image, .. } => {
                let slot = &slots[*index];
                *index += 1;
                let (x, y) = slot.origin();
                let part = slot.rect.intersect(&kept)?.offset(-x, -y);
                Some(Self::Piece {
                    image: image.clone(),
                    part,
                })
            }
            Self::Group { across, parts } => {
                let mut left: Vec<Self> = Vec::new();
                for part in parts {
                    match part.cropped(slots, index, kept) {
                        // A group left alone in a group going its way joins it.
                        Some(Self::Group {
                            across: inner,
                            parts: inners,
                        }) if inner == *across => left.extend(inners),
                        Some(part) => left.push(part),
                        None => {}
                    }
                }
                match left.len() {
                    0 => None,
                    1 => left.pop(),
                    _ => Some(Self::Group {
                        across: *across,
                        parts: left,
                    }),
                }
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

    fn rects(collage: &Collage) -> Vec<Rect> {
        collage.slots().iter().map(|slot| slot.rect).collect()
    }

    #[test]
    fn one_capture_is_its_own_image() {
        let alone = image(30, 20, 9);
        let collage = Collage::new(alone.clone());
        assert!(Arc::ptr_eq(&collage.compose(), &alone));
        assert_eq!(collage.bounds(), Rect::new(0, 0, 30, 20));
    }

    #[test]
    fn side_by_side_the_smaller_one_centers_on_white() {
        let collage =
            Collage::new(image(40, 30, 10)).docked(image(20, 10, 20), Some(0), Side::Right, 4);
        assert_eq!(collage.size(), (64, 30));
        assert_eq!(
            rects(&collage),
            [Rect::new(0, 0, 40, 30), Rect::new(44, 10, 64, 20)]
        );
        let composed = collage.compose();
        let shade = |x, y| composed.pixels()[composed.offset(x, y)];
        assert_eq!((shade(0, 0), shade(44, 10), shade(63, 19)), (10, 20, 20));
        // The gap and what's around the smaller one are white.
        assert_eq!(
            (shade(41, 15), shade(50, 5), shade(50, 25)),
            (255, 255, 255)
        );
    }

    #[test]
    fn docking_on_the_left_or_above_goes_first() {
        let left =
            Collage::new(image(40, 30, 10)).docked(image(20, 30, 20), Some(0), Side::Left, 0);
        assert_eq!(
            rects(&left),
            [Rect::new(0, 0, 20, 30), Rect::new(20, 0, 60, 30)]
        );
        let above =
            Collage::new(image(40, 30, 10)).docked(image(40, 10, 20), Some(0), Side::Top, 2);
        assert_eq!(
            rects(&above),
            [Rect::new(0, 0, 40, 10), Rect::new(0, 12, 40, 42)]
        );
    }

    #[test]
    fn one_big_and_two_stacked_beside_it() {
        // A big one, then one to its right, then another under that one.
        let collage = Collage::new(image(60, 50, 10))
            .docked(image(30, 20, 20), Some(0), Side::Right, 0)
            .docked(image(30, 30, 30), Some(1), Side::Bottom, 0);
        assert_eq!(collage.size(), (90, 50));
        assert_eq!(
            rects(&collage),
            [
                Rect::new(0, 0, 60, 50),
                Rect::new(60, 0, 90, 20),
                Rect::new(60, 20, 90, 50)
            ]
        );
    }

    #[test]
    fn a_third_beside_two_in_a_row_joins_the_row() {
        let collage = Collage::new(image(10, 10, 10))
            .docked(image(10, 10, 20), Some(0), Side::Right, 0)
            .docked(image(10, 10, 30), Some(1), Side::Right, 0);
        assert_eq!(collage.size(), (30, 10));
        let Tree::Group { parts, .. } = &collage.tree else {
            panic!("three pieces make a group");
        };
        assert_eq!(parts.len(), 3);
    }

    #[test]
    fn docked_against_them_all_it_spans_them() {
        let row =
            Collage::new(image(20, 10, 10)).docked(image(20, 10, 20), Some(0), Side::Right, 0);
        let under = row.docked(image(30, 10, 30), None, Side::Bottom, 0);
        assert_eq!(under.size(), (40, 20));
        assert_eq!(rects(&under)[2], Rect::new(5, 10, 35, 20));
        // Beside a row, it joins it.
        let beside = row.docked(image(10, 10, 30), None, Side::Left, 0);
        let Tree::Group { parts, .. } = &beside.tree else {
            panic!("three pieces make a group");
        };
        assert_eq!(parts.len(), 3);
        assert_eq!(rects(&beside)[0], Rect::new(0, 0, 10, 10));
    }

    #[test]
    fn four_make_a_grid() {
        let collage = Collage::new(image(20, 10, 10))
            .docked(image(20, 10, 20), Some(0), Side::Right, 0)
            .docked(image(20, 10, 30), Some(0), Side::Bottom, 0)
            .docked(image(20, 10, 40), Some(2), Side::Bottom, 0);
        assert_eq!(collage.size(), (40, 20));
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
    }

    #[test]
    fn every_pixel_has_an_owner() {
        let collage =
            Collage::new(image(40, 30, 10)).docked(image(20, 10, 20), Some(0), Side::Right, 4);
        // Left of the gap's middle, the first; right of it, the second, white or not.
        assert_eq!(collage.owner(41, 15), 0);
        assert_eq!(collage.owner(42, 2), 1);
        assert_eq!(collage.owner(60, 28), 1);
        // Off the collage, the nearest.
        assert_eq!(collage.owner(-5, 15), 0);
        assert_eq!(collage.owner(100, 15), 1);
    }

    #[test]
    fn cropping_keeps_parts_and_lets_go_of_what_falls_outside() {
        let collage = Collage::new(image(40, 30, 10))
            .docked(image(20, 30, 20), Some(0), Side::Right, 0)
            .docked(image(20, 30, 30), Some(1), Side::Right, 0);
        // The middle one wholly, and half of the first.
        let cropped = collage.cropped(Rect::new(20, 5, 60, 25)).unwrap();
        assert_eq!(
            rects(&cropped),
            [Rect::new(0, 0, 20, 20), Rect::new(20, 0, 40, 20)]
        );
        let slots = cropped.slots();
        assert_eq!(slots[0].part, Rect::new(20, 5, 40, 25));
        assert_eq!(slots[1].part, Rect::new(0, 5, 20, 25));
        assert_eq!(collage.cropped(Rect::new(90, 0, 99, 9)), None);
    }

    #[test]
    fn a_crop_of_one_piece_from_a_grid_leaves_just_it() {
        let collage = Collage::new(image(20, 10, 10))
            .docked(image(20, 10, 20), Some(0), Side::Right, 0)
            .docked(image(20, 10, 30), Some(0), Side::Bottom, 0);
        let cropped = collage.cropped(Rect::new(22, 5, 40, 15)).unwrap();
        assert!(matches!(cropped.tree, Tree::Piece { .. }));
        assert_eq!(cropped.bounds(), Rect::new(0, 0, 18, 10));
    }

    #[test]
    fn the_side_nearest_follows_the_diagonals() {
        let rect = Rect::new(0, 0, 200, 100);
        assert_eq!(Side::nearest(rect, 20.0, 50.0), Side::Left);
        assert_eq!(Side::nearest(rect, 180.0, 40.0), Side::Right);
        assert_eq!(Side::nearest(rect, 100.0, 10.0), Side::Top);
        assert_eq!(Side::nearest(rect, 110.0, 95.0), Side::Bottom);
        assert_eq!(Side::Right.half(rect), Rect::new(100, 0, 200, 100));
        assert_eq!(Side::Top.half(rect), Rect::new(0, 0, 200, 50));
    }
}
