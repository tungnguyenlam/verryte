use verryte_map::{Point, TileGrid};

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Tile {
    Grass,
    Wall,
    Water,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TacticalMap {
    pub width: u16,
    pub height: u16,
    pub tiles: TileGrid<Tile>,
}

impl TacticalMap {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            tiles: TileGrid::new(width, height, Tile::Grass),
        }
    }

    pub fn tile(&self, x: i16, y: i16) -> Tile {
        self.tiles
            .get(Point::new(x, y))
            .copied()
            .unwrap_or(Tile::Wall)
    }

    pub fn is_walkable(&self, pt: Point) -> bool {
        matches!(self.tile(pt.x, pt.y), Tile::Grass)
    }
}
