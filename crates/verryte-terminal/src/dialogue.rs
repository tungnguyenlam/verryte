use crate::color::Color;
use crate::grid::{Cell, Grid};
use crate::layout::{BorderStyle, Rect};

#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Default)]
pub enum PortraitAnimation {
    #[default]
    None,
    Bob {
        speed: f32,
        amplitude: f32,
    },
    Glow {
        speed: f32,
        color: Color,
    },
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Portrait {
    pub grid: Grid,
    pub animation: PortraitAnimation,
    pub time: f32,
}

impl Portrait {
    pub fn new(grid: Grid) -> Self {
        Self {
            grid,
            animation: PortraitAnimation::None,
            time: 0.0,
        }
    }

    pub fn with_animation(mut self, animation: PortraitAnimation) -> Self {
        self.animation = animation;
        self
    }

    pub fn tick(&mut self, dt: f32) {
        self.time += dt;
    }

    pub fn bob_y_offset(&self) -> i32 {
        match self.animation {
            PortraitAnimation::Bob { speed, amplitude } => {
                ((self.time * speed).sin() * amplitude) as i32
            }
            _ => 0,
        }
    }

    pub fn glow_alpha(&self) -> f32 {
        match self.animation {
            PortraitAnimation::Glow { speed, .. } => {
                ((self.time * speed).sin() * 0.5 + 0.5).clamp(0.0, 1.0)
            }
            _ => 0.0,
        }
    }

    pub fn glow_color(&self) -> Option<Color> {
        match self.animation {
            PortraitAnimation::Glow { color, .. } => Some(color),
            _ => None,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DialogueTheme {
    Arcane,
    Forest,
    Blood,
    Frost,
    Shadow,
    Dungeon,
}

impl std::fmt::Display for DialogueTheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Arcane => write!(f, "Arcane"),
            Self::Forest => write!(f, "Forest"),
            Self::Blood => write!(f, "Blood"),
            Self::Frost => write!(f, "Frost"),
            Self::Shadow => write!(f, "Shadow"),
            Self::Dungeon => write!(f, "Dungeon"),
        }
    }
}

impl DialogueTheme {
    /// Returns the main theme color (used for border).
    pub fn theme_color(&self) -> Color {
        match self {
            Self::Arcane => Color(150, 50, 250),
            Self::Forest => Color(50, 180, 80),
            Self::Blood => Color(200, 20, 20),
            Self::Frost => Color(80, 180, 240),
            Self::Shadow => Color(80, 0, 120),
            Self::Dungeon => Color::GREY,
        }
    }

    /// Returns the title color.
    pub fn title_color(&self) -> Color {
        match self {
            Self::Arcane => Color(220, 180, 255),
            Self::Forest => Color(180, 255, 180),
            Self::Blood => Color(255, 100, 100),
            Self::Frost => Color(180, 240, 255),
            Self::Shadow => Color(200, 100, 255),
            Self::Dungeon => Color::YELLOW,
        }
    }

