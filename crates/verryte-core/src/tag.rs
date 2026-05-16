//! A lightweight string tag for grouping and filtering entities.
//!
//! `Tag` is a simple marker component that lets games label entities with
//! human-readable identifiers. Combined with [`World::retain`] or
//! [`World::despawn_with`], tags make it easy to select, filter, or bulk-remove
//! entities without encoding game-specific logic into the engine.
//!
//! # Example
//!
//! ```ignore
//! let player = world.spawn();
//! world.insert(player, Tag::new("player"));
//!
//! let enemy = world.spawn();
//! world.insert(enemy, Tag::new("enemy"));
//!
//! // Despawn all enemies at end of wave.
//! world.retain(|e| world.get::<Tag>(e).map_or(true, |t| t.0 != "enemy"));
//! ```

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Tag(pub String);

impl Tag {
    pub fn new(name: &str) -> Self {
        Self(name.to_owned())
    }

    pub fn is(&self, name: &str) -> bool {
        self.0 == name
    }
}

impl std::fmt::Display for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<S: Into<String>> From<S> for Tag {
    fn from(s: S) -> Self {
        Self(s.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_equality() {
        assert!(Tag::new("player").is("player"));
        assert!(!Tag::new("player").is("enemy"));
    }

    #[test]
    fn tag_from_str() {
        let tag: Tag = "hero".into();
        assert_eq!(tag.0, "hero");
    }

    #[test]
    fn tag_display() {
        assert_eq!(format!("{}", Tag::new("boss")), "boss");
    }
}
