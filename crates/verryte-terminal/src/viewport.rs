use crate::camera::Camera;
use crate::grid::Grid;
use crate::layout::Rect;

/// A viewport that maps a logical tile grid to a physical terminal cell grid.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TileViewport {
    pub rect: Rect,
    pub tile_w: u16,
    pub tile_h: u16,
    pub camera: Camera,
}

impl TileViewport {
    pub fn new(rect: Rect, tile_w: u16, tile_h: u16) -> Self {
        Self {
            rect,
            tile_w,
            tile_h,
            camera: Camera::new(0.0, 0.0),
        }
    }

    pub fn world_to_screen(&self, world_x: f32, world_y: f32) -> (i32, i32) {
        let (cam_x, cam_y) = self.camera.top_left(self.rect.width, self.rect.height);
        let screen_x = (world_x * self.tile_w as f32).round() as i32 - cam_x as i32;
        let screen_y = (world_y * self.tile_h as f32).round() as i32 - cam_y as i32;
        (screen_x, screen_y)
    }

    pub fn screen_to_world(&self, screen_x: i32, screen_y: i32) -> (f32, f32) {
        let (cam_x, cam_y) = self.camera.top_left(self.rect.width, self.rect.height);
        let world_x = (screen_x + cam_x as i32) as f32 / self.tile_w as f32;
        let world_y = (screen_y + cam_y as i32) as f32 / self.tile_h as f32;
        (world_x, world_y)
    }

    pub fn visible_tiles(&self, map_w: u16, map_h: u16) -> (i16, i16, i16, i16) {
        let (cam_x, cam_y) = self.camera.top_left(self.rect.width, self.rect.height);
        let start_x = (cam_x as f32 / self.tile_w as f32).floor() as i16;
        let start_y = (cam_y as f32 / self.tile_h as f32).floor() as i16;
        let end_x = ((cam_x as f32 + self.rect.width as f32) / self.tile_w as f32).ceil() as i16;
        let end_y = ((cam_y as f32 + self.rect.height as f32) / self.tile_h as f32).ceil() as i16;

        (
            start_x.max(0),
            start_y.max(0),
            end_x.min(map_w as i16),
            end_y.min(map_h as i16),
        )
    }

    pub fn blit_sprite(&self, grid: &mut Grid, world_x: f32, world_y: f32, sprite: &Grid) {
        let (sx, sy) = self.world_to_screen(world_x, world_y);
        let rx = sx + (self.tile_w as i32 - sprite.width() as i32) / 2;
        let ry = sy + (self.tile_h as i32 - sprite.height() as i32) / 2;

        for (ox, oy, cell) in sprite.iter_cells() {
            if cell.is_transparent() {
                continue;
            }
            let tx = self.rect.x as i32 + rx + ox as i32;
            let ty = self.rect.y as i32 + ry + oy as i32;

            if self.rect.contains(tx as u16, ty as u16) {
                grid.put(tx as u16, ty as u16, *cell);
            }
        }
    }

