use std::fmt;

use crate::{Point, TileGrid};

/// Visibility state of a tile in a [`VisibilityMap`].
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Visibility {
    #[default]
    Hidden, // Never seen
    Explored, // Seen in the past, but not currently visible
    Visible,  // Currently in line of sight
}

impl fmt::Display for Visibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Visibility::Hidden => write!(f, "Hidden"),
            Visibility::Explored => write!(f, "Explored"),
            Visibility::Visible => write!(f, "Visible"),
        }
    }
}

/// Tracks field-of-view and exploration state for a grid.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VisibilityMap {
    grid: TileGrid<Visibility>,
}

impl VisibilityMap {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            grid: TileGrid::new(width, height, Visibility::Hidden),
        }
    }

    /// Reset all 'Visible' tiles to 'Explored'.
    ///
    /// Call this before a new FOV calculation to ensure only currently
    /// visible tiles are marked as 'Visible'.
    pub fn clear_visible(&mut self) {
        for tile in self.grid.tiles_mut() {
            if *tile == Visibility::Visible {
                *tile = Visibility::Explored;
            }
        }
    }

    /// Mark a point as 'Visible'.
    pub fn set_visible(&mut self, point: Point) {
        self.grid.set(point, Visibility::Visible);
    }

    /// Get the visibility of a point. Returns 'Hidden' for out-of-bounds.
    pub fn get(&self, point: Point) -> Visibility {
        self.grid.get(point).copied().unwrap_or(Visibility::Hidden)
    }

    pub fn is_visible(&self, point: Point) -> bool {
        self.get(point) == Visibility::Visible
    }

    pub fn is_explored(&self, point: Point) -> bool {
        self.get(point) != Visibility::Hidden
    }

    pub fn width(&self) -> u16 {
        self.grid.width()
    }

    pub fn height(&self) -> u16 {
        self.grid.height()
    }

    /// Compute field-of-view from a given center point and radius.
    ///
    /// `is_opaque` determines if a tile blocks vision.
    pub fn compute_fov<F>(&mut self, center: Point, radius: u16, is_opaque: F)
    where
        F: FnMut(Point) -> bool,
    {
        self.clear_visible();
        self.compute_fov_incremental(center, radius, is_opaque);
    }

    /// Compute field-of-view from a given center point and radius, accumulating onto the
    /// currently visible tiles without clearing existing 'Visible' tiles.
    ///
    /// `is_opaque` determines if a tile blocks vision.
    pub fn compute_fov_incremental<F>(&mut self, center: Point, radius: u16, mut is_opaque: F)
    where
        F: FnMut(Point) -> bool,
    {
        self.set_visible(center);

        for octant in 0..8 {
            self.compute_octant(center, radius, octant, 1, 0.0, 1.0, &mut is_opaque);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn compute_octant<F>(
        &mut self,
        center: Point,
        radius: u16,
        octant: u8,
        row: u16,
        mut start_slope: f32,
        end_slope: f32,
        is_opaque: &mut F,
    ) where
        F: FnMut(Point) -> bool,
    {
        if start_slope >= end_slope {
            return;
        }

        let radius_f = radius as f32;
        let mut prev_opaque = false;

        for r in row..=radius {
            let r_f = r as f32;
            let mut next_start_slope = start_slope;

            for col in 0..=r {
                let col_f = col as f32;
                let slope_l = (col_f - 0.5) / r_f;
                let slope_r = (col_f + 0.5) / r_f;

                if slope_r < start_slope {
                    continue;
                }
                if slope_l > end_slope {
                    break;
                }

                if col_f * col_f + r_f * r_f > radius_f * radius_f {
                    continue;
                }

                let p = self.transform_octant(center, r, col, octant);
                if p.x >= 0 && p.x < self.width() as i16 && p.y >= 0 && p.y < self.height() as i16 {
                    self.set_visible(p);
                }

                let current_opaque = if p.x >= 0
                    && p.x < self.width() as i16
                    && p.y >= 0
                    && p.y < self.height() as i16
                {
                    is_opaque(p)
                } else {
                    true
                };

                if prev_opaque && !current_opaque {
                    next_start_slope = slope_l;
                } else if !prev_opaque && current_opaque && r < radius {
                    self.compute_octant(
                        center,
                        radius,
                        octant,
                        r + 1,
                        start_slope,
                        slope_l,
                        is_opaque,
                    );
                }
                prev_opaque = current_opaque;
            }

            if prev_opaque {
                break;
            }
            start_slope = next_start_slope;
        }
    }

    fn transform_octant(&self, center: Point, row: u16, col: u16, octant: u8) -> Point {
        let r = row as i16;
        let c = col as i16;
        match octant {
            0 => Point::new(center.x + c, center.y - r),
            1 => Point::new(center.x + r, center.y - c),
            2 => Point::new(center.x + r, center.y + c),
            3 => Point::new(center.x + c, center.y + r),
            4 => Point::new(center.x - c, center.y + r),
            5 => Point::new(center.x - r, center.y + c),
            6 => Point::new(center.x - r, center.y - c),
            7 => Point::new(center.x - c, center.y - r),
            _ => Point::new(center.x, center.y),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_visibility_map_all_hidden() {
        let vm = VisibilityMap::new(5, 5);
        for y in 0..5 {
            for x in 0..5 {
                assert_eq!(vm.get(Point::new(x, y)), Visibility::Hidden);
            }
        }
    }

    #[test]
    fn set_visible_marks_tile() {
        let mut vm = VisibilityMap::new(5, 5);
        vm.set_visible(Point::new(2, 2));
        assert!(vm.is_visible(Point::new(2, 2)));
        assert!(!vm.is_visible(Point::new(0, 0)));
    }

    #[test]
    fn get_returns_hidden_for_out_of_bounds() {
        let vm = VisibilityMap::new(3, 3);
        assert_eq!(vm.get(Point::new(10, 10)), Visibility::Hidden);
        assert_eq!(vm.get(Point::new(-1, 0)), Visibility::Hidden);
    }

    #[test]
    fn clear_visible_demotes_to_explored() {
        let mut vm = VisibilityMap::new(5, 5);
        vm.set_visible(Point::new(1, 1));
        vm.set_visible(Point::new(3, 3));
        assert!(vm.is_visible(Point::new(1, 1)));

        vm.clear_visible();

        assert!(!vm.is_visible(Point::new(1, 1)));
        assert!(!vm.is_visible(Point::new(3, 3)));
        // Now explored
        assert!(vm.is_explored(Point::new(1, 1)));
        assert!(vm.is_explored(Point::new(3, 3)));
        // Never-set tile remains hidden
        assert!(!vm.is_explored(Point::new(0, 0)));
    }

    #[test]
    fn is_explored_false_for_hidden() {
        let vm = VisibilityMap::new(3, 3);
        assert!(!vm.is_explored(Point::new(1, 1)));
    }

    #[test]
    fn compute_fov_marks_center_visible() {
        let mut vm = VisibilityMap::new(10, 10);
        vm.compute_fov(Point::new(5, 5), 3, |_| false);
        assert!(vm.is_visible(Point::new(5, 5)));
    }

    #[test]
    fn compute_fov_marks_tiles_within_radius() {
        let mut vm = VisibilityMap::new(10, 10);
        vm.compute_fov(Point::new(5, 5), 3, |_| false);
        // Tiles within radius 3 should be visible (at least some)
        assert!(vm.is_visible(Point::new(5, 5)));
        assert!(vm.is_visible(Point::new(6, 5)));
        assert!(vm.is_visible(Point::new(5, 6)));
    }

    #[test]
    fn compute_fov_respects_radius() {
        let mut vm = VisibilityMap::new(20, 20);
        vm.compute_fov(Point::new(10, 10), 2, |_| false);
        // Very far tile should not be visible with small radius
        assert!(!vm.is_visible(Point::new(0, 0)));
        assert!(!vm.is_visible(Point::new(19, 19)));
    }

    #[test]
    fn compute_fov_opaque_blocks_vision() {
        let mut vm = VisibilityMap::new(10, 10);
        // Place a wall at (6, 5)
        vm.compute_fov(Point::new(4, 5), 5, |p| p == Point::new(6, 5));
        // Center should be visible
        assert!(vm.is_visible(Point::new(4, 5)));
        // Tile just before wall should be visible
        assert!(vm.is_visible(Point::new(5, 5)));
        // Wall itself should be visible
        assert!(vm.is_visible(Point::new(6, 5)));
        // Tile behind wall should NOT be visible (shadowed)
        assert!(!vm.is_visible(Point::new(7, 5)));
    }

    #[test]
    fn compute_fov_demotes_previous_visible() {
        let mut vm = VisibilityMap::new(10, 10);
        // First FOV
        vm.compute_fov(Point::new(5, 5), 3, |_| false);
        assert!(vm.is_visible(Point::new(5, 6)));

        // Second FOV from different position
        vm.compute_fov(Point::new(2, 2), 3, |_| false);
        // Old position's tiles should no longer be visible
        assert!(!vm.is_visible(Point::new(5, 6)));
        // But should be explored
        assert!(vm.is_explored(Point::new(5, 6)));
    }

    #[test]
    fn compute_fov_bounds_clipping() {
        let mut vm = VisibilityMap::new(5, 5);
        // FOV at corner - should not panic and should clip to grid
        vm.compute_fov(Point::new(0, 0), 10, |_| false);
        assert!(vm.is_visible(Point::new(0, 0)));
        // Out-of-bounds tiles should remain hidden
        assert_eq!(vm.get(Point::new(10, 10)), Visibility::Hidden);
    }

    #[test]
    fn compute_fov_incremental_accumulates() {
        let mut vm = VisibilityMap::new(10, 10);
        vm.clear_visible();
        vm.compute_fov_incremental(Point::new(1, 1), 2, |_| false);
        vm.compute_fov_incremental(Point::new(8, 8), 2, |_| false);

        // Both centers should be visible
        assert!(vm.is_visible(Point::new(1, 1)));
        assert!(vm.is_visible(Point::new(8, 8)));
        // Non-overlapping far tiles should not be visible
        assert!(!vm.is_visible(Point::new(5, 5)));
    }
}
