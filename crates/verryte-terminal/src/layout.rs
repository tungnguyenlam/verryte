//! Layout and geometry primitives for terminal UI.

/// Integer rectangle in terminal-cell coordinates.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
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

    pub fn inset(self, dx: u16, dy: u16) -> Rect {
        let x = self.x.saturating_add(dx);
        let y = self.y.saturating_add(dy);
        let width = self.width.saturating_sub(dx.saturating_mul(2));
        let height = self.height.saturating_sub(dy.saturating_mul(2));
        Rect::new(x, y, width, height)
    }

    pub fn translate(self, dx: i16, dy: i16) -> Rect {
        let x = (self.x as i16 + dx).max(0) as u16;
        let y = (self.y as i16 + dy).max(0) as u16;
        Rect::new(x, y, self.width, self.height)
    }

    pub fn split_horizontal(self, split_y: u16) -> (Rect, Rect) {
        let top_h = split_y.min(self.height);
        let bottom_h = self.height.saturating_sub(top_h);
        let top = Rect::new(self.x, self.y, self.width, top_h);
        let bottom = Rect::new(self.x, self.y.saturating_add(top_h), self.width, bottom_h);
        (top, bottom)
    }

    pub fn split_vertical(&self, split_x: u16) -> (Rect, Rect) {
        let left_w = split_x.min(self.width);
        let right_w = self.width.saturating_sub(left_w);
        let left = Rect::new(self.x, self.y, left_w, self.height);
        let right = Rect::new(self.x.saturating_add(left_w), self.y, right_w, self.height);
        (left, right)
    }

    pub fn split_horizontal_absolute(&self, y: u16) -> (Rect, Rect) {
        let split_y = y.saturating_sub(self.y);
        self.split_horizontal(split_y)
    }

    pub fn split_vertical_absolute(&self, x: u16) -> (Rect, Rect) {
        let split_x = x.saturating_sub(self.x);
        self.split_vertical(split_x)
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

/// A declarative layout engine for partitioning a [`Rect`] into multiple sub-regions.
#[derive(Clone, Debug)]
pub struct Layout {
    direction: LayoutDirection,
    constraints: Vec<Constraint>,
    margin: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LayoutDirection {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Constraint {
    Fixed(u16),
    Percent(u8),
    Remaining,
}

impl Layout {
    pub fn horizontal() -> Self {
        Self {
            direction: LayoutDirection::Horizontal,
            constraints: Vec::new(),
            margin: 0,
        }
    }

    pub fn vertical() -> Self {
        Self {
            direction: LayoutDirection::Vertical,
            constraints: Vec::new(),
            margin: 0,
        }
    }

    pub fn with_margin(mut self, margin: u16) -> Self {
        self.margin = margin;
        self
    }

    pub fn add_fixed(mut self, size: u16) -> Self {
        self.constraints.push(Constraint::Fixed(size));
        self
    }

    pub fn add_percent(mut self, percent: u8) -> Self {
        self.constraints.push(Constraint::Percent(percent));
        self
    }

    pub fn add_remaining(mut self) -> Self {
        self.constraints.push(Constraint::Remaining);
        self
    }

    pub fn split(&self, target: Rect) -> Vec<Rect> {
        let target = target.inset(self.margin, self.margin);
        if target.is_empty() {
            return vec![Rect::new(0, 0, 0, 0); self.constraints.len()];
        }

        let total_size = match self.direction {
            LayoutDirection::Horizontal => target.width,
            LayoutDirection::Vertical => target.height,
        };

        let mut fixed_sum = 0;
        let mut percent_sum = 0;
        let mut remaining_count = 0;

        for c in &self.constraints {
            match c {
                Constraint::Fixed(s) => fixed_sum += s,
                Constraint::Percent(p) => {
                    percent_sum += (*p as f32 / 100.0 * total_size as f32) as u16
                }
                Constraint::Remaining => remaining_count += 1,
            }
        }

        let total_allocated = fixed_sum + percent_sum;
        let left_over = total_size.saturating_sub(total_allocated);
        let remaining_size = if remaining_count > 0 {
            left_over / remaining_count
        } else {
            0
        };

        let mut rects = Vec::with_capacity(self.constraints.len());
        let mut offset = 0;

        for c in &self.constraints {
            let size = match c {
                Constraint::Fixed(s) => *s,
                Constraint::Percent(p) => (*p as f32 / 100.0 * total_size as f32) as u16,
                Constraint::Remaining => remaining_size,
            };

            let r = match self.direction {
                LayoutDirection::Horizontal => Rect::new(
                    target.x + offset,
                    target.y,
                    size.min(total_size - offset),
                    target.height,
                ),
                LayoutDirection::Vertical => Rect::new(
                    target.x,
                    target.y + offset,
                    target.width,
                    size.min(total_size - offset),
                ),
            };
            rects.push(r);
            offset += size;
        }

        rects
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

/// Border style options for styled TUI borders.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BorderStyle {
    None,
    Ascii,
    #[default]
    Single,
    Double,
    Heavy,
    Rounded,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_splits() {
        let r = Rect::new(10, 20, 30, 40);
        let (top, bottom) = r.split_horizontal(15);
        assert_eq!(top, Rect::new(10, 20, 30, 15));
        assert_eq!(bottom, Rect::new(10, 35, 30, 25));

        let (left, right) = r.split_vertical(10);
        assert_eq!(left, Rect::new(10, 20, 10, 40));
        assert_eq!(right, Rect::new(20, 20, 20, 40));

        let (top_a, bottom_a) = r.split_horizontal_absolute(25);
        assert_eq!(top_a, Rect::new(10, 20, 30, 5));
        assert_eq!(bottom_a, Rect::new(10, 25, 30, 35));

        let (left_a, right_a) = r.split_vertical_absolute(15);
        assert_eq!(left_a, Rect::new(10, 20, 5, 40));
        assert_eq!(right_a, Rect::new(15, 20, 25, 40));
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
    fn rect_area_computes_correctly() {
        assert_eq!(Rect::new(0, 0, 3, 4).area(), 12);
        assert_eq!(Rect::new(0, 0, 0, 5).area(), 0);
    }
}
