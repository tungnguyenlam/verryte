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

pub mod entity;
pub mod event;
pub mod schedule;
pub mod world;

pub use entity::Entity;
pub use event::Events;
pub use schedule::{Schedule, System};
pub use world::World;
