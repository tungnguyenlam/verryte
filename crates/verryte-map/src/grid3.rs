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
