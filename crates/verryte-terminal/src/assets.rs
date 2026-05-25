use crate::color::Color;
use crate::grid::{Cell, CellAttrs, Grid};
use crate::sprite::{ResolutionTier, Sprite};

/// Translates a graphical image into a terminal cell grid using half-block characters (`▀`).
pub fn image_to_grid(img: &image::DynamicImage) -> Grid {
    use image::GenericImageView;
    let (width, height) = img.dimensions();
    let grid_width = width as u16;
    let grid_height = height.div_ceil(2) as u16;
    let mut grid = Grid::new(grid_width, grid_height);

    for y in 0..grid_height {
        for x in 0..grid_width {
            let img_x = x as u32;
            let img_y_top = (y * 2) as u32;
            let img_y_bottom = (y * 2 + 1) as u32;

            let top_pixel = img.get_pixel(img_x, img_y_top);
            let top_fg = Color(top_pixel[0], top_pixel[1], top_pixel[2]);

            let bottom_bg = if img_y_bottom < height {
                let bottom_pixel = img.get_pixel(img_x, img_y_bottom);
                Color(bottom_pixel[0], bottom_pixel[1], bottom_pixel[2])
            } else {
                Color::BLACK
            };

            let cell = Cell {
                glyph: '▀',
                fg: top_fg,
                bg: bottom_bg,
                attrs: CellAttrs::NONE,
            };
            grid.put(x, y, cell);
        }
    }

    grid
}

/// Translates a graphical image into a terminal cell grid with chroma-key transparency.
pub fn image_to_grid_with_chroma_key(
    img: &image::DynamicImage,
    chroma_key: Color,
    tolerance: u8,
) -> Grid {
    use image::GenericImageView;
    let (width, height) = img.dimensions();
    let grid_width = width as u16;
    let grid_height = height.div_ceil(2) as u16;
    let mut grid = Grid::new(grid_width, grid_height);

    let is_chroma = |c: Color| -> bool {
        let dr = (c.0 as i16 - chroma_key.0 as i16).unsigned_abs() as u8;
        let dg = (c.1 as i16 - chroma_key.1 as i16).unsigned_abs() as u8;
        let db = (c.2 as i16 - chroma_key.2 as i16).unsigned_abs() as u8;
        dr < tolerance && dg < tolerance && db < tolerance
    };

    for y in 0..grid_height {
        for x in 0..grid_width {
            let img_x = x as u32;
            let img_y_top = (y * 2) as u32;
            let img_y_bottom = (y * 2 + 1) as u32;

            let top_pixel = img.get_pixel(img_x, img_y_top);
            let top_color = Color(top_pixel[0], top_pixel[1], top_pixel[2]);
            let top_transparent = is_chroma(top_color);

            let (bottom_color, bottom_transparent) = if img_y_bottom < height {
                let bottom_pixel = img.get_pixel(img_x, img_y_bottom);
                let c = Color(bottom_pixel[0], bottom_pixel[1], bottom_pixel[2]);
                (c, is_chroma(c))
            } else {
                (Color::BLACK, false)
            };

            let cell = if top_transparent && bottom_transparent {
                Cell::EMPTY
            } else if top_transparent {
                Cell {
                    glyph: '▄',
                    fg: bottom_color,
                    bg: Color::BLACK,
                    attrs: CellAttrs::NONE,
                }
            } else if bottom_transparent {
                Cell {
                    glyph: '▀',
                    fg: top_color,
                    bg: Color::BLACK,
                    attrs: CellAttrs::NONE,
                }
            } else {
                Cell {
                    glyph: '▀',
                    fg: top_color,
                    bg: bottom_color,
                    attrs: CellAttrs::NONE,
                }
            };
            grid.put(x, y, cell);
        }
    }

    grid
}

/// Represents a visual asset mapping to different fidelity levels.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum VisualAsset {
    SingleCell(Grid),
    BlockSprite(Grid),
    Animated(Sprite),
}

impl VisualAsset {
    pub fn single_cell(glyph: char, fg: Color, bg: Color) -> Self {
        let mut grid = Grid::new(1, 1);
        grid.put(0, 0, Cell::new(glyph).with_fg(fg).with_bg(bg));
        VisualAsset::SingleCell(grid)
    }

    pub fn render(&self, tier: ResolutionTier) -> &Grid {
        match self {
            VisualAsset::SingleCell(grid) => grid,
            VisualAsset::BlockSprite(grid) => grid,
            VisualAsset::Animated(sprite) => sprite.current_frame_at(tier),
        }
    }
}

/// A registry mapping semantic keys to visual assets.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VisualRegistry {
    assets: std::collections::HashMap<String, VisualAsset>,
}

impl VisualRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, name: &str, asset: VisualAsset) {
        self.assets.insert(name.to_string(), asset);
    }

    pub fn register_single_cell(&mut self, name: &str, glyph: char, fg: Color, bg: Color) {
        self.register(name, VisualAsset::single_cell(glyph, fg, bg));
    }

    pub fn get(&self, name: &str) -> Option<&VisualAsset> {
        self.assets.get(name)
    }

    pub fn register_image(&mut self, name: &str, img: &image::DynamicImage) {
        let grid = image_to_grid(img);
        self.register(name, VisualAsset::BlockSprite(grid));
    }

    pub fn register_image_with_chroma_key(
        &mut self,
        name: &str,
        img: &image::DynamicImage,
        chroma_key: Color,
        tolerance: u8,
    ) {
        let grid = image_to_grid_with_chroma_key(img, chroma_key, tolerance);
        self.register(name, VisualAsset::BlockSprite(grid));
    }

    pub fn register_sprite(&mut self, sprite: Sprite) {
        let name = sprite.name.clone();
        self.register(&name, VisualAsset::Animated(sprite));
    }
}
