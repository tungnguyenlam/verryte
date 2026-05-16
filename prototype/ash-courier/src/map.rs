use verryte_map::{Point, TileGrid};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Tile {
    Floor,
    Wall,
    Goal,
}

#[derive(Clone, Debug)]
pub struct Map {
    pub width: u16,
    pub height: u16,
    pub(crate) tiles: TileGrid<Tile>,
}

impl Map {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            tiles: TileGrid::new(width, height, Tile::Wall),
        }
    }

    pub fn tile(&self, x: i16, y: i16) -> Tile {
        self.tiles
            .get(Point { x, y })
            .copied()
            .unwrap_or(Tile::Wall)
    }

    pub fn is_walkable(&self, point: Point) -> bool {
        matches!(self.tile(point.x, point.y), Tile::Floor | Tile::Goal)
    }

    pub fn walkable_neighbors(&self, point: Point) -> Vec<Point> {
        self.tiles
            .neighbors4(point)
            .into_iter()
            .filter_map(|(neighbor, tile)| {
                matches!(tile, Tile::Floor | Tile::Goal).then_some(neighbor)
            })
            .collect()
    }

    pub fn visible_from(&self, point: Point, radius: u16) -> Vec<Point> {
        self.tiles
            .visible_points(point, radius, |tile| matches!(tile, Tile::Wall))
    }

    pub fn shortest_walkable_path(&self, start: Point, goal: Point) -> Option<Vec<Point>> {
        self.tiles.shortest_path4(start, goal, |_, tile| {
            matches!(tile, Tile::Floor | Tile::Goal)
        })
    }

    pub fn nearest_walkable_path(
        &self,
        start: Point,
        targets: impl IntoIterator<Item = Point>,
    ) -> Option<Vec<Point>> {
        self.tiles.nearest_path4(start, targets, |_, tile| {
            matches!(tile, Tile::Floor | Tile::Goal)
        })
    }

    pub fn nearest_walkable_distance(
        &self,
        start: Point,
        targets: impl IntoIterator<Item = Point>,
    ) -> Option<u16> {
        self.tiles.distance_to_nearest4(start, targets, |_, tile| {
            matches!(tile, Tile::Floor | Tile::Goal)
        })
    }

    pub fn reachable_from(&self, start: Point) -> Vec<Point> {
        self.tiles
            .reachable_points4(start, |_, tile| matches!(tile, Tile::Floor | Tile::Goal))
    }

    pub(crate) fn set(&mut self, x: u16, y: u16, tile: Tile) {
        self.tiles.set(Point::new(x as i16, y as i16), tile);
    }
}
