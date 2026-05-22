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

pub mod vfx;

/// 24-bit RGB color.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

    /// Linear interpolation between two colors.
    pub fn lerp(self, other: Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        let r = (self.0 as f32 + (other.0 as i16 - self.0 as i16) as f32 * t).round() as u8;
        let g = (self.1 as f32 + (other.1 as i16 - self.1 as i16) as f32 * t).round() as u8;
        let b = (self.2 as f32 + (other.2 as i16 - self.2 as i16) as f32 * t).round() as u8;
        Color(r, g, b)
    }

    /// Parse a 6-digit hex color string (with or without a leading '#').
    pub fn from_hex(hex: &str) -> Result<Self, String> {
        let hex = hex.strip_prefix('#').unwrap_or(hex);
        if hex.len() != 6 {
            return Err(format!(
                "Hex color must be 6 hex characters (excluding '#'), got: {}",
                hex
            ));
        }
        let r = u8::from_str_radix(&hex[0..2], 16)
            .map_err(|e| format!("Invalid red hex component: {}", e))?;
        let g = u8::from_str_radix(&hex[2..4], 16)
            .map_err(|e| format!("Invalid green hex component: {}", e))?;
        let b = u8::from_str_radix(&hex[4..6], 16)
            .map_err(|e| format!("Invalid blue hex component: {}", e))?;
        Ok(Color(r, g, b))
    }

    /// Convert to HSV representation: (hue [0, 360], saturation [0, 1], value [0, 1]).
    pub fn to_hsv(self) -> (f32, f32, f32) {
        let r = self.0 as f32 / 255.0;
        let g = self.1 as f32 / 255.0;
        let b = self.2 as f32 / 255.0;

        let min = r.min(g).min(b);
        let max = r.max(g).max(b);
        let delta = max - min;

        let v = max;

        let s = if max > 0.0 { delta / max } else { 0.0 };

        let h = if delta > 0.0 {
            let mut h_calc = if max == r {
                (g - b) / delta
            } else if max == g {
                2.0 + (b - r) / delta
            } else {
                4.0 + (r - g) / delta
            };
            h_calc *= 60.0;
            if h_calc < 0.0 {
                h_calc += 360.0;
            }
            h_calc
        } else {
            0.0
        };

        (h, s, v)
    }

    /// Create from HSV representation: (hue [0, 360], saturation [0, 1], value [0, 1]).
    pub fn from_hsv(h: f32, s: f32, v: f32) -> Self {
        let h = h.rem_euclid(360.0);
        let s = s.clamp(0.0, 1.0);
        let v = v.clamp(0.0, 1.0);

        let c = v * s;
        let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
        let m = v - c;

        let (r_prime, g_prime, b_prime) = if h < 60.0 {
            (c, x, 0.0)
        } else if h < 120.0 {
            (x, c, 0.0)
        } else if h < 180.0 {
            (0.0, c, x)
        } else if h < 240.0 {
            (0.0, x, c)
        } else if h < 300.0 {
            (x, 0.0, c)
        } else {
            (c, 0.0, x)
        };

        let r = ((r_prime + m) * 255.0).round().clamp(0.0, 255.0) as u8;
        let g = ((g_prime + m) * 255.0).round().clamp(0.0, 255.0) as u8;
        let b = ((b_prime + m) * 255.0).round().clamp(0.0, 255.0) as u8;

        Color(r, g, b)
    }
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{:02X}{:02X}{:02X}", self.0, self.1, self.2)
    }
}

impl From<(u8, u8, u8)> for Color {
    fn from((r, g, b): (u8, u8, u8)) -> Self {
        Color(r, g, b)
    }
}

impl From<Color> for (u8, u8, u8) {
    fn from(c: Color) -> Self {
        (c.0, c.1, c.2)
    }
}

/// Bitflags for terminal cell text attributes.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

    pub fn is_bold(self) -> bool {
        self.bold
    }

    pub fn is_underline(self) -> bool {
        self.underline
    }

    pub fn is_dim(self) -> bool {
        self.dim
    }

    pub fn is_italic(self) -> bool {
        self.italic
    }

    pub fn is_reverse(self) -> bool {
        self.reverse
    }

    pub fn is_blink(self) -> bool {
        self.blink
    }

    /// Returns `true` if no attributes are set.
    pub fn is_empty(self) -> bool {
        self == Self::NONE
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

    /// Shrink the rectangle by `(dx, dy)` on all sides.
    ///
    /// If the inset exceeds the rectangle's size, the result is an empty rect.
    /// Useful for computing inner panel regions or padding UI layouts.
    pub fn inset(self, dx: u16, dy: u16) -> Rect {
        let x = self.x.saturating_add(dx);
        let y = self.y.saturating_add(dy);
        let width = self.width.saturating_sub(dx.saturating_mul(2));
        let height = self.height.saturating_sub(dy.saturating_mul(2));
        Rect::new(x, y, width, height)
    }

    /// Offset the rectangle by `(dx, dy)`. Negative offsets are clamped to zero.
    ///
    /// Useful for moving UI elements or adjusting rects for viewport offsets.
    pub fn translate(self, dx: i16, dy: i16) -> Rect {
        let x = (self.x as i16 + dx).max(0) as u16;
        let y = (self.y as i16 + dy).max(0) as u16;
        Rect::new(x, y, self.width, self.height)
    }

    /// Split this rectangle horizontally at a relative height offset `split_y`.
    ///
    /// Returns a tuple of (top_rect, bottom_rect).
    pub fn split_horizontal(self, split_y: u16) -> (Rect, Rect) {
        let top_h = split_y.min(self.height);
        let bottom_h = self.height.saturating_sub(top_h);
        let top = Rect::new(self.x, self.y, self.width, top_h);
        let bottom = Rect::new(self.x, self.y.saturating_add(top_h), self.width, bottom_h);
        (top, bottom)
    }

    /// Split this rectangle vertically at a relative width offset `split_x`.
    ///
    /// Returns a tuple of (left_rect, right_rect).
    pub fn split_vertical(self, split_x: u16) -> (Rect, Rect) {
        let left_w = split_x.min(self.width);
        let right_w = self.width.saturating_sub(left_w);
        let left = Rect::new(self.x, self.y, left_w, self.height);
        let right = Rect::new(self.x.saturating_add(left_w), self.y, right_w, self.height);
        (left, right)
    }

    /// Split this rectangle horizontally at an absolute y-coordinate split point.
    ///
    /// Returns a tuple of (top_rect, bottom_rect).
    pub fn split_horizontal_absolute(self, split_y: u16) -> (Rect, Rect) {
        if split_y <= self.y {
            (Rect::new(self.x, self.y, self.width, 0), self)
        } else if split_y >= self.bottom() {
            (self, Rect::new(self.x, self.bottom(), self.width, 0))
        } else {
            let top_h = split_y - self.y;
            let bottom_h = self.bottom() - split_y;
            let top = Rect::new(self.x, self.y, self.width, top_h);
            let bottom = Rect::new(self.x, split_y, self.width, bottom_h);
            (top, bottom)
        }
    }

    /// Split this rectangle vertically at an absolute x-coordinate split point.
    ///
    /// Returns a tuple of (left_rect, right_rect).
    pub fn split_vertical_absolute(self, split_x: u16) -> (Rect, Rect) {
        if split_x <= self.x {
            (Rect::new(self.x, self.y, 0, self.height), self)
        } else if split_x >= self.right() {
            (self, Rect::new(self.right(), self.y, 0, self.height))
        } else {
            let left_w = split_x - self.x;
            let right_w = self.right() - split_x;
            let left = Rect::new(self.x, self.y, left_w, self.height);
            let right = Rect::new(split_x, self.y, right_w, self.height);
            (left, right)
        }
    }
}

