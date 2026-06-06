use crate::color::Color;
use crate::grid::{Cell, Grid};
use crate::layout::{Alignment, BorderStyle, Constraint, Layout, Rect};

/// A generic container widget with a background, optional border, and title.
///
/// Use `Panel` to group other widgets or provide a consistent background
/// for a UI section.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Panel {
    pub rect: Rect,
    pub bg: Color,
    pub border: BorderStyle,
    pub border_color: Color,
    pub title: Option<String>,
    pub title_color: Color,
}

impl Panel {
    pub fn new(rect: Rect) -> Self {
        Self {
            rect,
            bg: Color::BLACK,
            border: BorderStyle::None,
            border_color: Color::GREY,
            title: None,
            title_color: Color::WHITE,
        }
    }

    pub fn with_bg(mut self, bg: Color) -> Self {
        self.bg = bg;
        self
    }

    pub fn with_border(mut self, style: BorderStyle, color: Color) -> Self {
        self.border = style;
        self.border_color = color;
        self
    }

    pub fn with_title<S: Into<String>>(mut self, title: S, color: Color) -> Self {
        self.title = Some(title.into());
        self.title_color = color;
        self
    }

    /// Returns the inner rect (the area inside the border, if any).
    pub fn inner_rect(&self) -> Rect {
        if self.border != BorderStyle::None {
            self.rect.inset(1, 1)
        } else {
            self.rect
        }
    }

    pub fn render(&self, grid: &mut Grid) {
        if self.rect.is_empty() {
            return;
        }

        grid.fill_rect(self.rect, Cell::new(' ').with_bg(self.bg));

        if self.border != BorderStyle::None {
            grid.draw_border_styled(self.rect, self.border, self.border_color, self.bg);
            if let Some(ref title) = self.title {
                grid.draw_title(
                    self.rect,
                    title,
                    self.border_color,
                    self.bg,
                    self.title_color,
                );
            }
        }
    }
}

/// A UI widget for rendering a scrollable list of messages.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MessageLogView {
    pub rect: Rect,
    pub fg: Color,
    pub bg: Color,
    pub border: BorderStyle,
    pub border_color: Color,
    pub title: Option<String>,
}

impl MessageLogView {
    pub fn new(rect: Rect) -> Self {
        Self {
            rect,
            fg: Color::WHITE,
            bg: Color::BLACK,
            border: BorderStyle::None,
            border_color: Color::GREY,
            title: None,
        }
    }

    pub fn with_border(mut self, style: BorderStyle, color: Color) -> Self {
        self.border = style;
        self.border_color = color;
        self
    }

    pub fn with_title<S: Into<String>>(mut self, title: S) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_colors(mut self, fg: Color, bg: Color) -> Self {
        self.fg = fg;
        self.bg = bg;
        self
    }

    pub fn render(&self, grid: &mut Grid, messages: &[String]) {
        if self.rect.is_empty() {
            return;
        }

        grid.fill_rect(self.rect, Cell::new(' ').with_bg(self.bg));

        let inner_rect = if self.border != BorderStyle::None {
            grid.draw_border_styled(self.rect, self.border, self.border_color, self.bg);
            if let Some(ref title) = self.title {
                grid.draw_title(self.rect, title, self.border_color, self.bg, Color::WHITE);
            }
            self.rect.inset(1, 1)
        } else {
            self.rect
        };

        if inner_rect.is_empty() {
            return;
        }

        let mut current_y = inner_rect.bottom().saturating_sub(1);
        let width = inner_rect.width;

        for msg in messages.iter().rev() {
            let lines = if msg.contains('[') {
                if let Ok(rich_lines) = Grid::parse_and_wrap_rich(msg, width) {
                    rich_lines
                } else {
                    let wrapped = crate::wrap_text(msg, width as usize);
                    wrapped
                        .into_iter()
                        .map(|line| {
                            vec![crate::RichTextSegment {
                                text: line,
                                fg: self.fg,
                                bg: self.bg,
                                attrs: crate::CellAttrs::NONE,
                            }]
                        })
                        .collect()
                }
            } else {
                let wrapped = crate::wrap_text(msg, width as usize);
                wrapped
                    .into_iter()
                    .map(|line| {
                        vec![crate::RichTextSegment {
                            text: line,
                            fg: self.fg,
                            bg: self.bg,
                            attrs: crate::CellAttrs::NONE,
                        }]
                    })
                    .collect()
            };

            for line in lines.iter().rev() {
                if current_y < inner_rect.y {
                    return;
                }
                let mut lx = inner_rect.x;
                for seg in line {
                    for ch in seg.text.chars() {
                        if lx >= inner_rect.right() {
                            break;
                        }
                        let bg = if seg.bg == Color::BLACK && self.bg != Color::BLACK {
                            self.bg
                        } else {
                            seg.bg
                        };
                        grid.put(
                            lx,
                            current_y,
                            Cell {
                                glyph: ch,
                                fg: seg.fg,
                                bg,
                                attrs: seg.attrs,
                            },
                        );
                        lx += 1;
                    }
                }
                if current_y == 0 {
                    return;
                }
                current_y -= 1;
            }
        }
    }
}

/// A widget that displays performance metrics from the Diagnostics resource.
#[derive(Clone, Debug)]
pub struct PerformanceOverlay {
    pub rect: Rect,
    pub border_color: Color,
    pub bg: Color,
    pub text_fg: Color,
}

impl PerformanceOverlay {
    pub fn new(rect: Rect) -> Self {
        Self {
            rect,
            border_color: Color::YELLOW,
            bg: Color(0, 0, 40),
            text_fg: Color::WHITE,
        }
    }

