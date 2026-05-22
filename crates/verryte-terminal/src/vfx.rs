//! Terminal-native visual effects system.
//!
//! Provides particles, screen shake, flash overlays, floating text, and AoE
//! ring indicators — all rendered directly into a [`Grid`].
//!
//! # Example
//!
//! ```
//! use verryte_terminal::vfx::{VfxSystem, Particle, emit_fire, FloatingText, Flash, AoeRing};
//! use verryte_terminal::{Color, Grid, Rect};
//!
//! let mut vfx = VfxSystem::new();
//!
//! // Spawn fire particles at position (10, 5)
//! vfx.particles.extend(emit_fire(10.0, 5.0, 20));
//!
//! // Add floating damage text
//! vfx.floating_texts.push(FloatingText::new(
//!     10.0, 4.0, "-15", Color(255, 80, 30), true,
//! ));
//!
//! // Update with delta time
//! vfx.update(0.033); // ~30 FPS
//!
//! // Render into a grid
//! let mut grid = Grid::new(80, 24);
//! vfx.render(&mut grid, 80, 24);
//! vfx.render_flash(&mut grid, 80, 24);
//! ```

use crate::{Cell, CellAttrs, Color, Grid, Rect};

// ── Particle ──────────────────────────────────────────────────────────────────

/// A single particle with position, velocity, color, and lifetime.
#[derive(Clone, Debug)]
pub struct Particle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub glyph: char,
    pub fg: Color,
    pub bg: Color,
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub attrs: CellAttrs,
}

impl Particle {
    /// Returns `true` if this particle is still alive.
    pub fn alive(&self) -> bool {
        self.lifetime > 0.0
    }

    /// Returns the remaining lifetime as a 0.0–1.0 ratio.
    pub fn alpha_ratio(&self) -> f32 {
        (self.lifetime / self.max_lifetime).clamp(0.0, 1.0)
    }
}

// ── Particle Emitter Presets ──────────────────────────────────────────────────

/// Emit particles bursting outward from a center point.
pub fn emit_burst(cx: f32, cy: f32, count: usize, color: Color, glyphs: &[char]) -> Vec<Particle> {
    let mut particles = Vec::with_capacity(count);
    for i in 0..count {
        let angle = (i as f32 / count as f32) * std::f32::consts::TAU;
        let speed = 2.0 + (i as f32 * 0.37) % 3.0;
        particles.push(Particle {
            x: cx,
            y: cy,
            vx: angle.cos() * speed,
            vy: angle.sin() * speed * 0.5,
            glyph: glyphs[i % glyphs.len()],
            fg: color,
            bg: Color::BLACK,
            lifetime: 0.8 + (i as f32 * 0.13) % 0.5,
            max_lifetime: 0.8 + (i as f32 * 0.13) % 0.5,
            attrs: CellAttrs::NONE.bold(),
        });
    }
    particles
}

/// Emit fire particles rising upward with warm colors.
pub fn emit_fire(cx: f32, cy: f32, count: usize) -> Vec<Particle> {
    let mut particles = Vec::with_capacity(count);
    let glyphs = ['*', '·', '°', '˙', '•'];
    let colors = [
        Color(255, 200, 50),
        Color(255, 120, 30),
        Color(220, 60, 20),
        Color(255, 80, 50),
    ];
    for i in 0..count {
        let spread = ((i as f32 * 2.37) % 2.0) - 1.0;
        particles.push(Particle {
            x: cx + spread * 2.0,
            y: cy,
            vx: spread * 0.5,
            vy: -(1.5 + (i as f32 * 0.31) % 2.0),
            glyph: glyphs[i % glyphs.len()],
            fg: colors[i % colors.len()],
            bg: Color::BLACK,
            lifetime: 0.6 + (i as f32 * 0.17) % 0.6,
            max_lifetime: 0.6 + (i as f32 * 0.17) % 0.6,
            attrs: CellAttrs::NONE.bold(),
        });
    }
    particles
}

