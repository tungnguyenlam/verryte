use crate::Point;

/// A Dijkstra Map (also known as a distance field) for pathfinding, chasing, and fleeing.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DijkstraMap {
    pub width: u16,
    pub height: u16,
    pub distances: Vec<u32>,
}

impl DijkstraMap {
    pub const UNREACHABLE: u32 = u32::MAX;

    /// Create a new empty Dijkstra map with the given dimensions.
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            distances: vec![Self::UNREACHABLE; (width as usize) * (height as usize)],
        }
    }

    /// Compute distances from sources.
    ///
    /// `sources` are the targets/goals to chase (distance 0).
    /// `passable` determines if a coordinate is walkable.
    /// `diagonal` allows 8-way traversal if true, otherwise 4-way.
    pub fn compute<F>(
        width: u16,
        height: u16,
        sources: &[Point],
        mut passable: F,
        diagonal: bool,
    ) -> Self
    where
        F: FnMut(Point) -> bool,
    {
        let mut map = Self::new(width, height);
        let mut queue = std::collections::VecDeque::new();

        for &source in sources {
            if source.x >= 0 && source.x < width as i16 && source.y >= 0 && source.y < height as i16
            {
                let idx = (source.y as usize) * (width as usize) + (source.x as usize);
                map.distances[idx] = 0;
                queue.push_back((source, 0));
            }
        }

        while let Some((point, dist)) = queue.pop_front() {
            let next_dist = dist + 1;

            if diagonal {
                for neighbor in point.neighbors8() {
                    if neighbor.x >= 0
                        && neighbor.x < width as i16
                        && neighbor.y >= 0
                        && neighbor.y < height as i16
                        && passable(neighbor)
                    {
                        let idx = (neighbor.y as usize) * (width as usize) + (neighbor.x as usize);
                        if map.distances[idx] == Self::UNREACHABLE {
                            map.distances[idx] = next_dist;
                            queue.push_back((neighbor, next_dist));
                        }
                    }
                }
            } else {
                for neighbor in point.neighbors4() {
                    if neighbor.x >= 0
                        && neighbor.x < width as i16
                        && neighbor.y >= 0
                        && neighbor.y < height as i16
                        && passable(neighbor)
                    {
                        let idx = (neighbor.y as usize) * (width as usize) + (neighbor.x as usize);
                        if map.distances[idx] == Self::UNREACHABLE {
                            map.distances[idx] = next_dist;
                            queue.push_back((neighbor, next_dist));
                        }
                    }
                }
            }
        }

        map
    }

    /// Get distance at a point. Returns None if out of bounds or unreachable.
    pub fn get(&self, point: Point) -> Option<u32> {
        if point.x >= 0
            && point.x < self.width as i16
            && point.y >= 0
            && point.y < self.height as i16
        {
            let d = self.distances[(point.y as usize) * (self.width as usize) + (point.x as usize)];
            if d == Self::UNREACHABLE {
                None
            } else {
                Some(d)
            }
        } else {
            None
        }
    }

    /// Returns the neighbor point that has the lowest distance (toward sources).
    ///
    /// Returns None if all neighbors are unreachable or out of bounds.
    pub fn chase_direction(&self, from: Point, diagonal: bool) -> Option<Point> {
        let neighbors = if diagonal {
            from.neighbors8().to_vec()
        } else {
            from.neighbors4().to_vec()
        };

        let mut best_point = None;
        let mut best_dist = u32::MAX;

        for n in neighbors {
            if let Some(dist) = self.get(n) {
                if dist < best_dist {
                    best_dist = dist;
                    best_point = Some(n);
                }
            }
        }

        best_point
    }

    /// Returns the neighbor point that has the highest distance (away from sources).
    ///
    /// Returns None if all neighbors are unreachable or out of bounds.
    pub fn flee_direction(&self, from: Point, diagonal: bool) -> Option<Point> {
        let neighbors = if diagonal {
            from.neighbors8().to_vec()
        } else {
            from.neighbors4().to_vec()
        };

        let mut best_point = None;
        let mut best_dist = 0;
        let mut found_any = false;

        for n in neighbors {
            if let Some(dist) = self.get(n) {
                if dist > best_dist || (!found_any && dist >= best_dist) {
                    best_dist = dist;
                    best_point = Some(n);
                    found_any = true;
                }
            }
        }

        best_point
    }

    /// Returns a path from the starting point to the nearest source.
    ///
    /// The path includes the starting point and the target source.
    /// Returns an empty vector if the starting point is unreachable or already at a source.
    pub fn path_to(&self, from: Point, diagonal: bool) -> Vec<Point> {
        let mut path = Vec::new();
        let mut current = from;

        if self.get(current).is_none() {
            return path;
        }

        path.push(current);

        while let Some(dist) = self.get(current) {
            if dist == 0 {
                break;
            }

            if let Some(next) = self.chase_direction(current, diagonal) {
                if next == current {
                    break;
                }
                path.push(next);
                current = next;
            } else {
                break;
            }

            // Safety break for cycles (though Dijkstra shouldn't have them)
            if path.len() > (self.width as usize) * (self.height as usize) {
                break;
            }
        }

        path
    }

    /// Find all points in the Dijkstra map that have a distance <= max_range.
    pub fn find_all_within_range(&self, max_range: u32) -> Vec<(Point, u32)> {
        let mut results = Vec::new();
        for y in 0..self.height {
            for x in 0..self.width {
                let p = Point::new(x as i16, y as i16);
                if let Some(dist) = self.get(p) {
                    if dist <= max_range {
                        results.push((p, dist));
                    }
                }
            }
        }
        results
    }

    /// Returns a path from `from` that stops when it enters the range `[min_range, max_range]` from the sources.
    ///
    /// Returns None if unreachable or if no path can enter the range.
    pub fn chase_path_to_range(
        &self,
        from: Point,
        min_range: u32,
        max_range: u32,
        diagonal: bool,
    ) -> Option<Vec<Point>> {
        let current_dist = self.get(from)?;
        if current_dist >= min_range && current_dist <= max_range {
            return Some(vec![from]);
        }

        let mut path = vec![from];
        let mut current = from;

        while let Some(dist) = self.get(current) {
            if dist >= min_range && dist <= max_range {
                return Some(path);
            }
            if dist < min_range {
                // Too close, flee to back off into range
                if let Some(next) = self.flee_direction(current, diagonal) {
                    if next == current || path.contains(&next) {
                        break;
                    }
                    path.push(next);
                    current = next;
                } else {
                    break;
                }
            } else {
                // Too far, chase to get closer
                if let Some(next) = self.chase_direction(current, diagonal) {
                    if next == current || path.contains(&next) {
                        break;
                    }
                    path.push(next);
                    current = next;
                } else {
                    break;
                }
            }

            if path.len() > (self.width as usize) * (self.height as usize) {
                break;
            }
        }

        None
    }

    /// Compute distances from sources using a custom weight function for edge costs.
    ///
    /// `sources` are the targets/goals to chase (distance 0).
    /// `passable` determines if a coordinate is walkable.
    /// `cost` returns the movement cost from one point to an adjacent point.
    /// `diagonal` allows 8-way traversal if true, otherwise 4-way.
    pub fn compute_weighted<F, C>(
        width: u16,
        height: u16,
        sources: &[Point],
        mut passable: F,
        mut cost: C,
        diagonal: bool,
    ) -> Self
    where
        F: FnMut(Point) -> bool,
        C: FnMut(Point, Point) -> u32,
    {
        #[derive(Copy, Clone, Eq, PartialEq)]
        struct Node {
            cost: u32,
            point: Point,
        }

        impl Ord for Node {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                other.cost.cmp(&self.cost)
            }
        }

        impl PartialOrd for Node {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        let mut map = Self::new(width, height);
        let mut heap = std::collections::BinaryHeap::new();

        for &source in sources {
            if source.x >= 0 && source.x < width as i16 && source.y >= 0 && source.y < height as i16
            {
                let idx = (source.y as usize) * (width as usize) + (source.x as usize);
                map.distances[idx] = 0;
                heap.push(Node { cost: 0, point: source });
            }
        }

        while let Some(Node { cost: current_cost, point }) = heap.pop() {
            let idx = (point.y as usize) * (width as usize) + (point.x as usize);
            if current_cost > map.distances[idx] {
                continue;
            }

            let mut process_neighbor = |neighbor: Point| {
                if neighbor.x >= 0
                    && neighbor.x < width as i16
                    && neighbor.y >= 0
                    && neighbor.y < height as i16
                    && passable(neighbor)
                {
                    let next_cost = current_cost + cost(point, neighbor);
                    let n_idx = (neighbor.y as usize) * (width as usize) + (neighbor.x as usize);
                    if next_cost < map.distances[n_idx] {
                        map.distances[n_idx] = next_cost;
                        heap.push(Node { cost: next_cost, point: neighbor });
                    }
                }
            };

            if diagonal {
                for neighbor in point.neighbors8() {
                    process_neighbor(neighbor);
                }
            } else {
                for neighbor in point.neighbors4() {
                    process_neighbor(neighbor);
                }
            }
        }

        map
    }

    /// Formats the Dijkstra map as an ASCII string.
    ///
    /// Distances from 0 to 9 are rendered as their digit character.
    /// Distances from 10 to 35 are rendered as lowercase letters 'a' through 'z'.
    /// Other reachable distances are rendered as '+'.
    /// Unreachable points are rendered as '.'.
    pub fn to_ascii_string(&self) -> String {
        let mut s = String::new();
        for y in 0..self.height {
            for x in 0..self.width {
                let p = Point::new(x as i16, y as i16);
                match self.get(p) {
                    None => s.push('.'),
                    Some(dist) => {
                        if dist <= 9 {
                            s.push_str(&dist.to_string());
                        } else if dist <= 35 {
                            let ch = (b'a' + (dist - 10) as u8) as char;
                            s.push(ch);
                        } else {
                            s.push('+');
                        }
                    }
                }
            }
            if y < self.height - 1 {
                s.push('\n');
            }
        }
        s
    }
}