    pub fn render(&self, grid: &mut Grid, diagnostics: &verryte_core::diagnostics::Diagnostics) {
        if self.rect.is_empty() {
            return;
        }

        grid.fill_rect(self.rect, Cell::new(' ').with_bg(self.bg));
        grid.draw_border_styled(self.rect, BorderStyle::Single, self.border_color, self.bg);
        grid.draw_title(
            self.rect,
            " PERFORMANCE ",
            self.border_color,
            self.bg,
            Color::YELLOW,
        );

        let inner = self.rect.inset(1, 1);
        if inner.is_empty() {
            return;
        }

        let sorted_systems = diagnostics.sorted_by_duration();

        for (i, (name, metrics)) in sorted_systems.iter().enumerate() {
            let y = inner.y + i as u16;
            if y >= inner.bottom() {
                break;
            }

            let dur_ms = metrics.last_duration.as_secs_f32() * 1000.0;
            let line = format!("{:<15} {:>6.2}ms", name, dur_ms);

            let color = if dur_ms > 16.6 {
                Color::RED
            } else if dur_ms > 8.3 {
                Color::YELLOW
            } else {
                self.text_fg
            };

            grid.write_str(inner.x, y, &line, color, self.bg);
        }
    }
}

/// A vertical menu widget with a selectable active item and scroll support.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MenuView {
    pub rect: Rect,
    pub options: Vec<String>,
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub normal_fg: Color,
    pub selected_fg: Color,
    pub bg: Color,
    pub selection_marker: String,
    pub border: BorderStyle,
    pub border_color: Color,
}

impl MenuView {
    pub fn new(rect: Rect, options: Vec<String>) -> Self {
        Self {
            rect,
            options,
            selected_index: 0,
            scroll_offset: 0,
            normal_fg: Color::WHITE,
            selected_fg: Color::YELLOW,
            bg: Color::BLACK,
            selection_marker: "> ".to_string(),
            border: BorderStyle::None,
            border_color: Color::GREY,
        }
    }

    pub fn with_border(mut self, style: BorderStyle, color: Color) -> Self {
        self.border = style;
        self.border_color = color;
        self
    }

    pub fn with_selection_marker(mut self, marker: impl Into<String>) -> Self {
        self.selection_marker = marker.into();
        self
    }

    pub fn next(&mut self) {
        if !self.options.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.options.len();
            self.ensure_visible();
        }
    }

    pub fn prev(&mut self) {
        if !self.options.is_empty() {
            self.selected_index =
                (self.selected_index + self.options.len() - 1) % self.options.len();
            self.ensure_visible();
        }
    }

    fn visible_rows(&self) -> u16 {
        let inner = if self.border != BorderStyle::None {
            self.rect.inset(1, 1)
        } else {
            self.rect
        };
        inner.height
    }

    fn ensure_visible(&mut self) {
        let visible = self.visible_rows() as usize;
        if visible == 0 {
            return;
        }
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + visible {
            self.scroll_offset = self.selected_index - visible + 1;
        }
    }

    pub fn render(&self, grid: &mut Grid) {
        if self.rect.is_empty() {
            return;
        }

        grid.fill_rect(self.rect, Cell::new(' ').with_bg(self.bg));

        let inner_rect = if self.border != BorderStyle::None {
            grid.draw_border_styled(self.rect, self.border, self.border_color, self.bg);
            self.rect.inset(1, 1)
        } else {
            self.rect
        };

        if inner_rect.is_empty() {
            return;
        }

        let visible = self.visible_rows() as usize;
        let marker_width = self.selection_marker.chars().count();
        let has_scrollbar = self.options.len() > visible;
        let text_width = if has_scrollbar {
            inner_rect.width.saturating_sub(1)
        } else {
            inner_rect.width
        };

        for (view_row, option_index) in (self.scroll_offset..self.options.len())
            .enumerate()
            .take(visible)
        {
            let y = inner_rect.y + view_row as u16;
            let option = &self.options[option_index];
            let is_selected = option_index == self.selected_index;

            let (fg, text) = if is_selected {
                (
                    self.selected_fg,
                    format!("{}{}", self.selection_marker, option),
                )
            } else {
                let padding = " ".repeat(marker_width);
                (self.normal_fg, format!("{}{}", padding, option))
            };

            let clipped_text: String = text.chars().take(text_width as usize).collect();
            grid.write_str(inner_rect.x, y, &clipped_text, fg, self.bg);
        }

        if has_scrollbar {
            let scrollbar_rect = Rect::new(
                inner_rect.right().saturating_sub(1),
                inner_rect.y,
                1,
                inner_rect.height,
            );
            grid.draw_scrollbar(
                scrollbar_rect,
                self.options.len(),
                visible,
                self.scroll_offset,
                self.selected_fg,
                self.bg,
            );
        }
    }
}

/// A horizontal progress bar widget.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProgressBar {
    pub rect: Rect,
    pub value: f32, // 0.0 to 1.0
    pub filled_char: char,
    pub empty_char: char,
    pub filled_color: Color,
    pub empty_color: Color,
    pub bg: Color,
}

impl ProgressBar {
    pub fn new(rect: Rect) -> Self {
        Self {
            rect,
            value: 0.0,
            filled_char: '█',
            empty_char: '░',
            filled_color: Color::GREEN,
            empty_color: Color::GREY,
            bg: Color::BLACK,
        }
    }