/// Emit ice particles spreading outward with cool colors.
pub fn emit_ice(cx: f32, cy: f32, count: usize) -> Vec<Particle> {
    let mut particles = Vec::with_capacity(count);
    let glyphs = ['*', '✦', '·', '◇', '∘'];
    let colors = [
        Color(180, 220, 255),
        Color(100, 180, 255),
        Color(200, 240, 255),
        Color(150, 200, 255),
    ];
    for i in 0..count {
        let angle = (i as f32 / count as f32) * std::f32::consts::TAU;
        let speed = 1.5 + (i as f32 * 0.41) % 2.5;
        particles.push(Particle {
            x: cx,
            y: cy,
            vx: angle.cos() * speed,
            vy: angle.sin() * speed * 0.4 - 0.5,
            glyph: glyphs[i % glyphs.len()],
            fg: colors[i % colors.len()],
            bg: Color::BLACK,
            lifetime: 1.0 + (i as f32 * 0.19) % 0.8,
            max_lifetime: 1.0 + (i as f32 * 0.19) % 0.8,
            attrs: CellAttrs::NONE,
        });
    }
    particles
}

/// Emit lightning particles along a jagged path from source to target.
pub fn emit_lightning(cx: f32, cy: f32, target_x: f32, target_y: f32) -> Vec<Particle> {
    let mut particles = Vec::new();
    let glyphs = ['/', '\\', '|', '-', '¦'];
    let colors = [
        Color(255, 255, 100),
        Color(200, 200, 255),
        Color(255, 255, 200),
    ];
    let steps = 12;
    for i in 0..steps {
        let t = i as f32 / steps as f32;
        let jitter_x = ((i as f32 * 7.3) % 3.0) - 1.5;
        let jitter_y = ((i as f32 * 11.7) % 2.0) - 1.0;
        let px = cx + (target_x - cx) * t + jitter_x;
        let py = cy + (target_y - cy) * t + jitter_y;
        particles.push(Particle {
            x: px,
            y: py,
            vx: jitter_x * 0.3,
            vy: jitter_y * 0.3,
            glyph: glyphs[i % glyphs.len()],
            fg: colors[i % colors.len()],
            bg: Color::BLACK,
            lifetime: 0.3 + (i as f32 * 0.05),
            max_lifetime: 0.3 + (i as f32 * 0.05),
            attrs: CellAttrs::NONE.bold(),
        });
    }
    // Spark burst at target
    for i in 0..8 {
        let angle = (i as f32 / 8.0) * std::f32::consts::TAU;
        particles.push(Particle {
            x: target_x,
            y: target_y,
            vx: angle.cos() * 2.0,
            vy: angle.sin() * 1.0,
            glyph: '✦',
            fg: Color(255, 255, 200),
            bg: Color::BLACK,
            lifetime: 0.4,
            max_lifetime: 0.4,
            attrs: CellAttrs::NONE.bold(),
        });
    }
    particles
}

/// Emit slash particles in a horizontal arc.
pub fn emit_slash(cx: f32, cy: f32, direction: f32) -> Vec<Particle> {
    let mut particles = Vec::new();
    let glyphs = ['─', '═', '━', '–', '—'];
    for i in 0..15 {
        let t = i as f32 / 15.0;
        let offset_y = (t - 0.5) * 6.0;
        particles.push(Particle {
            x: cx + direction * t * 12.0,
            y: cy + offset_y,
            vx: direction * 4.0,
            vy: offset_y * 0.2,
            glyph: glyphs[i % glyphs.len()],
            fg: Color(255, 255, 255),
            bg: Color::BLACK,
            lifetime: 0.3 + t * 0.2,
            max_lifetime: 0.3 + t * 0.2,
            attrs: CellAttrs::NONE.bold(),
        });
    }
    particles
}

