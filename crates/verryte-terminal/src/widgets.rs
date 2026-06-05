use crate::color::Color;
use crate::grid::{Cell, Grid};
use crate::layout::{BorderStyle, Rect};

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

            grid.write_str(inner_rect.x, y, &text, fg, self.bg);
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
}
