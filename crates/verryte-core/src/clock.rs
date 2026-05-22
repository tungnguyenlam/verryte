//! Game clock and timing resource.
//!
//! [`GameClock`] tracks elapsed ticks, real-time duration, and provides
//! timing information useful for turn-based and real-time terminal games.
//!
//! Store it as a resource in the ECS [`World`](crate::world::World) so systems
//! can read timing state without passing it through function arguments.
//!
//! # Example
//!
//! ```ignore
//! let mut clock = GameClock::new();
//! clock.tick();
//! assert_eq!(clock.elapsed_ticks(), 1);
//!
//! // Pause during cutscenes or menus.
//! clock.pause();
//! clock.tick(); // does not advance
//! assert_eq!(clock.elapsed_ticks(), 1);
//!
//! clock.resume();
//! clock.tick();
//! assert_eq!(clock.elapsed_ticks(), 2);
//! ```

use std::time::{Duration, Instant};

/// Tracks game timing state: tick count, pause state, and real-time duration.
///
/// This resource is useful for:
/// - Turn-based games that need to track the current turn number
/// - Real-time games that need frame pacing or delta-time
/// - Systems that should only run every N ticks
/// - Measuring how long a game session has been running
/// - Pausing game logic during menus, cutscenes, or dialogs
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GameClock {
    elapsed_ticks: u64,
    paused: bool,
    #[cfg_attr(feature = "serde", serde(skip, default = "Instant::now"))]
    started_at: Instant,
    #[cfg_attr(feature = "serde", serde(skip))]
    paused_at: Option<Instant>,
    #[cfg_attr(feature = "serde", serde(default))]
    total_paused_duration: Duration,
}

impl GameClock {
    /// Create a new clock with zero ticks and running state.
    pub fn new() -> Self {
        Self {
            elapsed_ticks: 0,
            paused: false,
            started_at: Instant::now(),
            paused_at: None,
            total_paused_duration: Duration::ZERO,
        }
    }

    /// Advance the clock by one tick. Does nothing if paused.
    pub fn tick(&mut self) {
        if !self.paused {
            self.elapsed_ticks += 1;
        }
    }

    /// Advance the clock by multiple ticks. Does nothing if paused.
    pub fn tick_n(&mut self, n: u64) {
        if !self.paused {
            self.elapsed_ticks += n;
        }
    }

    /// Get the number of ticks that have elapsed.
    pub fn elapsed_ticks(&self) -> u64 {
        self.elapsed_ticks
    }

    /// Pause the clock. Ticks will not advance while paused.
    pub fn pause(&mut self) {
        if !self.paused {
            self.paused = true;
            self.paused_at = Some(Instant::now());
        }
    }

    /// Resume the clock.
    pub fn resume(&mut self) {
        if self.paused {
            if let Some(paused_at) = self.paused_at.take() {
                self.total_paused_duration += paused_at.elapsed();
            }
            self.paused = false;
        }
    }

    /// Check if the clock is currently paused.
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Toggle the paused state.
    pub fn toggle_pause(&mut self) {
        if self.paused {
            self.resume();
        } else {
            self.pause();
        }
    }

    /// Get the total real-time duration since the clock was created,
    /// excluding time spent paused.
    pub fn elapsed_real_time(&self) -> Duration {
        let total = self.started_at.elapsed();
        let current_pause = if self.paused {
            self.paused_at
                .map(|p| p.elapsed())
                .unwrap_or(Duration::ZERO)
        } else {
            Duration::ZERO
        };
        total - self.total_paused_duration - current_pause
    }

    /// Get the total time spent paused.
    pub fn total_paused_duration(&self) -> Duration {
        let current = if self.paused {
            self.paused_at
                .map(|p| p.elapsed())
                .unwrap_or(Duration::ZERO)
        } else {
            Duration::ZERO
        };
        self.total_paused_duration + current
    }

