//! Terminal-cell primitives.
//!
//! Verryte treats one terminal cell as the basic visual unit. This crate owns
//! the data structures for that model: [`Color`], [`Cell`], [`Grid`], and a
//! plain-text renderer that is enough for tests and snapshot comparisons.
//!
//! Actual TTY I/O — alternate screens, raw mode, ANSI emission — belongs in a
//! separate frontend crate that consumes a [`Grid`]. Keeping that boundary
//! lets the same game be rendered, snapshot-tested, or fed to an agent
//! without rewiring the engine.

/// 24-bit RGB color.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Color(pub u8, pub u8, pub u8);

impl Color {
    pub const BLACK: Color = Color(0, 0, 0);
    pub const WHITE: Color = Color(230, 230, 230);
    pub const RED: Color = Color(220, 60, 60);
    pub const GREEN: Color = Color(80, 200, 120);
    pub const BLUE: Color = Color(80, 130, 220);
    pub const YELLOW: Color = Color(220, 200, 80);
    pub const CYAN: Color = Color(100, 200, 220);
    pub const MAGENTA: Color = Color(200, 100, 200);
    pub const GREY: Color = Color(140, 140, 140);
    pub const DARK_GREY: Color = Color(60, 60, 60);
}

/// One terminal cell.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    pub glyph: char,
    pub fg: Color,
    pub bg: Color,
}

impl Cell {
    pub const EMPTY: Cell = Cell {
        glyph: ' ',
        fg: Color::WHITE,
        bg: Color::BLACK,
    };

    pub fn new(glyph: char) -> Self {
        Cell {
            glyph,
            fg: Color::WHITE,
            bg: Color::BLACK,
        }
    }

    pub fn with_fg(mut self, fg: Color) -> Self {
        self.fg = fg;
        self
    }

    pub fn with_bg(mut self, bg: Color) -> Self {
        self.bg = bg;
        self
    }

    /// Treat a space glyph as transparent for the purposes of layered draws.
    pub fn is_transparent(&self) -> bool {
        self.glyph == ' '
    }
}

/// Integer rectangle in terminal-cell coordinates.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    pub fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn right(self) -> u16 {
        self.x.saturating_add(self.width)
    }

    pub fn bottom(self) -> u16 {
        self.y.saturating_add(self.height)
    }

    pub fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }
}

/// A fixed-size rectangular cell buffer.
#[derive(Clone, Debug)]
pub struct Grid {
    width: u16,
    height: u16,
    cells: Vec<Cell>,
}

