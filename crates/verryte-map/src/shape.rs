use crate::Point;

/// Origin-centered sets of grid tiles used by AoE previews, telegraphs, and
/// other spatial queries. Coordinates are unbounded; callers clip to a map.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TileShape {
    /// Euclidean disk: `dx² + dy² <= radius²`, including the origin.
    Disk { radius: i16 },
    /// Chebyshev square: `max(|dx|, |dy|) <= radius`.
    Square { radius: i16 },
    /// Cardinal plus, including the origin, with arms of `radius` tiles.
    Cross { radius: i16 },
    /// Horizontal run of `length` tiles starting at the origin and going east.
    Line { length: i16 },
    /// Northward cone of `radius` (row `d` has width `2d + 1`).
    Cone { radius: i16 },
    /// Manhattan perimeter: `(|dx| + |dy|) == radius`. Origin is excluded when
    /// `radius > 0`.
    ManhattanRing { radius: i16 },
    /// Manhattan diamond: `(|dx| + |dy|) <= radius`.
    Diamond { radius: i16 },
}

impl TileShape {
    pub fn name(self) -> &'static str {
        match self {
            TileShape::Disk { .. } => "disk",
            TileShape::Square { .. } => "square",
            TileShape::Cross { .. } => "cross",
            TileShape::Line { .. } => "line",
            TileShape::Cone { .. } => "cone",
            TileShape::ManhattanRing { .. } => "ring",
            TileShape::Diamond { .. } => "diamond",
        }
    }

    /// All tiles in this shape around `origin`, in a stable but unspecified order.
    pub fn points(self, origin: Point) -> Vec<Point> {
        match self {
            TileShape::Disk { radius } => disk_points(origin, radius.max(0)),
            TileShape::Square { radius } => square_points(origin, radius.max(0)),
            TileShape::Cross { radius } => cross_points(origin, radius.max(0)),
            TileShape::Line { length } => line_points(origin, length.max(0)),
            TileShape::Cone { radius } => cone_points(origin, radius.max(0)),
            TileShape::ManhattanRing { radius } => ring_points(origin, radius.max(0)),
            TileShape::Diamond { radius } => diamond_points(origin, radius.max(0)),
        }
    }

    /// Returns whether `point` is inside this shape around `origin`.
    ///
    /// Membership matches [`TileShape::points`] without allocating the full tile list.
    pub fn contains(self, origin: Point, point: Point) -> bool {
        let dx = i32::from(point.x) - i32::from(origin.x);
        let dy = i32::from(point.y) - i32::from(origin.y);
        match self {
            TileShape::Disk { radius } => {
                let radius = i32::from(radius.max(0));
                dx * dx + dy * dy <= radius.saturating_mul(radius)
            }
            TileShape::Square { radius } => {
                let radius = i32::from(radius.max(0));
                dx.abs() <= radius && dy.abs() <= radius
            }
            TileShape::Cross { radius } => {
                let radius = i32::from(radius.max(0));
                (dx == 0 && dy.abs() <= radius) || (dy == 0 && dx.abs() <= radius)
            }
            TileShape::Line { length } => {
                let length = i32::from(length.max(0));
                dy == 0 && dx >= 0 && dx < length
            }
            TileShape::Cone { radius } => {
                let radius = i32::from(radius.max(0));
                let d = i32::from(origin.y) - i32::from(point.y);
                d >= 0 && d <= radius && dx.abs() <= d
            }
            TileShape::ManhattanRing { radius } => {
                let radius = i32::from(radius.max(0));
                if radius == 0 {
                    dx == 0 && dy == 0
                } else {
                    dx.abs() + dy.abs() == radius
                }
            }
            TileShape::Diamond { radius } => {
                let radius = i32::from(radius.max(0));
                dx.abs() + dy.abs() <= radius
            }
        }
    }

    /// Same as [`TileShape::points`], but the origin is always included first.
    pub fn points_including_origin(self, origin: Point) -> Vec<Point> {
        let mut points = self.points(origin);
        if let Some(index) = points.iter().position(|point| *point == origin) {
            if index != 0 {
                points.swap(0, index);
            }
        } else {
            points.insert(0, origin);
        }
        points
    }
}

fn disk_points(origin: Point, radius: i16) -> Vec<Point> {
    let mut tiles = Vec::new();
    let r2 = radius.saturating_mul(radius);
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx.saturating_mul(dx) + dy.saturating_mul(dy) <= r2 {
                tiles.push(origin.offset(dx, dy));
            }
        }
    }
    tiles
}

fn square_points(origin: Point, radius: i16) -> Vec<Point> {
    let mut tiles = Vec::new();
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            tiles.push(origin.offset(dx, dy));
        }
    }
    tiles
}

fn cross_points(origin: Point, radius: i16) -> Vec<Point> {
    let mut tiles = vec![origin];
    for d in 1..=radius {
        tiles.push(origin.offset(d, 0));
        tiles.push(origin.offset(-d, 0));
        tiles.push(origin.offset(0, d));
        tiles.push(origin.offset(0, -d));
    }
    tiles
}

fn line_points(origin: Point, length: i16) -> Vec<Point> {
    (0..length).map(|d| origin.offset(d, 0)).collect()
}

