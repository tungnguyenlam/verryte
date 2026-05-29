use crate::Point;

/// A rectangular region in grid coordinates.
///
/// Returned by [`TileGrid::bounding_box_of`] and usable for viewport
/// calculations, camera framing, and spatial queries.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Bounds {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Bounds {
    pub fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn right(self) -> u16 {
        self.x.saturating_add(self.width)
    }

    pub fn bottom(self) -> u16 {
        self.y.saturating_add(self.height)
    }

    pub fn contains(self, point: Point) -> bool {
        point.x >= self.x as i16
            && point.y >= self.y as i16
            && (point.x as u16) < self.right()
            && (point.y as u16) < self.bottom()
    }

    /// Returns `true` if this bounds overlaps `other`.
    pub fn intersects(self, other: Bounds) -> bool {
        if self.width == 0 || self.height == 0 || other.width == 0 || other.height == 0 {
            return false;
        }
        let x0 = self.x.max(other.x);
        let y0 = self.y.max(other.y);
        let x1 = self.right().min(other.right());
        let y1 = self.bottom().min(other.bottom());
        x0 < x1 && y0 < y1
    }

    /// Return the overlapping bounds between `self` and `other`, if any.
    pub fn intersection(self, other: Bounds) -> Option<Bounds> {
        let x0 = self.x.max(other.x);
        let y0 = self.y.max(other.y);
        let x1 = self.right().min(other.right());
        let y1 = self.bottom().min(other.bottom());
        if x0 < x1 && y0 < y1 {
            Some(Bounds::new(x0, y0, x1 - x0, y1 - y0))
        } else {
            None
        }
    }

    /// Clamp a point to the bounds rectangle.
    ///
    /// Returns `None` if the bounds are empty.
    pub fn clamp_point(self, point: Point) -> Option<Point> {
        if self.width == 0 || self.height == 0 {
            return None;
        }
        let min_x = self.x as i16;
        let min_y = self.y as i16;
        let max_x = self.x.saturating_add(self.width.saturating_sub(1)) as i16;
        let max_y = self.y.saturating_add(self.height.saturating_sub(1)) as i16;
        Some(Point::new(
            point.x.clamp(min_x, max_x),
            point.y.clamp(min_y, max_y),
        ))
    }

    pub fn center(self) -> Point {
        Point::new(
            self.x as i16 + (self.width / 2) as i16,
            self.y as i16 + (self.height / 2) as i16,
        )
    }
}

impl std::fmt::Display for Bounds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({},{} {}x{})", self.x, self.y, self.width, self.height)
    }
}