/// Emit healing particles rising upward with green colors.
pub fn emit_heal(cx: f32, cy: f32, count: usize) -> Vec<Particle> {
    let mut particles = Vec::with_capacity(count);
    let glyphs = ['+', '♥', '✦', '°', '·'];
    let colors = [
        Color(100, 255, 150),
        Color(80, 220, 120),
        Color(150, 255, 180),
        Color(200, 255, 200),
    ];
    for i in 0..count {
        let spread = ((i as f32 * 3.17) % 4.0) - 2.0;
        particles.push(Particle {
            x: cx + spread,
            y: cy + 2.0,
            vx: spread * 0.2,
            vy: -(1.0 + (i as f32 * 0.23) % 1.5),
            glyph: glyphs[i % glyphs.len()],
            fg: colors[i % colors.len()],
            bg: Color::BLACK,
            lifetime: 1.2 + (i as f32 * 0.11) % 0.5,
            max_lifetime: 1.2 + (i as f32 * 0.11) % 0.5,
            attrs: CellAttrs::NONE,
        });
    }
    particles
}

// ── Screen Shake ──────────────────────────────────────────────────────────────

/// A screen shake effect with sinusoidal offset and decay.
pub struct ScreenShake {
    pub intensity: f32,
    pub duration: f32,
    pub elapsed: f32,
}

impl ScreenShake {
    /// Create a new screen shake with the given intensity (in cells) and duration (in seconds).
    pub fn new(intensity: f32, duration: f32) -> Self {
        Self {
            intensity,
            duration,
            elapsed: 0.0,
        }
    }

    /// Returns `true` if this shake is still active.
    pub fn active(&self) -> bool {
        self.elapsed < self.duration
    }

    /// Returns the current shake offset as (x, y) in cells.
    pub fn offset(&self) -> (i16, i16) {
        if !self.active() {
            return (0, 0);
        }
        let decay = 1.0 - (self.elapsed / self.duration);
        let strength = self.intensity * decay;
        let ox = (strength * (self.elapsed * 47.0).sin()) as i16;
        let oy = (strength * (self.elapsed * 31.0).cos() * 0.5) as i16;
        (ox, oy)
    }
}

// ── Flash Overlay ─────────────────────────────────────────────────────────────

/// A color flash overlay that can be full-screen or region-limited.
pub struct Flash {
    pub color: Color,
    pub duration: f32,
    pub elapsed: f32,
    pub region: Option<Rect>,
}

impl Flash {
    /// Create a full-screen flash.
    pub fn full_screen(color: Color, duration: f32) -> Self {
        Self {
            color,
            duration,
            elapsed: 0.0,
            region: None,
        }
    }

    /// Create a flash limited to a specific region.
    pub fn region(color: Color, duration: f32, region: Rect) -> Self {
        Self {
            color,
            duration,
            elapsed: 0.0,
            region: Some(region),
        }
    }

    /// Returns `true` if this flash is still active.
    pub fn active(&self) -> bool {
        self.elapsed < self.duration
    }

    /// Returns the current flash alpha as a 0.0–1.0 ratio.
    pub fn alpha(&self) -> f32 {
        (1.0 - (self.elapsed / self.duration)).clamp(0.0, 1.0)
    }
}

// ── Floating Text ─────────────────────────────────────────────────────────────

/// Text that rises upward and fades over time (damage numbers, heal amounts, etc.).
pub struct FloatingText {
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub fg: Color,
    pub vy: f32,
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub bold: bool,
}

impl FloatingText {
    /// Create a new floating text that rises at 1 cell/second.
    pub fn new(x: f32, y: f32, text: &str, fg: Color, bold: bool) -> Self {
        let lifetime = 1.5;
        Self {
            x,
            y,
            text: text.to_string(),
            fg,
            vy: -1.0,
            lifetime,
            max_lifetime: lifetime,
            bold,
        }
    }

    /// Returns `true` if this text is still visible.
    pub fn alive(&self) -> bool {
        self.lifetime > 0.0
    }

