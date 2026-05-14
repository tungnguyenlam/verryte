//! The `World` owns entities, components, and resources.
//!
//! The storage strategy is intentionally simple:
//!
//! * Entities are allocated with generational indices.
//! * Each component type `T` lives in a dense `Vec<Option<(generation, T)>>`
//!   keyed by entity index. Stale handles silently miss.
//! * Resources are singletons keyed by [`TypeId`].
//!
//! It is not the fastest ECS in the world. It is small, transparent, and
//! enough to drive a turn-based terminal game without locking the engine into
//! an exotic API.

use std::any::{Any, TypeId};
use std::collections::HashMap;

use crate::entity::Entity;

trait Column: Any + Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn clear_index(&mut self, index: usize);
}

struct TypedColumn<T: 'static + Send + Sync> {
    slots: Vec<Option<(u32, T)>>,
}

impl<T: 'static + Send + Sync> TypedColumn<T> {
    fn new() -> Self {
        Self { slots: Vec::new() }
    }

    fn ensure(&mut self, index: usize) {
        if index >= self.slots.len() {
            self.slots.resize_with(index + 1, || None);
        }
    }
}

impl<T: 'static + Send + Sync> Column for TypedColumn<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn clear_index(&mut self, index: usize) {
        if let Some(slot) = self.slots.get_mut(index) {
            *slot = None;
        }
    }
}

