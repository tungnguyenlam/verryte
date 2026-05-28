use crate::color::Color;
use crate::layout::{Alignment, BorderStyle, Rect};

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

    pub fn is_empty(self) -> bool {
        self == Self::NONE
    }

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

    pub fn bold(mut self) -> Self {
        self.attrs = self.attrs.bold();
        self
    }

    pub fn dim(mut self) -> Self {
        self.attrs = self.attrs.dim();
        self
    }

    pub fn italic(mut self) -> Self {
        self.attrs = self.attrs.italic();
        self
    }

    pub fn underline(mut self) -> Self {
        self.attrs = self.attrs.underline();
        self
    }

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

/// A fixed-size rectangular cell buffer.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Grid {
    width: u16,
    height: u16,
    cells: Vec<Cell>,
}

impl Grid {
    pub const BORDER_TL: char = '\u{256D}'; // ╭
    pub const BORDER_TR: char = '\u{256E}'; // ╮
    pub const BORDER_BL: char = '\u{2570}'; // ╰
    pub const BORDER_BR: char = '\u{256F}'; // ╯
    pub const BORDER_H: char = '\u{2500}'; // ─
    pub const BORDER_V: char = '\u{2502}'; // │

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

    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    pub fn row(&self, y: u16) -> Option<&[Cell]> {
        if y < self.height {
            let start = (y as usize) * (self.width as usize);
            Some(&self.cells[start..start + self.width as usize])
        } else {
            None
        }
    }

    pub fn row_mut(&mut self, y: u16) -> Option<&mut [Cell]> {
        if y < self.height {
            let start = (y as usize) * (self.width as usize);
            let end = start + self.width as usize;
            Some(&mut self.cells[start..end])
        } else {
            None
        }
    }

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

    pub fn fill_row(&mut self, y: u16, cell: Cell) -> bool {
        if let Some(row) = self.row_mut(y) {
            row.fill(cell);
            true
        } else {
            false
        }
    }

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

