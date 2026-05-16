//! Map and spatial primitives for Verryte games.
//!
//! This crate owns the small, reusable pieces that terminal grid games keep
//! needing: integer positions, cardinal directions, rectangular sizes, and a
//! typed tile grid. It deliberately does not know about rendering, ECS storage,
//! input, or game-specific tile meanings.

use std::collections::{HashMap, VecDeque};

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

    /// Chebyshev (king-move) distance: the minimum number of 8-directional
    /// steps needed to reach `other`.
    pub fn chebyshev_distance(self, other: Point) -> u16 {
        self.x.abs_diff(other.x).max(self.y.abs_diff(other.y))
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

/// Integer points on a straight line, including both endpoints.
///
/// Uses Bresenham stepping so terminal games can ask map questions like line of
/// sight without depending on rendering code.
pub fn line_between(start: Point, end: Point) -> Vec<Point> {
    LineIter::new(start, end).collect()
}

/// Lazy Bresenham line iterator.
///
/// Yields integer points on the straight line from `start` to `end`, including
/// both endpoints. Useful for line-of-sight checks and raycasting without
/// allocating a `Vec`.
pub struct LineIter {
    x: i16,
    y: i16,
    x1: i16,
    y1: i16,
    dx: i16,
    dy: i16,
    sx: i16,
    sy: i16,
    err: i16,
    done: bool,
}

impl LineIter {
    pub fn new(start: Point, end: Point) -> Self {
        let dx = (end.x - start.x).abs();
        let dy = (end.y - start.y).abs();
        let sx = if start.x < end.x { 1 } else { -1 };
        let sy = if start.y < end.y { 1 } else { -1 };
        Self {
            x: start.x,
            y: start.y,
            x1: end.x,
            y1: end.y,
            dx,
            dy: -(dy as i16),
            sx,
            sy,
            err: (dx as i16) - (dy as i16),
            done: false,
        }
    }
}

impl Iterator for LineIter {
    type Item = Point;

    fn next(&mut self) -> Option<Point> {
        if self.done {
            return None;
        }
        let point = Point::new(self.x, self.y);
        if self.x == self.x1 && self.y == self.y1 {
            self.done = true;
            return Some(point);
        }
        let e2 = 2 * self.err;
        if e2 >= self.dy {
            self.err += self.dy;
            self.x += self.sx;
        }
        if e2 <= self.dx {
            self.err += self.dx;
            self.y += self.sy;
        }
        Some(point)
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

/// Eight-directional movement on a 2D grid (cardinal + diagonal).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Direction8 {
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
    NorthWest,
}

impl Direction8 {
    pub const ALL: [Direction8; 8] = [
        Direction8::North,
        Direction8::NorthEast,
        Direction8::East,
        Direction8::SouthEast,
        Direction8::South,
        Direction8::SouthWest,
        Direction8::West,
        Direction8::NorthWest,
    ];

    /// Returns only the four cardinal directions.
    pub const CARDINAL: [Direction8; 4] = [
        Direction8::North,
        Direction8::East,
        Direction8::South,
        Direction8::West,
    ];

    /// Returns only the four diagonal directions.
    pub const DIAGONAL: [Direction8; 4] = [
        Direction8::NorthEast,
        Direction8::SouthEast,
        Direction8::SouthWest,
        Direction8::NorthWest,
    ];

    pub fn delta(self) -> (i16, i16) {
        match self {
            Direction8::North => (0, -1),
            Direction8::NorthEast => (1, -1),
            Direction8::East => (1, 0),
            Direction8::SouthEast => (1, 1),
            Direction8::South => (0, 1),
            Direction8::SouthWest => (-1, 1),
            Direction8::West => (-1, 0),
            Direction8::NorthWest => (-1, -1),
        }
    }

    pub fn opposite(self) -> Direction8 {
        match self {
            Direction8::North => Direction8::South,
            Direction8::NorthEast => Direction8::SouthWest,
            Direction8::East => Direction8::West,
            Direction8::SouthEast => Direction8::NorthWest,
            Direction8::South => Direction8::North,
            Direction8::SouthWest => Direction8::NorthEast,
            Direction8::West => Direction8::East,
            Direction8::NorthWest => Direction8::SouthEast,
        }
    }

    /// Returns `true` if this is a cardinal (non-diagonal) direction.
    pub fn is_cardinal(self) -> bool {
        matches!(
            self,
            Direction8::North | Direction8::East | Direction8::South | Direction8::West
        )
    }

    /// Convert to a cardinal `Direction` if applicable.
    pub fn to_direction(self) -> Option<Direction> {
        match self {
            Direction8::North => Some(Direction::North),
            Direction8::East => Some(Direction::East),
            Direction8::South => Some(Direction::South),
            Direction8::West => Some(Direction::West),
            _ => None,
        }
    }

    /// Build a `Direction8` from a cardinal `Direction`.
    pub fn from_direction(d: Direction) -> Self {
        match d {
            Direction::North => Direction8::North,
            Direction::East => Direction8::East,
            Direction::South => Direction8::South,
            Direction::West => Direction8::West,
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

    /// Find the shortest 8-directional path between two in-bounds points.
    ///
    /// Diagonal steps cost 14 (≈ √2 × 10) and cardinal steps cost 10, so the
    /// returned path minimizes actual travel distance rather than step count.
    /// The path includes `start` and `goal`. `passable` is consulted for
    /// neighbor tiles; `start` is allowed even if its tile is not passable.
    pub fn shortest_path8<F>(&self, start: Point, goal: Point, passable: F) -> Option<Vec<Point>>
    where
        F: Fn(Point, &T) -> bool,
    {
        if !self.in_bounds(start) || !self.in_bounds(goal) {
            return None;
        }
        if start == goal {
            return Some(vec![start]);
        }

        // A* with integer costs: cardinal = 10, diagonal = 14.
        const CARDINAL_COST: u32 = 10;
        const DIAGONAL_COST: u32 = 14;

        let mut g_score = HashMap::new();
        let mut came_from = HashMap::new();
        let mut frontier = std::collections::BinaryHeap::new();

        g_score.insert(start, 0u32);
        frontier.push(std::cmp::Reverse((
            start.chebyshev_distance(goal) as u32 * CARDINAL_COST,
            start,
        )));

        while let Some(std::cmp::Reverse((_f, current))) = frontier.pop() {
            if current == goal {
                let mut path = vec![goal];
                let mut step = goal;
                while step != start {
                    step = came_from[&step];
                    path.push(step);
                }
                path.reverse();
                return Some(path);
            }

            let current_g = *g_score.get(&current).unwrap();

            for direction in Direction8::ALL {
                let neighbor = current.step8(direction);
                let Some(tile) = self.get(neighbor) else {
                    continue;
                };
                if !passable(neighbor, tile) {
                    continue;
                }

                let step_cost = if direction.is_cardinal() {
                    CARDINAL_COST
                } else {
                    DIAGONAL_COST
                };
                let tentative_g = current_g + step_cost;

                if tentative_g < *g_score.get(&neighbor).unwrap_or(&u32::MAX) {
                    came_from.insert(neighbor, current);
                    g_score.insert(neighbor, tentative_g);
                    let f = tentative_g + neighbor.chebyshev_distance(goal) as u32 * CARDINAL_COST;
                    frontier.push(std::cmp::Reverse((f, neighbor)));
                }
            }
        }

        None
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

    pub fn iter(&self) -> impl Iterator<Item = (Point, &T)> {
        self.points().zip(self.tiles.iter())
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Point, &mut T)> {
        let width = self.size.width as i16;
        let height = self.size.height as i16;
        let points = (0..height).flat_map(move |y| (0..width).map(move |x| Point { x, y }));
        points.zip(self.tiles.iter_mut())
    }

    pub fn fill(&mut self, tile: T)
    where
        T: Clone,
    {
        for t in &mut self.tiles {
            *t = tile.clone();
        }
    }

    pub fn neighbors4(&self, point: Point) -> Vec<(Point, &T)> {
        point
            .neighbors4()
            .into_iter()
            .filter_map(|neighbor| self.get(neighbor).map(|tile| (neighbor, tile)))
            .collect()
    }

    /// Returns all eight in-bounds neighbors (cardinal + diagonal) with their tiles.
    pub fn neighbors8(&self, point: Point) -> Vec<(Point, &T)> {
        point
            .neighbors8()
            .into_iter()
            .filter_map(|neighbor| self.get(neighbor).map(|tile| (neighbor, tile)))
            .collect()
    }

    /// Return every in-bounds point visible from `origin` within a Manhattan
    /// `radius`. Blocking tiles stop sight beyond themselves but remain visible.
    pub fn visible_points<F>(&self, origin: Point, radius: u16, blocks_light: F) -> Vec<Point>
    where
        F: Fn(&T) -> bool,
    {
        if !self.in_bounds(origin) {
            return Vec::new();
        }

        let mut visible = Vec::new();
        for point in self.points() {
            if origin.manhattan_distance(point) > radius {
                continue;
            }
            let line = line_between(origin, point);
            let blocked_before_target = line
                .iter()
                .skip(1)
                .take(line.len().saturating_sub(2))
                .any(|p| self.get(*p).is_some_and(&blocks_light));
            if !blocked_before_target {
                visible.push(point);
            }
        }
        visible
    }

    /// Find the shortest cardinal path between two in-bounds points.
    ///
    /// The returned path includes `start` and `goal`. `passable` is consulted
    /// for neighbor tiles; `start` is allowed even if its tile is not passable
    /// so callers can path out from transient entity positions.
    pub fn shortest_path4<F>(&self, start: Point, goal: Point, passable: F) -> Option<Vec<Point>>
    where
        F: Fn(Point, &T) -> bool,
    {
        if !self.in_bounds(start) || !self.in_bounds(goal) {
            return None;
        }
        if start == goal {
            return Some(vec![start]);
        }

        let mut frontier = VecDeque::new();
        let mut came_from = HashMap::new();
        frontier.push_back(start);
        came_from.insert(start, start);

        while let Some(current) = frontier.pop_front() {
            for neighbor in current.neighbors4() {
                if came_from.contains_key(&neighbor) {
                    continue;
                }
                let Some(tile) = self.get(neighbor) else {
                    continue;
                };
                if !passable(neighbor, tile) {
                    continue;
                }

                came_from.insert(neighbor, current);
                if neighbor == goal {
                    let mut path = vec![goal];
                    let mut step = goal;
                    while step != start {
                        step = came_from[&step];
                        path.push(step);
                    }
                    path.reverse();
                    return Some(path);
                }
                frontier.push_back(neighbor);
            }
        }

        None
    }

    /// Find the shortest cardinal path from `start` to the nearest target.
    ///
    /// Each returned path includes `start` and the chosen target. Ties keep the
    /// first shortest path found in target iteration order.
    pub fn nearest_path4<I, F>(&self, start: Point, targets: I, passable: F) -> Option<Vec<Point>>
    where
        I: IntoIterator<Item = Point>,
        F: Fn(Point, &T) -> bool,
    {
        targets
            .into_iter()
            .filter_map(|target| self.shortest_path4(start, target, &passable))
            .min_by_key(|path| path.len())
    }

    /// Return the cardinal distance from `start` to the nearest reachable target.
    ///
    /// Uses one breadth-first walk over passable neighbors, so callers can ask
    /// "how far is the nearest X?" without constructing full paths to every
    /// candidate target.
    pub fn distance_to_nearest4<I, F>(&self, start: Point, targets: I, passable: F) -> Option<u16>
    where
        I: IntoIterator<Item = Point>,
        F: Fn(Point, &T) -> bool,
    {
        if !self.in_bounds(start) {
            return None;
        }

        let targets: HashMap<Point, ()> = targets
            .into_iter()
            .filter(|point| self.in_bounds(*point))
            .map(|point| (point, ()))
            .collect();
        if targets.is_empty() {
            return None;
        }
        if targets.contains_key(&start) {
            return Some(0);
        }

        let mut frontier = VecDeque::new();
        let mut seen = HashMap::new();
        frontier.push_back((start, 0u16));
        seen.insert(start, ());

        while let Some((current, distance)) = frontier.pop_front() {
            for neighbor in current.neighbors4() {
                if seen.contains_key(&neighbor) {
                    continue;
                }
                let Some(tile) = self.get(neighbor) else {
                    continue;
                };
                if !passable(neighbor, tile) {
                    continue;
                }

                let next_distance = distance.saturating_add(1);
                if targets.contains_key(&neighbor) {
                    return Some(next_distance);
                }

                seen.insert(neighbor, ());
                frontier.push_back((neighbor, next_distance));
            }
        }

        None
    }

    /// Return every point reachable from `start` by cardinal movement.
    ///
    /// `start` is included when it is in bounds, even if its tile would not be
    /// passable for neighbors. This mirrors [`Self::shortest_path4`] and keeps
    /// entity positions inspectable even when gameplay permits transient states.
    pub fn reachable_points4<F>(&self, start: Point, passable: F) -> Vec<Point>
    where
        F: Fn(Point, &T) -> bool,
    {
        if !self.in_bounds(start) {
            return Vec::new();
        }

        let mut frontier = VecDeque::new();
        let mut seen = HashMap::new();
        let mut out = Vec::new();

        frontier.push_back(start);
        seen.insert(start, ());

        while let Some(current) = frontier.pop_front() {
            out.push(current);
            for neighbor in current.neighbors4() {
                if seen.contains_key(&neighbor) {
                    continue;
                }
                let Some(tile) = self.get(neighbor) else {
                    continue;
                };
                if !passable(neighbor, tile) {
                    continue;
                }
                seen.insert(neighbor, ());
                frontier.push_back(neighbor);
            }
        }

        out
    }

    /// Find the shortest 8-directional path from `start` to the nearest target.
    ///
    /// Each returned path includes `start` and the chosen target. Ties keep the
    /// first shortest path found in target iteration order. Uses the same cost
    /// model as [`Self::shortest_path8`] (cardinal = 10, diagonal = 14).
    pub fn nearest_path8<I, F>(&self, start: Point, targets: I, passable: F) -> Option<Vec<Point>>
    where
        I: IntoIterator<Item = Point>,
        F: Fn(Point, &T) -> bool,
    {
        targets
            .into_iter()
            .filter_map(|target| self.shortest_path8(start, target, &passable))
            .min_by_key(|path| path.len())
    }

    /// Return every point reachable from `start` by 8-directional movement.
    ///
    /// `start` is included when it is in bounds, even if its tile would not be
    /// passable for neighbors. This mirrors [`Self::shortest_path8`] and keeps
    /// entity positions inspectable even when gameplay permits transient states.
    pub fn reachable_points8<F>(&self, start: Point, passable: F) -> Vec<Point>
    where
        F: Fn(Point, &T) -> bool,
    {
        if !self.in_bounds(start) {
            return Vec::new();
        }

        let mut frontier = VecDeque::new();
        let mut seen = HashMap::new();
        let mut out = Vec::new();

        frontier.push_back(start);
        seen.insert(start, ());

        while let Some(current) = frontier.pop_front() {
            out.push(current);
            for neighbor in current.neighbors8() {
                if seen.contains_key(&neighbor) {
                    continue;
                }
                let Some(tile) = self.get(neighbor) else {
                    continue;
                };
                if !passable(neighbor, tile) {
                    continue;
                }
                seen.insert(neighbor, ());
                frontier.push_back(neighbor);
            }
        }

        out
    }

    /// Return the cardinal direction from `from` to `to`, if they are adjacent.
    pub fn direction_to(&self, from: Point, to: Point) -> Option<Direction> {
        Direction::ALL.into_iter().find(|&d| from.step(d) == to)
    }

    /// Check whether the straight line from `from` to `to` is clear of
    /// blocking tiles. Uses the lazy [`LineIter`] so no allocation is needed.
    ///
    /// Both endpoints are excluded from the blocking check: `from` is the
    /// observer and `to` is the target, so neither should block. Returns
    /// `false` if either point is out of bounds.
    pub fn is_line_of_sight_clear<F>(&self, from: Point, to: Point, blocks: F) -> bool
    where
        F: Fn(&T) -> bool,
    {
        if !self.in_bounds(from) || !self.in_bounds(to) {
            return false;
        }
        let mut iter = LineIter::new(from, to);
        // Skip the origin point.
        iter.next();
        for point in iter {
            // The last point is the target; don't check it.
            if point == to {
                return true;
            }
            if let Some(tile) = self.get(point) {
                if blocks(tile) {
                    return false;
                }
            }
        }
        true
    }

    /// Return neighbors that are further away from all `threats` than `from` is.
    ///
    /// This is a simple spatial heuristic for "stepping away" from hazards or
    /// enemies. If multiple neighbors are tied for maximum safety, all are
    /// returned. Returns an empty list if no neighbor is safer than `from`.
    pub fn safer_neighbors4<I, F>(&self, from: Point, threats: I, passable: F) -> Vec<Point>
    where
        I: IntoIterator<Item = Point> + Clone,
        F: Fn(Point, &T) -> bool,
    {
        let current_dist = self
            .distance_to_nearest4(from, threats.clone(), &passable)
            .unwrap_or(u16::MAX);

        let mut candidates = Vec::new();
        for neighbor in from.neighbors4() {
            let Some(tile) = self.get(neighbor) else {
                continue;
            };
            if !passable(neighbor, tile) {
                continue;
            }
            if let Some(dist) = self.distance_to_nearest4(neighbor, threats.clone(), &passable) {
                if dist > current_dist {
                    candidates.push((neighbor, dist));
                }
            }
        }

        if candidates.is_empty() {
            return Vec::new();
        }

        let max_dist = candidates.iter().map(|(_, d)| *d).max().unwrap_or(0);
        candidates
            .into_iter()
            .filter(|(_, d)| *d == max_dist)
            .map(|(n, _)| n)
            .collect()
    }

    /// Flood-fill from `start`, returning every connected point matching the
    /// predicate. Uses BFS so the result is ordered by distance from `start`.
    ///
    /// Useful for room detection, region labeling, and connected-component
    /// queries in terminal maps.
    pub fn flood_fill4<F>(&self, start: Point, matches: F) -> Vec<Point>
    where
        F: Fn(Point, &T) -> bool,
    {
        if !self.in_bounds(start) {
            return Vec::new();
        }
        let Some(tile) = self.get(start) else {
            return Vec::new();
        };
        if !matches(start, tile) {
            return Vec::new();
        }

        let mut frontier = VecDeque::new();
        let mut seen = HashMap::new();
        let mut out = Vec::new();

        frontier.push_back(start);
        seen.insert(start, ());

        while let Some(current) = frontier.pop_front() {
            out.push(current);
            for neighbor in current.neighbors4() {
                if seen.contains_key(&neighbor) {
                    continue;
                }
                let Some(tile) = self.get(neighbor) else {
                    continue;
                };
                if !matches(neighbor, tile) {
                    continue;
                }
                seen.insert(neighbor, ());
                frontier.push_back(neighbor);
            }
        }

        out
    }

    /// Count the number of connected regions matching the predicate.
    ///
    /// Walks every point in the grid. Each unvisited matching point starts a
    /// new region that is consumed via [`Self::flood_fill4`]. Useful for
    /// detecting how many rooms, lakes, or isolated areas a map contains.
    pub fn count_regions4<F>(&self, matches: F) -> usize
    where
        F: Fn(Point, &T) -> bool,
    {
        let mut visited = HashMap::new();
        let mut count = 0;

        for point in self.points() {
            if visited.contains_key(&point) {
                continue;
            }
            let Some(tile) = self.get(point) else {
                continue;
            };
            if !matches(point, tile) {
                continue;
            }
            count += 1;
            let region = self.flood_fill4(point, &matches);
            for p in region {
                visited.insert(p, ());
            }
        }

        count
    }

    /// Carve floor tiles into the grid using a random-walk algorithm.
    ///
    /// Starts at `start` and takes `steps` random cardinal steps, setting each
    /// visited tile to `floor`. This is a simple dungeon-generation primitive
    /// that produces organic, cave-like shapes when run multiple times.
    ///
    /// The `seed` controls reproducibility. Out-of-bounds steps are silently
    /// skipped, so the walk stays within the grid.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut grid = TileGrid::new(20, 20, Tile::Wall);
    /// grid.random_walk_fill4(Point::new(10, 10), 200, Tile::Floor, 42);
    /// ```
    pub fn random_walk_fill4(&mut self, start: Point, steps: usize, floor: T, seed: u64)
    where
        T: Clone + PartialEq,
    {
        if !self.in_bounds(start) {
            return;
        }

        // Simple xorshift64 PRNG for reproducible walks without external deps.
        let mut state = seed | 1; // Ensure non-zero.
        let mut next_u64 = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };

        let mut pos = start;
        self.set(pos, floor.clone());

        for _ in 0..steps {
            let dir_idx = (next_u64() as usize) % 4;
            let next = pos.step(Direction::ALL[dir_idx]);
            if self.in_bounds(next) {
                pos = next;
                self.set(pos, floor.clone());
            }
        }
    }

    /// Generate a dungeon using BSP (binary space partitioning) and carve it
    /// into the grid.
    ///
    /// Starts with the full grid, recursively splits into sub-regions,
    /// places a random room in each leaf, then connects sibling rooms with
    /// L-shaped corridors. `wall` is the background tile; `floor` is used for
    /// rooms and corridors. The `seed` controls reproducibility.
    ///
    /// Returns the list of room center points, useful for spawn placement.
    pub fn generate_bsp_dungeon(
        &mut self,
        wall: T,
        floor: T,
        min_room_size: u16,
        seed: u64,
    ) -> Vec<Point>
    where
        T: Clone + PartialEq,
    {
        self.fill(wall.clone());
        let w = self.width();
        let h = self.height();
        if w < 3 || h < 3 || min_room_size < 2 {
            return Vec::new();
        }

        let mut state = seed | 1;
        let mut rng = || -> u64 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };

        #[derive(Clone, Copy)]
        struct Region {
            x: u16,
            y: u16,
            w: u16,
            h: u16,
        }

        impl Region {
            fn center(self) -> Point {
                Point::new((self.x + self.w / 2) as i16, (self.y + self.h / 2) as i16)
            }
        }

        struct Node {
            region: Region,
            room: Option<Region>,
            left: Option<Box<Node>>,
            right: Option<Box<Node>>,
        }

        impl Node {
            fn leaf(region: Region) -> Self {
                Self {
                    region,
                    room: None,
                    left: None,
                    right: None,
                }
            }

            fn is_leaf(&self) -> bool {
                self.left.is_none() && self.right.is_none()
            }

            fn leaves_mut(&mut self) -> Vec<&mut Node> {
                if self.is_leaf() {
                    vec![self]
                } else {
                    let mut out = Vec::new();
                    if let Some(ref mut left) = self.left {
                        out.extend(left.leaves_mut());
                    }
                    if let Some(ref mut right) = self.right {
                        out.extend(right.leaves_mut());
                    }
                    out
                }
            }

            fn room_center(&self) -> Option<Point> {
                self.room.map(|r| r.center())
            }
        }

        // Recursive BSP split.
        fn split(node: &mut Node, min_size: u16, rng: &mut impl FnMut() -> u64) {
            let r = node.region;
            // Decide split direction: prefer splitting the longer axis.
            let horizontal = if r.w > r.h {
                true
            } else if r.h > r.w {
                false
            } else {
                rng() % 2 == 0
            };

            let max_span = if horizontal { r.h } else { r.w };
            if max_span < min_size * 2 + 1 {
                // Too small to split further; this is a leaf.
                return;
            }

            let split_range = max_span - min_size * 2 - 1;
            if split_range == 0 {
                return;
            }
            let offset = min_size + (rng() % split_range as u64) as u16;

            let (left_region, right_region) = if horizontal {
                (
                    Region {
                        x: r.x,
                        y: r.y,
                        w: r.w,
                        h: offset,
                    },
                    Region {
                        x: r.x,
                        y: r.y + offset,
                        w: r.w,
                        h: r.h - offset,
                    },
                )
            } else {
                (
                    Region {
                        x: r.x,
                        y: r.y,
                        w: offset,
                        h: r.h,
                    },
                    Region {
                        x: r.x + offset,
                        y: r.y,
                        w: r.w - offset,
                        h: r.h,
                    },
                )
            };

            node.left = Some(Box::new(Node::leaf(left_region)));
            node.right = Some(Box::new(Node::leaf(right_region)));
            if let Some(ref mut left) = node.left {
                split(left, min_size, rng);
            }
            if let Some(ref mut right) = node.right {
                split(right, min_size, rng);
            }
        }

        // Place rooms in leaf nodes.
        fn place_rooms(node: &mut Node, min_size: u16, rng: &mut impl FnMut() -> u64) {
            if node.is_leaf() {
                let r = node.region;
                // Room must fit within region with 1-cell border.
                let max_w = r.w.saturating_sub(2).max(min_size);
                let max_h = r.h.saturating_sub(2).max(min_size);
                let w_range = (max_w - min_size + 1) as u64;
                let h_range = (max_h - min_size + 1) as u64;
                let room_w = min_size + (rng() % w_range) as u16;
                let room_h = min_size + (rng() % h_range) as u16;
                let x_range = r.w.saturating_sub(room_w + 2).max(1) as u64;
                let y_range = r.h.saturating_sub(room_h + 2).max(1) as u64;
                let room_x = r.x + 1 + (rng() % x_range) as u16;
                let room_y = r.y + 1 + (rng() % y_range) as u16;
                node.room = Some(Region {
                    x: room_x,
                    y: room_y,
                    w: room_w,
                    h: room_h,
                });
            } else {
                if let Some(ref mut left) = node.left {
                    place_rooms(left, min_size, rng);
                }
                if let Some(ref mut right) = node.right {
                    place_rooms(right, min_size, rng);
                }
            }
        }

        let mut root = Node::leaf(Region { x: 0, y: 0, w, h });
        split(&mut root, min_room_size, &mut rng);
        place_rooms(&mut root, min_room_size, &mut rng);

        // Carve rooms.
        let mut centers = Vec::new();
        for leaf in root.leaves_mut() {
            if let Some(room) = leaf.room {
                for dy in 0..room.h {
                    for dx in 0..room.w {
                        let px = room.x + dx;
                        let py = room.y + dy;
                        let p = Point::new(px as i16, py as i16);
                        if self.in_bounds(p) {
                            self.set(p, floor.clone());
                        }
                    }
                }
                centers.push(room.center());
            }
        }

        // Collect corridor segments to carve between sibling rooms.
        fn collect_corridors(
            node: &mut Node,
            corridors: &mut Vec<(Point, Point)>,
        ) -> Option<Point> {
            if node.is_leaf() {
                return node.room_center();
            }
            let left_center = node
                .left
                .as_mut()
                .and_then(|n| collect_corridors(n, corridors));
            let right_center = node
                .right
                .as_mut()
                .and_then(|n| collect_corridors(n, corridors));
            if let (Some(lc), Some(rc)) = (left_center, right_center) {
                corridors.push((lc, rc));
            }
            left_center.or(right_center)
        }

        let mut corridors = Vec::new();
        collect_corridors(&mut root, &mut corridors);

        // Carve corridors as L-shaped passages.
        for (from, to) in corridors {
            let mid = if rng() % 2 == 0 {
                Point::new(to.x, from.y)
            } else {
                Point::new(from.x, to.y)
            };
            // Carve from -> mid -> to.
            for point in LineIter::new(from, mid).chain(LineIter::new(mid, to)) {
                if self.in_bounds(point) {
                    self.set(point, floor.clone());
                }
            }
        }

        centers
    }

    /// Place non-overlapping rectangular rooms on the grid.
    ///
    /// Fills the grid with `wall` first, then attempts to place `max_rooms`
    /// rooms with sizes in `[min_size, max_size]`. Rooms are carved with
    /// `floor`. Returns the centers of successfully placed rooms.
    ///
    /// Uses a simple rejection-sampling approach: try random positions,
    /// skip if overlapping with existing rooms. This is simpler than BSP
    /// and works well for cave-like or organic layouts.
    pub fn place_rooms<F1, F2, R>(
        &mut self,
        max_rooms: usize,
        min_size: u16,
        max_size: u16,
        wall: F1,
        floor: F2,
        rng: &mut R,
    ) -> Vec<Point>
    where
        F1: Fn() -> T,
        F2: Fn() -> T,
        R: FnMut() -> u64,
    {
        let w = self.width() as i16;
        let h = self.height() as i16;

        // Fill with wall.
        for y in 0..self.height() {
            for x in 0..self.width() {
                self.set(Point::new(x as i16, y as i16), wall());
            }
        }

        let mut rooms: Vec<(i16, i16, u16, u16)> = Vec::new();
        let mut centers = Vec::new();
        let max_size = max_size.min(w as u16).min(h as u16);
        let min_size = min_size.min(max_size);

        for _ in 0..max_rooms * 10 {
            if rooms.len() >= max_rooms {
                break;
            }
            let rw = (rng() % (max_size - min_size + 1) as u64) as u16 + min_size;
            let rh = (rng() % (max_size - min_size + 1) as u64) as u16 + min_size;
            let rx = (rng() % (w as u64 - rw as u64 + 1)) as i16;
            let ry = (rng() % (h as u64 - rh as u64 + 1)) as i16;

            // Check overlap with existing rooms (with 1-cell padding).
            let overlaps = rooms.iter().any(|&(ox, oy, ow, oh)| {
                rx < ox + ow as i16 + 1
                    && rx + rw as i16 + 1 > ox
                    && ry < oy + oh as i16 + 1
                    && ry + rh as i16 + 1 > oy
            });
            if overlaps {
                continue;
            }

            // Carve room.
            for dy in 0..rh {
                for dx in 0..rw {
                    let px = rx + dx as i16;
                    let py = ry + dy as i16;
                    if px >= 0 && py >= 0 {
                        self.set(Point::new(px, py), floor());
                    }
                }
            }

            rooms.push((rx, ry, rw, rh));
            centers.push(Point::new(rx + (rw / 2) as i16, ry + (rh / 2) as i16));
        }

        centers
    }

    /// Count how many tiles match the predicate.
    ///
    /// Useful for measuring map density, counting floor/wall ratios, or
    /// checking how many tiles satisfy a condition without iterating manually.
    pub fn count_matching<F>(&self, predicate: F) -> usize
    where
        F: Fn(Point, &T) -> bool,
    {
        self.iter().filter(|(p, t)| predicate(*p, t)).count()
    }

    /// Return the fraction of tiles that match the predicate, as a value
    /// between 0.0 and 1.0. Returns 0.0 for empty grids.
    pub fn density<F>(&self, predicate: F) -> f32
    where
        F: Fn(Point, &T) -> bool,
    {
        if self.is_empty() {
            return 0.0;
        }
        self.count_matching(predicate) as f32 / self.len() as f32
    }

    /// Find the bounding box of all tiles matching the predicate.
    ///
    /// Returns `None` if no tiles match.
    pub fn bounding_box_of<F>(&self, predicate: F) -> Option<Bounds>
    where
        F: Fn(Point, &T) -> bool,
    {
        let mut min_x = i16::MAX;
        let mut min_y = i16::MAX;
        let mut max_x = i16::MIN;
        let mut max_y = i16::MIN;
        let mut found = false;

        for (point, tile) in self.iter() {
            if predicate(point, tile) {
                found = true;
                min_x = min_x.min(point.x);
                min_y = min_y.min(point.y);
                max_x = max_x.max(point.x);
                max_y = max_y.max(point.y);
            }
        }

        if !found {
            return None;
        }

        Some(Bounds {
            x: min_x as u16,
            y: min_y as u16,
            width: (max_x - min_x + 1) as u16,
            height: (max_y - min_y + 1) as u16,
        })
    }

    /// Compute the visible tiles from `origin` using recursive shadowcasting.
    ///
    /// Returns all tiles within `radius` that are not blocked by walls. The
    /// origin tile is always included. Blocking tiles are visible themselves
    /// but cast shadows that prevent tiles behind them from being seen.
    ///
    /// This is the standard FOV algorithm for roguelikes: fast, accurate, and
    /// symmetric (if A can see B, B can see A).
    pub fn field_of_view<F>(&self, origin: Point, radius: u16, blocks_light: F) -> Vec<Point>
    where
        F: Fn(&T) -> bool,
    {
        if !self.in_bounds(origin) {
            return Vec::new();
        }

        let radius = radius as i16;
        let mut visible = Vec::new();
        visible.push(origin);

        // Shadowcasting: scan each of the 8 octants using standard multipliers.
        // Each [xx, xy, yx, yy] transforms recursive scan coordinates into grid coords.
        let mult: [[i16; 4]; 8] = [
            [1, 0, 0, -1],
            [0, 1, 1, 0],
            [0, -1, -1, 0],
            [-1, 0, 0, 1],
            [1, 0, 0, 1],
            [0, 1, -1, 0],
            [0, -1, 1, 0],
            [-1, 0, 0, -1],
        ];

        for [xx, xy, yx, yy] in mult {
            cast_light(
                self,
                origin,
                1,
                1.0,
                0.0,
                radius,
                xx,
                xy,
                yx,
                yy,
                &mut visible,
                &blocks_light,
            );
        }

        visible
    }
}

fn cast_light<F, T>(
    grid: &TileGrid<T>,
    origin: Point,
    row: i16,
    start_slope: f64,
    end_slope: f64,
    radius: i16,
    xx: i16,
    xy: i16,
    yx: i16,
    yy: i16,
    visible: &mut Vec<Point>,
    blocks_light: &F,
) where
    F: Fn(&T) -> bool,
{
    if start_slope < end_slope {
        return;
    }

    let mut next_start = start_slope;

    for i in row..=radius {
        let mut blocked = false;
        let mut j = i;
        while j >= 0 {
            let dx = (i as f64) * (start_slope + end_slope) / 2.0;
            let min_slope = (j as f64 - 0.5) / (i as f64 + 0.5);
            let max_slope = (j as f64 + 0.5) / (i as f64 - 0.5);

            if end_slope > max_slope {
                j -= 1;
                continue;
            } else if start_slope < min_slope {
                break;
            }

            let map_x = origin.x as i32 + (dx * xx as f64 + j as f64 * xy as f64).round() as i32;
            let map_y = origin.y as i32 + (dx * yx as f64 + j as f64 * yy as f64).round() as i32;

            if map_x >= 0 && map_y >= 0 {
                let point = Point::new(map_x as i16, map_y as i16);
                if grid.in_bounds(point) {
                    let dist = origin.manhattan_distance(point);
                    if dist as i16 <= radius {
                        if blocked {
                            if let Some(tile) = grid.get(point) {
                                if blocks_light(tile) {
                                    let new_start = next_start;
                                    next_start = min_slope;
                                    cast_light(
                                        grid,
                                        origin,
                                        i + 1,
                                        new_start,
                                        end_slope,
                                        radius,
                                        xx,
                                        xy,
                                        yx,
                                        yy,
                                        visible,
                                        blocks_light,
                                    );
                                } else {
                                    blocked = false;
                                    let p = Point::new(map_x as i16, map_y as i16);
                                    if !visible.contains(&p) {
                                        visible.push(p);
                                    }
                                }
                            }
                        } else if let Some(tile) = grid.get(point) {
                            let p = Point::new(map_x as i16, map_y as i16);
                            if !visible.contains(&p) {
                                visible.push(p);
                            }
                            if blocks_light(tile) {
                                blocked = true;
                                cast_light(
                                    grid,
                                    origin,
                                    i + 1,
                                    next_start,
                                    min_slope,
                                    radius,
                                    xx,
                                    xy,
                                    yx,
                                    yy,
                                    visible,
                                    blocks_light,
                                );
                                next_start = max_slope;
                            }
                        }
                    }
                }
            }
            j -= 1;
        }
        if blocked {
            break;
        }
    }
}

/// A rectangular region in grid coordinates.
///
/// Returned by [`TileGrid::bounding_box_of`] and usable for viewport
/// calculations, camera framing, and spatial queries.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
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

    pub fn center(self) -> Point {
        Point::new(
            self.x as i16 + (self.width / 2) as i16,
            self.y as i16 + (self.height / 2) as i16,
        )
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

/// A spatial hash for efficient proximity queries on grid-based entities.
///
/// Divides space into fixed-size cells and stores entities in the cell
/// corresponding to their position. Queries only check the relevant cells
/// instead of scanning all entities.
///
/// Useful for games with many entities where you frequently need to find
/// entities near a point (AI targeting, collision detection, interaction range).
///
/// # Example
///
/// ```ignore
/// let mut hash = SpatialHash::new(5); // 5-cell buckets
/// hash.insert(Point::new(3, 3), enemy_entity);
/// hash.insert(Point::new(4, 4), another_enemy);
///
/// // Find all entities within 2 cells of (5, 5).
/// for entity in hash.query(Point::new(5, 5), 2) {
///     // ...
/// }
/// ```
pub struct SpatialHash<T> {
    cell_size: i16,
    cells: HashMap<(i16, i16), Vec<(Point, T)>>,
}

impl<T> SpatialHash<T> {
    /// Create a new spatial hash with the given cell size.
    ///
    /// Smaller cells give finer granularity but use more memory.
    /// Larger cells use less memory but query more irrelevant entities.
    /// A good default is 3-10 depending on typical entity density.
    pub fn new(cell_size: i16) -> Self {
        Self {
            cell_size: cell_size.max(1),
            cells: HashMap::new(),
        }
    }

    fn cell_key(&self, point: Point) -> (i16, i16) {
        (
            point.x.div_euclid(self.cell_size),
            point.y.div_euclid(self.cell_size),
        )
    }

    /// Insert an entity at the given point.
    pub fn insert(&mut self, point: Point, value: T) {
        let key = self.cell_key(point);
        self.cells.entry(key).or_default().push((point, value));
    }

    /// Remove the first entity at `point` that equals `value` (by PartialEq).
    ///
    /// Returns `true` if an entity was found and removed.
    pub fn remove(&mut self, point: Point, value: &T) -> bool
    where
        T: PartialEq,
    {
        let key = self.cell_key(point);
        if let Some(entries) = self.cells.get_mut(&key) {
            if let Some(pos) = entries.iter().position(|(p, v)| *p == point && v == value) {
                entries.remove(pos);
                if entries.is_empty() {
                    self.cells.remove(&key);
                }
                return true;
            }
        }
        false
    }

    /// Query all entities within `radius` (Manhattan distance) of `center`.
    pub fn query<'a>(&'a self, center: Point, radius: u16) -> impl Iterator<Item = &'a T> + 'a {
        let radius = radius as i16;
        let cell_radius = (radius / self.cell_size) + 1;
        let (cx, cy) = self.cell_key(center);

        let mut results = Vec::new();
        for dx in -cell_radius..=cell_radius {
            for dy in -cell_radius..=cell_radius {
                if let Some(entries) = self.cells.get(&(cx + dx, cy + dy)) {
                    for (point, value) in entries {
                        if point.manhattan_distance(center) <= radius as u16 {
                            results.push(value);
                        }
                    }
                }
            }
        }
        results.into_iter()
    }

    /// Find the nearest entity to `center` within `radius`, using a custom
    /// comparison function.
    ///
    /// The comparison function receives two entity references and the center
    /// point, and should return an `Ordering`.
    pub fn nearest<F>(&self, center: Point, radius: u16, mut cmp: F) -> Option<&T>
    where
        F: FnMut(&T, &T, Point) -> std::cmp::Ordering,
    {
        self.query(center, radius).min_by(|a, b| cmp(a, b, center))
    }

    /// Remove all entities from the hash.
    pub fn clear(&mut self) {
        self.cells.clear();
    }

    /// Total number of entities in the hash.
    pub fn len(&self) -> usize {
        self.cells.values().map(|v| v.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Get the cell size.
    pub fn cell_size(&self) -> i16 {
        self.cell_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use verryte_core::Rng;

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

    #[test]
    fn line_between_includes_endpoints() {
        assert_eq!(
            line_between(Point::new(1, 1), Point::new(4, 2)),
            vec![
                Point::new(1, 1),
                Point::new(2, 1),
                Point::new(3, 2),
                Point::new(4, 2),
            ]
        );
    }

    #[test]
    fn line_iter_yields_same_points_as_line_between() {
        let start = Point::new(0, 0);
        let end = Point::new(5, 3);
        let iter_points: Vec<Point> = LineIter::new(start, end).collect();
        let vec_points = line_between(start, end);
        assert_eq!(iter_points, vec_points);
    }

    #[test]
    fn line_iter_handles_single_point() {
        let p = Point::new(3, 3);
        let mut iter = LineIter::new(p, p);
        assert_eq!(iter.next(), Some(p));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn line_iter_handles_vertical_line() {
        let points: Vec<Point> = LineIter::new(Point::new(2, 0), Point::new(2, 3)).collect();
        assert_eq!(
            points,
            vec![
                Point::new(2, 0),
                Point::new(2, 1),
                Point::new(2, 2),
                Point::new(2, 3),
            ]
        );
    }

    #[test]
    fn line_iter_can_short_circuit_early() {
        let mut iter = LineIter::new(Point::new(0, 0), Point::new(10, 10));
        assert_eq!(iter.next(), Some(Point::new(0, 0)));
        assert_eq!(iter.next(), Some(Point::new(1, 1)));
        drop(iter);
    }

    #[test]
    fn visible_points_respect_radius_and_blockers() {
        let grid = TileGrid::from_vec(
            5,
            3,
            vec![
                '.', '.', '#', '.', '.', '.', '.', '#', '.', '.', '.', '.', '.', '.', '.',
            ],
        )
        .unwrap();
        let visible = grid.visible_points(Point::new(0, 1), 4, |tile| *tile == '#');

        assert!(visible.contains(&Point::new(0, 1)));
        assert!(
            visible.contains(&Point::new(2, 1)),
            "blocking tile is visible"
        );
        assert!(
            !visible.contains(&Point::new(3, 1)),
            "blocked tile beyond wall is hidden"
        );
        assert!(
            !visible.contains(&Point::new(4, 2)),
            "outside Manhattan radius"
        );
    }

    #[test]
    fn shortest_path4_finds_cardinal_path_around_walls() {
        let grid = TileGrid::from_vec(
            5,
            4,
            vec![
                '.', '.', '.', '.', '.', '.', '#', '#', '#', '.', '.', '.', '.', '.', '.', '#',
                '#', '#', '.', '.',
            ],
        )
        .unwrap();

        let path = grid
            .shortest_path4(Point::new(0, 0), Point::new(4, 3), |_, tile| *tile == '.')
            .unwrap();

        assert_eq!(path.first(), Some(&Point::new(0, 0)));
        assert_eq!(path.last(), Some(&Point::new(4, 3)));
        assert_eq!(path.len(), 8);
        assert!(path
            .windows(2)
            .all(|pair| pair[0].manhattan_distance(pair[1]) == 1));
    }

    #[test]
    fn shortest_path4_returns_none_when_goal_is_blocked_or_out_of_bounds() {
        let grid = TileGrid::from_vec(3, 1, vec!['.', '#', '.']).unwrap();

        assert_eq!(
            grid.shortest_path4(Point::new(0, 0), Point::new(2, 0), |_, tile| *tile == '.'),
            None
        );
        assert_eq!(
            grid.shortest_path4(Point::new(0, 0), Point::new(3, 0), |_, tile| *tile == '.'),
            None
        );
    }

    #[test]
    fn nearest_path4_chooses_shortest_reachable_target() {
        let grid = TileGrid::from_vec(
            5,
            3,
            vec![
                '.', '.', '.', '.', '.', '.', '#', '#', '#', '.', '.', '.', '.', '.', '.',
            ],
        )
        .unwrap();

        let path = grid
            .nearest_path4(
                Point::new(0, 0),
                [Point::new(4, 0), Point::new(2, 2)],
                |_, tile| *tile == '.',
            )
            .unwrap();

        assert_eq!(path.first(), Some(&Point::new(0, 0)));
        assert_eq!(path.last(), Some(&Point::new(4, 0)));
        assert_eq!(path.len(), 5);
    }

    #[test]
    fn distance_to_nearest4_reports_shortest_reachable_distance() {
        let grid = TileGrid::from_vec(
            5,
            3,
            vec![
                '.', '.', '.', '.', '.', '.', '#', '#', '#', '.', '.', '.', '.', '.', '.',
            ],
        )
        .unwrap();

        let distance = grid.distance_to_nearest4(
            Point::new(0, 0),
            [Point::new(4, 0), Point::new(2, 2)],
            |_, tile| *tile == '.',
        );
        assert_eq!(distance, Some(4));
    }

    #[test]
    fn distance_to_nearest4_returns_none_when_targets_unreachable_or_empty() {
        let grid = TileGrid::from_vec(3, 1, vec!['.', '#', '.']).unwrap();
        assert_eq!(
            grid.distance_to_nearest4(Point::new(0, 0), [Point::new(2, 0)], |_, tile| *tile == '.'),
            None
        );
        assert_eq!(
            grid.distance_to_nearest4(Point::new(0, 0), Vec::<Point>::new(), |_, tile| *tile
                == '.'),
            None
        );
    }

    #[test]
    fn reachable_points4_walks_passable_region_in_bfs_order() {
        let grid = TileGrid::from_vec(
            4,
            3,
            vec!['.', '.', '#', '.', '.', '#', '.', '.', '.', '.', '.', '#'],
        )
        .unwrap();

        let reachable = grid.reachable_points4(Point::new(0, 0), |_, tile| *tile == '.');

        assert_eq!(
            reachable,
            vec![
                Point::new(0, 0),
                Point::new(0, 1),
                Point::new(1, 0),
                Point::new(0, 2),
                Point::new(1, 2),
                Point::new(2, 2),
                Point::new(2, 1),
                Point::new(3, 1),
                Point::new(3, 0),
            ]
        );
    }

    #[test]
    fn flood_fill4_fills_connected_region() {
        // 5x3 grid with a vertical wall at column 2 in rows 0-1.
        // Row 2 is fully open, connecting both sides.
        let grid = TileGrid::from_vec(
            5,
            3,
            vec![
                '.', '.', '#', '.', '.', '.', '.', '#', '.', '.', '.', '.', '.', '.', '.',
            ],
        )
        .unwrap();

        // From (0,0), flood fill reaches left side + bottom row + right side via bottom.
        let region = grid.flood_fill4(Point::new(0, 0), |_, tile| *tile == '.');
        assert_eq!(region.len(), 13);
        assert!(region.contains(&Point::new(0, 0)));
        assert!(region.contains(&Point::new(4, 2)));
    }

    #[test]
    fn flood_fill4_is_blocked_by_walls_on_all_sides() {
        // 5x3 grid with walls fully enclosing (1,1).
        let grid = TileGrid::from_vec(
            5,
            3,
            vec![
                '#', '#', '#', '#', '#', '#', '.', '#', '#', '#', '#', '#', '#', '#', '#',
            ],
        )
        .unwrap();

        let region = grid.flood_fill4(Point::new(1, 1), |_, tile| *tile == '.');
        assert_eq!(region.len(), 1);
        assert_eq!(region[0], Point::new(1, 1));
    }

    #[test]
    fn flood_fill4_returns_empty_for_non_matching_start() {
        let grid = TileGrid::from_vec(3, 1, vec!['.', '#', '.']).unwrap();
        let region = grid.flood_fill4(Point::new(1, 0), |_, tile| *tile == '.');
        assert!(region.is_empty());
    }

    #[test]
    fn flood_fill4_stops_at_boundaries() {
        // 5x3 grid: top two rows are '.', bottom row is '#'.
        let grid = TileGrid::from_vec(
            5,
            3,
            vec![
                '.', '.', '.', '.', '.', '.', '.', '.', '.', '.', '#', '#', '#', '#', '#',
            ],
        )
        .unwrap();
        let region = grid.flood_fill4(Point::new(0, 0), |_, tile| *tile == '.');
        assert_eq!(region.len(), 10);
    }

    #[test]
    fn count_regions4_counts_disconnected_areas() {
        let grid = TileGrid::from_vec(7, 1, vec!['.', '.', '#', '.', '.', '.', '#']).unwrap();

        assert_eq!(grid.count_regions4(|_, tile| *tile == '.'), 2);
    }

    #[test]
    fn count_regions4_returns_one_for_fully_connected() {
        let grid = TileGrid::from_vec(3, 3, vec!['.'; 9]).unwrap();
        assert_eq!(grid.count_regions4(|_, tile| *tile == '.'), 1);
    }

    #[test]
    fn count_regions4_returns_zero_for_no_matches() {
        let grid = TileGrid::from_vec(3, 3, vec!['#'; 9]).unwrap();
        assert_eq!(grid.count_regions4(|_, tile| *tile == '.'), 0);
    }

    #[test]
    fn direction8_deltas_and_opposites() {
        assert_eq!(Direction8::North.delta(), (0, -1));
        assert_eq!(Direction8::NorthEast.delta(), (1, -1));
        assert_eq!(Direction8::SouthWest.delta(), (-1, 1));
        assert_eq!(Direction8::North.opposite(), Direction8::South);
        assert_eq!(Direction8::NorthEast.opposite(), Direction8::SouthWest);
        assert!(Direction8::North.is_cardinal());
        assert!(!Direction8::NorthEast.is_cardinal());
        assert_eq!(
            Direction8::to_direction(Direction8::North),
            Some(Direction::North)
        );
        assert_eq!(Direction8::to_direction(Direction8::NorthEast), None);
        assert_eq!(
            Direction8::from_direction(Direction::East),
            Direction8::East
        );
    }

    #[test]
    fn point_neighbors8_includes_diagonals() {
        let p = Point::new(3, 3);
        let n = p.neighbors8();
        assert_eq!(n.len(), 8);
        assert!(n.contains(&Point::new(3, 2))); // North
        assert!(n.contains(&Point::new(4, 2))); // NorthEast
        assert!(n.contains(&Point::new(2, 4))); // SouthWest
        assert!(n.contains(&Point::new(4, 4))); // SouthEast
    }

    #[test]
    fn chebyshev_distance_is_king_move_count() {
        let a = Point::new(0, 0);
        assert_eq!(a.chebyshev_distance(Point::new(3, 3)), 3);
        assert_eq!(a.chebyshev_distance(Point::new(5, 2)), 5);
        assert_eq!(a.chebyshev_distance(Point::new(0, 7)), 7);
    }

    #[test]
    fn tile_grid_neighbors8_clips_to_bounds() {
        let grid = TileGrid::from_vec(3, 3, vec![0, 1, 2, 3, 4, 5, 6, 7, 8]).unwrap();
        let neighbors = grid.neighbors8(Point::new(1, 1));
        assert_eq!(neighbors.len(), 8);
        let vals: Vec<i32> = neighbors.iter().map(|(_, v)| **v).collect();
        assert!(vals.contains(&0));
        assert!(vals.contains(&8));
    }

    #[test]
    fn shortest_path8_finds_diagonal_path() {
        let grid = TileGrid::from_vec(5, 5, vec!['.'; 25]).unwrap();

        let path = grid
            .shortest_path8(Point::new(0, 0), Point::new(4, 4), |_, tile| *tile == '.')
            .unwrap();

        assert_eq!(path.first(), Some(&Point::new(0, 0)));
        assert_eq!(path.last(), Some(&Point::new(4, 4)));
        // Diagonal path should be 5 steps (all diagonals).
        assert_eq!(path.len(), 5);
        // Each step should be adjacent in 8-directional sense.
        assert!(path
            .windows(2)
            .all(|pair| pair[0].chebyshev_distance(pair[1]) == 1));
    }

    #[test]
    fn shortest_path8_prefers_diagonal_when_shorter() {
        let grid = TileGrid::from_vec(3, 3, vec!['.'; 9]).unwrap();

        // Cardinal path would be 5 steps; diagonal is 3.
        let path = grid
            .shortest_path8(Point::new(0, 0), Point::new(2, 2), |_, tile| *tile == '.')
            .unwrap();

        assert_eq!(path.len(), 3);
        assert_eq!(path[1], Point::new(1, 1));
    }

    #[test]
    fn shortest_path8_handles_same_start_and_goal() {
        let grid = TileGrid::from_vec(3, 3, vec!['.'; 9]).unwrap();
        let path = grid
            .shortest_path8(Point::new(1, 1), Point::new(1, 1), |_, tile| *tile == '.')
            .unwrap();
        assert_eq!(path, vec![Point::new(1, 1)]);
    }

    #[test]
    fn shortest_path8_returns_none_when_blocked() {
        let grid =
            TileGrid::from_vec(3, 3, vec!['.', '#', '.', '#', '#', '#', '.', '#', '.']).unwrap();
        assert_eq!(
            grid.shortest_path8(Point::new(0, 0), Point::new(2, 2), |_, tile| *tile == '.'),
            None
        );
    }

    #[test]
    fn nearest_path8_chooses_closest_target_diagonally() {
        let grid = TileGrid::from_vec(5, 5, vec!['.'; 25]).unwrap();

        let path = grid
            .nearest_path8(
                Point::new(0, 0),
                [Point::new(4, 4), Point::new(2, 0)],
                |_, tile| *tile == '.',
            )
            .unwrap();

        assert_eq!(path.first(), Some(&Point::new(0, 0)));
        assert_eq!(path.last(), Some(&Point::new(2, 0)));
        assert_eq!(path.len(), 3);
    }

    #[test]
    fn nearest_path8_returns_none_when_all_targets_unreachable() {
        let grid = TileGrid::from_vec(5, 5, vec!['.'; 25]).unwrap();
        assert_eq!(
            grid.nearest_path8(Point::new(0, 0), Vec::<Point>::new(), |_, tile| *tile
                == '.'),
            None
        );
    }

    #[test]
    fn reachable_points8_walks_passable_region_with_diagonals() {
        let grid = TileGrid::from_vec(
            5,
            3,
            vec![
                '.', '.', '#', '.', '.', '.', '.', '#', '.', '.', '.', '.', '.', '.', '.',
            ],
        )
        .unwrap();

        let reachable = grid.reachable_points8(Point::new(0, 0), |_, tile| *tile == '.');

        // With 8-directional movement, the wall at column 2 can be bypassed diagonally
        // through rows that connect (row 2 is fully open).
        assert!(reachable.contains(&Point::new(0, 0)));
        assert!(reachable.contains(&Point::new(4, 2)));
        // All 13 open tiles should be reachable with 8-directional movement.
        assert_eq!(reachable.len(), 13);
    }

    #[test]
    fn reachable_points8_returns_empty_for_out_of_bounds() {
        let grid = TileGrid::from_vec(3, 3, vec!['.'; 9]).unwrap();
        assert!(grid
            .reachable_points8(Point::new(-1, 0), |_, tile| *tile == '.')
            .is_empty());
    }

    #[test]
    fn random_walk_fill4_carves_floor_from_seed() {
        let mut grid = TileGrid::new(10, 10, '#');
        grid.random_walk_fill4(Point::new(5, 5), 100, '.', 42);

        // Start position should be floor.
        assert_eq!(grid.get(Point::new(5, 5)), Some(&'.'));
        // At least some floor tiles should exist.
        let floor_count = grid.tiles().iter().filter(|&&t| t == '.').count();
        assert!(
            floor_count > 10,
            "expected >10 floor tiles, got {floor_count}"
        );
        // All floor tiles should be connected (single random walk).
        let region = grid.flood_fill4(Point::new(5, 5), |_, &t| t == '.');
        assert_eq!(region.len(), floor_count);
    }

    #[test]
    fn random_walk_fill4_is_reproducible_with_same_seed() {
        let mut grid1 = TileGrid::new(10, 10, '#');
        grid1.random_walk_fill4(Point::new(5, 5), 50, '.', 123);

        let mut grid2 = TileGrid::new(10, 10, '#');
        grid2.random_walk_fill4(Point::new(5, 5), 50, '.', 123);

        assert_eq!(grid1, grid2);
    }

    #[test]
    fn random_walk_fill4_produces_different_results_with_different_seeds() {
        let mut grid1 = TileGrid::new(10, 10, '#');
        grid1.random_walk_fill4(Point::new(5, 5), 50, '.', 1);

        let mut grid2 = TileGrid::new(10, 10, '#');
        grid2.random_walk_fill4(Point::new(5, 5), 50, '.', 999);

        assert_ne!(grid1, grid2);
    }

    #[test]
    fn random_walk_fill4_does_nothing_for_out_of_bounds_start() {
        let mut grid = TileGrid::new(5, 5, '#');
        grid.random_walk_fill4(Point::new(-1, -1), 100, '.', 42);
        assert!(grid.tiles().iter().all(|&t| t == '#'));
    }

    #[test]
    fn is_line_of_sight_clear_returns_true_when_no_blockers() {
        let grid = TileGrid::from_vec(5, 1, vec!['.', '.', '.', '.', '.']).unwrap();
        assert!(grid.is_line_of_sight_clear(Point::new(0, 0), Point::new(4, 0), |t| *t == '#'));
    }

    #[test]
    fn is_line_of_sight_clear_returns_false_when_blocked() {
        let grid = TileGrid::from_vec(5, 1, vec!['.', '.', '#', '.', '.']).unwrap();
        assert!(!grid.is_line_of_sight_clear(Point::new(0, 0), Point::new(4, 0), |t| *t == '#'));
    }

    #[test]
    fn is_line_of_sight_clear_ignores_endpoints() {
        // The target tile itself is a wall, but LOS should still be clear
        // because the target is the thing being looked at.
        let grid = TileGrid::from_vec(3, 1, vec!['.', '.', '#']).unwrap();
        assert!(grid.is_line_of_sight_clear(Point::new(0, 0), Point::new(2, 0), |t| *t == '#'));
    }

    #[test]
    fn is_line_of_sight_clear_returns_false_for_out_of_bounds() {
        let grid = TileGrid::from_vec(3, 3, vec!['.'; 9]).unwrap();
        assert!(!grid.is_line_of_sight_clear(Point::new(0, 0), Point::new(5, 5), |t| *t == '#'));
        assert!(!grid.is_line_of_sight_clear(Point::new(-1, 0), Point::new(2, 2), |t| *t == '#'));
    }

    #[test]
    fn is_line_of_sight_clear_handles_same_point() {
        let grid = TileGrid::from_vec(3, 3, vec!['.'; 9]).unwrap();
        assert!(grid.is_line_of_sight_clear(Point::new(1, 1), Point::new(1, 1), |t| *t == '#'));
    }

    #[test]
    fn is_line_of_sight_clear_works_for_diagonal_lines() {
        // 3x3 grid with a blocker at (1,1).
        let grid =
            TileGrid::from_vec(3, 3, vec!['.', '.', '.', '.', '#', '.', '.', '.', '.']).unwrap();
        // Diagonal from (0,0) to (2,2) passes through (1,1).
        assert!(!grid.is_line_of_sight_clear(Point::new(0, 0), Point::new(2, 2), |t| *t == '#'));
        // But (0,0) to (2,0) is clear.
        assert!(grid.is_line_of_sight_clear(Point::new(0, 0), Point::new(2, 0), |t| *t == '#'));
    }

    #[test]
    fn bsp_dungeon_carves_rooms_and_corridors() {
        let mut grid = TileGrid::new(30, 20, '#');
        let centers = grid.generate_bsp_dungeon('#', '.', 3, 42);

        // BSP should produce multiple rooms.
        assert!(
            centers.len() >= 2,
            "expected >= 2 rooms, got {}",
            centers.len()
        );

        // All room centers should be floor.
        for &center in &centers {
            assert_eq!(grid.get(center), Some(&'.'));
        }

        // All rooms should be connected (single flood-fill from first center).
        let floor_count = grid.tiles().iter().filter(|&&t| t == '.').count();
        assert!(
            floor_count > 20,
            "expected >20 floor tiles, got {floor_count}"
        );
        let region = grid.flood_fill4(centers[0], |_, &t| t == '.');
        assert_eq!(
            region.len(),
            floor_count,
            "all floor tiles should be connected"
        );
    }

    #[test]
    fn bsp_dungeon_is_reproducible_with_same_seed() {
        let mut grid1 = TileGrid::new(25, 25, '#');
        grid1.generate_bsp_dungeon('#', '.', 3, 99);

        let mut grid2 = TileGrid::new(25, 25, '#');
        grid2.generate_bsp_dungeon('#', '.', 3, 99);

        assert_eq!(grid1, grid2);
    }

    #[test]
    fn bsp_dungeon_differs_with_different_seeds() {
        let mut grid1 = TileGrid::new(25, 25, '#');
        grid1.generate_bsp_dungeon('#', '.', 3, 1);

        let mut grid2 = TileGrid::new(25, 25, '#');
        grid2.generate_bsp_dungeon('#', '.', 3, 2);

        assert_ne!(grid1, grid2);
    }

    #[test]
    fn bsp_dungeon_returns_empty_for_too_small_grid() {
        let mut grid = TileGrid::new(2, 2, '#');
        let centers = grid.generate_bsp_dungeon('#', '.', 3, 42);
        assert!(centers.is_empty());
    }

    #[test]
    fn count_matching_returns_correct_count() {
        let grid = TileGrid::from_vec(5, 1, vec!['.', '.', '#', '.', '#']).unwrap();
        assert_eq!(grid.count_matching(|_, &t| t == '.'), 3);
        assert_eq!(grid.count_matching(|_, &t| t == '#'), 2);
        assert_eq!(grid.count_matching(|_, &t| t == 'x'), 0);
    }

    #[test]
    fn density_returns_fraction() {
        let grid = TileGrid::from_vec(4, 1, vec!['.', '.', '#', '#']).unwrap();
        assert!((grid.density(|_, &t| t == '.') - 0.5).abs() < f32::EPSILON);
        assert!((grid.density(|_, &t| t == '#') - 0.5).abs() < f32::EPSILON);
        assert!((grid.density(|_, &t| t == 'x') - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn density_returns_zero_for_empty_grid() {
        let grid: TileGrid<char> = TileGrid::new(0, 0, '.');
        assert!((grid.density(|_, &t| t == '.') - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn bounding_box_of_returns_none_when_no_match() {
        let grid = TileGrid::from_vec(3, 3, vec!['.'; 9]).unwrap();
        assert!(grid.bounding_box_of(|_, &t| t == '#').is_none());
    }

    #[test]
    fn bounding_box_of_returns_tight_rect() {
        // 5x5 grid with '#' at (1,1), (3,1), (1,3), (3,3).
        let mut grid = TileGrid::new(5, 5, '.');
        grid.set(Point::new(1, 1), '#');
        grid.set(Point::new(3, 1), '#');
        grid.set(Point::new(1, 3), '#');
        grid.set(Point::new(3, 3), '#');

        let bounds = grid.bounding_box_of(|_, &t| t == '#').unwrap();
        assert_eq!(bounds.x, 1);
        assert_eq!(bounds.y, 1);
        assert_eq!(bounds.width, 3);
        assert_eq!(bounds.height, 3);
    }

    #[test]
    fn bounds_contains_and_center() {
        let b = Bounds::new(2, 3, 5, 7);
        assert_eq!(b.right(), 7);
        assert_eq!(b.bottom(), 10);
        assert!(b.contains(Point::new(2, 3)));
        assert!(b.contains(Point::new(6, 9)));
        assert!(!b.contains(Point::new(1, 3)));
        assert!(!b.contains(Point::new(7, 3)));
        assert_eq!(b.center(), Point::new(4, 6));
    }

    #[test]
    fn field_of_view_includes_origin() {
        let grid = TileGrid::from_vec(5, 5, vec!['.'; 25]).unwrap();
        let fov = grid.field_of_view(Point::new(2, 2), 10, |t| *t == '#');
        assert!(fov.contains(&Point::new(2, 2)));
    }

    #[test]
    fn field_of_view_sees_all_in_open_area() {
        let grid = TileGrid::from_vec(5, 5, vec!['.'; 25]).unwrap();
        let fov = grid.field_of_view(Point::new(2, 2), 3, |t| *t == '#');
        // Within radius 3 from center, all tiles should be visible.
        for point in grid.points() {
            if point.manhattan_distance(Point::new(2, 2)) <= 3 {
                assert!(fov.contains(&point), "point {point:?} should be visible");
            }
        }
    }

    #[test]
    fn field_of_view_blocks_behind_wall() {
        // 7x1 grid with a wall at position 3.
        let grid = TileGrid::from_vec(7, 1, vec!['.', '.', '.', '#', '.', '.', '.']).unwrap();

        let fov = grid.field_of_view(Point::new(0, 0), 6, |t| *t == '#');

        // Wall itself should be visible.
        assert!(fov.contains(&Point::new(3, 0)));
        // Tiles behind the wall should NOT be visible.
        assert!(!fov.contains(&Point::new(4, 0)));
        assert!(!fov.contains(&Point::new(5, 0)));
        assert!(!fov.contains(&Point::new(6, 0)));
    }

    #[test]
    fn field_of_view_respects_radius() {
        let grid = TileGrid::from_vec(11, 11, vec!['.'; 121]).unwrap();
        let fov = grid.field_of_view(Point::new(5, 5), 2, |t| *t == '#');

        // All visible tiles should be within radius 2.
        for &point in &fov {
            assert!(
                point.manhattan_distance(Point::new(5, 5)) <= 2,
                "point {point:?} exceeds radius"
            );
        }
    }

    #[test]
    fn field_of_view_returns_empty_for_out_of_bounds() {
        let grid = TileGrid::from_vec(5, 5, vec!['.'; 25]).unwrap();
        let fov = grid.field_of_view(Point::new(-1, -1), 5, |t| *t == '#');
        assert!(fov.is_empty());
    }

    #[test]
    fn spatial_hash_insert_and_query() {
        let mut hash = SpatialHash::<u32>::new(3);
        hash.insert(Point::new(0, 0), 1);
        hash.insert(Point::new(1, 0), 2);
        hash.insert(Point::new(10, 10), 3);

        let nearby: Vec<u32> = hash.query(Point::new(0, 0), 2).copied().collect();
        assert_eq!(nearby.len(), 2);
        assert!(nearby.contains(&1));
        assert!(nearby.contains(&2));
        assert!(!nearby.contains(&3));
    }

    #[test]
    fn spatial_hash_remove() {
        let mut hash = SpatialHash::<u32>::new(3);
        hash.insert(Point::new(0, 0), 1);
        hash.insert(Point::new(0, 0), 2);
        assert_eq!(hash.query(Point::new(0, 0), 1).count(), 2);

        hash.remove(Point::new(0, 0), &1);
        let nearby: Vec<u32> = hash.query(Point::new(0, 0), 1).copied().collect();
        assert_eq!(nearby, vec![2]);
    }

    #[test]
    fn spatial_hash_cell_size_affects_grouping() {
        let mut hash = SpatialHash::<u32>::new(5);
        hash.insert(Point::new(0, 0), 1);
        hash.insert(Point::new(4, 4), 2); // Same cell (0,0) with cell_size=5
        hash.insert(Point::new(5, 5), 3); // Different cell (1,1)

        // Query with radius 8 to include (4,4) which is manhattan distance 8 from (0,0).
        let nearby: Vec<u32> = hash.query(Point::new(0, 0), 8).copied().collect();
        assert_eq!(nearby.len(), 2);
        assert!(nearby.contains(&1));
        assert!(nearby.contains(&2));
        assert!(!nearby.contains(&3));
    }

    #[test]
    fn spatial_hash_handles_empty_query() {
        let hash = SpatialHash::<u32>::new(3);
        assert_eq!(hash.query(Point::new(0, 0), 5).count(), 0);
    }

    #[test]
    fn spatial_hash_clear_removes_all() {
        let mut hash = SpatialHash::<u32>::new(3);
        hash.insert(Point::new(0, 0), 1);
        hash.insert(Point::new(100, 100), 2);
        hash.clear();
        assert_eq!(hash.query(Point::new(0, 0), 200).count(), 0);
    }

    #[test]
    fn spatial_hash_len_counts_all_entries() {
        let mut hash = SpatialHash::<u32>::new(3);
        hash.insert(Point::new(0, 0), 1);
        hash.insert(Point::new(0, 0), 2);
        hash.insert(Point::new(10, 10), 3);
        assert_eq!(hash.len(), 3);
    }

    #[test]
    fn spatial_hash_nearest_finds_closest() {
        let mut hash = SpatialHash::<Point>::new(3);
        let origin = Point::new(0, 0);
        let far = Point::new(10, 10);
        let near = Point::new(2, 0);
        hash.insert(origin, origin);
        hash.insert(far, far);
        hash.insert(near, near);

        // Find nearest to (5, 0). Expected: near (2,0) at distance 3, not origin (0,0) at distance 5.
        let center = Point::new(5, 0);
        let nearest = hash.nearest(center, 20, |a, b, c| {
            a.manhattan_distance(c).cmp(&b.manhattan_distance(c))
        });
        assert_eq!(nearest, Some(&near));
    }

    #[test]
    fn place_rooms_carves_rooms_on_wall_background() {
        let mut grid = TileGrid::from_vec(20, 20, vec!['.'; 400]).unwrap();
        let mut rng = Rng::seed(42);
        let centers = grid.place_rooms(5, 3, 6, || '#', || '.', &mut || rng.next_u64());

        assert!(!centers.is_empty());
        // Room centers should be within bounds.
        for c in &centers {
            assert!(grid.in_bounds(*c));
        }
        // At least one room center should have floor under it.
        let has_floor = centers.iter().any(|c| *grid.get(*c).unwrap() == '.');
        assert!(has_floor);
    }

    #[test]
    fn place_rooms_returns_deterministic_results() {
        let mut grid1 = TileGrid::from_vec(20, 20, vec!['.'; 400]).unwrap();
        let mut grid2 = TileGrid::from_vec(20, 20, vec!['.'; 400]).unwrap();
        let mut rng1 = Rng::seed(99);
        let mut rng2 = Rng::seed(99);

        let c1 = grid1.place_rooms(3, 2, 5, || '#', || '.', &mut || rng1.next_u64());
        let c2 = grid2.place_rooms(3, 2, 5, || '#', || '.', &mut || rng2.next_u64());

        assert_eq!(c1, c2);
    }

    #[test]
    fn place_rooms_respects_max_rooms() {
        let mut grid = TileGrid::from_vec(50, 50, vec!['.'; 2500]).unwrap();
        let mut rng = Rng::seed(42);
        let centers = grid.place_rooms(3, 3, 5, || '#', || '.', &mut || rng.next_u64());
        assert!(centers.len() <= 3);
    }
}
