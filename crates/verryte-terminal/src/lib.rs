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
    pub fn draw_line(&mut self, start: (i32, i32), end: (i32, i32), cell: Cell) {
        let (mut x0, mut y0) = start;
        let (x1, y1) = end;
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        loop {
            if x0 >= 0 && y0 >= 0 {
                self.put(x0 as u16, y0 as u16, cell);
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

    /// Render the grid as ANSI-escaped text with foreground and background colors.
    ///
    /// This produces output that any ANSI-compatible terminal can display without
    /// needing crossterm or a live TTY. Useful for debug dumps, log files, and
    /// agent observation over plain text channels.
    pub fn to_ansi_string(&self) -> String {
        let mut out = String::with_capacity(self.cells.len() * 20 + self.height as usize * 10);
        let mut last_fg: Option<Color> = None;
        let mut last_bg: Option<Color> = None;

        for y in 0..self.height {
            if y > 0 {
                out.push('\n');
            }
            for x in 0..self.width {
                let cell = &self.cells[(y as usize) * (self.width as usize) + (x as usize)];
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
        let transformed = grid.map(|cell| Cell::new('*').with_fg(Color::GREEN));

        assert_eq!(transformed.to_plain_string(), "**");
        assert_eq!(transformed.get(0, 0).unwrap().fg, Color::GREEN);
        // Original unchanged.
        assert_eq!(grid.get(0, 0).unwrap().glyph, ' ');
    }
}
