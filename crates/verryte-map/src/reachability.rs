use crate::{Point, TileGrid};
use std::collections::HashMap;

/// Calculates the exact set of tiles a unit can move to, given an action point limit
/// and variable movement costs.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ReachabilityMap {
    pub start: Point,
    pub max_cost: u32,
    /// Maps each reachable point to the cost to reach it and its predecessor (for path reconstruction).
    pub reachable: HashMap<Point, (u32, Point)>,
}

impl ReachabilityMap {
    /// Compute reachability from a start point on the grid.
    ///
    /// - `passable` is a closure returning true if a point can be walked through.
    /// - `cost` is a closure returning the movement cost to enter a neighbor.
    pub fn compute<T, F, C>(
        grid: &TileGrid<T>,
        start: Point,
        max_cost: u32,
        mut passable: F,
        mut cost: C,
    ) -> Self
    where
        F: FnMut(Point, &T) -> bool,
        C: FnMut(Point, &T) -> u32,
    {
        let mut reachable = HashMap::new();
        reachable.insert(start, (0, start));

        let mut queue = std::collections::BinaryHeap::new();
        // Heap items: (negative_cost, point) to pop lowest cost first
        queue.push((0i32, start));

        while let Some((neg_cost, current)) = queue.pop() {
            let current_cost = -neg_cost as u32;

            // If we found a cheaper way in the queue, skip
            if let Some(&(best_cost, _)) = reachable.get(&current) {
                if current_cost > best_cost {
                    continue;
                }
            }

            // Expand to cardinally adjacent neighbors
            for neighbor in current.neighbors4() {
                if !grid.in_bounds(neighbor) {
                    continue;
                }
                if let Some(tile) = grid.get(neighbor) {
                    if !passable(neighbor, tile) {
                        continue;
                    }
                    let step_cost = cost(neighbor, tile);
                    let new_cost = current_cost.saturating_add(step_cost);
                    if new_cost <= max_cost {
                        let is_cheaper = match reachable.get(&neighbor) {
                            Some(&(old_cost, _)) => new_cost < old_cost,
                            None => true,
                        };
                        if is_cheaper {
                            reachable.insert(neighbor, (new_cost, current));
                            queue.push((-(new_cost as i32), neighbor));
                        }
                    }
                }
            }
        }

        Self {
            start,
            max_cost,
            reachable,
        }
    }

    /// Check if a point is reachable.
    pub fn is_reachable(&self, point: Point) -> bool {
        self.reachable.contains_key(&point)
    }

    /// Get the cost to reach a point.
    pub fn cost_to(&self, point: Point) -> Option<u32> {
        self.reachable.get(&point).map(|&(c, _)| c)
    }

    /// Reconstruct the shortest path from the start point to the target point.
    /// Returns None if the target is not reachable.
    pub fn path_to(&self, target: Point) -> Option<Vec<Point>> {
        if !self.is_reachable(target) {
            return None;
        }
        let mut path = Vec::new();
        let mut current = target;
        while current != self.start {
            path.push(current);
            let &(_, pred) = self.reachable.get(&current).unwrap();
            current = pred;
        }
        path.push(self.start);
        path.reverse();
        Some(path)
    }
}
