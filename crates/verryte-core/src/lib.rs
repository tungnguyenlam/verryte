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
pub mod tag;
pub mod world;

pub use clock::GameClock;
pub use diagnostics::{Diagnostics, SystemMetrics};
pub use entity::Entity;
pub use event::{EventReader, EventReaderIter, Events};
pub use log::MessageLog;
pub use rng::Rng;
pub use schedule::{NamedSystem, Schedule, System};
pub use tag::Tag;
pub use world::{Query, Query2, Query3, World};
