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

/// Bitflags for terminal cell text attributes.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct CellAttrs {
    pub bold: bool,
    pub underline: bool,
    pub dim: bool,
    pub italic: bool,
    pub reverse: bool,
    pub blink: bool,
}

impl CellAttrs {
    pub const NONE: CellAttrs = CellAttrs {
        bold: false,
        underline: false,
        dim: false,
        italic: false,
        reverse: false,
        blink: false,
    };

    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    pub fn underline(mut self) -> Self {
        self.underline = true;
        self
    }

    pub fn dim(mut self) -> Self {
        self.dim = true;
        self
    }

    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    pub fn reverse(mut self) -> Self {
        self.reverse = true;
        self
    }

    pub fn blink(mut self) -> Self {
        self.blink = true;
        self
    }

    /// Convert to ANSI escape sequences. Returns an empty string if no attributes are set.
    pub fn to_ansi(&self) -> String {
        if *self == Self::NONE {
            return String::new();
        }
        let mut codes = Vec::new();
        if self.bold {
            codes.push("1");
        }
        if self.dim {
            codes.push("2");
        }
        if self.italic {
            codes.push("3");
        }
        if self.underline {
            codes.push("4");
        }
        if self.blink {
            codes.push("5");
        }
        if self.reverse {
            codes.push("7");
        }
        format!("\x1b[{}m", codes.join(";"))
    }
}

/// One terminal cell.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    pub glyph: char,
    pub fg: Color,
    pub bg: Color,
    pub attrs: CellAttrs,
}

impl Cell {
    pub const EMPTY: Cell = Cell {
        glyph: ' ',
        fg: Color::WHITE,
        bg: Color::BLACK,
        attrs: CellAttrs::NONE,
    };

    pub fn new(glyph: char) -> Self {
        Cell {
            glyph,
            fg: Color::WHITE,
            bg: Color::BLACK,
            attrs: CellAttrs::NONE,
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

    pub fn with_attrs(mut self, attrs: CellAttrs) -> Self {
        self.attrs = attrs;
        self
    }

    /// Treat a space glyph as transparent for the purposes of layered draws.
    pub fn is_transparent(&self) -> bool {
        self.glyph == ' '
    }
}

/// One changed cell between two grids.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct CellChange {
    pub x: u16,
    pub y: u16,
    pub before: Option<Cell>,
    pub after: Option<Cell>,
}

/// A named rendering layer.
///
/// Layers let games separate background maps, entity overlays, and UI elements
/// into independent [`Grid`] buffers that are composited at render time. Each
/// layer has a name for debugging and a draw order (lower numbers draw first).
///
/// # Example
///
/// ```ignore
/// let mut layers = vec![
///     Layer::new("map", 0, Grid::new(80, 24)),
///     Layer::new("entities", 1, Grid::new(80, 24)),
///     Layer::new("ui", 2, Grid::new(80, 24)),
/// ];
///
/// // Draw map tiles into layers[0], entities into layers[1], etc.
/// // Composite at render time:
/// let mut frame = Grid::new(80, 24);
/// layers.sort_by_key(|l| l.order);
/// for layer in &layers {
///     frame.blit(&layer.grid, 0, 0);
/// }
/// ```
#[derive(Clone, Debug)]
pub struct Layer {
    pub name: &'static str,
    pub order: u8,
    pub grid: Grid,
    pub visible: bool,
}

impl Layer {
    pub fn new(name: &'static str, order: u8, grid: Grid) -> Self {
        Self {
            name,
            order,
            grid,
            visible: true,
        }
    }

    /// Composite all visible layers onto a target grid, respecting draw order.
    ///
    /// Layers are sorted by `order` (lowest first), then blitted onto `target`
    /// at (0, 0). Only visible layers are drawn.
    pub fn composite(layers: &[Layer], target: &mut Grid) {
        let mut sorted: Vec<&Layer> = layers.iter().filter(|l| l.visible).collect();
        sorted.sort_by_key(|l| l.order);
        for layer in sorted {
            target.blit(&layer.grid, 0, 0);
        }
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

    /// Return the total number of cells in this rectangle.
    pub fn area(self) -> usize {
        (self.width as usize) * (self.height as usize)
    }

    pub fn contains(self, x: u16, y: u16) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    pub fn intersect(self, other: Rect) -> Rect {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        if x < right && y < bottom {
            Rect::new(x, y, right - x, bottom - y)
        } else {
            Rect::new(0, 0, 0, 0)
        }
    }

    /// Return the smallest rectangle containing both `self` and `other`.
    ///
    /// Useful for computing dirty regions when multiple screen areas change
    /// in the same frame.
    pub fn union(self, other: Rect) -> Rect {
        if self.is_empty() {
            return other;
        }
        if other.is_empty() {
            return self;
        }
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        Rect::new(x, y, right - x, bottom - y)
    }

    /// Offset the rectangle by `(dx, dy)`. Negative offsets are clamped to zero.
    ///
    /// Useful for moving UI elements or adjusting rects for viewport offsets.
    pub fn translate(self, dx: i16, dy: i16) -> Rect {
        let x = (self.x as i16 + dx).max(0) as u16;
        let y = (self.y as i16 + dy).max(0) as u16;
        Rect::new(x, y, self.width, self.height)
    }
}

/// Horizontal text alignment within a bounded width.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Alignment {
    #[default]
    Left,
    Center,
    Right,
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

    /// Return a shared slice of the cells in row `y`. Returns `None` if out of bounds.
    ///
    /// Useful for scanning a single row without iterating the full grid.
    pub fn row(&self, y: u16) -> Option<&[Cell]> {
        if y < self.height {
            let start = (y as usize) * (self.width as usize);
            Some(&self.cells[start..start + self.width as usize])
        } else {
            None
        }
    }

    /// Return a mutable slice of the cells in row `y`. Returns `None` if out of bounds.
    pub fn row_mut(&mut self, y: u16) -> Option<&mut [Cell]> {
        if y < self.height {
            let start = (y as usize) * (self.width as usize);
            let end = start + self.width as usize;
            Some(&mut self.cells[start..end])
        } else {
            None
        }
    }

    /// Return a freshly allocated Vec of cells in column `x`.
    /// Returns `None` if out of bounds.
    ///
    /// Unlike `row`, this cannot return a slice because cells are stored
    /// row-major. Useful for scanning columns for vertical effects,
    /// column-aligned UI elements, or vertical text.
    pub fn col(&self, x: u16) -> Option<Vec<Cell>> {
        if x >= self.width {
            return None;
        }
        let w = self.width as usize;
        let h = self.height as usize;
        let mut result = Vec::with_capacity(h);
        for y in 0..h {
            result.push(self.cells[y * w + x as usize]);
        }
        Some(result)
    }

    /// Fill a single row with the provided cell. Returns `false` if out of bounds.
    pub fn fill_row(&mut self, y: u16, cell: Cell) -> bool {
        if let Some(row) = self.row_mut(y) {
            row.fill(cell);
            true
        } else {
            false
        }
    }

    /// Fill a single column with the provided cell. Returns `false` if out of bounds.
    pub fn fill_col(&mut self, x: u16, cell: Cell) -> bool {
        if x >= self.width {
            return false;
        }
        let w = self.width as usize;
        for y in 0..self.height as usize {
            self.cells[y * w + x as usize] = cell;
        }
        true
    }

    /// Iterate over all cells with their (x, y) positions in row-major order.
    pub fn iter_cells(&self) -> impl Iterator<Item = (u16, u16, &Cell)> + '_ {
        self.cells.iter().enumerate().map(move |(i, cell)| {
            let x = (i % self.width as usize) as u16;
            let y = (i / self.width as usize) as u16;
            (x, y, cell)
        })
    }

    /// Compare two grids cell-by-cell.
    ///
    /// Positions that exist in only one grid report `None` on the missing
    /// side. This makes the diff usable for both same-size frame changes and
    /// resize-aware snapshot tests.
    pub fn diff(&self, other: &Grid) -> Vec<CellChange> {
        let width = self.width.max(other.width);
        let height = self.height.max(other.height);
        let mut changes = Vec::new();

        for y in 0..height {
            for x in 0..width {
                let before = self.get(x, y).copied();
                let after = other.get(x, y).copied();
                if before != after {
                    changes.push(CellChange {
                        x,
                        y,
                        before,
                        after,
                    });
                }
            }
        }

        changes
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

    /// Copy a rectangular viewport out of this grid.
    ///
    /// The returned grid is clipped to the source bounds. This is useful for
    /// terminal games that render a full map but need a camera-sized local
    /// observation for TTY frontends, tests, or agents.
    pub fn viewport(&self, rect: Rect) -> Grid {
        let clipped = rect.intersect(Rect::new(0, 0, self.width, self.height));
        let mut out = Grid::new(clipped.width, clipped.height);
        for y in 0..clipped.height {
            for x in 0..clipped.width {
                if let Some(cell) = self.get(clipped.x + x, clipped.y + y).copied() {
                    out.put(x, y, cell);
                }
            }
        }
        out
    }

    /// Find the first cell matching a predicate, returning its position and reference.
    ///
    /// Scans row-major (left-to-right, top-to-bottom). Useful for locating
    /// specific glyphs, colored cells, or other visual markers without
    /// manual iteration.
    pub fn find_cell<F>(&self, mut predicate: F) -> Option<(u16, u16, &Cell)>
    where
        F: FnMut(&Cell) -> bool,
    {
        for y in 0..self.height {
            let row_start = (y as usize) * (self.width as usize);
            let row_end = row_start + self.width as usize;
            for (offset, cell) in self.cells[row_start..row_end].iter().enumerate() {
                if predicate(cell) {
                    return Some((offset as u16, y, cell));
                }
            }
        }
        None
    }

    /// Swap two cells by position. Returns `false` if either position is out of bounds.
    ///
    /// Useful for animations, drag-and-drop UI, or rearranging grid content.
    pub fn swap_cells(&mut self, x1: u16, y1: u16, x2: u16, y2: u16) -> bool {
        let Some(i1) = self.index(x1, y1) else {
            return false;
        };
        let Some(i2) = self.index(x2, y2) else {
            return false;
        };
        self.cells.swap(i1, i2);
        true
    }

    /// Shift all content up by `n` rows. Rows that scroll off the top are
    /// discarded; new rows at the bottom are filled with `fill`.
    ///
    /// Useful for scrolling text areas, message logs, and terminal output.
    /// A scroll of 0 or ≥ height is equivalent to clearing.
    pub fn scroll_up(&mut self, n: u16, fill: Cell) {
        if n == 0 || self.height == 0 {
            return;
        }
        let w = self.width as usize;
        if n >= self.height {
            self.cells.fill(fill);
            return;
        }
        let offset = (n as usize) * w;
        self.cells.copy_within(offset.., 0);
        let new_start = self.cells.len() - offset;
        self.cells[new_start..].fill(fill);
    }

    /// Shift all content down by `n` rows. Rows that scroll off the bottom are
    /// discarded; new rows at the top are filled with `fill`.
    ///
    /// Useful for inserting new content at the top of a display area.
    pub fn scroll_down(&mut self, n: u16, fill: Cell) {
        if n == 0 || self.height == 0 {
            return;
        }
        let w = self.width as usize;
        if n >= self.height {
            self.cells.fill(fill);
            return;
        }
        let offset = (n as usize) * w;
        let len = self.cells.len();
        self.cells.copy_within(..len - offset, offset);
        self.cells[..offset].fill(fill);
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
            self.put(
                cx,
                y,
                Cell {
                    glyph: ch,
                    fg,
                    bg,
                    attrs: CellAttrs::NONE,
                },
            );
            cx = cx.saturating_add(1);
        }
    }

