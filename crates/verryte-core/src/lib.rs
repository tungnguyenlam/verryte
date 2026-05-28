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
pub use world::{Query, Query2, Query3, World};

/// A request to play a specific sound by name.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AudioEvent {
    pub name: String,
    pub volume: Option<f32>,
    pub pan: Option<f32>,
    pub looped: bool,
}

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