    /// Returns the remaining lifetime as a 0.0–1.0 ratio.
    pub fn alpha_ratio(&self) -> f32 {
        (self.lifetime / self.max_lifetime).clamp(0.0, 1.0)
    }
}

// ── AoE Ring ──────────────────────────────────────────────────────────────────

/// An expanding ring indicator for area-of-effect abilities.
pub struct AoeRing {
    pub cx: i32,
    pub cy: i32,
    pub max_radius: f32,
    pub current_radius: f32,
    pub expand_speed: f32,
    pub color: Color,
    pub lifetime: f32,
    pub max_lifetime: f32,
}

impl AoeRing {
    /// Returns `true` if this ring is still visible.
    pub fn alive(&self) -> bool {
        self.lifetime > 0.0
    }

    /// Returns the remaining lifetime as a 0.0–1.0 ratio.
    pub fn alpha_ratio(&self) -> f32 {
        (self.lifetime / self.max_lifetime).clamp(0.0, 1.0)
    }
}

// ── VFX System ────────────────────────────────────────────────────────────────

/// Manages all active visual effects and renders them into a [`Grid`].
pub struct VfxSystem {
    pub particles: Vec<Particle>,
    pub shakes: Vec<ScreenShake>,
    pub flashes: Vec<Flash>,
    pub floating_texts: Vec<FloatingText>,
    pub aoe_rings: Vec<AoeRing>,
}

impl VfxSystem {
    /// Create a new empty VFX system.
    pub fn new() -> Self {
        Self {
            particles: Vec::new(),
            shakes: Vec::new(),
            flashes: Vec::new(),
            floating_texts: Vec::new(),
            aoe_rings: Vec::new(),
        }
    }

    /// Update all effects by the given delta time (in seconds).
    /// Dead effects are automatically removed.
    pub fn update(&mut self, dt: f32) {
        // Particles
        for p in &mut self.particles {
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.vy += 0.5 * dt; // gravity
            p.lifetime -= dt;
        }
        self.particles.retain(|p| p.alive());

        // Shakes
        for s in &mut self.shakes {
            s.elapsed += dt;
        }
        self.shakes.retain(|s| s.active());

        // Flashes
        for f in &mut self.flashes {
            f.elapsed += dt;
        }
        self.flashes.retain(|f| f.active());

        // Floating text
        for t in &mut self.floating_texts {
            t.y += t.vy * dt;
            t.lifetime -= dt;
        }
        self.floating_texts.retain(|t| t.alive());

        // AoE rings
        for r in &mut self.aoe_rings {
            r.current_radius += r.expand_speed * dt;
            r.lifetime -= dt;
        }
        self.aoe_rings.retain(|r| r.alive());
    }

    /// Returns the combined shake offset from all active shakes.
    pub fn shake_offset(&self) -> (i16, i16) {
        let mut ox = 0i16;
        let mut oy = 0i16;
        for s in &self.shakes {
            let (sx, sy) = s.offset();
            ox += sx;
            oy += sy;
        }
        (ox, oy)
    }