    /// Write text aligned within a horizontal range `[x, x + width)`.
    ///
    /// `Alignment::Left` starts at `x`, `Alignment::Center` centers the text,
    /// and `Alignment::Right` right-aligns to `x + width - 1`. Text is clipped
    /// if it exceeds the width.
    pub fn write_aligned(
        &mut self,
        x: u16,
        y: u16,
        width: u16,
        text: &str,
        alignment: Alignment,
        fg: Color,
        bg: Color,
    ) {
        if width == 0 || y >= self.height {
            return;
        }
        let text_width = text.chars().count() as u16;
        let start_x = match alignment {
            Alignment::Left => x,
            Alignment::Center => {
                if text_width >= width {
                    x
                } else {
                    x + (width - text_width) / 2
                }
            }
            Alignment::Right => {
                if text_width >= width {
                    x
                } else {
                    x + width - text_width
                }
            }
        };
        self.write_str(start_x, y, text, fg, bg);
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

    /// Draw a border around a rectangle and place a title in the top-left edge.
    pub fn draw_panel(&mut self, rect: Rect, title: &str, border_cell: Cell, title_fg: Color) {
        self.draw_border(rect, border_cell);
        if !title.is_empty() {
            let label = format!(" {} ", title);
            self.write_str(rect.x + 1, rect.y, &label, title_fg, border_cell.bg);
        }
    }

    /// Unicode box-drawing characters for a rounded border.
    pub const BORDER_TL: char = '\u{250C}'; // ┌
    pub const BORDER_TR: char = '\u{2510}'; // ┐
    pub const BORDER_BL: char = '\u{2514}'; // └
    pub const BORDER_BR: char = '\u{2518}'; // ┘
    pub const BORDER_H: char = '\u{2500}'; // ─
    pub const BORDER_V: char = '\u{2502}'; // │

    /// Draw a rounded (Unicode box-drawing) border around a rectangle.
    ///
    /// Uses ┌─┐ / │ │ / └─┘ characters. Clips to grid bounds.
    pub fn draw_border_rounded(&mut self, rect: Rect, fg: Color, bg: Color) {
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
        let left = rect.x;
        let right = x_end - 1;

        // Corners
        self.put(
            left,
            top,
            Cell::new(Self::BORDER_TL).with_fg(fg).with_bg(bg),
        );
        self.put(
            right,
            top,
            Cell::new(Self::BORDER_TR).with_fg(fg).with_bg(bg),
        );
        self.put(
            left,
            bottom,
            Cell::new(Self::BORDER_BL).with_fg(fg).with_bg(bg),
        );
        self.put(
            right,
            bottom,
            Cell::new(Self::BORDER_BR).with_fg(fg).with_bg(bg),
        );

        // Horizontal edges
        for x in (left + 1)..right {
            self.put(x, top, Cell::new(Self::BORDER_H).with_fg(fg).with_bg(bg));
            self.put(x, bottom, Cell::new(Self::BORDER_H).with_fg(fg).with_bg(bg));
        }

        // Vertical edges
        for y in (top + 1)..bottom {
            self.put(left, y, Cell::new(Self::BORDER_V).with_fg(fg).with_bg(bg));
            self.put(right, y, Cell::new(Self::BORDER_V).with_fg(fg).with_bg(bg));
        }
    }

    /// Draw a horizontal line from (x1, y) to (x2, y) inclusive.
    ///
    /// Clips to grid bounds. Returns the number of cells written.
    pub fn draw_hline(&mut self, x1: u16, x2: u16, y: u16, cell: Cell) -> u16 {
        if y >= self.height {
            return 0;
        }
        let start = x1.min(x2);
        let end = x1.max(x2);
        let mut count = 0;
        for x in start..=end {
            if x < self.width {
                self.put(x, y, cell);
                count += 1;
            }
        }
        count
    }

    /// Draw a vertical line from (x, y1) to (x, y2) inclusive.
    ///
    /// Clips to grid bounds. Returns the number of cells written.
    pub fn draw_vline(&mut self, x: u16, y1: u16, y2: u16, cell: Cell) -> u16 {
        if x >= self.width {
            return 0;
        }
        let start = y1.min(y2);
        let end = y1.max(y2);
        let mut count = 0;
        for y in start..=end {
            if y < self.height {
                self.put(x, y, cell);
                count += 1;
            }
        }
        count
    }

    /// Draw a rounded (Unicode box-drawing) panel with a title.
    ///
    /// Combines [`Self::draw_border_rounded`] with title placement in the top
    /// border. The title is centered on the top edge, overwriting the border
    /// line. Clips to grid bounds.
    pub fn draw_rounded_panel(
        &mut self,
        rect: Rect,
        title: &str,
        border_fg: Color,
        title_fg: Color,
        bg: Color,
    ) {
        self.draw_border_rounded(rect, border_fg, bg);

        if !title.is_empty() && rect.width >= 3 {
            let title_len = title.len() as u16;
            let available = rect.width.saturating_sub(2);
            let title_width = title_len.min(available);
            let offset = (available.saturating_sub(title_width) + 1) / 2;
            let x = rect.x + offset;
            if x + title_width <= rect.right() && rect.y < self.height {
                self.write_str(x, rect.y, title, title_fg, bg);
            }
        }
    }

    /// Draw a clipped straight line using integer cell coordinates.
    ///
    /// Returns the number of cells written.
    pub fn draw_line(&mut self, start: (i32, i32), end: (i32, i32), cell: Cell) -> u16 {
        let (mut x0, mut y0) = start;
        let (x1, y1) = end;
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let mut count = 0u16;

        loop {
            if x0 >= 0 && y0 >= 0 {
                if self.put(x0 as u16, y0 as u16, cell) {
                    count += 1;
                }
            }
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
        count
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

    /// Copy a rectangular region from `other` into `self`.
    ///
    /// `src` defines the region to copy from the source grid. `dst_x` and
    /// `dst_y` define the top-left destination in this grid. Transparent cells
    /// (space glyph) are skipped. Out-of-bounds areas are clipped.
    pub fn blit_region(&mut self, other: &Grid, src: Rect, dst_x: i32, dst_y: i32) {
        let clipped = src.intersect(Rect::new(0, 0, other.width, other.height));
        if clipped.is_empty() {
            return;
        }
        for sy in clipped.y..clipped.bottom() {
            for sx in clipped.x..clipped.right() {
                let dx = dst_x + (sx - clipped.x) as i32;
                let dy = dst_y + (sy - clipped.y) as i32;
                if dx < 0 || dy < 0 {
                    continue;
                }
                let (dx, dy) = (dx as u16, dy as u16);
                if dx >= self.width || dy >= self.height {
                    continue;
                }
                let src_cell = other.get(sx, sy).copied().unwrap_or(Cell::EMPTY);
                if src_cell.is_transparent() {
                    continue;
                }
                self.put(dx, dy, src_cell);
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

    /// Render the grid as ANSI-escaped text with foreground and background colors.
    ///
    /// This produces output that any ANSI-compatible terminal can display without
    /// needing crossterm or a live TTY. Useful for debug dumps, log files, and
    /// agent observation over plain text channels.
    pub fn to_ansi_string(&self) -> String {
        let mut out = String::with_capacity(self.cells.len() * 20 + self.height as usize * 10);
        let mut last_fg: Option<Color> = None;
        let mut last_bg: Option<Color> = None;
        let mut last_attrs: Option<CellAttrs> = None;

        for y in 0..self.height {
            if y > 0 {
                out.push('\n');
            }
            for x in 0..self.width {
                let cell = &self.cells[(y as usize) * (self.width as usize) + (x as usize)];
                if last_attrs != Some(cell.attrs) {
                    let attr_str = cell.attrs.to_ansi();
                    if attr_str.is_empty() {
                        out.push_str("\x1b[0m");
                    } else {
                        out.push_str(&attr_str);
                    }
                    last_attrs = Some(cell.attrs);
                }
                if last_fg != Some(cell.fg) {
                    out.push_str(&format!(
                        "\x1b[38;2;{};{};{}m",
                        cell.fg.0, cell.fg.1, cell.fg.2
                    ));
                    last_fg = Some(cell.fg);
                }
                if last_bg != Some(cell.bg) {
                    out.push_str(&format!(
                        "\x1b[48;2;{};{};{}m",
                        cell.bg.0, cell.bg.1, cell.bg.2
                    ));
                    last_bg = Some(cell.bg);
                }
                out.push(cell.glyph);
            }
        }
        out.push_str("\x1b[0m");
        out
    }

    /// Render the grid as an HTML `<pre>` block with inline CSS colors.
    ///
    /// This produces output that can be embedded in any HTML page, making it
    /// useful for web-based debug viewers, CI reports, or sharing terminal
    /// game state over non-terminal channels.
    pub fn to_html_string(&self) -> String {
        let mut out = String::with_capacity(self.cells.len() * 40 + self.height as usize * 20);
        out.push_str("<pre style=\"line-height:1.2;font-family:monospace;\">");

        for y in 0..self.height {
            if y > 0 {
                out.push_str("<br>");
            }
            for x in 0..self.width {
                let cell = &self.cells[(y as usize) * (self.width as usize) + (x as usize)];
                let Color(r, g, b) = cell.fg;
                let Color(br, bg, bb) = cell.bg;
                let escaped = match cell.glyph {
                    '<' => "&lt;".to_owned(),
                    '>' => "&gt;".to_owned(),
                    '&' => "&amp;".to_owned(),
                    '"' => "&quot;".to_owned(),
                    ch => ch.to_string(),
                };
                out.push_str(&format!(
                    "<span style=\"color:rgb({r},{g},{b});background:rgb({br},{bg},{bb});\">{escaped}</span>"
                ));
            }
        }
        out.push_str("</pre>");
        out
    }

    /// Draw a circle outline using the midpoint circle algorithm.
    ///
    /// The circle is centered at `(cx, cy)` with the given `radius`. Cells are
    /// clipped to the grid bounds.
    pub fn draw_circle(&mut self, cx: i32, cy: i32, radius: u16, cell: Cell) {
        if radius == 0 {
            return;
        }
        let mut x = 0i32;
        let mut y = radius as i32;
        let mut d = 1 - y as i32;

        let plot = |grid: &mut Grid, px: i32, py: i32| {
            if px >= 0 && py >= 0 {
                grid.put(px as u16, py as u16, cell);
            }
        };

        while x <= y {
            plot(self, cx + x, cy + y);
            plot(self, cx - x, cy + y);
            plot(self, cx + x, cy - y);
            plot(self, cx - x, cy - y);
            plot(self, cx + y, cy + x);
            plot(self, cx - y, cy + x);
            plot(self, cx + y, cy - x);
            plot(self, cx - y, cy - x);

            if d < 0 {
                d += 2 * x + 3;
            } else {
                d += 2 * (x - y) + 5;
                y -= 1;
            }
            x += 1;
        }
    }

    /// Fill a solid circle using a scanline approach.
    ///
    /// The circle is centered at `(cx, cy)` with the given `radius`. Cells are
    /// clipped to the grid bounds.
    pub fn fill_circle(&mut self, cx: i32, cy: i32, radius: u16, cell: Cell) {
        if radius == 0 {
            return;
        }
        let r = radius as i32;
        let r2 = r * r;

        for dy in -r..=r {
            let dx_max = ((r2 - dy * dy) as f64).sqrt() as i32;
            let py = cy + dy;
            if py < 0 || py >= self.height as i32 {
                continue;
            }
            let x_start = (cx - dx_max).max(0) as u16;
            let x_end = ((cx + dx_max).min(self.width as i32 - 1)) as u16;
            for px in x_start..=x_end {
                self.put(px, py as u16, cell);
            }
        }
    }

    /// Draw a diamond (rhombus) outline using Manhattan distance.
    ///
    /// The diamond is centered at `(cx, cy)` with the given `radius`. Only
    /// cells at exactly `radius` Manhattan distance from the center are drawn.
    /// Useful for AoE indicators, range displays, and selection highlights.
    pub fn draw_diamond(&mut self, cx: i32, cy: i32, radius: u16, cell: Cell) {
        if radius == 0 {
            return;
        }
        let r = radius as i32;
        for dx in -r..=r {
            let dy_pos = r - dx.abs();
            let dy_neg = -dy_pos;
            for dy in [dy_neg, dy_pos] {
                let px = cx + dx;
                let py = cy + dy;
                if px >= 0 && py >= 0 {
                    self.put(px as u16, py as u16, cell);
                }
            }
        }
    }

    /// Fill a solid diamond (rhombus) using Manhattan distance.
    ///
    /// The diamond is centered at `(cx, cy)` with the given `radius`. All
    /// cells within `radius` Manhattan distance from the center are filled.
    /// Clips to grid bounds.
    pub fn fill_diamond(&mut self, cx: i32, cy: i32, radius: u16, cell: Cell) {
        if radius == 0 {
            return;
        }
        let r = radius as i32;
        for dy in -r..=r {
            let dx_max = r - dy.abs();
            let py = cy + dy;
            if py < 0 || py >= self.height as i32 {
                continue;
            }
            let x_start = (cx - dx_max).max(0) as u16;
            let x_end = ((cx + dx_max).min(self.width as i32 - 1)) as u16;
            for px in x_start..=x_end {
                self.put(px, py as u16, cell);
            }
        }
    }

    /// Draw a horizontal progress bar at `(x, y)` with the given `width`.
    ///
    /// `ratio` is the fill proportion (0.0 to 1.0). Filled cells use
    /// `fill_cell` and empty cells use `empty_cell`. Values outside 0.0–1.0
    /// are clamped. Returns the number of filled cells.
    pub fn draw_progress_bar(
        &mut self,
        x: u16,
        y: u16,
        width: u16,
        ratio: f32,
        fill_cell: Cell,
        empty_cell: Cell,
    ) -> u16 {
        if y >= self.height || width == 0 {
            return 0;
        }
        let clamped = ratio.clamp(0.0, 1.0);
        let filled = (clamped * width as f32).round() as u16;
        let x_end = (x + width).min(self.width);
        let mut count = 0u16;
        let mut cx = x;
        while cx < x_end {
            let cell = if cx - x < filled {
                fill_cell
            } else {
                empty_cell
            };
            self.put(cx, y, cell);
            if cx - x < filled {
                count += 1;
            }
            cx += 1;
        }
        count
    }

    /// Apply a transformation function to every cell in the grid.
    ///
    /// Useful for bulk color adjustments, glyph remapping, dimming/brightening
    /// effects, and post-processing the frame before render.
    pub fn transform<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Cell),
    {
        for cell in &mut self.cells {
            f(cell);
        }
    }

    /// Create a new grid by applying a transformation function to each cell.
    ///
    /// The output grid has the same dimensions. Useful for creating modified
    /// copies without mutating the original.
    pub fn map<F>(&self, mut f: F) -> Grid
    where
        F: FnMut(Cell) -> Cell,
    {
        let cells = self.cells.iter().map(|&cell| f(cell)).collect();
        Grid {
            width: self.width,
            height: self.height,
            cells,
        }
    }

    /// Fill a rectangle with a horizontal color gradient from `start` to `end`.
    ///
    /// Each cell's foreground color is interpolated between `start` and `end`
    /// based on its horizontal position within the rect. The glyph is set to
    /// `glyph`. Clips to grid bounds.
    pub fn draw_gradient(&mut self, rect: Rect, start: Color, end: Color, glyph: char) {
        if rect.is_empty() {
            return;
        }
        let x_end = rect.right().min(self.width);
        let y_end = rect.bottom().min(self.height);
        let width = x_end.saturating_sub(rect.x);
        if width == 0 {
            return;
        }

        for y in rect.y..y_end {
            for x in rect.x..x_end {
                let t = (x - rect.x) as f32 / (width - 1) as f32;
                let r = (start.0 as f32 + (end.0 as f32 - start.0 as f32) * t) as u8;
                let g = (start.1 as f32 + (end.1 as f32 - start.1 as f32) * t) as u8;
                let b = (start.2 as f32 + (end.2 as f32 - start.2 as f32) * t) as u8;
                let cell = Cell::new(glyph).with_fg(Color(r, g, b));
                self.put(x, y, cell);
            }
        }
    }

    /// Fill a rectangle with a vertical color gradient from `top` to `bottom`.
    pub fn draw_gradient_v(&mut self, rect: Rect, top: Color, bottom: Color, glyph: char) {
        if rect.is_empty() {
            return;
        }
        let x_end = rect.right().min(self.width);
        let y_end = rect.bottom().min(self.height);
        let height = y_end.saturating_sub(rect.y);
        if height == 0 {
            return;
        }

        for y in rect.y..y_end {
            let t = (y - rect.y) as f32 / (height - 1) as f32;
            let r = (top.0 as f32 + (bottom.0 as f32 - top.0 as f32) * t) as u8;
            let g = (top.1 as f32 + (bottom.1 as f32 - top.1 as f32) * t) as u8;
            let b = (top.2 as f32 + (bottom.2 as f32 - top.2 as f32) * t) as u8;
            for x in rect.x..x_end {
                let cell = Cell::new(glyph).with_fg(Color(r, g, b));
                self.put(x, y, cell);
            }
        }
    }
}

/// Wrap text into lines that fit within `width` terminal cells.
///
/// Breaks on whitespace when possible, falling back to hard wrapping at
/// `width` for long words. Preserves existing newlines. Useful for rendering
/// message boxes, help screens, and dialogue in terminal games.
pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut remaining = paragraph;
        while !remaining.is_empty() {
            if remaining.len() <= width {
                lines.push(remaining.to_owned());
                break;
            }
            let break_point = remaining
                .char_indices()
                .take(width)
                .last()
                .map(|(i, c)| i + c.len_utf8())
                .unwrap_or(0);

            let last_space = remaining[..break_point].rfind(' ').map(|i| i + 1);

            if let Some(space_pos) = last_space {
                lines.push(remaining[..space_pos].trim_end().to_owned());
                remaining = &remaining[space_pos..];
            } else {
                lines.push(remaining[..break_point].to_owned());
                remaining = &remaining[break_point..];
            }
            remaining = remaining.trim_start();
        }
    }
    lines
}

/// Write wrapped text into a [`Grid`] starting at `(x, y)`.
///
/// Each line is written left-to-right with the given colors. Lines that
/// exceed the grid height are silently dropped.
pub fn write_wrapped_text(
    grid: &mut Grid,
    x: u16,
    y: u16,
    text: &str,
    width: u16,
    fg: Color,
    bg: Color,
) {
    let lines = wrap_text(text, width as usize);
    for (i, line) in lines.iter().enumerate() {
        let ly = y.saturating_add(i as u16);
        if ly >= grid.height() {
            break;
        }
        grid.write_str(x, ly, line, fg, bg);
    }
}

/// A named collection of colors for consistent theming.
///
/// Games can define palettes for different visual themes (dark, light, dungeon,
/// space, etc.) and reference colors by semantic name rather than raw RGB values.
/// This makes it easy to swap themes or let players customize the look.
///
/// # Example
///
/// ```ignore
/// let palette = ColorPalette::dark_dungeon();
/// let player_cell = Cell::new('@').with_fg(palette.player);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColorPalette {
    pub name: &'static str,
    pub background: Color,
    pub foreground: Color,
    pub player: Color,
    pub wall: Color,
    pub floor: Color,
    pub hazard: Color,
    pub item: Color,
    pub goal: Color,
    pub ui_border: Color,
    pub ui_title: Color,
    pub ui_text: Color,
    pub ui_highlight: Color,
    pub ui_dim: Color,
}

impl ColorPalette {
    /// A dark dungeon theme with muted earth tones.
    pub fn dark_dungeon() -> Self {
        Self {
            name: "dark_dungeon",
            background: Color(15, 15, 20),
            foreground: Color(200, 200, 200),
            player: Color(100, 220, 100),
            wall: Color(80, 80, 90),
            floor: Color(50, 50, 55),
            hazard: Color(220, 80, 80),
            item: Color(220, 200, 80),
            goal: Color(80, 180, 220),
            ui_border: Color(100, 100, 110),
            ui_title: Color(220, 200, 80),
            ui_text: Color(200, 200, 200),
            ui_highlight: Color(100, 220, 100),
            ui_dim: Color(100, 100, 100),
        }
    }

    /// A light theme suitable for bright terminals.
    pub fn light_classic() -> Self {
        Self {
            name: "light_classic",
            background: Color(240, 240, 240),
            foreground: Color(30, 30, 30),
            player: Color(0, 120, 0),
            wall: Color(120, 120, 120),
            floor: Color(220, 220, 220),
            hazard: Color(180, 30, 30),
            item: Color(180, 140, 0),
            goal: Color(0, 100, 180),
            ui_border: Color(150, 150, 150),
            ui_title: Color(180, 140, 0),
            ui_text: Color(30, 30, 30),
            ui_highlight: Color(0, 120, 0),
            ui_dim: Color(150, 150, 150),
        }
    }

    /// A high-contrast amber-on-black theme reminiscent of vintage terminals.
    pub fn amber_terminal() -> Self {
        Self {
            name: "amber_terminal",
            background: Color(10, 8, 5),
            foreground: Color(255, 180, 50),
            player: Color(255, 220, 100),
            wall: Color(120, 90, 30),
            floor: Color(40, 30, 15),
            hazard: Color(255, 80, 50),
            item: Color(255, 220, 100),
            goal: Color(200, 255, 150),
            ui_border: Color(180, 140, 40),
            ui_title: Color(255, 220, 100),
            ui_text: Color(255, 180, 50),
            ui_highlight: Color(255, 255, 150),
            ui_dim: Color(100, 80, 30),
        }
    }

    /// A cyberpunk neon theme with vivid colors on dark backgrounds.
    pub fn cyberpunk() -> Self {
        Self {
            name: "cyberpunk",
            background: Color(10, 5, 20),
            foreground: Color(200, 200, 255),
            player: Color(0, 255, 200),
            wall: Color(60, 30, 80),
            floor: Color(20, 15, 35),
            hazard: Color(255, 50, 100),
            item: Color(255, 255, 0),
            goal: Color(100, 100, 255),
            ui_border: Color(80, 50, 120),
            ui_title: Color(0, 255, 200),
            ui_text: Color(200, 200, 255),
            ui_highlight: Color(0, 255, 200),
            ui_dim: Color(80, 60, 100),
        }
    }

    /// Create a cell with the floor color as background.
    pub fn floor_cell(&self, glyph: char) -> Cell {
        Cell::new(glyph)
            .with_fg(self.foreground)
            .with_bg(self.floor)
    }

    /// Create a cell with the wall color.
    pub fn wall_cell(&self, glyph: char) -> Cell {
        Cell::new(glyph).with_fg(self.wall).with_bg(self.background)
    }

    /// Create a cell for the player character.
    pub fn player_cell(&self, glyph: char) -> Cell {
        Cell::new(glyph).with_fg(self.player).with_bg(self.floor)
    }

    /// Create a cell for a hazard tile.
    pub fn hazard_cell(&self, glyph: char) -> Cell {
        Cell::new(glyph).with_fg(self.hazard).with_bg(self.floor)
    }

    /// Create a cell for an item tile.
    pub fn item_cell(&self, glyph: char) -> Cell {
        Cell::new(glyph).with_fg(self.item).with_bg(self.floor)
    }

    /// Create a cell for the goal tile.
    pub fn goal_cell(&self, glyph: char) -> Cell {
        Cell::new(glyph).with_fg(self.goal).with_bg(self.floor)
    }
}

impl Default for ColorPalette {
    fn default() -> Self {
        Self::dark_dungeon()
    }
}

/// A managed collection of named rendering layers.
///
/// Provides convenient add/find/remove operations on top of the raw `Vec<Layer>`
/// pattern. Layers are kept sorted by draw order for efficient compositing.
///
/// # Example
///
/// ```ignore
/// let mut layers = Layers::new();
/// layers.add(Layer::new("map", 0, Grid::new(80, 24)));
/// layers.add(Layer::new("entities", 10, Grid::new(80, 24)));
/// layers.add(Layer::new("ui", 20, Grid::new(80, 24)));
///
/// let map = layers.get_mut("map").unwrap();
/// // draw into map...
///
/// let mut frame = Grid::new(80, 24);
/// layers.composite(&mut frame);
/// ```
#[derive(Clone, Debug)]
pub struct Layers {
    layers: Vec<Layer>,
}

impl Layers {
    pub fn new() -> Self {
        Self { layers: Vec::new() }
    }

    /// Add a layer. If a layer with the same name already exists, it is replaced.
    pub fn add(&mut self, layer: Layer) {
        if let Some(pos) = self.layers.iter().position(|l| l.name == layer.name) {
            self.layers[pos] = layer;
        } else {
            self.layers.push(layer);
        }
        self.layers.sort_by_key(|l| l.order);
    }

    /// Get a reference to a layer by name.
    pub fn get(&self, name: &str) -> Option<&Layer> {
        self.layers.iter().find(|l| l.name == name)
    }

    /// Get a mutable reference to a layer by name.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.name == name)
    }

    /// Remove a layer by name. Returns `true` if found and removed.
    pub fn remove(&mut self, name: &str) -> bool {
        if let Some(pos) = self.layers.iter().position(|l| l.name == name) {
            self.layers.remove(pos);
            true
        } else {
            false
        }
    }

    /// Composite all visible layers onto a target grid.
    pub fn composite(&self, target: &mut Grid) {
        Layer::composite(&self.layers, target);
    }

    /// Return the number of layers.
    pub fn len(&self) -> usize {
        self.layers.len()
    }

    /// Check if there are no layers.
    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    /// Iterate over all layers in draw order.
    pub fn iter(&self) -> std::slice::Iter<'_, Layer> {
        self.layers.iter()
    }
}

