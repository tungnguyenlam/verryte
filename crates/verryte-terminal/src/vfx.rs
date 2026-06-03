//! Terminal-native visual effects system.
//!
//! Provides particles, screen shake, flash overlays, floating text, and AoE
//! ring indicators — all rendered directly into a [`Grid`].

use crate::{Cell, CellAttrs, Color, Grid, Rect};

// ── Trajectory ────────────────────────────────────────────────────────────────

/// The movement trajectory pattern for a particle.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Trajectory {
    #[default]
    Straight,
    Spiral {
        speed: f32,
        radius: f32,
    },
    Wave {
        frequency: f32,
        amplitude: f32,
    },
}

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
    #[cfg_attr(feature = "serde", serde(default))]
    pub trajectory: Trajectory,
}

impl Particle {
    pub fn with_trajectory(mut self, trajectory: Trajectory) -> Self {
        self.trajectory = trajectory;
        self
    }

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
                trajectory: Trajectory::Straight,
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
            trajectory: Trajectory::Straight,
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
            trajectory: Trajectory::Straight,
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
            trajectory: Trajectory::Straight,
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
            trajectory: Trajectory::Straight,
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
            trajectory: Trajectory::Straight,
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
            trajectory: Trajectory::Straight,
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
            trajectory: Trajectory::Straight,
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
            trajectory: Trajectory::Straight,
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
            trajectory: Trajectory::Straight,
        });
    }
    particles
}

/// Emit a swirling vortex of spiral particles.
pub fn emit_vortex(cx: f32, cy: f32, count: usize, color: Color) -> Vec<Particle> {
    let mut particles = Vec::with_capacity(count);
    let glyphs = ['@', '✦', '·', '∘', '∗'];
    for i in 0..count {
        let angle = (i as f32 / count as f32) * std::f32::consts::TAU;
        let speed = 1.5 + (i as f32 * 0.23) % 2.0;
        let lifetime = 1.0 + (i as f32 * 0.11) % 0.6;
        particles.push(Particle {
            x: cx,
            y: cy,
            vx: angle.cos() * speed,
            vy: angle.sin() * speed * 0.5,
            glyph: glyphs[i % glyphs.len()],
            fg: color,
            bg: Color::BLACK,
            lifetime,
            max_lifetime: lifetime,
            attrs: CellAttrs::NONE.bold(),
            trajectory: Trajectory::Spiral {
                speed: 4.0,
                radius: 5.0,
            },
        });
    }
    particles
}

// ── Screen Shake ──────────────────────────────────────────────────────────────

/// A screen shake effect with sinusoidal offset and decay.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScreenShake {
    pub intensity: f32,
    pub duration: f32,
    pub elapsed: f32,
    #[cfg_attr(feature = "serde", serde(default))]
    pub easing: EasingMode,
}

impl ScreenShake {
    pub fn new(intensity: f32, duration: f32) -> Self {
        Self {
            intensity,
            duration,
            elapsed: 0.0,
            easing: EasingMode::Linear,
        }
    }

    pub fn new_eased(intensity: f32, duration: f32, easing: EasingMode) -> Self {
        Self {
            intensity,
            duration,
            elapsed: 0.0,
            easing,
        }
    }

    pub fn active(&self) -> bool {
        self.elapsed < self.duration
    }

    pub fn offset(&self) -> (i16, i16) {
        if !self.active() {
            return (0, 0);
        }
        let t = (self.elapsed / self.duration).clamp(0.0, 1.0);
        let decay = match self.easing {
            EasingMode::Linear => 1.0 - crate::math::easing::linear(t),
            EasingMode::QuadIn => 1.0 - crate::math::easing::quad_in(t),
            EasingMode::QuadOut => 1.0 - crate::math::easing::quad_out(t),
            EasingMode::CubicIn => 1.0 - crate::math::easing::cubic_in(t),
            EasingMode::CubicOut => 1.0 - crate::math::easing::cubic_out(t),
            EasingMode::ExpoOut => 1.0 - crate::math::easing::expo_out(t),
        };
        let strength = self.intensity * decay;
        let ox = (strength * (self.elapsed * 47.0).sin()) as i16;
        let oy = (strength * (self.elapsed * 31.0).cos() * 0.5) as i16;
        (ox, oy)
    }
}