fn cone_points(origin: Point, radius: i16) -> Vec<Point> {
    let mut tiles = Vec::new();
    for d in 0..=radius {
        for dx in -d..=d {
            tiles.push(origin.offset(dx, -d));
        }
    }
    tiles
}

fn ring_points(origin: Point, radius: i16) -> Vec<Point> {
    if radius == 0 {
        return vec![origin];
    }
    let mut tiles = Vec::new();
    for dy in -radius..=radius {
        let remaining = radius - dy.abs();
        if remaining == 0 {
            tiles.push(origin.offset(0, dy));
        } else {
            tiles.push(origin.offset(remaining, dy));
            tiles.push(origin.offset(-remaining, dy));
        }
    }
    tiles
}

fn diamond_points(origin: Point, radius: i16) -> Vec<Point> {
    let mut tiles = Vec::new();
    for dy in -radius..=radius {
        let remaining = radius - dy.abs();
        for dx in -remaining..=remaining {
            tiles.push(origin.offset(dx, dy));
        }
    }
    tiles
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disk_radius_one_is_center_plus_cardinals() {
        let origin = Point::new(5, 5);
        let tiles = TileShape::Disk { radius: 1 }.points(origin);
        assert_eq!(tiles.len(), 5);
        assert!(tiles.contains(&origin));
        assert!(tiles.contains(&Point::new(5, 4)));
        assert!(tiles.contains(&Point::new(5, 6)));
        assert!(tiles.contains(&Point::new(4, 5)));
        assert!(tiles.contains(&Point::new(6, 5)));
        assert!(!tiles.contains(&Point::new(6, 6)));
    }

    #[test]
    fn square_radius_one_is_three_by_three() {
        let tiles = TileShape::Square { radius: 1 }.points(Point::new(5, 5));
        assert_eq!(tiles.len(), 9);
    }

    #[test]
    fn cross_radius_two_has_nine_tiles() {
        let origin = Point::new(5, 5);
        let tiles = TileShape::Cross { radius: 2 }.points(origin);
        assert_eq!(tiles.len(), 9);
        assert!(tiles.contains(&origin));
        assert!(tiles.contains(&Point::new(7, 5)));
        assert!(tiles.contains(&Point::new(3, 5)));
        assert!(tiles.contains(&Point::new(5, 7)));
        assert!(tiles.contains(&Point::new(5, 3)));
        assert!(!tiles.contains(&Point::new(6, 6)));
    }

    #[test]
    fn line_runs_east_from_origin() {
        let tiles = TileShape::Line { length: 3 }.points(Point::new(2, 4));
        assert_eq!(
            tiles,
            vec![Point::new(2, 4), Point::new(3, 4), Point::new(4, 4)]
        );
    }

    #[test]
    fn cone_opens_northward() {
        let tiles = TileShape::Cone { radius: 1 }.points(Point::new(5, 5));
        assert!(tiles.contains(&Point::new(5, 5)));
        assert!(tiles.contains(&Point::new(4, 4)));
        assert!(tiles.contains(&Point::new(5, 4)));
        assert!(tiles.contains(&Point::new(6, 4)));
        assert_eq!(tiles.len(), 4);
    }

    #[test]
    fn manhattan_ring_excludes_interior() {
        let origin = Point::new(5, 5);
        let tiles = TileShape::ManhattanRing { radius: 2 }.points(origin);
        assert_eq!(tiles.len(), 8);
        assert!(!tiles.contains(&origin));
        assert!(tiles.contains(&Point::new(7, 5)));
        assert!(tiles.contains(&Point::new(6, 6)));
        assert!(!tiles.contains(&Point::new(6, 5)));
    }

    #[test]
    fn diamond_includes_interior() {
        let origin = Point::new(5, 5);
        let tiles = TileShape::Diamond { radius: 1 }.points(origin);
        assert_eq!(tiles.len(), 5);
        assert!(tiles.contains(&origin));
    }

    #[test]
    fn points_including_origin_prepends_missing_center() {
        let origin = Point::new(5, 5);
        let tiles = TileShape::ManhattanRing { radius: 2 }.points_including_origin(origin);
        assert_eq!(tiles[0], origin);
        assert_eq!(tiles.len(), 9);
    }

    #[test]
    fn contains_matches_points_for_all_shapes() {
        let origin = Point::new(5, 5);
        let shapes = [
            TileShape::Disk { radius: 2 },
            TileShape::Square { radius: 2 },
            TileShape::Cross { radius: 2 },
            TileShape::Line { length: 3 },
            TileShape::Cone { radius: 2 },
            TileShape::ManhattanRing { radius: 2 },
            TileShape::ManhattanRing { radius: 0 },
            TileShape::Diamond { radius: 2 },
            TileShape::Disk { radius: 0 },
            TileShape::Line { length: 0 },
            TileShape::Cross { radius: -1 },
        ];
        for shape in shapes {
            let tiles = shape.points(origin);
            for y in 0..12 {
                for x in 0..12 {
                    let point = Point::new(x, y);
                    assert_eq!(
                        shape.contains(origin, point),
                        tiles.contains(&point),
                        "{shape:?} contains {point:?} disagreed with points()"
                    );
                }
            }
        }
    }
}
