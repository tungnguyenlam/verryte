#![cfg(feature = "serde")]

use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use std::marker::PhantomData;

use crate::entity::Entity;
use crate::world::World;

/// A complete snapshot of the game world.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct WorldSnapshot {
    pub entities: Vec<EntitySnapshot>,
    pub resources: HashMap<String, serde_json::Value>,
}

/// A snapshot of a single entity and its components.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EntitySnapshot {
    pub entity: Entity,
    pub components: HashMap<String, serde_json::Value>,
}

/// Represents the difference between two [`WorldSnapshot`]s.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct WorldDiff {
    pub added_entities: Vec<EntitySnapshot>,
    pub removed_entities: Vec<Entity>,
    pub changed_entities: HashMap<Entity, EntityDiff>,
    pub added_resources: HashMap<String, serde_json::Value>,
    pub removed_resources: Vec<String>,
    pub changed_resources: HashMap<String, serde_json::Value>,
}

/// Represents the difference in components for a single entity.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct EntityDiff {
    pub added_components: HashMap<String, serde_json::Value>,
    pub removed_components: Vec<String>,
    pub changed_components: HashMap<String, serde_json::Value>,
}

impl WorldSnapshot {
    /// Compute the difference to transform `self` into `other`.
    pub fn diff(&self, other: &WorldSnapshot) -> WorldDiff {
        let mut diff = WorldDiff::default();

        let self_entities: HashMap<Entity, &EntitySnapshot> =
            self.entities.iter().map(|e| (e.entity, e)).collect();
        let other_entities: HashMap<Entity, &EntitySnapshot> =
            other.entities.iter().map(|e| (e.entity, e)).collect();

        // Entities in other but not in self are "added"
        for &entity in other_entities.keys() {
            if !self_entities.contains_key(&entity) {
                diff.added_entities.push(other_entities[&entity].clone());
            }
        }

        // Entities in self but not in other are "removed"
        for &entity in self_entities.keys() {
            if !other_entities.contains_key(&entity) {
                diff.removed_entities.push(entity);
            } else {
                // Entity exists in both, check for component changes
                let s_ent = self_entities[&entity];
                let o_ent = other_entities[&entity];
                let mut e_diff = EntityDiff::default();

                for (name, val) in &o_ent.components {
                    if !s_ent.components.contains_key(name) {
                        e_diff.added_components.insert(name.clone(), val.clone());
                    } else if s_ent.components[name] != *val {
                        e_diff.changed_components.insert(name.clone(), val.clone());
                    }
                }

                for name in s_ent.components.keys() {
                    if !o_ent.components.contains_key(name) {
                        e_diff.removed_components.push(name.clone());
                    }
                }

                if !e_diff.added_components.is_empty()
                    || !e_diff.removed_components.is_empty()
                    || !e_diff.changed_components.is_empty()
                {
                    diff.changed_entities.insert(entity, e_diff);
                }
            }
        }

        // Resources
        for (name, val) in &other.resources {
            if !self.resources.contains_key(name) {
                diff.added_resources.insert(name.clone(), val.clone());
            } else if self.resources[name] != *val {
                diff.changed_resources.insert(name.clone(), val.clone());
            }
        }

        for name in self.resources.keys() {
            if !other.resources.contains_key(name) {
                diff.removed_resources.push(name.clone());
            }
        }

        diff
    }
}

pub trait ComponentRegistration: Send + Sync {
    fn serialize(&self, world: &World, entity: Entity) -> Option<serde_json::Value>;
    fn deserialize(
        &self,
        world: &mut World,
        entity: Entity,
        value: serde_json::Value,
    ) -> Result<(), String>;
    fn remove(&self, world: &mut World, entity: Entity);
}

pub trait ResourceRegistration: Send + Sync {
    fn serialize(&self, world: &World) -> Option<serde_json::Value>;
    fn deserialize(&self, world: &mut World, value: serde_json::Value) -> Result<(), String>;
    fn remove(&self, world: &mut World);
}

struct TypedComponentRegistration<T> {
    _marker: PhantomData<T>,
}

impl<T: 'static + Send + Sync + Serialize + DeserializeOwned> ComponentRegistration
    for TypedComponentRegistration<T>
{
    fn serialize(&self, world: &World, entity: Entity) -> Option<serde_json::Value> {
        world
            .get::<T>(entity)
            .and_then(|c| serde_json::to_value(c).ok())
    }

    fn deserialize(
        &self,
        world: &mut World,
        entity: Entity,
        value: serde_json::Value,
    ) -> Result<(), String> {
        let component: T = serde_json::from_value(value).map_err(|e| e.to_string())?;
        world.insert(entity, component);
        Ok(())
    }

    fn remove(&self, world: &mut World, entity: Entity) {
        world.remove::<T>(entity);
    }
}