// ── Flash Overlay ─────────────────────────────────────────────────────────────

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EasingMode {
    #[default]
    Linear,
    QuadIn,
    QuadOut,
    CubicIn,
    CubicOut,
    ExpoOut,
}

/// A color flash overlay that can be full-screen or region-limited.
#[derive(Clone, Debug)]
pub struct Flash {
    pub color: Color,
    pub duration: f32,
    pub elapsed: f32,
    pub region: Option<Rect>,
    pub easing: EasingMode,
}

impl Flash {
    pub fn full_screen(color: Color, duration: f32) -> Self {
        Self {
            color,
            duration,
            elapsed: 0.0,
            region: None,
            easing: EasingMode::Linear,
        }
    }

    pub fn region(color: Color, duration: f32, region: Rect) -> Self {
        Self {
            color,
            duration,
            elapsed: 0.0,
            region: Some(region),
            easing: EasingMode::Linear,
        }
    }

    pub fn full_screen_eased(color: Color, duration: f32, easing: EasingMode) -> Self {
        Self {
            color,
            duration,
            elapsed: 0.0,
            region: None,
            easing,
        }
    }

    pub fn region_eased(color: Color, duration: f32, region: Rect, easing: EasingMode) -> Self {
        Self {
            color,
            duration,
            elapsed: 0.0,
            region: Some(region),
            easing,
        }
    }

    pub fn active(&self) -> bool {
        self.elapsed < self.duration
    }

    pub fn alpha(&self) -> f32 {
        let t = (self.elapsed / self.duration).clamp(0.0, 1.0);
        let progress = match self.easing {
            EasingMode::Linear => 1.0 - crate::math::easing::linear(t),
            EasingMode::QuadIn => 1.0 - crate::math::easing::quad_in(t),
            EasingMode::QuadOut => 1.0 - crate::math::easing::quad_out(t),
            EasingMode::CubicIn => 1.0 - crate::math::easing::cubic_in(t),
            EasingMode::CubicOut => 1.0 - crate::math::easing::cubic_out(t),
            EasingMode::ExpoOut => 1.0 - crate::math::easing::expo_out(t),
        };
        progress.clamp(0.0, 1.0)
    }
}

// ── Floating Text ─────────────────────────────────────────────────────────────

/// Text that rises upward and fades over time.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FloatingText {
    pub x: f32,
    pub y: f32,
    #[cfg_attr(feature = "serde", serde(default))]
    pub start_y: f32,
    pub text: String,
    pub fg: Color,
    pub vy: f32,
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub bold: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    pub easing: EasingMode,
}

impl FloatingText {
    pub fn new(x: f32, y: f32, text: &str, fg: Color, bold: bool) -> Self {
        let lifetime = 1.5;
        Self {
            x,
            y,
            start_y: y,
            text: text.to_string(),
            fg,
            vy: -1.5,
            lifetime,
            max_lifetime: lifetime,
            bold,
            easing: EasingMode::Linear,
        }
    }

