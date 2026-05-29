use crate::{Point3, TileGrid};

/// A 3D grid of tiles, represented as multiple 2D layers.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(bound = "T: serde::Serialize + serde::de::DeserializeOwned")
)]
pub struct TileGrid3<T> {
    layers: Vec<TileGrid<T>>,
}

impl<T: Clone> TileGrid3<T> {
    pub fn new(width: u16, height: u16, depth: u16, fill: T) -> Self {
        let mut layers = Vec::with_capacity(depth as usize);
        for _ in 0..depth {
            layers.push(TileGrid::new(width, height, fill.clone()));
        }
        Self { layers }
    }

    pub fn width(&self) -> u16 {
        self.layers.first().map(|l| l.width()).unwrap_or(0)
    }

    pub fn height(&self) -> u16 {
        self.layers.first().map(|l| l.height()).unwrap_or(0)
    }

    pub fn depth(&self) -> u16 {
        self.layers.len() as u16
    }

    pub fn get(&self, p: Point3) -> Option<&T> {
        self.layers.get(p.z as usize)?.get(p.to_2d())
    }

    pub fn get_mut(&mut self, p: Point3) -> Option<&mut T> {
        self.layers.get_mut(p.z as usize)?.get_mut(p.to_2d())
    }

    pub fn set(&mut self, p: Point3, tile: T) -> bool {
        if let Some(layer) = self.layers.get_mut(p.z as usize) {
            layer.set(p.to_2d(), tile)
        } else {
            false
        }
    }

    pub fn layer(&self, z: i16) -> Option<&TileGrid<T>> {
        self.layers.get(z as usize)
    }

    pub fn layer_mut(&mut self, z: i16) -> Option<&mut TileGrid<T>> {
        self.layers.get_mut(z as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid3_new_dimensions() {
        let grid = TileGrid3::new(4, 5, 3, 0u8);
        assert_eq!(grid.width(), 4);
        assert_eq!(grid.height(), 5);
        assert_eq!(grid.depth(), 3);
    }

    #[test]
    fn grid3_get_returns_fill() {
        let grid = TileGrid3::new(3, 3, 2, 42u8);
        assert_eq!(grid.get(Point3::new(0, 0, 0)), Some(&42));
        assert_eq!(grid.get(Point3::new(2, 1, 1)), Some(&42));
    }

    #[test]
    fn grid3_get_out_of_bounds_returns_none() {
        let grid = TileGrid3::new(3, 3, 2, 0u8);
        assert_eq!(grid.get(Point3::new(5, 5, 0)), None);
        assert_eq!(grid.get(Point3::new(0, 0, 5)), None);
    }

    #[test]
    fn grid3_set_and_get() {
        let mut grid = TileGrid3::new(3, 3, 2, 0u8);
        assert!(grid.set(Point3::new(1, 2, 0), 99));
        assert_eq!(grid.get(Point3::new(1, 2, 0)), Some(&99));
        // Other layer unchanged
        assert_eq!(grid.get(Point3::new(1, 2, 1)), Some(&0));
    }

    #[test]
    fn grid3_set_out_of_bounds_returns_false() {
        let mut grid = TileGrid3::new(3, 3, 2, 0u8);
        assert!(!grid.set(Point3::new(10, 10, 0), 1));
        assert!(!grid.set(Point3::new(0, 0, 10), 1));
    }

    #[test]
    fn grid3_get_mut_and_modify() {
        let mut grid = TileGrid3::new(3, 3, 2, 0u8);
        if let Some(tile) = grid.get_mut(Point3::new(1, 1, 0)) {
            *tile = 55;
        }
        assert_eq!(grid.get(Point3::new(1, 1, 0)), Some(&55));
    }

    #[test]
    fn grid3_layer_access() {
        let grid = TileGrid3::new(3, 3, 3, 7u8);
        assert!(grid.layer(0).is_some());
        assert!(grid.layer(2).is_some());
        assert!(grid.layer(3).is_none());
    }

    #[test]
    fn grid3_layer_mut() {
        let mut grid = TileGrid3::new(3, 3, 2, 0u8);
        if let Some(layer) = grid.layer_mut(0) {
            layer.set(crate::Point::new(1, 1), 88);
        }
        assert_eq!(grid.get(Point3::new(1, 1, 0)), Some(&88));
    }

    #[test]
    fn grid3_layers_are_independent() {
        let mut grid = TileGrid3::new(3, 3, 3, 0u8);
        grid.set(Point3::new(0, 0, 0), 1);
        grid.set(Point3::new(0, 0, 1), 2);
        grid.set(Point3::new(0, 0, 2), 3);
        assert_eq!(grid.get(Point3::new(0, 0, 0)), Some(&1));
        assert_eq!(grid.get(Point3::new(0, 0, 1)), Some(&2));
        assert_eq!(grid.get(Point3::new(0, 0, 2)), Some(&3));
    }
}
