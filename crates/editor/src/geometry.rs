/// An axis-aligned rectangle in pixels. `right` and `bottom` are exclusive.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    /// The rectangle spanned by two corners, in any order.
    pub fn from_corners(a: (i32, i32), b: (i32, i32)) -> Self {
        Self::new(a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1))
    }

    pub const fn width(&self) -> i32 {
        self.right - self.left
    }

    pub const fn height(&self) -> i32 {
        self.bottom - self.top
    }

    pub const fn is_empty(&self) -> bool {
        self.width() <= 0 || self.height() <= 0
    }

    pub const fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }

    pub fn offset(&self, dx: i32, dy: i32) -> Self {
        Self::new(
            self.left + dx,
            self.top + dy,
            self.right + dx,
            self.bottom + dy,
        )
    }

    /// Grows every edge outward by `by` pixels (shrinks when negative).
    pub fn inflate(&self, by: i32) -> Self {
        Self::new(
            self.left - by,
            self.top - by,
            self.right + by,
            self.bottom + by,
        )
    }

    /// The overlapping area, or `None` when the rectangles don't overlap.
    pub fn intersect(&self, other: &Self) -> Option<Self> {
        let rect = Self::new(
            self.left.max(other.left),
            self.top.max(other.top),
            self.right.min(other.right),
            self.bottom.min(other.bottom),
        );
        (!rect.is_empty()).then_some(rect)
    }

    /// The smallest rectangle containing both.
    pub fn union(&self, other: &Self) -> Self {
        Self::new(
            self.left.min(other.left),
            self.top.min(other.top),
            self.right.max(other.right),
            self.bottom.max(other.bottom),
        )
    }

    /// Interpolates every edge from `self` to `to`; `t` goes from 0 to 1.
    pub fn lerp(&self, to: &Self, t: f32) -> Self {
        let mix = |a: i32, b: i32| a + ((b - a) as f32 * t).round() as i32;
        Self::new(
            mix(self.left, to.left),
            mix(self.top, to.top),
            mix(self.right, to.right),
            mix(self.bottom, to.bottom),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corners_are_normalized() {
        assert_eq!(
            Rect::from_corners((30, 5), (10, 25)),
            Rect::new(10, 5, 30, 25)
        );
    }

    #[test]
    fn contains_excludes_right_and_bottom_edges() {
        let rect = Rect::new(0, 0, 10, 10);
        assert!(rect.contains(0, 0));
        assert!(rect.contains(9, 9));
        assert!(!rect.contains(10, 5));
        assert!(!rect.contains(5, 10));
    }

    #[test]
    fn intersect_clips_and_rejects_disjoint() {
        let a = Rect::new(0, 0, 10, 10);
        assert_eq!(
            a.intersect(&Rect::new(5, -5, 20, 5)),
            Some(Rect::new(5, 0, 10, 5))
        );
        assert_eq!(a.intersect(&Rect::new(10, 0, 20, 10)), None);
    }

    #[test]
    fn union_spans_both() {
        let a = Rect::new(0, 0, 10, 10);
        assert_eq!(a.union(&Rect::new(20, -5, 30, 5)), Rect::new(0, -5, 30, 10));
    }

    #[test]
    fn lerp_reaches_both_ends() {
        let a = Rect::new(0, 0, 10, 10);
        let b = Rect::new(100, 50, 200, 150);
        assert_eq!(a.lerp(&b, 0.0), a);
        assert_eq!(a.lerp(&b, 1.0), b);
        assert_eq!(a.lerp(&b, 0.5), Rect::new(50, 25, 105, 80));
    }
}
