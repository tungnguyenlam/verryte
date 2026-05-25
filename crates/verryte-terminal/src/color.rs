//! 24-bit RGB color and blending modes.

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

    /// Blend another color onto this one with a given alpha value (0.0 = self, 1.0 = other).
    pub fn blend_alpha(self, other: Color, alpha: f32) -> Color {
        let a = alpha.clamp(0.0, 1.0);
        Color(
            (self.0 as f32 * (1.0 - a) + other.0 as f32 * a).round() as u8,
            (self.1 as f32 * (1.0 - a) + other.1 as f32 * a).round() as u8,
            (self.2 as f32 * (1.0 - a) + other.2 as f32 * a).round() as u8,
        )
    }

    /// Blend another color onto this one using the specified mode.
    pub fn blend(self, other: Color, mode: BlendMode) -> Color {
        match mode {
            BlendMode::Normal => other,
            BlendMode::Add => Color(
                (self.0 as u16 + other.0 as u16).min(255) as u8,
                (self.1 as u16 + other.1 as u16).min(255) as u8,
                (self.2 as u16 + other.2 as u16).min(255) as u8,
            ),
            BlendMode::Multiply => Color(
                ((self.0 as f32 / 255.0) * (other.0 as f32 / 255.0) * 255.0).round() as u8,
                ((self.1 as f32 / 255.0) * (other.1 as f32 / 255.0) * 255.0).round() as u8,
                ((self.2 as f32 / 255.0) * (other.2 as f32 / 255.0) * 255.0).round() as u8,
            ),
            BlendMode::Screen => {
                let s_r = self.0 as f32 / 255.0;
                let s_g = self.1 as f32 / 255.0;
                let s_b = self.2 as f32 / 255.0;
                let o_r = other.0 as f32 / 255.0;
                let o_g = other.1 as f32 / 255.0;
                let o_b = other.2 as f32 / 255.0;
                Color(
                    (255.0 * (1.0 - (1.0 - s_r) * (1.0 - o_r))).round() as u8,
                    (255.0 * (1.0 - (1.0 - s_g) * (1.0 - o_g))).round() as u8,
                    (255.0 * (1.0 - (1.0 - s_b) * (1.0 - o_b))).round() as u8,
                )
            }
            BlendMode::Overlay => {
                let blend_chan = |bg_val: u8, fg_val: u8| -> u8 {
                    let bg = bg_val as f32 / 255.0;
                    let fg = fg_val as f32 / 255.0;
                    let res = if bg < 0.5 {
                        2.0 * bg * fg
                    } else {
                        1.0 - 2.0 * (1.0 - bg) * (1.0 - fg)
                    };
                    (res * 255.0).round().clamp(0.0, 255.0) as u8
                };
                Color(
                    blend_chan(self.0, other.0),
                    blend_chan(self.1, other.1),
                    blend_chan(self.2, other.2),
                )
            }
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BlendMode {
    Normal,
    Add,
    Multiply,
    Screen,
    Overlay,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_operations() {
        let c1 = Color(0, 100, 200);
        let c2 = Color(100, 200, 50);
        let mid = c1.lerp(c2, 0.5);
        assert_eq!(mid, Color(50, 150, 125));

        assert_eq!(Color::from_hex("#FF00FF").unwrap(), Color(255, 0, 255));
        assert_eq!(Color::from_hex("00FF00").unwrap(), Color(0, 255, 0));

        let color = Color(128, 64, 192);
        let (h, s, v) = color.to_hsv();
        let back = Color::from_hsv(h, s, v);
        assert!((back.0 as i16 - color.0 as i16).abs() <= 1);
        assert!((back.1 as i16 - color.1 as i16).abs() <= 1);
        assert!((back.2 as i16 - color.2 as i16).abs() <= 1);
    }

    #[test]
    fn test_color_blend() {
        let bg = Color(100, 100, 100);
        let fg = Color(200, 150, 100);
        let normal = bg.blend(fg, BlendMode::Normal);
        assert_eq!(normal, fg);
        let alpha_half = bg.blend_alpha(fg, 0.5);
        assert_eq!(alpha_half, Color(150, 125, 100));
    }
}
