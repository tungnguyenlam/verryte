use crate::grid::Grid;

/// A named rendering layer.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Layer {
    pub name: String,
    pub order: u8,
    pub grid: Grid,
    pub visible: bool,
}

impl Layer {
    pub fn new<S: Into<String>>(name: S, order: u8, grid: Grid) -> Self {
        Self {
            name: name.into(),
            order,
            grid,
            visible: true,
        }
    }

    /// Composite all visible layers onto a target grid, respecting draw order.
    pub fn composite(layers: &[Layer], target: &mut Grid) {
        let mut sorted: Vec<&Layer> = layers.iter().filter(|l| l.visible).collect();
        sorted.sort_by_key(|l| l.order);
        for layer in sorted {
            target.blit(&layer.grid, 0, 0);
        }
    }
}

/// A collection of named rendering layers.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Layers {
    layers: Vec<Layer>,
}

impl Layers {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, layer: Layer) {
        if let Some(pos) = self.layers.iter().position(|l| l.name == layer.name) {
            self.layers[pos] = layer;
        } else {
            self.layers.push(layer);
        }
    }

    pub fn get(&self, name: &str) -> Option<&Layer> {
        self.layers.iter().find(|l| l.name == name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.name == name)
    }

    pub fn remove(&mut self, name: &str) -> bool {
        if let Some(pos) = self.layers.iter().position(|l| l.name == name) {
            self.layers.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn composite(&self, target: &mut Grid) {
        Layer::composite(&self.layers, target);
    }

    pub fn len(&self) -> usize {
        self.layers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Layer> {
        let mut sorted: Vec<&Layer> = self.layers.iter().collect();
        sorted.sort_by_key(|l| l.order);
        sorted.into_iter()
    }
}