/// The container that holds entities, their components, and engine resources.
pub struct World {
    generations: Vec<u32>,
    alive: Vec<bool>,
    free: Vec<u32>,
    columns: HashMap<TypeId, Box<dyn Column>>,
    resources: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl World {
    pub fn new() -> Self {
        Self {
            generations: Vec::new(),
            alive: Vec::new(),
            free: Vec::new(),
            columns: HashMap::new(),
            resources: HashMap::new(),
        }
    }

    /// Allocate a fresh [`Entity`].
    pub fn spawn(&mut self) -> Entity {
        if let Some(index) = self.free.pop() {
            let idx = index as usize;
            self.alive[idx] = true;
            self.generations[idx] = self.generations[idx].wrapping_add(1).max(1);
            Entity {
                index,
                generation: self.generations[idx],
            }
        } else {
            let index = self.generations.len() as u32;
            self.generations.push(1);
            self.alive.push(true);
            Entity {
                index,
                generation: 1,
            }
        }
    }

    pub fn is_alive(&self, entity: Entity) -> bool {
        let idx = entity.index as usize;
        idx < self.generations.len()
            && self.alive[idx]
            && self.generations[idx] == entity.generation
    }

    /// Despawn an entity and drop every component it owns. Returns `false` if
    /// the handle was already stale.
    pub fn despawn(&mut self, entity: Entity) -> bool {
        if !self.is_alive(entity) {
            return false;
        }
        let idx = entity.index as usize;
        self.alive[idx] = false;
        for column in self.columns.values_mut() {
            column.clear_index(idx);
        }
        self.free.push(entity.index);
        true
    }

    pub fn entity_count(&self) -> usize {
        self.alive.iter().filter(|a| **a).count()
    }

    /// Attach a component to an entity. Returns the previous value if one was
    /// already set, or `None` if the slot was empty or the entity is stale.
    pub fn insert<T: 'static + Send + Sync>(&mut self, entity: Entity, value: T) -> Option<T> {
        if !self.is_alive(entity) {
            return None;
        }
        let column = self
            .columns
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(TypedColumn::<T>::new()));
        let typed = column
            .as_any_mut()
            .downcast_mut::<TypedColumn<T>>()
            .expect("column type matches TypeId");
        let idx = entity.index as usize;
        typed.ensure(idx);
        let prev = typed.slots[idx].take();
        typed.slots[idx] = Some((entity.generation, value));
        prev.map(|(_, v)| v)
    }

    pub fn get<T: 'static + Send + Sync>(&self, entity: Entity) -> Option<&T> {
        let column = self.columns.get(&TypeId::of::<T>())?;
        let typed = column.as_any().downcast_ref::<TypedColumn<T>>()?;
        let slot = typed.slots.get(entity.index as usize)?.as_ref()?;
        if slot.0 == entity.generation {
            Some(&slot.1)
        } else {
            None
        }
    }

    pub fn get_mut<T: 'static + Send + Sync>(&mut self, entity: Entity) -> Option<&mut T> {
        let column = self.columns.get_mut(&TypeId::of::<T>())?;
        let typed = column.as_any_mut().downcast_mut::<TypedColumn<T>>()?;
        let slot = typed.slots.get_mut(entity.index as usize)?.as_mut()?;
        if slot.0 == entity.generation {
            Some(&mut slot.1)
        } else {
            None
        }
    }

    pub fn has<T: 'static + Send + Sync>(&self, entity: Entity) -> bool {
        self.get::<T>(entity).is_some()
    }

    pub fn remove<T: 'static + Send + Sync>(&mut self, entity: Entity) -> Option<T> {
        let column = self.columns.get_mut(&TypeId::of::<T>())?;
        let typed = column.as_any_mut().downcast_mut::<TypedColumn<T>>()?;
        let slot = typed.slots.get_mut(entity.index as usize)?;
        match slot.take() {
            Some((gen, value)) if gen == entity.generation => Some(value),
            other => {
                *slot = other;
                None
            }
        }
    }

    /// Collect every live `(entity, &component)` pair for a given component
    /// type. The result allocates so callers can hand the iterator off freely;
    /// the engine is not yet hot-loop oriented.
    pub fn query<T: 'static + Send + Sync>(&self) -> Vec<(Entity, &T)> {
        let mut out = Vec::new();
        let Some(column) = self.columns.get(&TypeId::of::<T>()) else {
            return out;
        };
        let Some(typed) = column.as_any().downcast_ref::<TypedColumn<T>>() else {
            return out;
        };
        for (i, slot) in typed.slots.iter().enumerate() {
            if let Some((gen, value)) = slot {
                if (i < self.alive.len()) && self.alive[i] {
                    out.push((
                        Entity {
                            index: i as u32,
                            generation: *gen,
                        },
                        value,
                    ));
                }
            }
        }
        out
    }

    /// Collect every live entity that has both component types.
    pub fn query2<A, B>(&self) -> Vec<(Entity, &A, &B)>
    where
        A: 'static + Send + Sync,
        B: 'static + Send + Sync,
    {
        let mut out = Vec::new();
        if TypeId::of::<A>() == TypeId::of::<B>() {
            return out;
        }

        let Some(column_a) = self.columns.get(&TypeId::of::<A>()) else {
            return out;
        };
        let Some(column_b) = self.columns.get(&TypeId::of::<B>()) else {
            return out;
        };
        let Some(typed_a) = column_a.as_any().downcast_ref::<TypedColumn<A>>() else {
            return out;
        };
        let Some(typed_b) = column_b.as_any().downcast_ref::<TypedColumn<B>>() else {
            return out;
        };

        for (i, slot_a) in typed_a.slots.iter().enumerate() {
            if i >= self.alive.len() || !self.alive[i] {
                continue;
            }
            let Some((gen_a, value_a)) = slot_a else {
                continue;
            };
            let Some((gen_b, value_b)) = typed_b.slots.get(i).and_then(|slot| slot.as_ref()) else {
                continue;
            };
            if gen_a == gen_b {
                out.push((
                    Entity {
                        index: i as u32,
                        generation: *gen_a,
                    },
                    value_a,
                    value_b,
                ));
            }
        }
        out
    }

    /// Visit every live component of type `T` mutably.
    pub fn for_each_mut<T, F>(&mut self, mut f: F)
    where
        T: 'static + Send + Sync,
        F: FnMut(Entity, &mut T),
    {
        let alive = &self.alive;
        let Some(column) = self.columns.get_mut(&TypeId::of::<T>()) else {
            return;
        };
        let Some(typed) = column.as_any_mut().downcast_mut::<TypedColumn<T>>() else {
            return;
        };
        for (i, slot) in typed.slots.iter_mut().enumerate() {
            if let Some((gen, value)) = slot.as_mut() {
                if i < alive.len() && alive[i] {
                    f(
                        Entity {
                            index: i as u32,
                            generation: *gen,
                        },
                        value,
                    );
                }
            }
        }
    }

    /// Install a resource of type `R`. Returns the previous value if present.
    pub fn insert_resource<R: 'static + Send + Sync>(&mut self, resource: R) -> Option<R> {
        let prev = self.resources.insert(TypeId::of::<R>(), Box::new(resource));
        prev.and_then(|boxed| boxed.downcast::<R>().ok().map(|b| *b))
    }

    pub fn resource<R: 'static + Send + Sync>(&self) -> Option<&R> {
        self.resources.get(&TypeId::of::<R>())?.downcast_ref::<R>()
    }

    pub fn resource_mut<R: 'static + Send + Sync>(&mut self) -> Option<&mut R> {
        self.resources
            .get_mut(&TypeId::of::<R>())?
            .downcast_mut::<R>()
    }

    pub fn remove_resource<R: 'static + Send + Sync>(&mut self) -> Option<R> {
        let boxed = self.resources.remove(&TypeId::of::<R>())?;
        boxed.downcast::<R>().ok().map(|b| *b)
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct Pos(i32, i32);

    #[derive(Debug, PartialEq)]
    struct Tag(&'static str);

    #[derive(Debug, PartialEq)]
    struct Counter(u32);

    #[test]
    fn spawn_insert_get_roundtrip() {
        let mut world = World::new();
        let e = world.spawn();
        assert!(world.is_alive(e));
        assert!(world.insert(e, Pos(1, 2)).is_none());
        assert_eq!(world.get::<Pos>(e), Some(&Pos(1, 2)));
        assert!(world.has::<Pos>(e));
    }

    #[test]
    fn despawned_handles_dont_resolve() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Tag("alpha"));
        assert!(world.despawn(e));
        assert!(!world.is_alive(e));
        assert_eq!(world.get::<Tag>(e), None);
        assert!(!world.despawn(e));
    }

    #[test]
    fn generation_invalidates_old_handle() {
        let mut world = World::new();
        let a = world.spawn();
        world.despawn(a);
        let b = world.spawn();
        assert_eq!(a.index(), b.index(), "index slot should be reused");
        assert_ne!(a.generation(), b.generation());
        world.insert(b, Tag("new"));
        assert_eq!(world.get::<Tag>(a), None);
        assert_eq!(world.get::<Tag>(b), Some(&Tag("new")));
    }

    #[test]
    fn query_only_returns_live_entities() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        let c = world.spawn();
        world.insert(a, Counter(1));
        world.insert(b, Counter(2));
        world.insert(c, Counter(3));
        world.despawn(b);
        let mut values: Vec<u32> = world
            .query::<Counter>()
            .into_iter()
            .map(|(_, c)| c.0)
            .collect();
        values.sort_unstable();
        assert_eq!(values, vec![1, 3]);
    }

    #[test]
    fn query2_only_returns_entities_with_both_components() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        let c = world.spawn();
        world.insert(a, Pos(1, 1));
        world.insert(a, Tag("player"));
        world.insert(b, Pos(2, 2));
        world.insert(c, Tag("marker"));

        let rows = world.query2::<Pos, Tag>();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, a);
        assert_eq!(rows[0].1, &Pos(1, 1));
        assert_eq!(rows[0].2, &Tag("player"));
    }

    #[test]
    fn for_each_mut_updates_live_components() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        world.insert(a, Counter(0));
        world.insert(b, Counter(0));
        world.for_each_mut::<Counter, _>(|_, c| c.0 += 5);
        assert_eq!(world.get::<Counter>(a), Some(&Counter(5)));
        assert_eq!(world.get::<Counter>(b), Some(&Counter(5)));
    }

    #[test]
    fn resources_are_singletons_per_type() {
        let mut world = World::new();
        assert_eq!(world.insert_resource(Counter(10)), None);
        assert_eq!(world.resource::<Counter>(), Some(&Counter(10)));
        let prev = world.insert_resource(Counter(11));
        assert_eq!(prev, Some(Counter(10)));
        world.resource_mut::<Counter>().unwrap().0 += 1;
        assert_eq!(world.resource::<Counter>(), Some(&Counter(12)));
        assert_eq!(world.remove_resource::<Counter>(), Some(Counter(12)));
        assert_eq!(world.resource::<Counter>(), None);
    }

    #[test]
    fn remove_returns_value_and_clears_slot() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Pos(7, 8));
        assert_eq!(world.remove::<Pos>(e), Some(Pos(7, 8)));
        assert_eq!(world.get::<Pos>(e), None);
        assert_eq!(world.remove::<Pos>(e), None);
    }
}
