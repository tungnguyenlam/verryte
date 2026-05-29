use crate::{Direction, Direction8};

/// A point in a terminal grid or tile map.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Point {
    pub x: i16,
    pub y: i16,
}

impl Point {
    pub const ZERO: Point = Point { x: 0, y: 0 };

    pub fn new(x: i16, y: i16) -> Self {
        Self { x, y }
    }

    pub fn offset(self, dx: i16, dy: i16) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
        }
    }

    /// Offset with saturation on overflow. Useful for clamping grid operations
    /// without explicit bounds checks at each step.
    pub fn saturating_offset(self, dx: i16, dy: i16) -> Self {
        Self {
            x: self.x.saturating_add(dx),
            y: self.y.saturating_add(dy),
        }
    }

    pub fn step(self, direction: Direction) -> Self {
        let (dx, dy) = direction.delta();
        self.offset(dx, dy)
    }

    pub fn manhattan_distance(self, other: Point) -> u16 {
        self.x.abs_diff(other.x) + self.y.abs_diff(other.y)
    }

    /// Chebyshev (king-move) distance: the minimum number of 8-directional
    /// steps needed to reach `other`.
    pub fn chebyshev_distance(self, other: Point) -> u16 {
        self.x.abs_diff(other.x).max(self.y.abs_diff(other.y))
    }

    /// Straight-line Euclidean distance.
    pub fn euclidean_distance(self, other: Point) -> f32 {
        let dx = (self.x as f32) - (other.x as f32);
        let dy = (self.y as f32) - (other.y as f32);
        (dx * dx + dy * dy).sqrt()
    }

    pub fn neighbors4(self) -> [Point; 4] {
        Direction::ALL.map(|direction| self.step(direction))
    }

    /// Returns all eight surrounding points (cardinal + diagonal).
    pub fn neighbors8(self) -> [Point; 8] {
        Direction8::ALL.map(|direction| self.step8(direction))
    }

    pub fn step8(self, direction: Direction8) -> Self {
        let (dx, dy) = direction.delta();
        self.offset(dx, dy)
    }
}

impl std::fmt::Display for Point {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{},{}", self.x, self.y)
    }
}

impl From<(i16, i16)> for Point {
    fn from((x, y): (i16, i16)) -> Self {
        Point { x, y }
    }
}

impl From<Point> for (i16, i16) {
    fn from(p: Point) -> (i16, i16) {
        (p.x, p.y)
    }
}

/// A point in a 3D grid or layered tile map.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Point3 {
    pub x: i16,
    pub y: i16,
    pub z: i16,
}

impl Point3 {
    pub const ZERO: Point3 = Point3 { x: 0, y: 0, z: 0 };

    pub fn new(x: i16, y: i16, z: i16) -> Self {
        Self { x, y, z }
    }

    pub fn offset(self, dx: i16, dy: i16, dz: i16) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
            z: self.z + dz,
        }
    }

    pub fn to_2d(self) -> Point {
        Point::new(self.x, self.y)
    }

    pub fn from_2d(p: Point, z: i16) -> Self {
        Self::new(p.x, p.y, z)
    }

    pub fn manhattan_distance(self, other: Point3) -> u16 {
        self.x.abs_diff(other.x) + self.y.abs_diff(other.y) + self.z.abs_diff(other.z)
    }
}

impl std::fmt::Display for Point3 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{},{},{}", self.x, self.y, self.z)
    }
}

impl From<(i16, i16, i16)> for Point3 {
    fn from((x, y, z): (i16, i16, i16)) -> Self {
        Point3 { x, y, z }
    }
}
