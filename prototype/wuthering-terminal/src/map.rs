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

    pub fn from_ascii(ascii: &str) -> Self {
        let lines: Vec<&str> = ascii.trim().lines().map(|l| l.trim()).collect();
        let height = lines.len() as u16;
        let width = lines.iter().map(|l| l.len()).max().unwrap_or(0) as u16;

        let mut tiles = TileGrid::new(width, height, Tile::Grass);
        for (y, line) in lines.iter().enumerate() {
            for (x, ch) in line.chars().enumerate() {
                let tile = match ch {
                    '#' => Tile::Wall,
                    '~' => Tile::Water,
                    _ => Tile::Grass,
                };
                tiles.set(Point::new(x as i16, y as i16), tile);
            }
        }

        Self {
            width,
            height,
            tiles,
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
