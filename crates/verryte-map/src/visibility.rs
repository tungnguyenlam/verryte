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
    pub fn compute_fov<F>(&mut self, center: Point, radius: u16, mut is_opaque: F)
    where
        F: FnMut(Point) -> bool,
    {
        self.clear_visible();
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