    pub fn with_value(mut self, value: f32) -> Self {
        self.value = value;
        self
    }

    pub fn with_colors(mut self, filled: Color, empty: Color) -> Self {
        self.filled_color = filled;
        self.empty_color = empty;
        self
    }

    pub fn with_chars(mut self, filled: char, empty: char) -> Self {
        self.filled_char = filled;
        self.empty_char = empty;
        self
    }

    pub fn render(&self, grid: &mut Grid) {
        if self.rect.is_empty() {
            return;
        }

        let width = self.rect.width;
        let filled_width = (self.value.clamp(0.0, 1.0) * width as f32).round() as u16;

        for x in 0..width {
            let (ch, fg) = if x < filled_width {
                (self.filled_char, self.filled_color)
            } else {
                (self.empty_char, self.empty_color)
            };
            grid.put(
                self.rect.x + x,
                self.rect.y,
                Cell::new(ch).with_fg(fg).with_bg(self.bg),
            );
        }
    }
}

/// A vertical progress bar widget that fills from bottom to top.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VerticalProgressBar {
    pub rect: Rect,
    pub value: f32,
    pub filled_char: char,
    pub empty_char: char,
    pub filled_color: Color,
    pub empty_color: Color,
    pub bg: Color,
}

impl VerticalProgressBar {
    pub fn new(rect: Rect) -> Self {
        Self {
            rect,
            value: 0.0,
            filled_char: '█',
            empty_char: '░',
            filled_color: Color::GREEN,
            empty_color: Color::GREY,
            bg: Color::BLACK,
        }
    }

    pub fn with_value(mut self, value: f32) -> Self {
        self.value = value;
        self
    }

    pub fn with_colors(mut self, filled: Color, empty: Color) -> Self {
        self.filled_color = filled;
        self.empty_color = empty;
        self
    }

    pub fn with_chars(mut self, filled: char, empty: char) -> Self {
        self.filled_char = filled;
        self.empty_char = empty;
        self
    }

    pub fn render(&self, grid: &mut Grid) {
        if self.rect.is_empty() {
            return;
        }

        let height = self.rect.height;
        let filled_height = (self.value.clamp(0.0, 1.0) * height as f32).round() as u16;

        for dy in 0..height {
            let y = self.rect.y + self.rect.height - 1 - dy;
            let (ch, fg) = if dy < filled_height {
                (self.filled_char, self.filled_color)
            } else {
                (self.empty_char, self.empty_color)
            };
            grid.put(self.rect.x, y, Cell::new(ch).with_fg(fg).with_bg(self.bg));
        }
    }
}

/// A floating tooltip widget that can be placed at a specific coordinate.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Tooltip {
    pub text: String,
    pub fg: Color,
    pub bg: Color,
    pub border_color: Color,
    pub max_width: u16,
}

impl Tooltip {
    pub fn new<S: Into<String>>(text: S) -> Self {
        Self {
            text: text.into(),
            fg: Color::WHITE,
            bg: Color(20, 20, 40),
            border_color: Color::GREY,
            max_width: 30,
        }
    }

    pub fn with_colors(mut self, fg: Color, bg: Color) -> Self {
        self.fg = fg;
        self.bg = bg;
        self
    }

    pub fn with_max_width(mut self, width: u16) -> Self {
        self.max_width = width;
        self
    }

    /// Render the tooltip at the given position, automatically sizing it to fit the text.
    /// The tooltip is placed so that `(x, y)` is the anchor point (usually the top-left or top-center).
    pub fn render(&self, grid: &mut Grid, x: u16, y: u16) {
        let wrapped = crate::wrap_text(&self.text, self.max_width as usize);
        if wrapped.is_empty() {
            return;
        }

        let width = wrapped.iter().map(|l| l.chars().count()).max().unwrap_or(0) as u16;
        let height = wrapped.len() as u16;

        let rect = Rect::new(x, y, width + 2, height + 2);
        // Clamp to grid bounds
        let mut rect = rect;
        if rect.right() > grid.width() {
            rect.x = grid.width().saturating_sub(rect.width);
        }
        if rect.bottom() > grid.height() {
            rect.y = grid.height().saturating_sub(rect.height);
        }

        grid.fill_rect(rect, Cell::new(' ').with_bg(self.bg));
        grid.draw_border_styled(rect, BorderStyle::Single, self.border_color, self.bg);

        let inner = rect.inset(1, 1);
        for (i, line) in wrapped.iter().enumerate() {
            grid.write_str(inner.x, inner.y + i as u16, line, self.fg, self.bg);
        }
    }
}

/// A button widget with hover and active state support.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Button {
    pub label: String,
    pub rect: Rect,
    pub fg: Color,
    pub bg: Color,
    pub border_color: Color,
    pub hovered: bool,
    pub active: bool,
}

impl Button {
    pub fn new<S: Into<String>>(label: S, rect: Rect) -> Self {
        Self {
            label: label.into(),
            rect,
            fg: Color::WHITE,
            bg: Color(30, 30, 50),
            border_color: Color::GREY,
            hovered: false,
            active: false,
        }
    }

    pub fn with_colors(mut self, fg: Color, bg: Color) -> Self {
        self.fg = fg;
        self.bg = bg;
        self
    }

