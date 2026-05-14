//! Map and spatial primitives for Verryte games.
//!
//! This crate owns the small, reusable pieces that terminal grid games keep
//! needing: integer positions, cardinal directions, rectangular sizes, and a
//! typed tile grid. It deliberately does not know about rendering, ECS storage,
//! input, or game-specific tile meanings.

/// A point in a terminal grid or tile map.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

    pub fn step(self, direction: Direction) -> Self {
        let (dx, dy) = direction.delta();
        self.offset(dx, dy)
    }

    pub fn manhattan_distance(self, other: Point) -> u16 {
        self.x.abs_diff(other.x) + self.y.abs_diff(other.y)
    }

    pub fn neighbors4(self) -> [Point; 4] {
        Direction::ALL.map(|direction| self.step(direction))
    }
}

/// Cardinal movement on a 2D grid.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    North,
    South,
    East,
    West,
}

impl Direction {
    pub const ALL: [Direction; 4] = [
        Direction::North,
        Direction::South,
        Direction::East,
        Direction::West,
    ];

    pub fn delta(self) -> (i16, i16) {
        match self {
            Direction::North => (0, -1),
            Direction::South => (0, 1),
            Direction::East => (1, 0),
            Direction::West => (-1, 0),
        }
    }

    pub fn opposite(self) -> Direction {
        match self {
            Direction::North => Direction::South,
            Direction::South => Direction::North,
            Direction::East => Direction::West,
            Direction::West => Direction::East,
        }
    }
}

/// Width and height of a rectangular grid.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
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

/// A typed, fixed-size rectangular tile grid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TileGrid<T> {
    size: Size,
    tiles: Vec<T>,
}

impl<T: Clone> TileGrid<T> {
    pub fn new(width: u16, height: u16, fill: T) -> Self {
        let size = Size::new(width, height);
        Self {
            size,
            tiles: vec![fill; size.area()],
        }
    }
}

impl<T> TileGrid<T> {
    pub fn from_vec(width: u16, height: u16, tiles: Vec<T>) -> Result<Self, GridError> {
        let size = Size::new(width, height);
        if tiles.len() != size.area() {
            return Err(GridError::WrongTileCount {
                expected: size.area(),
                actual: tiles.len(),
            });
        }
        Ok(Self { size, tiles })
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn width(&self) -> u16 {
        self.size.width
    }

    pub fn height(&self) -> u16 {
        self.size.height
    }

    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    pub fn in_bounds(&self, point: Point) -> bool {
        self.size.contains(point)
    }

    pub fn index(&self, point: Point) -> Option<usize> {
        if self.in_bounds(point) {
            Some((point.y as usize) * (self.size.width as usize) + (point.x as usize))
        } else {
            None
        }
    }

    pub fn get(&self, point: Point) -> Option<&T> {
        self.index(point).map(|i| &self.tiles[i])
    }

    pub fn get_mut(&mut self, point: Point) -> Option<&mut T> {
        let i = self.index(point)?;
        Some(&mut self.tiles[i])
    }

    pub fn set(&mut self, point: Point, tile: T) -> bool {
        if let Some(slot) = self.get_mut(point) {
            *slot = tile;
            true
        } else {
            false
        }
    }

    pub fn tiles(&self) -> &[T] {
        &self.tiles
    }

    pub fn tiles_mut(&mut self) -> &mut [T] {
        &mut self.tiles
    }

    pub fn points(&self) -> impl Iterator<Item = Point> {
        let width = self.size.width as i16;
        let height = self.size.height as i16;
        (0..height).flat_map(move |y| (0..width).map(move |x| Point { x, y }))
    }

    pub fn neighbors4(&self, point: Point) -> Vec<(Point, &T)> {
        point
            .neighbors4()
            .into_iter()
            .filter_map(|neighbor| self.get(neighbor).map(|tile| (neighbor, tile)))
            .collect()
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum GridError {
    WrongTileCount { expected: usize, actual: usize },
}

impl std::fmt::Display for GridError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GridError::WrongTileCount { expected, actual } => {
                write!(f, "tile grid expected {expected} tiles, got {actual}")
            }
        }
    }
}

impl std::error::Error for GridError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_steps_by_direction() {
        let p = Point::new(4, 4);
        assert_eq!(p.step(Direction::North), Point::new(4, 3));
        assert_eq!(p.step(Direction::South), Point::new(4, 5));
        assert_eq!(p.step(Direction::East), Point::new(5, 4));
        assert_eq!(p.step(Direction::West), Point::new(3, 4));
        assert_eq!(Direction::North.opposite(), Direction::South);
        assert_eq!(p.manhattan_distance(Point::new(1, 8)), 7);
        assert_eq!(
            p.neighbors4(),
            [
                Point::new(4, 3),
                Point::new(4, 5),
                Point::new(5, 4),
                Point::new(3, 4),
            ]
        );
    }

    #[test]
    fn size_contains_rejects_negative_and_edge_points() {
        let size = Size::new(3, 2);
        assert!(size.contains(Point::new(2, 1)));
        assert!(!size.contains(Point::new(3, 1)));
        assert!(!size.contains(Point::new(1, 2)));
        assert!(!size.contains(Point::new(-1, 1)));
    }

    #[test]
    fn tile_grid_get_set_and_points_are_row_major() {
        let mut grid = TileGrid::new(3, 2, '.');
        assert_eq!(grid.len(), 6);
        assert!(grid.set(Point::new(1, 1), '#'));
        assert_eq!(grid.get(Point::new(1, 1)), Some(&'#'));
        assert!(!grid.set(Point::new(3, 1), '!'));

        let points: Vec<Point> = grid.points().collect();
        assert_eq!(
            points,
            vec![
                Point::new(0, 0),
                Point::new(1, 0),
                Point::new(2, 0),
                Point::new(0, 1),
                Point::new(1, 1),
                Point::new(2, 1),
            ]
        );
    }

    #[test]
    fn tile_grid_neighbors4_clip_to_bounds() {
        let grid = TileGrid::from_vec(3, 2, vec![0, 1, 2, 3, 4, 5]).unwrap();
        let neighbors = grid.neighbors4(Point::new(1, 0));
        assert_eq!(
            neighbors,
            vec![
                (Point::new(1, 1), &4),
                (Point::new(2, 0), &2),
                (Point::new(0, 0), &0),
            ]
        );
    }

    #[test]
    fn from_vec_validates_tile_count() {
        let err = TileGrid::from_vec(2, 2, vec![1, 2, 3]).unwrap_err();
        assert_eq!(
            err,
            GridError::WrongTileCount {
                expected: 4,
                actual: 3
            }
        );
    }
}
