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

pub trait ComponentRegistration: Send + Sync {
    fn serialize(&self, world: &World, entity: Entity) -> Option<serde_json::Value>;
    fn deserialize(
        &self,
        world: &mut World,
        entity: Entity,
        value: serde_json::Value,
    ) -> Result<(), String>;
}

pub trait ResourceRegistration: Send + Sync {
    fn serialize(&self, world: &World) -> Option<serde_json::Value>;
    fn deserialize(&self, world: &mut World, value: serde_json::Value) -> Result<(), String>;
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
}