    pub fn iter_cells(&self) -> impl Iterator<Item = (u16, u16, &Cell)> + '_ {
        self.cells.iter().enumerate().map(move |(i, cell)| {
            let x = (i % self.width as usize) as u16;
            let y = (i / self.width as usize) as u16;
            (x, y, cell)
        })
    }

    pub fn find_cell<F>(&self, mut f: F) -> Option<(u16, u16, &Cell)>
    where
        F: FnMut(&Cell) -> bool,
    {
        self.iter_cells().find(|(_, _, cell)| f(cell))
    }

    pub fn find_all_cells<F>(&self, mut f: F) -> Vec<(u16, u16, &Cell)>
    where
        F: FnMut(&Cell) -> bool,
    {
        self.iter_cells().filter(|(_, _, cell)| f(cell)).collect()
    }

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

    pub fn put(&mut self, x: u16, y: u16, cell: Cell) -> bool {
        if let Some(i) = self.index(x, y) {
            self.cells[i] = cell;
            true
        } else {
            false
        }
    }

    pub fn swap_cells(&mut self, x1: u16, y1: u16, x2: u16, y2: u16) -> bool {
        let i1 = self.index(x1, y1);
        let i2 = self.index(x2, y2);
        if let (Some(i1), Some(i2)) = (i1, i2) {
            self.cells.swap(i1, i2);
            true
        } else {
            false
        }
    }

    /// Apply a generic filter closure to all cells within the given `rect`.
    pub fn apply_filter<F>(&mut self, rect: Rect, filter: F)
    where
        F: Fn(&mut Cell),
    {
        let x_start = rect.x;
        let y_start = rect.y;
        let x_end = rect.right().min(self.width);
        let y_end = rect.bottom().min(self.height);

        for y in y_start..y_end {
            for x in x_start..x_end {
                if let Some(cell) = self.get_mut(x, y) {
                    filter(cell);
                }
            }
        }
    }

    /// Blur the foreground and background colors of cells within the given `rect`
    /// using a box blur of the specified `radius`.
    #[allow(clippy::manual_checked_ops)]
    pub fn apply_blur(&mut self, rect: Rect, radius: usize) {
        if radius == 0 {
            return;
        }
        let temp = self.clone();
        let x_start = rect.x;
        let y_start = rect.y;
        let x_end = rect.right().min(self.width);
        let y_end = rect.bottom().min(self.height);

        for y in y_start..y_end {
            for x in x_start..x_end {
                let mut sum_fg_r = 0u32;
                let mut sum_fg_g = 0u32;
                let mut sum_fg_b = 0u32;
                let mut sum_bg_r = 0u32;
                let mut sum_bg_g = 0u32;
                let mut sum_bg_b = 0u32;
                let mut count = 0u32;

                // Box boundaries
                let ny_start = y.saturating_sub(radius as u16);
                let ny_end = (y + radius as u16 + 1).min(self.height);
                let nx_start = x.saturating_sub(radius as u16);
                let nx_end = (x + radius as u16 + 1).min(self.width);

                for ny in ny_start..ny_end {
                    for nx in nx_start..nx_end {
                        if let Some(cell) = temp.get(nx, ny) {
                            sum_fg_r += cell.fg.0 as u32;
                            sum_fg_g += cell.fg.1 as u32;
                            sum_fg_b += cell.fg.2 as u32;
                            sum_bg_r += cell.bg.0 as u32;
                            sum_bg_g += cell.bg.1 as u32;
                            sum_bg_b += cell.bg.2 as u32;
                            count += 1;
                        }
                    }
                }

                if count > 0 {
                    if let Some(cell) = self.get_mut(x, y) {
                        cell.fg = Color(
                            (sum_fg_r / count) as u8,
                            (sum_fg_g / count) as u8,
                            (sum_fg_b / count) as u8,
                        );
                        cell.bg = Color(
                            (sum_bg_r / count) as u8,
                            (sum_bg_g / count) as u8,
                            (sum_bg_b / count) as u8,
                        );
                    }
                }
            }
        }
    }

    /// Tint the colors of cells within the given `rect` with a specified `tint` color
    /// and `alpha` intensity (0.0 to 1.0) using the specified `BlendMode`.
    pub fn apply_tint(
        &mut self,
        rect: Rect,
        tint: Color,
        alpha: f32,
        mode: crate::color::BlendMode,
    ) {
        let alpha = alpha.clamp(0.0, 1.0);
        if alpha <= 0.0 {
            return;
        }
        self.apply_filter(rect, |cell| {
            let blended_fg = cell.fg.blend(tint, mode);
            cell.fg = cell.fg.blend_alpha(blended_fg, alpha);
            let blended_bg = cell.bg.blend(tint, mode);
            cell.bg = cell.bg.blend_alpha(blended_bg, alpha);
        });
    }

    /// Adjust Hue, Saturation, and Value (Value/Brightness multiplier) of cells within the given `rect`.
    pub fn adjust_hsv(&mut self, rect: Rect, h_shift: f32, s_mult: f32, v_mult: f32) {
        self.apply_filter(rect, |cell| {
            // Apply to FG
            let (h, s, v) = cell.fg.to_hsv();
            cell.fg = Color::from_hsv(h + h_shift, s * s_mult, v * v_mult);

            // Apply to BG
            let (h, s, v) = cell.bg.to_hsv();
            cell.bg = Color::from_hsv(h + h_shift, s * s_mult, v * v_mult);
        });
    }

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

    pub fn blit(&mut self, other: &Grid, x: i32, y: i32) {
        for (ox, oy, cell) in other.iter_cells() {
            if cell.is_transparent() {
                continue;
            }
            let tx = x + ox as i32;
            let ty = y + oy as i32;
            if tx >= 0 && ty >= 0 && (tx as u16) < self.width && (ty as u16) < self.height {
                self.put(tx as u16, ty as u16, *cell);
            }
        }
    }

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

    pub fn blit_alpha(&mut self, other: &Grid, x: u16, y: u16, alpha: f32) {
        let alpha = alpha.clamp(0.0, 1.0);
        if alpha <= 0.0 {
            return;
        }
        for (ox, oy, cell) in other.iter_cells() {
            if cell.is_transparent() {
                continue;
            }
            let tx = x.saturating_add(ox);
            let ty = y.saturating_add(oy);
            if let Some(target) = self.get_mut(tx, ty) {
                if alpha >= 1.0 {
                    *target = *cell;
                } else {
                    target.fg = target.fg.blend_alpha(cell.fg, alpha);
                    target.bg = target.bg.blend_alpha(cell.bg, alpha);
                    if alpha > 0.5 {
                        target.glyph = cell.glyph;
                        target.attrs = cell.attrs;
                    }
                }
            }
        }
    }

    pub fn tint(&mut self, color: Color, alpha: f32) {
        let alpha = alpha.clamp(0.0, 1.0);
        if alpha <= 0.0 {
            return;
        }
        for cell in &mut self.cells {
            cell.fg = cell.fg.blend_alpha(color, alpha * 0.5);
            cell.bg = cell.bg.blend_alpha(color, alpha);
        }
    }

    pub fn write_str(&mut self, x: u16, y: u16, text: &str, fg: Color, bg: Color) {
        let mut cx = x;
        for ch in text.chars() {
            if cx >= self.width {
                break;
            }
            self.put(cx, y, Cell::new(ch).with_fg(fg).with_bg(bg));
            cx = cx.saturating_add(1);
        }
    }

    pub fn write_lines<S: AsRef<str>>(
        &mut self,
        x: u16,
        y: u16,
        lines: impl IntoIterator<Item = S>,
        fg: Color,
        bg: Color,
    ) -> u16 {
        let mut count = 0;
        for (i, line) in lines.into_iter().enumerate() {
            let ly = y.saturating_add(i as u16);
            if ly >= self.height {
                break;
            }
            self.write_str(x, ly, line.as_ref(), fg, bg);
            count += 1;
        }
        count
    }

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
        let text_len = text.chars().count() as u16;
        let start_x = match alignment {
            Alignment::Left => x,
            Alignment::Center => x + width.saturating_sub(text_len) / 2,
            Alignment::Right => x + width.saturating_sub(text_len),
        };
        self.write_str(start_x, y, text, fg, bg);
    }

    pub fn fill_rect(&mut self, rect: Rect, cell: Cell) {
        let x_end = rect.right().min(self.width);
        let y_end = rect.bottom().min(self.height);
        for y in rect.y..y_end {
            for x in rect.x..x_end {
                self.put(x, y, cell);
            }
        }
    }

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

    pub fn draw_border_rounded(&mut self, rect: Rect, fg: Color, bg: Color) {
        self.draw_border_styled(rect, BorderStyle::Rounded, fg, bg);
    }

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
            BorderStyle::None => return,
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

        self.put(left, top, Cell::new(tl).with_fg(fg).with_bg(bg));
        self.put(right, top, Cell::new(tr).with_fg(fg).with_bg(bg));
        self.put(left, bottom, Cell::new(bl).with_fg(fg).with_bg(bg));
        self.put(right, bottom, Cell::new(br).with_fg(fg).with_bg(bg));

        for x in (left + 1)..right {
            self.put(x, top, Cell::new(h).with_fg(fg).with_bg(bg));
            self.put(x, bottom, Cell::new(h).with_fg(fg).with_bg(bg));
        }

        for y in (top + 1)..bottom {
            self.put(left, y, Cell::new(v).with_fg(fg).with_bg(bg));
            self.put(right, y, Cell::new(v).with_fg(fg).with_bg(bg));
        }
    }

    pub fn draw_title(
        &mut self,
        rect: Rect,
        title: &str,
        _border_fg: Color,
        bg: Color,
        title_fg: Color,
    ) {
        if !title.is_empty() {
            let label = format!(" {} ", title);
            self.write_str(rect.x + 1, rect.y, &label, title_fg, bg);
        }
    }

    pub fn draw_panel(&mut self, rect: Rect, title: &str, border_cell: Cell, title_fg: Color) {
        self.draw_border(rect, border_cell);
        if !title.is_empty() {
            let label = format!(" {} ", title);
            self.write_str(rect.x + 1, rect.y, &label, title_fg, border_cell.bg);
        }
    }

    pub fn draw_rounded_panel(
        &mut self,
        rect: Rect,
        title: &str,
        border_fg: Color,
        title_fg: Color,
        bg: Color,
    ) {
        self.draw_border_rounded(rect, border_fg, bg);
        if !title.is_empty() {
            let label = format!(" {} ", title);
            // Center the title
            let tx = rect.x + (rect.width.saturating_sub(label.len() as u16)) / 2;
            self.write_str(tx, rect.y, &label, title_fg, bg);
        }
    }

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
        let lines = crate::wrap_text(text, inner.width as usize);
        let max_lines = inner.height as usize;
        for (i, line) in lines.iter().enumerate() {
            if i >= max_lines {
                break;
            }
            self.write_str(inner.x, inner.y + i as u16, line, text_fg, bg);
        }
        lines.len().min(max_lines) as u16
    }

    pub fn draw_hline(&mut self, x1: u16, x2: u16, y: u16, cell: Cell) -> u16 {
        if y >= self.height {
            return 0;
        }
        let start = x1.min(x2);
        let end = x1.max(x2).min(self.width - 1);
        let mut count = 0;
        for x in start..=end {
            self.put(x, y, cell);
            count += 1;
        }
        count
    }

    pub fn draw_vline(&mut self, x: u16, y1: u16, y2: u16, cell: Cell) -> u16 {
        if x >= self.width {
            return 0;
        }
        let start = y1.min(y2);
        let end = y1.max(y2).min(self.height - 1);
        let mut count = 0;
        for y in start..=end {
            self.put(x, y, cell);
            count += 1;
        }
        count
    }

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
            if x0 >= 0
                && y0 >= 0
                && (x0 as u16) < self.width
                && (y0 as u16) < self.height
                && self.put(x0 as u16, y0 as u16, cell)
            {
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

    pub fn draw_circle(&mut self, cx: i32, cy: i32, radius: u16, cell: Cell) {
        if radius == 0 {
            return;
        }
        let mut x = 0i32;
        let mut y = radius as i32;
        let mut d = 1 - radius as i32;

        let plot = |grid: &mut Grid, px: i32, py: i32| {
            if px >= 0 && py >= 0 && (px as u16) < grid.width && (py as u16) < grid.height {
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
                if px >= 0 && py >= 0 && (px as u16) < self.width && (py as u16) < self.height {
                    self.put(px as u16, py as u16, cell);
                }
            }
        }
    }

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

    /// Draws a circular arc between start and end angles (in radians, clockwise from positive X axis).
    pub fn draw_arc(
        &mut self,
        cx: i32,
        cy: i32,
        radius: u16,
        start_angle: f32,
        end_angle: f32,
        cell: Cell,
    ) {
        if radius == 0 {
            return;
        }
        let two_pi = std::f32::consts::TAU;
        let start = start_angle.rem_euclid(two_pi);
        let mut end = end_angle.rem_euclid(two_pi);
        if end < start {
            end += two_pi;
        }

        let is_angle_between = |angle: f32| -> bool {
            let a = angle.rem_euclid(two_pi);
            let a_plus = a + two_pi;
            (start <= a && a <= end) || (start <= a_plus && a_plus <= end)
        };

        let mut x = 0i32;
        let mut y = radius as i32;
        let mut d = 1 - radius as i32;

        let plot_if_between = |grid: &mut Grid, px: i32, py: i32| {
            if px >= 0 && py >= 0 && (px as u16) < grid.width && (py as u16) < grid.height {
                let dx = px - cx;
                let dy = py - cy;
                let angle = (dy as f32).atan2(dx as f32);
                if is_angle_between(angle) {
                    grid.put(px as u16, py as u16, cell);
                }
            }
        };

        while x <= y {
            plot_if_between(self, cx + x, cy + y);
            plot_if_between(self, cx - x, cy + y);
            plot_if_between(self, cx + x, cy - y);
            plot_if_between(self, cx - x, cy - y);
            plot_if_between(self, cx + y, cy + x);
            plot_if_between(self, cx - y, cy + x);
            plot_if_between(self, cx + y, cy - x);
            plot_if_between(self, cx - y, cy - x);

            if d < 0 {
                d += 2 * x + 3;
            } else {
                d += 2 * (x - y) + 5;
                y -= 1;
            }
            x += 1;
        }
    }

    /// Fills a pie slice (sector) defined by center, radius, and start/end angles in radians.
    pub fn fill_pie(
        &mut self,
        cx: i32,
        cy: i32,
        radius: u16,
        start_angle: f32,
        end_angle: f32,
        cell: Cell,
    ) {
        if radius == 0 {
            return;
        }
        let r = radius as i32;
        let r2 = r * r;
        let two_pi = std::f32::consts::TAU;
        let start = start_angle.rem_euclid(two_pi);
        let mut end = end_angle.rem_euclid(two_pi);
        if end < start {
            end += two_pi;
        }

        let is_angle_between = |angle: f32| -> bool {
            let a = angle.rem_euclid(two_pi);
            let a_plus = a + two_pi;
            (start <= a && a <= end) || (start <= a_plus && a_plus <= end)
        };

        for dy in -r..=r {
            let dx_max = ((r2 - dy * dy) as f64).sqrt() as i32;
            let py = cy + dy;
            if py < 0 || py >= self.height as i32 {
                continue;
            }
            for dx in -dx_max..=dx_max {
                let px = cx + dx;
                if px < 0 || px >= self.width as i32 {
                    continue;
                }
                if dx == 0 && dy == 0 {
                    self.put(px as u16, py as u16, cell);
                    continue;
                }
                let angle = (dy as f32).atan2(dx as f32);
                if is_angle_between(angle) {
                    self.put(px as u16, py as u16, cell);
                }
            }
        }
    }

    pub fn draw_shadow(&mut self, rect: Rect) {
        // Draw a shadow to the right and bottom of the rect.
        // Right shadow: (rect.right, rect.y + 1) to (rect.right + 1, rect.bottom + 1)
        // Bottom shadow: (rect.x + 1, rect.bottom) to (rect.right + 1, rect.bottom + 1)
        let shadow_color = Color(20, 20, 20);
        let alpha = 0.5;

        // Right shadow (1 cell wide)
        let rx = rect.right();
        if rx < self.width {
            for y in (rect.y + 1)..rect.bottom().min(self.height) {
                if let Some(cell) = self.get_mut(rx, y) {
                    cell.bg = cell.bg.blend_alpha(shadow_color, alpha);
                }
            }
        }

        // Bottom shadow (1 cell high)
        let ry = rect.bottom();
        if ry < self.height {
            for x in (rect.x + 1)..(rect.right() + 1).min(self.width) {
                if let Some(cell) = self.get_mut(x, ry) {
                    cell.bg = cell.bg.blend_alpha(shadow_color, alpha);
                }
            }
        }
    }

    pub fn scroll_up(&mut self, amount: u16, clear_cell: Cell) {
        if amount == 0 {
            return;
        }
        if amount >= self.height {
            self.clear(clear_cell);
            return;
        }
        let w = self.width as usize;
        let h = self.height as usize;
        let amt = amount as usize;
        self.cells.copy_within((amt * w).., 0);
        self.cells[(h - amt) * w..].fill(clear_cell);
    }

    pub fn scroll_down(&mut self, amount: u16, clear_cell: Cell) {
        if amount == 0 {
            return;
        }
        if amount >= self.height {
            self.clear(clear_cell);
            return;
        }
        let w = self.width as usize;
        let h = self.height as usize;
        let amt = amount as usize;
        self.cells.copy_within(0..(h - amt) * w, amt * w);
        self.cells[0..amt * w].fill(clear_cell);
    }

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

    pub fn transform<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Cell),
    {
        for cell in &mut self.cells {
            f(cell);
        }
    }

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
                let t = (x - rect.x) as f32 / (width - 1).max(1) as f32;
                let color = start.lerp(end, t);
                self.put(x, y, Cell::new(glyph).with_fg(color));
            }
        }
    }

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
            let t = (y - rect.y) as f32 / (height - 1).max(1) as f32;
            let color = top.lerp(bottom, t);
            for x in rect.x..x_end {
                self.put(x, y, Cell::new(glyph).with_fg(color));
            }
        }
    }

    /// Convert the grid to a compact binary format.
    ///
    /// Format: [u16 width][u16 height][cells...]
    /// Each cell: [u32 glyph][u8 fg_r, g, b][u8 bg_r, g, b][u8 attrs_bits]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(4 + self.cells.len() * 8);
        bytes.extend_from_slice(&self.width.to_le_bytes());
        bytes.extend_from_slice(&self.height.to_le_bytes());
        for cell in &self.cells {
            bytes.extend_from_slice(&(cell.glyph as u32).to_le_bytes());
            bytes.push(cell.fg.0);
            bytes.push(cell.fg.1);
            bytes.push(cell.fg.2);
            bytes.push(cell.bg.0);
            bytes.push(cell.bg.1);
            bytes.push(cell.bg.2);
            let mut attrs = 0u8;
            if cell.attrs.bold {
                attrs |= 1 << 0;
            }
            if cell.attrs.underline {
                attrs |= 1 << 1;
            }
            if cell.attrs.dim {
                attrs |= 1 << 2;
            }
            if cell.attrs.italic {
                attrs |= 1 << 3;
            }
            if cell.attrs.reverse {
                attrs |= 1 << 4;
            }
            if cell.attrs.blink {
                attrs |= 1 << 5;
            }
            bytes.push(attrs);
        }
        bytes
    }

    /// Load a grid from the compact binary format.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 4 {
            return Err("buffer too small".to_string());
        }
        let width = u16::from_le_bytes([bytes[0], bytes[1]]);
        let height = u16::from_le_bytes([bytes[2], bytes[3]]);
        let size = width as usize * height as usize;
        let mut cells = Vec::with_capacity(size);
        let mut pos = 4;
        for _ in 0..size {
            if pos + 11 > bytes.len() {
                return Err("buffer truncated".to_string());
            }
            let glyph_u32 =
                u32::from_le_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]]);
            let glyph = std::char::from_u32(glyph_u32).unwrap_or(' ');
            let fg = Color(bytes[pos + 4], bytes[pos + 5], bytes[pos + 6]);
            let bg = Color(bytes[pos + 7], bytes[pos + 8], bytes[pos + 9]);
            let attr_bits = bytes[pos + 10];
            let attrs = CellAttrs {
                bold: attr_bits & (1 << 0) != 0,
                underline: attr_bits & (1 << 1) != 0,
                dim: attr_bits & (1 << 2) != 0,
                italic: attr_bits & (1 << 3) != 0,
                reverse: attr_bits & (1 << 4) != 0,
                blink: attr_bits & (1 << 5) != 0,
            };
            cells.push(Cell {
                glyph,
                fg,
                bg,
                attrs,
            });
            pos += 11;
        }
        Ok(Self {
            width,
            height,
            cells,
        })
    }

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

    pub fn to_svg_string(&self) -> String {
        let cell_w = 9.0;
        let cell_h = 18.0;
        let svg_w = self.width as f32 * cell_w;
        let svg_h = self.height as f32 * cell_h;
        let mut out = String::with_capacity(self.cells.len() * 80 + 300);
        out.push_str(&format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{:.1}" height="{:.1}" viewBox="0 0 {:.1} {:.1}">"#,
            svg_w, svg_h, svg_w, svg_h
        ));
        out.push_str(r#"<style>text { font-family: monospace; font-size: 14px; text-anchor: middle; dominant-baseline: middle; }</style>"#);
        out.push_str(&format!(
            r#"<rect width="{:.1}" height="{:.1}" fill="black"/>"#,
            svg_w, svg_h
        ));
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
        let filled_count = (clamped * width as f32).round() as u16;
        let mut count = 0;
        for i in 0..width {
            let cx = x + i;
            if cx >= self.width {
                break;
            }
            let cell = if i < filled_count {
                count += 1;
                fill_cell
            } else {
                empty_cell
            };
            self.put(cx, y, cell);
        }
        count
    }

    pub fn write_rich(&mut self, x: u16, y: u16, text: &str) -> Result<u16, String> {
        let mut parser = RichTextParser::new(text);
        let mut current_x = x;
        while let Some(segment) = parser.next_segment()? {
            if current_x >= self.width {
                break;
            }
            for ch in segment.text.chars() {
                if current_x >= self.width {
                    break;
                }
                self.put(
                    current_x,
                    y,
                    Cell {
                        glyph: ch,
                        fg: segment.fg,
                        bg: segment.bg,
                        attrs: segment.attrs,
                    },
                );
                current_x += 1;
            }
        }
        Ok(current_x - x)
    }

    pub fn write_rich_wrapped(
        &mut self,
        x: u16,
        y: u16,
        text: &str,
        width: u16,
    ) -> Result<u16, String> {
        let mut parser = RichTextParser::new(text);
        let mut segments = Vec::new();
        while let Some(segment) = parser.next_segment()? {
            segments.push(segment);
        }

        let mut lines = Vec::new();
        let mut current_line = Vec::new();
        let mut current_width = 0;

        for segment in segments {
            let words = segment.text.split_inclusive(' ');
            for word in words {
                let word_len = word.chars().count();
                if current_width + word_len > width as usize && current_width > 0 {
                    lines.push(current_line);
                    current_line = Vec::new();
                    current_width = 0;
                }
                current_line.push(RichTextSegment {
                    text: word.to_string(),
                    fg: segment.fg,
                    bg: segment.bg,
                    attrs: segment.attrs,
                });
                current_width += word_len;
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }

        let mut line_count = 0;
        for (i, line) in lines.into_iter().enumerate() {
            let ly = y + i as u16;
            if ly >= self.height {
                break;
            }
            let mut lx = x;
            for seg in line {
                for ch in seg.text.chars() {
                    if lx >= self.width || lx >= x + width {
                        break;
                    }
                    self.put(
                        lx,
                        ly,
                        Cell {
                            glyph: ch,
                            fg: seg.fg,
                            bg: seg.bg,
                            attrs: seg.attrs,
                        },
                    );
                    lx += 1;
                }
            }
            line_count += 1;
        }
        Ok(line_count)
    }

    pub fn parse_and_wrap_rich(
        text: &str,
        width: u16,
    ) -> Result<Vec<Vec<RichTextSegment>>, String> {
        let mut parser = RichTextParser::new(text);
        let mut segments = Vec::new();
        while let Some(segment) = parser.next_segment()? {
            segments.push(segment);
        }

        let mut lines = Vec::new();
        let mut current_line = Vec::new();
        let mut current_width = 0;

        for segment in segments {
            let words = segment.text.split_inclusive(' ');
            for word in words {
                let word_len = word.chars().count();
                if current_width + word_len > width as usize && current_width > 0 {
                    lines.push(current_line);
                    current_line = Vec::new();
                    current_width = 0;
                }
                current_line.push(RichTextSegment {
                    text: word.to_string(),
                    fg: segment.fg,
                    bg: segment.bg,
                    attrs: segment.attrs,
                });
                current_width += word_len;
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }
        Ok(lines)
    }
}

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
    let mut count = 0;
    for i in 0..width {
        let cx = x + i;
        if cx >= x_end {
            break;
        }
        let idx = if values.len() == 1 {
            0
        } else {
            (i as f32 * (values.len() - 1) as f32 / (width - 1).max(1) as f32).round() as usize
        };
        let val = values[idx.min(values.len() - 1)];
        let level = if range > 0.0 {
            ((val - min) / range * 8.0).round() as usize
        } else {
            8
        };
        grid.put(
            cx,
            y,
            Cell::new(BLOCKS[level.min(8)]).with_fg(fg).with_bg(bg),
        );
        count += 1;
    }
    count
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RichTextSegment {
    pub text: String,
    pub fg: Color,
    pub bg: Color,
    pub attrs: CellAttrs,
}

struct RichTextParser<'a> {
    input: &'a str,
    pos: usize,
    state: RichTextState,
}

