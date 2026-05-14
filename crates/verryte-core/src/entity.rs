//! Generational entity handles.
//!
//! An [`Entity`] is a small `Copy` handle. The [`World`](crate::World) owns the
//! generation table and decides whether a handle is still alive, so callers can
//! freely store entities in components, resources, or test snapshots without
//! worrying about dangling references — a stale handle simply fails to resolve.

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Entity {
    pub(crate) index: u32,
    pub(crate) generation: u32,
}

impl Entity {
    pub fn index(self) -> u32 {
        self.index
    }

    pub fn generation(self) -> u32 {
        self.generation
    }
}
