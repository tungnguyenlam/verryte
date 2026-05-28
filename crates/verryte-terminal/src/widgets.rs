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
            let lines = if msg.contains('[') {
                if let Ok(rich_lines) = Grid::parse_and_wrap_rich(msg, width) {
                    rich_lines
                } else {
                    let wrapped = crate::wrap_text(msg, width as usize);
                    wrapped.into_iter().map(|line| {
                        vec![crate::RichTextSegment {
                            text: line,
                            fg: self.fg,
                            bg: self.bg,
                            attrs: crate::CellAttrs::NONE,
                        }]
                    }).collect()
                }
            } else {
                let wrapped = crate::wrap_text(msg, width as usize);
                wrapped.into_iter().map(|line| {
                    vec![crate::RichTextSegment {
                        text: line,
                        fg: self.fg,
                        bg: self.bg,
                        attrs: crate::CellAttrs::NONE,
                    }]
                }).collect()
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
            self.selected_index =
                (self.selected_index + self.options.len() - 1) % self.options.len();
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
                (
                    self.selected_fg,
                    format!("{}{}", self.selection_marker, option),
                )
            } else {
                let padding = " ".repeat(self.selection_marker.chars().count());
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
