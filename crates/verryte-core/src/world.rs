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
    fn into_any(self: Box<Self>) -> Box<dyn Any + Send + Sync>;
    fn shrink_to_fit(&mut self);
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
    fn into_any(self: Box<Self>) -> Box<dyn Any + Send + Sync> {
        self
    }
    fn shrink_to_fit(&mut self) {
        while self.slots.last().is_none() {
            self.slots.pop();
        }
        self.slots.shrink_to_fit();
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

    /// Pre-allocate entity ID slots for bulk spawning.
    ///
    /// Reserves capacity for `n` additional entities without actually spawning
    /// them. This avoids repeated reallocations when spawning many entities at
    /// once (e.g., during level generation or map population).
    ///
    /// The reserved slots are added to the free list, so subsequent `spawn()`
    /// calls will reuse them without growing the internal vectors.
    pub fn reserve_entities(&mut self, n: usize) {
        let start = self.generations.len() as u32;
        self.generations.reserve(n);
        self.alive.reserve(n);
        self.free.reserve(n);
        for i in 0..n {
            self.generations.push(1);
            self.alive.push(false);
            self.free.push(start + i as u32);
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

    /// Despawn every live entity that has component `T`.
    ///
    /// Returns the number of entities removed. Useful for bulk cleanup of
    /// temporary entities like projectiles, effects, or expired pickups.
    pub fn despawn_with<T: 'static + Send + Sync>(&mut self) -> usize {
        let entities: Vec<Entity> = self.query::<T>().into_iter().map(|(e, _)| e).collect();
        let count = entities.len();
        for entity in entities {
            self.despawn(entity);
        }
        count
    }

    /// Keep only entities for which `predicate` returns `true`.
    ///
    /// Every live entity is tested; entities that fail the predicate are
    /// despawned. Returns the number of entities removed.
    pub fn retain<F>(&mut self, mut predicate: F) -> usize
    where
        F: FnMut(Entity) -> bool,
    {
        let to_remove: Vec<Entity> = self
            .alive
            .iter()
            .enumerate()
            .filter_map(|(idx, &alive)| {
                if !alive {
                    return None;
                }
                let gen = self.generations[idx];
                let entity = Entity {
                    index: idx as u32,
                    generation: gen,
                };
                (!predicate(entity)).then_some(entity)
            })
            .collect();
        let count = to_remove.len();
        for entity in to_remove {
            self.despawn(entity);
        }
        count
    }

    pub fn entity_count(&self) -> usize {
        self.alive.iter().filter(|a| **a).count()
    }

    /// Iterate over all live entities.
    pub fn entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.alive.iter().enumerate().filter_map(|(idx, &alive)| {
            if alive {
                Some(Entity {
                    index: idx as u32,
                    generation: self.generations[idx],
                })
            } else {
                None
            }
        })
    }

    /// Despawn all live entities and clear their components, leaving resources intact.
    pub fn clear_entities(&mut self) {
        for idx in 0..self.alive.len() {
            if self.alive[idx] {
                self.alive[idx] = false;
                for column in self.columns.values_mut() {
                    column.clear_index(idx);
                }
                self.free.push(idx as u32);
            }
        }
    }

    /// Clear all state: entities, components, and resources. Returns to a fresh world.
    pub fn clear(&mut self) {
        self.clear_entities();
        self.resources.clear();
    }

    /// Reclaim memory from dead entity slots.
    ///
    /// Trims trailing `None` entries from every component column and calls
    /// `shrink_to_fit()` on the underlying vectors. This does not compact
    /// holes in the middle of columns — it only trims unused capacity at the
    /// end. Useful after bulk despawns or level transitions.
    pub fn shrink(&mut self) {
        for column in self.columns.values_mut() {
            column.shrink_to_fit();
        }
        // Also trim generations/alive/free if possible.
        while self.generations.last().is_some_and(|&g| g == 0) {
            self.generations.pop();
            self.alive.pop();
        }
        self.generations.shrink_to_fit();
        self.alive.shrink_to_fit();
        self.free.shrink_to_fit();
        self.resources.shrink_to_fit();
        self.columns.shrink_to_fit();
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

    /// Get a component, or insert a default value if the entity doesn't have it.
    ///
    /// Returns a mutable reference to the component. If the entity is stale,
    /// returns `None`.
    pub fn get_or_insert<T: 'static + Send + Sync + Default>(
        &mut self,
        entity: Entity,
    ) -> Option<&mut T> {
        if !self.is_alive(entity) {
            return None;
        }
        if self.get::<T>(entity).is_some() {
            return self.get_mut::<T>(entity);
        }
        self.insert(entity, T::default());
        self.get_mut::<T>(entity)
    }

    /// Get a component, or insert a provided value if the entity doesn't have it.
    ///
    /// Returns a mutable reference to the component. If the entity is stale,
    /// returns `None`.
    pub fn get_or_insert_with<T, F>(&mut self, entity: Entity, f: F) -> Option<&mut T>
    where
        T: 'static + Send + Sync,
        F: FnOnce() -> T,
    {
        if !self.is_alive(entity) {
            return None;
        }
        if self.get::<T>(entity).is_some() {
            return self.get_mut::<T>(entity);
        }
        self.insert(entity, f());
        self.get_mut::<T>(entity)
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

    /// Count how many live entities have a given component type.
    pub fn count_with<T: 'static + Send + Sync>(&self) -> usize {
        let Some(column) = self.columns.get(&TypeId::of::<T>()) else {
            return 0;
        };
        let Some(typed) = column.as_any().downcast_ref::<TypedColumn<T>>() else {
            return 0;
        };
        typed
            .slots
            .iter()
            .enumerate()
            .filter(|(i, slot)| {
                slot.is_some()
                    && i < &self.alive.len()
                    && self.alive[*i]
                    && slot
                        .as_ref()
                        .is_some_and(|(gen, _)| *gen == self.generations[*i])
            })
            .count()
    }

    /// Check whether any live entity has a given component type.
    ///
    /// Returns `true` if at least one entity has the component. Equivalent to
    /// `count_with::<T>() > 0` but short-circuits on the first match.
    pub fn contains<T: 'static + Send + Sync>(&self) -> bool {
        let Some(column) = self.columns.get(&TypeId::of::<T>()) else {
            return false;
        };
        let Some(typed) = column.as_any().downcast_ref::<TypedColumn<T>>() else {
            return false;
        };
        typed.slots.iter().enumerate().any(|(i, slot)| {
            slot.is_some()
                && i < self.alive.len()
                && self.alive[i]
                && slot
                    .as_ref()
                    .is_some_and(|(gen, _)| *gen == self.generations[i])
        })
    }

    /// Query entities with a component, returning an iterator.
    pub fn query_iter<T: 'static + Send + Sync>(&self) -> Query<'_, T> {
        Query {
            iter: self.query::<T>().into_iter(),
        }
    }

    /// Query entities with two components, returning an iterator.
    pub fn query2_iter<A, B>(&self) -> Query2<'_, A, B>
    where
        A: 'static + Send + Sync,
        B: 'static + Send + Sync,
    {
        Query2 {
            iter: self.query2::<A, B>().into_iter(),
        }
    }

    /// Query entities with three components, returning an iterator.
    pub fn query3_iter<A, B, C>(&self) -> Query3<'_, A, B, C>
    where
        A: 'static + Send + Sync,
        B: 'static + Send + Sync,
        C: 'static + Send + Sync,
    {
        Query3 {
            iter: self.query3::<A, B, C>().into_iter(),
        }
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

    /// Collect every live entity that has all three component types.
    pub fn query3<A, B, C>(&self) -> Vec<(Entity, &A, &B, &C)>
    where
        A: 'static + Send + Sync,
        B: 'static + Send + Sync,
        C: 'static + Send + Sync,
    {
        let mut out = Vec::new();
        if TypeId::of::<A>() == TypeId::of::<B>()
            || TypeId::of::<A>() == TypeId::of::<C>()
            || TypeId::of::<B>() == TypeId::of::<C>()
        {
            return out;
        }

        let Some(column_a) = self.columns.get(&TypeId::of::<A>()) else {
            return out;
        };
        let Some(column_b) = self.columns.get(&TypeId::of::<B>()) else {
            return out;
        };
        let Some(column_c) = self.columns.get(&TypeId::of::<C>()) else {
            return out;
        };
        let Some(typed_a) = column_a.as_any().downcast_ref::<TypedColumn<A>>() else {
            return out;
        };
        let Some(typed_b) = column_b.as_any().downcast_ref::<TypedColumn<B>>() else {
            return out;
        };
        let Some(typed_c) = column_c.as_any().downcast_ref::<TypedColumn<C>>() else {
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
            if gen_a != gen_b {
                continue;
            }
            let Some((gen_c, value_c)) = typed_c.slots.get(i).and_then(|slot| slot.as_ref()) else {
                continue;
            };
            if gen_a != gen_c {
                continue;
            }
            out.push((
                Entity {
                    index: i as u32,
                    generation: *gen_a,
                },
                value_a,
                value_b,
                value_c,
            ));
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

    /// Visit every live entity that has both `A` and `B`, yielding mutable
    /// references to both components.
    ///
    /// Returns `false` immediately if `A` and `B` are the same type, since
    /// two simultaneous mutable borrows of the same column are not allowed.
    pub fn for_each2_mut<A, B, F>(&mut self, mut f: F) -> bool
    where
        A: 'static + Send + Sync,
        B: 'static + Send + Sync,
        F: FnMut(Entity, &mut A, &mut B),
    {
        if TypeId::of::<A>() == TypeId::of::<B>() {
            return false;
        }
        let id_a = TypeId::of::<A>();
        let id_b = TypeId::of::<B>();

        // Collect matching indices first (read-only).
        let Some(col_a) = self.columns.get(&id_a) else {
            return false;
        };
        let Some(col_b) = self.columns.get(&id_b) else {
            return false;
        };
        let Some(typed_a) = col_a.as_any().downcast_ref::<TypedColumn<A>>() else {
            return false;
        };
        let Some(typed_b) = col_b.as_any().downcast_ref::<TypedColumn<B>>() else {
            return false;
        };
        let alive = &self.alive;
        let len = typed_a
            .slots
            .len()
            .min(typed_b.slots.len())
            .min(alive.len());

        let indices: Vec<usize> = (0..len)
            .filter(|&i| {
                if !alive[i] {
                    return false;
                }
                typed_a.slots[i]
                    .as_ref()
                    .and_then(|(ga, _)| typed_b.slots[i].as_ref().map(|(gb, _)| ga == gb))
                    .unwrap_or(false)
            })
            .collect();

        if indices.is_empty() {
            return true;
        }

        // Swap out the columns map to get owned access to both columns.
        let mut columns = std::mem::take(&mut self.columns);
        let col_a = columns.remove(&id_a).unwrap();
        let col_b = columns.remove(&id_b).unwrap();

        // Convert to Box<dyn Any> for safe downcast.
        let any_a = col_a.into_any();
        let any_b = col_b.into_any();
        let mut typed_a = any_a.downcast::<TypedColumn<A>>().unwrap();
        let mut typed_b = any_b.downcast::<TypedColumn<B>>().unwrap();

        for i in indices {
            let gen = typed_a.slots[i].as_ref().unwrap().0;
            let (_, val_a) = typed_a.slots[i].as_mut().unwrap();
            let (_, val_b) = typed_b.slots[i].as_mut().unwrap();
            let entity = Entity {
                index: i as u32,
                generation: gen,
            };
            f(entity, val_a, val_b);
        }

        // Restore columns.
        columns.insert(id_a, typed_a);
        columns.insert(id_b, typed_b);
        self.columns = columns;
        true
    }

    /// Visit every live entity that has `A`, `B`, and `C`, yielding mutable
    /// references to all three components.
    ///
    /// Returns `false` immediately if any two types are the same. Uses the
    /// same column-swap pattern as `for_each2_mut` to get owned access for
    /// safe downcasting.
    pub fn for_each3_mut<A, B, C, F>(&mut self, mut f: F) -> bool
    where
        A: 'static + Send + Sync,
        B: 'static + Send + Sync,
        C: 'static + Send + Sync,
        F: FnMut(Entity, &mut A, &mut B, &mut C),
    {
        let id_a = TypeId::of::<A>();
        let id_b = TypeId::of::<B>();
        let id_c = TypeId::of::<C>();

        if id_a == id_b || id_a == id_c || id_b == id_c {
            return false;
        }

        let Some(col_a) = self.columns.get(&id_a) else {
            return false;
        };
        let Some(col_b) = self.columns.get(&id_b) else {
            return false;
        };
        let Some(col_c) = self.columns.get(&id_c) else {
            return false;
        };
        let Some(typed_a) = col_a.as_any().downcast_ref::<TypedColumn<A>>() else {
            return false;
        };
        let Some(typed_b) = col_b.as_any().downcast_ref::<TypedColumn<B>>() else {
            return false;
        };
        let Some(typed_c) = col_c.as_any().downcast_ref::<TypedColumn<C>>() else {
            return false;
        };
        let alive = &self.alive;
        let len = typed_a
            .slots
            .len()
            .min(typed_b.slots.len())
            .min(typed_c.slots.len())
            .min(alive.len());

        let indices: Vec<usize> = (0..len)
            .filter(|&i| {
                if !alive[i] {
                    return false;
                }
                let Some((ga, _)) = typed_a.slots[i].as_ref() else {
                    return false;
                };
                let Some((gb, _)) = typed_b.slots[i].as_ref() else {
                    return false;
                };
                let Some((gc, _)) = typed_c.slots[i].as_ref() else {
                    return false;
                };
                ga == gb && ga == gc
            })
            .collect();

        if indices.is_empty() {
            return true;
        }

        let mut columns = std::mem::take(&mut self.columns);
        let col_a = columns.remove(&id_a).unwrap();
        let col_b = columns.remove(&id_b).unwrap();
        let col_c = columns.remove(&id_c).unwrap();

        let any_a = col_a.into_any();
        let any_b = col_b.into_any();
        let any_c = col_c.into_any();
        let mut typed_a = any_a.downcast::<TypedColumn<A>>().unwrap();
        let mut typed_b = any_b.downcast::<TypedColumn<B>>().unwrap();
        let mut typed_c = any_c.downcast::<TypedColumn<C>>().unwrap();

        for i in indices {
            let gen = typed_a.slots[i].as_ref().unwrap().0;
            let (_, val_a) = typed_a.slots[i].as_mut().unwrap();
            let (_, val_b) = typed_b.slots[i].as_mut().unwrap();
            let (_, val_c) = typed_c.slots[i].as_mut().unwrap();
            let entity = Entity {
                index: i as u32,
                generation: gen,
            };
            f(entity, val_a, val_b, val_c);
        }

        columns.insert(id_a, typed_a);
        columns.insert(id_b, typed_b);
        columns.insert(id_c, typed_c);
        self.columns = columns;
        true
    }

    /// Install a resource of type `R`. Returns the previous value if present.
    pub fn insert_resource<R: 'static + Send + Sync>(&mut self, resource: R) -> Option<R> {
        let prev = self.resources.insert(TypeId::of::<R>(), Box::new(resource));
        prev.and_then(|boxed| boxed.downcast::<R>().ok().map(|b| *b))
    }

    pub fn resource<R: 'static + Send + Sync>(&self) -> Option<&R> {
        self.resources.get(&TypeId::of::<R>())?.downcast_ref::<R>()
    }

    pub fn has_resource<R: 'static + Send + Sync>(&self) -> bool {
        self.resources.contains_key(&TypeId::of::<R>())
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

    /// Spawn an entity and attach components in a single fluent call.
    ///
    /// Returns an [`EntityBuilder`] that lets you chain component insertions
    /// and finalize with `.build()`. This avoids the spawn-then-insert pattern
    /// and keeps entity creation compact.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let player = world.builder()
    ///     .with(Position { x: 0, y: 0 })
    ///     .with(Health(100))
    ///     .with(Tag("player"))
    ///     .build();
    /// ```
    pub fn builder(&mut self) -> EntityBuilder<'_> {
        let entity = self.spawn();
        EntityBuilder {
            world: self,
            entity,
        }
    }

    /// Spawn `n` entities, each with the same component value.
    ///
    /// The component is cloned for each entity. Returns the list of spawned
    /// entities. Useful for bulk placement of hazards, enemies, items, or any
    /// entity type that shares initial component state.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let hazards = world.spawn_batch(5, Position { x: 0, y: 0 });
    /// assert_eq!(hazards.len(), 5);
    /// ```
    pub fn spawn_batch<T: 'static + Send + Sync + Clone>(
        &mut self,
        n: usize,
        component: T,
    ) -> Vec<Entity> {
        let mut entities = Vec::with_capacity(n);
        for _ in 0..n {
            let e = self.spawn();
            self.insert(e, component.clone());
            entities.push(e);
        }
        entities
    }
}