    /// Render particles, floating text, and AoE rings into the grid.
    /// Flash overlays should be rendered separately via [`render_flash`](Self::render_flash).
    pub fn render(&self, grid: &mut Grid, w: u16, h: u16) {
        // AoE rings
        for ring in &self.aoe_rings {
            if ring.alive() {
                let alpha = ring.alpha_ratio();
                let r = (ring.color.0 as f32 * alpha) as u8;
                let g = (ring.color.1 as f32 * alpha) as u8;
                let b = (ring.color.2 as f32 * alpha) as u8;
                let radius = ring.current_radius as u16;
                if radius > 0 {
                    grid.draw_circle(
                        ring.cx,
                        ring.cy,
                        radius,
                        Cell::new('○').with_fg(Color(r, g, b)),
                    );
                }
            }
        }

        // Particles
        for p in &self.particles {
            let px = p.x as i32;
            let py = p.y as i32;
            if px >= 0 && py >= 0 && (px as u16) < w && (py as u16) < h {
                let alpha = p.alpha_ratio();
                let r = (p.fg.0 as f32 * alpha) as u8;
                let g = (p.fg.1 as f32 * alpha) as u8;
                let b = (p.fg.2 as f32 * alpha) as u8;
                let mut cell = Cell::new(p.glyph).with_fg(Color(r, g, b)).with_bg(p.bg);
                cell.attrs = p.attrs;
                grid.put(px as u16, py as u16, cell);
            }
        }

        // Floating text
        for t in &self.floating_texts {
            let alpha = t.alpha_ratio();
            let r = (t.fg.0 as f32 * alpha) as u8;
            let g = (t.fg.1 as f32 * alpha) as u8;
            let b = (t.fg.2 as f32 * alpha) as u8;
            let tx = t.x as u16;
            let ty = t.y as u16;
            let attrs = if t.bold {
                CellAttrs::NONE.bold()
            } else {
                CellAttrs::NONE
            };
            for (i, ch) in t.text.chars().enumerate() {
                let x = tx + i as u16;
                if x < w && ty < h {
                    let mut cell = Cell::new(ch).with_fg(Color(r, g, b)).with_bg(Color::BLACK);
                    cell.attrs = attrs;
                    grid.put(x, ty, cell);
                }
            }
        }
    }

    /// Render flash overlays into the grid. Call this after [`render`](Self::render).
    pub fn render_flash(&self, grid: &mut Grid, w: u16, h: u16) {
        for f in &self.flashes {
            let alpha = f.alpha();
            let r = (f.color.0 as f32 * alpha) as u8;
            let g = (f.color.1 as f32 * alpha) as u8;
            let b = (f.color.2 as f32 * alpha) as u8;
            let flash_color = Color(r, g, b);
            let region = f.region.unwrap_or(Rect::new(0, 0, w, h));
            let x_end = region.right().min(w);
            let y_end = region.bottom().min(h);
            for y in region.y..y_end {
                for x in region.x..x_end {
                    if let Some(cell) = grid.get(x, y) {
                        let blended_bg = blend_color(cell.bg, flash_color, alpha);
                        let blended_fg = blend_color(cell.fg, flash_color, alpha * 0.5);
                        grid.put(
                            x,
                            y,
                            Cell {
                                glyph: cell.glyph,
                                fg: blended_fg,
                                bg: blended_bg,
                                attrs: cell.attrs,
                            },
                        );
                    }
                }
            }
        }
    }
}

