use crate::{line_between, Bounds, Direction, Direction8, GridError, LineIter, Point, Rect, Size};

use std::collections::{HashMap, VecDeque};
use std::ops::{Index, IndexMut};

pub(crate) struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    pub(crate) fn new(seed: u64) -> Self {
        Self { state: seed | 1 }
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }
}

/// A typed, fixed-size rectangular tile grid.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(bound = "T: serde::Serialize + serde::de::DeserializeOwned")
)]
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

    /// Construct a tile grid from a multi-line ASCII string.
    ///
    /// Each character in the string is mapped to a tile via `f`. Lines are
    /// separated by `\n`. The grid width is the length of the longest line;
    /// shorter lines are padded with the tile produced by `f(' ', y)`.
    /// Empty input produces a 0×0 grid.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let ascii = "\
    /// #####
    /// #...#
    /// #.@.#
    /// #...#
    /// #####";
    /// let grid = TileGrid::from_ascii(ascii, |ch, _x, _y| match ch {
    ///     '#' => Tile::Wall,
    ///     '.' => Tile::Floor,
    ///     '@' => Tile::Floor,
    ///     _ => Tile::Floor,
    /// });
    /// assert_eq!(grid.width(), 5);
    /// assert_eq!(grid.height(), 5);
    /// ```
    pub fn from_ascii<F>(input: &str, mut f: F) -> Self
    where
        F: FnMut(char, u16, u16) -> T,
    {
        if input.is_empty() {
            return Self {
                size: Size::new(0, 0),
                tiles: Vec::new(),
            };
        }
        let lines: Vec<&str> = input.split('\n').collect();
        let height = lines.len() as u16;
        let width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as u16;
        let mut tiles = Vec::with_capacity((width as usize) * (height as usize));
        for (y, line) in lines.iter().enumerate() {
            for (x, ch) in line.chars().enumerate() {
                tiles.push(f(ch, x as u16, y as u16));
            }
            let line_len = line.chars().count() as u16;
            for x in line_len..width {
                tiles.push(f(' ', x, y as u16));
            }
        }
        Self {
            size: Size::new(width, height),
            tiles,
        }
    }

    /// Construct a grid by calling a closure for each (x, y) position.
    ///
    /// The closure receives the coordinates and returns the tile value.
    /// Useful for procedural generation, noise functions, or test fixtures.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let grid: TileGrid<i32> = TileGrid::from_fn(4, 3, |x, y| (x + y) as i32);
    /// assert_eq!(grid.get(Point::new(1, 2)), Some(&3));
    /// ```
    pub fn from_fn<F>(width: u16, height: u16, mut f: F) -> Self
    where
        F: FnMut(u16, u16) -> T,
    {
        let mut tiles = Vec::with_capacity((width as usize) * (height as usize));
        for y in 0..height {
            for x in 0..width {
                tiles.push(f(x, y));
            }
        }
        Self {
            size: Size::new(width, height),
            tiles,
        }
    }

    /// Transform each tile into a different type, producing a new grid.
    ///
    /// The mapping function receives the point and a reference to the tile.
    /// Useful for converting a logical tile map into a display representation.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let logical: TileGrid<bool> = TileGrid::new(3, 2, true);
    /// let display: TileGrid<char> = logical.map_tiles(|_, &b| if b { '.' } else { '#' });
    /// ```
    pub fn map_tiles<U, F>(&self, f: F) -> TileGrid<U>
    where
        F: Fn(Point, &T) -> U,
    {
        let mut tiles = Vec::with_capacity(self.tiles.len());
        let width = self.size.width as i16;
        for y in 0..self.size.height as i16 {
            for x in 0..width {
                let p = Point::new(x, y);
                let idx = (y as usize) * (width as usize) + (x as usize);
                tiles.push(f(p, &self.tiles[idx]));
            }
        }
        TileGrid {
            size: self.size,
            tiles,
        }
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

    /// Return the full bounds of the grid (origin at 0,0).
    pub fn bounds(&self) -> Bounds {
        Bounds::new(0, 0, self.size.width, self.size.height)
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

    /// Returns `true` if the point is on the outer edge (perimeter) of the grid.
    pub fn is_on_edge(&self, point: Point) -> bool {
        self.in_bounds(point)
            && (point.x == 0
                || point.y == 0
                || point.x == self.size.width as i16 - 1
                || point.y == self.size.height as i16 - 1)
    }

    /// Returns a list of all points on the perimeter of the grid.
    pub fn perimeter_points(&self) -> Vec<Point> {
        let mut points = Vec::new();
        let w = self.size.width as i16;
        let h = self.size.height as i16;
        if w == 0 || h == 0 {
            return points;
        }
        // Top and bottom edges
        for x in 0..w {
            points.push(Point::new(x, 0));
            if h > 1 {
                points.push(Point::new(x, h - 1));
            }
        }
        // Left and right edges (excluding corners)
        for y in 1..(h - 1) {
            points.push(Point::new(0, y));
            if w > 1 {
                points.push(Point::new(w - 1, y));
            }
        }
        points
    }

    /// Returns `true` if the point is in bounds and its tile matches the predicate.
    ///
    /// Combines bounds checking and tile inspection in one call. Useful for
    /// guard clauses in movement or interaction logic.
    pub fn contains_point<F>(&self, point: Point, matches: F) -> bool
    where
        F: Fn(&T) -> bool,
    {
        self.get(point).is_some_and(matches)
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

    /// Find the shortest 8-directional path between two in-bounds points, with a maximum path cost limit.
    ///
    /// If the path's tentative cost exceeds `max_cost`, the pathfinder will not expand beyond that point.
    /// Returns None if no path exists within `max_cost`.
    pub fn shortest_path8_limit<F>(
        &self,
        start: Point,
        goal: Point,
        max_cost: u32,
        passable: F,
    ) -> Option<Vec<Point>>
    where
        F: Fn(Point, &T) -> bool,
    {
        if !self.in_bounds(start) || !self.in_bounds(goal) {
            return None;
        }
        if start == goal {
            return Some(vec![start]);
        }

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
            if current_g >= max_cost {
                continue;
            }

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
                if tentative_g > max_cost {
                    continue;
                }

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

    /// Find the shortest 8-directional path from `start` to the closest of the specified `goals` points,
    /// with an optional maximum path cost limit.
    ///
    /// The heuristic is the minimum Chebyshev distance from the current node to any of the goals.
    pub fn shortest_path8_multi_goal<F>(
        &self,
        start: Point,
        goals: &[Point],
        max_cost: Option<u32>,
        passable: F,
    ) -> Option<Vec<Point>>
    where
        F: Fn(Point, &T) -> bool,
    {
        if !self.in_bounds(start) || goals.is_empty() {
            return None;
        }

        let valid_goals: Vec<Point> = goals
            .iter()
            .copied()
            .filter(|&g| self.in_bounds(g))
            .collect();
        if valid_goals.is_empty() {
            return None;
        }

        if valid_goals.contains(&start) {
            return Some(vec![start]);
        }

        const CARDINAL_COST: u32 = 10;
        const DIAGONAL_COST: u32 = 14;
        let limit = max_cost.unwrap_or(u32::MAX);

        let mut g_score = HashMap::new();
        let mut came_from = HashMap::new();
        let mut frontier = std::collections::BinaryHeap::new();

        g_score.insert(start, 0u32);

        let min_h = valid_goals
            .iter()
            .map(|&g| start.chebyshev_distance(g) as u32 * CARDINAL_COST)
            .min()
            .unwrap();
        frontier.push(std::cmp::Reverse((min_h, start)));

        while let Some(std::cmp::Reverse((_f, current))) = frontier.pop() {
            if valid_goals.contains(&current) {
                let mut path = vec![current];
                let mut step = current;
                while step != start {
                    step = came_from[&step];
                    path.push(step);
                }
                path.reverse();
                return Some(path);
            }

            let current_g = *g_score.get(&current).unwrap();
            if current_g >= limit {
                continue;
            }

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
                if tentative_g > limit {
                    continue;
                }

                if tentative_g < *g_score.get(&neighbor).unwrap_or(&u32::MAX) {
                    came_from.insert(neighbor, current);
                    g_score.insert(neighbor, tentative_g);
                    let h = valid_goals
                        .iter()
                        .map(|&g| neighbor.chebyshev_distance(g) as u32 * CARDINAL_COST)
                        .min()
                        .unwrap();
                    frontier.push(std::cmp::Reverse((tentative_g + h, neighbor)));
                }
            }
        }

        None
    }

    /// Find the shortest cardinal path from `start` to the closest of the specified `goals` points,
    /// with an optional maximum path cost limit.
    ///
    /// The heuristic is the minimum Manhattan distance from the current node to any of the goals.
    pub fn shortest_path4_multi_goal<F>(
        &self,
        start: Point,
        goals: &[Point],
        max_cost: Option<u32>,
        passable: F,
    ) -> Option<Vec<Point>>
    where
        F: Fn(Point, &T) -> bool,
    {
        if !self.in_bounds(start) || goals.is_empty() {
            return None;
        }

        let valid_goals: Vec<Point> = goals
            .iter()
            .copied()
            .filter(|&g| self.in_bounds(g))
            .collect();
        if valid_goals.is_empty() {
            return None;
        }

        if valid_goals.contains(&start) {
            return Some(vec![start]);
        }

        const STEP_COST: u32 = 10;
        let limit = max_cost.unwrap_or(u32::MAX);

        let mut g_score = HashMap::new();
        let mut came_from = HashMap::new();
        let mut frontier = std::collections::BinaryHeap::new();

        g_score.insert(start, 0u32);

        let min_h = valid_goals
            .iter()
            .map(|&g| start.manhattan_distance(g) as u32 * STEP_COST)
            .min()
            .unwrap();
        frontier.push(std::cmp::Reverse((min_h, start)));

        while let Some(std::cmp::Reverse((_f, current))) = frontier.pop() {
            if valid_goals.contains(&current) {
                let mut path = vec![current];
                let mut step = current;
                while step != start {
                    step = came_from[&step];
                    path.push(step);
                }
                path.reverse();
                return Some(path);
            }

            let current_g = *g_score.get(&current).unwrap();
            if current_g >= limit {
                continue;
            }

            for neighbor in current.neighbors4() {
                let Some(tile) = self.get(neighbor) else {
                    continue;
                };
                if !passable(neighbor, tile) {
                    continue;
                }

                let tentative_g = current_g + STEP_COST;
                if tentative_g > limit {
                    continue;
                }

                if tentative_g < *g_score.get(&neighbor).unwrap_or(&u32::MAX) {
                    came_from.insert(neighbor, current);
                    g_score.insert(neighbor, tentative_g);
                    let h = valid_goals
                        .iter()
                        .map(|&g| neighbor.manhattan_distance(g) as u32 * STEP_COST)
                        .min()
                        .unwrap();
                    frontier.push(std::cmp::Reverse((tentative_g + h, neighbor)));
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

    /// Swap the tiles at two points. Returns `false` if either point is out of bounds.
    ///
    /// Useful for puzzle mechanics, sliding tiles, or rearranging map content.
    pub fn swap(&mut self, a: Point, b: Point) -> bool {
        let Some(idx_a) = self.index(a) else {
            return false;
        };
        let Some(idx_b) = self.index(b) else {
            return false;
        };
        self.tiles.swap(idx_a, idx_b);
        true
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

    /// Iterate over points within the provided bounds, clipped to the grid.
    pub fn points_in(&self, bounds: Bounds) -> impl Iterator<Item = Point> {
        let x0 = bounds.x.min(self.size.width);
        let y0 = bounds.y.min(self.size.height);
        let x1 = bounds.right().min(self.size.width);
        let y1 = bounds.bottom().min(self.size.height);
        let x0 = x0 as i16;
        let y0 = y0 as i16;
        let x1 = x1 as i16;
        let y1 = y1 as i16;
        (y0..y1).flat_map(move |y| (x0..x1).map(move |x| Point { x, y }))
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

    /// Transform all tiles in place using a function that receives each point and tile.
    ///
    /// Useful for applying terrain rules, erosion, or other cell-wise operations
    /// that depend on position.
    pub fn map_in_place<F>(&mut self, mut f: F)
    where
        F: FnMut(Point, &T) -> T,
    {
        let width = self.size.width as i16;
        for y in 0..self.size.height as i16 {
            for x in 0..width {
                let p = Point::new(x, y);
                let new_tile = f(
                    p,
                    &self.tiles[(y as usize) * (width as usize) + (x as usize)],
                );
                self.tiles[(y as usize) * (width as usize) + (x as usize)] = new_tile;
            }
        }
    }

    pub fn fill(&mut self, tile: T)
    where
        T: Clone,
    {
        for t in &mut self.tiles {
            *t = tile.clone();
        }
    }

    /// Fill a rectangular region with a tile, clipped to grid bounds.
    ///
    /// The region is specified by its top-left corner `(x, y)` and dimensions.
    /// Out-of-bounds areas are silently skipped.
    pub fn fill_rect(&mut self, x: i16, y: i16, width: u16, height: u16, tile: T)
    where
        T: Clone,
    {
        let w = self.width() as i16;
        let h = self.height() as i16;
        let x_end = (x + width as i16).min(w);
        let y_end = (y + height as i16).min(h);
        for cy in y.max(0)..y_end {
            for cx in x.max(0)..x_end {
                self.set(Point::new(cx, cy), tile.clone());
            }
        }
    }

    /// Extract a rectangular sub-region as a new grid.
    ///
    /// The region starts at `(x, y)` with the given `width` and `height`.
    /// Areas outside the source grid are filled with `fill`. Useful for
    /// viewport/camera extraction or chunking large maps.
    pub fn crop(&self, x: i16, y: i16, width: u16, height: u16, fill: T) -> TileGrid<T>
    where
        T: Clone,
    {
        let mut tiles = Vec::with_capacity((width as usize) * (height as usize));
        for cy in 0..height as i16 {
            for cx in 0..width as i16 {
                let src = Point::new(x + cx, y + cy);
                tiles.push(self.get(src).cloned().unwrap_or_else(|| fill.clone()));
            }
        }
        TileGrid {
            size: Size::new(width, height),
            tiles,
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
    #[deprecated(since = "0.1.0", note = "Use field_of_view instead")]
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

    /// Find the shortest cardinal path between two in-bounds points, with a maximum path step limit.
    ///
    /// If the path length (in steps) exceeds `max_cost`, the pathfinder will terminate early.
    /// Returns None if no path exists within `max_cost` steps.
    pub fn shortest_path4_limit<F>(
        &self,
        start: Point,
        goal: Point,
        max_cost: u32,
        passable: F,
    ) -> Option<Vec<Point>>
    where
        F: Fn(Point, &T) -> bool,
    {
        if !self.in_bounds(start) || !self.in_bounds(goal) {
            return None;
        }
        if start == goal {
            return Some(vec![start]);
        }
        if max_cost == 0 {
            return None;
        }

        let mut frontier = VecDeque::new();
        let mut came_from = HashMap::new();
        let mut distance = HashMap::new();

        frontier.push_back(start);
        came_from.insert(start, start);
        distance.insert(start, 0u32);

        while let Some(current) = frontier.pop_front() {
            let current_dist = *distance.get(&current).unwrap();
            if current_dist >= max_cost {
                continue;
            }

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
                distance.insert(neighbor, current_dist + 1);

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

    /// Find the shortest cardinal path from `start` to the nearest tile matching `predicate`.
    ///
    /// The returned path starts at `start` and ends at the matching point.
    /// `passable` is consulted for neighbor tiles; `start` is allowed even if not passable.
    pub fn shortest_path_to_predicate4<P, F>(
        &self,
        start: Point,
        mut predicate: P,
        passable: F,
    ) -> Option<Vec<Point>>
    where
        P: FnMut(Point, &T) -> bool,
        F: Fn(Point, &T) -> bool,
    {
        if !self.in_bounds(start) {
            return None;
        }
        if let Some(tile) = self.get(start) {
            if predicate(start, tile) {
                return Some(vec![start]);
            }
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
                if predicate(neighbor, tile) {
                    let mut path = vec![neighbor];
                    let mut step = neighbor;
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

    /// Find the shortest 8-directional path from `start` to the nearest tile matching `predicate`.
    ///
    /// The returned path starts at `start` and ends at the matching point.
    /// `passable` is consulted for neighbor tiles; `start` is allowed even if not passable.
    pub fn shortest_path_to_predicate8<P, F>(
        &self,
        start: Point,
        mut predicate: P,
        passable: F,
    ) -> Option<Vec<Point>>
    where
        P: FnMut(Point, &T) -> bool,
        F: Fn(Point, &T) -> bool,
    {
        if !self.in_bounds(start) {
            return None;
        }
        if let Some(tile) = self.get(start) {
            if predicate(start, tile) {
                return Some(vec![start]);
            }
        }

        let mut frontier = VecDeque::new();
        let mut came_from = HashMap::new();
        frontier.push_back(start);
        came_from.insert(start, start);

        while let Some(current) = frontier.pop_front() {
            for neighbor in current.neighbors8() {
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
                if predicate(neighbor, tile) {
                    let mut path = vec![neighbor];
                    let mut step = neighbor;
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

    /// Find the shortest cardinal path from `start` to the nearest tile matching `predicate` with custom costs.
    ///
    /// The returned path starts at `start` and ends at the matching point.
    /// `passable` is consulted for neighbor tiles; `start` is allowed even if not passable.
    /// `cost` is a function taking `from`, `to`, and the tile at `to`, returning the step cost.
    pub fn shortest_path_to_predicate4_weighted<P, F, C>(
        &self,
        start: Point,
        mut predicate: P,
        passable: F,
        cost: C,
    ) -> Option<Vec<Point>>
    where
        P: FnMut(Point, &T) -> bool,
        F: Fn(Point, &T) -> bool,
        C: Fn(Point, Point, &T) -> u32,
    {
        if !self.in_bounds(start) {
            return None;
        }
        if let Some(tile) = self.get(start) {
            if predicate(start, tile) {
                return Some(vec![start]);
            }
        }

        let mut g_score = HashMap::new();
        let mut came_from = HashMap::new();
        let mut frontier = std::collections::BinaryHeap::new();

        g_score.insert(start, 0u32);
        frontier.push(std::cmp::Reverse((0u32, start)));

        while let Some(std::cmp::Reverse((current_cost, current))) = frontier.pop() {
            if let Some(tile) = self.get(current) {
                if current != start && predicate(current, tile) {
                    let mut path = vec![current];
                    let mut step = current;
                    while step != start {
                        step = came_from[&step];
                        path.push(step);
                    }
                    path.reverse();
                    return Some(path);
                }
            }

            if current_cost > *g_score.get(&current).unwrap_or(&u32::MAX) {
                continue;
            }

            for neighbor in current.neighbors4() {
                let Some(tile) = self.get(neighbor) else {
                    continue;
                };
                if !passable(neighbor, tile) {
                    continue;
                }

                let step_cost = cost(current, neighbor, tile);
                let tentative_g = current_cost + step_cost;

                if tentative_g < *g_score.get(&neighbor).unwrap_or(&u32::MAX) {
                    came_from.insert(neighbor, current);
                    g_score.insert(neighbor, tentative_g);
                    frontier.push(std::cmp::Reverse((tentative_g, neighbor)));
                }
            }
        }

        None
    }

    /// Find the shortest 8-directional path from `start` to the nearest tile matching `predicate` with custom costs.
    ///
    /// The returned path starts at `start` and ends at the matching point.
    /// `passable` is consulted for neighbor tiles; `start` is allowed even if not passable.
    /// `cost` is a function taking `from`, `to`, and the tile at `to`, returning the step cost.
    pub fn shortest_path_to_predicate8_weighted<P, F, C>(
        &self,
        start: Point,
        mut predicate: P,
        passable: F,
        cost: C,
    ) -> Option<Vec<Point>>
    where
        P: FnMut(Point, &T) -> bool,
        F: Fn(Point, &T) -> bool,
        C: Fn(Point, Point, &T) -> u32,
    {
        if !self.in_bounds(start) {
            return None;
        }
        if let Some(tile) = self.get(start) {
            if predicate(start, tile) {
                return Some(vec![start]);
            }
        }

        let mut g_score = HashMap::new();
        let mut came_from = HashMap::new();
        let mut frontier = std::collections::BinaryHeap::new();

        g_score.insert(start, 0u32);
        frontier.push(std::cmp::Reverse((0u32, start)));

        while let Some(std::cmp::Reverse((current_cost, current))) = frontier.pop() {
            if let Some(tile) = self.get(current) {
                if current != start && predicate(current, tile) {
                    let mut path = vec![current];
                    let mut step = current;
                    while step != start {
                        step = came_from[&step];
                        path.push(step);
                    }
                    path.reverse();
                    return Some(path);
                }
            }

            if current_cost > *g_score.get(&current).unwrap_or(&u32::MAX) {
                continue;
            }

            for neighbor in current.neighbors8() {
                let Some(tile) = self.get(neighbor) else {
                    continue;
                };
                if !passable(neighbor, tile) {
                    continue;
                }

                let step_cost = cost(current, neighbor, tile);
                let tentative_g = current_cost + step_cost;

                if tentative_g < *g_score.get(&neighbor).unwrap_or(&u32::MAX) {
                    came_from.insert(neighbor, current);
                    g_score.insert(neighbor, tentative_g);
                    frontier.push(std::cmp::Reverse((tentative_g, neighbor)));
                }
            }
        }

        None
    }

    /// Find the shortest cardinal path between two in-bounds points with custom costs.
    ///
    /// The returned path includes `start` and `goal`. `passable` is consulted
    /// for neighbor tiles; `start` is allowed even if its tile is not passable.
    /// `cost` is a function taking `from`, `to`, and the tile at `to`, returning
    /// the step cost.
    pub fn shortest_path4_weighted<F, C>(
        &self,
        start: Point,
        goal: Point,
        passable: F,
        cost: C,
    ) -> Option<Vec<Point>>
    where
        F: Fn(Point, &T) -> bool,
        C: Fn(Point, Point, &T) -> u32,
    {
        if !self.in_bounds(start) || !self.in_bounds(goal) {
            return None;
        }
        if start == goal {
            return Some(vec![start]);
        }

        let mut g_score = HashMap::new();
        let mut came_from = HashMap::new();
        let mut frontier = std::collections::BinaryHeap::new();

        g_score.insert(start, 0u32);
        frontier.push(std::cmp::Reverse((0u32, start)));

        while let Some(std::cmp::Reverse((current_cost, current))) = frontier.pop() {
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

            if current_cost > *g_score.get(&current).unwrap_or(&u32::MAX) {
                continue;
            }

            for neighbor in current.neighbors4() {
                let Some(tile) = self.get(neighbor) else {
                    continue;
                };
                if !passable(neighbor, tile) {
                    continue;
                }

                let step_cost = cost(current, neighbor, tile);
                let tentative_g = current_cost + step_cost;

                if tentative_g < *g_score.get(&neighbor).unwrap_or(&u32::MAX) {
                    came_from.insert(neighbor, current);
                    g_score.insert(neighbor, tentative_g);
                    frontier.push(std::cmp::Reverse((tentative_g, neighbor)));
                }
            }
        }

        None
    }

    /// Find the shortest 8-directional path between two in-bounds points with custom costs.
    ///
    /// The returned path includes `start` and `goal`. `passable` is consulted
    /// for neighbor tiles; `start` is allowed even if its tile is not passable.
    /// `cost` is a function taking `from`, `to`, and the tile at `to`, returning
    /// the step cost.
    pub fn shortest_path8_weighted<F, C>(
        &self,
        start: Point,
        goal: Point,
        passable: F,
        cost: C,
    ) -> Option<Vec<Point>>
    where
        F: Fn(Point, &T) -> bool,
        C: Fn(Point, Point, &T) -> u32,
    {
        if !self.in_bounds(start) || !self.in_bounds(goal) {
            return None;
        }
        if start == goal {
            return Some(vec![start]);
        }

        let mut g_score = HashMap::new();
        let mut came_from = HashMap::new();
        let mut frontier = std::collections::BinaryHeap::new();

        g_score.insert(start, 0u32);
        frontier.push(std::cmp::Reverse((0u32, start)));

        while let Some(std::cmp::Reverse((current_cost, current))) = frontier.pop() {
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

            if current_cost > *g_score.get(&current).unwrap_or(&u32::MAX) {
                continue;
            }

            for direction in Direction8::ALL {
                let neighbor = current.step8(direction);
                let Some(tile) = self.get(neighbor) else {
                    continue;
                };
                if !passable(neighbor, tile) {
                    continue;
                }

                let step_cost = cost(current, neighbor, tile);
                let tentative_g = current_cost + step_cost;

                if tentative_g < *g_score.get(&neighbor).unwrap_or(&u32::MAX) {
                    came_from.insert(neighbor, current);
                    g_score.insert(neighbor, tentative_g);
                    frontier.push(std::cmp::Reverse((tentative_g, neighbor)));
                }
            }
        }

        None
    }

    /// Find the shortest cardinal path from `start` to the nearest target.
    ///
    /// Each returned path includes `start` and the chosen target. Ties keep the
    /// first shortest path found in target iteration order.
    /// A* pathfinder for 4-directional grids with custom cost and heuristic.
    pub fn astar4_ex<F, C, H>(
        &self,
        start: Point,
        goal: Point,
        passable: F,
        cost: C,
        heuristic: H,
    ) -> Option<Vec<Point>>
    where
        F: Fn(Point, &T) -> bool,
        C: Fn(Point, Point, &T) -> u32,
        H: Fn(Point, Point) -> u32,
    {
        if !self.in_bounds(start) || !self.in_bounds(goal) {
            return None;
        }
        if start == goal {
            return Some(vec![start]);
        }

        let mut g_score = HashMap::new();
        let mut came_from = HashMap::new();
        let mut frontier = std::collections::BinaryHeap::new();

        g_score.insert(start, 0u32);
        let h_score = heuristic(start, goal);
        frontier.push(std::cmp::Reverse((h_score, start)));

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

            let current_g = g_score[&current];

            for neighbor in current.neighbors4() {
                let Some(tile) = self.get(neighbor) else {
                    continue;
                };
                if neighbor != goal && !passable(neighbor, tile) {
                    continue;
                }

                let step_cost = cost(current, neighbor, tile);
                let tentative_g = current_g + step_cost;

                let old_g = g_score.get(&neighbor).copied().unwrap_or(u32::MAX);
                if tentative_g < old_g {
                    g_score.insert(neighbor, tentative_g);
                    came_from.insert(neighbor, current);
                    let f_score = tentative_g + heuristic(neighbor, goal);
                    frontier.push(std::cmp::Reverse((f_score, neighbor)));
                }
            }
        }

        None
    }

    /// A* pathfinder for 4-directional grids using standard Manhattan distance heuristic.
    pub fn astar4<F>(&self, start: Point, goal: Point, passable: F) -> Option<Vec<Point>>
    where
        F: Fn(Point, &T) -> bool,
    {
        self.astar4_ex(
            start,
            goal,
            passable,
            |_, _, _| 10,
            |p1, p2| (p1.manhattan_distance(p2) as u32) * 10,
        )
    }

    /// A* pathfinder for 8-directional grids with custom cost and heuristic.
    pub fn astar8_ex<F, C, H>(
        &self,
        start: Point,
        goal: Point,
        passable: F,
        cost: C,
        heuristic: H,
    ) -> Option<Vec<Point>>
    where
        F: Fn(Point, &T) -> bool,
        C: Fn(Point, Point, &T) -> u32,
        H: Fn(Point, Point) -> u32,
    {
        if !self.in_bounds(start) || !self.in_bounds(goal) {
            return None;
        }
        if start == goal {
            return Some(vec![start]);
        }

        let mut g_score = HashMap::new();
        let mut came_from = HashMap::new();
        let mut frontier = std::collections::BinaryHeap::new();

        g_score.insert(start, 0u32);
        let h_score = heuristic(start, goal);
        frontier.push(std::cmp::Reverse((h_score, start)));

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

            let current_g = g_score[&current];

            for neighbor in current.neighbors8() {
                let Some(tile) = self.get(neighbor) else {
                    continue;
                };
                if neighbor != goal && !passable(neighbor, tile) {
                    continue;
                }

                let step_cost = cost(current, neighbor, tile);
                let tentative_g = current_g + step_cost;

                let old_g = g_score.get(&neighbor).copied().unwrap_or(u32::MAX);
                if tentative_g < old_g {
                    g_score.insert(neighbor, tentative_g);
                    came_from.insert(neighbor, current);
                    let f_score = tentative_g + heuristic(neighbor, goal);
                    frontier.push(std::cmp::Reverse((f_score, neighbor)));
                }
            }
        }

        None
    }

    /// A* pathfinder for 8-directional grids using standard Chebyshev/Octile distance heuristic.
    pub fn astar8<F>(&self, start: Point, goal: Point, passable: F) -> Option<Vec<Point>>
    where
        F: Fn(Point, &T) -> bool,
    {
        self.astar8_ex(
            start,
            goal,
            passable,
            |p1, p2, _| {
                let dx = (p1.x - p2.x).abs();
                let dy = (p1.y - p2.y).abs();
                if dx > 0 && dy > 0 {
                    14 // Diagonal cost
                } else {
                    10 // Cardinal cost
                }
            },
            |p1, p2| {
                let dx = (p1.x - p2.x).unsigned_abs() as u32;
                let dy = (p1.y - p2.y).unsigned_abs() as u32;
                let min = dx.min(dy);
                let max = dx.max(dy);
                min * 14 + (max - min) * 10
            },
        )
    }

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

    /// Return every point reachable from `start` by cardinal movement within
    /// `max_steps`.
    ///
    /// `start` is included when it is in bounds. Points are returned in BFS
    /// order (nearest first). Useful for movement range indicators or limited
    /// visibility without scanning the entire map.
    pub fn reachable_points4_bounded<F>(
        &self,
        start: Point,
        max_steps: u16,
        passable: F,
    ) -> Vec<Point>
    where
        F: Fn(Point, &T) -> bool,
    {
        if !self.in_bounds(start) {
            return Vec::new();
        }

        let mut frontier = VecDeque::new();
        let mut seen = HashMap::new();
        let mut out = Vec::new();

        frontier.push_back((start, 0u16));
        seen.insert(start, ());

        while let Some((current, dist)) = frontier.pop_front() {
            out.push(current);
            if dist >= max_steps {
                continue;
            }
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
                frontier.push_back((neighbor, dist + 1));
            }
        }

        out
    }

    /// Return every point reachable from `start` by 8-directional movement
    /// within `max_steps`.
    ///
    /// `start` is included when it is in bounds. Points are returned in BFS
    /// order (nearest first). Uses Chebyshev distance for step counting.
    pub fn reachable_points8_bounded<F>(
        &self,
        start: Point,
        max_steps: u16,
        passable: F,
    ) -> Vec<Point>
    where
        F: Fn(Point, &T) -> bool,
    {
        if !self.in_bounds(start) {
            return Vec::new();
        }

        let mut frontier = VecDeque::new();
        let mut seen = HashMap::new();
        let mut out = Vec::new();

        frontier.push_back((start, 0u16));
        seen.insert(start, ());

        while let Some((current, dist)) = frontier.pop_front() {
            out.push(current);
            if dist >= max_steps {
                continue;
            }
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
                frontier.push_back((neighbor, dist + 1));
            }
        }

        out
    }

    /// Return the 8-directional distance from `start` to the nearest reachable target.
    ///
    /// Uses BFS with 8-directional neighbors, so the distance reflects the
    /// minimum number of steps when diagonal movement is allowed. Returns
    /// `None` if no target is reachable or `start` is out of bounds.
    pub fn distance_to_nearest8<I, F>(&self, start: Point, targets: I, passable: F) -> Option<u16>
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

    /// Flood-fill from `start`, returning every connected point matching the
    /// predicate. Uses BFS with 8-way connectivity (cardinal + diagonal),
    /// so the result is ordered by Chebyshev distance from `start`.
    pub fn flood_fill8<F>(&self, start: Point, matches: F) -> Vec<Point>
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
            for neighbor in current.neighbors8() {
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

        let mut rng = XorShift64::new(seed);

        let mut pos = start;
        self.set(pos, floor.clone());

        for _ in 0..steps {
            let dir_idx = (rng.next_u64() as usize) % 4;
            let next = pos.step(Direction::ALL[dir_idx]);
            if self.in_bounds(next) {
                pos = next;
                self.set(pos, floor.clone());
            }
        }
    }

    /// Generate a maze using the randomized depth-first search (recursive backtracker) algorithm.
    ///
    /// The entire grid is first filled with the `wall_tile`, then paths are carved.
    /// Note that for the maze pathways to carve cleanly, the grid width and height should
    /// ideally be odd numbers.
    ///
    /// The `seed` controls reproducibility.
    pub fn generate_maze(&mut self, wall_tile: T, path_tile: T, seed: u64)
    where
        T: Clone,
    {
        // Fill the grid with wall_tile
        self.fill(wall_tile.clone());

        if self.width() < 3 || self.height() < 3 {
            return;
        }

        let mut rng = XorShift64::new(seed);

        let mut visited = std::collections::HashSet::new();
        let mut stack = Vec::new();

        let start = Point::new(1, 1);
        self.set(start, path_tile.clone());
        visited.insert(start);
        stack.push(start);

        while let Some(current) = stack.last().copied() {
            let mut neighbors = Vec::new();

            // Check neighbors 2 steps away
            let candidates = [
                (0, -2), // North
                (0, 2),  // South
                (2, 0),  // East
                (-2, 0), // West
            ];

            for &(dx, dy) in &candidates {
                let nx = current.x + dx;
                let ny = current.y + dy;
                if nx > 0 && nx < self.width() as i16 - 1 && ny > 0 && ny < self.height() as i16 - 1
                {
                    let p = Point::new(nx, ny);
                    if !visited.contains(&p) {
                        neighbors.push((p, dx, dy));
                    }
                }
            }

            if !neighbors.is_empty() {
                // Pick a random unvisited neighbor
                let idx = (rng.next_u64() as usize) % neighbors.len();
                let (next_point, dx, dy) = neighbors[idx];

                // Carve the wall between current and next_point
                let mid_x = current.x + dx / 2;
                let mid_y = current.y + dy / 2;
                let mid_point = Point::new(mid_x, mid_y);

                self.set(mid_point, path_tile.clone());
                self.set(next_point, path_tile.clone());

                visited.insert(next_point);
                stack.push(next_point);
            } else {
                stack.pop();
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

        let mut xorshift = XorShift64::new(seed);
        let mut rng = || xorshift.next_u64();

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
                rng().is_multiple_of(2)
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
            let mid = if rng().is_multiple_of(2) {
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

    /// Generate a cave using cellular automata.
    ///
    /// Fills the grid randomly based on `fill_chance` (0.0–1.0), then runs
    /// `iterations` smoothing passes. Each pass applies the standard cave rule:
    /// a cell becomes `floor` if it has ≥ `birth_limit` wall neighbors,
    /// otherwise it becomes `wall`. Border cells are always kept as `wall`.
    ///
    /// The `seed` controls reproducibility. Returns the count of floor tiles
    /// carved.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut grid = TileGrid::new(40, 20, Tile::Wall);
    /// let floors = grid.cellular_automata_cave(Tile::Wall, Tile::Floor, 0.45, 5, 4, 12345);
    /// ```
    pub fn cellular_automata_cave(
        &mut self,
        wall: T,
        floor: T,
        fill_chance: f64,
        iterations: usize,
        birth_limit: u8,
        seed: u64,
    ) -> usize
    where
        T: Clone + PartialEq,
    {
        let w = self.width();
        let h = self.height();
        if w < 3 || h < 3 {
            return 0;
        }

        let mut xorshift = XorShift64::new(seed);
        let mut rng = || xorshift.next_u64();

        // Initial random fill: borders stay as wall, interior uses fill_chance.
        for y in 0..h {
            for x in 0..w {
                let is_border = x == 0 || y == 0 || x == w - 1 || y == h - 1;
                let tile = if is_border || (rng() as f64 / u64::MAX as f64) < fill_chance {
                    wall.clone()
                } else {
                    floor.clone()
                };
                self.set(Point::new(x as i16, y as i16), tile);
            }
        }

        // Smoothing passes.
        let mut buffer = vec![wall.clone(); (w * h) as usize];
        for _ in 0..iterations {
            for y in 0..h {
                for x in 0..w {
                    let is_border = x == 0 || y == 0 || x == w - 1 || y == h - 1;
                    if is_border {
                        buffer[(y * w + x) as usize] = wall.clone();
                        continue;
                    }
                    let mut wall_count = 0u8;
                    for dy in -1i16..=1 {
                        for dx in -1i16..=1 {
                            if dx == 0 && dy == 0 {
                                continue;
                            }
                            let nx = x as i16 + dx;
                            let ny = y as i16 + dy;
                            if nx < 0
                                || ny < 0
                                || nx >= w as i16
                                || ny >= h as i16
                                || self.get(Point::new(nx, ny)).is_none_or(|t| *t == wall)
                            {
                                wall_count += 1;
                            }
                        }
                    }
                    buffer[(y * w + x) as usize] = if wall_count >= birth_limit {
                        wall.clone()
                    } else {
                        floor.clone()
                    };
                }
            }
            for y in 0..h {
                for x in 0..w {
                    self.set(
                        Point::new(x as i16, y as i16),
                        buffer[(y * w + x) as usize].clone(),
                    );
                }
            }
        }

        self.count_matching(|_, t| *t == floor)
    }

    /// Generate a series of rooms using Binary Space Partitioning (BSP).
    ///
    /// Recursively splits the grid into smaller rectangles until `max_depth` or
    /// `min_size` is reached. Then places a room inside each leaf partition.
    /// Returns the centers of all generated rooms.
    pub fn bsp_rooms(
        &mut self,
        floor: T,
        min_room_size: u16,
        max_depth: usize,
        seed: u64,
    ) -> Vec<Point>
    where
        T: Clone + PartialEq,
    {
        let w = self.width();
        let h = self.height();
        let mut partitions = vec![Rect::new(0, 0, w, h)];
        let mut leaves = Vec::new();

        let mut xorshift = XorShift64::new(seed);
        let mut rng = || xorshift.next_u64();

        for _ in 0..max_depth {
            let mut next_gen = Vec::new();
            let mut split_any = false;

            for p in partitions {
                if p.width > min_room_size * 2 + 2 && p.height > min_room_size * 2 + 2 {
                    // Split
                    let split_horizontal = (rng() % 2) == 0;
                    if split_horizontal {
                        let split_y = (rng() % (p.height as u64 - min_room_size as u64 * 2)) as u16
                            + min_room_size
                            + 1;
                        next_gen.push(Rect::new(p.x, p.y, p.width, split_y));
                        next_gen.push(Rect::new(
                            p.x,
                            p.y + split_y as i16,
                            p.width,
                            p.height - split_y,
                        ));
                    } else {
                        let split_x = (rng() % (p.width as u64 - min_room_size as u64 * 2)) as u16
                            + min_room_size
                            + 1;
                        next_gen.push(Rect::new(p.x, p.y, split_x, p.height));
                        next_gen.push(Rect::new(
                            p.x + split_x as i16,
                            p.y,
                            p.width - split_x,
                            p.height,
                        ));
                    }
                    split_any = true;
                } else {
                    leaves.push(p);
                }
            }

            partitions = next_gen;
            if !split_any {
                break;
            }
        }
        leaves.extend(partitions);

        let mut centers = Vec::new();
        for p in leaves {
            if p.width < min_room_size + 2 || p.height < min_room_size + 2 {
                continue;
            }

            // Room size inside partition
            let rw = (rng() % (p.width as u64 - min_room_size as u64)) as u16 + min_room_size;
            let rh = (rng() % (p.height as u64 - min_room_size as u64)) as u16 + min_room_size;

            // Centered in partition
            let rx = p.x + ((p.width - rw) / 2) as i16;
            let ry = p.y + ((p.height - rh) / 2) as i16;

            for dy in 0..rh {
                for dx in 0..rw {
                    self.set(Point::new(rx + dx as i16, ry + dy as i16), floor.clone());
                }
            }
            centers.push(Point::new(rx + (rw / 2) as i16, ry + (rh / 2) as i16));
        }

        // Connect centers with simple L-corridors
        for i in 0..centers.len().saturating_sub(1) {
            let start = centers[i];
            let end = centers[i + 1];

            // Horizontal then vertical
            let mut curr = start;
            while curr.x != end.x {
                self.set(curr, floor.clone());
                curr.x += if end.x > curr.x { 1 } else { -1 };
            }
            while curr.y != end.y {
                self.set(curr, floor.clone());
                curr.y += if end.y > curr.y { 1 } else { -1 };
            }
        }

        centers
    }

    /// Generate a Dijkstra map starting from the given goals.
    ///
    /// The resulting map contains the distance from each reachable tile to the
    /// nearest goal. Useful for AI pathfinding, scent trails, or influence maps.
    pub fn dijkstra_map<F>(&self, goals: &[Point], mut is_walkable: F) -> HashMap<Point, u32>
    where
        F: FnMut(Point, &T) -> bool,
    {
        let mut d_map = HashMap::new();
        let mut queue = VecDeque::new();

        for &goal in goals {
            d_map.insert(goal, 0);
            queue.push_back(goal);
        }

        let w = self.width() as i16;
        let h = self.height() as i16;

        while let Some(p) = queue.pop_front() {
            let dist = *d_map.get(&p).unwrap();
            for neighbor in p.neighbors4() {
                if neighbor.x >= 0
                    && neighbor.y >= 0
                    && neighbor.x < w
                    && neighbor.y < h
                    && !d_map.contains_key(&neighbor)
                    && is_walkable(neighbor, self.get(neighbor).unwrap())
                {
                    d_map.insert(neighbor, dist + 1);
                    queue.push_back(neighbor);
                }
            }
        }
        d_map
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

    /// Find the first point matching the predicate, scanning row-major.
    pub fn find_matching<F>(&self, predicate: F) -> Option<Point>
    where
        F: Fn(Point, &T) -> bool,
    {
        self.iter().find_map(|(p, t)| predicate(p, t).then_some(p))
    }

    /// Collect all points matching the predicate in row-major order.
    pub fn points_matching<F>(&self, predicate: F) -> Vec<Point>
    where
        F: Fn(Point, &T) -> bool,
    {
        self.iter()
            .filter_map(|(p, t)| predicate(p, t).then_some(p))
            .collect()
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

    /// Returns integer points on the straight line from `start` to `end`,
    /// including both endpoints.
    pub fn raycast(&self, start: Point, end: Point) -> Vec<Point> {
        line_between(start, end)
    }

    /// Casts a ray from `start` to `end` until it hits an opaque tile or goes out of bounds.
    ///
    /// The starting point is always included and is not checked for opacity.
    /// Returns a tuple containing the path of points traversed (including the first
    /// blocked point, if any) and a boolean indicating whether the ray was blocked.
    pub fn raycast_opaque<F>(
        &self,
        start: Point,
        end: Point,
        mut is_opaque: F,
    ) -> (Vec<Point>, bool)
    where
        F: FnMut(Point, &T) -> bool,
    {
        let mut path = Vec::new();
        let mut blocked = false;
        for p in LineIter::new(start, end) {
            path.push(p);
            if p == start {
                continue;
            }
            if let Some(tile) = self.get(p) {
                if is_opaque(p, tile) {
                    blocked = true;
                    break;
                }
            } else {
                blocked = true;
                break;
            }
        }
        (path, blocked)
    }

    /// Casts a ray from `start` to `end` up to a maximum range, or until it hits an opaque tile/out of bounds.
    ///
    /// The starting point is always included and is not checked for opacity.
    /// The range check evaluates Chebyshev distance from the start point.
    /// Returns a tuple containing the path of points traversed and a boolean indicating whether the ray was blocked.
    pub fn raycast_opaque_range<F>(
        &self,
        start: Point,
        end: Point,
        max_range: u16,
        mut is_opaque: F,
    ) -> (Vec<Point>, bool)
    where
        F: FnMut(Point, &T) -> bool,
    {
        let mut path = Vec::new();
        let mut blocked = false;
        for p in LineIter::new(start, end) {
            if start.chebyshev_distance(p) > max_range {
                break;
            }
            path.push(p);
            if p == start {
                continue;
            }
            if let Some(tile) = self.get(p) {
                if is_opaque(p, tile) {
                    blocked = true;
                    break;
                }
            } else {
                blocked = true;
                break;
            }
        }
        (path, blocked)
    }

    /// Flood-fills from a starting point, returning all connected passable points.
    ///
    /// Performs a breadth-first search using 4-way cardinal connectivity.
    pub fn flood_fill<F>(
        &self,
        start: Point,
        mut is_passable: F,
    ) -> std::collections::HashSet<Point>
    where
        F: FnMut(Point, &T) -> bool,
    {
        let mut visited = std::collections::HashSet::new();
        if !self.in_bounds(start) {
            return visited;
        }
        if let Some(tile) = self.get(start) {
            if !is_passable(start, tile) {
                return visited;
            }
        }

        let mut queue = std::collections::VecDeque::new();
        queue.push_back(start);
        visited.insert(start);

        while let Some(current) = queue.pop_front() {
            for neighbor in current.neighbors4() {
                if !visited.contains(&neighbor) {
                    if let Some(tile) = self.get(neighbor) {
                        if is_passable(neighbor, tile) {
                            visited.insert(neighbor);
                            queue.push_back(neighbor);
                        }
                    }
                }
            }
        }
        visited
    }

    /// Find the shortest path through a sequence of waypoints using cardinal movement.
    ///
    /// The input waypoints must have at least 2 points (representing start and goal).
    /// Returns the consolidated path visiting all waypoints in order, or None if any segment is unreachable.
    pub fn shortest_path_via_waypoints4<F>(
        &self,
        waypoints: &[Point],
        passable: F,
    ) -> Option<Vec<Point>>
    where
        F: Fn(Point, &T) -> bool,
    {
        if waypoints.len() < 2 {
            return None;
        }
        let mut full_path = Vec::new();
        for i in 0..waypoints.len() - 1 {
            let start = waypoints[i];
            let goal = waypoints[i + 1];
            let segment = self.shortest_path4(start, goal, &passable)?;
            if i > 0 {
                full_path.extend(segment.into_iter().skip(1));
            } else {
                full_path.extend(segment);
            }
        }
        Some(full_path)
    }

    /// Find the shortest path through a sequence of waypoints using 8-directional movement.
    ///
    /// The input waypoints must have at least 2 points (representing start and goal).
    /// Returns the consolidated path visiting all waypoints in order, or None if any segment is unreachable.
    pub fn shortest_path_via_waypoints8<F>(
        &self,
        waypoints: &[Point],
        passable: F,
    ) -> Option<Vec<Point>>
    where
        F: Fn(Point, &T) -> bool,
    {
        if waypoints.len() < 2 {
            return None;
        }
        let mut full_path = Vec::new();
        for i in 0..waypoints.len() - 1 {
            let start = waypoints[i];
            let goal = waypoints[i + 1];
            let segment = self.shortest_path8(start, goal, &passable)?;
            if i > 0 {
                full_path.extend(segment.into_iter().skip(1));
            } else {
                full_path.extend(segment);
            }
        }
        Some(full_path)
    }

    /// Find the shortest path through a sequence of waypoints using weighted cardinal movement.
    pub fn shortest_path_via_waypoints4_weighted<F, C>(
        &self,
        waypoints: &[Point],
        passable: F,
        cost: C,
    ) -> Option<Vec<Point>>
    where
        F: Fn(Point, &T) -> bool,
        C: Fn(Point, Point, &T) -> u32,
    {
        if waypoints.len() < 2 {
            return None;
        }
        let mut full_path = Vec::new();
        for i in 0..waypoints.len() - 1 {
            let start = waypoints[i];
            let goal = waypoints[i + 1];
            let segment = self.shortest_path4_weighted(start, goal, &passable, &cost)?;
            if i > 0 {
                full_path.extend(segment.into_iter().skip(1));
            } else {
                full_path.extend(segment);
            }
        }
        Some(full_path)
    }

    /// Find the shortest path through a sequence of waypoints using weighted 8-directional movement.
    pub fn shortest_path_via_waypoints8_weighted<F, C>(
        &self,
        waypoints: &[Point],
        passable: F,
        cost: C,
    ) -> Option<Vec<Point>>
    where
        F: Fn(Point, &T) -> bool,
        C: Fn(Point, Point, &T) -> u32,
    {
        if waypoints.len() < 2 {
            return None;
        }
        let mut full_path = Vec::new();
        for i in 0..waypoints.len() - 1 {
            let start = waypoints[i];
            let goal = waypoints[i + 1];
            let segment = self.shortest_path8_weighted(start, goal, &passable, &cost)?;
            if i > 0 {
                full_path.extend(segment.into_iter().skip(1));
            } else {
                full_path.extend(segment);
            }
        }
        Some(full_path)
    }

    /// Get all points within a Chebyshev or Euclidean radius from a center point.
    ///
    /// If `use_euclidean` is true, a circle is returned; otherwise, a square/box (Chebyshev).
    pub fn points_in_circle(&self, center: Point, radius: u16, use_euclidean: bool) -> Vec<Point> {
        let mut points = Vec::new();
        let r = radius as i16;
        let start_x = (center.x - r).max(0);
        let end_x = (center.x + r).min(self.width() as i16 - 1);
        let start_y = (center.y - r).max(0);
        let end_y = (center.y + r).min(self.height() as i16 - 1);

        for y in start_y..=end_y {
            for x in start_x..=end_x {
                let p = Point::new(x, y);
                if use_euclidean {
                    if center.euclidean_distance(p) <= radius as f32 {
                        points.push(p);
                    }
                } else {
                    points.push(p);
                }
            }
        }
        points
    }

    /// Get all points in an AoE ring between `min_radius` and `max_radius` (inclusive).
    pub fn points_in_ring(
        &self,
        center: Point,
        min_radius: u16,
        max_radius: u16,
        use_euclidean: bool,
    ) -> Vec<Point> {
        let mut points = Vec::new();
        let r = max_radius as i16;
        let start_x = (center.x - r).max(0);
        let end_x = (center.x + r).min(self.width() as i16 - 1);
        let start_y = (center.y - r).max(0);
        let end_y = (center.y + r).min(self.height() as i16 - 1);

        for y in start_y..=end_y {
            for x in start_x..=end_x {
                let p = Point::new(x, y);
                if use_euclidean {
                    let d = center.euclidean_distance(p);
                    if d >= min_radius as f32 && d <= max_radius as f32 {
                        points.push(p);
                    }
                } else {
                    let d = center.chebyshev_distance(p);
                    if d >= min_radius && d <= max_radius {
                        points.push(p);
                    }
                }
            }
        }
        points
    }

    /// Get all points in a directional cone/wedge facing `direction`.
    ///
    /// `angle_degrees` specifies the full arc of the cone (e.g. 90 degrees).
    pub fn points_in_cone(
        &self,
        origin: Point,
        range: u16,
        direction: Direction8,
        angle_degrees: f32,
    ) -> Vec<Point> {
        let mut points = Vec::new();
        let r = range as i16;
        let start_x = (origin.x - r).max(0);
        let end_x = (origin.x + r).min(self.width() as i16 - 1);
        let start_y = (origin.y - r).max(0);
        let end_y = (origin.y + r).min(self.height() as i16 - 1);

        let (dx, dy) = direction.delta();
        let target_angle = (dy as f32).atan2(dx as f32); // in radians
        let half_arc = (angle_degrees / 2.0).to_radians();

        for y in start_y..=end_y {
            for x in start_x..=end_x {
                let p = Point::new(x, y);
                if p == origin {
                    points.push(p);
                    continue;
                }
                if origin.chebyshev_distance(p) <= range {
                    let px = (p.x - origin.x) as f32;
                    let py = (p.y - origin.y) as f32;
                    let angle = py.atan2(px);

                    // Normalize difference to [-PI, PI]
                    let mut diff = angle - target_angle;
                    while diff > std::f32::consts::PI {
                        diff -= 2.0 * std::f32::consts::PI;
                    }
                    while diff < -std::f32::consts::PI {
                        diff += 2.0 * std::f32::consts::PI;
                    }

                    if diff.abs() <= half_arc {
                        points.push(p);
                    }
                }
            }
        }
        points
    }
}

impl<T> Index<Point> for TileGrid<T> {
    type Output = T;

    fn index(&self, point: Point) -> &Self::Output {
        let idx = point.y as usize * self.width() as usize + point.x as usize;
        &self.tiles[idx]
    }
}

impl<T> IndexMut<Point> for TileGrid<T> {
    fn index_mut(&mut self, point: Point) -> &mut Self::Output {
        let idx = point.y as usize * self.width() as usize + point.x as usize;
        &mut self.tiles[idx]
    }
}

#[allow(clippy::too_many_arguments)]
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