struct TypedResourceRegistration<T> {
    _marker: PhantomData<T>,
}

impl<T: 'static + Send + Sync + Serialize + DeserializeOwned> ResourceRegistration
    for TypedResourceRegistration<T>
{
    fn serialize(&self, world: &World) -> Option<serde_json::Value> {
        world
            .resource::<T>()
            .and_then(|r| serde_json::to_value(r).ok())
    }

    fn deserialize(&self, world: &mut World, value: serde_json::Value) -> Result<(), String> {
        let resource: T = serde_json::from_value(value).map_err(|e| e.to_string())?;
        world.insert_resource(resource);
        Ok(())
    }

    fn remove(&self, world: &mut World) {
        world.remove_resource::<T>();
    }
}

/// A registry that maps component and resource names to their types and serialization logic.
#[derive(Default)]
pub struct WorldRegistry {
    components: HashMap<String, Box<dyn ComponentRegistration>>,
    resources: HashMap<String, Box<dyn ResourceRegistration>>,
}

impl WorldRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a component type for snapshotting.
    pub fn register_component<T>(&mut self, name: &str)
    where
        T: 'static + Send + Sync + Serialize + DeserializeOwned,
    {
        self.components.insert(
            name.to_string(),
            Box::new(TypedComponentRegistration::<T> {
                _marker: PhantomData,
            }),
        );
    }

    /// Register a resource type for snapshotting.
    pub fn register_resource<T>(&mut self, name: &str)
    where
        T: 'static + Send + Sync + Serialize + DeserializeOwned,
    {
        self.resources.insert(
            name.to_string(),
            Box::new(TypedResourceRegistration::<T> {
                _marker: PhantomData,
            }),
        );
    }

    /// Register built-in core resources for snapshotting.
    pub fn register_core_resources(&mut self) {
        self.register_resource::<crate::log::MessageLog>("MessageLog");
        self.register_resource::<crate::clock::GameClock>("GameClock");
        self.register_resource::<crate::rng::Rng>("Rng");
    }

    /// Create a snapshot of the given world.
    pub fn snapshot(&self, world: &World) -> WorldSnapshot {
        let mut snapshot = WorldSnapshot::default();

        // Snapshot entities
        for entity in world.entities() {
            let mut entity_snap = EntitySnapshot {
                entity,
                components: HashMap::new(),
            };

            for (name, reg) in &self.components {
                if let Some(value) = reg.serialize(world, entity) {
                    entity_snap.components.insert(name.clone(), value);
                }
            }

            if !entity_snap.components.is_empty() {
                snapshot.entities.push(entity_snap);
            }
        }

        // Snapshot resources
        for (name, reg) in &self.resources {
            if let Some(value) = reg.serialize(world) {
                snapshot.resources.insert(name.clone(), value);
            }
        }

        snapshot
    }

    /// Apply a snapshot to the world, clearing all existing state first.
    pub fn apply(&self, world: &mut World, snapshot: WorldSnapshot) -> Result<(), String> {
        world.clear();

        // Restore entities
        for entity_snap in snapshot.entities {
            world.spawn_at(entity_snap.entity);
            for (name, value) in entity_snap.components {
                if let Some(reg) = self.components.get(&name) {
                    reg.deserialize(world, entity_snap.entity, value)?;
                }
            }
        }

        // Restore resources
        for (name, value) in snapshot.resources {
            if let Some(reg) = self.resources.get(&name) {
                reg.deserialize(world, value)?;
            }
        }

        Ok(())
    }

    /// Apply a difference to the world, patching it from its current state.
    pub fn apply_diff(&self, world: &mut World, diff: WorldDiff) -> Result<(), String> {
        // Remove entities
        for entity in diff.removed_entities {
            world.despawn(entity);
        }

        // Add entities
        for entity_snap in diff.added_entities {
            world.spawn_at(entity_snap.entity);
            for (name, value) in entity_snap.components {
                if let Some(reg) = self.components.get(&name) {
                    reg.deserialize(world, entity_snap.entity, value)?;
                }
            }
        }

        // Change entities
        for (entity, e_diff) in diff.changed_entities {
            // Remove components
            for name in e_diff.removed_components {
                if let Some(reg) = self.components.get(&name) {
                    reg.remove(world, entity);
                }
            }
            // Add/Change components
            for (name, value) in e_diff
                .added_components
                .into_iter()
                .chain(e_diff.changed_components)
            {
                if let Some(reg) = self.components.get(&name) {
                    reg.deserialize(world, entity, value)?;
                }
            }
        }

        // Remove resources
        for name in diff.removed_resources {
            if let Some(reg) = self.resources.get(&name) {
                reg.remove(world);
            }
        }

        // Add/Change resources
        for (name, value) in diff
            .added_resources
            .into_iter()
            .chain(diff.changed_resources)
        {
            if let Some(reg) = self.resources.get(&name) {
                reg.deserialize(world, value)?;
            }
        }

        Ok(())
    }
}

