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
    /// A sentinel entity handle that will never resolve to a live entity.
    ///
    /// Use this as a placeholder in components or resources where an entity
    /// reference is required but may not point to anything (for example,
    /// "target" or "parent" fields that are initially unset).
    pub const INVALID: Self = Self {
        index: u32::MAX,
        generation: u32::MAX,
    };

    pub fn index(self) -> u32 {
        self.index
    }

    pub fn generation(self) -> u32 {
        self.generation
    }

    /// Returns `true` if this is the [`INVALID`](Self::INVALID) sentinel.
    pub fn is_invalid(self) -> bool {
        self.index == u32::MAX && self.generation == u32::MAX
    }
}

impl std::fmt::Display for Entity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}#{}", self.index, self.generation)
    }
}