/// A fluent builder for spawning entities with multiple components.
///
/// Created by [`World::builder`]. Call `.with(component)` to attach components
/// and `.build()` to finalize and return the entity.
pub struct EntityBuilder<'w> {
    world: &'w mut World,
    entity: Entity,
}

impl<'w> EntityBuilder<'w> {
    /// Attach a component to the entity being built.
    pub fn with<T: 'static + Send + Sync>(self, value: T) -> Self {
        self.world.insert(self.entity, value);
        self
    }

    /// Finalize the builder and return the spawned entity.
    pub fn build(self) -> Entity {
        self.entity
    }

    /// Get a reference to the entity being built.
    pub fn entity(&self) -> Entity {
        self.entity
    }
}

/// An iterator over query results.
pub struct Query<'a, T> {
    iter: std::vec::IntoIter<(Entity, &'a T)>,
}

impl<'a, T> Iterator for Query<'a, T> {
    type Item = (Entity, &'a T);
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a, T> ExactSizeIterator for Query<'a, T> {}

/// An iterator over two-component query results.
pub struct Query2<'a, A, B> {
    iter: std::vec::IntoIter<(Entity, &'a A, &'a B)>,
}

impl<'a, A, B> Iterator for Query2<'a, A, B> {
    type Item = (Entity, &'a A, &'a B);
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a, A, B> ExactSizeIterator for Query2<'a, A, B> {}

/// An iterator over three-component query results.
pub struct Query3<'a, A, B, C> {
    iter: std::vec::IntoIter<(Entity, &'a A, &'a B, &'a C)>,
}

impl<'a, A, B, C> Iterator for Query3<'a, A, B, C> {
    type Item = (Entity, &'a A, &'a B, &'a C);
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a, A, B, C> ExactSizeIterator for Query3<'a, A, B, C> {}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Clone)]
    struct Pos(i32, i32);

    #[derive(Debug, PartialEq)]
    struct Tag(&'static str);

    #[derive(Debug, PartialEq, Clone)]
    struct Counter(u32);

    impl Default for Counter {
        fn default() -> Self {
            Self(0)
        }
    }

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
    fn clear_entities_removes_all_entities_but_keeps_resources() {
        let mut world = World::new();
        let e1 = world.spawn();
        world.insert(e1, Tag("one"));
        let e2 = world.spawn();
        world.insert(e2, Tag("two"));

        world.insert_resource(Counter(42));

        assert_eq!(world.entity_count(), 2);
        world.clear_entities();

        assert_eq!(world.entity_count(), 0);
        assert!(!world.is_alive(e1));
        assert!(!world.is_alive(e2));
        assert!(world.query::<Tag>().is_empty());
        assert_eq!(world.resource::<Counter>(), Some(&Counter(42)));
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
    fn has_resource_checks_existence_without_borrowing() {
        let mut world = World::new();
        assert!(!world.has_resource::<Counter>());
        world.insert_resource(Counter(42));
        assert!(world.has_resource::<Counter>());
        assert!(!world.has_resource::<Tag>());
        world.remove_resource::<Counter>();
        assert!(!world.has_resource::<Counter>());
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

    #[test]
    fn query3_only_returns_entities_with_all_three_components() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        let c = world.spawn();
        let d = world.spawn();
        world.insert(a, Pos(1, 1));
        world.insert(a, Tag("alpha"));
        world.insert(a, Counter(1));
        world.insert(b, Pos(2, 2));
        world.insert(b, Tag("beta"));
        world.insert(c, Pos(3, 3));
        world.insert(c, Counter(3));
        world.insert(d, Tag("delta"));

        let rows = world.query3::<Pos, Tag, Counter>();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, a);
        assert_eq!(rows[0].1, &Pos(1, 1));
        assert_eq!(rows[0].2, &Tag("alpha"));
        assert_eq!(rows[0].3, &Counter(1));
    }

    #[test]
    fn query3_returns_empty_when_no_entity_has_all_three() {
        let mut world = World::new();
        let a = world.spawn();
        world.insert(a, Pos(0, 0));
        world.insert(a, Tag("only-two"));

        assert!(world.query3::<Pos, Tag, Counter>().is_empty());
    }

    #[test]
    fn despawn_with_removes_all_entities_having_component() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        let c = world.spawn();
        world.insert(a, Pos(1, 1));
        world.insert(a, Tag("player"));
        world.insert(b, Pos(2, 2));
        world.insert(b, Tag("enemy"));
        world.insert(c, Pos(3, 3));

        let removed = world.despawn_with::<Tag>();
        assert_eq!(removed, 2);
        assert_eq!(world.entity_count(), 1);
        assert!(world.is_alive(c));
        assert!(!world.is_alive(a));
        assert!(!world.is_alive(b));
    }

    #[test]
    fn despawn_with_returns_zero_when_none_match() {
        let mut world = World::new();
        let a = world.spawn();
        world.insert(a, Pos(0, 0));

        assert_eq!(world.despawn_with::<Tag>(), 0);
        assert!(world.is_alive(a));
    }

    #[test]
    fn retain_keeps_entities_matching_predicate() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        let c = world.spawn();
        world.insert(a, Counter(1));
        world.insert(b, Counter(5));
        world.insert(c, Counter(10));

        // Pre-collect the entities to keep.
        let keep: Vec<Entity> = world
            .query::<Counter>()
            .into_iter()
            .filter(|(_, c)| c.0 >= 5)
            .map(|(e, _)| e)
            .collect();
        let removed = world.retain(|e| keep.contains(&e));
        assert_eq!(removed, 1);
        assert_eq!(world.entity_count(), 2);
        assert!(!world.is_alive(a));
        assert!(world.is_alive(b));
        assert!(world.is_alive(c));
    }

