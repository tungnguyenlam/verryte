//! Mathematical utilities for terminal games.

/// Standard easing functions for animations and VFX.
///
/// Each function takes a normalized time `t` (0.0 to 1.0) and returns
/// the eased value.
pub mod easing {
    /// Linear interpolation (no easing).
    pub fn linear(t: f32) -> f32 {
        t
    }

    /// Quadratic in: `t^2`.
    pub fn quad_in(t: f32) -> f32 {
        t * t
    }

    /// Quadratic out: `1 - (1 - t)^2`.
    pub fn quad_out(t: f32) -> f32 {
        1.0 - (1.0 - t) * (1.0 - t)
    }

    /// Quadratic in-out.
    pub fn quad_in_out(t: f32) -> f32 {
        if t < 0.5 {
            2.0 * t * t
        } else {
            1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
        }
    }

    /// Cubic in: `t^3`.
    pub fn cubic_in(t: f32) -> f32 {
        t * t * t
    }

    /// Cubic out: `1 - (1 - t)^3`.
    pub fn cubic_out(t: f32) -> f32 {
        1.0 - (1.0 - t).powi(3)
    }

    /// Cubic in-out.
    pub fn cubic_in_out(t: f32) -> f32 {
        if t < 0.5 {
            4.0 * t * t * t
        } else {
            1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
        }
    }

    /// Quartic in: `t^4`.
    pub fn quart_in(t: f32) -> f32 {
        t * t * t * t
    }

    /// Quartic out: `1 - (1 - t)^4`.
    pub fn quart_out(t: f32) -> f32 {
        1.0 - (1.0 - t).powi(4)
    }

    /// Quintic in: `t^5`.
    pub fn quint_in(t: f32) -> f32 {
        t * t * t * t * t
    }

    /// Quintic out: `1 - (1 - t)^5`.
    pub fn quint_out(t: f32) -> f32 {
        1.0 - (1.0 - t).powi(5)
    }

    /// Exponential in: `2^(10 * (t - 1))`.
    pub fn expo_in(t: f32) -> f32 {
        if t == 0.0 {
            0.0
        } else {
            2.0f32.powf(10.0 * t - 10.0)
        }
    }

    /// Exponential out: `1 - 2^(-10 * t)`.
    pub fn expo_out(t: f32) -> f32 {
        if t == 1.0 {
            1.0
        } else {
            1.0 - 2.0f32.powf(-10.0 * t)
        }
    }

    /// Elastic out: bouncy overshoot.
    pub fn elastic_out(t: f32) -> f32 {
        let c4 = (2.0 * std::f32::consts::PI) / 3.0;
        if t == 0.0 {
            0.0
        } else if (t - 1.0).abs() < f32::EPSILON {
            1.0
        } else {
            2.0f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c4).sin() + 1.0
        }
    }

    /// Bounce out: several small bounces at the end.
    pub fn bounce_out(mut t: f32) -> f32 {
        let n1 = 7.5625;
        let d1 = 2.75;

        if t < 1.0 / d1 {
            n1 * t * t
        } else if t < 2.0 / d1 {
            t -= 1.5 / d1;
            n1 * t * t + 0.75
        } else if t < 2.5 / d1 {
            t -= 2.25 / d1;
            n1 * t * t + 0.9375
        } else {
            t -= 2.625 / d1;
            n1 * t * t + 0.984375
        }
    }

    /// Back in: overshoots the start slightly.
    pub fn back_in(t: f32) -> f32 {
        let c1 = 1.70158;
        let c3 = c1 + 1.0;
        c3 * t * t * t - c1 * t * t
    }

    /// Back out: overshoots the end slightly.
    pub fn back_out(t: f32) -> f32 {
        let c1 = 1.70158;
        let c3 = c1 + 1.0;
        1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
    }

    /// Back in-out.
    pub fn back_in_out(t: f32) -> f32 {
        let c1 = 1.70158;
        let c2 = c1 * 1.525;

        if t < 0.5 {
            ((2.0 * t).powi(2) * ((c2 + 1.0) * 2.0 * t - c2)) / 2.0
        } else {
            ((2.0 * t - 2.0).powi(2) * ((c2 + 1.0) * (t * 2.0 - 2.0) + c2) + 2.0) / 2.0
        }
    }

    /// Circular in.
    pub fn circ_in(t: f32) -> f32 {
        1.0 - (1.0 - t.powi(2)).sqrt()
    }

    /// Circular out.
    pub fn circ_out(t: f32) -> f32 {
        (1.0 - (t - 1.0).powi(2)).sqrt()
    }

    /// Circular in-out.
    pub fn circ_in_out(t: f32) -> f32 {
        if t < 0.5 {
            (1.0 - (1.0 - (2.0 * t).powi(2)).sqrt()) / 2.0
        } else {
            ((1.0 - (-2.0 * t + 2.0).powi(2)).sqrt() + 1.0) / 2.0
        }
    }
}