    /// Renders a single layer of a tile grid into the screen grid.
    pub fn render_layer<T: Clone>(
        &self,
        screen: &mut Grid,
        layer: &verryte_map::TileGrid<T>,
        render_tile: impl Fn(&T) -> crate::grid::Cell,
    ) {
        let (x1, y1, x2, y2) = self.visible_tiles(layer.width(), layer.height());
        for ty in y1..y2 {
            for tx in x1..x2 {
                if let Some(tile) = layer.get(verryte_map::Point::new(tx, ty)) {
                    let cell = render_tile(tile);
                    if cell.is_transparent() {
                        continue;
                    }
                    let (sx, sy) = self.world_to_screen(tx as f32, ty as f32);
                    for dy in 0..self.tile_h {
                        for dx in 0..self.tile_w {
                            let abs_x = self.rect.x as i32 + sx + dx as i32;
                            let abs_y = self.rect.y as i32 + sy + dy as i32;
                            if self.rect.contains(abs_x as u16, abs_y as u16) {
                                screen.put(abs_x as u16, abs_y as u16, cell);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Renders multiple layers from a 3D tile grid.
    pub fn render_grid3<T: Clone>(
        &self,
        screen: &mut Grid,
        grid3: &verryte_map::TileGrid3<T>,
        render_tile: impl Fn(&T) -> crate::grid::Cell,
    ) {
        for z in 0..grid3.depth() {
            if let Some(layer) = grid3.layer(z as i16) {
                self.render_layer(screen, layer, &render_tile);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Cell;
    use verryte_map::{Point, TileGrid};

    fn make_viewport() -> TileViewport {
        TileViewport::new(Rect::new(0, 0, 40, 20), 8, 4)
    }

    #[test]
    fn viewport_new_sets_fields() {
        let vp = make_viewport();
        assert_eq!(vp.rect, Rect::new(0, 0, 40, 20));
        assert_eq!(vp.tile_w, 8);
        assert_eq!(vp.tile_h, 4);
    }

    #[test]
    fn world_to_screen_at_origin() {
        // Camera at (0,0) centers viewport, so top_left = (-20, -10)
        // world_to_screen(0,0) = 0*8 - (-20) = 20, 0*4 - (-10) = 10
        let vp = make_viewport();
        let (sx, sy) = vp.world_to_screen(0.0, 0.0);
        assert_eq!(sx, 20);
        assert_eq!(sy, 10);
    }

    #[test]
    fn world_to_screen_with_tile_offset() {
        let vp = make_viewport();
        let (sx, sy) = vp.world_to_screen(2.0, 3.0);
        // 2*8 + 20 = 36, 3*4 + 10 = 22
        assert_eq!(sx, 36);
        assert_eq!(sy, 22);
    }

    #[test]
    fn screen_to_world_at_origin() {
        let vp = make_viewport();
        let (wx, wy) = vp.screen_to_world(20, 10);
        assert!((wx - 0.0).abs() < 0.1);
        assert!((wy - 0.0).abs() < 0.1);
    }

    #[test]
    fn world_screen_roundtrip() {
        let vp = make_viewport();
        let (sx, sy) = vp.world_to_screen(3.0, 5.0);
        let (wx, wy) = vp.screen_to_world(sx, sy);
        assert!((wx - 3.0).abs() < 0.1);
        assert!((wy - 5.0).abs() < 0.1);
    }

    #[test]
    fn visible_tiles_clamps_to_map() {
        let vp = make_viewport();
        let (x1, y1, x2, y2) = vp.visible_tiles(3, 3);
        assert_eq!(x1, 0);
        assert_eq!(y1, 0);
        assert_eq!(x2, 3);
        assert_eq!(y2, 3);
    }

    #[test]
    fn blit_sprite_centers_in_tile() {
        let vp = make_viewport();
        let mut screen = Grid::new(40, 20);
        let sprite = Grid::new(4, 2);
        vp.blit_sprite(&mut screen, 0.0, 0.0, &sprite);
        // Should not panic
    }

    #[test]
    fn render_layer_fills_tiles() {
        let vp = make_viewport();
        let mut screen = Grid::new(40, 20);
        let mut layer: TileGrid<bool> = TileGrid::new(3, 3, false);
        layer.set(Point::new(0, 0), true);
        layer.set(Point::new(1, 1), true);

        vp.render_layer(&mut screen, &layer, |&val| {
            if val {
                Cell::new('#')
            } else {
                Cell::new('.')
            }
        });

        // Tile (0,0) maps to screen (20, 10) which is within 40x20 grid
        let cell = screen.get(20, 10).unwrap();
        assert_eq!(cell.glyph, '#');

        // Tile (1,1) maps to screen (28, 14) which is within 40x20 grid
        let cell = screen.get(28, 14).unwrap();
        assert_eq!(cell.glyph, '#');
    }

    #[test]
    fn render_layer_skips_transparent() {
        let vp = make_viewport();
        let mut screen = Grid::new(40, 20);
        // Place marker at tile (0,0) screen location
        screen.put(20, 10, Cell::new('X'));
        let layer: TileGrid<bool> = TileGrid::new(2, 2, false);

        vp.render_layer(&mut screen, &layer, |_| Cell::EMPTY);

        // EMPTY cells are transparent, should not overwrite
        let cell = screen.get(20, 10).unwrap();
        assert_eq!(cell.glyph, 'X');
    }
}
