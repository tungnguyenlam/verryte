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
    Homeward {
        target_x: f32,
        target_y: f32,
        speed: f32,
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

/// Emit particles that start in a burst/ring and then converge toward a target point.
#[allow(clippy::too_many_arguments)]
pub fn emit_homeward(
    cx: f32,
    cy: f32,
    tx: f32,
    ty: f32,
    count: usize,
    color: Color,
    glyphs: &[char],
    speed: f32,
) -> Vec<Particle> {
    let mut particles = Vec::with_capacity(count);
    for i in 0..count {
        let angle = (i as f32 / count as f32) * std::f32::consts::TAU;
        let radius = 2.0;
        let px = cx + angle.cos() * radius;
        let py = cy + angle.sin() * radius * 0.5;

        particles.push(Particle {
            x: px,
            y: py,
            vx: 0.0,
            vy: 0.0,
            glyph: glyphs[i % glyphs.len()],
            fg: color,
            bg: Color::BLACK,
            lifetime: 1.0,
            max_lifetime: 1.0,
            attrs: CellAttrs::NONE.bold(),
            trajectory: Trajectory::Homeward {
                target_x: tx,
                target_y: ty,
                speed,
            },
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

/// Emit a shockwave ring of particles expanding outward radially.
pub fn emit_shockwave(cx: f32, cy: f32, count: usize, color: Color) -> Vec<Particle> {
    let mut particles = Vec::with_capacity(count);
    let glyphs = ['=', '≡', '·', '∘', '°'];
    for i in 0..count {
        let angle = (i as f32 / count as f32) * std::f32::consts::TAU;
        let speed = 3.0;
        let lifetime = 0.4 + (i % 3) as f32 * 0.1;
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
            trajectory: Trajectory::Straight,
        });
    }
    particles
}

/// Emit gentle falling snowflakes across the top of the area.
pub fn emit_snow(width: u16, height: u16) -> Vec<Particle> {
    let count = ((width as usize) / 3 + 4).max(8);
    let mut particles = Vec::with_capacity(count);
    let glyphs = ['·', '*', '◦'];
    let colors = [
        Color(255, 255, 255), // White
        Color(220, 235, 255), // Light blue
        Color(200, 220, 255), // Pale blue
        Color(240, 245, 255), // Near white
    ];
    for i in 0..count {
        let x = (i as f32 * (width.max(1) as f32 / count.max(1) as f32))
            + ((i as f32 * 2.37) % 2.0)
            - 1.0;
        let speed = 0.4 + (i as f32 * 0.13) % 0.6;
        let lifetime = 2.0 + (i as f32 * 0.19) % 1.5;
        particles.push(Particle {
            x: x.max(0.0),
            y: (i as f32 * 0.73) % (height.max(1) as f32 * 0.15).max(2.0),
            vx: ((i as f32 * 1.37) % 1.0) - 0.5,
            vy: speed,
            glyph: glyphs[i % glyphs.len()],
            fg: colors[i % colors.len()],
            bg: Color::BLACK,
            lifetime,
            max_lifetime: lifetime,
            attrs: CellAttrs::NONE,
            trajectory: Trajectory::Wave {
                frequency: 0.5 + (i as f32 * 0.11) % 0.5,
                amplitude: 0.3 + (i as f32 * 0.07) % 0.4,
            },
        });
    }
    particles
}

/// Emit rain drops falling fast from the top of the area.
pub fn emit_rain(width: u16, _height: u16) -> Vec<Particle> {
    let count = ((width as usize) / 2 + 6).max(10);
    let mut particles = Vec::with_capacity(count);
    let glyphs = ['│', '┃', '|'];
    let colors = [
        Color(100, 180, 255), // Light cyan
        Color(80, 160, 235),  // Blue
        Color(120, 200, 255), // Cyan
        Color(60, 140, 220),  // Deep blue
    ];
    for i in 0..count {
        let x = (i as f32 * (width.max(1) as f32 / count.max(1) as f32))
            + ((i as f32 * 3.17) % 2.0)
            - 1.0;
        let speed = 4.0 + (i as f32 * 0.31) % 3.0;
        let lifetime = 0.3 + (i as f32 * 0.07) % 0.3;
        particles.push(Particle {
            x: x.max(0.0),
            y: (i as f32 * 0.43) % 2.0,
            vx: ((i as f32 * 0.97) % 0.4) - 0.2,
            vy: speed,
            glyph: glyphs[i % glyphs.len()],
            fg: colors[i % colors.len()],
            bg: Color::BLACK,
            lifetime,
            max_lifetime: lifetime,
            attrs: CellAttrs::NONE.bold(),
            trajectory: Trajectory::Straight,
        });
    }
    particles
}

/// Emit blowing sand particles drifting from left to right.
pub fn emit_sandstorm(width: u16, height: u16) -> Vec<Particle> {
    let count = ((height as usize) / 2 + 4).max(8);
    let mut particles = Vec::with_capacity(count);
    let glyphs = ['·', '°', '∘'];
    let colors = [
        Color(210, 180, 100), // Sand
        Color(180, 150, 80),  // Brown sand
        Color(230, 200, 120), // Light sand
        Color(160, 130, 70),  // Dark sand
    ];
    for i in 0..count {
        let y = (i as f32 * (height.max(1) as f32 / count.max(1) as f32))
            + ((i as f32 * 1.73) % 2.0)
            - 1.0;
        let speed = 2.0 + (i as f32 * 0.23) % 2.0;
        let lifetime = 1.0 + (i as f32 * 0.13) % 1.0;
        particles.push(Particle {
            x: (i as f32 * 0.67) % (width.max(1) as f32 * 0.1).max(2.0),
            y: y.max(0.0),
            vx: speed,
            vy: ((i as f32 * 2.53) % 2.0) - 1.0,
            glyph: glyphs[i % glyphs.len()],
            fg: colors[i % colors.len()],
            bg: Color::BLACK,
            lifetime,
            max_lifetime: lifetime,
            attrs: CellAttrs::NONE,
            trajectory: Trajectory::Straight,
        });
    }
    particles
}

/// Emit floating embers rising from the bottom of the area.
pub fn emit_embers(width: u16, height: u16) -> Vec<Particle> {
    let count = ((width as usize) / 3 + 4).max(8);
    let mut particles = Vec::with_capacity(count);
    let glyphs = ['·', '°', '✦'];
    let colors = [
        Color(255, 160, 40), // Orange
        Color(255, 80, 20),  // Red-orange
        Color(255, 200, 60), // Yellow
        Color(220, 60, 10),  // Deep red
    ];
    for i in 0..count {
        let x = (i as f32 * (width.max(1) as f32 / count.max(1) as f32))
            + ((i as f32 * 2.71) % 2.0)
            - 1.0;
        let speed = 0.5 + (i as f32 * 0.17) % 0.8;
        let lifetime = 1.2 + (i as f32 * 0.13) % 1.0;
        particles.push(Particle {
            x: x.max(0.0),
            y: height as f32 - 1.0 - ((i as f32 * 0.53) % 3.0),
            vx: ((i as f32 * 1.67) % 1.0) - 0.5,
            vy: -speed,
            glyph: glyphs[i % glyphs.len()],
            fg: colors[i % colors.len()],
            bg: Color::BLACK,
            lifetime,
            max_lifetime: lifetime,
            attrs: CellAttrs::NONE.bold(),
            trajectory: Trajectory::Straight,
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

// ── Ring Pulse ────────────────────────────────────────────────────────────────

/// An expanding ring effect that grows outward and fades.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RingPulse {
    pub center_x: f32,
    pub center_y: f32,
    pub radius: f32,
    pub max_radius: f32,
    pub speed: f32,
    pub color: Color,
    pub alpha: f32,
    pub glyph: char,
}

impl RingPulse {
    pub fn new(center_x: f32, center_y: f32, max_radius: f32, speed: f32, color: Color) -> Self {
        Self {
            center_x,
            center_y,
            radius: 0.0,
            max_radius,
            speed,
            color,
            alpha: 1.0,
            glyph: '○',
        }
    }

    pub fn with_glyph(mut self, glyph: char) -> Self {
        self.glyph = glyph;
        self
    }

    pub fn alive(&self) -> bool {
        self.alpha > 0.0 && self.radius < self.max_radius
    }

    pub fn alpha_ratio(&self) -> f32 {
        self.alpha.clamp(0.0, 1.0)
    }
}

// ── Trail ─────────────────────────────────────────────────────────────────────

/// A series of fading points left behind by a moving entity.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Trail {
    pub points: Vec<(f32, f32, f32)>,
    pub max_length: usize,
    pub color: Color,
    pub fade_speed: f32,
}

impl Trail {
    pub fn new(max_length: usize, color: Color, fade_speed: f32) -> Self {
        Self {
            points: Vec::new(),
            max_length,
            color,
            fade_speed,
        }
    }

    pub fn push(&mut self, x: f32, y: f32) {
        self.points.push((x, y, 1.0));
        if self.points.len() > self.max_length {
            self.points.remove(0);
        }
    }

    pub fn alive(&self) -> bool {
        !self.points.is_empty() && self.points.iter().any(|&(_, _, age)| age > 0.0)
    }
}

// ── Aura ──────────────────────────────────────────────────────────────────────

/// A pulsing glow effect around a position.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Aura {
    pub center_x: f32,
    pub center_y: f32,
    pub radius: f32,
    pub color: Color,
    pub phase: f32,
    pub pulse_speed: f32,
    pub glyphs: [char; 4],
    pub lifetime: f32,
    pub max_lifetime: f32,
}

impl Aura {
    pub fn new(center_x: f32, center_y: f32, radius: f32, color: Color) -> Self {
        Self {
            center_x,
            center_y,
            radius,
            color,
            phase: 0.0,
            pulse_speed: 3.0,
            glyphs: ['·', '∘', '°', '○'],
            lifetime: 3.0,
            max_lifetime: 3.0,
        }
    }

    pub fn with_pulse_speed(mut self, pulse_speed: f32) -> Self {
        self.pulse_speed = pulse_speed;
        self
    }

    pub fn with_glyphs(mut self, glyphs: [char; 4]) -> Self {
        self.glyphs = glyphs;
        self
    }

    pub fn with_lifetime(mut self, lifetime: f32) -> Self {
        self.lifetime = lifetime;
        self.max_lifetime = lifetime;
        self
    }

    pub fn alive(&self) -> bool {
        self.lifetime > 0.0
    }

    pub fn alpha_ratio(&self) -> f32 {
        (self.lifetime / self.max_lifetime).clamp(0.0, 1.0)
    }

    pub fn current_glyph(&self) -> char {
        let index = ((self.phase * self.pulse_speed) as usize) % self.glyphs.len();
        self.glyphs[index]
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
    pub ring_pulses: Vec<RingPulse>,
    pub trails: Vec<Trail>,
    pub auras: Vec<Aura>,
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
            ring_pulses: Vec::new(),
            trails: Vec::new(),
            auras: Vec::new(),
        }
    }

    /// Clears all active visual effects (particles, shakes, flashes, floating texts, aoe rings, highlights).
    pub fn clear(&mut self) {
        self.particles.clear();
        self.shakes.clear();
        self.flashes.clear();
        self.floating_texts.clear();
        self.aoe_rings.clear();
        self.highlights.clear();
        self.ring_pulses.clear();
        self.trails.clear();
        self.auras.clear();
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
                Trajectory::Homeward {
                    target_x,
                    target_y,
                    speed,
                } => {
                    let dx = target_x - p.x;
                    let dy = target_y - p.y;
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist > 0.1 {
                        p.x += (dx / dist) * speed * dt;
                        p.y += (dy / dist) * speed * dt;
                    } else {
                        p.x = target_x;
                        p.y = target_y;
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

        for r in &mut self.ring_pulses {
            r.radius += r.speed * dt;
            r.alpha = (1.0 - r.radius / r.max_radius).clamp(0.0, 1.0);
        }
        self.ring_pulses.retain(|r| r.alive());

        for t in &mut self.trails {
            for point in &mut t.points {
                point.2 -= t.fade_speed * dt;
            }
            t.points.retain(|p| p.2 > 0.0);
        }
        self.trails.retain(|t| t.alive());

        for a in &mut self.auras {
            a.phase += dt;
            a.lifetime -= dt;
        }
        self.auras.retain(|a| a.alive());
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

    /// Trigger a shockwave particle effect.
    pub fn trigger_shockwave(&mut self, cx: f32, cy: f32, count: usize, color: Color) {
        self.particles.extend(emit_shockwave(cx, cy, count, color));
    }

    /// Trigger a vortex particle effect.
    pub fn trigger_vortex(&mut self, cx: f32, cy: f32, count: usize, color: Color) {
        self.particles.extend(emit_vortex(cx, cy, count, color));
    }

    /// Add a ring pulse effect.
    pub fn trigger_ring_pulse(
        &mut self,
        center_x: f32,
        center_y: f32,
        max_radius: f32,
        speed: f32,
        color: Color,
    ) {
        self.ring_pulses
            .push(RingPulse::new(center_x, center_y, max_radius, speed, color));
    }

    /// Add a trail effect.
    pub fn trigger_trail(&mut self, max_length: usize, color: Color, fade_speed: f32) {
        self.trails.push(Trail::new(max_length, color, fade_speed));
    }

    /// Add an aura effect.
    pub fn trigger_aura(
        &mut self,
        center_x: f32,
        center_y: f32,
        radius: f32,
        color: Color,
        lifetime: f32,
    ) {
        self.auras
            .push(Aura::new(center_x, center_y, radius, color).with_lifetime(lifetime));
    }

    /// Trigger gentle snowfall across the given area.
    pub fn trigger_snow(&mut self, w: u16, h: u16) {
        self.particles.extend(emit_snow(w, h));
    }

    /// Trigger rain drops across the given area.
    pub fn trigger_rain(&mut self, w: u16, h: u16) {
        self.particles.extend(emit_rain(w, h));
    }

    /// Trigger a sandstorm blowing across the given area.
    pub fn trigger_sandstorm(&mut self, w: u16, h: u16) {
        self.particles.extend(emit_sandstorm(w, h));
    }

    /// Trigger floating embers rising across the given area.
    pub fn trigger_embers(&mut self, w: u16, h: u16) {
        self.particles.extend(emit_embers(w, h));
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

        for rp in &self.ring_pulses {
            if rp.alive() {
                let color = rp.color.blend_alpha(Color::BLACK, 1.0 - rp.alpha_ratio());
                let radius = rp.radius as u16;
                if radius > 0 {
                    grid.draw_circle(
                        rp.center_x as i32,
                        rp.center_y as i32,
                        radius,
                        Cell::new(rp.glyph).with_fg(color),
                    );
                }
            }
        }

        for trail in &self.trails {
            for &(px, py, age) in &trail.points {
                let tx = px as i32;
                let ty = py as i32;
                if tx >= 0 && ty >= 0 && (tx as u16) < w && (ty as u16) < h {
                    let alpha = age.clamp(0.0, 1.0);
                    let color = trail.color.blend_alpha(Color::BLACK, 1.0 - alpha);
                    let glyph = if alpha > 0.6 { '·' } else { '∘' };
                    grid.put(tx as u16, ty as u16, Cell::new(glyph).with_fg(color));
                }
            }
        }

        for a in &self.auras {
            if a.alive() {
                let life_alpha = a.alpha_ratio();
                let pulse_val = (a.phase * a.pulse_speed).sin() * 0.5 + 0.5;
                let alpha = (life_alpha * pulse_val).clamp(0.0, 1.0);
                let glyph = a.current_glyph();
                let color = a.color.blend_alpha(Color::BLACK, 1.0 - alpha);
                let radius = a.radius as i32;
                for dy in -radius..=radius {
                    for dx in -radius..=radius {
                        let dist_sq = dx * dx + dy * dy;
                        let r_sq = radius * radius;
                        let inner_sq = ((radius - 1).max(0)) * ((radius - 1).max(0));
                        if dist_sq <= r_sq && dist_sq >= inner_sq {
                            let px = (a.center_x as i32 + dx) as u16;
                            let py = (a.center_y as i32 + dy) as u16;
                            if px < w && py < h {
                                grid.put(px, py, Cell::new(glyph).with_fg(color));
                            }
                        }
                    }
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

        for rp in &self.ring_pulses {
            if rp.alive() {
                let (sx, sy) = viewport.world_to_screen(rp.center_x, rp.center_y);
                let color = rp.color.blend_alpha(Color::BLACK, 1.0 - rp.alpha_ratio());
                let radius = rp.radius as u16;
                if radius > 0 {
                    grid.draw_circle(
                        viewport.rect.x as i32 + sx,
                        viewport.rect.y as i32 + sy,
                        radius,
                        Cell::new(rp.glyph).with_fg(color),
                    );
                }
            }
        }

        for trail in &self.trails {
            for &(px, py, age) in &trail.points {
                let (sx, sy) = viewport.world_to_screen(px, py);
                let tx = (viewport.rect.x as i32 + sx) as u16;
                let ty = (viewport.rect.y as i32 + sy) as u16;
                if viewport.rect.contains(tx, ty) {
                    let alpha = age.clamp(0.0, 1.0);
                    let color = trail.color.blend_alpha(Color::BLACK, 1.0 - alpha);
                    let glyph = if alpha > 0.6 { '·' } else { '∘' };
                    grid.put(tx, ty, Cell::new(glyph).with_fg(color));
                }
            }
        }

        for a in &self.auras {
            if a.alive() {
                let (sx, sy) = viewport.world_to_screen(a.center_x, a.center_y);
                let screen_cx = viewport.rect.x as i32 + sx;
                let screen_cy = viewport.rect.y as i32 + sy;
                let life_alpha = a.alpha_ratio();
                let pulse_val = (a.phase * a.pulse_speed).sin() * 0.5 + 0.5;
                let alpha = (life_alpha * pulse_val).clamp(0.0, 1.0);
                let glyph = a.current_glyph();
                let color = a.color.blend_alpha(Color::BLACK, 1.0 - alpha);
                let radius = a.radius as i32;
                for dy in -radius..=radius {
                    for dx in -radius..=radius {
                        let dist_sq = dx * dx + dy * dy;
                        let r_sq = radius * radius;
                        let inner_sq = ((radius - 1).max(0)) * ((radius - 1).max(0));
                        if dist_sq <= r_sq && dist_sq >= inner_sq {
                            let px = (screen_cx + dx) as u16;
                            let py = (screen_cy + dy) as u16;
                            if viewport.rect.contains(px, py) {
                                grid.put(px, py, Cell::new(glyph).with_fg(color));
                            }
                        }
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
        vfx.trigger_shockwave(5.0, 5.0, 8, Color::MAGENTA);
        vfx.trigger_vortex(2.0, 2.0, 6, Color::WHITE);

        assert_eq!(vfx.shakes.len(), 2);
        assert_eq!(vfx.flashes.len(), 2);
        assert_eq!(vfx.floating_texts.len(), 1);
        assert_eq!(vfx.aoe_rings.len(), 1);
        assert_eq!(vfx.highlights.len(), 1);
        // 8 shockwave particles + 6 vortex particles = 14 particles
        assert_eq!(vfx.particles.len(), 14);
    }

    #[test]
    fn test_vfx_system_clear() {
        let mut vfx = VfxSystem::new();
        vfx.trigger_shake(2.0, 0.5);
        vfx.trigger_flash(Color::RED, 0.2);
        assert_eq!(vfx.shakes.len(), 1);
        assert_eq!(vfx.flashes.len(), 1);

        vfx.clear();
        assert_eq!(vfx.shakes.len(), 0);
        assert_eq!(vfx.flashes.len(), 0);
    }

    #[test]
    fn test_homeward_particles() {
        let particles = emit_homeward(
            10.0,
            10.0, // center
            20.0,
            20.0, // target
            4,    // count
            Color::GREEN,
            &['+'],
            5.0, // speed
        );
        assert_eq!(particles.len(), 4);
        for p in &particles {
            assert_eq!(p.fg, Color::GREEN);
            assert_eq!(p.glyph, '+');
            assert!(matches!(
                p.trajectory,
                Trajectory::Homeward {
                    target_x: 20.0,
                    target_y: 20.0,
                    speed: 5.0
                }
            ));
        }

        // Run update on particles to verify homeward progression
        let p = particles[0].clone();
        let initial_dist = ((20.0 - p.x) * (20.0 - p.x) + (20.0 - p.y) * (20.0 - p.y)).sqrt();

        // Update with Trajectory::Homeward logic manually or inside VfxSystem
        let mut system = VfxSystem::new();
        system.particles.push(p);
        system.update(0.1); // step dt = 0.1

        let updated_p = &system.particles[0];
        let new_dist = ((20.0 - updated_p.x) * (20.0 - updated_p.x)
            + (20.0 - updated_p.y) * (20.0 - updated_p.y))
            .sqrt();
        // Distance should be closer!
        assert!(new_dist < initial_dist);
    }

    #[test]
    fn test_ring_pulse_alive_and_alpha() {
        let mut rp = RingPulse::new(5.0, 5.0, 4.0, 2.0, Color::CYAN);
        assert!(rp.alive());
        assert_eq!(rp.alpha_ratio(), 1.0);
        assert_eq!(rp.glyph, '○');

        rp.radius = 4.0;
        assert!(!rp.alive());
    }

    #[test]
    fn test_ring_pulse_with_glyph() {
        let rp = RingPulse::new(0.0, 0.0, 3.0, 1.0, Color::RED).with_glyph('◎');
        assert_eq!(rp.glyph, '◎');
    }

    #[test]
    fn test_ring_pulse_update() {
        let mut vfx = VfxSystem::new();
        vfx.ring_pulses
            .push(RingPulse::new(5.0, 5.0, 4.0, 2.0, Color::CYAN));
        vfx.update(0.5);
        assert!(vfx.ring_pulses[0].radius > 0.0);
        assert!(vfx.ring_pulses[0].alpha < 1.0);
    }

    #[test]
    fn test_ring_pulse_render_does_not_panic() {
        let mut vfx = VfxSystem::new();
        vfx.ring_pulses
            .push(RingPulse::new(10.0, 10.0, 4.0, 2.0, Color::CYAN));
        let mut grid = Grid::new(20, 20);
        vfx.render(&mut grid, 20, 20);
    }

    #[test]
    fn test_trail_push_and_alive() {
        let mut trail = Trail::new(5, Color::WHITE, 1.0);
        assert!(!trail.alive());

        trail.push(1.0, 1.0);
        assert!(trail.alive());
        assert_eq!(trail.points.len(), 1);

        for i in 0..10 {
            trail.push(i as f32, i as f32);
        }
        assert_eq!(trail.points.len(), 5);
    }

    #[test]
    fn test_trail_update_fades_points() {
        let mut vfx = VfxSystem::new();
        let mut trail = Trail::new(10, Color::WHITE, 0.5);
        trail.push(5.0, 5.0);
        trail.push(6.0, 6.0);
        vfx.trails.push(trail);

        vfx.update(0.4);
        assert!(!vfx.trails[0].points.is_empty());
        for &(_, _, age) in &vfx.trails[0].points {
            assert!(age < 1.0);
            assert!(age > 0.0);
        }
    }

    #[test]
    fn test_trail_render_does_not_panic() {
        let mut vfx = VfxSystem::new();
        let mut trail = Trail::new(5, Color::GREEN, 1.0);
        trail.push(5.0, 5.0);
        trail.push(6.0, 6.0);
        vfx.trails.push(trail);
        let mut grid = Grid::new(20, 20);
        vfx.render(&mut grid, 20, 20);
    }

    #[test]
    fn test_aura_alive_and_alpha() {
        let mut aura = Aura::new(5.0, 5.0, 3.0, Color::BLUE);
        assert!(aura.alive());
        assert_eq!(aura.alpha_ratio(), 1.0);

        aura.lifetime = 0.0;
        assert!(!aura.alive());
        assert_eq!(aura.alpha_ratio(), 0.0);
    }

    #[test]
    fn test_aura_with_glyphs_and_pulse_speed() {
        let aura = Aura::new(0.0, 0.0, 2.0, Color::YELLOW)
            .with_glyphs(['*', '+', 'x', '#'])
            .with_pulse_speed(5.0)
            .with_lifetime(10.0);
        assert_eq!(aura.glyphs, ['*', '+', 'x', '#']);
        assert_eq!(aura.pulse_speed, 5.0);
        assert_eq!(aura.max_lifetime, 10.0);
    }

    #[test]
    fn test_aura_current_glyph_cycles() {
        let mut aura = Aura::new(0.0, 0.0, 2.0, Color::WHITE);
        aura.phase = 0.0;
        let g0 = aura.current_glyph();
        aura.phase = std::f32::consts::PI / aura.pulse_speed;
        let g1 = aura.current_glyph();
        // With default glyphs ['·', '∘', '°', '○'] and different phases,
        // the glyph index should differ.
        assert_eq!(g0, '·');
        // phase * pulse_speed = PI ≈ 3.14, as usize = 3, 3 % 4 = 3 → '○'
        assert_eq!(g1, '○');
    }

    #[test]
    fn test_aura_update() {
        let mut vfx = VfxSystem::new();
        vfx.auras
            .push(Aura::new(5.0, 5.0, 2.0, Color::GREEN).with_lifetime(2.0));
        vfx.update(0.5);
        assert!(vfx.auras[0].phase > 0.0);
        assert!(vfx.auras[0].lifetime < 2.0);
    }

    #[test]
    fn test_aura_render_does_not_panic() {
        let mut vfx = VfxSystem::new();
        vfx.auras.push(Aura::new(10.0, 10.0, 3.0, Color::MAGENTA));
        let mut grid = Grid::new(20, 20);
        vfx.render(&mut grid, 20, 20);
    }

    #[test]
    fn test_trigger_ring_pulse() {
        let mut vfx = VfxSystem::new();
        vfx.trigger_ring_pulse(5.0, 5.0, 4.0, 2.0, Color::CYAN);
        assert_eq!(vfx.ring_pulses.len(), 1);
    }

    #[test]
    fn test_trigger_trail() {
        let mut vfx = VfxSystem::new();
        vfx.trigger_trail(8, Color::WHITE, 1.0);
        assert_eq!(vfx.trails.len(), 1);
        assert_eq!(vfx.trails[0].max_length, 8);
    }

    #[test]
    fn test_trigger_aura() {
        let mut vfx = VfxSystem::new();
        vfx.trigger_aura(5.0, 5.0, 3.0, Color::BLUE, 5.0);
        assert_eq!(vfx.auras.len(), 1);
        assert_eq!(vfx.auras[0].max_lifetime, 5.0);
    }

    #[test]
    fn test_vfx_system_clear_includes_new_types() {
        let mut vfx = VfxSystem::new();
        vfx.trigger_ring_pulse(5.0, 5.0, 4.0, 2.0, Color::CYAN);
        vfx.trigger_trail(5, Color::WHITE, 1.0);
        vfx.trigger_aura(5.0, 5.0, 2.0, Color::GREEN, 3.0);
        assert_eq!(vfx.ring_pulses.len(), 1);
        assert_eq!(vfx.trails.len(), 1);
        assert_eq!(vfx.auras.len(), 1);

        vfx.clear();
        assert_eq!(vfx.ring_pulses.len(), 0);
        assert_eq!(vfx.trails.len(), 0);
        assert_eq!(vfx.auras.len(), 0);
    }

    #[test]
    fn test_new_effects_render_world_does_not_panic() {
        let mut vfx = VfxSystem::new();
        vfx.ring_pulses
            .push(RingPulse::new(5.0, 5.0, 4.0, 2.0, Color::CYAN));
        let mut trail = Trail::new(5, Color::WHITE, 1.0);
        trail.push(5.0, 5.0);
        vfx.trails.push(trail);
        vfx.auras.push(Aura::new(5.0, 5.0, 2.0, Color::GREEN));
        vfx.highlights
            .push(SpatialHighlight::new(vec![(5, 5)], Color::BLUE, 1.0));

        let mut grid = Grid::new(20, 20);
        let viewport = crate::TileViewport::new(Rect::new(0, 0, 20, 20), 1, 1);
        vfx.render_world(&mut grid, &viewport);
    }

    #[test]
    fn test_emit_snow_produces_particles() {
        let snow = emit_snow(80, 24);
        assert!(snow.len() >= 8, "snow should produce at least 8 particles");
        for p in &snow {
            assert!(p.alive());
            assert!(p.vy > 0.0, "snow should fall downward");
            assert!(
                matches!(p.trajectory, Trajectory::Wave { .. }),
                "snow should use Wave trajectory for sway"
            );
        }
    }

    #[test]
    fn test_emit_snow_uses_correct_glyphs() {
        let snow = emit_snow(40, 20);
        let valid = ['·', '*', '◦'];
        for p in &snow {
            assert!(valid.contains(&p.glyph), "unexpected glyph: {}", p.glyph);
        }
    }

    #[test]
    fn test_emit_snow_small_dimensions() {
        let snow = emit_snow(1, 1);
        assert!(snow.len() >= 8);
        for p in &snow {
            assert!(p.alive());
        }
    }

    #[test]
    fn test_emit_rain_produces_particles() {
        let rain = emit_rain(80, 24);
        assert!(
            rain.len() >= 10,
            "rain should produce at least 10 particles"
        );
        for p in &rain {
            assert!(p.alive());
            assert!(p.vy > 0.0, "rain should fall downward");
            assert!(
                matches!(p.trajectory, Trajectory::Straight),
                "rain should use Straight trajectory"
            );
        }
    }

    #[test]
    fn test_emit_rain_falls_fast() {
        let rain = emit_rain(80, 24);
        for p in &rain {
            assert!(p.vy >= 4.0, "rain should fall fast, got {}", p.vy);
        }
    }

    #[test]
    fn test_emit_rain_uses_correct_glyphs() {
        let rain = emit_rain(40, 20);
        let valid = ['│', '┃', '|'];
        for p in &rain {
            assert!(valid.contains(&p.glyph), "unexpected glyph: {}", p.glyph);
        }
    }

    #[test]
    fn test_emit_sandstorm_produces_particles() {
        let sand = emit_sandstorm(80, 24);
        assert!(
            sand.len() >= 8,
            "sandstorm should produce at least 8 particles"
        );
        for p in &sand {
            assert!(p.alive());
            assert!(p.vx > 0.0, "sand should blow rightward");
        }
    }

    #[test]
    fn test_emit_sandstorm_uses_correct_glyphs() {
        let sand = emit_sandstorm(80, 24);
        let valid = ['·', '°', '∘'];
        for p in &sand {
            assert!(valid.contains(&p.glyph), "unexpected glyph: {}", p.glyph);
        }
    }

    #[test]
    fn test_emit_sandstorm_vertical_scatter() {
        let sand = emit_sandstorm(80, 24);
        let has_positive = sand.iter().any(|p| p.vy > 0.0);
        let has_negative = sand.iter().any(|p| p.vy < 0.0);
        assert!(
            has_positive && has_negative,
            "sandstorm should scatter vertically in both directions"
        );
    }

    #[test]
    fn test_emit_embers_produces_particles() {
        let embers = emit_embers(80, 24);
        assert!(
            embers.len() >= 8,
            "embers should produce at least 8 particles"
        );
        for p in &embers {
            assert!(p.alive());
            assert!(p.vy < 0.0, "embers should rise upward");
        }
    }

    #[test]
    fn test_emit_embers_uses_correct_glyphs() {
        let embers = emit_embers(40, 20);
        let valid = ['·', '°', '✦'];
        for p in &embers {
            assert!(valid.contains(&p.glyph), "unexpected glyph: {}", p.glyph);
        }
    }

    #[test]
    fn test_emit_embers_spawns_near_bottom() {
        let embers = emit_embers(80, 24);
        for p in &embers {
            assert!(
                p.y >= 20.0,
                "embers should spawn near bottom, got y={}",
                p.y
            );
        }
    }

    #[test]
    fn test_weather_trigger_helpers() {
        let mut vfx = VfxSystem::new();
        vfx.trigger_snow(80, 24);
        let snow_count = vfx.particles.len();
        assert!(snow_count > 0);

        vfx.trigger_rain(80, 24);
        let after_rain = vfx.particles.len();
        assert!(after_rain > snow_count);

        vfx.trigger_sandstorm(80, 24);
        let after_sand = vfx.particles.len();
        assert!(after_sand > after_rain);

        vfx.trigger_embers(80, 24);
        let after_embers = vfx.particles.len();
        assert!(after_embers > after_sand);
    }

    #[test]
    fn test_weather_particles_update_and_expire() {
        let mut vfx = VfxSystem::new();
        vfx.trigger_snow(40, 20);
        vfx.trigger_rain(40, 20);
        let initial = vfx.particles.len();
        assert!(initial > 0);

        for _ in 0..20 {
            vfx.update(0.5);
        }
        assert!(
            vfx.particles.is_empty(),
            "all weather particles should expire after enough time"
        );
    }

    #[test]
    fn test_weather_particles_render_does_not_panic() {
        let mut vfx = VfxSystem::new();
        vfx.trigger_snow(20, 10);
        vfx.trigger_rain(20, 10);
        vfx.trigger_sandstorm(20, 10);
        vfx.trigger_embers(20, 10);
        let mut grid = Grid::new(20, 10);
        vfx.render(&mut grid, 20, 10);
    }
}
