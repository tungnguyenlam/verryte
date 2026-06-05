use crate::Point;

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
            dy: -dy,
            sx,
            sy,
            err: dx - dy,
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

/// Compress a full grid path of coordinates into a list of waypoints (start, end, and points where direction changes).
pub fn compress_path_to_waypoints(path: &[Point]) -> Vec<Point> {
    if path.len() <= 2 {
        return path.to_vec();
    }
    let mut compressed = vec![path[0]];
    for i in 1..path.len() - 1 {
        let prev = path[i - 1];
        let curr = path[i];
        let next = path[i + 1];

        let dx1 = curr.x - prev.x;
        let dy1 = curr.y - prev.y;
        let dx2 = next.x - curr.x;
        let dy2 = next.y - curr.y;

        // If the direction vector changes (cross product is non-zero, or sign/scale changes), curr is a waypoint
        if dx1 * dy2 != dx2 * dy1
            || (dx1.signum() != dx2.signum())
            || (dy1.signum() != dy2.signum())
        {
            compressed.push(curr);
        }
    }
    compressed.push(*path.last().unwrap());
    compressed
}

/// Convert a coordinate path into a list of 8-directional moves.
/// Returns an error if two consecutive points in the path are not adjacent.
pub fn path_to_directions(path: &[Point]) -> Result<Vec<crate::Direction8>, &'static str> {
    if path.len() < 2 {
        return Ok(Vec::new());
    }
    let mut directions = Vec::with_capacity(path.len() - 1);
    for window in path.windows(2) {
        let from = window[0];
        let to = window[1];
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        if dx.abs() > 1 || dy.abs() > 1 || (dx == 0 && dy == 0) {
            return Err("Path points must be adjacent and not equal");
        }
        let dir = crate::Direction8::from_offset(dx, dy).ok_or("Invalid step direction")?;
        directions.push(dir);
    }
    Ok(directions)
}
