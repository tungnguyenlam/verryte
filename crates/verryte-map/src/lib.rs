//! Map and spatial primitives for Verryte games.
//!
//! This crate owns the small, reusable pieces that terminal grid games keep
//! needing: integer positions, cardinal directions, rectangular sizes, and a
//! typed tile grid. It deliberately does not know about rendering, ECS storage,
//! input, or game-specific tile meanings.

mod bounds;
mod dijkstra;
mod direction;
mod error;
mod grid;
mod grid3;
mod line;
mod point;
mod reachability;
mod rect;
mod shape;
mod size;
mod spatial_hash;
mod visibility;

pub use bounds::Bounds;
pub use dijkstra::DijkstraMap;
pub use direction::{Direction, Direction8};
pub use error::GridError;
pub use grid::TileGrid;
pub use grid3::TileGrid3;
pub use line::{
    compress_path_to_waypoints, has_line_of_sight, line_between, path_to_directions, raycast,
    LineIter,
};
pub use point::{Point, Point3};
pub use reachability::ReachabilityMap;
pub use rect::Rect;
pub use shape::TileShape;
pub use size::Size;
pub use spatial_hash::SpatialHash;
pub use visibility::{Visibility, VisibilityMap};

#[cfg(test)]
mod tests;