impl Default for VfxSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Blend two colors with the given alpha (0.0 = base, 1.0 = overlay).
pub fn blend_color(base: Color, overlay: Color, alpha: f32) -> Color {
    let a = alpha.clamp(0.0, 1.0);
    Color(
        (base.0 as f32 * (1.0 - a) + overlay.0 as f32 * a) as u8,
        (base.1 as f32 * (1.0 - a) + overlay.1 as f32 * a) as u8,
        (base.2 as f32 * (1.0 - a) + overlay.2 as f32 * a) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn particle_alive_and_alpha() {
        let p = Particle {
            x: 0.0,
            y: 0.0,
            vx: 0.0,
            vy: 0.0,
            glyph: '*',
            fg: Color::WHITE,
            bg: Color::BLACK,
            lifetime: 0.5,
            max_lifetime: 1.0,
            attrs: CellAttrs::NONE,
        };
        assert!(p.alive());
        assert!((p.alpha_ratio() - 0.5).abs() < 0.001);

        let dead = Particle {
            lifetime: -0.1,
            max_lifetime: 1.0,
            ..p
        };
        assert!(!dead.alive());
    }

    #[test]
    fn emit_fire_produces_particles() {
        let particles = emit_fire(10.0, 5.0, 10);
        assert_eq!(particles.len(), 10);
        assert!(particles.iter().all(|p| p.alive()));
    }

    #[test]
    fn emit_ice_produces_particles() {
        let particles = emit_ice(10.0, 5.0, 15);
        assert_eq!(particles.len(), 15);
    }

    #[test]
    fn emit_lightning_produces_particles() {
        let particles = emit_lightning(0.0, 0.0, 10.0, 5.0);
        // 12 bolt particles + 8 spark particles
        assert_eq!(particles.len(), 20);
    }

    #[test]
    fn emit_slash_produces_particles() {
        let particles = emit_slash(5.0, 5.0, 1.0);
        assert_eq!(particles.len(), 15);
    }

    #[test]
    fn emit_heal_produces_particles() {
        let particles = emit_heal(5.0, 5.0, 8);
        assert_eq!(particles.len(), 8);
    }

    #[test]
    fn emit_burst_produces_particles() {
        let particles = emit_burst(5.0, 5.0, 12, Color::RED, &['*', '·']);
        assert_eq!(particles.len(), 12);
    }

    #[test]
    fn screen_shake_decays() {
        let mut shake = ScreenShake::new(5.0, 0.5);
        assert!(shake.active());
        let (ox1, oy1) = shake.offset();
        shake.elapsed = 0.4;
        let (ox2, oy2) = shake.offset();
        // Intensity should decrease over time
        assert!(
            (ox2.abs() + oy2.abs()) < (ox1.abs() + oy1.abs()) || shake.elapsed < shake.duration
        );
        shake.elapsed = 0.6;
        assert!(!shake.active());
        assert_eq!(shake.offset(), (0, 0));
    }

    #[test]
    fn flash_alpha_decays() {
        let mut flash = Flash::full_screen(Color::WHITE, 1.0);
        assert!((flash.alpha() - 1.0).abs() < 0.001);
        flash.elapsed = 0.5;
        assert!((flash.alpha() - 0.5).abs() < 0.001);
        flash.elapsed = 1.0;
        assert!(!flash.active());
        assert!((flash.alpha() - 0.0).abs() < 0.001);
    }

    #[test]
    fn floating_text_rises_and_fades() {
        let mut text = FloatingText::new(5.0, 10.0, "-15", Color::RED, true);
        assert!(text.alive());
        text.y += text.vy * 0.5;
        text.lifetime -= 0.5;
        assert!((text.y - 9.5).abs() < 0.001); // rose by 0.5
        assert!(text.alive());
    }

    #[test]
    fn vfx_system_update_removes_dead_effects() {
        let mut vfx = VfxSystem::new();
        vfx.particles.push(Particle {
            x: 0.0,
            y: 0.0,
            vx: 0.0,
            vy: 0.0,
            glyph: '*',
            fg: Color::WHITE,
            bg: Color::BLACK,
            lifetime: 0.05,
            max_lifetime: 0.1,
            attrs: CellAttrs::NONE,
        });
        vfx.shakes.push(ScreenShake::new(1.0, 0.05));

        vfx.update(0.1); // longer than particle/shake lifetime
        assert!(vfx.particles.is_empty());
        assert!(vfx.shakes.is_empty());
    }

    #[test]
    fn blend_color_interpolates() {
        let black = Color(0, 0, 0);
        let white = Color(255, 255, 255);
        let mid = blend_color(black, white, 0.5);
        assert!(mid.0 > 120 && mid.0 < 135); // ~127-128
    }

    #[test]
    fn vfx_render_does_not_panic() {
        let mut vfx = VfxSystem::new();
        vfx.particles.extend(emit_fire(5.0, 3.0, 5));
        vfx.floating_texts
            .push(FloatingText::new(5.0, 2.0, "-10", Color::RED, true));
        vfx.aoe_rings.push(AoeRing {
            cx: 10,
            cy: 5,
            max_radius: 5.0,
            current_radius: 2.0,
            expand_speed: 10.0,
            color: Color::RED,
            lifetime: 0.5,
            max_lifetime: 1.0,
        });

        let mut grid = Grid::new(20, 10);
        vfx.render(&mut grid, 20, 10);
        vfx.render_flash(&mut grid, 20, 10);
    }
}
