use crate::color::Color;
use crate::grid::Cell;

/// A named collection of colors for consistent theming.
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
    pub fn dark_dungeon() -> Self {
        Self {
            name: "dark_dungeon".to_string(),
            background: Color(15, 15, 20),
            foreground: Color(200, 200, 200),
            primary: Color(80, 80, 90),
            secondary: Color(50, 50, 55),
            accent: Color(220, 200, 80),
            danger: Color(220, 80, 80),
            success: Color(80, 180, 220),
            info: Color(100, 220, 100),
            warning: Color(220, 150, 50),
            ui_border: Color(100, 100, 110),
            ui_title: Color(220, 200, 80),
            ui_text: Color(200, 200, 200),
            ui_highlight: Color(100, 220, 100),
            ui_muted: Color(100, 100, 100),
        }
    }

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

    pub fn high_contrast() -> Self {
        Self {
            name: "high_contrast".to_string(),
            background: Color::BLACK,
            foreground: Color::WHITE,
            primary: Color::CYAN,
            secondary: Color::WHITE,
            accent: Color::YELLOW,
            danger: Color::RED,
            success: Color::GREEN,
            info: Color::CYAN,
            warning: Color::YELLOW,
            ui_border: Color::WHITE,
            ui_title: Color::YELLOW,
            ui_text: Color::WHITE,
            ui_highlight: Color::GREEN,
            ui_muted: Color::GREY,
        }
    }

    pub fn secondary_cell(&self, glyph: char) -> Cell {
        Cell::new(glyph)
            .with_fg(self.foreground)
            .with_bg(self.secondary)
    }

    pub fn primary_cell(&self, glyph: char) -> Cell {
        Cell::new(glyph)
            .with_fg(self.primary)
            .with_bg(self.background)
    }

    pub fn info_cell(&self, glyph: char) -> Cell {
        Cell::new(glyph).with_fg(self.info).with_bg(self.secondary)
    }

    pub fn danger_cell(&self, glyph: char) -> Cell {
        Cell::new(glyph)
            .with_fg(self.danger)
            .with_bg(self.secondary)
    }

    pub fn accent_cell(&self, glyph: char) -> Cell {
        Cell::new(glyph)
            .with_fg(self.accent)
            .with_bg(self.secondary)
    }

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
