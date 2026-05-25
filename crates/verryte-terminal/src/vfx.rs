//! Terminal-native visual effects system.
//!
//! Provides particles, screen shake, flash overlays, floating text, and AoE
//! ring indicators — all rendered directly into a [`Grid`].

use crate::{Cell, CellAttrs, Color, Grid, Rect};

// ── Particle ──────────────────────────────────────────────────────────────────

/// A single particle with position, velocity, color, and lifetime.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

/// Configuration for a particle emitter.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VfxEmitter {
    pub count: usize,
    pub glyphs: Vec<char>,
    pub colors: Vec<Color>,
    pub speed_min: f32,
    pub speed_max: f32,
    pub lifetime_min: f32,
    pub lifetime_max: f32,
    pub gravity: f32,
    pub spread: f32,
}

impl VfxEmitter {
    pub fn new() -> Self {
        Self {
            count: 10,
            glyphs: vec!['*'],
            colors: vec![Color::WHITE],
            speed_min: 1.0,
            speed_max: 3.0,
            lifetime_min: 0.5,
            lifetime_max: 1.0,
            gravity: 0.5,
            spread: 1.0,
        }
    }

    pub fn emit(&self, cx: f32, cy: f32) -> Vec<Particle> {
        let mut particles = Vec::with_capacity(self.count);
        for i in 0..self.count {
            let angle = (i as f32 / self.count as f32) * std::f32::consts::TAU;
            let speed =
                self.speed_min + (i as f32 * 0.13) % (self.speed_max - self.speed_min).max(0.1);
            let lifetime = self.lifetime_min
                + (i as f32 * 0.07) % (self.lifetime_max - self.lifetime_min).max(0.1);

            particles.push(Particle {
                x: cx + ((i as f32 * 0.5) % self.spread) - self.spread / 2.0,
                y: cy,
                vx: angle.cos() * speed,
                vy: angle.sin() * speed * 0.5,
                glyph: self.glyphs[i % self.glyphs.len()],
                fg: self.colors[i % self.colors.len()],
                bg: Color::BLACK,
                lifetime,
                max_lifetime: lifetime,
                attrs: CellAttrs::NONE.bold(),
            });
        }
        particles
    }
}