impl Default for Layers {
    fn default() -> Self {
        Self::new()
    }
}

/// A single animation frame: a [`Grid`] with an associated display duration.
///
/// Frames are used in [`Sprite`] and [`SpriteSheet`] to build animated
/// terminal graphics. The duration is in arbitrary ticks — the game loop
/// decides how many ticks each frame should display.
#[derive(Clone, Debug)]
pub struct Frame {
    pub grid: Grid,
    pub duration: u32,
}

impl Frame {
    pub fn new(grid: Grid, duration: u32) -> Self {
        Self { grid, duration }
    }
}

/// A named sequence of [`Frame`]s for terminal animation.
///
/// Sprites track their own playback state (current frame, elapsed ticks) so
/// games can advance them each tick and render the current frame. Sprites
/// loop by default and can be paused or reset.
///
/// # Example
///
/// ```ignore
/// let mut sprite = Sprite::new("walk", vec![
///     Frame::new(grid_frame_1, 2),
///     Frame::new(grid_frame_2, 2),
///     Frame::new(grid_frame_3, 2),
/// ]);
///
/// // Each game tick:
/// sprite.tick();
/// let current_grid = sprite.current_frame();
/// ```
#[derive(Clone, Debug)]
pub struct Sprite {
    pub name: String,
    frames: Vec<Frame>,
    current: usize,
    elapsed: u32,
    paused: bool,
}

