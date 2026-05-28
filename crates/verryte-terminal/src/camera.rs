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

    pub target_zoom: f32,
    pub zoom_lerp: f32,

    pub shake_intensity: f32,
    pub shake_decay: f32,
    pub shake_offset_x: f32,
    pub shake_offset_y: f32,
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
            target_zoom: 1.0,
            zoom_lerp: 0.1,
            shake_intensity: 0.0,
            shake_decay: 0.9,
            shake_offset_x: 0.0,
            shake_offset_y: 0.0,
        }
    }

    pub fn with_smooth(mut self, factor: f32) -> Self {
        self.smooth = true;
        self.lerp_factor = factor;
        self
    }

    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom;
        self.target_zoom = zoom;
    }

    pub fn zoom_to(&mut self, zoom: f32) {
        if self.smooth {
            self.target_zoom = zoom;
        } else {
            self.zoom = zoom;
            self.target_zoom = zoom;
        }
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

    pub fn shake(&mut self, intensity: f32) {
        self.shake_intensity = intensity;
    }

    pub fn tick(&mut self, rng: &mut verryte_core::Rng) {
        if self.smooth {
            self.center_x += (self.target_x - self.center_x) * self.lerp_factor;
            self.center_y += (self.target_y - self.center_y) * self.lerp_factor;
            self.zoom += (self.target_zoom - self.zoom) * self.zoom_lerp;
        }

        if self.shake_intensity > 0.01 {
            self.shake_offset_x = (rng.next_f64() as f32 * 2.0 - 1.0) * self.shake_intensity;
            self.shake_offset_y = (rng.next_f64() as f32 * 2.0 - 1.0) * self.shake_intensity;
            self.shake_intensity *= self.shake_decay;
        } else {
            self.shake_offset_x = 0.0;
            self.shake_offset_y = 0.0;
            self.shake_intensity = 0.0;
        }
    }

    /// Calculate the top-left corner of the viewport for a given window size.
    pub fn top_left(&self, width: u16, height: u16) -> (i16, i16) {
        let zoomed_w = (width as f32 / self.zoom).round() as u16;
        let zoomed_h = (height as f32 / self.zoom).round() as u16;
        let x = (self.center_x + self.shake_offset_x - (zoomed_w as f32 / 2.0)).round() as i16;
        let y = (self.center_y + self.shake_offset_y - (zoomed_h as f32 / 2.0)).round() as i16;
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
    pub fn clamp_to_bounds(
        &mut self,
        min_x: f32,
        min_y: f32,
        max_x: f32,
        max_y: f32,
        width: u16,
        height: u16,
    ) {
        let zoomed_w = (width as f32 / self.zoom).round() as u16;
        let zoomed_h = (height as f32 / self.zoom).round() as u16;

        let half_w = zoomed_w as f32 / 2.0;
        let half_h = zoomed_h as f32 / 2.0;

        let clamp_min_x = min_x + half_w;
        let clamp_max_x = max_x - half_w;
        let clamp_min_y = min_y + half_h;
        let clamp_max_y = max_y - half_h;

        if clamp_max_x > clamp_min_x {
            self.center_x = self.center_x.clamp(clamp_min_x, clamp_max_x);
            self.target_x = self.target_x.clamp(clamp_min_x, clamp_max_x);
        } else {
            self.center_x = (min_x + max_x) / 2.0;
            self.target_x = self.center_x;
        }

        if clamp_max_y > clamp_min_y {
            self.center_y = self.center_y.clamp(clamp_min_y, clamp_max_y);
            self.target_y = self.target_y.clamp(clamp_min_y, clamp_max_y);
        } else {
            self.center_y = (min_y + max_y) / 2.0;
            self.target_y = self.center_y;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use verryte_core::Rng;

    #[test]
    fn test_camera_zoom_shake_clamp() {
        let mut camera = Camera::new(10.0, 10.0);
        assert_eq!(camera.zoom, 1.0);

        // Test zoom
        camera.zoom_to(2.0);
        assert_eq!(camera.zoom, 2.0);

        // Test shake
        camera.shake(5.0);
        assert_eq!(camera.shake_intensity, 5.0);

        let mut rng = Rng::seed(12345);
        camera.tick(&mut rng);
        assert!(camera.shake_offset_x.abs() > 0.0);
        assert!(camera.shake_offset_y.abs() > 0.0);
        assert_eq!(camera.shake_intensity, 4.5); // 5.0 * 0.9 (decay)

        // Test clamp bounds
        camera.set_zoom(1.0);
        camera.clamp_to_bounds(0.0, 0.0, 20.0, 20.0, 10, 10);
        // zoomed width = 10, so half_w = 5. Center must be clamped to [5, 15]
        assert_eq!(camera.center_x, 10.0);

        camera.look_at(25.0, 25.0);
        camera.clamp_to_bounds(0.0, 0.0, 20.0, 20.0, 10, 10);
        assert_eq!(camera.center_x, 15.0);
    }
}