impl Default for VfxEmitter {
    fn default() -> Self {
        Self::new()
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

/// Emit bloom particles rising and floating outward in green and gold.
pub fn emit_bloom(cx: f32, cy: f32, count: usize) -> Vec<Particle> {
    let mut particles = Vec::with_capacity(count);
    let glyphs = ['❀', '✿', '❁', '✦', '·'];
    let colors = [
        Color(50, 220, 100),  // Green
        Color(255, 215, 0),   // Gold
        Color(150, 255, 150), // Light green
        Color(255, 230, 100), // Light gold
    ];
    for i in 0..count {
        let angle = (i as f32 / count as f32) * std::f32::consts::TAU;
        let speed = 0.5 + (i as f32 * 0.17) % 1.5;
        particles.push(Particle {
            x: cx,
            y: cy,
            vx: angle.cos() * speed,
            vy: -0.5 - (i as f32 * 0.23) % 1.2,
            glyph: glyphs[i % glyphs.len()],
            fg: colors[i % colors.len()],
            bg: Color::BLACK,
            lifetime: 0.8 + (i as f32 * 0.13) % 0.6,
            max_lifetime: 0.8 + (i as f32 * 0.13) % 0.6,
            attrs: CellAttrs::NONE,
        });
    }
    particles
}

/// Emit shatter particles flying outward in cold/crystalline colors.
pub fn emit_shatter(cx: f32, cy: f32, count: usize) -> Vec<Particle> {
    let mut particles = Vec::with_capacity(count);
    let glyphs = ['✦', '◇', '❄', '·', '∘'];
    let colors = [
        Color(100, 200, 255), // Cyan
        Color(240, 248, 255), // Alice Blue
        Color(255, 255, 255), // White
        Color(30, 144, 255),  // Dodger Blue
    ];
    for i in 0..count {
        let angle = (i as f32 / count as f32) * std::f32::consts::TAU;
        let speed = 2.0 + (i as f32 * 0.31) % 4.0;
        particles.push(Particle {
            x: cx,
            y: cy,
            vx: angle.cos() * speed,
            vy: angle.sin() * speed * 0.5,
            glyph: glyphs[i % glyphs.len()],
            fg: colors[i % colors.len()],
            bg: Color::BLACK,
            lifetime: 0.5 + (i as f32 * 0.09) % 0.4,
            max_lifetime: 0.5 + (i as f32 * 0.09) % 0.4,
            attrs: CellAttrs::NONE.bold(),
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
    pub fn new(intensity: f32, duration: f32) -> Self {
        Self {
            intensity,
            duration,
            elapsed: 0.0,
        }
    }

    pub fn active(&self) -> bool {
        self.elapsed < self.duration
    }

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
    pub fn full_screen(color: Color, duration: f32) -> Self {
        Self {
            color,
            duration,
            elapsed: 0.0,
            region: None,
        }
    }

    pub fn region(color: Color, duration: f32, region: Rect) -> Self {
        Self {
            color,
            duration,
            elapsed: 0.0,
            region: Some(region),
        }
    }

    pub fn active(&self) -> bool {
        self.elapsed < self.duration
    }

    pub fn alpha(&self) -> f32 {
        (1.0 - (self.elapsed / self.duration)).clamp(0.0, 1.0)
    }
}

// ── Floating Text ─────────────────────────────────────────────────────────────

/// Text that rises upward and fades over time.
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

    pub fn alive(&self) -> bool {
        self.lifetime > 0.0
    }

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
    pub fn alive(&self) -> bool {
        self.lifetime > 0.0
    }

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
    pub fn new() -> Self {
        Self {
            particles: Vec::new(),
            shakes: Vec::new(),
            flashes: Vec::new(),
            floating_texts: Vec::new(),
            aoe_rings: Vec::new(),
        }
    }

    pub fn update(&mut self, dt: f32) {
        for p in &mut self.particles {
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.vy += 0.5 * dt;
            p.lifetime -= dt;
        }
        self.particles.retain(|p| p.alive());

        for s in &mut self.shakes {
            s.elapsed += dt;
        }
        self.shakes.retain(|s| s.active());

        for f in &mut self.flashes {
            f.elapsed += dt;
        }
        self.flashes.retain(|f| f.active());

        for t in &mut self.floating_texts {
            t.y += t.vy * dt;
            t.lifetime -= dt;
        }
        self.floating_texts.retain(|t| t.alive());

        for r in &mut self.aoe_rings {
            r.current_radius += r.expand_speed * dt;
            r.lifetime -= dt;
        }
        self.aoe_rings.retain(|r| r.alive());
    }

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

    pub fn render(&self, grid: &mut Grid, w: u16, h: u16) {
        for ring in &self.aoe_rings {
            if ring.alive() {
                let alpha = ring.alpha_ratio();
                let color = ring.color.blend_alpha(Color::BLACK, 1.0 - alpha);
                let radius = ring.current_radius as u16;
                if radius > 0 {
                    grid.draw_circle(ring.cx, ring.cy, radius, Cell::new('○').with_fg(color));
                }
            }
        }

        for p in &self.particles {
            let px = p.x as i32;
            let py = p.y as i32;
            if px >= 0 && py >= 0 && (px as u16) < w && (py as u16) < h {
                let alpha = crate::math::easing::quad_out(p.alpha_ratio());
                let color = p.fg.blend_alpha(p.bg, 1.0 - alpha);
                let mut cell = Cell::new(p.glyph).with_fg(color).with_bg(p.bg);
                cell.attrs = p.attrs;
                grid.put(px as u16, py as u16, cell);
            }
        }

        for t in &self.floating_texts {
            let alpha = t.alpha_ratio();
            let color = t.fg.blend_alpha(Color::BLACK, 1.0 - alpha);
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
                    let mut cell = Cell::new(ch).with_fg(color).with_bg(Color::BLACK);
                    cell.attrs = attrs;
                    grid.put(x, ty, cell);
                }
            }
        }
    }

    pub fn render_flash(&self, grid: &mut Grid, w: u16, h: u16) {
        for f in &self.flashes {
            let alpha = f.alpha();
            let flash_color = f.color;
            let region = f.region.unwrap_or(Rect::new(0, 0, w, h));
            let x_end = region.right().min(w);
            let y_end = region.bottom().min(h);
            for y in region.y..y_end {
                for x in region.x..x_end {
                    if let Some(cell) = grid.get_mut(x, y) {
                        cell.bg = cell.bg.blend_alpha(flash_color, alpha);
                        cell.fg = cell.fg.blend_alpha(flash_color, alpha * 0.5);
                    }
                }
            }
        }
    }

    pub fn render_world(&self, grid: &mut Grid, viewport: &crate::TileViewport) {
        for ring in &self.aoe_rings {
            if ring.alive() {
                let (sx, sy) = viewport.world_to_screen(ring.cx as f32, ring.cy as f32);
                let alpha = ring.alpha_ratio();
                let color = ring.color.blend_alpha(Color::BLACK, 1.0 - alpha);
                let radius = ring.current_radius as u16;
                if radius > 0 {
                    grid.draw_circle(
                        viewport.rect.x as i32 + sx,
                        viewport.rect.y as i32 + sy,
                        radius,
                        Cell::new('○').with_fg(color),
                    );
                }
            }
        }

        for p in &self.particles {
            if p.alive() {
                let (sx, sy) = viewport.world_to_screen(p.x, p.y);
                let tx = (viewport.rect.x as i32 + sx) as u16;
                let ty = (viewport.rect.y as i32 + sy) as u16;
                if viewport.rect.contains(tx, ty) {
                    let alpha = crate::math::easing::quad_out(p.alpha_ratio());
                    let color = p.fg.blend_alpha(p.bg, 1.0 - alpha);
                    let mut cell = Cell::new(p.glyph).with_fg(color).with_bg(p.bg);
                    cell.attrs = p.attrs;
                    grid.put(tx, ty, cell);
                }
            }
        }

        for t in &self.floating_texts {
            let (sx, sy) = viewport.world_to_screen(t.x, t.y);
            let tx = (viewport.rect.x as i32 + sx) as u16;
            let ty = (viewport.rect.y as i32 + sy) as u16;
            let alpha = t.alpha_ratio();
            let color = t.fg.blend_alpha(Color::BLACK, 1.0 - alpha);
            let attrs = if t.bold {
                CellAttrs::NONE.bold()
            } else {
                CellAttrs::NONE
            };
            for (i, ch) in t.text.chars().enumerate() {
                let cx = tx + i as u16;
                if viewport.rect.contains(cx, ty) {
                    let mut cell = Cell::new(ch).with_fg(color).with_bg(Color::BLACK);
                    cell.attrs = attrs;
                    grid.put(cx, ty, cell);
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
    base.blend_alpha(overlay, alpha)
}
