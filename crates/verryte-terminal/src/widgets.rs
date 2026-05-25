use crate::color::Color;
use crate::grid::{Cell, Grid};
use crate::layout::{BorderStyle, Rect};

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
            let wrapped = crate::wrap_text(msg, width as usize);
            for line in wrapped.iter().rev() {
                if current_y < inner_rect.y {
                    return;
                }
                grid.write_str(inner_rect.x, current_y, line, self.fg, self.bg);
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

        let mut sorted_systems: Vec<_> = diagnostics.systems.iter().collect();
        sorted_systems.sort_by_key(|b| std::cmp::Reverse(b.1.last_duration));

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

/// A vertical menu widget with a selectable active item.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MenuView {
    pub rect: Rect,
    pub options: Vec<String>,
    pub selected_index: usize,
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
        }
    }

    pub fn prev(&mut self) {
        if !self.options.is_empty() {
            self.selected_index = (self.selected_index + self.options.len() - 1) % self.options.len();
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

        for (i, option) in self.options.iter().enumerate() {
            let y = inner_rect.y + i as u16;
            if y >= inner_rect.bottom() {
                break;
            }

            let is_selected = i == self.selected_index;
            let (fg, text) = if is_selected {
                (self.selected_fg, format!("{}{}", self.selection_marker, option))
            } else {
                let padding = " ".repeat(self.selection_marker.chars().count());
                (self.normal_fg, format!("{}{}", padding, option))
            };

            grid.write_str(inner_rect.x, y, &text, fg, self.bg);
        }
    }
}
