use crate::color::Color;
use crate::grid::{Cell, Grid};
use crate::layout::{BorderStyle, Rect};

/// A UI widget for rendering interactive dialogue boxes.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DialogueBox {
    pub rect: Rect,
    pub border: BorderStyle,
    pub border_color: Color,
    pub bg: Color,
    pub title_fg: Color,
    pub text_fg: Color,
    pub typewriter_speed: f32, // chars per second
}

impl DialogueBox {
    pub fn new(rect: Rect) -> Self {
        Self {
            rect,
            border: BorderStyle::Rounded,
            border_color: Color::WHITE,
            bg: Color::BLACK,
            title_fg: Color::YELLOW,
            text_fg: Color::WHITE,
            typewriter_speed: 30.0,
        }
    }

    pub fn render(
        &self,
        grid: &mut Grid,
        title: &str,
        text: &str,
        visible_chars: usize,
        portrait: Option<&Grid>,
        choices: &[String],
        selected_choice: Option<usize>,
    ) {
        if self.rect.is_empty() {
            return;
        }

        // Draw background and border
        grid.fill_rect(self.rect, Cell::new(' ').with_bg(self.bg));
        grid.draw_border_styled(self.rect, self.border, self.border_color, self.bg);
        if !title.is_empty() {
            grid.draw_title(self.rect, title, self.border_color, self.bg, self.title_fg);
        }

        let inner = self.rect.inset(1, 1);
        if inner.is_empty() {
            return;
        }

        // Draw portrait if provided
        let text_x_offset = if let Some(p) = portrait {
            let px = inner.x + 1;
            let py = inner.y + (inner.height.saturating_sub(p.height())) / 2;
            grid.blit(p, px as i32, py as i32);
            p.width() + 2
        } else {
            0
        };

        let text_rect = Rect::new(
            inner.x + text_x_offset,
            inner.y,
            inner.width.saturating_sub(text_x_offset),
            inner.height,
        );

        if text_rect.is_empty() {
            return;
        }

        // Draw visible text
        let visible_text: String = text.chars().take(visible_chars).collect();
        let lines = crate::wrap_text(&visible_text, text_rect.width as usize);

        let mut current_y = text_rect.y;
        for line in lines.iter().take(text_rect.height as usize) {
            grid.write_str(text_rect.x, current_y, line, self.text_fg, self.bg);
            current_y += 1;
        }

        // Draw choices if typing is finished
        if visible_chars >= text.len() && !choices.is_empty() {
            let choice_y_start = text_rect.bottom().saturating_sub(choices.len() as u16);
            let mut cy = choice_y_start.max(current_y + 1);

            for (i, choice) in choices.iter().enumerate() {
                if cy >= text_rect.bottom() {
                    break;
                }
                let prefix = if selected_choice == Some(i) {
                    "> "
                } else {
                    "  "
                };
                let color = if selected_choice == Some(i) {
                    self.title_fg
                } else {
                    self.text_fg
                };
                let choice_text = format!("{}{}", prefix, choice);
                grid.write_str(text_rect.x, cy, &choice_text, color, self.bg);
                cy += 1;
            }
        }
    }
}

/// Manages the state of a dialogue sequence.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DialogueState {
    pub title: String,
    pub text: String,
    pub choices: Vec<String>,
    pub selected_choice: usize,
    pub visible_chars: f32,
    pub finished: bool,
}

impl DialogueState {
    pub fn new(title: &str, text: &str) -> Self {
        Self {
            title: title.to_string(),
            text: text.to_string(),
            choices: Vec::new(),
            selected_choice: 0,
            visible_chars: 0.0,
            finished: false,
        }
    }

    pub fn with_choices(mut self, choices: Vec<String>) -> Self {
        self.choices = choices;
        self
    }

    pub fn update(&mut self, dt: f32, speed: f32) {
        if self.visible_chars < self.text.len() as f32 {
            self.visible_chars += speed * dt;
            if self.visible_chars >= self.text.len() as f32 {
                self.visible_chars = self.text.len() as f32;
                if self.choices.is_empty() {
                    self.finished = true;
                }
            }
        }
    }

    pub fn is_typing(&self) -> bool {
        self.visible_chars < self.text.len() as f32
    }

    pub fn skip_typing(&mut self) {
        self.visible_chars = self.text.len() as f32;
        if self.choices.is_empty() {
            self.finished = true;
        }
    }

    pub fn next_choice(&mut self) {
        if !self.choices.is_empty() {
            self.selected_choice = (self.selected_choice + 1) % self.choices.len();
        }
    }

    pub fn prev_choice(&mut self) {
        if !self.choices.is_empty() {
            if self.selected_choice == 0 {
                self.selected_choice = self.choices.len() - 1;
            } else {
                self.selected_choice -= 1;
            }
        }
    }
}