    #[test]
    fn retain_removes_all_when_none_match() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        world.insert(a, Counter(1));
        world.insert(b, Counter(2));

        let removed = world.retain(|_| false);
        assert_eq!(removed, 2);
        assert_eq!(world.entity_count(), 0);
    }

    #[test]
    fn retain_keeps_all_when_all_match() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        world.insert(a, Counter(1));
        world.insert(b, Counter(2));

        let removed = world.retain(|_| true);
        assert_eq!(removed, 0);
        assert_eq!(world.entity_count(), 2);
    }

    #[test]
    fn retain_does_not_affect_resources() {
        let mut world = World::new();
        let a = world.spawn();
        world.insert(a, Counter(1));
        world.insert_resource(Tag("keep"));

        world.retain(|_| false);
        assert_eq!(world.resource::<Tag>(), Some(&Tag("keep")));
    }

    #[test]
    fn for_each2_mut_visits_entities_with_both_components() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        let c = world.spawn();
        world.insert(a, Pos(0, 0));
        world.insert(a, Counter(1));
        world.insert(b, Pos(1, 1));
        world.insert(b, Counter(2));
        world.insert(c, Pos(2, 2));

        world.for_each2_mut::<Pos, Counter, _>(|_, pos, counter| {
            pos.0 += 10;
            counter.0 += 100;
        });