impl Sprite {
    pub fn new<S: Into<String>>(name: S, frames: Vec<Frame>) -> Self {
        Self {
            name: name.into(),
            frames,
            current: 0,
            elapsed: 0,
            paused: false,
        }
    }

    /// Advance the sprite by one tick. Returns `true` if the frame changed.
    pub fn tick(&mut self) -> bool {
        if self.paused || self.frames.is_empty() {
            return false;
        }
        self.elapsed += 1;
        if self.elapsed >= self.frames[self.current].duration {
            self.elapsed = 0;
            self.current = (self.current + 1) % self.frames.len();
            true
        } else {
            false
        }
    }

    /// Get the current frame's grid.
    pub fn current_frame(&self) -> &Grid {
        &self.frames[self.current].grid
    }

    /// Get the current frame index.
    pub fn current_index(&self) -> usize {
        self.current
    }

    /// Get the total number of frames.
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Pause or resume the sprite.
    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    /// Check if the sprite is paused.
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Reset to the first frame.
    pub fn reset(&mut self) {
        self.current = 0;
        self.elapsed = 0;
    }

    /// Jump to a specific frame. Clamped to valid range.
    pub fn set_frame(&mut self, index: usize) {
        if !self.frames.is_empty() {
            self.current = index.min(self.frames.len() - 1);
            self.elapsed = 0;
        }
    }
}

/// A collection of named [`Sprite`]s indexed by name.
///
/// Useful for storing all animation states for an entity (idle, walk, attack,
/// death) in one place. Games can look up sprites by name and swap the active
/// animation.
///
/// # Example
///
/// ```ignore
/// let mut sheet = SpriteSheet::new();
/// sheet.add("idle", Sprite::new("idle", idle_frames));
/// sheet.add("walk", Sprite::new("walk", walk_frames));
///
/// let sprite = sheet.get_mut("walk").unwrap();
/// sprite.tick();
/// grid.blit(sprite.current_frame(), 0, 0);
/// ```
#[derive(Clone, Debug)]
pub struct SpriteSheet {
    sprites: Vec<Sprite>,
}

impl SpriteSheet {
    pub fn new() -> Self {
        Self {
            sprites: Vec::new(),
        }
    }

    /// Add a sprite. If a sprite with the same name exists, it is replaced.
    pub fn add(&mut self, sprite: Sprite) {
        if let Some(pos) = self.sprites.iter().position(|s| s.name == sprite.name) {
            self.sprites[pos] = sprite;
        } else {
            self.sprites.push(sprite);
        }
    }

    /// Get a sprite by name.
    pub fn get(&self, name: &str) -> Option<&Sprite> {
        self.sprites.iter().find(|s| s.name == name)
    }

    /// Get a mutable sprite by name.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Sprite> {
        self.sprites.iter_mut().find(|s| s.name == name)
    }

    /// Remove a sprite by name.
    pub fn remove(&mut self, name: &str) -> bool {
        if let Some(pos) = self.sprites.iter().position(|s| s.name == name) {
            self.sprites.remove(pos);
            true
        } else {
            false
        }
    }

    /// Advance all sprites by one tick.
    pub fn tick_all(&mut self) {
        for sprite in &mut self.sprites {
            sprite.tick();
        }
    }

    /// Reset all sprites to their first frame.
    pub fn reset_all(&mut self) {
        for sprite in &mut self.sprites {
            sprite.reset();
        }
    }

    /// Number of sprites in the sheet.
    pub fn len(&self) -> usize {
        self.sprites.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sprites.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Sprite> {
        self.sprites.iter()
    }
}

impl Default for SpriteSheet {
    fn default() -> Self {
        Self::new()
    }
}

/// Draw a sparkline (mini bar chart) at `(x, y)` using Unicode block characters.
///
/// Renders `values` as a compact horizontal sparkline within `width` cells.
/// Values are normalized to the range and mapped to Unicode block characters
/// from empty to full: `▁▂▃▄▅▆▇█`.
///
/// Returns the number of cells written. Useful for inline stats in terminal
/// game UIs (health history, damage trends, turn counts).
pub fn draw_sparkline(
    grid: &mut Grid,
    x: u16,
    y: u16,
    width: u16,
    values: &[f32],
    fg: Color,
    bg: Color,
) -> u16 {
    if y >= grid.height() || width == 0 || values.is_empty() {
        return 0;
    }

    const BLOCKS: [char; 9] = [
        ' ', '\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}', '\u{2587}',
        '\u{2588}',
    ];

    let min = values.iter().cloned().fold(f32::INFINITY, f32::min);
    let max = values.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let range = max - min;

    let x_end = (x + width).min(grid.width());
    let mut count = 0u16;
    let mut cx = x;

    while cx < x_end {
        let idx = if values.len() == 1 {
            0
        } else {
            ((cx - x) as f64 * (values.len() - 1) as f64 / (width - 1).max(1) as f64).round()
                as usize
        };
        let idx = idx.min(values.len() - 1);
        let val = values[idx];
        let level = if range > 0.0 {
            ((val - min) / range * 8.0).round() as usize
        } else {
            8
        };
        let level = level.min(8);
        let cell = Cell::new(BLOCKS[level]).with_fg(fg).with_bg(bg);
        grid.put(cx, y, cell);
        count += 1;
        cx += 1;
    }
    count
}

