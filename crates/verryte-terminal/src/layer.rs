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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Cell;

    #[test]
    fn test_layer_new() {
        let grid = Grid::new(5, 5);
        let layer = Layer::new("bg", 0, grid.clone());
        assert_eq!(layer.name, "bg");
        assert_eq!(layer.order, 0);
        assert!(layer.visible);
    }

    #[test]
    fn test_layer_composite_ordering() {
        let mut bg = Grid::new(3, 3);
        bg.put(0, 0, Cell::new('B'));
        let mut fg = Grid::new(3, 3);
        fg.put(0, 0, Cell::new('F'));

        let layers = vec![Layer::new("fg", 10, fg), Layer::new("bg", 0, bg)];
        let mut target = Grid::new(3, 3);
        Layer::composite(&layers, &mut target);
        // Higher order (fg) should be on top
        assert_eq!(target.get(0, 0).unwrap().glyph, 'F');
    }

    #[test]
    fn test_layer_composite_skips_invisible() {
        let mut bg = Grid::new(3, 3);
        bg.put(0, 0, Cell::new('B'));
        let mut fg = Grid::new(3, 3);
        fg.put(0, 0, Cell::new('F'));

        let mut fg_layer = Layer::new("fg", 10, fg);
        fg_layer.visible = false;

        let layers = vec![fg_layer, Layer::new("bg", 0, bg)];
        let mut target = Grid::new(3, 3);
        Layer::composite(&layers, &mut target);
        assert_eq!(target.get(0, 0).unwrap().glyph, 'B');
    }

    #[test]
    fn test_layers_add_and_get() {
        let mut layers = Layers::new();
        layers.add(Layer::new("a", 0, Grid::new(2, 2)));
        layers.add(Layer::new("b", 1, Grid::new(2, 2)));
        assert_eq!(layers.len(), 2);
        assert!(layers.get("a").is_some());
        assert!(layers.get("b").is_some());
        assert!(layers.get("c").is_none());
    }

    #[test]
    fn test_layers_add_replaces_by_name() {
        let mut layers = Layers::new();
        layers.add(Layer::new("x", 0, Grid::new(2, 2)));
        layers.add(Layer::new("x", 1, Grid::new(3, 3)));
        assert_eq!(layers.len(), 1);
        assert_eq!(layers.get("x").unwrap().order, 1);
    }

    #[test]
    fn test_layers_remove() {
        let mut layers = Layers::new();
        layers.add(Layer::new("a", 0, Grid::new(2, 2)));
        assert!(layers.remove("a"));
        assert!(!layers.remove("a"));
        assert!(layers.is_empty());
    }

    #[test]
    fn test_layers_get_mut() {
        let mut layers = Layers::new();
        layers.add(Layer::new("a", 0, Grid::new(2, 2)));
        if let Some(layer) = layers.get_mut("a") {
            layer.visible = false;
        }
        assert!(!layers.get("a").unwrap().visible);
    }

    #[test]
    fn test_layers_composite() {
        let mut bg = Grid::new(3, 3);
        bg.put(0, 0, Cell::new('A'));
        let mut fg = Grid::new(3, 3);
        fg.put(0, 0, Cell::new('Z'));

        let mut layers = Layers::new();
        layers.add(Layer::new("bg", 0, bg));
        layers.add(Layer::new("fg", 10, fg));

        let mut target = Grid::new(3, 3);
        layers.composite(&mut target);
        assert_eq!(target.get(0, 0).unwrap().glyph, 'Z');
    }

    #[test]
    fn test_layers_iter_sorted() {
        let mut layers = Layers::new();
        layers.add(Layer::new("c", 20, Grid::new(1, 1)));
        layers.add(Layer::new("a", 0, Grid::new(1, 1)));
        layers.add(Layer::new("b", 10, Grid::new(1, 1)));
        let names: Vec<_> = layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, vec!["a", "b", "c"]);
    }
}