#[derive(Clone)]
struct RichTextState {
    fg: Color,
    bg: Color,
    attrs: CellAttrs,
}

impl<'a> RichTextParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            pos: 0,
            state: RichTextState {
                fg: Color::WHITE,
                bg: Color::BLACK,
                attrs: CellAttrs::NONE,
            },
        }
    }

    fn next_segment(&mut self) -> Result<Option<RichTextSegment>, String> {
        if self.pos >= self.input.len() {
            return Ok(None);
        }

        let mut text = String::new();

        while self.pos < self.input.len() {
            let ch = self.input.as_bytes()[self.pos] as char;
            if ch == '[' {
                if self.pos + 1 < self.input.len()
                    && self.input.as_bytes()[self.pos + 1] as char == '['
                {
                    text.push('[');
                    self.pos += 2;
                    continue;
                }

                if !text.is_empty() {
                    return Ok(Some(RichTextSegment {
                        text,
                        fg: self.state.fg,
                        bg: self.state.bg,
                        attrs: self.state.attrs,
                    }));
                }

                self.pos += 1;
                let end_pos = self.input[self.pos..]
                    .find(']')
                    .ok_or_else(|| "Unclosed tag".to_string())?
                    + self.pos;
                let tag = &self.input[self.pos..end_pos];
                self.pos = end_pos + 1;

                match tag {
                    "b" => self.state.attrs.bold = true,
                    "/b" => self.state.attrs.bold = false,
                    "i" => self.state.attrs.italic = true,
                    "/i" => self.state.attrs.italic = false,
                    "u" => self.state.attrs.underline = true,
                    "/u" => self.state.attrs.underline = false,
                    "/" => {
                        self.state = RichTextState {
                            fg: Color::WHITE,
                            bg: Color::BLACK,
                            attrs: CellAttrs::NONE,
                        };
                    }
                    t if t.starts_with("fg:") => {
                        self.state.fg = Color::from_hex(&t[3..])?;
                    }
                    "/fg" => self.state.fg = Color::WHITE,
                    t if t.starts_with("bg:") => {
                        self.state.bg = Color::from_hex(&t[3..])?;
                    }
                    "/bg" => self.state.bg = Color::BLACK,
                    _ => return Err(format!("Unknown tag: {}", tag)),
                }
            } else {
                text.push(ch);
                self.pos += 1;
            }
        }

        if text.is_empty() {
            Ok(None)
        } else {
            Ok(Some(RichTextSegment {
                text,
                fg: self.state.fg,
                bg: self.state.bg,
                attrs: self.state.attrs,
            }))
        }
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
    fn write_str_clips_at_right_edge() {
        let mut grid = Grid::new(4, 1);
        grid.write_str(2, 0, "hello", Color::WHITE, Color::BLACK);
        assert_eq!(grid.to_plain_string(), "  he");
    }

    #[test]
    fn diff_reports_changed_cells() {
        let mut before = Grid::new(2, 1);
        before.write_str(0, 0, "ab", Color::WHITE, Color::BLACK);
        let mut after = Grid::new(2, 1);
        after.write_str(0, 0, "ac", Color::WHITE, Color::BLACK);

        assert_eq!(
            before.diff(&after),
            vec![CellChange {
                x: 1,
                y: 0,
                before: Some(Cell::new('b')),
                after: Some(Cell::new('c')),
            },]
        );
    }

    #[test]
    fn test_parse_and_wrap_rich() {
        let text = "Hello [fg:ff0000][b]world[/][/fg]!";
        let lines = Grid::parse_and_wrap_rich(text, 15).unwrap();
        assert_eq!(lines.len(), 1);
        let line = &lines[0];
        assert_eq!(line.len(), 3);
        assert_eq!(line[0].text, "Hello ");
        assert_eq!(line[0].fg, Color::WHITE);
        assert_eq!(line[1].text, "world");
        assert_eq!(line[1].fg, Color(255, 0, 0));
        assert!(line[1].attrs.bold);
        assert_eq!(line[2].text, "!");
        assert_eq!(line[2].fg, Color::WHITE);
    }

    #[test]
    fn test_grid_arc_and_pie() {
        let mut grid = Grid::new(5, 5);
        let cell = Cell::new('*');
        // Draw arc from 0 to PI (bottom half of circle)
        grid.draw_arc(2, 2, 2, 0.0, std::f32::consts::PI, cell);
        // Assert center is empty
        assert_eq!(grid.get(2, 2).unwrap().glyph, ' ');
        // Assert bottom part has some points
        assert_eq!(grid.get(2, 4).unwrap().glyph, '*');

        let mut grid_pie = Grid::new(5, 5);
        // Fill pie from -PI/4 to PI/4 (right slice)
        grid_pie.fill_pie(
            2,
            2,
            2,
            -std::f32::consts::FRAC_PI_4,
            std::f32::consts::FRAC_PI_4,
            cell,
        );
        assert_eq!(grid_pie.get(2, 2).unwrap().glyph, '*');
        assert_eq!(grid_pie.get(4, 2).unwrap().glyph, '*');
        assert_eq!(grid_pie.get(0, 2).unwrap().glyph, ' ');
    }

    #[test]
    fn test_grid_filters() {
        let mut grid = Grid::new(3, 3);
        grid.put(
            0,
            0,
            Cell::new('A')
                .with_fg(Color(10, 20, 30))
                .with_bg(Color(40, 50, 60)),
        );
        grid.put(
            1,
            0,
            Cell::new('B')
                .with_fg(Color(100, 110, 120))
                .with_bg(Color(130, 140, 150)),
        );

        // Test apply_filter
        grid.apply_filter(Rect::new(0, 0, 1, 1), |cell| {
            cell.glyph = 'X';
        });
        assert_eq!(grid.get(0, 0).unwrap().glyph, 'X');
        assert_eq!(grid.get(1, 0).unwrap().glyph, 'B');

        // Test apply_blur
        let mut grid_blur = Grid::new(3, 1);
        grid_blur.put(
            0,
            0,
            Cell::new('A')
                .with_fg(Color(10, 10, 10))
                .with_bg(Color(0, 0, 0)),
        );
        grid_blur.put(
            1,
            0,
            Cell::new('B')
                .with_fg(Color(30, 30, 30))
                .with_bg(Color(100, 100, 100)),
        );
        grid_blur.put(
            2,
            0,
            Cell::new('C')
                .with_fg(Color(50, 50, 50))
                .with_bg(Color(200, 200, 200)),
        );
        grid_blur.apply_blur(Rect::new(0, 0, 3, 1), 1);
        // The middle cell (1, 0) should average (10+30+50)/3 = 30 for fg and (0+100+200)/3 = 100 for bg
        assert_eq!(grid_blur.get(1, 0).unwrap().fg, Color(30, 30, 30));
        assert_eq!(grid_blur.get(1, 0).unwrap().bg, Color(100, 100, 100));

        // Test apply_tint
        let mut grid_tint = Grid::new(1, 1);
        grid_tint.put(
            0,
            0,
            Cell::new('A')
                .with_fg(Color(100, 100, 100))
                .with_bg(Color(0, 0, 0)),
        );
        grid_tint.apply_tint(
            Rect::new(0, 0, 1, 1),
            Color(200, 200, 200),
            0.5,
            crate::color::BlendMode::Normal,
        );
        // 100 * 0.5 + 200 * 0.5 = 150
        assert_eq!(grid_tint.get(0, 0).unwrap().fg, Color(150, 150, 150));

        // Test adjust_hsv
        let mut grid_hsv = Grid::new(1, 1);
        grid_hsv.put(
            0,
            0,
            Cell::new('A')
                .with_fg(Color(128, 64, 192))
                .with_bg(Color(0, 0, 0)),
        );
        grid_hsv.adjust_hsv(Rect::new(0, 0, 1, 1), 0.0, 1.0, 0.5); // Halve the brightness
        let half_val = grid_hsv.get(0, 0).unwrap().fg;
        // Verify value has decreased
        assert!(half_val.0 < 128);
    }

    #[test]
    fn find_all_cells_returns_matching_positions() {
        let mut grid = Grid::new(3, 3);
        grid.put(0, 0, Cell::new('X'));
        grid.put(1, 1, Cell::new('X'));
        grid.put(2, 2, Cell::new('X'));
        grid.put(0, 1, Cell::new('Y'));

        let matches = grid.find_all_cells(|c| c.glyph == 'X');
        assert_eq!(matches.len(), 3);
        assert!(matches.iter().any(|(x, y, _)| *x == 0 && *y == 0));
        assert!(matches.iter().any(|(x, y, _)| *x == 1 && *y == 1));
        assert!(matches.iter().any(|(x, y, _)| *x == 2 && *y == 2));
    }

    #[test]
    fn find_all_cells_empty_when_no_match() {
        let grid = Grid::new(2, 2);
        let matches = grid.find_all_cells(|c| c.glyph == 'Z');
        assert!(matches.is_empty());
    }

    #[test]
    fn grid_is_empty_for_zero_dimensions() {
        let grid = Grid::new(0, 0);
        assert!(grid.is_empty());
        let grid2 = Grid::new(5, 0);
        assert!(grid2.is_empty());
        let grid3 = Grid::new(0, 5);
        assert!(grid3.is_empty());
        let grid4 = Grid::new(3, 3);
        assert!(!grid4.is_empty());
    }
}