    /// Reset the clock to zero ticks and restart the timer.
    pub fn reset(&mut self) {
        self.elapsed_ticks = 0;
        self.paused = false;
        self.started_at = Instant::now();
        self.paused_at = None;
        self.total_paused_duration = Duration::ZERO;
    }

    /// Set the tick count directly. Useful for loading saved games.
    pub fn set_elapsed_ticks(&mut self, ticks: u64) {
        self.elapsed_ticks = ticks;
    }
}

impl Default for GameClock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn clock_starts_at_zero() {
        let clock = GameClock::new();
        assert_eq!(clock.elapsed_ticks(), 0);
        assert!(!clock.is_paused());
    }

    #[test]
    fn tick_advances_counter() {
        let mut clock = GameClock::new();
        clock.tick();
        assert_eq!(clock.elapsed_ticks(), 1);
        clock.tick();
        assert_eq!(clock.elapsed_ticks(), 2);
    }

    #[test]
    fn tick_n_advances_by_multiple() {
        let mut clock = GameClock::new();
        clock.tick_n(5);
        assert_eq!(clock.elapsed_ticks(), 5);
        clock.tick_n(3);
        assert_eq!(clock.elapsed_ticks(), 8);
    }

    #[test]
    fn pause_prevents_tick() {
        let mut clock = GameClock::new();
        clock.tick();
        clock.pause();
        clock.tick();
        clock.tick_n(100);
        assert_eq!(clock.elapsed_ticks(), 1);
    }

    #[test]
    fn resume_allows_tick_again() {
        let mut clock = GameClock::new();
        clock.tick();
        clock.pause();
        clock.tick();
        clock.resume();
        clock.tick();
        assert_eq!(clock.elapsed_ticks(), 2);
    }

    #[test]
    fn toggle_pause_flips_state() {
        let mut clock = GameClock::new();
        assert!(!clock.is_paused());
        clock.toggle_pause();
        assert!(clock.is_paused());
        clock.toggle_pause();
        assert!(!clock.is_paused());
    }

    #[test]
    fn reset_clears_state() {
        let mut clock = GameClock::new();
        clock.tick_n(10);
        clock.pause();
        clock.resume();
        clock.reset();
        assert_eq!(clock.elapsed_ticks(), 0);
        assert!(!clock.is_paused());
    }

    #[test]
    fn set_elapsed_ticks() {
        let mut clock = GameClock::new();
        clock.set_elapsed_ticks(42);
        assert_eq!(clock.elapsed_ticks(), 42);
    }

    #[test]
    fn elapsed_real_time_is_positive() {
        let clock = GameClock::new();
        thread::sleep(Duration::from_millis(10));
        assert!(clock.elapsed_real_time() > Duration::ZERO);
    }

    #[test]
    fn paused_time_not_counted_in_real_time() {
        let mut clock = GameClock::new();
        thread::sleep(Duration::from_millis(10));
        let before_pause = clock.elapsed_real_time();

        clock.pause();
        thread::sleep(Duration::from_millis(20));
        let during_pause = clock.elapsed_real_time();

        // During pause, elapsed_real_time should not increase significantly.
        assert!(
            during_pause.as_millis() <= before_pause.as_millis() + 5,
            "paused time should not count toward elapsed_real_time"
        );

        clock.resume();
        thread::sleep(Duration::from_millis(10));
        let after_resume = clock.elapsed_real_time();

        // After resume, time should continue accumulating.
        assert!(after_resume > before_pause);
    }

    #[test]
    fn total_paused_duration_tracks_pause_time() {
        let mut clock = GameClock::new();
        clock.pause();
        thread::sleep(Duration::from_millis(20));
        let paused_dur = clock.total_paused_duration();
        assert!(
            paused_dur >= Duration::from_millis(15),
            "should have tracked at least 15ms of pause time"
        );
    }

    #[test]
    fn default_is_same_as_new() {
        let clock = GameClock::default();
        assert_eq!(clock.elapsed_ticks(), 0);
        assert!(!clock.is_paused());
    }
}
