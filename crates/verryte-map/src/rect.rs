use crate::Point;

/// A simple rectangle in 2D space.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Rect {
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    pub fn new(x: i16, y: i16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn center(&self) -> Point {
        Point::new(
            self.x + (self.width / 2) as i16,
            self.y + (self.height / 2) as i16,
        )
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.x + other.width as i16
            && self.x + self.width as i16 > other.x
            && self.y < other.y + other.height as i16
            && self.y + self.height as i16 > other.y
    }

    pub fn area(&self) -> usize {
        (self.width as usize) * (self.height as usize)
    }

    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// Returns `true` if `self` fully contains `other`.
    pub fn contains_rect(&self, other: &Rect) -> bool {
        self.x <= other.x
            && self.y <= other.y
            && self.x + self.width as i16 >= other.x + other.width as i16
            && self.y + self.height as i16 >= other.y + other.height as i16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_contains_rect_inside() {
        let outer = Rect::new(0, 0, 10, 10);
        let inner = Rect::new(2, 3, 4, 5);
        assert!(outer.contains_rect(&inner));
    }

    #[test]
    fn rect_contains_rect_equal() {
        let r = Rect::new(1, 2, 5, 5);
        assert!(r.contains_rect(&r));
    }

    #[test]
    fn rect_contains_rect_outside() {
        let outer = Rect::new(0, 0, 5, 5);
        let other = Rect::new(3, 3, 5, 5);
        assert!(!outer.contains_rect(&other));
    }

    #[test]
    fn rect_contains_rect_partial_overlap() {
        let outer = Rect::new(0, 0, 5, 5);
        let other = Rect::new(4, 4, 5, 5);
        assert!(!outer.contains_rect(&other));
    }

    #[test]
    fn rect_contains_rect_empty_self() {
        let outer = Rect::new(0, 0, 0, 0);
        let other = Rect::new(0, 0, 5, 5);
        assert!(!outer.contains_rect(&other));
    }

    #[test]
    fn rect_contains_rect_empty_other() {
        let outer = Rect::new(0, 0, 5, 5);
        let other = Rect::new(2, 2, 0, 0);
        assert!(outer.contains_rect(&other));
    }
}