    /// Returns the background color.
    pub fn bg_color(&self) -> Color {
        match self {
            Self::Arcane => Color(10, 5, 20),
            Self::Forest => Color(5, 15, 10),
            Self::Blood => Color(20, 5, 5),
            Self::Frost => Color(5, 10, 20),
            Self::Shadow => Color(10, 0, 15),
            Self::Dungeon => Color::BLACK,
        }
    }
}

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
    pub typewriter_speed: f32,
    pub portrait: Option<Portrait>,
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
            portrait: None,
        }
    }

    pub fn with_theme(mut self, theme: DialogueTheme) -> Self {
        self.border_color = theme.theme_color();
        self.title_fg = theme.title_color();
        self.bg = theme.bg_color();
        self.text_fg = Color::WHITE;
        self
    }

    pub fn set_portrait(&mut self, portrait: Portrait) {
        self.portrait = Some(portrait);
    }

    pub fn clear_portrait(&mut self) {
        self.portrait = None;
    }

    pub fn tick_portrait(&mut self, dt: f32) {
        if let Some(ref mut p) = self.portrait {
            p.tick(dt);
        }
    }

    #[allow(clippy::too_many_arguments)]
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

        // Draw portrait if provided (parameter takes priority, then stored portrait)
        let text_x_offset = if let Some(p) = portrait {
            let px = inner.x + 1;
            let py = inner.y + (inner.height.saturating_sub(p.height())) / 2;
            grid.blit(p, px as i32, py as i32);
            p.width() + 2
        } else if let Some(ref stored_portrait) = self.portrait {
            let p = &stored_portrait.grid;
            let px = inner.x + 1;
            let bob_offset = stored_portrait.bob_y_offset();
            let base_y = inner.y as i32 + (inner.height.saturating_sub(p.height())) as i32 / 2;
            let py = (base_y + bob_offset).max(0) as u16;
            grid.blit(p, px as i32, py as i32);

            if let Some(glow_color) = stored_portrait.glow_color() {
                let alpha = stored_portrait.glow_alpha();
                let portrait_rect = Rect::new(
                    px,
                    py,
                    p.width().min(inner.width),
                    p.height().min(inner.height),
                );
                grid.apply_tint(
                    portrait_rect,
                    glow_color,
                    alpha * 0.15,
                    crate::color::BlendMode::Screen,
                );
            }

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
            let start_y = choice_y_start.max(current_y + 1);

            for (cy, (i, choice)) in (start_y..).zip(choices.iter().enumerate()) {
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
    pub chosen: Option<usize>,
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
            chosen: None,
        }
    }

    pub fn with_choices(mut self, choices: Vec<String>) -> Self {
        self.choices = choices;
        self
    }

    pub fn update(&mut self, dt: f32, speed: f32) -> usize {
        let prev = self.visible_chars.floor() as usize;
        if self.visible_chars < self.text.len() as f32 {
            self.visible_chars += speed * dt;
            if self.visible_chars >= self.text.len() as f32 {
                self.visible_chars = self.text.len() as f32;
                if self.choices.is_empty() {
                    self.finished = true;
                }
            }
        }
        let next = self.visible_chars.floor() as usize;
        next.saturating_sub(prev)
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

impl std::fmt::Display for DialogueState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Dialogue({:?}: {:.0}/{:.0} chars, finished={}, choices={})",
            self.title,
            self.visible_chars,
            self.text.len() as f32,
            self.finished,
            self.choices.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dialogue_state_new() {
        let state = DialogueState::new("Title", "Hello world");
        assert_eq!(state.title, "Title");
        assert_eq!(state.text, "Hello world");
        assert!(state.choices.is_empty());
        assert_eq!(state.selected_choice, 0);
        assert_eq!(state.visible_chars, 0.0);
        assert!(!state.finished);
        assert!(state.chosen.is_none());
    }

    #[test]
    fn test_dialogue_state_with_choices() {
        let state = DialogueState::new("T", "text").with_choices(vec!["Yes".into(), "No".into()]);
        assert_eq!(state.choices.len(), 2);
    }

    #[test]
    fn test_dialogue_state_update_typing() {
        let mut state = DialogueState::new("T", "abcdef");
        let chars = state.update(0.1, 3.0); // 3 chars/sec * 0.1 sec = 0.3 chars
        assert_eq!(chars, 0); // not enough for a full char
        assert!(state.is_typing());
    }

    #[test]
    fn test_dialogue_state_update_finished_no_choices() {
        let mut state = DialogueState::new("T", "ab");
        state.update(1.0, 100.0);
        assert!(!state.is_typing());
        assert!(state.finished);
    }

    #[test]
    fn test_dialogue_state_update_finished_with_choices() {
        let mut state = DialogueState::new("T", "ab").with_choices(vec!["A".into(), "B".into()]);
        state.update(1.0, 100.0);
        assert!(!state.is_typing());
        assert!(!state.finished);
    }

    #[test]
    fn test_dialogue_state_skip_typing() {
        let mut state = DialogueState::new("T", "long text here");
        state.skip_typing();
        assert!(!state.is_typing());
        assert!(state.finished);
    }

    #[test]
    fn test_dialogue_state_skip_with_choices() {
        let mut state = DialogueState::new("T", "text").with_choices(vec!["A".into()]);
        state.skip_typing();
        assert!(!state.is_typing());
        assert!(!state.finished);
    }

    #[test]
    fn test_dialogue_state_next_prev_choice() {
        let mut state =
            DialogueState::new("T", "text").with_choices(vec!["A".into(), "B".into(), "C".into()]);
        assert_eq!(state.selected_choice, 0);
        state.next_choice();
        assert_eq!(state.selected_choice, 1);
        state.next_choice();
        assert_eq!(state.selected_choice, 2);
        state.next_choice();
        assert_eq!(state.selected_choice, 0);
        state.prev_choice();
        assert_eq!(state.selected_choice, 2);
    }

    #[test]
    fn test_dialogue_state_empty_choices_noop() {
        let mut state = DialogueState::new("T", "text");
        state.next_choice();
        state.prev_choice();
        assert_eq!(state.selected_choice, 0);
    }

    #[test]
    fn test_dialogue_box_render_empty_rect() {
        let b = DialogueBox::new(Rect::new(0, 0, 0, 0));
        let mut grid = Grid::new(20, 20);
        b.render(&mut grid, "T", "Text", 0, None, &[], None);
    }

    #[test]
    fn test_dialogue_box_render_basic() {
        let b = DialogueBox::new(Rect::new(2, 2, 16, 8));
        let mut grid = Grid::new(20, 20);
        b.render(&mut grid, "Title", "Hello", 5, None, &[], None);
    }

    #[test]
    fn test_dialogue_box_render_with_choices() {
        let b = DialogueBox::new(Rect::new(2, 2, 20, 12));
        let mut grid = Grid::new(30, 20);
        let choices = vec!["Yes".into(), "No".into()];
        b.render(&mut grid, "Q", "Question?", 100, None, &choices, Some(0));
    }

    #[test]
    fn test_dialogue_box_render_with_portrait() {
        let b = DialogueBox::new(Rect::new(2, 2, 20, 10));
        let mut grid = Grid::new(30, 20);
        let portrait = Grid::new(6, 6);
        b.render(&mut grid, "Title", "Text", 4, Some(&portrait), &[], None);
    }

    #[test]
    fn test_dialogue_box_themes() {
        for theme in [
            DialogueTheme::Arcane,
            DialogueTheme::Forest,
            DialogueTheme::Blood,
            DialogueTheme::Frost,
            DialogueTheme::Dungeon,
        ] {
            let b = DialogueBox::new(Rect::new(0, 0, 10, 5)).with_theme(theme);
            assert_ne!(b.border_color, Color::BLACK);
            assert_eq!(b.border_color, theme.theme_color());
            assert_eq!(b.title_fg, theme.title_color());
            assert_eq!(b.bg, theme.bg_color());
        }
    }

    #[test]
    fn test_dialogue_theme_display() {
        assert_eq!(DialogueTheme::Arcane.to_string(), "Arcane");
        assert_eq!(DialogueTheme::Forest.to_string(), "Forest");
        assert_eq!(DialogueTheme::Blood.to_string(), "Blood");
        assert_eq!(DialogueTheme::Frost.to_string(), "Frost");
        assert_eq!(DialogueTheme::Dungeon.to_string(), "Dungeon");
    }

    #[test]
    fn test_dialogue_state_display() {
        let state = DialogueState::new("Greeting", "Hello world");
        let s = format!("{state}");
        assert!(s.contains("Greeting"));
        assert!(s.contains("finished=false"));
        assert!(s.contains("choices=0"));
    }

    #[test]
    fn test_dialogue_state_display_with_choices() {
        let state = DialogueState::new("Q", "Pick one").with_choices(vec!["A".into(), "B".into()]);
        let s = format!("{state}");
        assert!(s.contains("choices=2"));
    }

    #[test]
    fn test_portrait_new_and_animation() {
        let grid = Grid::new(6, 6);
        let portrait = Portrait::new(grid).with_animation(PortraitAnimation::Bob {
            speed: 2.0,
            amplitude: 1.0,
        });
        assert_eq!(
            portrait.animation,
            PortraitAnimation::Bob {
                speed: 2.0,
                amplitude: 1.0
            }
        );
        assert_eq!(portrait.time, 0.0);
    }

    #[test]
    fn test_portrait_bob_y_offset() {
        let grid = Grid::new(4, 4);
        let mut portrait = Portrait::new(grid).with_animation(PortraitAnimation::Bob {
            speed: std::f32::consts::PI,
            amplitude: 2.0,
        });
        portrait.tick(0.0);
        let offset_0 = portrait.bob_y_offset();
        portrait.tick(0.5);
        let offset_0_5 = portrait.bob_y_offset();
        assert_ne!(offset_0, offset_0_5);
    }

    #[test]
    fn test_portrait_glow() {
        let grid = Grid::new(4, 4);
        let mut portrait = Portrait::new(grid).with_animation(PortraitAnimation::Glow {
            speed: 1.0,
            color: Color(100, 200, 255),
        });
        portrait.tick(0.0);
        let alpha = portrait.glow_alpha();
        assert!((0.0..=1.0).contains(&alpha));
        assert_eq!(portrait.glow_color(), Some(Color(100, 200, 255)));
    }

    #[test]
    fn test_dialogue_box_set_portrait() {
        let mut b = DialogueBox::new(Rect::new(2, 2, 30, 10));
        assert!(b.portrait.is_none());
        let portrait = Portrait::new(Grid::new(6, 6));
        b.set_portrait(portrait);
        assert!(b.portrait.is_some());
        b.clear_portrait();
        assert!(b.portrait.is_none());
    }

    #[test]
    fn test_dialogue_box_render_with_stored_portrait() {
        let mut b = DialogueBox::new(Rect::new(2, 2, 30, 10));
        let portrait = Portrait::new(Grid::new(6, 6)).with_animation(PortraitAnimation::Bob {
            speed: 1.0,
            amplitude: 0.5,
        });
        b.set_portrait(portrait);
        b.tick_portrait(1.0);
        let mut grid = Grid::new(40, 20);
        b.render(&mut grid, "Speaker", "Hello there!", 12, None, &[], None);
        assert!(b.portrait.as_ref().unwrap().time > 0.0);
    }

    #[test]
    fn test_portrait_none_animation() {
        let portrait = Portrait::new(Grid::new(2, 2));
        assert_eq!(portrait.bob_y_offset(), 0);
        assert_eq!(portrait.glow_alpha(), 0.0);
        assert_eq!(portrait.glow_color(), None);
    }
}