    pub fn set_hovered(&mut self, hovered: bool) {
        self.hovered = hovered;
    }

    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    pub fn render(&self, grid: &mut Grid) {
        let (current_fg, current_bg) = if self.active {
            (Color::BLACK, Color::WHITE)
        } else if self.hovered {
            (self.fg, self.bg.blend_alpha(Color::WHITE, 0.2))
        } else {
            (self.fg, self.bg)
        };

        grid.fill_rect(self.rect, Cell::new(' ').with_bg(current_bg));
        grid.draw_border_styled(
            self.rect,
            BorderStyle::Single,
            self.border_color,
            current_bg,
        );

        // Center the label
        let label_len = self.label.chars().count() as u16;
        let start_x = self.rect.x + self.rect.width.saturating_sub(label_len) / 2;
        let start_y = self.rect.y + self.rect.height.saturating_sub(1) / 2;

        grid.write_str(start_x, start_y, &self.label, current_fg, current_bg);
    }
}

/// A table widget for rendering structured rows and columns of text.
///
/// Supports header row, horizontal dividers, alignment, column width constraints,
/// column sorting, row filtering, alternating row colors, and scroll indicators.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Table {
    pub rect: Rect,
    pub bg: Color,
    pub border: BorderStyle,
    pub border_color: Color,
    pub header_fg: Color,
    pub header_bg: Color,
    pub row_fg: Color,
    pub row_bg: Color,
    pub alt_row_bg: Option<Color>,
    pub constraints: Vec<Constraint>,
    pub headers: Option<Vec<String>>,
    pub rows: Vec<Vec<String>>,
    pub alignments: Vec<Alignment>,
    pub sort_column: Option<usize>,
    pub sort_ascending: bool,
    pub filtered_indices: Option<Vec<usize>>,
    pub scroll_offset: usize,
}

impl Table {
    pub fn new(rect: Rect) -> Self {
        Self {
            rect,
            bg: Color::BLACK,
            border: BorderStyle::None,
            border_color: Color::GREY,
            header_fg: Color::WHITE,
            header_bg: Color::BLACK,
            row_fg: Color::WHITE,
            row_bg: Color::BLACK,
            alt_row_bg: None,
            constraints: Vec::new(),
            headers: None,
            rows: Vec::new(),
            alignments: Vec::new(),
            sort_column: None,
            sort_ascending: true,
            filtered_indices: None,
            scroll_offset: 0,
        }
    }

    pub fn with_bg(mut self, bg: Color) -> Self {
        self.bg = bg;
        self
    }

    pub fn with_border(mut self, style: BorderStyle, color: Color) -> Self {
        self.border = style;
        self.border_color = color;
        self
    }

    pub fn with_header_colors(mut self, fg: Color, bg: Color) -> Self {
        self.header_fg = fg;
        self.header_bg = bg;
        self
    }

    pub fn with_row_colors(mut self, fg: Color, bg: Color) -> Self {
        self.row_fg = fg;
        self.row_bg = bg;
        self
    }

    pub fn with_alt_row_bg(mut self, alt_bg: Color) -> Self {
        self.alt_row_bg = Some(alt_bg);
        self
    }

    pub fn with_constraints(mut self, constraints: Vec<Constraint>) -> Self {
        self.constraints = constraints;
        self
    }

    pub fn with_headers(mut self, headers: Vec<String>) -> Self {
        self.headers = Some(headers);
        self
    }

    pub fn with_rows(mut self, rows: Vec<Vec<String>>) -> Self {
        self.rows = rows;
        self
    }

    pub fn with_alignments(mut self, alignments: Vec<Alignment>) -> Self {
        self.alignments = alignments;
        self
    }

    pub fn toggle_sort(&mut self, col: usize) {
        if self.sort_column == Some(col) {
            self.sort_ascending = !self.sort_ascending;
        } else {
            self.sort_column = Some(col);
            self.sort_ascending = true;
        }
    }

    pub fn apply_sort(&mut self) {
        if let Some(col) = self.sort_column {
            let asc = self.sort_ascending;
            self.rows.sort_by(|a, b| {
                let av = a.get(col).map(|s| s.as_str()).unwrap_or("");
                let bv = b.get(col).map(|s| s.as_str()).unwrap_or("");
                let ord = av.cmp(bv);
                if asc {
                    ord
                } else {
                    ord.reverse()
                }
            });
            self.filtered_indices = None;
        }
    }

