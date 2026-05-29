use crate::Point;

/// Width and height of a rectangular grid.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Size {
    pub width: u16,
    pub height: u16,
}

impl Size {
    pub fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }

    pub fn area(self) -> usize {
        (self.width as usize) * (self.height as usize)
    }

    pub fn contains(self, point: Point) -> bool {
        point.x >= 0
            && point.y >= 0
            && (point.x as u16) < self.width
            && (point.y as u16) < self.height
    }
}

impl std::fmt::Display for Size {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

impl From<(u16, u16)> for Size {
    fn from((width, height): (u16, u16)) -> Self {
        Size { width, height }
    }
}

impl From<Size> for (u16, u16) {
    fn from(s: Size) -> Self {
        (s.width, s.height)
    }
}