/// A resource that manages a stack of `WorldSnapshot`s for undo/redo capability.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct WorldCheckpointStack {
    undo_stack: Vec<WorldSnapshot>,
    redo_stack: Vec<WorldSnapshot>,
    max_checkpoints: usize,
}

impl WorldCheckpointStack {
    /// Create a new checkpoint stack with a limit on history size.
    pub fn new(max_checkpoints: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_checkpoints,
        }
    }

    /// Push a new checkpoint, clearing the redo history.
    pub fn push_checkpoint(&mut self, snapshot: WorldSnapshot) {
        self.undo_stack.push(snapshot);
        if self.undo_stack.len() > self.max_checkpoints {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    /// Undo a state transition. Takes the current state snapshot to push onto the redo stack.
    /// Returns the previous state snapshot if undo was successful.
    pub fn undo(&mut self, current: WorldSnapshot) -> Option<WorldSnapshot> {
        let prev = self.undo_stack.pop()?;
        self.redo_stack.push(current);
        if self.redo_stack.len() > self.max_checkpoints {
            self.redo_stack.remove(0);
        }
        Some(prev)
    }

    /// Redo a previously undone state transition. Takes the current state snapshot to push onto the undo stack.
    /// Returns the next state snapshot if redo was successful.
    pub fn redo(&mut self, current: WorldSnapshot) -> Option<WorldSnapshot> {
        let next = self.redo_stack.pop()?;
        self.undo_stack.push(current);
        if self.undo_stack.len() > self.max_checkpoints {
            self.undo_stack.remove(0);
        }
        Some(next)
    }

    /// Clear all undo and redo history.
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    /// Check if undo is available.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Check if redo is available.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Get the count of available undo checkpoints.
    pub fn undo_len(&self) -> usize {
        self.undo_stack.len()
    }

    /// Get the count of available redo checkpoints.
    pub fn redo_len(&self) -> usize {
        self.redo_stack.len()
    }
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
    struct Pos {
        x: i32,
        y: i32,
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
    struct HP(i32);

    #[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
    struct GlobalConfig {
        difficulty: u32,
    }

    #[test]
    fn test_world_snapshot_roundtrip() {
        let mut world = World::new();
        let e1 = world.spawn();
        world.insert(e1, Pos { x: 1, y: 2 });
        world.insert(e1, HP(10));

        let e2 = world.spawn();
        world.insert(e2, Pos { x: 3, y: 4 });

        world.insert_resource(GlobalConfig { difficulty: 5 });

        let mut registry = WorldRegistry::new();
        registry.register_component::<Pos>("Pos");
        registry.register_component::<HP>("HP");
        registry.register_resource::<GlobalConfig>("Config");

        let snapshot = registry.snapshot(&world);

        // Serialize snapshot to JSON string to simulate saving to disk
        let json = serde_json::to_string(&snapshot).unwrap();
        let snapshot_recovered: WorldSnapshot = serde_json::from_str(&json).unwrap();

        let mut new_world = World::new();
        registry.apply(&mut new_world, snapshot_recovered).unwrap();

        assert!(new_world.is_alive(e1));
        assert!(new_world.is_alive(e2));

        assert_eq!(new_world.get::<Pos>(e1), Some(&Pos { x: 1, y: 2 }));
        assert_eq!(new_world.get::<HP>(e1), Some(&HP(10)));
        assert_eq!(new_world.get::<Pos>(e2), Some(&Pos { x: 3, y: 4 }));
        assert_eq!(new_world.get::<HP>(e2), None);

        assert_eq!(
            new_world.resource::<GlobalConfig>(),
            Some(&GlobalConfig { difficulty: 5 })
        );
    }

    #[test]
    fn test_diff_identical_snapshots() {
        let snap = WorldSnapshot {
            entities: vec![EntitySnapshot {
                entity: Entity {
                    index: 0,
                    generation: 1,
                },
                components: HashMap::from([("Pos".into(), serde_json::json!({"x": 1, "y": 2}))]),
            }],
            resources: HashMap::from([("Config".into(), serde_json::json!({"difficulty": 5}))]),
        };
        let diff = snap.diff(&snap);
        assert!(diff.added_entities.is_empty());
        assert!(diff.removed_entities.is_empty());
        assert!(diff.changed_entities.is_empty());
        assert!(diff.added_resources.is_empty());
        assert!(diff.removed_resources.is_empty());
        assert!(diff.changed_resources.is_empty());
    }

    #[test]
    fn test_diff_added_entity() {
        let empty = WorldSnapshot::default();
        let with_entity = WorldSnapshot {
            entities: vec![EntitySnapshot {
                entity: Entity {
                    index: 0,
                    generation: 1,
                },
                components: HashMap::from([("Pos".into(), serde_json::json!({"x": 1, "y": 2}))]),
            }],
            resources: HashMap::new(),
        };
        let diff = empty.diff(&with_entity);
        assert_eq!(diff.added_entities.len(), 1);
        assert!(diff.removed_entities.is_empty());
    }

    #[test]
    fn test_diff_removed_entity() {
        let empty = WorldSnapshot::default();
        let with_entity = WorldSnapshot {
            entities: vec![EntitySnapshot {
                entity: Entity {
                    index: 0,
                    generation: 1,
                },
                components: HashMap::from([("Pos".into(), serde_json::json!({"x": 1, "y": 2}))]),
            }],
            resources: HashMap::new(),
        };
        let diff = with_entity.diff(&empty);
        assert!(diff.added_entities.is_empty());
        assert_eq!(diff.removed_entities.len(), 1);
    }

    #[test]
    fn test_diff_changed_component() {
        let entity = Entity {
            index: 0,
            generation: 1,
        };
        let snap_a = WorldSnapshot {
            entities: vec![EntitySnapshot {
                entity,
                components: HashMap::from([("HP".into(), serde_json::json!(10))]),
            }],
            resources: HashMap::new(),
        };
        let snap_b = WorldSnapshot {
            entities: vec![EntitySnapshot {
                entity,
                components: HashMap::from([("HP".into(), serde_json::json!(5))]),
            }],
            resources: HashMap::new(),
        };
        let diff = snap_a.diff(&snap_b);
        assert!(diff.added_entities.is_empty());
        assert!(diff.removed_entities.is_empty());
        let e_diff = diff.changed_entities.get(&entity).unwrap();
        assert!(e_diff.changed_components.contains_key("HP"));
    }

    #[test]
    fn test_diff_added_removed_component() {
        let entity = Entity {
            index: 0,
            generation: 1,
        };
        let snap_a = WorldSnapshot {
            entities: vec![EntitySnapshot {
                entity,
                components: HashMap::from([("Pos".into(), serde_json::json!({"x": 1, "y": 2}))]),
            }],
            resources: HashMap::new(),
        };
        let snap_b = WorldSnapshot {
            entities: vec![EntitySnapshot {
                entity,
                components: HashMap::from([("HP".into(), serde_json::json!(10))]),
            }],
            resources: HashMap::new(),
        };
        let diff = snap_a.diff(&snap_b);
        let e_diff = diff.changed_entities.get(&entity).unwrap();
        assert!(e_diff.added_components.contains_key("HP"));
        assert!(e_diff.removed_components.contains(&"Pos".to_string()));
    }

    #[test]
    fn test_diff_resource_changes() {
        let snap_a = WorldSnapshot {
            entities: vec![],
            resources: HashMap::from([
                ("Config".into(), serde_json::json!({"difficulty": 1})),
                ("Old".into(), serde_json::json!("gone")),
            ]),
        };
        let snap_b = WorldSnapshot {
            entities: vec![],
            resources: HashMap::from([
                ("Config".into(), serde_json::json!({"difficulty": 5})),
                ("New".into(), serde_json::json!("fresh")),
            ]),
        };
        let diff = snap_a.diff(&snap_b);
        assert!(diff.added_resources.contains_key("New"));
        assert!(diff.removed_resources.contains(&"Old".to_string()));
        assert!(diff.changed_resources.contains_key("Config"));
    }

    #[test]
    fn test_snapshot_empty_world() {
        let world = World::new();
        let registry = WorldRegistry::new();
        let snap = registry.snapshot(&world);
        assert!(snap.entities.is_empty());
        assert!(snap.resources.is_empty());
    }

    #[test]
    fn test_snapshot_entity_without_registered_components() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Pos { x: 1, y: 2 });

        let mut registry = WorldRegistry::new();
        // Only register HP, not Pos — entity should be excluded (no matching components)
        registry.register_component::<HP>("HP");

        let snap = registry.snapshot(&world);
        assert!(snap.entities.is_empty());
    }

    #[test]
    fn test_apply_clears_existing_world() {
        let mut registry = WorldRegistry::new();
        registry.register_component::<Pos>("Pos");

        // Build a snapshot with one entity
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Pos { x: 42, y: 99 });
        let snap = registry.snapshot(&world);

        // Create a target world with different entities
        let mut target = World::new();
        let e_old = target.spawn();
        target.insert(e_old, Pos { x: 0, y: 0 });
        let _e_old2 = target.spawn();

        // Apply snapshot — old entities should be replaced
        registry.apply(&mut target, snap).unwrap();
        // Snapshot entity should be alive with correct data
        assert_eq!(target.get::<Pos>(e), Some(&Pos { x: 42, y: 99 }));
    }

    #[test]
    fn test_apply_diff_incremental_updates() {
        let mut registry = WorldRegistry::new();
        registry.register_component::<Pos>("Pos");
        registry.register_component::<HP>("HP");
        registry.register_resource::<GlobalConfig>("Config");

        // Initial state
        let mut world = World::new();
        let e1 = world.spawn();
        world.insert(e1, Pos { x: 1, y: 1 });
        world.insert_resource(GlobalConfig { difficulty: 1 });
        let snap1 = registry.snapshot(&world);

        // Modified state
        world.insert(e1, Pos { x: 2, y: 2 }); // Change component
        world.insert(e1, HP(100)); // Add component
        let e2 = world.spawn();
        world.insert(e2, Pos { x: 10, y: 10 }); // Add entity
        world.insert_resource(GlobalConfig { difficulty: 2 }); // Change resource
        let snap2 = registry.snapshot(&world);

        // Compute diff
        let diff = snap1.diff(&snap2);

        // Apply diff to a world that matches snap1
        let mut target = World::new();
        target.spawn_at(e1);
        target.insert(e1, Pos { x: 1, y: 1 });
        target.insert_resource(GlobalConfig { difficulty: 1 });

        registry.apply_diff(&mut target, diff).unwrap();

        // Verify target matches snap2
        assert_eq!(target.get::<Pos>(e1), Some(&Pos { x: 2, y: 2 }));
        assert_eq!(target.get::<HP>(e1), Some(&HP(100)));
        assert!(target.is_alive(e2));
        assert_eq!(target.get::<Pos>(e2), Some(&Pos { x: 10, y: 10 }));
        assert_eq!(
            target.resource::<GlobalConfig>(),
            Some(&GlobalConfig { difficulty: 2 })
        );
    }

    #[test]
    fn test_world_checkpoint_stack() {
        let mut stack = WorldCheckpointStack::new(2);
        let mut snap1 = WorldSnapshot::default();
        snap1
            .resources
            .insert("val".to_string(), serde_json::Value::Number(1.into()));
        let mut snap2 = WorldSnapshot::default();
        snap2
            .resources
            .insert("val".to_string(), serde_json::Value::Number(2.into()));
        let mut snap3 = WorldSnapshot::default();
        snap3
            .resources
            .insert("val".to_string(), serde_json::Value::Number(3.into()));

        assert!(!stack.can_undo());
        assert!(!stack.can_redo());

        stack.push_checkpoint(snap1.clone());
        assert!(stack.can_undo());
        assert_eq!(stack.undo_len(), 1);

        stack.push_checkpoint(snap2.clone());
        assert_eq!(stack.undo_len(), 2);

        // Test capacity limit of 2: pushing snap3 should drop snap1
        stack.push_checkpoint(snap3.clone());
        assert_eq!(stack.undo_len(), 2);

        let current = WorldSnapshot::default();
        let undone = stack.undo(current.clone()).unwrap();
        // We popped snap3
        assert_eq!(undone.resources.get("val").unwrap().as_i64().unwrap(), 3);
        assert_eq!(stack.redo_len(), 1);

        let redone = stack.redo(current.clone()).unwrap();
        assert_eq!(redone.resources, current.resources);
        // We popped current (from redo stack)
        assert_eq!(stack.undo_len(), 2);
    }
}
