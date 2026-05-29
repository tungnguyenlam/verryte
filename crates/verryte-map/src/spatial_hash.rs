use crate::Point;

use std::collections::HashMap;

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

    /// Query all entities within `radius` (Chebyshev distance) of `center`.
    pub fn query_chebyshev<'a>(
        &'a self,
        center: Point,
        radius: u16,
    ) -> impl Iterator<Item = &'a T> + 'a {
        let radius = radius as i16;
        let cell_radius = (radius / self.cell_size) + 1;
        let (cx, cy) = self.cell_key(center);

        let mut results = Vec::new();
        for dx in -cell_radius..=cell_radius {
            for dy in -cell_radius..=cell_radius {
                if let Some(entries) = self.cells.get(&(cx + dx, cy + dy)) {
                    for (point, value) in entries {
                        if point.chebyshev_distance(center) <= radius as u16 {
                            results.push(value);
                        }
                    }
                }
            }
        }
        results.into_iter()
    }

    /// Query all entities within `radius` (Euclidean distance) of `center`.
    pub fn query_euclidean<'a>(
        &'a self,
        center: Point,
        radius: f32,
    ) -> impl Iterator<Item = &'a T> + 'a {
        let cell_radius = (radius / self.cell_size as f32).ceil() as i16 + 1;
        let (cx, cy) = self.cell_key(center);

        let mut results = Vec::new();
        for dx in -cell_radius..=cell_radius {
            for dy in -cell_radius..=cell_radius {
                if let Some(entries) = self.cells.get(&(cx + dx, cy + dy)) {
                    for (point, value) in entries {
                        if point.euclidean_distance(center) <= radius {
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
