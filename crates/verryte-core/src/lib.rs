//! Core primitives for Verryte.
//!
//! `verryte-core` is the data-first layer of the engine: it owns the ECS world,
//! resource storage, event queues, and a minimal schedule. It deliberately knows
//! nothing about terminals or input; those concerns live in sibling crates.
//!
//! The two-line shape the engine commits to is:
//!
//! ```text
//! terminal event -> game action -> game system -> observable state
//! script command -> game action -> game system -> observable state
//! ```
//!
//! `verryte-core` is the right-hand side of that arrow: systems run against a
//! [`World`], and tests/agents read state straight off the same `World`.

pub mod clock;
pub mod diagnostics;
pub mod entity;
pub mod event;
pub mod log;
pub mod rng;
pub mod schedule;
pub mod snapshot;
pub mod tag;
pub mod world;

pub use clock::{FixedTime, GameClock};
pub use diagnostics::{Diagnostics, SystemMetrics};
pub use entity::Entity;
pub use event::{EventReader, EventReaderIter, Events};
pub use log::MessageLog;
pub use rng::Rng;
pub use schedule::{NamedSystem, Schedule, System};
#[cfg(feature = "serde")]
pub use snapshot::{EntitySnapshot, WorldRegistry, WorldSnapshot};
pub use tag::Tag;
pub use world::{Children, Parent, Query, Query2, Query3, World};

/// A request to play a specific sound by name.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AudioEvent {
    pub name: String,
    pub volume: Option<f32>,
    pub pan: Option<f32>,
    pub looped: bool,
}

/// Type alias for an event channel carrying [`AudioEvent`] messages.
pub type AudioEvents = Events<AudioEvent>;

impl AudioEvent {
    pub fn play(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            volume: None,
            pan: None,
            looped: false,
        }
    }

    pub fn loop_music(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            volume: None,
            pan: None,
            looped: true,
        }
    }

    pub fn with_volume(mut self, volume: f32) -> Self {
        self.volume = Some(volume);
        self
    }

    pub fn with_pan(mut self, pan: f32) -> Self {
        self.pan = Some(pan);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_event_play_sets_name() {
        let ev = AudioEvent::play("hit");
        assert_eq!(ev.name, "hit");
        assert!(!ev.looped);
        assert!(ev.volume.is_none());
        assert!(ev.pan.is_none());
    }

    #[test]
    fn audio_event_loop_music_sets_loop_flag() {
        let ev = AudioEvent::loop_music("theme");
        assert_eq!(ev.name, "theme");
        assert!(ev.looped);
    }

    #[test]
    fn audio_event_with_volume() {
        let ev = AudioEvent::play("sfx").with_volume(0.5);
        assert_eq!(ev.volume, Some(0.5));
    }

    #[test]
    fn audio_event_with_pan() {
        let ev = AudioEvent::play("sfx").with_pan(-1.0);
        assert_eq!(ev.pan, Some(-1.0));
    }

    #[test]
    fn audio_event_builder_chain() {
        let ev = AudioEvent::loop_music("ambient")
            .with_volume(0.8)
            .with_pan(0.3);
        assert_eq!(ev.name, "ambient");
        assert!(ev.looped);
        assert_eq!(ev.volume, Some(0.8));
        assert_eq!(ev.pan, Some(0.3));
    }

    #[test]
    fn audio_event_clone_eq() {
        let a = AudioEvent::play("clank").with_volume(1.0);
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn audio_event_from_string_name() {
        let ev = AudioEvent::play(String::from("long_name"));
        assert_eq!(ev.name, "long_name");
    }

    #[test]
    fn audio_events_type_alias_works() {
        let mut events: AudioEvents = Events::new();
        events.send(AudioEvent::play("hit"));
        assert_eq!(events.len(), 1);
        let ev = events.drain().next().unwrap();
        assert_eq!(ev.name, "hit");
    }
}