impl std::fmt::Display for Rect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Rect({},{} {}x{})",
            self.x, self.y, self.width, self.height
        )
    }
}

impl From<(u16, u16, u16, u16)> for Rect {
    fn from((x, y, w, h): (u16, u16, u16, u16)) -> Self {
        Rect::new(x, y, w, h)
    }
}

/// Horizontal text alignment within a bounded width.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Alignment {
    #[default]
    Left,
    Center,
    Right,
}

impl std::fmt::Display for Alignment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Alignment::Left => write!(f, "Left"),
            Alignment::Center => write!(f, "Center"),
            Alignment::Right => write!(f, "Right"),
        }
    }
}

/// Border style options for styled TUI borders.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BorderStyle {
    Ascii,
    #[default]
    Single,
    Double,
    Heavy,
    Rounded,
}

/// Viewport camera that manages position and zoom levels.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Camera {
    pub center_x: f32,
    pub center_y: f32,
    pub zoom: f32,
    pub smooth: bool,
    pub target_x: f32,
    pub target_y: f32,
    pub lerp_factor: f32,
}

impl Camera {
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            center_x: x,
            center_y: y,
            zoom: 1.0,
            smooth: false,
            target_x: x,
            target_y: y,
            lerp_factor: 0.1,
        }
    }

    pub fn with_smooth(mut self, factor: f32) -> Self {
        self.smooth = true;
        self.lerp_factor = factor;
        self
    }

    pub fn look_at(&mut self, x: f32, y: f32) {
        if self.smooth {
            self.target_x = x;
            self.target_y = y;
        } else {
            self.center_x = x;
            self.center_y = y;
            self.target_x = x;
            self.target_y = y;
        }
    }

    pub fn tick(&mut self) {
        if self.smooth {
            self.center_x += (self.target_x - self.center_x) * self.lerp_factor;
            self.center_y += (self.target_y - self.center_y) * self.lerp_factor;
        }
    }

    /// Calculate the top-left corner of the viewport for a given window size.
    pub fn top_left(&self, width: u16, height: u16) -> (i16, i16) {
        let zoomed_w = (width as f32 / self.zoom).round() as u16;
        let zoomed_h = (height as f32 / self.zoom).round() as u16;
        let x = (self.center_x - (zoomed_w as f32 / 2.0)).round() as i16;
        let y = (self.center_y - (zoomed_h as f32 / 2.0)).round() as i16;
        (x, y)
    }

    /// Return the [`Rect`] representing the current viewport in grid coordinates.
    pub fn viewport_rect(&self, width: u16, height: u16) -> Rect {
        let (x, y) = self.top_left(width, height);
        let zoomed_w = (width as f32 / self.zoom).round() as u16;
        let zoomed_h = (height as f32 / self.zoom).round() as u16;
        Rect {
            x: x.max(0) as u16,
            y: y.max(0) as u16,
            width: zoomed_w,
            height: zoomed_h,
        }
    }

    /// Clamp the camera's center and target position within the given boundaries.
    pub fn clamp_to_bounds(&mut self, min_x: f32, min_y: f32, max_x: f32, max_y: f32) {
        self.center_x = self.center_x.clamp(min_x, max_x);
        self.center_y = self.center_y.clamp(min_y, max_y);
        self.target_x = self.target_x.clamp(min_x, max_x);
        self.target_y = self.target_y.clamp(min_y, max_y);
    }
}
/// A fixed-size rectangular cell buffer.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

    /// Fill the entire grid with a background cell, preserving foreground glyphs.
    ///
    /// Sets the background color of every cell to `bg` without changing the
    /// glyph or foreground color. This is useful for theme-aware TTY rendering
    /// where the background should span the full terminal.
    pub fn fill_background(&mut self, bg: Color) {
        for cell in &mut self.cells {
            cell.bg = bg;
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

    pub fn get_mut(&mut self, x: u16, y: u16) -> Option<&mut Cell> {
        let i = self.index(x, y)?;
        Some(&mut self.cells[i])
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

    /// Write a string containing markup styling tags directly into standard grid cells.
    ///
    /// Supported tags:
    /// - `[b]` (bold), `[u]` (underline), `[d]` (dim), `[i]` (italic), `[r]` (reverse)
    /// - `[fg:#rrggbb]` or `[fg:color_name]` to set foreground color
    /// - `[bg:#rrggbb]` or `[bg:color_name]` to set background color
    /// - `[/b]`, `[/u]`, `[/d]`, `[/i]`, `[/r]`, `[/fg]`, `[/bg]` to close/restore tags
    /// - `[/]` to reset all formatting back to the root default state
    /// - `[[` to render a literal `[`
    ///
    /// Returns the number of cells/glyphs written or an error string if markup is malformed.
    pub fn write_rich(&mut self, x: u16, y: u16, markup: &str) -> Result<u16, String> {
        #[derive(Clone, Debug)]
        struct MarkupState {
            bold: bool,
            underline: bool,
            dim: bool,
            italic: bool,
            reverse: bool,
            fg: Color,
            bg: Color,
        }

        let mut state_stack = vec![MarkupState {
            bold: false,
            underline: false,
            dim: false,
            italic: false,
            reverse: false,
            fg: Color::WHITE,
            bg: Color::BLACK,
        }];

        let parse_color = |s: &str| -> Result<Color, String> {
            if s.starts_with('#') {
                if s.len() != 7 {
                    return Err(format!("Invalid hex color length: {}", s));
                }
                let r = u8::from_str_radix(&s[1..3], 16).map_err(|e| e.to_string())?;
                let g = u8::from_str_radix(&s[3..5], 16).map_err(|e| e.to_string())?;
                let b = u8::from_str_radix(&s[5..7], 16).map_err(|e| e.to_string())?;
                Ok(Color(r, g, b))
            } else {
                match s.to_ascii_lowercase().as_str() {
                    "black" => Ok(Color::BLACK),
                    "white" => Ok(Color::WHITE),
                    "red" => Ok(Color::RED),
                    "green" => Ok(Color::GREEN),
                    "blue" => Ok(Color::BLUE),
                    "yellow" => Ok(Color::YELLOW),
                    "cyan" => Ok(Color::CYAN),
                    "magenta" => Ok(Color::MAGENTA),
                    "grey" | "gray" => Ok(Color::GREY),
                    "dark_grey" | "dark_gray" | "darkgrey" | "darkgray" => Ok(Color::DARK_GREY),
                    _ => Err(format!("Unknown color name: {}", s)),
                }
            }
        };

        let mut chars = markup.chars().peekable();
        let mut cx = x;
        let mut written_count = 0;

        while let Some(ch) = chars.next() {
            if ch == '[' {
                if chars.peek() == Some(&'[') {
                    // Escaped '['
                    chars.next();
                    let current = state_stack.last().unwrap();
                    let mut attrs = CellAttrs::NONE;
                    attrs.bold = current.bold;
                    attrs.underline = current.underline;
                    attrs.dim = current.dim;
                    attrs.italic = current.italic;
                    attrs.reverse = current.reverse;

                    if cx < self.width {
                        self.put(
                            cx,
                            y,
                            Cell {
                                glyph: '[',
                                fg: current.fg,
                                bg: current.bg,
                                attrs,
                            },
                        );
                        cx += 1;
                    }
                    written_count += 1;
                } else {
                    // Parse tag
                    let mut tag_content = String::new();
                    let mut closed = false;
                    while let Some(&next_ch) = chars.peek() {
                        if next_ch == ']' {
                            chars.next();
                            closed = true;
                            break;
                        } else {
                            tag_content.push(chars.next().unwrap());
                        }
                    }

                    if !closed {
                        return Err("Unclosed markup tag".to_string());
                    }

                    let current = state_stack.last().unwrap().clone();
                    match tag_content.as_str() {
                        "b" => {
                            let mut next_state = current;
                            next_state.bold = true;
                            state_stack.push(next_state);
                        }
                        "u" => {
                            let mut next_state = current;
                            next_state.underline = true;
                            state_stack.push(next_state);
                        }
                        "d" => {
                            let mut next_state = current;
                            next_state.dim = true;
                            state_stack.push(next_state);
                        }
                        "i" => {
                            let mut next_state = current;
                            next_state.italic = true;
                            state_stack.push(next_state);
                        }
                        "r" => {
                            let mut next_state = current;
                            next_state.reverse = true;
                            state_stack.push(next_state);
                        }
                        "/b" | "/u" | "/d" | "/i" | "/r" | "/fg" | "/bg" => {
                            if state_stack.len() > 1 {
                                state_stack.pop();
                            }
                        }
                        "/" => {
                            while state_stack.len() > 1 {
                                state_stack.pop();
                            }
                        }
                        other => {
                            if let Some(rest) = other.strip_prefix("fg:") {
                                let color = parse_color(rest)?;
                                let mut next_state = current;
                                next_state.fg = color;
                                state_stack.push(next_state);
                            } else if let Some(rest) = other.strip_prefix("bg:") {
                                let color = parse_color(rest)?;
                                let mut next_state = current;
                                next_state.bg = color;
                                state_stack.push(next_state);
                            } else {
                                return Err(format!("Unknown markup tag: {}", other));
                            }
                        }
                    }
                }
            } else {
                let current = state_stack.last().unwrap();
                let mut attrs = CellAttrs::NONE;
                attrs.bold = current.bold;
                attrs.underline = current.underline;
                attrs.dim = current.dim;
                attrs.italic = current.italic;
                attrs.reverse = current.reverse;

                if cx < self.width {
                    self.put(
                        cx,
                        y,
                        Cell {
                            glyph: ch,
                            fg: current.fg,
                            bg: current.bg,
                            attrs,
                        },
                    );
                    cx += 1;
                }
                written_count += 1;
            }
        }

        Ok(written_count)
    }

    /// Write multiple lines starting at `(x, y)`, clipping to the grid height.
    ///
    /// Returns the number of lines written.
    pub fn write_lines<I, S>(&mut self, x: u16, y: u16, lines: I, fg: Color, bg: Color) -> usize
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        if y >= self.height {
            return 0;
        }
        let mut written = 0;
        let mut row = y;
        for line in lines {
            if row >= self.height {
                break;
            }
            self.write_str(x, row, line.as_ref(), fg, bg);
            written += 1;
            row = row.saturating_add(1);
        }
        written
    }

    /// Write text aligned within a horizontal range `[x, x + width)`.
    ///
    /// `Alignment::Left` starts at `x`, `Alignment::Center` centers the text,
    /// and `Alignment::Right` right-aligns to `x + width - 1`. Text is clipped
    /// if it exceeds the width.
    #[allow(clippy::too_many_arguments)]
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

    /// Draw a styled border around a rectangle.
    ///
    /// Uses custom box-drawing character maps according to the specified style.
    /// Clips to grid bounds.
    pub fn draw_border_styled(&mut self, rect: Rect, style: BorderStyle, fg: Color, bg: Color) {
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

        let (tl, tr, bl, br, h, v) = match style {
            BorderStyle::Ascii => ('+', '+', '+', '+', '-', '|'),
            BorderStyle::Single => (
                '\u{250C}', '\u{2510}', '\u{2514}', '\u{2518}', '\u{2500}', '\u{2502}',
            ),
            BorderStyle::Double => (
                '\u{2554}', '\u{2557}', '\u{255A}', '\u{255D}', '\u{2550}', '\u{2551}',
            ),
            BorderStyle::Heavy => (
                '\u{250F}', '\u{2513}', '\u{2517}', '\u{251B}', '\u{2501}', '\u{2503}',
            ),
            BorderStyle::Rounded => (
                '\u{256D}', '\u{256E}', '\u{2570}', '\u{256F}', '\u{2500}', '\u{2502}',
            ),
        };

        // Corners
        self.put(left, top, Cell::new(tl).with_fg(fg).with_bg(bg));
        self.put(right, top, Cell::new(tr).with_fg(fg).with_bg(bg));
        self.put(left, bottom, Cell::new(bl).with_fg(fg).with_bg(bg));
        self.put(right, bottom, Cell::new(br).with_fg(fg).with_bg(bg));

        // Horizontal edges
        for x in (left + 1)..right {
            self.put(x, top, Cell::new(h).with_fg(fg).with_bg(bg));
            self.put(x, bottom, Cell::new(h).with_fg(fg).with_bg(bg));
        }

        // Vertical edges
        for y in (top + 1)..bottom {
            self.put(left, y, Cell::new(v).with_fg(fg).with_bg(bg));
            self.put(right, y, Cell::new(v).with_fg(fg).with_bg(bg));
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
            let offset = available.saturating_sub(title_width).div_ceil(2);
            let x = rect.x + offset;
            if x + title_width <= rect.right() && rect.y < self.height {
                self.write_str(x, rect.y, title, title_fg, bg);
            }
        }
    }

    /// Draw a rounded panel with wrapped text content.
    ///
    /// Combines [`Self::draw_rounded_panel`] with [`write_wrapped_text`] to
    /// produce a bordered text box. The text is word-wrapped within the panel's
    /// inner area (inside the border). Returns the number of lines written.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_text_box(
        &mut self,
        rect: Rect,
        title: &str,
        text: &str,
        border_fg: Color,
        title_fg: Color,
        text_fg: Color,
        bg: Color,
    ) -> u16 {
        self.draw_rounded_panel(rect, title, border_fg, title_fg, bg);
        let inner = rect.inset(1, 1);
        if inner.is_empty() {
            return 0;
        }
        let lines = wrap_text(text, inner.width as usize);
        let max_lines = inner.height as usize;
        for (i, line) in lines.iter().enumerate() {
            if i >= max_lines {
                break;
            }
            self.write_str(inner.x, inner.y + i as u16, line, text_fg, bg);
        }
        lines.len().min(max_lines) as u16
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
            if x0 >= 0 && y0 >= 0 && self.put(x0 as u16, y0 as u16, cell) {
                count += 1;
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

    /// Convert the grid contents into a standalone vector SVG string.
    ///
    /// This is useful for high-fidelity vector snapshots, web embedding, or
    /// rendering terminal screens inside web browsers with absolute scalability.
    pub fn to_svg_string(&self) -> String {
        let cell_w = 9.0;
        let cell_h = 18.0;
        let svg_w = self.width as f32 * cell_w;
        let svg_h = self.height as f32 * cell_h;

        let mut out = String::with_capacity(self.cells.len() * 80 + 300);
        // SVG header
        out.push_str(&format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{:.1}" height="{:.1}" viewBox="0 0 {:.1} {:.1}">"#,
            svg_w, svg_h, svg_w, svg_h
        ));
        // Add styling for monospace text rendering
        out.push_str(r#"<style>text { font-family: monospace; font-size: 14px; text-anchor: middle; dominant-baseline: middle; }</style>"#);
        // Base black background for the entire grid
        out.push_str(&format!(
            r#"<rect width="{:.1}" height="{:.1}" fill="black"/>"#,
            svg_w, svg_h
        ));

        // Draw background rects for cells that aren't BLACK background
        for y in 0..self.height {
            for x in 0..self.width {
                let cell = &self.cells[(y as usize) * (self.width as usize) + (x as usize)];
                if cell.bg != Color::BLACK {
                    let rx = x as f32 * cell_w;
                    let ry = y as f32 * cell_h;
                    out.push_str(&format!(
                        r#"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" fill="rgb({},{},{})"/>"#,
                        rx, ry, cell_w, cell_h, cell.bg.0, cell.bg.1, cell.bg.2
                    ));
                }
            }
        }

        // Draw text characters
        for y in 0..self.height {
            for x in 0..self.width {
                let cell = &self.cells[(y as usize) * (self.width as usize) + (x as usize)];
                if cell.glyph != ' ' {
                    let cx = x as f32 * cell_w + cell_w / 2.0;
                    let cy = y as f32 * cell_h + cell_h / 2.0;
                    let escaped = match cell.glyph {
                        '&' => "&amp;",
                        '<' => "&lt;",
                        '>' => "&gt;",
                        '"' => "&quot;",
                        '\'' => "&apos;",
                        _ => {
                            if cell.glyph.is_control() {
                                " "
                            } else {
                                ""
                            }
                        }
                    };

                    let mut style = String::new();
                    if cell.attrs.bold {
                        style.push_str("font-weight:bold;");
                    } else if cell.attrs.dim {
                        style.push_str("opacity:0.6;");
                    }
                    if cell.attrs.italic {
                        style.push_str("font-style:italic;");
                    }
                    if cell.attrs.underline {
                        style.push_str("text-decoration:underline;");
                    }

                    let style_attr = if style.is_empty() {
                        "".to_string()
                    } else {
                        format!(" style=\"{}\"", style)
                    };

                    if escaped.is_empty() {
                        out.push_str(&format!(
                            r#"<text x="{:.1}" y="{:.1}" fill="rgb({},{},{})"{}>{}</text>"#,
                            cx, cy, cell.fg.0, cell.fg.1, cell.fg.2, style_attr, cell.glyph
                        ));
                    } else {
                        out.push_str(&format!(
                            r#"<text x="{:.1}" y="{:.1}" fill="rgb({},{},{})"{}>{}</text>"#,
                            cx, cy, cell.fg.0, cell.fg.1, cell.fg.2, style_attr, escaped
                        ));
                    }
                }
            }
        }

        out.push_str("</svg>");
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
        let mut d = 1 - radius as i32;

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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ColorPalette {
    pub name: String,
    pub background: Color,
    pub foreground: Color,
    pub primary: Color,
    pub secondary: Color,
    pub accent: Color,
    pub danger: Color,
    pub success: Color,
    pub info: Color,
    pub warning: Color,
    pub ui_border: Color,
    pub ui_title: Color,
    pub ui_text: Color,
    pub ui_highlight: Color,
    pub ui_muted: Color,
}

impl ColorPalette {
    /// A dark dungeon theme with muted earth tones.
    pub fn dark_dungeon() -> Self {
        Self {
            name: "dark_dungeon".to_string(),
            background: Color(15, 15, 20),
            foreground: Color(200, 200, 200),
            primary: Color(80, 80, 90),   // walls
            secondary: Color(50, 50, 55), // floor
            accent: Color(220, 200, 80),  // items
            danger: Color(220, 80, 80),   // hazards
            success: Color(80, 180, 220), // goal
            info: Color(100, 220, 100),   // player
            warning: Color(220, 150, 50),
            ui_border: Color(100, 100, 110),
            ui_title: Color(220, 200, 80),
            ui_text: Color(200, 200, 200),
            ui_highlight: Color(100, 220, 100),
            ui_muted: Color(100, 100, 100),
        }
    }

    /// A light theme suitable for bright terminals.
    pub fn light_classic() -> Self {
        Self {
            name: "light_classic".to_string(),
            background: Color(240, 240, 240),
            foreground: Color(30, 30, 30),
            primary: Color(120, 120, 120),
            secondary: Color(220, 220, 220),
            accent: Color(180, 140, 0),
            danger: Color(180, 30, 30),
            success: Color(0, 100, 180),
            info: Color(0, 120, 0),
            warning: Color(150, 80, 0),
            ui_border: Color(150, 150, 150),
            ui_title: Color(0, 80, 150),
            ui_text: Color(30, 30, 30),
            ui_highlight: Color(200, 230, 200),
            ui_muted: Color(180, 180, 180),
        }
    }

    /// A high-contrast amber-on-black theme reminiscent of vintage terminals.
    pub fn amber_terminal() -> Self {
        Self {
            name: "amber_terminal".to_string(),
            background: Color(10, 8, 5),
            foreground: Color(255, 180, 50),
            primary: Color(120, 90, 30),
            secondary: Color(40, 30, 15),
            accent: Color(255, 220, 100),
            danger: Color(255, 80, 50),
            success: Color(200, 255, 150),
            info: Color(255, 220, 100),
            warning: Color(255, 150, 50),
            ui_border: Color(180, 140, 40),
            ui_title: Color(255, 220, 100),
            ui_text: Color(255, 180, 50),
            ui_highlight: Color(255, 255, 150),
            ui_muted: Color(100, 80, 30),
        }
    }

    /// A cyberpunk neon theme with vivid colors on dark backgrounds.
    pub fn cyberpunk() -> Self {
        Self {
            name: "cyberpunk".to_string(),
            background: Color(10, 5, 20),
            foreground: Color(200, 200, 255),
            primary: Color(60, 30, 80),
            secondary: Color(20, 15, 35),
            accent: Color(255, 255, 0),
            danger: Color(255, 50, 100),
            success: Color(100, 100, 255),
            info: Color(0, 255, 200),
            warning: Color(255, 150, 50),
            ui_border: Color(80, 50, 120),
            ui_title: Color(0, 255, 200),
            ui_text: Color(200, 200, 255),
            ui_highlight: Color(0, 255, 200),
            ui_muted: Color(80, 60, 100),
        }
    }

    /// Create a cell with the secondary color as background.
    pub fn secondary_cell(&self, glyph: char) -> Cell {
        Cell::new(glyph)
            .with_fg(self.foreground)
            .with_bg(self.secondary)
    }

    /// Create a cell with the primary color.
    pub fn primary_cell(&self, glyph: char) -> Cell {
        Cell::new(glyph)
            .with_fg(self.primary)
            .with_bg(self.background)
    }

    /// Create a cell for the info character (e.g. player).
    pub fn info_cell(&self, glyph: char) -> Cell {
        Cell::new(glyph).with_fg(self.info).with_bg(self.secondary)
    }

    /// Create a cell for a danger tile (e.g. hazard).
    pub fn danger_cell(&self, glyph: char) -> Cell {
        Cell::new(glyph)
            .with_fg(self.danger)
            .with_bg(self.secondary)
    }

    /// Create a cell for an accent tile (e.g. item).
    pub fn accent_cell(&self, glyph: char) -> Cell {
        Cell::new(glyph)
            .with_fg(self.accent)
            .with_bg(self.secondary)
    }

    /// Create a cell for the success tile (e.g. goal).
    pub fn success_cell(&self, glyph: char) -> Cell {
        Cell::new(glyph)
            .with_fg(self.success)
            .with_bg(self.secondary)
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

/// Visual fidelity tiers for adaptive resolution rendering.
///
/// Tier selection is based on terminal dimensions (columns and rows).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ResolutionTier {
    #[default]
    TINY, // 6x8
    SMALL,  // 8x12
    MEDIUM, // 12x16
    LARGE,  // 16x20
    XLARGE, // 20x24
    ULTRA,  // 28x32
}

impl ResolutionTier {
    pub const ALL: [ResolutionTier; 6] = [
        ResolutionTier::TINY,
        ResolutionTier::SMALL,
        ResolutionTier::MEDIUM,
        ResolutionTier::LARGE,
        ResolutionTier::XLARGE,
        ResolutionTier::ULTRA,
    ];

    /// Select the appropriate tier for a given terminal width and height.
    pub fn from_size(width: u16, height: u16) -> Self {
        if width >= 160 && height >= 48 {
            ResolutionTier::ULTRA
        } else if width >= 140 && height >= 42 {
            ResolutionTier::XLARGE
        } else if width >= 120 && height >= 36 {
            ResolutionTier::LARGE
        } else if width >= 100 && height >= 30 {
            ResolutionTier::MEDIUM
        } else if width >= 80 && height >= 24 {
            ResolutionTier::SMALL
        } else {
            ResolutionTier::TINY
        }
    }

    /// Returns the target sprite width and height for this tier.
    pub fn sprite_size(self) -> (u16, u16) {
        match self {
            ResolutionTier::TINY => (6, 4), // 6x8 pixels packed into 6x4 half-blocks
            ResolutionTier::SMALL => (8, 6), // 8x12 pixels -> 8x6 cells
            ResolutionTier::MEDIUM => (12, 8), // 12x16 pixels -> 12x8 cells
            ResolutionTier::LARGE => (16, 10), // 16x20 pixels -> 16x10 cells
            ResolutionTier::XLARGE => (20, 12), // 20x24 pixels -> 20x12 cells
            ResolutionTier::ULTRA => (28, 16), // 28x32 pixels -> 28x16 cells
        }
    }
}

/// A single animation frame: a [`Grid`] with an associated display duration.
///
/// Frames are used in [`Sprite`] and [`SpriteSheet`] to build animated
/// terminal graphics. The duration is in arbitrary ticks — the game loop
/// decides how many ticks each frame should display.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
/// Verryte supports adaptive resolution: a sprite can hold different frame
/// sequences for different [`ResolutionTier`]s.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Sprite {
    pub name: String,
    tiers: std::collections::BTreeMap<ResolutionTier, Vec<Frame>>,
    current_tier: ResolutionTier,
    current_frame: usize,
    elapsed: u32,
    paused: bool,
}

impl Sprite {
    pub fn new<S: Into<String>>(name: S, frames: Vec<Frame>) -> Self {
        let mut tiers = std::collections::BTreeMap::new();
        tiers.insert(ResolutionTier::default(), frames);
        Self {
            name: name.into(),
            tiers,
            current_tier: ResolutionTier::default(),
            current_frame: 0,
            elapsed: 0,
            paused: false,
        }
    }

    /// Add a frame sequence for a specific resolution tier.
    pub fn with_tier(mut self, tier: ResolutionTier, frames: Vec<Frame>) -> Self {
        self.tiers.insert(tier, frames);
        self
    }

    /// Set the current resolution tier. If the tier is not available,
    /// it falls back to the nearest available lower tier.
    pub fn set_tier(&mut self, tier: ResolutionTier) {
        if self.tiers.contains_key(&tier) {
            self.current_tier = tier;
        } else {
            // Fallback to highest available tier that is <= requested tier.
            if let Some((&t, _)) = self.tiers.range(..=tier).next_back() {
                self.current_tier = t;
            } else if let Some((&t, _)) = self.tiers.range(tier..).next() {
                // Or highest if none are lower.
                self.current_tier = t;
            }
        }
        // Reset current frame if out of bounds for the new tier.
        if let Some(frames) = self.tiers.get(&self.current_tier) {
            if self.current_frame >= frames.len() {
                self.current_frame = 0;
                self.elapsed = 0;
            }
        }
    }

    /// Advance the sprite by one tick. Returns `true` if the frame changed.
    pub fn tick(&mut self) -> bool {
        if self.paused {
            return false;
        }
        let Some(frames) = self.tiers.get(&self.current_tier) else {
            return false;
        };
        if frames.is_empty() {
            return false;
        }

        self.elapsed += 1;
        if self.elapsed >= frames[self.current_frame].duration {
            self.elapsed = 0;
            self.current_frame = (self.current_frame + 1) % frames.len();
            true
        } else {
            false
        }
    }

    /// Get the current frame's grid.
    pub fn current_frame(&self) -> &Grid {
        let frames = self.tiers.get(&self.current_tier).expect("tier must exist");
        &frames[self.current_frame].grid
    }

    /// Get the current frame index.
    pub fn current_index(&self) -> usize {
        self.current_frame
    }

    /// Get the total number of frames in the current tier.
    pub fn frame_count(&self) -> usize {
        self.tiers
            .get(&self.current_tier)
            .map(|f| f.len())
            .unwrap_or(0)
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
        self.current_frame = 0;
        self.elapsed = 0;
    }

    /// Jump to a specific frame. Clamped to valid range.
    pub fn set_frame(&mut self, index: usize) {
        if let Some(frames) = self.tiers.get(&self.current_tier) {
            if !frames.is_empty() {
                self.current_frame = index.min(frames.len() - 1);
                self.elapsed = 0;
            }
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

/// Translates a graphical image into a terminal cell grid using half-block characters (`▀`).
/// Each cell represents two vertical pixels: the foreground color matches the top pixel,
/// and the background color matches the bottom pixel.
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
///
/// Pixels matching the `chroma_key` color (within `tolerance`) become transparent.
/// When both top and bottom pixels are transparent, the cell becomes `Cell::EMPTY`.
/// When only the top pixel is transparent, the cell uses `▄` with the bottom color as fg.
/// When only the bottom pixel is transparent, the cell uses `▀` with the top color as fg.
///
/// This is useful for loading character sprites with white/transparent backgrounds.
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
    SingleCell { glyph: char, fg: Color, bg: Color },
    BlockSprite(Grid),
    Animated(Sprite),
}

impl VisualAsset {
    /// Render the asset at a given resolution tier.
    pub fn render(&self, tier: ResolutionTier) -> Grid {
        match self {
            VisualAsset::SingleCell { glyph, fg, bg } => {
                let mut grid = Grid::new(1, 1);
                grid.put(0, 0, Cell::new(*glyph).with_fg(*fg).with_bg(*bg));
                grid
            }
            VisualAsset::BlockSprite(grid) => grid.clone(),
            VisualAsset::Animated(sprite) => {
                let mut s = sprite.clone();
                s.set_tier(tier);
                s.current_frame().clone()
            }
        }
    }
}

/// A registry mapping semantic keys to visual assets.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VisualRegistry {
    assets: std::collections::HashMap<String, VisualAsset>,
}

impl VisualRegistry {
    pub fn new() -> Self {
        Self {
            assets: std::collections::HashMap::new(),
        }
    }

    pub fn register(&mut self, name: &str, asset: VisualAsset) {
        self.assets.insert(name.to_string(), asset);
    }

    pub fn register_single_cell(&mut self, name: &str, glyph: char, fg: Color, bg: Color) {
        self.register(name, VisualAsset::SingleCell { glyph, fg, bg });
    }

    pub fn get(&self, name: &str) -> Option<&VisualAsset> {
        self.assets.get(name)
    }
}

impl Default for VisualRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_splits() {
        let r = Rect::new(10, 20, 30, 40);

        // relative horizontal split
        let (top, bottom) = r.split_horizontal(15);
        assert_eq!(top, Rect::new(10, 20, 30, 15));
        assert_eq!(bottom, Rect::new(10, 35, 30, 25));

        // relative horizontal split clamp
        let (top2, bottom2) = r.split_horizontal(50);
        assert_eq!(top2, Rect::new(10, 20, 30, 40));
        assert_eq!(bottom2, Rect::new(10, 60, 30, 0));

        // relative vertical split
        let (left, right) = r.split_vertical(10);
        assert_eq!(left, Rect::new(10, 20, 10, 40));
        assert_eq!(right, Rect::new(20, 20, 20, 40));

        // absolute horizontal split
        let (top_a, bottom_a) = r.split_horizontal_absolute(25);
        assert_eq!(top_a, Rect::new(10, 20, 30, 5));
        assert_eq!(bottom_a, Rect::new(10, 25, 30, 35));

        // absolute vertical split
        let (left_a, right_a) = r.split_vertical_absolute(15);
        assert_eq!(left_a, Rect::new(10, 20, 5, 40));
        assert_eq!(right_a, Rect::new(15, 20, 25, 40));
    }

    #[test]
    fn test_color_operations() {
        // lerp
        let c1 = Color(0, 100, 200);
        let c2 = Color(100, 200, 50);
        let mid = c1.lerp(c2, 0.5);
        assert_eq!(mid, Color(50, 150, 125));

        // from_hex
        assert_eq!(Color::from_hex("#FF00FF").unwrap(), Color(255, 0, 255));
        assert_eq!(Color::from_hex("00FF00").unwrap(), Color(0, 255, 0));
        assert!(Color::from_hex("invalid").is_err());

        // HSV conversions
        let color = Color(128, 64, 192);
        let (h, s, v) = color.to_hsv();
        let back = Color::from_hsv(h, s, v);
        // allow small floating point roundoffs
        assert!((back.0 as i16 - color.0 as i16).abs() <= 1);
        assert!((back.1 as i16 - color.1 as i16).abs() <= 1);
        assert!((back.2 as i16 - color.2 as i16).abs() <= 1);
    }

    #[test]
    fn test_camera_clamp_to_bounds() {
        let mut camera = Camera::new(10.0, 10.0);
        camera.clamp_to_bounds(0.0, 0.0, 5.0, 5.0);
        assert_eq!(camera.center_x, 5.0);
        assert_eq!(camera.center_y, 5.0);
        assert_eq!(camera.target_x, 5.0);
        assert_eq!(camera.target_y, 5.0);

        let mut camera2 = Camera::new(-2.0, 12.0).with_smooth(0.5);
        camera2.look_at(20.0, 20.0);
        camera2.clamp_to_bounds(0.0, 0.0, 15.0, 15.0);
        assert_eq!(camera2.center_x, 0.0);
        assert_eq!(camera2.center_y, 12.0);
        assert_eq!(camera2.target_x, 15.0);
        assert_eq!(camera2.target_y, 15.0);
    }

    #[test]
    fn test_image_to_grid_conversion() {
        use image::{DynamicImage, Rgb, RgbImage};
        let mut img_buf = RgbImage::new(2, 4);
        // Column 0, top pair: (0,0) red, (0,1) green
        img_buf.put_pixel(0, 0, Rgb([255, 0, 0]));
        img_buf.put_pixel(0, 1, Rgb([0, 255, 0]));
        // Column 0, bottom pair: (0,2) blue, (0,3) yellow
        img_buf.put_pixel(0, 2, Rgb([0, 0, 255]));
        img_buf.put_pixel(0, 3, Rgb([255, 255, 0]));

        // Column 1, top pair: (1,0) white, (1,1) black
        img_buf.put_pixel(1, 0, Rgb([255, 255, 255]));
        img_buf.put_pixel(1, 1, Rgb([0, 0, 0]));
        // Column 1, bottom pair: (1,2) cyan, (1,3) magenta
        img_buf.put_pixel(1, 2, Rgb([0, 255, 255]));
        img_buf.put_pixel(1, 3, Rgb([255, 0, 255]));

        let img = DynamicImage::ImageRgb8(img_buf);
        let grid = image_to_grid(&img);

        assert_eq!(grid.width(), 2);
        assert_eq!(grid.height(), 2);

        // Check cell at (0, 0)
        let cell_0_0 = grid.get(0, 0).unwrap();
        assert_eq!(cell_0_0.glyph, '▀');
        assert_eq!(cell_0_0.fg, Color(255, 0, 0));
        assert_eq!(cell_0_0.bg, Color(0, 255, 0));

        // Check cell at (0, 1)
        let cell_0_1 = grid.get(0, 1).unwrap();
        assert_eq!(cell_0_1.glyph, '▀');
        assert_eq!(cell_0_1.fg, Color(0, 0, 255));
        assert_eq!(cell_0_1.bg, Color(255, 255, 0));

        // Check cell at (1, 0)
        let cell_1_0 = grid.get(1, 0).unwrap();
        assert_eq!(cell_1_0.glyph, '▀');
        assert_eq!(cell_1_0.fg, Color(255, 255, 255));
        assert_eq!(cell_1_0.bg, Color(0, 0, 0));

        // Check cell at (1, 1)
        let cell_1_1 = grid.get(1, 1).unwrap();
        assert_eq!(cell_1_1.glyph, '▀');
        assert_eq!(cell_1_1.fg, Color(0, 255, 255));
        assert_eq!(cell_1_1.bg, Color(255, 0, 255));
    }

    #[test]
    fn test_image_to_grid_with_chroma_key() {
        use image::{DynamicImage, Rgb, RgbImage};
        let mut img_buf = RgbImage::new(2, 4);
        // Column 0: red/green (no transparency)
        img_buf.put_pixel(0, 0, Rgb([255, 0, 0]));
        img_buf.put_pixel(0, 1, Rgb([0, 255, 0]));
        // Column 0 bottom: blue/white (white = chroma key)
        img_buf.put_pixel(0, 2, Rgb([0, 0, 255]));
        img_buf.put_pixel(0, 3, Rgb([255, 255, 255]));

        // Column 1: white/black (top = chroma key)
        img_buf.put_pixel(1, 0, Rgb([255, 255, 255]));
        img_buf.put_pixel(1, 1, Rgb([0, 0, 0]));
        // Column 1 bottom: white/white (both = chroma key)
        img_buf.put_pixel(1, 2, Rgb([255, 255, 255]));
        img_buf.put_pixel(1, 3, Rgb([255, 255, 255]));

        let img = DynamicImage::ImageRgb8(img_buf);
        let white = Color(255, 255, 255);
        let grid = image_to_grid_with_chroma_key(&img, white, 30);

        assert_eq!(grid.width(), 2);
        assert_eq!(grid.height(), 2);

        // (0,0): red top, green bottom — no transparency
        let c = grid.get(0, 0).unwrap();
        assert_eq!(c.glyph, '▀');
        assert_eq!(c.fg, Color(255, 0, 0));
        assert_eq!(c.bg, Color(0, 255, 0));

        // (0,1): blue top, white bottom — bottom is transparent, use ▀
        let c = grid.get(0, 1).unwrap();
        assert_eq!(c.glyph, '▀');
        assert_eq!(c.fg, Color(0, 0, 255));
        assert_eq!(c.bg, Color::BLACK);

        // (1,0): white top, black bottom — top is transparent, use ▄
        let c = grid.get(1, 0).unwrap();
        assert_eq!(c.glyph, '▄');
        assert_eq!(c.fg, Color(0, 0, 0));
        assert_eq!(c.bg, Color::BLACK);

        // (1,1): white/white — both transparent, Cell::EMPTY
        let c = grid.get(1, 1).unwrap();
        assert!(c.is_transparent());
    }

    #[test]
    fn test_visual_registry_lookups() {
        let mut registry = VisualRegistry::new();
        registry.register_single_cell("test", '@', Color::RED, Color::BLACK);

        let grid = Grid::new(2, 2);
        registry.register("block", VisualAsset::BlockSprite(grid.clone()));

        if let Some(VisualAsset::SingleCell { glyph, fg, bg }) = registry.get("test") {
            assert_eq!(*glyph, '@');
            assert_eq!(*fg, Color::RED);
            assert_eq!(*bg, Color::BLACK);
        } else {
            panic!("Expected SingleCell");
        }

        if let Some(VisualAsset::BlockSprite(g)) = registry.get("block") {
            assert_eq!(g.width(), 2);
            assert_eq!(g.height(), 2);
        } else {
            panic!("Expected BlockSprite");
        }
    }

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
    fn write_lines_clips_to_bottom_edge() {
        let mut grid = Grid::new(4, 2);
        let written = grid.write_lines(0, 0, ["one", "two", "three"], Color::WHITE, Color::BLACK);
        assert_eq!(written, 2);
        assert_eq!(grid.to_plain_string(), "one \ntwo ");
    }

    #[test]
    fn write_lines_returns_zero_when_starting_below_grid() {
        let mut grid = Grid::new(4, 2);
        let written = grid.write_lines(0, 5, ["nope"], Color::WHITE, Color::BLACK);
        assert_eq!(written, 0);
        assert_eq!(grid.to_plain_string(), "    \n    ");
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
    fn draw_border_styled_uses_various_styles() {
        let mut grid = Grid::new(6, 5);

        // Test Ascii style
        grid.draw_border_styled(
            Rect::new(1, 1, 4, 3),
            BorderStyle::Ascii,
            Color::WHITE,
            Color::BLACK,
        );
        let s = grid.to_plain_string();
        let lines: Vec<&str> = s.lines().collect();
        assert!(lines[1].contains('+'));
        assert!(lines[1].contains('-'));
        assert!(lines[2].contains('|'));

        // Test Double style
        grid.clear(Cell::EMPTY);
        grid.draw_border_styled(
            Rect::new(1, 1, 4, 3),
            BorderStyle::Double,
            Color::WHITE,
            Color::BLACK,
        );
        let s = grid.to_plain_string();
        let lines: Vec<&str> = s.lines().collect();
        assert!(lines[1].contains('\u{2554}')); // ╔
        assert!(lines[1].contains('\u{2550}')); // ═
        assert!(lines[2].contains('\u{2551}')); // ║

        // Test Heavy style
        grid.clear(Cell::EMPTY);
        grid.draw_border_styled(
            Rect::new(1, 1, 4, 3),
            BorderStyle::Heavy,
            Color::WHITE,
            Color::BLACK,
        );
        let s = grid.to_plain_string();
        let lines: Vec<&str> = s.lines().collect();
        assert!(lines[1].contains('\u{250F}')); // ┏
        assert!(lines[1].contains('\u{2501}')); // ━
        assert!(lines[2].contains('\u{2503}')); // ┃

        // Test Rounded style
        grid.clear(Cell::EMPTY);
        grid.draw_border_styled(
            Rect::new(1, 1, 4, 3),
            BorderStyle::Rounded,
            Color::WHITE,
            Color::BLACK,
        );
        let s = grid.to_plain_string();
        let lines: Vec<&str> = s.lines().collect();
        assert!(lines[1].contains('\u{256D}')); // ╭
        assert!(lines[1].contains('\u{2500}')); // ─
        assert!(lines[2].contains('\u{2502}')); // │
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
    fn draw_text_box_renders_wrapped_content() {
        let mut grid = Grid::new(20, 8);
        grid.draw_text_box(
            Rect::new(0, 0, 20, 8),
            "TITLE",
            "Hello world this is a test of text wrapping inside a box.",
            Color::WHITE,
            Color::YELLOW,
            Color::WHITE,
            Color::BLACK,
        );
        let s = grid.to_plain_string();
        let lines: Vec<&str> = s.lines().collect();
        // Should have a border at top and bottom
        assert!(lines[0].contains(Grid::BORDER_TL));
        assert!(lines[7].contains(Grid::BORDER_BL));
        // Text should be present somewhere inside
        assert!(s.contains("Hello"));
    }

    #[test]
    fn draw_text_box_handles_empty_text() {
        let mut grid = Grid::new(10, 5);
        let lines = grid.draw_text_box(
            Rect::new(0, 0, 10, 5),
            "OK",
            "",
            Color::WHITE,
            Color::YELLOW,
            Color::WHITE,
            Color::BLACK,
        );
        // wrap_text("") produces one empty line
        assert_eq!(lines, 1);
        // Border should still be drawn
        let s = grid.to_plain_string();
        assert!(s.contains(Grid::BORDER_TL));
    }

    #[test]
    fn draw_text_box_clips_to_rect() {
        let mut grid = Grid::new(15, 4);
        let lines = grid.draw_text_box(
            Rect::new(0, 0, 15, 4),
            "",
            "This is a long text that should be clipped to the available space within the box.",
            Color::WHITE,
            Color::YELLOW,
            Color::WHITE,
            Color::BLACK,
        );
        // Inner height is 2 (4 - 2 for borders)
        assert!(lines <= 2);
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
    fn to_svg_string_renders_correctly() {
        let mut grid = Grid::new(3, 1);
        grid.put(0, 0, Cell::new('R').with_fg(Color::RED));
        grid.put(
            1,
            0,
            Cell::new('G').with_fg(Color::GREEN).with_bg(Color::WHITE),
        );
        grid.put(2, 0, Cell::new('<').with_fg(Color::BLUE));

        let svg = grid.to_svg_string();
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("width=\"27.0\""));
        assert!(svg.contains("height=\"18.0\""));
        assert!(svg.contains("fill=\"black\""));
        assert!(svg.contains("rgb(220,60,60)")); // Red fg
        assert!(svg.contains("rgb(80,200,120)")); // Green fg
        assert!(svg.contains("rgb(230,230,230)")); // White bg
        assert!(svg.contains("&lt;")); // Escaped '<'
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
                name: "hidden".to_string(),
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
        assert_eq!(p.info, Color(100, 220, 100));
        assert_eq!(p.danger, Color(220, 80, 80));
    }

    #[test]
    fn color_palette_creates_cells_with_correct_colors() {
        let p = ColorPalette::dark_dungeon();
        let player = p.info_cell('@');
        assert_eq!(player.glyph, '@');
        assert_eq!(player.fg, p.info);
        assert_eq!(player.bg, p.secondary);

        let wall = p.primary_cell('#');
        assert_eq!(wall.fg, p.primary);
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

        let names: Vec<&str> = layers.iter().map(|l| l.name.as_str()).collect();
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
    fn rect_inset_shrinks_rect() {
        let r = Rect::new(1, 2, 10, 8);
        let inner = r.inset(1, 2);
        assert_eq!(inner.x, 2);
        assert_eq!(inner.y, 4);
        assert_eq!(inner.width, 8);
        assert_eq!(inner.height, 4);
    }

    #[test]
    fn rect_inset_clamps_to_empty() {
        let r = Rect::new(0, 0, 4, 4);
        let inner = r.inset(3, 3);
        assert_eq!(inner.width, 0);
        assert_eq!(inner.height, 0);
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

    #[test]
    fn color_display() {
        assert_eq!(format!("{}", Color(255, 128, 0)), "#FF8000");
        assert_eq!(format!("{}", Color::BLACK), "#000000");
        assert_eq!(format!("{}", Color::WHITE), "#E6E6E6");
    }

    #[test]
    fn color_from_tuple_roundtrip() {
        let c = Color::from((100, 200, 50));
        assert_eq!(c, Color(100, 200, 50));
        let (r, g, b): (u8, u8, u8) = c.into();
        assert_eq!((r, g, b), (100, 200, 50));
    }

    #[test]
    fn rect_display() {
        let r = Rect::new(10, 5, 80, 24);
        assert_eq!(format!("{}", r), "Rect(10,5 80x24)");
    }

    #[test]
    fn rect_from_tuple() {
        let r = Rect::from((1, 2, 3, 4));
        assert_eq!(r, Rect::new(1, 2, 3, 4));
    }

    #[test]
    fn alignment_display() {
        assert_eq!(format!("{}", Alignment::Left), "Left");
        assert_eq!(format!("{}", Alignment::Center), "Center");
        assert_eq!(format!("{}", Alignment::Right), "Right");
    }

    #[test]
    fn cell_attrs_getters() {
        let none = CellAttrs::NONE;
        assert!(!none.is_bold());
        assert!(!none.is_underline());
        assert!(!none.is_dim());
        assert!(!none.is_italic());
        assert!(!none.is_reverse());
        assert!(!none.is_blink());
        assert!(none.is_empty());

        let bold_italic = CellAttrs::NONE.bold().italic();
        assert!(bold_italic.is_bold());
        assert!(bold_italic.is_italic());
        assert!(!bold_italic.is_underline());
        assert!(!bold_italic.is_empty());
    }

    #[test]
    fn test_grid_write_rich_simple() {
        let mut grid = Grid::new(10, 1);
        let written = grid.write_rich(0, 0, "[b]hello[/b]").unwrap();
        assert_eq!(written, 5);
        let cell = grid.get(0, 0).unwrap();
        assert_eq!(cell.glyph, 'h');
        assert!(cell.attrs.is_bold());
    }

    #[test]
    fn test_grid_write_rich_nested() {
        let mut grid = Grid::new(20, 1);
        let written = grid
            .write_rich(0, 0, "[b]bold [i]italic[/i] bold[/b]")
            .unwrap();
        assert_eq!(written, 16);

        // "bold "
        let c1 = grid.get(0, 0).unwrap();
        assert!(c1.attrs.is_bold());
        assert!(!c1.attrs.is_italic());

        // "italic"
        let c2 = grid.get(5, 0).unwrap();
        assert!(c2.attrs.is_bold());
        assert!(c2.attrs.is_italic());

        // " bold"
        let c3 = grid.get(12, 0).unwrap();
        assert!(c3.attrs.is_bold());
        assert!(!c3.attrs.is_italic());
    }

    #[test]
    fn test_grid_write_rich_colors() {
        let mut grid = Grid::new(10, 1);
        grid.write_rich(0, 0, "[fg:#FF0000]red[bg:blue]blue[/bg][/fg]")
            .unwrap();

        let c_red = grid.get(0, 0).unwrap();
        assert_eq!(c_red.fg, Color(255, 0, 0));
        assert_eq!(c_red.bg, Color::BLACK);

        let c_blue = grid.get(3, 0).unwrap();
        assert_eq!(c_blue.fg, Color(255, 0, 0));
        assert_eq!(c_blue.bg, Color::BLUE);
    }

    #[test]
    fn test_grid_write_rich_reset() {
        let mut grid = Grid::new(10, 1);
        grid.write_rich(0, 0, "[b][fg:red]test[/]plain").unwrap();

        let c_test = grid.get(0, 0).unwrap();
        assert!(c_test.attrs.is_bold());
        assert_eq!(c_test.fg, Color::RED);

        let c_plain = grid.get(4, 0).unwrap();
        assert!(!c_plain.attrs.is_bold());
        assert_eq!(c_plain.fg, Color::WHITE);
    }

    #[test]
    fn test_grid_write_rich_escaped() {
        let mut grid = Grid::new(10, 1);
        grid.write_rich(0, 0, "[[hello").unwrap();
        let cell = grid.get(0, 0).unwrap();
        assert_eq!(cell.glyph, '[');
    }

    #[test]
    fn test_grid_write_rich_errors() {
        let mut grid = Grid::new(10, 1);
        assert!(grid.write_rich(0, 0, "[unclosed").is_err());
        assert!(grid.write_rich(0, 0, "[fg:invalid]").is_err());
        assert!(grid.write_rich(0, 0, "[invalid_tag]text").is_err());
    }
}
