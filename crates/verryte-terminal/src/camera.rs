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

    /// Smoothly or instantly look at a target position, returning whether the
    /// camera is now within `threshold` distance of the target.
    pub fn follow(&mut self, target_x: f32, target_y: f32, threshold: f32) -> bool {
        self.look_at(target_x, target_y);
        let dx = self.center_x - target_x;
        let dy = self.center_y - target_y;
        (dx * dx + dy * dy).sqrt() <= threshold
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
    fn test_camera_new_defaults() {
        let cam = Camera::new(0.0, 0.0);
        assert_eq!(cam.center_x, 0.0);
        assert_eq!(cam.center_y, 0.0);
        assert_eq!(cam.zoom, 1.0);
        assert!(!cam.smooth);
        assert_eq!(cam.target_x, 0.0);
        assert_eq!(cam.target_y, 0.0);
        assert_eq!(cam.lerp_factor, 0.1);
    }

    #[test]
    fn test_camera_with_smooth() {
        let cam = Camera::new(5.0, 5.0).with_smooth(0.2);
        assert!(cam.smooth);
        assert_eq!(cam.lerp_factor, 0.2);
    }

    #[test]
    fn test_camera_look_at_smooth() {
        let mut cam = Camera::new(0.0, 0.0).with_smooth(0.1);
        cam.look_at(10.0, 10.0);
        assert_eq!(cam.center_x, 0.0);
        assert_eq!(cam.target_x, 10.0);

        let mut rng = Rng::seed(1);
        cam.tick(&mut rng);
        assert!(cam.center_x > 0.0);
        assert!(cam.center_x < 10.0);
    }

    #[test]
    fn test_camera_look_at_instant() {
        let mut cam = Camera::new(0.0, 0.0);
        cam.look_at(10.0, 10.0);
        assert_eq!(cam.center_x, 10.0);
        assert_eq!(cam.center_y, 10.0);
    }

    #[test]
    fn test_camera_zoom_to_smooth() {
        let mut cam = Camera::new(0.0, 0.0).with_smooth(0.5);
        cam.zoom_to(2.0);
        assert_eq!(cam.zoom, 1.0);
        assert_eq!(cam.target_zoom, 2.0);

        let mut rng = Rng::seed(1);
        cam.tick(&mut rng);
        assert!(cam.zoom > 1.0);
        assert!(cam.zoom < 2.0);
    }

    #[test]
    fn test_camera_zoom_to_instant() {
        let mut cam = Camera::new(0.0, 0.0);
        cam.zoom_to(3.0);
        assert_eq!(cam.zoom, 3.0);
    }

    #[test]
    fn test_camera_shake_decay() {
        let mut cam = Camera::new(0.0, 0.0);
        cam.shake(10.0);
        let mut rng = Rng::seed(42);

        cam.tick(&mut rng);
        assert!(cam.shake_intensity < 10.0);
        assert!(cam.shake_intensity > 0.0);

        for _ in 0..100 {
            cam.tick(&mut rng);
        }
        assert_eq!(cam.shake_intensity, 0.0);
        assert_eq!(cam.shake_offset_x, 0.0);
    }

    #[test]
    fn test_camera_top_left_and_viewport_rect() {
        let cam = Camera::new(10.0, 10.0);
        let (x, y) = cam.top_left(20, 20);
        assert_eq!(x, 0);
        assert_eq!(y, 0);

        let rect = cam.viewport_rect(20, 20);
        assert_eq!(rect.x, 0);
        assert_eq!(rect.y, 0);
        assert_eq!(rect.width, 20);
        assert_eq!(rect.height, 20);
    }

    #[test]
    fn test_camera_viewport_rect_zoomed() {
        let mut cam = Camera::new(10.0, 10.0);
        cam.set_zoom(2.0);
        let rect = cam.viewport_rect(20, 20);
        assert_eq!(rect.width, 10);
        assert_eq!(rect.height, 10);
    }

    #[test]
    fn test_camera_clamp_to_bounds_wide_map() {
        let mut cam = Camera::new(50.0, 50.0);
        cam.clamp_to_bounds(0.0, 0.0, 100.0, 100.0, 20, 20);
        assert_eq!(cam.center_x, 50.0);
        assert_eq!(cam.center_y, 50.0);

        cam.look_at(200.0, 200.0);
        cam.clamp_to_bounds(0.0, 0.0, 100.0, 100.0, 20, 20);
        assert!(cam.center_x <= 100.0);
    }

    #[test]
    fn test_camera_clamp_small_viewport() {
        let mut cam = Camera::new(5.0, 5.0);
        // Viewport wider than map: should center
        cam.clamp_to_bounds(0.0, 0.0, 5.0, 5.0, 10, 10);
        assert_eq!(cam.center_x, 2.5);
    }

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

    #[test]
    fn test_camera_follow_instant() {
        let mut cam = Camera::new(0.0, 0.0);
        let arrived = cam.follow(10.0, 10.0, 0.5);
        assert!(arrived);
        assert_eq!(cam.center_x, 10.0);
        assert_eq!(cam.center_y, 10.0);
    }

    #[test]
    fn test_camera_follow_smooth_arrives() {
        let mut cam = Camera::new(0.0, 0.0).with_smooth(1.0);
        let arrived = cam.follow(5.0, 5.0, 1.0);
        // Smooth mode sets target but doesn't move center yet
        assert!(!arrived);
        assert_eq!(cam.target_x, 5.0);
        // After tick, center should be at target with lerp_factor=1.0
        let mut rng = Rng::seed(1);
        cam.tick(&mut rng);
        assert_eq!(cam.center_x, 5.0);
    }

    #[test]
    fn test_camera_follow_smooth_not_arrived() {
        let mut cam = Camera::new(0.0, 0.0).with_smooth(0.1);
        let arrived = cam.follow(10.0, 10.0, 0.5);
        // Smooth mode sets target but doesn't move center yet
        assert!(!arrived);
        assert_eq!(cam.target_x, 10.0);
        assert_eq!(cam.center_x, 0.0);
        // After tick, center should move toward target
        let mut rng = Rng::seed(1);
        cam.tick(&mut rng);
        assert!(cam.center_x > 0.0);
        assert!(cam.center_x < 10.0);
    }
}