impl Grid {
    pub fn new(width: u16, height: u16) -> Self {
        let size = (width as usize) * (height as usize);
        Self {
            width,
            height,
            cells: vec![Cell::EMPTY; size],
        }
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    pub fn clear(&mut self, cell: Cell) {
        for c in &mut self.cells {
            *c = cell;
        }
    }

    fn index(&self, x: u16, y: u16) -> Option<usize> {
        if x < self.width && y < self.height {
            Some((y as usize) * (self.width as usize) + (x as usize))
        } else {
            None
        }
    }

    pub fn get(&self, x: u16, y: u16) -> Option<&Cell> {
        let i = self.index(x, y)?;
        Some(&self.cells[i])
    }

    /// Write a cell at (x, y). Returns `false` if the position is out of bounds.
    pub fn put(&mut self, x: u16, y: u16, cell: Cell) -> bool {
        if let Some(i) = self.index(x, y) {
            self.cells[i] = cell;
            true
        } else {
            false
        }
    }

    /// Write a string left-to-right, clipping at the right edge.
    pub fn write_str(&mut self, x: u16, y: u16, text: &str, fg: Color, bg: Color) {
        let mut cx = x;
        for ch in text.chars() {
            if cx >= self.width {
                break;
            }
            self.put(cx, y, Cell { glyph: ch, fg, bg });
            cx = cx.saturating_add(1);
        }
    }

    /// Fill a clipped rectangle with one cell.
    pub fn fill_rect(&mut self, rect: Rect, cell: Cell) {
        let x_end = rect.right().min(self.width);
        let y_end = rect.bottom().min(self.height);
        for y in rect.y..y_end {
            for x in rect.x..x_end {
                self.put(x, y, cell);
            }
        }
    }

    /// Draw a clipped single-cell border around a rectangle.
    pub fn draw_border(&mut self, rect: Rect, cell: Cell) {
        if rect.is_empty() {
            return;
        }
        let x_end = rect.right().min(self.width);
        let y_end = rect.bottom().min(self.height);
        if rect.x >= x_end || rect.y >= y_end {
            return;
        }
        let top = rect.y;
        let bottom = y_end - 1;
        for x in rect.x..x_end {
            self.put(x, top, cell);
            self.put(x, bottom, cell);
        }
        for y in rect.y..y_end {
            self.put(rect.x, y, cell);
            self.put(x_end - 1, y, cell);
        }
    }

    /// Copy `other` into `self` at (dst_x, dst_y), treating transparent cells
    /// (space glyph) as "do not draw". This is the engine's layer-composition
    /// primitive.
    pub fn blit(&mut self, other: &Grid, dst_x: i32, dst_y: i32) {
        for sy in 0..other.height as i32 {
            for sx in 0..other.width as i32 {
                let dx = dst_x + sx;
                let dy = dst_y + sy;
                if dx < 0 || dy < 0 {
                    continue;
                }
                let (dx, dy) = (dx as u16, dy as u16);
                if dx >= self.width || dy >= self.height {
                    continue;
                }
                let src = other
                    .get(sx as u16, sy as u16)
                    .copied()
                    .unwrap_or(Cell::EMPTY);
                if src.is_transparent() {
                    continue;
                }
                self.put(dx, dy, src);
            }
        }
    }

    /// Render the grid as plain rows, one line per row, glyphs only. Useful
    /// for tests and snapshot dumps.
    pub fn to_plain_string(&self) -> String {
        let mut out = String::with_capacity(self.cells.len() + self.height as usize);
        for y in 0..self.height {
            for x in 0..self.width {
                out.push(self.cells[(y as usize) * (self.width as usize) + (x as usize)].glyph);
            }
            if y + 1 < self.height {
                out.push('\n');
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_and_get_within_bounds() {
        let mut grid = Grid::new(3, 2);
        assert!(grid.put(1, 1, Cell::new('x')));
        assert_eq!(grid.get(1, 1).unwrap().glyph, 'x');
    }

    #[test]
    fn out_of_bounds_writes_are_clipped() {
        let mut grid = Grid::new(2, 2);
        assert!(!grid.put(5, 5, Cell::new('!')));
        assert_eq!(grid.get(5, 5), None);
    }

    #[test]
    fn write_str_clips_at_right_edge() {
        let mut grid = Grid::new(4, 1);
        grid.write_str(2, 0, "hello", Color::WHITE, Color::BLACK);
        assert_eq!(grid.to_plain_string(), "  he");
    }

    #[test]
    fn fill_rect_is_clipped_to_grid() {
        let mut grid = Grid::new(4, 3);
        grid.fill_rect(Rect::new(2, 1, 5, 5), Cell::new('x'));
        assert_eq!(grid.to_plain_string(), "    \n  xx\n  xx");
    }

    #[test]
    fn draw_border_handles_small_and_clipped_rects() {
        let mut grid = Grid::new(5, 4);
        grid.draw_border(Rect::new(1, 1, 3, 2), Cell::new('#'));
        assert_eq!(grid.to_plain_string(), "     \n ### \n ### \n     ");

        grid.draw_border(Rect::new(4, 3, 5, 5), Cell::new('!'));
        assert_eq!(grid.get(4, 3).unwrap().glyph, '!');
    }

    #[test]
    fn blit_skips_transparent_cells() {
        let mut base = Grid::new(4, 1);
        base.write_str(0, 0, "....", Color::WHITE, Color::BLACK);
        let mut overlay = Grid::new(4, 1);
        // Space at index 0 should remain transparent.
        overlay.put(1, 0, Cell::new('@'));
        base.blit(&overlay, 0, 0);
        assert_eq!(base.to_plain_string(), ".@..");
    }

    #[test]
    fn plain_string_has_one_newline_between_rows() {
        let mut grid = Grid::new(2, 2);
        grid.put(0, 0, Cell::new('a'));
        grid.put(1, 0, Cell::new('b'));
        grid.put(0, 1, Cell::new('c'));
        grid.put(1, 1, Cell::new('d'));
        assert_eq!(grid.to_plain_string(), "ab\ncd");
    }
}