impl Grid {
    /// Resize the grid to new dimensions.
    ///
    /// Existing cells are preserved where they fit within the new bounds.
    /// New cells are filled with `Cell::EMPTY`. If the grid shrinks, cells
    /// outside the new bounds are lost.
    ///
    /// This is useful for responsive layouts that need to adapt to terminal
    /// resize events.
    pub fn resize(&mut self, new_width: u16, new_height: u16) {
        if new_width == self.width && new_height == self.height {
            return;
        }
        let mut new_cells = vec![Cell::EMPTY; (new_width as usize) * (new_height as usize)];

        let copy_width = new_width.min(self.width) as usize;
        let copy_height = new_height.min(self.height) as usize;

        for y in 0..copy_height {
            let src_row = y * self.width as usize;
            let dst_row = y * new_width as usize;
            new_cells[dst_row..dst_row + copy_width]
                .copy_from_slice(&self.cells[src_row..src_row + copy_width]);
        }

        self.width = new_width;
        self.height = new_height;
        self.cells = new_cells;
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
    fn draw_panel_renders_border_and_title() {
        let mut grid = Grid::new(10, 3);
        grid.draw_panel(Rect::new(0, 0, 10, 3), "LOG", Cell::new('#'), Color::WHITE);
        // Should look like:
        // # LOG ####
        // #        #
        // ##########
        let s = grid.to_plain_string();
        let lines: Vec<&str> = s.lines().collect();
        assert!(lines[0].starts_with("# LOG #"));
        assert!(lines[1].starts_with("#"));
        assert!(lines[2].starts_with("##########"));
    }

    #[test]
    fn draw_line_clips_to_grid() {
        let mut grid = Grid::new(5, 3);
        grid.draw_line((-2, 1), (4, 1), Cell::new('-'));
        assert_eq!(grid.to_plain_string(), "     \n-----\n     ");

        let mut diagonal = Grid::new(4, 4);
        diagonal.draw_line((0, 0), (3, 3), Cell::new('\\'));
        assert_eq!(diagonal.to_plain_string(), "\\   \n \\  \n  \\ \n   \\");
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

    #[test]
    fn diff_reports_changed_and_resized_cells() {
        let mut before = Grid::new(2, 1);
        before.write_str(0, 0, "ab", Color::WHITE, Color::BLACK);
        let mut after = Grid::new(3, 1);
        after.write_str(0, 0, "ac!", Color::WHITE, Color::BLACK);

        assert_eq!(
            before.diff(&after),
            vec![
                CellChange {
                    x: 1,
                    y: 0,
                    before: Some(Cell::new('b')),
                    after: Some(Cell::new('c')),
                },
                CellChange {
                    x: 2,
                    y: 0,
                    before: None,
                    after: Some(Cell::new('!')),
                },
            ]
        );
    }

    #[test]
    fn viewport_copies_and_clips_rectangles() {
        let mut grid = Grid::new(4, 3);
        grid.write_str(0, 0, "abcd", Color::WHITE, Color::BLACK);
        grid.write_str(0, 1, "efgh", Color::WHITE, Color::BLACK);
        grid.write_str(0, 2, "ijkl", Color::WHITE, Color::BLACK);

        assert_eq!(
            grid.viewport(Rect::new(1, 1, 2, 2)).to_plain_string(),
            "fg\njk"
        );
        assert_eq!(grid.viewport(Rect::new(2, 2, 5, 5)).to_plain_string(), "kl");
    }

    #[test]
    fn find_cell_locates_matching_glyph() {
        let mut grid = Grid::new(4, 3);
        grid.write_str(0, 0, "....", Color::WHITE, Color::BLACK);
        grid.write_str(0, 1, ".@..", Color::WHITE, Color::BLACK);
        grid.write_str(0, 2, "....", Color::WHITE, Color::BLACK);

        let found = grid.find_cell(|c| c.glyph == '@');
        assert!(found.is_some());
        let (x, y, cell) = found.unwrap();
        assert_eq!(x, 1);
        assert_eq!(y, 1);
        assert_eq!(cell.glyph, '@');
    }

    #[test]
    fn find_cell_returns_none_when_no_match() {
        let grid = Grid::new(2, 2);
        let found = grid.find_cell(|c| c.glyph == 'X');
        assert!(found.is_none());
    }

    #[test]
    fn find_cell_returns_first_match_row_major() {
        let mut grid = Grid::new(3, 2);
        grid.write_str(0, 0, "X.X", Color::WHITE, Color::BLACK);
        grid.write_str(0, 1, ".X.", Color::WHITE, Color::BLACK);

        let found = grid.find_cell(|c| c.glyph == 'X');
        assert!(found.is_some());
        let (x, y, _) = found.unwrap();
        assert_eq!(x, 0);
        assert_eq!(y, 0);
    }

    #[test]
    fn ansi_string_contains_escape_codes_for_colors() {
        let mut grid = Grid::new(3, 1);
        grid.put(0, 0, Cell::new('R').with_fg(Color::RED));
        grid.put(1, 0, Cell::new('G').with_fg(Color::GREEN));
        grid.put(2, 0, Cell::new('B').with_fg(Color::BLUE));

        let ansi = grid.to_ansi_string();
        assert!(ansi.contains("\x1b[38;2;220;60;60m"), "red fg");
        assert!(ansi.contains("\x1b[38;2;80;200;120m"), "green fg");
        assert!(ansi.contains("\x1b[38;2;80;130;220m"), "blue fg");
        assert!(ansi.contains("\x1b[48;2;0;0;0m"), "black bg");
        assert!(ansi.ends_with("\x1b[0m"), "reset at end");
        assert!(ansi.contains('R'));
        assert!(ansi.contains('G'));
        assert!(ansi.contains('B'));
    }

    #[test]
    fn ansi_string_reuses_color_when_consecutive_cells_match() {
        let mut grid = Grid::new(4, 1);
        grid.put(0, 0, Cell::new('a').with_fg(Color::WHITE));
        grid.put(1, 0, Cell::new('b').with_fg(Color::WHITE));
        grid.put(2, 0, Cell::new('c').with_fg(Color::RED));
        grid.put(3, 0, Cell::new('d').with_fg(Color::WHITE));

        let ansi = grid.to_ansi_string();
        let fg_red_count = ansi.matches("\x1b[38;2;220;60;60m").count();
        let fg_white_count = ansi.matches("\x1b[38;2;230;230;230m").count();
        assert_eq!(fg_red_count, 1);
        assert_eq!(fg_white_count, 2);
    }

    #[test]
    fn draw_circle_outlines_a_circle() {
        let mut grid = Grid::new(7, 7);
        grid.draw_circle(3, 3, 3, Cell::new('*'));
        let s = grid.to_plain_string();

        let lines: Vec<&str> = s.lines().collect();
        assert_eq!(lines.len(), 7);
        // Circle should have some stars on multiple rows.
        let star_count: usize = s.chars().filter(|&c| c == '*').count();
        assert!(
            star_count >= 8,
            "circle should have at least 8 outline cells"
        );
        // Center should be empty (outline only).
        assert_eq!(lines[3].chars().nth(3), Some(' '));
    }

    #[test]
    fn fill_circle_fills_interior() {
        let mut grid = Grid::new(7, 7);
        grid.fill_circle(3, 3, 2, Cell::new('#'));
        let s = grid.to_plain_string();

        // Filled circle should have more cells than outline-only.
        let fill_count: usize = s.chars().filter(|&c| c == '#').count();
        assert!(
            fill_count >= 9,
            "filled circle should have at least 9 cells"
        );
        // Center should be filled.
        let lines: Vec<&str> = s.lines().collect();
        assert_eq!(lines[3].chars().nth(3), Some('#'));
    }

    #[test]
    fn draw_circle_is_clipped_to_grid() {
        let mut grid = Grid::new(3, 3);
        grid.draw_circle(0, 0, 5, Cell::new('o'));
        // Should not panic and should only write within bounds.
        assert!(grid.to_plain_string().len() <= 3 * 3 + 2);
    }

    #[test]
    fn wrap_text_breaks_on_whitespace() {
        let lines = wrap_text("hello world this is a test", 10);
        assert_eq!(lines, vec!["hello", "world", "this is a", "test"]);
    }

    #[test]
    fn wrap_text_hard_wraps_long_words() {
        let lines = wrap_text("superlongword short", 8);
        assert_eq!(lines, vec!["superlon", "gword", "short"]);
    }

    #[test]
    fn wrap_text_preserves_newlines() {
        let lines = wrap_text("line one\nline two", 20);
        assert_eq!(lines, vec!["line one", "line two"]);
    }

    #[test]
    fn wrap_text_returns_empty_for_zero_width() {
        let lines = wrap_text("hello", 0);
        assert!(lines.is_empty());
    }

    #[test]
    fn write_wrapped_text_fits_in_grid() {
        let mut grid = Grid::new(12, 4);
        write_wrapped_text(
            &mut grid,
            1,
            0,
            "hello world this is a test of wrapping",
            10,
            Color::WHITE,
            Color::BLACK,
        );
        let s = grid.to_plain_string();
        let lines: Vec<&str> = s.lines().collect();
        assert!(lines[0].contains("hello"));
        assert!(lines[1].contains("world"));
    }

    #[test]
    fn draw_border_rounded_uses_box_drawing_chars() {
        let mut grid = Grid::new(6, 5);
        grid.draw_border_rounded(Rect::new(1, 1, 4, 3), Color::WHITE, Color::BLACK);
        let s = grid.to_plain_string();
        let lines: Vec<&str> = s.lines().collect();

        assert_eq!(lines.len(), 5);
        // Top row of border has corners and horizontal edges.
        assert!(lines[1].contains(Grid::BORDER_TL));
        assert!(lines[1].contains(Grid::BORDER_TR));
        assert!(lines[1].contains(Grid::BORDER_H));
        // Middle row has vertical edges on the sides.
        assert!(lines[2].contains(Grid::BORDER_V));
        // Bottom row of border has corners and horizontal edges.
        assert!(lines[3].contains(Grid::BORDER_BL));
        assert!(lines[3].contains(Grid::BORDER_BR));
        assert!(lines[3].contains(Grid::BORDER_H));
    }

    #[test]
    fn draw_border_rounded_clips_to_grid() {
        let mut grid = Grid::new(3, 3);
        // Border extends beyond grid; should not panic.
        grid.draw_border_rounded(Rect::new(0, 0, 10, 10), Color::WHITE, Color::BLACK);
        assert!(grid.to_plain_string().contains(Grid::BORDER_TL));
    }

    #[test]
    fn draw_hline_writes_horizontal_cells() {
        let mut grid = Grid::new(8, 3);
        let count = grid.draw_hline(1, 6, 1, Cell::new('-'));
        assert_eq!(count, 6);
        let s = grid.to_plain_string();
        let lines: Vec<&str> = s.lines().collect();
        assert_eq!(lines[1], " ------ ");
    }

    #[test]
    fn draw_hline_reverses_and_clips() {
        let mut grid = Grid::new(5, 2);
        let count = grid.draw_hline(4, 0, 0, Cell::new('='));
        assert_eq!(count, 5);
        assert_eq!(grid.to_plain_string().lines().next(), Some("====="));

        let count2 = grid.draw_hline(0, 10, 1, Cell::new('='));
        assert_eq!(count2, 5);
    }

    #[test]
    fn draw_vline_writes_vertical_cells() {
        let mut grid = Grid::new(3, 5);
        let count = grid.draw_vline(1, 0, 4, Cell::new('|'));
        assert_eq!(count, 5);
        let s = grid.to_plain_string();
        let lines: Vec<&str> = s.lines().collect();
        for line in &lines {
            assert_eq!(line.chars().nth(1), Some('|'));
        }
    }

    #[test]
    fn draw_vline_clips_to_grid() {
        let mut grid = Grid::new(3, 3);
        let count = grid.draw_vline(0, 0, 10, Cell::new('|'));
        assert_eq!(count, 3);
    }

    #[test]
    fn draw_rounded_panel_draws_border_and_centered_title() {
        let mut grid = Grid::new(12, 4);
        grid.draw_rounded_panel(
            Rect::new(1, 1, 10, 2),
            "STATUS",
            Color::WHITE,
            Color::YELLOW,
            Color::BLACK,
        );
        let s = grid.to_plain_string();
        let lines: Vec<&str> = s.lines().collect();

        assert_eq!(lines.len(), 4);
        // Title should be centered on the top border.
        assert!(lines[1].contains("STATUS"));
        // Corners should be box-drawing characters.
        assert!(lines[1].contains(Grid::BORDER_TL));
        assert!(lines[1].contains(Grid::BORDER_TR));
        assert!(lines[2].contains(Grid::BORDER_BL));
        assert!(lines[2].contains(Grid::BORDER_BR));
    }

    #[test]
    fn draw_rounded_panel_handles_short_title() {
        let mut grid = Grid::new(8, 4);
        grid.draw_rounded_panel(
            Rect::new(1, 1, 6, 2),
            "X",
            Color::WHITE,
            Color::YELLOW,
            Color::BLACK,
        );
        let s = grid.to_plain_string();
        let lines: Vec<&str> = s.lines().collect();

        // Single char title should be roughly centered.
        assert!(lines[1].contains('X'));
    }

    #[test]
    fn draw_rounded_panel_clips_to_grid() {
        let mut grid = Grid::new(5, 3);
        grid.draw_rounded_panel(
            Rect::new(0, 0, 10, 10),
            "BIG PANEL",
            Color::WHITE,
            Color::YELLOW,
            Color::BLACK,
        );
        // Should not panic and should have some border chars (bottom corners survive).
        let s = grid.to_plain_string();
        assert!(s.contains(Grid::BORDER_BL) || s.contains(Grid::BORDER_BR));
    }

    #[test]
    fn draw_diamond_outlines_a_rhombus() {
        let mut grid = Grid::new(7, 7);
        grid.draw_diamond(3, 3, 3, Cell::new('*'));
        let s = grid.to_plain_string();

        let lines: Vec<&str> = s.lines().collect();
        assert_eq!(lines.len(), 7);
        let star_count: usize = s.chars().filter(|&c| c == '*').count();
        // Diamond outline at radius 3 should have 12 cells (4 * radius).
        assert_eq!(star_count, 12);
        // Center should be empty (outline only).
        assert_eq!(lines[3].chars().nth(3), Some(' '));
        // Top and bottom points should be present.
        assert_eq!(lines[0].chars().nth(3), Some('*'));
        assert_eq!(lines[6].chars().nth(3), Some('*'));
    }

    #[test]
    fn fill_diamond_fills_interior() {
        let mut grid = Grid::new(7, 7);
        grid.fill_diamond(3, 3, 2, Cell::new('#'));
        let s = grid.to_plain_string();

        let fill_count: usize = s.chars().filter(|&c| c == '#').count();
        // Filled diamond at radius 2: 1 + 2 + 3 + 2 + 1 = 9 cells (2r(r+1)+1).
        assert_eq!(fill_count, 13);
        // Center should be filled.
        let lines: Vec<&str> = s.lines().collect();
        assert_eq!(lines[3].chars().nth(3), Some('#'));
    }

    #[test]
    fn draw_diamond_is_clipped_to_grid() {
        let mut grid = Grid::new(3, 3);
        grid.draw_diamond(0, 0, 5, Cell::new('o'));
        assert!(grid.to_plain_string().len() <= 3 * 3 + 2);
    }

    #[test]
    fn fill_diamond_is_clipped_to_grid() {
        let mut grid = Grid::new(3, 3);
        grid.fill_diamond(0, 0, 5, Cell::new('o'));
        assert!(grid.to_plain_string().len() <= 3 * 3 + 2);
    }

    #[test]
    fn draw_progress_bar_full_fill() {
        let mut grid = Grid::new(10, 1);
        let count = grid.draw_progress_bar(0, 0, 10, 1.0, Cell::new('#'), Cell::new('.'));
        assert_eq!(count, 10);
        assert_eq!(grid.to_plain_string(), "##########");
    }

    #[test]
    fn draw_progress_bar_empty() {
        let mut grid = Grid::new(10, 1);
        let count = grid.draw_progress_bar(0, 0, 10, 0.0, Cell::new('#'), Cell::new('.'));
        assert_eq!(count, 0);
        assert_eq!(grid.to_plain_string(), "..........");
    }

    #[test]
    fn draw_progress_bar_half_fill() {
        let mut grid = Grid::new(10, 1);
        let count = grid.draw_progress_bar(0, 0, 10, 0.5, Cell::new('#'), Cell::new('.'));
        assert_eq!(count, 5);
        assert_eq!(grid.to_plain_string(), "#####.....");
    }

    #[test]
    fn draw_progress_bar_clamps_ratio() {
        let mut grid = Grid::new(5, 1);
        grid.draw_progress_bar(0, 0, 5, 1.5, Cell::new('#'), Cell::new('.'));
        assert_eq!(grid.to_plain_string(), "#####");

        let mut grid2 = Grid::new(5, 1);
        grid2.draw_progress_bar(0, 0, 5, -0.5, Cell::new('#'), Cell::new('.'));
        assert_eq!(grid2.to_plain_string(), ".....");
    }

    #[test]
    fn draw_progress_bar_clips_to_grid() {
        let mut grid = Grid::new(5, 2);
        let count = grid.draw_progress_bar(3, 0, 10, 0.5, Cell::new('#'), Cell::new('.'));
        // Only 2 cells fit (columns 3,4), 50% of 10 = 5 filled, but only 2 visible.
        assert_eq!(count, 2);
        assert_eq!(grid.get(3, 0).unwrap().glyph, '#');
        assert_eq!(grid.get(4, 0).unwrap().glyph, '#');
    }

    #[test]
    fn draw_progress_bar_out_of_bounds_y() {
        let mut grid = Grid::new(5, 2);
        let count = grid.draw_progress_bar(0, 5, 5, 1.0, Cell::new('#'), Cell::new('.'));
        assert_eq!(count, 0);
    }

    #[test]
    fn draw_progress_bar_with_offset() {
        let mut grid = Grid::new(8, 1);
        let count = grid.draw_progress_bar(2, 0, 4, 0.75, Cell::new('#'), Cell::new('.'));
        assert_eq!(count, 3);
        // Positions 0-1 are default empty (space), bar at 2-5, positions 6-7 are default.
        assert_eq!(grid.get(2, 0).unwrap().glyph, '#');
        assert_eq!(grid.get(3, 0).unwrap().glyph, '#');
        assert_eq!(grid.get(4, 0).unwrap().glyph, '#');
        assert_eq!(grid.get(5, 0).unwrap().glyph, '.');
    }

    #[test]
    fn to_html_string_contains_pre_tag_and_spans() {
        let mut grid = Grid::new(3, 1);
        grid.put(0, 0, Cell::new('R').with_fg(Color::RED));
        grid.put(1, 0, Cell::new('G').with_fg(Color::GREEN));
        grid.put(2, 0, Cell::new('B').with_fg(Color::BLUE));

        let html = grid.to_html_string();
        assert!(html.starts_with("<pre"));
        assert!(html.ends_with("</pre>"));
        assert!(html.contains("R"));
        assert!(html.contains("G"));
        assert!(html.contains("B"));
        assert!(html.contains("color:rgb(220,60,60)"));
        assert!(html.contains("color:rgb(80,200,120)"));
        assert!(html.contains("color:rgb(80,130,220)"));
    }

    #[test]
    fn to_html_string_escapes_special_chars() {
        let mut grid = Grid::new(4, 1);
        grid.write_str(0, 0, "<>&\"", Color::WHITE, Color::BLACK);

        let html = grid.to_html_string();
        assert!(html.contains("&lt;"));
        assert!(html.contains("&gt;"));
        assert!(html.contains("&amp;"));
        assert!(html.contains("&quot;"));
    }

    #[test]
    fn layer_composite_respects_order_and_visibility() {
        let mut bg = Grid::new(4, 2);
        bg.write_str(0, 0, "....", Color::BLACK, Color::BLACK);
        bg.write_str(0, 1, "....", Color::BLACK, Color::BLACK);

        let mut fg = Grid::new(4, 2);
        fg.put(1, 0, Cell::new('@'));
        fg.put(2, 1, Cell::new('!'));

        let layers = vec![Layer::new("bg", 0, bg), Layer::new("fg", 1, fg)];

        let mut result = Grid::new(4, 2);
        Layer::composite(&layers, &mut result);

        assert_eq!(result.get(1, 0).unwrap().glyph, '@');
        assert_eq!(result.get(2, 1).unwrap().glyph, '!');
        assert_eq!(result.get(0, 0).unwrap().glyph, '.');
    }

    #[test]
    fn layer_composite_skips_hidden_layers() {
        let mut bg = Grid::new(3, 1);
        bg.write_str(0, 0, "abc", Color::BLACK, Color::BLACK);

        let mut overlay = Grid::new(3, 1);
        overlay.write_str(0, 0, "XYZ", Color::WHITE, Color::BLACK);

        let layers = vec![
            Layer::new("bg", 0, bg),
            Layer {
                name: "hidden",
                order: 1,
                grid: overlay,
                visible: false,
            },
        ];

        let mut result = Grid::new(3, 1);
        Layer::composite(&layers, &mut result);

        assert_eq!(result.to_plain_string(), "abc");
    }

    #[test]
    fn layer_composite_draws_lower_order_first() {
        let bottom = Grid::new(2, 1);
        let top = Grid::new(2, 1);

        // Draw bottom first (order 0), then top (order 1).
        let layers = vec![Layer::new("bottom", 0, bottom), Layer::new("top", 1, top)];
        let mut result = Grid::new(2, 1);
        Layer::composite(&layers, &mut result);
        // Both are empty grids, so result is empty.
        assert_eq!(result.to_plain_string(), "  ");
    }

    #[test]
    fn transform_applies_function_to_all_cells() {
        let mut grid = Grid::new(3, 2);
        grid.write_str(0, 0, "abc", Color::WHITE, Color::BLACK);
        grid.write_str(0, 1, "def", Color::WHITE, Color::BLACK);

        grid.transform(|cell| {
            cell.fg = Color::RED;
            cell.glyph = cell.glyph.to_ascii_uppercase();
        });

        assert_eq!(grid.to_plain_string(), "ABC\nDEF");
        assert_eq!(grid.get(0, 0).unwrap().fg, Color::RED);
        assert_eq!(grid.get(2, 1).unwrap().fg, Color::RED);
    }

    #[test]
    fn map_creates_transformed_copy() {
        let grid = Grid::new(2, 1);
        let transformed = grid.map(|_cell| Cell::new('*').with_fg(Color::GREEN));

        assert_eq!(transformed.to_plain_string(), "**");
        assert_eq!(transformed.get(0, 0).unwrap().fg, Color::GREEN);
        // Original unchanged.
        assert_eq!(grid.get(0, 0).unwrap().glyph, ' ');
    }

    #[test]
    fn color_palette_dark_dungeon_has_expected_colors() {
        let p = ColorPalette::dark_dungeon();
        assert_eq!(p.name, "dark_dungeon");
        assert_eq!(p.background, Color(15, 15, 20));
        assert_eq!(p.player, Color(100, 220, 100));
        assert_eq!(p.hazard, Color(220, 80, 80));
    }

    #[test]
    fn color_palette_creates_cells_with_correct_colors() {
        let p = ColorPalette::dark_dungeon();
        let player = p.player_cell('@');
        assert_eq!(player.glyph, '@');
        assert_eq!(player.fg, p.player);
        assert_eq!(player.bg, p.floor);

        let wall = p.wall_cell('#');
        assert_eq!(wall.fg, p.wall);
        assert_eq!(wall.bg, p.background);
    }

    #[test]
    fn color_palette_default_is_dark_dungeon() {
        let p = ColorPalette::default();
        assert_eq!(p.name, "dark_dungeon");
    }

    #[test]
    fn layers_add_and_get_by_name() {
        let mut layers = Layers::new();
        layers.add(Layer::new("map", 0, Grid::new(10, 5)));
        layers.add(Layer::new("ui", 10, Grid::new(10, 5)));

        assert_eq!(layers.len(), 2);
        assert!(layers.get("map").is_some());
        assert!(layers.get("ui").is_some());
        assert!(layers.get("missing").is_none());
    }

    #[test]
    fn layers_add_replaces_existing() {
        let mut layers = Layers::new();
        layers.add(Layer::new("map", 0, Grid::new(10, 5)));
        layers.add(Layer::new("map", 5, Grid::new(20, 10)));

        assert_eq!(layers.len(), 1);
        let map = layers.get("map").unwrap();
        assert_eq!(map.order, 5);
        assert_eq!(map.grid.width(), 20);
    }

    #[test]
    fn layers_remove_by_name() {
        let mut layers = Layers::new();
        layers.add(Layer::new("map", 0, Grid::new(10, 5)));
        layers.add(Layer::new("ui", 10, Grid::new(10, 5)));

        assert!(layers.remove("map"));
        assert_eq!(layers.len(), 1);
        assert!(layers.get("map").is_none());
        assert!(!layers.remove("missing"));
    }

    #[test]
    fn layers_composite_onto_target() {
        let mut layers = Layers::new();
        let mut bg = Grid::new(4, 2);
        bg.write_str(0, 0, "....", Color::BLACK, Color::BLACK);
        bg.write_str(0, 1, "....", Color::BLACK, Color::BLACK);
        layers.add(Layer::new("bg", 0, bg));

        let mut result = Grid::new(4, 2);
        layers.composite(&mut result);
        assert_eq!(result.to_plain_string(), "....\n....");
    }

    #[test]
    fn layers_iter_returns_in_draw_order() {
        let mut layers = Layers::new();
        layers.add(Layer::new("ui", 20, Grid::new(1, 1)));
        layers.add(Layer::new("map", 0, Grid::new(1, 1)));
        layers.add(Layer::new("entities", 10, Grid::new(1, 1)));

        let names: Vec<&str> = layers.iter().map(|l| l.name).collect();
        assert_eq!(names, vec!["map", "entities", "ui"]);
    }

    #[test]
    fn grid_resize_preserves_overlapping_content() {
        let mut grid = Grid::new(5, 3);
        grid.write_str(0, 0, "abcde", Color::WHITE, Color::BLACK);
        grid.write_str(0, 1, "fghij", Color::WHITE, Color::BLACK);
        grid.write_str(0, 2, "klmno", Color::WHITE, Color::BLACK);

        // Shrink width.
        grid.resize(3, 3);
        assert_eq!(grid.width(), 3);
        assert_eq!(grid.height(), 3);
        assert_eq!(grid.to_plain_string(), "abc\nfgh\nklm");

        // Grow width.
        grid.resize(6, 3);
        assert_eq!(grid.width(), 6);
        assert_eq!(grid.to_plain_string().lines().next(), Some("abc   "));
    }

    #[test]
    fn grid_resize_shrinks_height() {
        let mut grid = Grid::new(3, 4);
        grid.write_str(0, 0, "aaa", Color::WHITE, Color::BLACK);
        grid.write_str(0, 1, "bbb", Color::WHITE, Color::BLACK);
        grid.write_str(0, 2, "ccc", Color::WHITE, Color::BLACK);
        grid.write_str(0, 3, "ddd", Color::WHITE, Color::BLACK);

        grid.resize(3, 2);
        assert_eq!(grid.height(), 2);
        assert_eq!(grid.to_plain_string(), "aaa\nbbb");
    }

    #[test]
    fn grid_resize_grows_height_with_empty() {
        let mut grid = Grid::new(3, 2);
        grid.write_str(0, 0, "aaa", Color::WHITE, Color::BLACK);
        grid.write_str(0, 1, "bbb", Color::WHITE, Color::BLACK);

        grid.resize(3, 4);
        assert_eq!(grid.height(), 4);
        let plain = grid.to_plain_string();
        let lines: Vec<&str> = plain.lines().collect();
        assert_eq!(lines[0], "aaa");
        assert_eq!(lines[1], "bbb");
        assert_eq!(lines[2], "   ");
        assert_eq!(lines[3], "   ");
    }

    #[test]
    fn grid_resize_noop_when_same_size() {
        let mut grid = Grid::new(5, 3);
        grid.write_str(0, 0, "hello", Color::WHITE, Color::BLACK);
        let original = grid.to_plain_string();

        grid.resize(5, 3);
        assert_eq!(grid.to_plain_string(), original);
    }

    #[test]
    fn sprite_advances_frames_on_tick() {
        let f1 = Frame::new(Grid::new(2, 1), 2);
        let f2 = Frame::new(Grid::new(2, 1), 2);
        let mut sprite = Sprite::new("test", vec![f1, f2]);

        assert_eq!(sprite.current_index(), 0);
        // Frame 0: tick 1 (no change yet)
        assert!(!sprite.tick());
        assert_eq!(sprite.current_index(), 0);
        // Frame 0: tick 2 (advance)
        assert!(sprite.tick());
        assert_eq!(sprite.current_index(), 1);
        // Frame 1: tick 1 (no change)
        assert!(!sprite.tick());
        assert_eq!(sprite.current_index(), 1);
        // Frame 1: tick 2 (loop back to 0)
        assert!(sprite.tick());
        assert_eq!(sprite.current_index(), 0);
    }

    #[test]
    fn sprite_pause_prevents_advancement() {
        let f1 = Frame::new(Grid::new(1, 1), 1);
        let f2 = Frame::new(Grid::new(1, 1), 1);
        let mut sprite = Sprite::new("test", vec![f1, f2]);

        sprite.tick();
        assert_eq!(sprite.current_index(), 1);
        sprite.toggle_pause();
        assert!(sprite.is_paused());
        for _ in 0..10 {
            assert!(!sprite.tick());
        }
        assert_eq!(sprite.current_index(), 1);
        sprite.toggle_pause();
        assert!(sprite.tick());
        assert_eq!(sprite.current_index(), 0);
    }

    #[test]
    fn sprite_reset_returns_to_first_frame() {
        let f1 = Frame::new(Grid::new(1, 1), 1);
        let f2 = Frame::new(Grid::new(1, 1), 1);
        let mut sprite = Sprite::new("test", vec![f1, f2]);

        sprite.tick();
        assert_eq!(sprite.current_index(), 1);
        sprite.reset();
        assert_eq!(sprite.current_index(), 0);
    }

    #[test]
    fn sprite_set_frame_clamps() {
        let mut sprite = Sprite::new(
            "test",
            vec![
                Frame::new(Grid::new(1, 1), 1),
                Frame::new(Grid::new(1, 1), 1),
            ],
        );
        sprite.set_frame(100);
        assert_eq!(sprite.current_index(), 1);
        sprite.set_frame(0);
        assert_eq!(sprite.current_index(), 0);
    }

    #[test]
    fn sprite_sheet_add_and_get() {
        let mut sheet = SpriteSheet::new();
        sheet.add(Sprite::new("idle", vec![Frame::new(Grid::new(1, 1), 1)]));
        sheet.add(Sprite::new("walk", vec![Frame::new(Grid::new(1, 1), 1)]));

        assert_eq!(sheet.len(), 2);
        assert!(sheet.get("idle").is_some());
        assert!(sheet.get_mut("walk").is_some());
        assert!(sheet.get("missing").is_none());
    }

    #[test]
    fn sprite_sheet_add_replaces_existing() {
        let mut sheet = SpriteSheet::new();
        sheet.add(Sprite::new("walk", vec![Frame::new(Grid::new(1, 1), 1)]));
        sheet.add(Sprite::new(
            "walk",
            vec![
                Frame::new(Grid::new(1, 1), 1),
                Frame::new(Grid::new(1, 1), 1),
            ],
        ));
        assert_eq!(sheet.len(), 1);
        assert_eq!(sheet.get("walk").unwrap().frame_count(), 2);
    }

    #[test]
    fn sprite_sheet_tick_all_advances_every_sprite() {
        let mut sheet = SpriteSheet::new();
        sheet.add(Sprite::new(
            "a",
            vec![
                Frame::new(Grid::new(1, 1), 1),
                Frame::new(Grid::new(1, 1), 1),
            ],
        ));
        sheet.add(Sprite::new(
            "b",
            vec![
                Frame::new(Grid::new(1, 1), 1),
                Frame::new(Grid::new(1, 1), 1),
            ],
        ));

        sheet.tick_all();
        assert_eq!(sheet.get("a").unwrap().current_index(), 1);
        assert_eq!(sheet.get("b").unwrap().current_index(), 1);
    }

    #[test]
    fn sparkline_renders_blocks_for_values() {
        let mut grid = Grid::new(8, 1);
        let values = vec![0.0, 0.25, 0.5, 0.75, 1.0, 0.75, 0.5, 0.25];
        let count = draw_sparkline(&mut grid, 0, 0, 8, &values, Color::WHITE, Color::BLACK);
        assert_eq!(count, 8);
        let s = grid.to_plain_string();
        // Should contain block characters.
        assert!(s.contains('\u{2581}') || s.contains('\u{2588}'));
    }

    #[test]
    fn sparkline_handles_single_value() {
        let mut grid = Grid::new(5, 1);
        let count = draw_sparkline(&mut grid, 0, 0, 5, &[42.0], Color::WHITE, Color::BLACK);
        assert_eq!(count, 5);
    }

    #[test]
    fn sparkline_returns_zero_for_empty_values() {
        let mut grid = Grid::new(5, 1);
        let count = draw_sparkline(&mut grid, 0, 0, 5, &[], Color::WHITE, Color::BLACK);
        assert_eq!(count, 0);
    }

    #[test]
    fn sparkline_clips_to_grid_width() {
        let mut grid = Grid::new(3, 1);
        let values = vec![0.0, 0.5, 1.0, 0.5, 0.0];
        let count = draw_sparkline(&mut grid, 0, 0, 5, &values, Color::WHITE, Color::BLACK);
        assert_eq!(count, 3);
    }

    #[test]
    fn write_aligned_left_starts_at_x() {
        let mut grid = Grid::new(10, 1);
        grid.write_aligned(2, 0, 6, "hi", Alignment::Left, Color::WHITE, Color::BLACK);
        assert_eq!(grid.to_plain_string(), "  hi      ");
    }

    #[test]
    fn write_aligned_center() {
        let mut grid = Grid::new(10, 1);
        grid.write_aligned(
            0,
            0,
            10,
            "hi",
            Alignment::Center,
            Color::WHITE,
            Color::BLACK,
        );
        assert_eq!(grid.to_plain_string(), "    hi    ");
    }

    #[test]
    fn write_aligned_right() {
        let mut grid = Grid::new(10, 1);
        grid.write_aligned(0, 0, 10, "hi", Alignment::Right, Color::WHITE, Color::BLACK);
        assert_eq!(grid.to_plain_string(), "        hi");
    }

    #[test]
    fn write_aligned_clips_when_text_too_wide() {
        let mut grid = Grid::new(10, 1);
        // Text "hello" (5 chars) is wider than width 4, so it starts at x=0.
        // write_str writes all 5 chars since grid is 10 wide.
        grid.write_aligned(
            0,
            0,
            4,
            "hello",
            Alignment::Center,
            Color::WHITE,
            Color::BLACK,
        );
        assert_eq!(grid.to_plain_string(), "hello     ");
    }

    #[test]
    fn alignment_default_is_left() {
        assert_eq!(Alignment::default(), Alignment::Left);
    }

    #[test]
    fn sprite_sheet_remove_by_name() {
        let mut sheet = SpriteSheet::new();
        sheet.add(Sprite::new("a", vec![Frame::new(Grid::new(1, 1), 1)]));
        sheet.add(Sprite::new("b", vec![Frame::new(Grid::new(1, 1), 1)]));
        assert!(sheet.remove("a"));
        assert_eq!(sheet.len(), 1);
        assert!(sheet.get("a").is_none());
        assert!(!sheet.remove("missing"));
    }

    #[test]
    fn sprite_sheet_reset_all() {
        let mut sheet = SpriteSheet::new();
        sheet.add(Sprite::new(
            "a",
            vec![
                Frame::new(Grid::new(1, 1), 1),
                Frame::new(Grid::new(1, 1), 1),
            ],
        ));
        sheet.tick_all();
        assert_eq!(sheet.get("a").unwrap().current_index(), 1);
        sheet.reset_all();
        assert_eq!(sheet.get("a").unwrap().current_index(), 0);
    }

    #[test]
    fn draw_gradient_fills_rect_with_interpolated_colors() {
        let mut grid = Grid::new(5, 1);
        grid.draw_gradient(
            Rect::new(0, 0, 5, 1),
            Color(0, 0, 0),
            Color(100, 100, 100),
            '#',
        );
        // First cell should be start color.
        assert_eq!(grid.get(0, 0).unwrap().fg, Color(0, 0, 0));
        // Last cell should be end color.
        assert_eq!(grid.get(4, 0).unwrap().fg, Color(100, 100, 100));
        // Middle cell should be interpolated.
        let mid = grid.get(2, 0).unwrap().fg;
        assert!(mid.0 > 0 && mid.0 < 100);
    }

    #[test]
    fn draw_gradient_v_fills_vertically() {
        let mut grid = Grid::new(1, 5);
        grid.draw_gradient_v(
            Rect::new(0, 0, 1, 5),
            Color(0, 0, 0),
            Color(80, 80, 80),
            '#',
        );
        assert_eq!(grid.get(0, 0).unwrap().fg, Color(0, 0, 0));
        assert_eq!(grid.get(0, 4).unwrap().fg, Color(80, 80, 80));
    }

    #[test]
    fn draw_gradient_clips_to_grid() {
        let mut grid = Grid::new(3, 3);
        grid.draw_gradient(Rect::new(0, 0, 10, 10), Color::BLACK, Color::WHITE, '#');
        // Should not panic and should fill within bounds.
        assert_eq!(grid.get(0, 0).unwrap().glyph, '#');
        assert_eq!(grid.get(2, 2).unwrap().glyph, '#');
    }

    #[test]
    fn rect_union_combines_two_rects() {
        let a = Rect::new(2, 3, 4, 5);
        let b = Rect::new(5, 1, 3, 6);
        let u = a.union(b);
        assert_eq!(u.x, 2);
        assert_eq!(u.y, 1);
        assert_eq!(u.right(), 8);
        assert_eq!(u.bottom(), 8);
    }

    #[test]
    fn rect_union_with_empty_returns_other() {
        let a = Rect::new(0, 0, 0, 0);
        let b = Rect::new(1, 2, 3, 4);
        assert_eq!(a.union(b), b);
        assert_eq!(b.union(a), b);
    }

    #[test]
    fn swap_cells_exchanges_positions() {
        let mut grid = Grid::new(3, 2);
        grid.write_str(0, 0, "ABC", Color::WHITE, Color::BLACK);
        grid.write_str(0, 1, "DEF", Color::WHITE, Color::BLACK);

        assert!(grid.swap_cells(0, 0, 2, 1));
        assert_eq!(grid.get(0, 0).unwrap().glyph, 'F');
        assert_eq!(grid.get(2, 1).unwrap().glyph, 'A');
    }

    #[test]
    fn swap_cells_returns_false_for_out_of_bounds() {
        let mut grid = Grid::new(2, 2);
        assert!(!grid.swap_cells(0, 0, 5, 0));
        assert!(!grid.swap_cells(0, 0, 0, 5));
    }

    #[test]
    fn swap_cells_same_position_is_noop() {
        let mut grid = Grid::new(2, 2);
        grid.write_str(0, 0, "AB", Color::WHITE, Color::BLACK);
        assert!(grid.swap_cells(0, 0, 0, 0));
        assert_eq!(grid.get(0, 0).unwrap().glyph, 'A');
    }

    #[test]
    fn row_returns_slice_of_row() {
        let mut grid = Grid::new(3, 2);
        grid.write_str(0, 0, "ABC", Color::WHITE, Color::BLACK);
        grid.write_str(0, 1, "DEF", Color::WHITE, Color::BLACK);

        let row0 = grid.row(0).unwrap();
        assert_eq!(row0.len(), 3);
        assert_eq!(row0[0].glyph, 'A');
        assert_eq!(row0[1].glyph, 'B');
        assert_eq!(row0[2].glyph, 'C');

        let row1 = grid.row(1).unwrap();
        assert_eq!(row1[0].glyph, 'D');
    }

    #[test]
    fn row_returns_none_for_out_of_bounds() {
        let grid = Grid::new(2, 2);
        assert!(grid.row(2).is_none());
    }

    #[test]
    fn row_mut_returns_mutable_slice() {
        let mut grid = Grid::new(3, 2);
        grid.write_str(0, 0, "ABC", Color::WHITE, Color::BLACK);
        if let Some(row) = grid.row_mut(1) {
            row.fill(Cell::new('x'));
        }
        assert_eq!(grid.get(0, 1).unwrap().glyph, 'x');
        assert_eq!(grid.get(2, 1).unwrap().glyph, 'x');
    }

    #[test]
    fn fill_row_writes_full_row() {
        let mut grid = Grid::new(3, 2);
        assert!(grid.fill_row(1, Cell::new('x')));
        assert_eq!(grid.get(0, 1).unwrap().glyph, 'x');
        assert_eq!(grid.get(2, 1).unwrap().glyph, 'x');
        assert!(!grid.fill_row(4, Cell::new('y')));
    }

    #[test]
    fn fill_col_writes_full_column() {
        let mut grid = Grid::new(2, 3);
        assert!(grid.fill_col(1, Cell::new('y')));
        assert_eq!(grid.get(1, 0).unwrap().glyph, 'y');
        assert_eq!(grid.get(1, 2).unwrap().glyph, 'y');
        assert!(!grid.fill_col(5, Cell::new('z')));
    }

    #[test]
    fn col_returns_cells_of_column() {
        let mut grid = Grid::new(3, 4);
        grid.write_str(0, 0, "ABC", Color::WHITE, Color::BLACK);
        grid.write_str(0, 1, "DEF", Color::WHITE, Color::BLACK);
        grid.write_str(0, 2, "GHI", Color::WHITE, Color::BLACK);
        grid.write_str(0, 3, "JKL", Color::WHITE, Color::BLACK);

        let col1 = grid.col(1).unwrap();
        assert_eq!(col1.len(), 4);
        assert_eq!(col1[0].glyph, 'B');
        assert_eq!(col1[1].glyph, 'E');
        assert_eq!(col1[2].glyph, 'H');
        assert_eq!(col1[3].glyph, 'K');
    }

    #[test]
    fn col_returns_none_for_out_of_bounds() {
        let grid = Grid::new(2, 2);
        assert!(grid.col(2).is_none());
    }

    #[test]
    fn iter_cells_yields_all_positions() {
        let mut grid = Grid::new(3, 2);
        grid.write_str(0, 0, "ABC", Color::WHITE, Color::BLACK);
        grid.write_str(0, 1, "DEF", Color::WHITE, Color::BLACK);

        let cells: Vec<_> = grid.iter_cells().collect();
        assert_eq!(cells.len(), 6);
        assert_eq!(cells[0], (0, 0, &Cell::new('A')));
        assert_eq!(cells[2], (2, 0, &Cell::new('C')));
        assert_eq!(cells[3], (0, 1, &Cell::new('D')));
        assert_eq!(cells[5], (2, 1, &Cell::new('F')));
    }

    #[test]
    fn rect_translate_offsets_position() {
        let r = Rect::new(5, 10, 3, 4);
        let t = r.translate(2, -3);
        assert_eq!(t.x, 7);
        assert_eq!(t.y, 7);
        assert_eq!(t.width, 3);
        assert_eq!(t.height, 4);
    }

    #[test]
    fn rect_translate_clamps_negative() {
        let r = Rect::new(2, 3, 4, 5);
        let t = r.translate(-10, -10);
        assert_eq!(t.x, 0);
        assert_eq!(t.y, 0);
        assert_eq!(t.width, 4);
        assert_eq!(t.height, 5);
    }

    #[test]
    fn rect_area_computes_correctly() {
        assert_eq!(Rect::new(0, 0, 3, 4).area(), 12);
        assert_eq!(Rect::new(0, 0, 0, 5).area(), 0);
        assert_eq!(Rect::new(0, 0, 1, 1).area(), 1);
    }

    #[test]
    fn blit_region_copies_sub_rectangle() {
        let mut src = Grid::new(4, 3);
        src.write_str(0, 0, "ABCD", Color::WHITE, Color::BLACK);
        src.write_str(0, 1, "EFGH", Color::WHITE, Color::BLACK);
        src.write_str(0, 2, "IJKL", Color::WHITE, Color::BLACK);

        let mut dst = Grid::new(3, 3);
        dst.blit_region(&src, Rect::new(1, 1, 2, 2), 0, 0);

        assert_eq!(dst.get(0, 0).unwrap().glyph, 'F');
        assert_eq!(dst.get(1, 0).unwrap().glyph, 'G');
        assert_eq!(dst.get(0, 1).unwrap().glyph, 'J');
        assert_eq!(dst.get(1, 1).unwrap().glyph, 'K');
    }

    #[test]
    fn blit_region_clips_to_source_bounds() {
        let mut src = Grid::new(2, 2);
        src.write_str(0, 0, "AB", Color::WHITE, Color::BLACK);
        src.write_str(0, 1, "CD", Color::WHITE, Color::BLACK);

        let mut dst = Grid::new(2, 2);
        dst.blit_region(&src, Rect::new(0, 0, 5, 5), 0, 0);

        assert_eq!(dst.get(0, 0).unwrap().glyph, 'A');
        assert_eq!(dst.get(1, 1).unwrap().glyph, 'D');
    }

    #[test]
    fn scroll_up_shifts_content() {
        let mut grid = Grid::new(3, 4);
        grid.write_str(0, 0, "ABC", Color::WHITE, Color::BLACK);
        grid.write_str(0, 1, "DEF", Color::WHITE, Color::BLACK);
        grid.write_str(0, 2, "GHI", Color::WHITE, Color::BLACK);
        grid.write_str(0, 3, "JKL", Color::WHITE, Color::BLACK);

        grid.scroll_up(2, Cell::EMPTY);

        assert_eq!(grid.get(0, 0).unwrap().glyph, 'G');
        assert_eq!(grid.get(0, 1).unwrap().glyph, 'J');
        assert_eq!(grid.get(0, 2).unwrap().glyph, ' ');
        assert_eq!(grid.get(0, 3).unwrap().glyph, ' ');
    }

    #[test]
    fn scroll_down_shifts_content() {
        let mut grid = Grid::new(3, 4);
        grid.write_str(0, 0, "ABC", Color::WHITE, Color::BLACK);
        grid.write_str(0, 1, "DEF", Color::WHITE, Color::BLACK);
        grid.write_str(0, 2, "GHI", Color::WHITE, Color::BLACK);
        grid.write_str(0, 3, "JKL", Color::WHITE, Color::BLACK);

        grid.scroll_down(1, Cell::EMPTY);

        assert_eq!(grid.get(0, 0).unwrap().glyph, ' ');
        assert_eq!(grid.get(0, 1).unwrap().glyph, 'A');
        assert_eq!(grid.get(0, 2).unwrap().glyph, 'D');
        assert_eq!(grid.get(0, 3).unwrap().glyph, 'G');
    }

    #[test]
    fn scroll_up_zero_is_noop() {
        let mut grid = Grid::new(2, 2);
        grid.write_str(0, 0, "AB", Color::WHITE, Color::BLACK);
        grid.scroll_up(0, Cell::EMPTY);
        assert_eq!(grid.get(0, 0).unwrap().glyph, 'A');
    }

    #[test]
    fn scroll_up_full_height_clears() {
        let mut grid = Grid::new(2, 2);
        grid.write_str(0, 0, "AB", Color::WHITE, Color::BLACK);
        grid.scroll_up(2, Cell::EMPTY);
        assert_eq!(grid.get(0, 0).unwrap().glyph, ' ');
    }

    #[test]
    fn fill_rect_fills_region() {
        let mut grid = Grid::new(5, 5);
        let filled = Cell::new('X');
        grid.fill_rect(Rect::new(1, 1, 3, 3), filled);

        // Inside region.
        assert_eq!(grid.get(1, 1).unwrap().glyph, 'X');
        assert_eq!(grid.get(3, 3).unwrap().glyph, 'X');
        // Outside region.
        assert_eq!(grid.get(0, 0).unwrap().glyph, ' ');
        assert_eq!(grid.get(4, 4).unwrap().glyph, ' ');
    }

    #[test]
    fn fill_rect_clips_to_bounds() {
        let mut grid = Grid::new(3, 3);
        let filled = Cell::new('X');
        grid.fill_rect(Rect::new(1, 1, 5, 5), filled);

        assert_eq!(grid.get(2, 2).unwrap().glyph, 'X');
        assert_eq!(grid.get(0, 0).unwrap().glyph, ' ');
    }

    #[test]
    fn cell_attrs_to_ansi_handles_multiple_attributes() {
        let attrs = CellAttrs::NONE.bold().underline();
        let ansi = attrs.to_ansi();
        assert_eq!(ansi, "\x1b[1;4m");

        let attrs = CellAttrs::NONE.bold().dim().italic();
        let ansi = attrs.to_ansi();
        assert_eq!(ansi, "\x1b[1;2;3m");

        let attrs = CellAttrs::NONE.reverse().blink();
        let ansi = attrs.to_ansi();
        assert_eq!(ansi, "\x1b[5;7m");

        let attrs = CellAttrs::NONE;
        let ansi = attrs.to_ansi();
        assert_eq!(ansi, "");
    }

    #[test]
    fn cell_attrs_to_ansi_all_attributes() {
        let attrs = CellAttrs {
            bold: true,
            underline: true,
            dim: true,
            italic: true,
            reverse: true,
            blink: true,
        };
        let ansi = attrs.to_ansi();
        assert_eq!(ansi, "\x1b[1;2;3;4;5;7m");
    }
}
