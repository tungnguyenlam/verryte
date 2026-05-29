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
}