        assert_eq!(world.get::<Pos>(a), Some(&Pos(10, 0)));
        assert_eq!(world.get::<Counter>(a), Some(&Counter(101)));
        assert_eq!(world.get::<Pos>(b), Some(&Pos(11, 1)));
        assert_eq!(world.get::<Counter>(b), Some(&Counter(102)));
        assert_eq!(world.get::<Counter>(c), None);
    }

    #[test]
    fn for_each2_mut_returns_false_for_same_type() {
        let mut world = World::new();
        let a = world.spawn();
        world.insert(a, Counter(5));

        let result = world.for_each2_mut::<Counter, Counter, _>(|_, _, _| {});
        assert!(!result);
        assert_eq!(world.get::<Counter>(a), Some(&Counter(5)));
    }

    #[test]
    fn for_each2_mut_returns_false_when_column_missing() {
        let mut world = World::new();
        let a = world.spawn();
        world.insert(a, Pos(0, 0));

        let result = world.for_each2_mut::<Pos, Counter, _>(|_, _, _| {});
        assert!(!result);
    }

    #[test]
    fn get_or_insert_returns_existing_component() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Counter(42));

        let c = world.get_or_insert::<Counter>(e).unwrap();
        assert_eq!(c.0, 42);
        c.0 = 99;
        assert_eq!(world.get::<Counter>(e), Some(&Counter(99)));
    }

    #[test]
    fn get_or_insert_inserts_default_when_missing() {
        let mut world = World::new();
        let e = world.spawn();

        let c = world.get_or_insert::<Counter>(e).unwrap();
        assert_eq!(c.0, 0);
        c.0 = 5;
        assert_eq!(world.get::<Counter>(e), Some(&Counter(5)));
    }

    #[test]
    fn get_or_insert_returns_none_for_stale_entity() {
        let mut world = World::new();
        let e = world.spawn();
        world.despawn(e);

        assert!(world.get_or_insert::<Counter>(e).is_none());
    }

    #[test]
    fn get_or_insert_with_uses_closure() {
        let mut world = World::new();
        let e = world.spawn();

        let c = world.get_or_insert_with(e, || Counter(77)).unwrap();
        assert_eq!(c.0, 77);
    }

    #[test]
    fn entities_iterates_all_live_entities() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        let c = world.spawn();
        world.despawn(b);

        let entities: Vec<Entity> = world.entities().collect();
        assert_eq!(entities.len(), 2);
        assert!(entities.contains(&a));
        assert!(entities.contains(&c));
        assert!(!entities.contains(&b));
    }

    #[test]
    fn entities_is_empty_for_no_entities() {
        let world = World::new();
        assert_eq!(world.entities().count(), 0);
    }

    #[test]
    fn query2_iter_yields_matching_entities() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        let c = world.spawn();
        world.insert(a, Pos(1, 1));
        world.insert(a, Tag("player"));
        world.insert(b, Pos(2, 2));
        world.insert(c, Tag("marker"));

        let count = world.query2_iter::<Pos, Tag>().count();
        assert_eq!(count, 1);

        let results: Vec<_> = world.query2_iter::<Pos, Tag>().collect();
        assert_eq!(results[0].0, a);
        assert_eq!(results[0].1, &Pos(1, 1));
        assert_eq!(results[0].2, &Tag("player"));
    }

    #[test]
    fn query3_iter_yields_matching_entities() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        world.insert(a, Pos(1, 1));
        world.insert(a, Tag("alpha"));
        world.insert(a, Counter(1));
        world.insert(b, Pos(2, 2));
        world.insert(b, Tag("beta"));

        let results: Vec<_> = world.query3_iter::<Pos, Tag, Counter>().collect();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, a);
        assert_eq!(results[0].1, &Pos(1, 1));
        assert_eq!(results[0].2, &Tag("alpha"));
        assert_eq!(results[0].3, &Counter(1));
    }

    #[test]
    fn for_each3_mut_visits_entities_with_all_three_components() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        let c = world.spawn();
        world.insert(a, Pos(0, 0));
        world.insert(a, Tag("alpha"));
        world.insert(a, Counter(1));
        world.insert(b, Pos(1, 1));
        world.insert(b, Tag("beta"));
        world.insert(b, Counter(2));
        world.insert(c, Pos(2, 2));
        world.insert(c, Tag("gamma"));
        // c has no Counter.

        world.for_each3_mut::<Pos, Tag, Counter, _>(|_, pos, tag, counter| {
            pos.0 += 10;
            counter.0 += 100;
            assert!(!tag.0.is_empty());
        });

        assert_eq!(world.get::<Pos>(a), Some(&Pos(10, 0)));
        assert_eq!(world.get::<Counter>(a), Some(&Counter(101)));
        assert_eq!(world.get::<Pos>(b), Some(&Pos(11, 1)));
        assert_eq!(world.get::<Counter>(b), Some(&Counter(102)));
        assert_eq!(world.get::<Counter>(c), None);
    }

    #[test]
    fn for_each3_mut_returns_false_for_duplicate_types() {
        let mut world = World::new();
        let a = world.spawn();
        world.insert(a, Counter(5));

        let result = world.for_each3_mut::<Counter, Counter, Pos, _>(|_, _, _, _| {});
        assert!(!result);
    }

    #[test]
    fn for_each3_mut_returns_false_when_column_missing() {
        let mut world = World::new();
        let a = world.spawn();
        world.insert(a, Pos(0, 0));
        world.insert(a, Tag("test"));

        let result = world.for_each3_mut::<Pos, Tag, Counter, _>(|_, _, _, _| {});
        assert!(!result);
    }

    #[test]
    fn for_each3_mut_returns_true_for_empty_match() {
        let mut world = World::new();
        let a = world.spawn();
        world.insert(a, Pos(0, 0));
        world.insert(a, Tag("test"));
        world.insert(a, Counter(5));

        // All three columns exist, so it returns true even though there are matches.
        let result = world.for_each3_mut::<Pos, Tag, Counter, _>(|_, _, _, _| {});
        assert!(result);
    }

    #[test]
    fn entity_builder_attaches_components() {
        let mut world = World::new();
        let entity = world.builder().with(Pos(1, 2)).with(Tag("player")).build();

        assert!(world.is_alive(entity));
        assert_eq!(world.get::<Pos>(entity), Some(&Pos(1, 2)));
        assert_eq!(world.get::<Tag>(entity), Some(&Tag("player")));
    }

    #[test]
    fn count_with_returns_correct_count() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        let _c = world.spawn();
        world.insert(a, Pos(0, 0));
        world.insert(b, Pos(1, 1));
        world.insert(b, Tag("player"));
        // c has no components.

        assert_eq!(world.count_with::<Pos>(), 2);
        assert_eq!(world.count_with::<Tag>(), 1);
        assert_eq!(world.count_with::<Counter>(), 0);
    }

    #[test]
    fn count_with_excludes_despawned_entities() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        world.insert(a, Pos(0, 0));
        world.insert(b, Pos(1, 1));

        world.despawn(b);
        assert_eq!(world.count_with::<Pos>(), 1);
    }

    #[test]
    fn reserve_entities_preallocates_slots() {
        let mut world = World::new();
        world.reserve_entities(5);
        assert_eq!(world.entity_count(), 0);
        assert_eq!(world.free.len(), 5);

        // Spawning should reuse reserved slots without growing generations.
        let gen_len_before = world.generations.len();
        let e = world.spawn();
        assert!(world.is_alive(e));
        assert_eq!(world.entity_count(), 1);
        assert_eq!(world.generations.len(), gen_len_before);
    }

    #[test]
    fn reserve_entities_with_existing_entities() {
        let mut world = World::new();
        let a = world.spawn();
        world.insert(a, Pos(0, 0));
        world.reserve_entities(3);
        assert_eq!(world.free.len(), 3);
        assert_eq!(world.entity_count(), 1);
    }

    #[test]
    fn shrink_trims_empty_columns() {
        let mut world = World::new();
        let a = world.spawn();
        world.insert(a, Pos(0, 0));
        world.insert(a, Tag("test"));
        world.insert(a, Counter(5));
        world.despawn(a);
        world.shrink();
        assert_eq!(world.entity_count(), 0);
        assert!(world.query::<Pos>().is_empty());
    }

    #[test]
    fn shrink_preserves_live_entities() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        world.insert(a, Pos(0, 0));
        world.insert(b, Pos(1, 1));
        world.shrink();
        assert_eq!(world.entity_count(), 2);
        assert_eq!(world.get::<Pos>(a), Some(&Pos(0, 0)));
        assert_eq!(world.get::<Pos>(b), Some(&Pos(1, 1)));
    }

    #[test]
    fn contains_returns_true_when_component_present() {
        let mut world = World::new();
        assert!(!world.contains::<Pos>());
        let e = world.spawn();
        world.insert(e, Pos(1, 2));
        assert!(world.contains::<Pos>());
        assert!(!world.contains::<Tag>());
    }

    #[test]
    fn contains_returns_false_after_despawn() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Counter(1));
        assert!(world.contains::<Counter>());
        world.despawn(e);
        assert!(!world.contains::<Counter>());
    }

    #[test]
    fn query_exact_size_iterator() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        let c = world.spawn();
        world.insert(a, Pos(1, 1));
        world.insert(b, Pos(2, 2));
        world.insert(c, Pos(3, 3));

        let q = world.query_iter::<Pos>();
        assert_eq!(q.len(), 3);
    }

    #[test]
    fn query2_exact_size_iterator() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        world.insert(a, Pos(1, 1));
        world.insert(a, Tag("x"));
        world.insert(b, Pos(2, 2));
        world.insert(b, Tag("y"));

        let q = world.query2_iter::<Pos, Tag>();
        assert_eq!(q.len(), 2);
    }

    #[test]
    fn query3_exact_size_iterator() {
        let mut world = World::new();
        let a = world.spawn();
        world.insert(a, Pos(1, 1));
        world.insert(a, Tag("x"));
        world.insert(a, Counter(1));

        let q = world.query3_iter::<Pos, Tag, Counter>();
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn spawn_batch_creates_multiple_entities() {
        let mut world = World::new();
        let entities = world.spawn_batch(5, Counter(42));
        assert_eq!(entities.len(), 5);
        assert_eq!(world.entity_count(), 5);
        for e in &entities {
            assert!(world.is_alive(*e));
            assert_eq!(world.get::<Counter>(*e), Some(&Counter(42)));
        }
    }

    #[test]
    fn spawn_batch_returns_distinct_entities() {
        let mut world = World::new();
        let entities = world.spawn_batch(3, Pos(0, 0));
        for i in 0..entities.len() {
            for j in (i + 1)..entities.len() {
                assert_ne!(entities[i], entities[j]);
            }
        }
    }

    #[test]
    fn spawn_batch_zero_is_noop() {
        let mut world = World::new();
        let entities = world.spawn_batch(0, Counter(1));
        assert!(entities.is_empty());
        assert_eq!(world.entity_count(), 0);
    }
}
