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
