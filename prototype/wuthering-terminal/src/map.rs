use crate::components::WeatherType;
use verryte_map::{Point, TileGrid};

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Tile {
    Grass,
    Wall,
    Water,
    Lava,
    Ice,
    Stairs,
    Mud,
    SpikeTrap,
    PoisonCloud,
    HealingSpring,
    CrackedFloor,
    PressurePlate,
    ThornBush,
    SteamVent,
    ExplodingBarrel,
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
                    '^' => Tile::Lava,
                    '-' => Tile::Ice,
                    '>' => Tile::Stairs,
                    '=' => Tile::Mud,
                    '!' => Tile::SpikeTrap,
                    'p' => Tile::PoisonCloud,
                    '+' => Tile::HealingSpring,
                    '%' => Tile::CrackedFloor,
                    'T' => Tile::PressurePlate,
                    '*' => Tile::ThornBush,
                    'v' | 'V' => Tile::SteamVent,
                    'o' => Tile::ExplodingBarrel,
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
        matches!(
            self.tile(pt.x, pt.y),
            Tile::Grass
                | Tile::Water
                | Tile::Lava
                | Tile::Ice
                | Tile::Stairs
                | Tile::Mud
                | Tile::SpikeTrap
                | Tile::PoisonCloud
                | Tile::HealingSpring
                | Tile::CrackedFloor
                | Tile::PressurePlate
                | Tile::ThornBush
                | Tile::SteamVent
        )
    }

    pub fn movement_cost(&self, pt: Point) -> i32 {
        match self.tile(pt.x, pt.y) {
            Tile::Grass | Tile::Ice | Tile::Stairs => 1,
            Tile::Water | Tile::Lava => 2,
            Tile::Mud => 3,
            Tile::Wall => 999,
            Tile::SpikeTrap
            | Tile::PoisonCloud
            | Tile::HealingSpring
            | Tile::CrackedFloor
            | Tile::PressurePlate
            | Tile::SteamVent => 1,
            Tile::ThornBush => 2,
            Tile::ExplodingBarrel => 999,
        }
    }

    pub fn movement_cost_with_weather(&self, pt: Point, weather: WeatherType) -> i32 {
        match (self.tile(pt.x, pt.y), weather) {
            (Tile::Water, WeatherType::Rainy) => 1,
            (Tile::Ice, WeatherType::Snowing) => 0,
            _ => self.movement_cost(pt),
        }
    }

    pub fn add_ice_patches(&mut self, seed: u64, patch_count: usize) {
        let mut state = seed | 1;
        let mut rng = || -> u64 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };

        for _ in 0..patch_count {
            let cx = (rng() % (self.width.saturating_sub(2)) as u64) as i16 + 1;
            let cy = (rng() % (self.height.saturating_sub(2)) as u64) as i16 + 1;
            for dx in -1i16..=1 {
                for dy in -1i16..=1 {
                    let x = cx + dx;
                    let y = cy + dy;
                    if x >= 0
                        && x < self.width as i16
                        && y >= 0
                        && y < self.height as i16
                        && self.tile(x, y) == Tile::Grass
                    {
                        self.tiles.set(Point::new(x, y), Tile::Ice);
                    }
                }
            }
        }
    }

    pub fn tactical() -> Self {
        Self::from_ascii(
            "\
........................\n\
....##..........##.....\n\
....##..........##.....\n\
........................\n\
........~~~~....~~~~....\n\
........~~~~....~~~~....\n\
........................\n\
..........##...........\n\
..........#.#..........\n\
..........##...........\n\
........................\n\
........~~~~....^^^^....\n\
........~~~~....^^^^....\n\
........................\n\
....##..........##.....\n\
....##..........##.....",
        )
    }
}