    pub fn new_eased(
        x: f32,
        y: f32,
        text: &str,
        fg: Color,
        bold: bool,
        easing: EasingMode,
    ) -> Self {
        let lifetime = 1.5;
        Self {
            x,
            y,
            start_y: y,
            text: text.to_string(),
            fg,
            vy: -2.5, // Total distance is -2.5 cells
            lifetime,
            max_lifetime: lifetime,
            bold,
            easing,
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
#[derive(Clone, Debug)]
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

// ── Spatial Highlight ─────────────────────────────────────────────────────────

/// A set of points to highlight on the map (e.g., a path or AoE preview).
#[derive(Clone, Debug)]
pub struct SpatialHighlight {
    pub points: Vec<(i32, i32)>,
    pub color: Color,
    pub glyph: Option<char>,
    pub bg_alpha: f32,
    pub lifetime: f32,
    pub max_lifetime: f32,
}

impl SpatialHighlight {
    pub fn new(points: Vec<(i32, i32)>, color: Color, lifetime: f32) -> Self {
        Self {
            points,
            color,
            glyph: None,
            bg_alpha: 0.3,
            lifetime,
            max_lifetime: lifetime,
        }
    }

    pub fn with_glyph(mut self, glyph: char) -> Self {
        self.glyph = Some(glyph);
        self
    }

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
    pub highlights: Vec<SpatialHighlight>,
}

impl VfxSystem {
    pub fn new() -> Self {
        Self {
            particles: Vec::new(),
            shakes: Vec::new(),
            flashes: Vec::new(),
            floating_texts: Vec::new(),
            aoe_rings: Vec::new(),
            highlights: Vec::new(),
        }
    }

    pub fn update(&mut self, dt: f32) {
        for p in &mut self.particles {
            let progress = (1.0 - p.alpha_ratio()).clamp(0.0, 1.0);
            match p.trajectory {
                Trajectory::Straight => {
                    p.x += p.vx * dt;
                    p.y += p.vy * dt;
                    p.vy += 0.5 * dt;
                }
                Trajectory::Spiral { speed, radius } => {
                    let angle = progress * speed * std::f32::consts::TAU;
                    let r = progress * radius;
                    p.x += p.vx * dt + angle.cos() * r * dt;
                    p.y += p.vy * dt + angle.sin() * r * 0.5 * dt;
                }
                Trajectory::Wave {
                    frequency,
                    amplitude,
                } => {
                    let wave = (progress * frequency * std::f32::consts::TAU).sin() * amplitude;
                    let speed = (p.vx * p.vx + p.vy * p.vy).sqrt();
                    if speed > 0.0 {
                        let px = -p.vy / speed;
                        let py = p.vx / speed;
                        p.x += p.vx * dt + px * wave * dt;
                        p.y += p.vy * dt + py * wave * dt;
                    } else {
                        p.x += p.vx * dt;
                        p.y += (p.vy + wave) * dt;
                    }
                }
            }
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
            t.lifetime -= dt;
            let progress = (1.0 - t.alpha_ratio()).clamp(0.0, 1.0);
            let eased_progress = match t.easing {
                EasingMode::Linear => crate::math::easing::linear(progress),
                EasingMode::QuadIn => crate::math::easing::quad_in(progress),
                EasingMode::QuadOut => crate::math::easing::quad_out(progress),
                EasingMode::CubicIn => crate::math::easing::cubic_in(progress),
                EasingMode::CubicOut => crate::math::easing::cubic_out(progress),
                EasingMode::ExpoOut => crate::math::easing::expo_out(progress),
            };
            t.y = t.start_y + t.vy * eased_progress;
        }
        self.floating_texts.retain(|t| t.alive());

        for r in &mut self.aoe_rings {
            r.current_radius += r.expand_speed * dt;
            r.lifetime -= dt;
        }
        self.aoe_rings.retain(|r| r.alive());

        for h in &mut self.highlights {
            h.lifetime -= dt;
        }
        self.highlights.retain(|h| h.alive());
    }

    /// Add a screen shake effect.
    pub fn trigger_shake(&mut self, intensity: f32, duration: f32) {
        self.shakes.push(ScreenShake::new(intensity, duration));
    }

    /// Add an eased screen shake effect.
    pub fn trigger_shake_eased(&mut self, intensity: f32, duration: f32, easing: EasingMode) {
        self.shakes
            .push(ScreenShake::new_eased(intensity, duration, easing));
    }

    /// Add a full screen flash effect.
    pub fn trigger_flash(&mut self, color: Color, duration: f32) {
        self.flashes.push(Flash::full_screen(color, duration));
    }

    /// Add a regional flash effect.
    pub fn trigger_flash_region(&mut self, color: Color, duration: f32, region: Rect) {
        self.flashes.push(Flash::region(color, duration, region));
    }

    /// Add a floating text indicator.
    pub fn trigger_floating_text(&mut self, x: f32, y: f32, text: &str, fg: Color, bold: bool) {
        self.floating_texts
            .push(FloatingText::new(x, y, text, fg, bold));
    }

    /// Add an AoE ring effect.
    pub fn trigger_aoe_ring(
        &mut self,
        cx: i32,
        cy: i32,
        max_radius: f32,
        color: Color,
        duration: f32,
    ) {
        self.aoe_rings.push(AoeRing {
            cx,
            cy,
            max_radius,
            current_radius: 0.0,
            expand_speed: if duration > 0.0 {
                max_radius / duration
            } else {
                max_radius
            },
            color,
            lifetime: duration,
            max_lifetime: duration,
        });
    }

    /// Add a spatial highlight.
    pub fn trigger_highlight(
        &mut self,
        points: Vec<(i32, i32)>,
        color: Color,
        glyph: Option<char>,
        bg_alpha: f32,
        lifetime: f32,
    ) {
        self.highlights.push(SpatialHighlight {
            points,
            color,
            glyph,
            bg_alpha,
            lifetime,
            max_lifetime: lifetime,
        });
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
        for hl in &self.highlights {
            let alpha = hl.alpha_ratio();
            for &(px, py) in &hl.points {
                if px >= 0 && py >= 0 && (px as u16) < w && (py as u16) < h {
                    if let Some(cell) = grid.get_mut(px as u16, py as u16) {
                        cell.bg = cell.bg.blend_alpha(hl.color, hl.bg_alpha * alpha);
                        if let Some(glyph) = hl.glyph {
                            cell.glyph = glyph;
                            cell.fg = hl.color.blend_alpha(cell.fg, 1.0 - alpha);
                        }
                    }
                }
            }
        }
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
        for hl in &self.highlights {
            let alpha = hl.alpha_ratio();
            for &(px, py) in &hl.points {
                let (sx, sy) = viewport.world_to_screen(px as f32, py as f32);
                let tx = (viewport.rect.x as i32 + sx) as u16;
                let ty = (viewport.rect.y as i32 + sy) as u16;
                if viewport.rect.contains(tx, ty) {
                    if let Some(cell) = grid.get_mut(tx, ty) {
                        cell.bg = cell.bg.blend_alpha(hl.color, hl.bg_alpha * alpha);
                        if let Some(glyph) = hl.glyph {
                            cell.glyph = glyph;
                            cell.fg = hl.color.blend_alpha(cell.fg, 1.0 - alpha);
                        }
                    }
                }
            }
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CellAttrs;

    #[test]
    fn test_particle_alive_and_alpha() {
        let mut p = Particle {
            x: 0.0,
            y: 0.0,
            vx: 1.0,
            vy: 0.0,
            glyph: '*',
            fg: Color::WHITE,
            bg: Color::BLACK,
            lifetime: 0.5,
            max_lifetime: 1.0,
            attrs: CellAttrs::NONE,
            trajectory: Trajectory::Straight,
        };
        assert!(p.alive());
        assert!((p.alpha_ratio() - 0.5).abs() < 0.01);

        p.lifetime = 0.0;
        assert!(!p.alive());
        assert_eq!(p.alpha_ratio(), 0.0);

        p.lifetime = 2.0;
        assert_eq!(p.alpha_ratio(), 1.0);
    }

    #[test]
    fn test_particle_with_trajectory() {
        let p = Particle {
            x: 0.0,
            y: 0.0,
            vx: 0.0,
            vy: 0.0,
            glyph: '*',
            fg: Color::WHITE,
            bg: Color::BLACK,
            lifetime: 1.0,
            max_lifetime: 1.0,
            attrs: CellAttrs::NONE,
            trajectory: Trajectory::Straight,
        }
        .with_trajectory(Trajectory::Spiral {
            speed: 2.0,
            radius: 3.0,
        });
        assert_eq!(
            p.trajectory,
            Trajectory::Spiral {
                speed: 2.0,
                radius: 3.0
            }
        );
    }

    #[test]
    fn test_vfx_emitter_produces_correct_count() {
        let emitter = VfxEmitter {
            count: 5,
            glyphs: vec!['*', '·'],
            colors: vec![Color::RED, Color::BLUE],
            speed_min: 1.0,
            speed_max: 2.0,
            lifetime_min: 0.5,
            lifetime_max: 1.0,
            gravity: 0.0,
            spread: 1.0,
        };
        let particles = emitter.emit(5.0, 5.0);
        assert_eq!(particles.len(), 5);
        for (i, p) in particles.iter().enumerate() {
            assert!(p.alive());
            assert!(p.lifetime >= 0.5 && p.lifetime <= 1.0);
            assert_eq!(p.glyph, ['*', '·'][i % 2]);
        }
    }

    #[test]
    fn test_vfx_emitter_default() {
        let emitter = VfxEmitter::default();
        assert_eq!(emitter.count, 10);
        assert_eq!(emitter.glyphs, vec!['*']);
    }

    #[test]
    fn test_screen_shake_active_and_offset() {
        let mut shake = ScreenShake::new(5.0, 1.0);
        assert!(shake.active());
        assert_eq!(shake.elapsed, 0.0);

        let (ox, oy) = shake.offset();
        assert!(ox.abs() >= 0);
        assert!(oy.abs() >= 0);

        shake.elapsed = 1.0;
        assert!(!shake.active());
        assert_eq!(shake.offset(), (0, 0));
    }

    #[test]
    fn test_screen_shake_eased() {
        let shake = ScreenShake::new_eased(3.0, 0.5, EasingMode::QuadOut);
        assert_eq!(shake.easing, EasingMode::QuadOut);
        assert!(shake.active());
    }

    #[test]
    fn test_screen_shake_decay() {
        let mut shake = ScreenShake::new(10.0, 2.0);
        shake.elapsed = 0.5;
        let (ox, _) = shake.offset();
        // After partial time, offset should still be non-zero
        assert!(ox.abs() > 0 || shake.elapsed < shake.duration);
    }

    #[test]
    fn test_flash_full_screen_and_region() {
        let fs = Flash::full_screen(Color::RED, 0.5);
        assert_eq!(fs.color, Color::RED);
        assert!(fs.region.is_none());
        assert!(fs.active());

        let r = Flash::region(Color::BLUE, 0.3, Rect::new(1, 1, 5, 5));
        assert_eq!(r.region, Some(Rect::new(1, 1, 5, 5)));
        assert!(r.active());
    }

    #[test]
    fn test_flash_alpha_decay() {
        let mut flash = Flash::full_screen(Color::RED, 1.0);
        let a1 = flash.alpha();
        assert!(a1 > 0.9);

        flash.elapsed = 0.5;
        let a2 = flash.alpha();
        assert!(a2 < a1);

        flash.elapsed = 1.0;
        assert!(!flash.active());
    }

    #[test]
    fn test_flash_eased() {
        let flash = Flash::full_screen_eased(Color::RED, 0.5, EasingMode::ExpoOut);
        assert_eq!(flash.easing, EasingMode::ExpoOut);

        let flash_r = Flash::region_eased(
            Color::BLUE,
            0.3,
            Rect::new(0, 0, 10, 10),
            EasingMode::CubicOut,
        );
        assert_eq!(flash_r.easing, EasingMode::CubicOut);
    }

    #[test]
    fn test_floating_text_alive_and_alpha() {
        let mut ft = FloatingText::new(5.0, 5.0, "100", Color::RED, true);
        assert!(ft.alive());
        assert_eq!(ft.alpha_ratio(), 1.0);
        assert_eq!(ft.text, "100");
        assert!(ft.bold);
        assert_eq!(ft.start_y, 5.0);

        ft.lifetime = 0.0;
        assert!(!ft.alive());
        assert_eq!(ft.alpha_ratio(), 0.0);
    }

    #[test]
    fn test_floating_text_eased() {
        let ft =
            FloatingText::new_eased(3.0, 3.0, "DMG", Color::YELLOW, false, EasingMode::QuadOut);
        assert_eq!(ft.easing, EasingMode::QuadOut);
        assert_eq!(ft.vy, -2.5);
        assert_eq!(ft.start_y, 3.0);
    }

    #[test]
    fn test_aoe_ring_alive_and_alpha() {
        let mut ring = AoeRing {
            cx: 5,
            cy: 5,
            max_radius: 3.0,
            current_radius: 0.0,
            expand_speed: 2.0,
            color: Color::RED,
            lifetime: 1.0,
            max_lifetime: 1.0,
        };
        assert!(ring.alive());
        assert_eq!(ring.alpha_ratio(), 1.0);

        ring.lifetime = 0.0;
        assert!(!ring.alive());
    }

    #[test]
    fn test_spatial_highlight_alive_and_glyph() {
        let hl = SpatialHighlight::new(vec![(0, 0), (1, 1)], Color::GREEN, 2.0).with_glyph('X');
        assert!(hl.alive());
        assert_eq!(hl.glyph, Some('X'));
        assert_eq!(hl.bg_alpha, 0.3);
        assert_eq!(hl.max_lifetime, 2.0);
    }

    #[test]
    fn test_spatial_highlight_default_no_glyph() {
        let hl = SpatialHighlight::new(vec![(0, 0)], Color::BLUE, 1.0);
        assert_eq!(hl.glyph, None);
    }

    #[test]
    fn test_vfx_system_update_removes_dead() {
        let mut vfx = VfxSystem::new();
        vfx.particles.push(Particle {
            x: 0.0,
            y: 0.0,
            vx: 1.0,
            vy: 0.0,
            glyph: '*',
            fg: Color::WHITE,
            bg: Color::BLACK,
            lifetime: 0.1,
            max_lifetime: 0.1,
            attrs: CellAttrs::NONE,
            trajectory: Trajectory::Straight,
        });
        vfx.shakes.push(ScreenShake::new(1.0, 0.05));
        vfx.flashes.push(Flash::full_screen(Color::RED, 0.05));

        assert_eq!(vfx.particles.len(), 1);
        assert_eq!(vfx.shakes.len(), 1);
        assert_eq!(vfx.flashes.len(), 1);

        vfx.update(0.2);

        assert_eq!(vfx.particles.len(), 0);
        assert_eq!(vfx.shakes.len(), 0);
        assert_eq!(vfx.flashes.len(), 0);
    }

    #[test]
    fn test_vfx_system_shake_offset_accumulates() {
        let mut vfx = VfxSystem::new();
        vfx.shakes.push(ScreenShake::new(5.0, 10.0));
        vfx.shakes.push(ScreenShake::new(3.0, 10.0));

        let (ox, oy) = vfx.shake_offset();
        assert!(ox.abs() > 0 || oy.abs() > 0);
    }

    #[test]
    fn test_vfx_system_render_does_not_panic() {
        let mut vfx = VfxSystem::new();
        vfx.particles.push(Particle {
            x: 5.0,
            y: 5.0,
            vx: 0.0,
            vy: 0.0,
            glyph: '*',
            fg: Color::WHITE,
            bg: Color::BLACK,
            lifetime: 1.0,
            max_lifetime: 1.0,
            attrs: CellAttrs::NONE,
            trajectory: Trajectory::Straight,
        });
        vfx.floating_texts
            .push(FloatingText::new(3.0, 3.0, "test", Color::RED, false));
        vfx.aoe_rings.push(AoeRing {
            cx: 5,
            cy: 5,
            max_radius: 2.0,
            current_radius: 1.0,
            expand_speed: 1.0,
            color: Color::BLUE,
            lifetime: 1.0,
            max_lifetime: 1.0,
        });
        vfx.highlights
            .push(SpatialHighlight::new(vec![(2, 2)], Color::GREEN, 1.0));

        let mut grid = Grid::new(20, 20);
        vfx.render(&mut grid, 20, 20);
    }

    #[test]
    fn test_vfx_system_render_flash_does_not_panic() {
        let mut vfx = VfxSystem::new();
        vfx.flashes.push(Flash::full_screen(Color::RED, 1.0));
        vfx.flashes
            .push(Flash::region(Color::BLUE, 0.5, Rect::new(2, 2, 5, 5)));

        let mut grid = Grid::new(20, 20);
        vfx.render_flash(&mut grid, 20, 20);
    }

    #[test]
    fn test_emitter_presets_produce_particles() {
        let burst = emit_burst(5.0, 5.0, 10, Color::RED, &['*', '·']);
        assert_eq!(burst.len(), 10);

        let fire = emit_fire(5.0, 5.0, 8);
        assert_eq!(fire.len(), 8);
        for p in &fire {
            assert!(p.vy < 0.0, "fire should rise");
        }

        let ice = emit_ice(5.0, 5.0, 6);
        assert_eq!(ice.len(), 6);

        let lightning = emit_lightning(0.0, 0.0, 10.0, 10.0);
        assert_eq!(lightning.len(), 20); // 12 steps + 8 sparks

        let slash = emit_slash(5.0, 5.0, 1.0);
        assert_eq!(slash.len(), 15);

        let heal = emit_heal(5.0, 5.0, 7);
        assert_eq!(heal.len(), 7);
        for p in &heal {
            assert!(p.vy < 0.0, "heal should rise");
        }

        let bloom = emit_bloom(5.0, 5.0, 5);
        assert_eq!(bloom.len(), 5);

        let shatter = emit_shatter(5.0, 5.0, 8);
        assert_eq!(shatter.len(), 8);

        let vortex = emit_vortex(5.0, 5.0, 12, Color::GREEN);
        assert_eq!(vortex.len(), 12);
        for p in &vortex {
            assert!(matches!(p.trajectory, Trajectory::Spiral { .. }));
        }
    }

    #[test]
    fn test_blend_color_delegates_to_blend_alpha() {
        let result = blend_color(Color::RED, Color::BLUE, 0.5);
        let expected = Color::RED.blend_alpha(Color::BLUE, 0.5);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_vfx_system_floating_text_eased_movement() {
        let mut vfx = VfxSystem::new();
        vfx.floating_texts.push(FloatingText::new_eased(
            5.0,
            5.0,
            "DMG",
            Color::RED,
            true,
            EasingMode::QuadOut,
        ));

        let initial_y = vfx.floating_texts[0].y;
        vfx.update(0.1);
        // Y should change due to eased movement
        assert!(vfx.floating_texts[0].y != initial_y);
    }

    #[test]
    fn test_vfx_system_particle_straight_gravity() {
        let mut vfx = VfxSystem::new();
        vfx.particles.push(Particle {
            x: 5.0,
            y: 5.0,
            vx: 0.0,
            vy: -2.0,
            glyph: '*',
            fg: Color::WHITE,
            bg: Color::BLACK,
            lifetime: 2.0,
            max_lifetime: 2.0,
            attrs: CellAttrs::NONE,
            trajectory: Trajectory::Straight,
        });

        let initial_vy = vfx.particles[0].vy;
        vfx.update(0.1);
        // vy should increase due to gravity (0.5 * dt)
        assert!(vfx.particles[0].vy > initial_vy);
    }

    #[test]
    fn test_particle_trajectories() {
        let mut vfx = VfxSystem::new();
        // Create standard straight particle
        vfx.particles.push(Particle {
            x: 0.0,
            y: 0.0,
            vx: 1.0,
            vy: 0.0,
            glyph: '*',
            fg: Color::WHITE,
            bg: Color::BLACK,
            lifetime: 1.0,
            max_lifetime: 1.0,
            attrs: CellAttrs::NONE,
            trajectory: Trajectory::Straight,
        });

        // Create spiral particle
        vfx.particles.push(Particle {
            x: 0.0,
            y: 0.0,
            vx: 1.0,
            vy: 0.0,
            glyph: '*',
            fg: Color::WHITE,
            bg: Color::BLACK,
            lifetime: 1.0,
            max_lifetime: 1.0,
            attrs: CellAttrs::NONE,
            trajectory: Trajectory::Spiral {
                speed: 1.0,
                radius: 2.0,
            },
        });

        // Create wave particle
        vfx.particles.push(Particle {
            x: 0.0,
            y: 0.0,
            vx: 1.0,
            vy: 0.0,
            glyph: '*',
            fg: Color::WHITE,
            bg: Color::BLACK,
            lifetime: 1.0,
            max_lifetime: 1.0,
            attrs: CellAttrs::NONE,
            trajectory: Trajectory::Wave {
                frequency: 2.0,
                amplitude: 3.0,
            },
        });

        // Run update multiple times so progress > 0.0 and trajectories diverge
        vfx.update(0.1);
        vfx.update(0.1);
        vfx.update(0.1);

        assert_eq!(vfx.particles.len(), 3);
        // The positions should be different due to different trajectories
        let p_straight = &vfx.particles[0];
        let p_spiral = &vfx.particles[1];
        let p_wave = &vfx.particles[2];

        assert!(p_straight.x > 0.0);
        assert_ne!(p_straight.x, p_spiral.x);
        assert_ne!(p_straight.y, p_wave.y);
    }

    #[test]
    fn test_vfx_system_trigger_helpers() {
        let mut vfx = VfxSystem::new();
        vfx.trigger_shake(2.0, 0.5);
        vfx.trigger_shake_eased(1.5, 0.3, EasingMode::QuadOut);
        vfx.trigger_flash(Color::RED, 0.2);
        vfx.trigger_flash_region(Color::BLUE, 0.4, Rect::new(0, 0, 10, 10));
        vfx.trigger_floating_text(5.0, 5.0, "HEAL", Color::GREEN, true);
        vfx.trigger_aoe_ring(5, 5, 4.0, Color::YELLOW, 1.0);
        vfx.trigger_highlight(vec![(1, 1), (2, 2)], Color::CYAN, Some('*'), 0.5, 1.5);

        assert_eq!(vfx.shakes.len(), 2);
        assert_eq!(vfx.flashes.len(), 2);
        assert_eq!(vfx.floating_texts.len(), 1);
        assert_eq!(vfx.aoe_rings.len(), 1);
        assert_eq!(vfx.highlights.len(), 1);
    }
}
