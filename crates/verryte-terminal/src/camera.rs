use crate::layout::Rect;

/// Viewport camera that manages position and zoom levels.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Camera {
    pub center_x: f32,
    pub center_y: f32,
    pub zoom: f32,
    pub smooth: bool,
    pub target_x: f32,
    pub target_y: f32,
    pub lerp_factor: f32,
}

impl Camera {
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            center_x: x,
            center_y: y,
            zoom: 1.0,
            smooth: false,
            target_x: x,
            target_y: y,
            lerp_factor: 0.1,
        }
    }

    pub fn with_smooth(mut self, factor: f32) -> Self {
        self.smooth = true;
        self.lerp_factor = factor;
        self
    }

    pub fn look_at(&mut self, x: f32, y: f32) {
        if self.smooth {
            self.target_x = x;
            self.target_y = y;
        } else {
            self.center_x = x;
            self.center_y = y;
            self.target_x = x;
            self.target_y = y;
        }
    }

    pub fn tick(&mut self) {
        if self.smooth {
            self.center_x += (self.target_x - self.center_x) * self.lerp_factor;
            self.center_y += (self.target_y - self.center_y) * self.lerp_factor;
        }
    }

    /// Calculate the top-left corner of the viewport for a given window size.
    pub fn top_left(&self, width: u16, height: u16) -> (i16, i16) {
        let zoomed_w = (width as f32 / self.zoom).round() as u16;
        let zoomed_h = (height as f32 / self.zoom).round() as u16;
        let x = (self.center_x - (zoomed_w as f32 / 2.0)).round() as i16;
        let y = (self.center_y - (zoomed_h as f32 / 2.0)).round() as i16;
        (x, y)
    }

    /// Return the [`Rect`] representing the current viewport in grid coordinates.
    pub fn viewport_rect(&self, width: u16, height: u16) -> Rect {
        let (x, y) = self.top_left(width, height);
        let zoomed_w = (width as f32 / self.zoom).round() as u16;
        let zoomed_h = (height as f32 / self.zoom).round() as u16;
        Rect {
            x: x.max(0) as u16,
            y: y.max(0) as u16,
            width: zoomed_w,
            height: zoomed_h,
        }
    }

    /// Clamp the camera's center and target position within the given boundaries.
    pub fn clamp_to_bounds(&mut self, min_x: f32, min_y: f32, max_x: f32, max_y: f32) {
        self.center_x = self.center_x.clamp(min_x, max_x);
        self.center_y = self.center_y.clamp(min_y, max_y);
        self.target_x = self.target_x.clamp(min_x, max_x);
        self.target_y = self.target_y.clamp(min_y, max_y);
    }
}