    pub fn filter_rows<F>(&mut self, predicate: F)
    where
        F: Fn(&[String]) -> bool,
    {
        let indices: Vec<usize> = self
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| predicate(row))
            .map(|(i, _)| i)
            .collect();
        self.filtered_indices = Some(indices);
        self.scroll_offset = 0;
    }

    pub fn clear_filter(&mut self) {
        self.filtered_indices = None;
    }

    fn visible_row_count(&self) -> u16 {
        let inner = if self.border != BorderStyle::None {
            self.rect.inset(1, 1)
        } else {
            self.rect
        };
        let header_rows: u16 = if self.headers.is_some() { 2 } else { 0 };
        inner.height.saturating_sub(header_rows)
    }

    fn effective_rows(&self) -> Vec<&Vec<String>> {
        if let Some(ref indices) = self.filtered_indices {
            indices.iter().filter_map(|&i| self.rows.get(i)).collect()
        } else {
            self.rows.iter().collect()
        }
    }

    pub fn scroll_down(&mut self) {
        let vis = self.visible_row_count() as usize;
        let total = self.effective_rows().len();
        if total > vis && self.scroll_offset < total - vis {
            self.scroll_offset += 1;
        }
    }

    pub fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    pub fn render(&self, grid: &mut Grid) {
        if self.rect.is_empty() {
            return;
        }

        grid.fill_rect(self.rect, Cell::new(' ').with_bg(self.bg));

        if self.border != BorderStyle::None {
            grid.draw_border_styled(self.rect, self.border, self.border_color, self.bg);
        }

        let inner_rect = if self.border != BorderStyle::None {
            self.rect.inset(1, 1)
        } else {
            self.rect
        };

        if inner_rect.is_empty() || self.constraints.is_empty() {
            return;
        }

        let effective = self.effective_rows();
        let has_scrollbar = effective.len() > self.visible_row_count() as usize;
        let content_width = if has_scrollbar {
            inner_rect.width.saturating_sub(1)
        } else {
            inner_rect.width
        };
        let content_rect = Rect::new(inner_rect.x, inner_rect.y, content_width, inner_rect.height);

        let mut layout = Layout::horizontal();
        for &constraint in &self.constraints {
            match constraint {
                Constraint::Fixed(w) => {
                    layout = layout.add_fixed(w);
                }
                Constraint::Percent(p) => {
                    layout = layout.add_percent(p);
                }
                Constraint::Remaining => {
                    layout = layout.add_remaining();
                }
            }
        }

        let col_rects = layout.split(content_rect);
        let mut current_y = content_rect.y;

        if let Some(ref headers) = self.headers {
            if current_y < content_rect.bottom() {
                let header_row_rect = Rect::new(content_rect.x, current_y, content_rect.width, 1);
                grid.fill_rect(header_row_rect, Cell::new(' ').with_bg(self.header_bg));

                for (col_idx, header) in headers.iter().enumerate() {
                    if col_idx >= col_rects.len() {
                        break;
                    }
                    let col_rect = col_rects[col_idx];
                    if col_rect.width == 0 {
                        continue;
                    }
                    let alignment = self
                        .alignments
                        .get(col_idx)
                        .copied()
                        .unwrap_or(Alignment::Left);

                    let display_header = if self.sort_column == Some(col_idx) {
                        if self.sort_ascending {
                            format!("{} ▲", header)
                        } else {
                            format!("{} ▼", header)
                        }
                    } else {
                        header.clone()
                    };

                    self.render_cell(
                        grid,
                        &display_header,
                        col_rect.x,
                        current_y,
                        col_rect.width,
                        alignment,
                        self.header_fg,
                        self.header_bg,
                    );
                }
                current_y += 1;
            }

            if current_y < content_rect.bottom() {
                let sep_char = match self.border {
                    BorderStyle::Ascii => '-',
                    BorderStyle::Double => '═',
                    BorderStyle::Heavy => '━',
                    _ => '─',
                };
                let cell = Cell::new(sep_char)
                    .with_fg(self.border_color)
                    .with_bg(self.bg);
                for x in content_rect.x..content_rect.right() {
                    grid.put(x, current_y, cell);
                }
                current_y += 1;
            }
        }

        let visible = self.visible_row_count() as usize;
        let start = self.scroll_offset.min(effective.len().saturating_sub(1));
        let end = (start + visible).min(effective.len());

        for (view_idx, row) in effective[start..end].iter().enumerate() {
            if current_y >= content_rect.bottom() {
                break;
            }

            let actual_row_idx = start + view_idx;
            let row_bg = if let Some(alt_bg) = self.alt_row_bg {
                if actual_row_idx % 2 == 1 {
                    alt_bg
                } else {
                    self.row_bg
                }
            } else {
                self.row_bg
            };

            let row_rect = Rect::new(content_rect.x, current_y, content_rect.width, 1);
            grid.fill_rect(row_rect, Cell::new(' ').with_bg(row_bg));

            for (col_idx, cell_value) in row.iter().enumerate() {
                if col_idx >= col_rects.len() {
                    break;
                }
                let col_rect = col_rects[col_idx];
                if col_rect.width == 0 {
                    continue;
                }
                let alignment = self
                    .alignments
                    .get(col_idx)
                    .copied()
                    .unwrap_or(Alignment::Left);

                self.render_cell(
                    grid,
                    cell_value,
                    col_rect.x,
                    current_y,
                    col_rect.width,
                    alignment,
                    self.row_fg,
                    row_bg,
                );
            }
            current_y += 1;
        }

        if has_scrollbar {
            let scrollbar_rect = Rect::new(
                inner_rect.right().saturating_sub(1),
                inner_rect.y,
                1,
                inner_rect.height,
            );
            grid.draw_scrollbar(
                scrollbar_rect,
                effective.len(),
                visible,
                self.scroll_offset,
                self.border_color,
                self.bg,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_cell(
        &self,
        grid: &mut Grid,
        text: &str,
        x: u16,
        y: u16,
        width: u16,
        alignment: Alignment,
        fg: Color,
        bg: Color,
    ) {
        let display_text = text.chars().take(width as usize).collect::<String>();
        let text_len = display_text.chars().count() as u16;
        let start_x = match alignment {
            Alignment::Left => x,
            Alignment::Center => {
                let space = width.saturating_sub(text_len);
                x + space / 2
            }
            Alignment::Right => {
                let space = width.saturating_sub(text_len);
                x + space
            }
        };

        grid.write_str(start_x, y, &display_text, fg, bg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_panel_new() {
        let panel = Panel::new(Rect::new(0, 0, 10, 5));
        assert_eq!(panel.rect, Rect::new(0, 0, 10, 5));
        assert_eq!(panel.bg, Color::BLACK);
        assert_eq!(panel.border, BorderStyle::None);
    }

    #[test]
    fn test_panel_builder() {
        let panel = Panel::new(Rect::new(0, 0, 10, 5))
            .with_bg(Color::RED)
            .with_border(BorderStyle::Single, Color::WHITE)
            .with_title("Test", Color::YELLOW);
        assert_eq!(panel.bg, Color::RED);
        assert_eq!(panel.border, BorderStyle::Single);
        assert_eq!(panel.title, Some("Test".to_string()));
        assert_eq!(panel.title_color, Color::YELLOW);
    }

    #[test]
    fn test_panel_inner_rect() {
        let p1 = Panel::new(Rect::new(0, 0, 10, 10));
        assert_eq!(p1.inner_rect(), Rect::new(0, 0, 10, 10));

        let p2 = Panel::new(Rect::new(0, 0, 10, 10)).with_border(BorderStyle::Single, Color::WHITE);
        assert_eq!(p2.inner_rect(), Rect::new(1, 1, 8, 8));
    }

    #[test]
    fn test_progress_bar_new() {
        let bar = ProgressBar::new(Rect::new(0, 0, 10, 1));
        assert_eq!(bar.value, 0.0);
        assert_eq!(bar.filled_char, '█');
        assert_eq!(bar.empty_char, '░');
    }

    #[test]
    fn test_progress_bar_builder() {
        let bar = ProgressBar::new(Rect::new(0, 0, 10, 1))
            .with_value(0.5)
            .with_colors(Color::RED, Color::BLUE)
            .with_chars('#', '-');
        assert_eq!(bar.value, 0.5);
        assert_eq!(bar.filled_color, Color::RED);
        assert_eq!(bar.filled_char, '#');
    }

    #[test]
    fn test_progress_bar_render() {
        let bar = ProgressBar::new(Rect::new(0, 0, 10, 1)).with_value(0.5);
        let mut grid = Grid::new(10, 1);
        bar.render(&mut grid);
        assert_eq!(grid.get(0, 0).unwrap().glyph, '█');
        assert_eq!(grid.get(4, 0).unwrap().glyph, '█');
        assert_eq!(grid.get(5, 0).unwrap().glyph, '░');
    }

    #[test]
    fn test_progress_bar_render_empty_rect() {
        let bar = ProgressBar::new(Rect::new(0, 0, 0, 0));
        let mut grid = Grid::new(5, 5);
        bar.render(&mut grid);
    }

    #[test]
    fn test_vertical_progress_bar_new() {
        let bar = VerticalProgressBar::new(Rect::new(0, 0, 1, 10));
        assert_eq!(bar.value, 0.0);
    }

    #[test]
    fn test_vertical_progress_bar_render() {
        let bar = VerticalProgressBar::new(Rect::new(0, 0, 1, 10)).with_value(0.5);
        let mut grid = Grid::new(5, 10);
        bar.render(&mut grid);
        // Bottom 5 cells filled (y=9 down to y=5), top 5 empty
        assert_eq!(grid.get(0, 9).unwrap().glyph, '█');
        assert_eq!(grid.get(0, 5).unwrap().glyph, '█');
        assert_eq!(grid.get(0, 4).unwrap().glyph, '░');
        assert_eq!(grid.get(0, 0).unwrap().glyph, '░');
    }

    #[test]
    fn test_vertical_progress_bar_empty_rect() {
        let bar = VerticalProgressBar::new(Rect::new(0, 0, 0, 0));
        let mut grid = Grid::new(5, 5);
        bar.render(&mut grid);
    }

    #[test]
    fn test_menu_view_new() {
        let menu = MenuView::new(
            Rect::new(0, 0, 10, 5),
            vec!["A".into(), "B".into(), "C".into()],
        );
        assert_eq!(menu.selected_index, 0);
        assert_eq!(menu.scroll_offset, 0);
        assert_eq!(menu.options.len(), 3);
    }

    #[test]
    fn test_menu_view_navigation() {
        let mut menu = MenuView::new(Rect::new(0, 0, 10, 5), vec!["A".into(), "B".into()]);
        menu.next();
        assert_eq!(menu.selected_index, 1);
        menu.next();
        assert_eq!(menu.selected_index, 0);
        menu.prev();
        assert_eq!(menu.selected_index, 1);
    }

    #[test]
    fn test_menu_view_scroll_tracking() {
        let options: Vec<String> = (0..20).map(|i| format!("Item {}", i)).collect();
        let mut menu = MenuView::new(Rect::new(0, 0, 15, 5), options);
        // Visible area is 5 rows (no border). Scrolling should follow selection.
        for _ in 0..5 {
            menu.next();
        }
        assert_eq!(menu.selected_index, 5);
        assert_eq!(menu.scroll_offset, 1);
        // Going back to index 4: still visible in window [1..6], no scroll change
        menu.prev();
        assert_eq!(menu.selected_index, 4);
        assert_eq!(menu.scroll_offset, 1);
        // Go back 3 more to index 1: still in window
        for _ in 0..3 {
            menu.prev();
        }
        assert_eq!(menu.selected_index, 1);
        assert_eq!(menu.scroll_offset, 1);
        // Go back one more to index 0: below window start, scroll back
        menu.prev();
        assert_eq!(menu.selected_index, 0);
        assert_eq!(menu.scroll_offset, 0);
    }

    #[test]
    fn test_menu_view_scroll_wraps() {
        let options: Vec<String> = (0..20).map(|i| format!("Item {}", i)).collect();
        let mut menu = MenuView::new(Rect::new(0, 0, 15, 5), options);
        // Jump to last item via prev (wraps around)
        menu.prev();
        assert_eq!(menu.selected_index, 19);
        // Scroll should show the last 5 items
        assert_eq!(menu.scroll_offset, 15);
    }

    #[test]
    fn test_menu_view_render() {
        let menu = MenuView::new(
            Rect::new(0, 0, 15, 5),
            vec!["Option1".into(), "Option2".into()],
        )
        .with_border(BorderStyle::Rounded, Color::WHITE);
        let mut grid = Grid::new(15, 5);
        menu.render(&mut grid);
    }

    #[test]
    fn test_menu_view_render_scrollable() {
        let options: Vec<String> = (0..20).map(|i| format!("Item {}", i)).collect();
        let mut menu = MenuView::new(Rect::new(0, 0, 15, 5), options);
        menu.selected_index = 10;
        menu.scroll_offset = 6;
        let mut grid = Grid::new(15, 5);
        menu.render(&mut grid);
        // Should render items 6..11, not 0..5
    }

    #[test]
    fn test_menu_view_render_empty_rect() {
        let menu = MenuView::new(Rect::new(0, 0, 0, 0), vec!["A".into()]);
        let mut grid = Grid::new(5, 5);
        menu.render(&mut grid);
    }

    #[test]
    fn test_message_log_view_new() {
        let view = MessageLogView::new(Rect::new(0, 0, 20, 5));
        assert_eq!(view.fg, Color::WHITE);
        assert_eq!(view.border, BorderStyle::None);
    }

    #[test]
    fn test_message_log_view_builder() {
        let view = MessageLogView::new(Rect::new(0, 0, 20, 5))
            .with_border(BorderStyle::Rounded, Color::YELLOW)
            .with_title("Log")
            .with_colors(Color::GREEN, Color::BLACK);
        assert_eq!(view.border, BorderStyle::Rounded);
        assert_eq!(view.title, Some("Log".to_string()));
    }

    #[test]
    fn test_message_log_view_render() {
        let view = MessageLogView::new(Rect::new(0, 0, 20, 5));
        let mut grid = Grid::new(20, 5);
        let msgs = vec!["Hello".into(), "World".into()];
        view.render(&mut grid, &msgs);
    }

    #[test]
    fn test_message_log_view_render_empty_rect() {
        let view = MessageLogView::new(Rect::new(0, 0, 0, 0));
        let mut grid = Grid::new(5, 5);
        view.render(&mut grid, &["msg".into()]);
    }

    #[test]
    fn test_tooltip_new() {
        let tip = Tooltip::new("Hello");
        assert_eq!(tip.text, "Hello");
        assert_eq!(tip.max_width, 30);
    }

    #[test]
    fn test_tooltip_builder() {
        let tip = Tooltip::new("test")
            .with_colors(Color::RED, Color::BLACK)
            .with_max_width(20);
        assert_eq!(tip.fg, Color::RED);
        assert_eq!(tip.max_width, 20);
    }

    #[test]
    fn test_tooltip_render() {
        let tip = Tooltip::new("Info text");
        let mut grid = Grid::new(30, 10);
        tip.render(&mut grid, 5, 5);
    }

    #[test]
    fn test_tooltip_render_clamps_to_grid() {
        let tip = Tooltip::new("Long text that goes off screen");
        let mut grid = Grid::new(10, 5);
        tip.render(&mut grid, 8, 3);
    }

    #[test]
    fn test_performance_overlay_render() {
        let overlay = PerformanceOverlay::new(Rect::new(0, 0, 20, 5));
        let mut grid = Grid::new(20, 5);
        let mut diag = verryte_core::diagnostics::Diagnostics::new();
        diag.record("system_a", std::time::Duration::from_millis(5));
        diag.record("system_b", std::time::Duration::from_millis(20));
        overlay.render(&mut grid, &diag);
    }

    #[test]
    fn test_performance_overlay_render_empty_rect() {
        let overlay = PerformanceOverlay::new(Rect::new(0, 0, 0, 0));
        let mut grid = Grid::new(5, 5);
        let diag = verryte_core::diagnostics::Diagnostics::new();
        overlay.render(&mut grid, &diag);
    }

    #[test]
    fn test_button_widget() {
        let mut button = Button::new("Click Me", Rect::new(0, 0, 12, 3));
        assert_eq!(button.label, "Click Me");
        assert!(!button.hovered);
        assert!(!button.active);

        button = button.with_colors(Color::RED, Color::BLACK);
        assert_eq!(button.fg, Color::RED);

        let mut grid = Grid::new(12, 3);
        button.render(&mut grid);

        // Check normal state rendering
        assert_eq!(grid.get(0, 0).unwrap().bg, Color::BLACK);
        // Border should be drawn
        assert!(grid.get(0, 0).unwrap().glyph != ' ');

        // Check hovered state rendering
        button.set_hovered(true);
        button.render(&mut grid);
        // Bg should be blended with white
        assert!(grid.get(0, 0).unwrap().bg != Color::BLACK);

        // Check active state rendering
        button.set_active(true);
        button.render(&mut grid);
        // active uses WHITE bg and BLACK fg
        assert_eq!(grid.get(0, 0).unwrap().bg, Color::WHITE);
    }

    #[test]
    fn test_table_widget() {
        let table = Table::new(Rect::new(0, 0, 10, 5))
            .with_constraints(vec![Constraint::Fixed(4), Constraint::Remaining])
            .with_headers(vec!["Col1".to_string(), "Col2".to_string()])
            .with_rows(vec![
                vec!["a".to_string(), "b".to_string()],
                vec!["long_string".to_string(), "c".to_string()],
            ])
            .with_alignments(vec![Alignment::Left, Alignment::Right]);

        let mut grid = Grid::new(10, 5);
        table.render(&mut grid);

        // Col1 header "Col1" at row 0 (x=0..4) -> starts at x=0
        // Col2 header "Col2" right-aligned at row 0 (x=4..10) -> length 4, space 2 -> starts at x=6
        assert_eq!(grid.get(0, 0).unwrap().glyph, 'C');
        assert_eq!(grid.get(1, 0).unwrap().glyph, 'o');
        assert_eq!(grid.get(2, 0).unwrap().glyph, 'l');
        assert_eq!(grid.get(3, 0).unwrap().glyph, '1');

        assert_eq!(grid.get(6, 0).unwrap().glyph, 'C');
        assert_eq!(grid.get(7, 0).unwrap().glyph, 'o');
        assert_eq!(grid.get(8, 0).unwrap().glyph, 'l');
        assert_eq!(grid.get(9, 0).unwrap().glyph, '2');

        // Divider row 1
        assert_eq!(grid.get(0, 1).unwrap().glyph, '─');

        // Data row 2 ("a", "b")
        assert_eq!(grid.get(0, 2).unwrap().glyph, 'a');
        assert_eq!(grid.get(9, 2).unwrap().glyph, 'b');

        // Data row 3 ("long_string" -> truncated to "long", "c" -> right aligned to 9)
        assert_eq!(grid.get(0, 3).unwrap().glyph, 'l');
        assert_eq!(grid.get(1, 3).unwrap().glyph, 'o');
        assert_eq!(grid.get(2, 3).unwrap().glyph, 'n');
        assert_eq!(grid.get(3, 3).unwrap().glyph, 'g');
        assert_eq!(grid.get(9, 3).unwrap().glyph, 'c');
    }

    #[test]
    fn test_table_toggle_sort() {
        let mut table = Table::new(Rect::new(0, 0, 10, 5))
            .with_headers(vec!["A".to_string(), "B".to_string()])
            .with_rows(vec![
                vec!["c".to_string(), "3".to_string()],
                vec!["a".to_string(), "1".to_string()],
                vec!["b".to_string(), "2".to_string()],
            ]);

        assert!(table.sort_column.is_none());
        table.toggle_sort(0);
        assert_eq!(table.sort_column, Some(0));
        assert!(table.sort_ascending);
        table.apply_sort();
        assert_eq!(table.rows[0][0], "a");
        assert_eq!(table.rows[1][0], "b");
        assert_eq!(table.rows[2][0], "c");

        table.toggle_sort(0);
        assert_eq!(table.sort_column, Some(0));
        assert!(!table.sort_ascending);
        table.apply_sort();
        assert_eq!(table.rows[0][0], "c");
        assert_eq!(table.rows[1][0], "b");
        assert_eq!(table.rows[2][0], "a");
    }

    #[test]
    fn test_table_filter_rows() {
        let mut table = Table::new(Rect::new(0, 0, 10, 5)).with_rows(vec![
            vec!["a".to_string(), "10".to_string()],
            vec!["b".to_string(), "5".to_string()],
            vec!["c".to_string(), "10".to_string()],
        ]);

        table.filter_rows(|row| row.get(1).map(|s| s == "10").unwrap_or(false));
        let effective = table.effective_rows();
        assert_eq!(effective.len(), 2);
        assert_eq!(effective[0][0], "a");
        assert_eq!(effective[1][0], "c");

        table.clear_filter();
        let effective = table.effective_rows();
        assert_eq!(effective.len(), 3);
    }

    #[test]
    fn test_table_alternating_row_bg() {
        let table = Table::new(Rect::new(0, 0, 10, 7))
            .with_constraints(vec![Constraint::Remaining])
            .with_headers(vec!["Name".to_string()])
            .with_alt_row_bg(Color(20, 20, 30))
            .with_rows(vec![
                vec!["row0".to_string()],
                vec!["row1".to_string()],
                vec!["row2".to_string()],
                vec!["row3".to_string()],
            ]);

        let mut grid = Grid::new(10, 7);
        table.render(&mut grid);

        // Row 0 (even) uses default row_bg (black)
        assert_eq!(grid.get(0, 2).unwrap().bg, Color::BLACK);
        // Row 1 (odd) uses alt_row_bg
        assert_eq!(grid.get(0, 3).unwrap().bg, Color(20, 20, 30));
        // Row 2 (even) uses default row_bg
        assert_eq!(grid.get(0, 4).unwrap().bg, Color::BLACK);
        // Row 3 (odd) uses alt_row_bg
        assert_eq!(grid.get(0, 5).unwrap().bg, Color(20, 20, 30));
    }

    #[test]
    fn test_table_scroll_indicator() {
        let rows: Vec<Vec<String>> = (0..20).map(|i| vec![format!("row{}", i)]).collect();
        let mut table = Table::new(Rect::new(0, 0, 15, 8))
            .with_constraints(vec![Constraint::Remaining])
            .with_headers(vec!["Name".to_string()])
            .with_rows(rows);

        table.scroll_offset = 5;
        let mut grid = Grid::new(15, 8);
        table.render(&mut grid);

        // With no border, inner rect is (0,0,15,8). Scrollbar at x=14.
        let scrollbar_col = 14;
        let has_scrollbar_content = (0..8).any(|y| {
            grid.get(scrollbar_col, y)
                .map(|c| c.glyph == '░' || c.glyph == '█')
                .unwrap_or(false)
        });
        assert!(has_scrollbar_content);
    }
}
