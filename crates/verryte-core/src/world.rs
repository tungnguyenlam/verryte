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
use std::collections::hash_map::Entry;
use std::collections::HashMap;

use crate::entity::Entity;

trait Column: Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn clear_index(&mut self, index: usize);
    fn has_index(&self, index: usize) -> bool;
    fn into_any(self: Box<Self>) -> Box<dyn Any + Send + Sync>;
    fn shrink_to_fit(&mut self);
}

struct TypedColumn<T: 'static + Send + Sync> {
    slots: Vec<Option<(u32, T)>>,
    added_ticks: Vec<u64>,
    changed_ticks: Vec<u64>,
}

impl<T: 'static + Send + Sync> TypedColumn<T> {
    fn new() -> Self {
        Self {
            slots: Vec::new(),
            added_ticks: Vec::new(),
            changed_ticks: Vec::new(),
        }
    }

    fn ensure(&mut self, index: usize) {
        if index >= self.slots.len() {
            self.slots.resize_with(index + 1, || None);
            self.added_ticks.resize(index + 1, 0);
            self.changed_ticks.resize(index + 1, 0);
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
            self.added_ticks[index] = 0;
            self.changed_ticks[index] = 0;
        }
    }
    fn has_index(&self, index: usize) -> bool {
        self.slots.get(index).and_then(|s| s.as_ref()).is_some()
    }
    fn into_any(self: Box<Self>) -> Box<dyn Any + Send + Sync> {
        self
    }
    fn shrink_to_fit(&mut self) {
        while self.slots.last().is_none() {
            self.slots.pop();
            self.added_ticks.pop();
            self.changed_ticks.pop();
        }
        self.slots.shrink_to_fit();
        self.added_ticks.shrink_to_fit();
        self.changed_ticks.shrink_to_fit();
    }
}

/// The container that holds entities, their components, and engine resources.
///
/// `World` is the core data storage for Verryte. It uses a sparse-set structure
/// to map entity IDs to component data. It also acts as a type-map for single-instance
/// resources (like `GameClock`, `MessageLog`, or `TacticalMap`).
///
/// # Examples
///
/// **Spawning entities and adding components:**
/// ```
/// use verryte_core::World;
///
/// struct Position { x: i32, y: i32 }
/// struct Health(i32);
///
/// let mut world = World::new();
/// let player = world.spawn();
/// world.insert(player, Position { x: 0, y: 0 });
/// world.insert(player, Health(100));
/// ```
///
/// **Using the Builder pattern:**
/// ```
/// # use verryte_core::World;
/// # struct Position { x: i32, y: i32 }
/// # struct Health(i32);
/// # let mut world = World::new();
/// let enemy = world.builder()
///     .with(Position { x: 10, y: 10 })
///     .with(Health(50))
///     .build();
/// ```
///
/// **Working with Resources:**
/// ```
/// # use verryte_core::World;
/// # let mut world = World::new();
/// struct Config { volume: f32 }
///
/// world.insert_resource(Config { volume: 0.8 });
///
/// // Borrow resource
/// if let Some(config) = world.resource::<Config>() {
///     assert_eq!(config.volume, 0.8);
/// }
///
/// // Mutate resource
/// if let Some(mut config) = world.resource_mut::<Config>() {
///     config.volume = 1.0;
/// }
/// ```
/// **Querying components:**
/// ```
/// # use verryte_core::World;
/// # struct Position { x: i32, y: i32 }
/// # struct Health(i32);
/// # let mut world = World::new();
/// # let player = world.builder().with(Position { x: 0, y: 0 }).with(Health(100)).build();
/// // Iterate over all entities with both Position and Health
/// for (entity, pos, health) in world.query2::<Position, Health>() {
///     println!("Entity {:?} is at ({}, {}) with {} HP", entity, pos.x, pos.y, health.0);
/// }
/// ```
///
/// **Managing Resources:**
/// ```
/// # use verryte_core::World;
/// struct Score(u32);
///
/// # let mut world = World::new();
/// world.insert_resource(Score(0));
///
/// {
///     let mut score = world.resource_mut::<Score>().unwrap();
///     score.0 += 100;
/// }
///
/// assert_eq!(world.resource::<Score>().unwrap().0, 100);
/// ```
pub struct World {
    generations: Vec<u32>,
    alive: Vec<bool>,
    free: Vec<u32>,
    columns: HashMap<TypeId, Box<dyn Column>>,
    column_names: HashMap<TypeId, &'static str>,
    resources: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
    resource_ticks: HashMap<TypeId, u64>,
    change_tick: u64,
}

impl World {
    pub fn new() -> Self {
        Self {
            generations: Vec::new(),
            alive: Vec::new(),
            free: Vec::new(),
            columns: HashMap::new(),
            column_names: HashMap::new(),
            resources: HashMap::new(),
            resource_ticks: HashMap::new(),
            change_tick: 1,
        }
    }

    /// Increments the world's internal change tick and returns the new value.
    ///
    /// The change tick is used for tracking when components or resources are modified.
    pub fn increment_tick(&mut self) -> u64 {
        self.change_tick += 1;
        self.change_tick
    }

    /// Returns the current global change tick.
    pub fn read_tick(&self) -> u64 {
        self.change_tick
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

    /// Force allocate an entity at a specific ID slot with a specific generation.
    /// This is useful for save/load snapshot deserialization.
    /// Any existing entity at this index will be despawned.
    pub fn spawn_at(&mut self, entity: Entity) {
        let idx = entity.index as usize;
        let old_len = self.generations.len();
        if idx >= old_len {
            self.generations.resize(idx + 1, 0);
            self.alive.resize(idx + 1, false);
            for i in old_len..idx {
                self.free.push(i as u32);
            }
        }

        if self.alive[idx] {
            for column in self.columns.values_mut() {
                column.clear_index(idx);
            }
        }

        self.generations[idx] = entity.generation;
        self.alive[idx] = true;
        self.free.retain(|&x| x != entity.index);
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

        // Unlink parent-child relationships first
        if let Some(parent) = self.remove::<Parent>(entity) {
            if let Some(children) = self.get_mut::<Children>(parent.0) {
                children.0.retain(|&c| c != entity);
            }
        }

        if let Some(children) = self.remove::<Children>(entity) {
            for child in children.0 {
                self.remove::<Parent>(child);
            }
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

    /// Despawn every live entity in the world, dropping all their components.
    ///
    /// Returns the number of entities removed. Useful for wiping the game world
    /// between levels or test setups without discarding global resources.
    pub fn despawn_all(&mut self) -> usize {
        let entities: Vec<Entity> = self.entities().collect();
        let count = entities.len();
        for entity in entities {
            self.despawn(entity);
        }
        count
    }

    /// Spawn a new entity with a single component immediately.
    ///
    /// This is an ergonomic alternative to the builder pattern for simple
    /// single-component entities like tags, simple markers, or resources.
    pub fn spawn_with1<A>(&mut self, component: A) -> Entity
    where
        A: 'static + Send + Sync,
    {
        let entity = self.spawn();
        self.insert(entity, component);
        entity
    }

    /// Spawn a new entity with two components immediately.
    ///
    /// Extremely common for basic spatial entities: `spawn_with2(Position, Marker)`.
    pub fn spawn_with2<A, B>(&mut self, comp_a: A, comp_b: B) -> Entity
    where
        A: 'static + Send + Sync,
        B: 'static + Send + Sync,
    {
        let entity = self.spawn();
        self.insert(entity, comp_a);
        self.insert(entity, comp_b);
        entity
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
        let type_id = TypeId::of::<T>();
        self.column_names
            .entry(type_id)
            .or_insert_with(|| std::any::type_name::<T>());

        let column = self
            .columns
            .entry(type_id)
            .or_insert_with(|| Box::new(TypedColumn::<T>::new()));
        let typed = column
            .as_any_mut()
            .downcast_mut::<TypedColumn<T>>()
            .expect("column type matches TypeId");
        let idx = entity.index as usize;
        typed.ensure(idx);
        let prev = typed.slots[idx].take();
        typed.slots[idx] = Some((entity.generation, value));
        typed.added_ticks[idx] = self.change_tick;
        typed.changed_ticks[idx] = self.change_tick;
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
        let idx = entity.index as usize;
        let slot = typed.slots.get_mut(idx)?.as_mut()?;
        if slot.0 == entity.generation {
            typed.changed_ticks[idx] = self.change_tick;
            Some(&mut slot.1)
        } else {
            None
        }
    }

    /// Returns the (added_tick, changed_tick) for a component.
    pub fn component_ticks<T: 'static + Send + Sync>(&self, entity: Entity) -> Option<(u64, u64)> {
        let column = self.columns.get(&TypeId::of::<T>())?;
        let typed = column.as_any().downcast_ref::<TypedColumn<T>>()?;
        let idx = entity.index as usize;
        let slot = typed.slots.get(idx)?.as_ref()?;
        if slot.0 == entity.generation {
            Some((typed.added_ticks[idx], typed.changed_ticks[idx]))
        } else {
            None
        }
    }

    /// Returns `true` if the component was added since `last_tick`.
    pub fn is_added<T: 'static + Send + Sync>(&self, entity: Entity, last_tick: u64) -> bool {
        self.component_ticks::<T>(entity)
            .map(|(added, _)| added > last_tick)
            .unwrap_or(false)
    }

    /// Returns `true` if the component was changed since `last_tick`.
    pub fn is_changed<T: 'static + Send + Sync>(&self, entity: Entity, last_tick: u64) -> bool {
        self.component_ticks::<T>(entity)
            .map(|(_, changed)| changed > last_tick)
            .unwrap_or(false)
    }

    /// Returns a human-readable summary of all entities and their component types.
    pub fn inspect_entities(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("World ({} entities):\n", self.entity_count()));

        for i in 0..self.generations.len() {
            if !self.alive[i] {
                continue;
            }
            let entity = Entity {
                index: i as u32,
                generation: self.generations[i],
            };
            out.push_str(&format!("  Entity {}:", entity));

            let mut components = Vec::new();
            for (type_id, col) in &self.columns {
                if col.has_index(i) {
                    if let Some(name) = self.column_names.get(type_id) {
                        components.push(*name);
                    }
                }
            }
            components.sort();
            for (j, name) in components.iter().enumerate() {
                if j > 0 {
                    out.push(',');
                }
                out.push_str(&format!(" {}", name));
            }
            out.push('\n');
        }
        out
    }

    pub fn get2<A, B>(&self, entity: Entity) -> Option<(&A, &B)>
    where
        A: 'static + Send + Sync,
        B: 'static + Send + Sync,
    {
        let a = self.get::<A>(entity)?;
        let b = self.get::<B>(entity)?;
        Some((a, b))
    }

    pub fn get3<A, B, C>(&self, entity: Entity) -> Option<(&A, &B, &C)>
    where
        A: 'static + Send + Sync,
        B: 'static + Send + Sync,
        C: 'static + Send + Sync,
    {
        let a = self.get::<A>(entity)?;
        let b = self.get::<B>(entity)?;
        let c = self.get::<C>(entity)?;
        Some((a, b, c))
    }

    pub fn with_mut2<A, B, F, R>(&mut self, entity: Entity, f: F) -> Option<R>
    where
        A: 'static + Send + Sync,
        B: 'static + Send + Sync,
        F: FnOnce(&mut A, &mut B) -> R,
    {
        if !self.is_alive(entity) {
            return None;
        }
        let type_a = TypeId::of::<A>();
        let type_b = TypeId::of::<B>();
        if type_a == type_b {
            return None;
        }

        let mut col_a = self.columns.remove(&type_a)?;
        let mut col_b = self.columns.remove(&type_b)?;

        let mut res = None;
        {
            let typed_a = col_a.as_any_mut().downcast_mut::<TypedColumn<A>>();
            let typed_b = col_b.as_any_mut().downcast_mut::<TypedColumn<B>>();
            if let (Some(ta), Some(tb)) = (typed_a, typed_b) {
                let slot_a = ta
                    .slots
                    .get_mut(entity.index as usize)
                    .and_then(|s| s.as_mut());
                let slot_b = tb
                    .slots
                    .get_mut(entity.index as usize)
                    .and_then(|s| s.as_mut());
                if let (Some(sa), Some(sb)) = (slot_a, slot_b) {
                    if sa.0 == entity.generation && sb.0 == entity.generation {
                        res = Some(f(&mut sa.1, &mut sb.1));
                    }
                }
            }
        }

        self.columns.insert(type_a, col_a);
        self.columns.insert(type_b, col_b);
        res
    }

    pub fn with_mut3<A, B, C, F, R>(&mut self, entity: Entity, f: F) -> Option<R>
    where
        A: 'static + Send + Sync,
        B: 'static + Send + Sync,
        C: 'static + Send + Sync,
        F: FnOnce(&mut A, &mut B, &mut C) -> R,
    {
        if !self.is_alive(entity) {
            return None;
        }
        let type_a = TypeId::of::<A>();
        let type_b = TypeId::of::<B>();
        let type_c = TypeId::of::<C>();
        if type_a == type_b || type_a == type_c || type_b == type_c {
            return None;
        }

        let mut col_a = self.columns.remove(&type_a)?;
        let mut col_b = self.columns.remove(&type_b)?;
        let mut col_c = self.columns.remove(&type_c)?;

        let mut res = None;
        {
            let typed_a = col_a.as_any_mut().downcast_mut::<TypedColumn<A>>();
            let typed_b = col_b.as_any_mut().downcast_mut::<TypedColumn<B>>();
            let typed_c = col_c.as_any_mut().downcast_mut::<TypedColumn<C>>();
            if let (Some(ta), Some(tb), Some(tc)) = (typed_a, typed_b, typed_c) {
                let slot_a = ta
                    .slots
                    .get_mut(entity.index as usize)
                    .and_then(|s| s.as_mut());
                let slot_b = tb
                    .slots
                    .get_mut(entity.index as usize)
                    .and_then(|s| s.as_mut());
                let slot_c = tc
                    .slots
                    .get_mut(entity.index as usize)
                    .and_then(|s| s.as_mut());
                if let (Some(sa), Some(sb), Some(sc)) = (slot_a, slot_b, slot_c) {
                    if sa.0 == entity.generation
                        && sb.0 == entity.generation
                        && sc.0 == entity.generation
                    {
                        res = Some(f(&mut sa.1, &mut sb.1, &mut sc.1));
                    }
                }
            }
        }

        self.columns.insert(type_a, col_a);
        self.columns.insert(type_b, col_b);
        self.columns.insert(type_c, col_c);
        res
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
    /// Returns all entities that have a component of type `T`.
    ///
    /// This is a convenience method that collects results into a `Vec`. For
    /// better performance in tight loops, consider using `query_iter`.
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

    /// Query entities with four components, returning an iterator.
    pub fn query4_iter<A, B, C, D>(&self) -> Query4<'_, A, B, C, D>
    where
        A: 'static + Send + Sync,
        B: 'static + Send + Sync,
        C: 'static + Send + Sync,
        D: 'static + Send + Sync,
    {
        Query4 {
            iter: self.query4::<A, B, C, D>().into_iter(),
        }
    }

    /// Query entities with five components, returning an iterator.
    pub fn query5_iter<A, B, C, D, E>(&self) -> Query5<'_, A, B, C, D, E>
    where
        A: 'static + Send + Sync,
        B: 'static + Send + Sync,
        C: 'static + Send + Sync,
        D: 'static + Send + Sync,
        E: 'static + Send + Sync,
    {
        Query5 {
            iter: self.query5::<A, B, C, D, E>().into_iter(),
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

    /// Collect every live entity that has all four component types.
    pub fn query4<A, B, C, D>(&self) -> Vec<(Entity, &A, &B, &C, &D)>
    where
        A: 'static + Send + Sync,
        B: 'static + Send + Sync,
        C: 'static + Send + Sync,
        D: 'static + Send + Sync,
    {
        let mut out = Vec::new();
        if TypeId::of::<A>() == TypeId::of::<B>()
            || TypeId::of::<A>() == TypeId::of::<C>()
            || TypeId::of::<A>() == TypeId::of::<D>()
            || TypeId::of::<B>() == TypeId::of::<C>()
            || TypeId::of::<B>() == TypeId::of::<D>()
            || TypeId::of::<C>() == TypeId::of::<D>()
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
        let Some(column_d) = self.columns.get(&TypeId::of::<D>()) else {
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
        let Some(typed_d) = column_d.as_any().downcast_ref::<TypedColumn<D>>() else {
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
            let Some((gen_d, value_d)) = typed_d.slots.get(i).and_then(|slot| slot.as_ref()) else {
                continue;
            };
            if gen_a != gen_d {
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
                value_d,
            ));
        }
        out
    }

    /// Collect every live entity that has all five component types.
    pub fn query5<A, B, C, D, E>(&self) -> Vec<(Entity, &A, &B, &C, &D, &E)>
    where
        A: 'static + Send + Sync,
        B: 'static + Send + Sync,
        C: 'static + Send + Sync,
        D: 'static + Send + Sync,
        E: 'static + Send + Sync,
    {
        let mut out = Vec::new();
        let id_a = TypeId::of::<A>();
        let id_b = TypeId::of::<B>();
        let id_c = TypeId::of::<C>();
        let id_d = TypeId::of::<D>();
        let id_e = TypeId::of::<E>();
        if id_a == id_b
            || id_a == id_c
            || id_a == id_d
            || id_a == id_e
            || id_b == id_c
            || id_b == id_d
            || id_b == id_e
            || id_c == id_d
            || id_c == id_e
            || id_d == id_e
        {
            return out;
        }

        let Some(column_a) = self.columns.get(&id_a) else {
            return out;
        };
        let Some(column_b) = self.columns.get(&id_b) else {
            return out;
        };
        let Some(column_c) = self.columns.get(&id_c) else {
            return out;
        };
        let Some(column_d) = self.columns.get(&id_d) else {
            return out;
        };
        let Some(column_e) = self.columns.get(&id_e) else {
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
        let Some(typed_d) = column_d.as_any().downcast_ref::<TypedColumn<D>>() else {
            return out;
        };
        let Some(typed_e) = column_e.as_any().downcast_ref::<TypedColumn<E>>() else {
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
            let Some((gen_d, value_d)) = typed_d.slots.get(i).and_then(|slot| slot.as_ref()) else {
                continue;
            };
            if gen_a != gen_d {
                continue;
            }
            let Some((gen_e, value_e)) = typed_e.slots.get(i).and_then(|slot| slot.as_ref()) else {
                continue;
            };
            if gen_a != gen_e {
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
                value_d,
                value_e,
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

    /// Collect every live `(entity, &mut component)` pair for a given
    /// component type. The mutable variant of [`query`](Self::query).
    ///
    /// This is useful when systems need to collect mutable references for
    /// later processing rather than applying changes inside a `for_each_mut`
    /// closure.
    pub fn query_mut<T: 'static + Send + Sync>(&mut self) -> Vec<(Entity, &mut T)> {
        let mut out = Vec::new();
        let Some(column) = self.columns.get_mut(&TypeId::of::<T>()) else {
            return out;
        };
        let Some(typed) = column.as_any_mut().downcast_mut::<TypedColumn<T>>() else {
            return out;
        };
        for (i, slot) in typed.slots.iter_mut().enumerate() {
            if let Some((gen, value)) = slot.as_mut() {
                if i < self.alive.len() && self.alive[i] {
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

    /// Mutably query two component types simultaneously.
    /// Returns a guard that owns the mutably borrowed columns, allowing safe iteration and lookup.
    pub fn query_mut2<A, B>(&mut self) -> Option<QueryMut2Guard<'_, A, B>>
    where
        A: 'static + Send + Sync,
        B: 'static + Send + Sync,
    {
        let id_a = TypeId::of::<A>();
        let id_b = TypeId::of::<B>();
        if id_a == id_b {
            return None;
        }

        let has_a = self.columns.contains_key(&id_a);
        let has_b = self.columns.contains_key(&id_b);
        if !has_a || !has_b {
            return None;
        }

        let col_a = self.columns.get(&id_a).unwrap();
        let col_b = self.columns.get(&id_b).unwrap();
        let typed_a = col_a.as_any().downcast_ref::<TypedColumn<A>>().unwrap();
        let typed_b = col_b.as_any().downcast_ref::<TypedColumn<B>>().unwrap();

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

        let box_a = self.columns.remove(&id_a).unwrap();
        let box_b = self.columns.remove(&id_b).unwrap();

        let any_a = box_a.into_any();
        let any_b = box_b.into_any();
        let typed_a = any_a.downcast::<TypedColumn<A>>().unwrap();
        let typed_b = any_b.downcast::<TypedColumn<B>>().unwrap();

        Some(QueryMut2Guard {
            world: self,
            col_a: Some(typed_a),
            col_b: Some(typed_b),
            indices,
        })
    }

    /// Mutably query three component types simultaneously.
    /// Returns a guard that owns the mutably borrowed columns, allowing safe iteration and lookup.
    pub fn query_mut3<A, B, C>(&mut self) -> Option<QueryMut3Guard<'_, A, B, C>>
    where
        A: 'static + Send + Sync,
        B: 'static + Send + Sync,
        C: 'static + Send + Sync,
    {
        let id_a = TypeId::of::<A>();
        let id_b = TypeId::of::<B>();
        let id_c = TypeId::of::<C>();
        if id_a == id_b || id_a == id_c || id_b == id_c {
            return None;
        }

        let has_a = self.columns.contains_key(&id_a);
        let has_b = self.columns.contains_key(&id_b);
        let has_c = self.columns.contains_key(&id_c);
        if !has_a || !has_b || !has_c {
            return None;
        }

        let col_a = self.columns.get(&id_a).unwrap();
        let col_b = self.columns.get(&id_b).unwrap();
        let col_c = self.columns.get(&id_c).unwrap();
        let typed_a = col_a.as_any().downcast_ref::<TypedColumn<A>>().unwrap();
        let typed_b = col_b.as_any().downcast_ref::<TypedColumn<B>>().unwrap();
        let typed_c = col_c.as_any().downcast_ref::<TypedColumn<C>>().unwrap();

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
                typed_a.slots[i]
                    .as_ref()
                    .and_then(|(ga, _)| {
                        typed_b.slots[i].as_ref().and_then(|(gb, _)| {
                            typed_c.slots[i]
                                .as_ref()
                                .map(|(gc, _)| ga == gb && gb == gc)
                        })
                    })
                    .unwrap_or(false)
            })
            .collect();

        let box_a = self.columns.remove(&id_a).unwrap();
        let box_b = self.columns.remove(&id_b).unwrap();
        let box_c = self.columns.remove(&id_c).unwrap();

        let any_a = box_a.into_any();
        let any_b = box_b.into_any();
        let any_c = box_c.into_any();
        let typed_a = any_a.downcast::<TypedColumn<A>>().unwrap();
        let typed_b = any_b.downcast::<TypedColumn<B>>().unwrap();
        let typed_c = any_c.downcast::<TypedColumn<C>>().unwrap();

        Some(QueryMut3Guard {
            world: self,
            col_a: Some(typed_a),
            col_b: Some(typed_b),
            col_c: Some(typed_c),
            indices,
        })
    }

    /// Mutably query four component types simultaneously.
    /// Returns a guard that owns the mutably borrowed columns, allowing safe iteration and lookup.
    pub fn query_mut4<A, B, C, D>(&mut self) -> Option<QueryMut4Guard<'_, A, B, C, D>>
    where
        A: 'static + Send + Sync,
        B: 'static + Send + Sync,
        C: 'static + Send + Sync,
        D: 'static + Send + Sync,
    {
        let id_a = TypeId::of::<A>();
        let id_b = TypeId::of::<B>();
        let id_c = TypeId::of::<C>();
        let id_d = TypeId::of::<D>();
        if id_a == id_b
            || id_a == id_c
            || id_a == id_d
            || id_b == id_c
            || id_b == id_d
            || id_c == id_d
        {
            return None;
        }

        let has_a = self.columns.contains_key(&id_a);
        let has_b = self.columns.contains_key(&id_b);
        let has_c = self.columns.contains_key(&id_c);
        let has_d = self.columns.contains_key(&id_d);
        if !has_a || !has_b || !has_c || !has_d {
            return None;
        }

        let col_a = self.columns.get(&id_a).unwrap();
        let col_b = self.columns.get(&id_b).unwrap();
        let col_c = self.columns.get(&id_c).unwrap();
        let col_d = self.columns.get(&id_d).unwrap();
        let typed_a = col_a.as_any().downcast_ref::<TypedColumn<A>>().unwrap();
        let typed_b = col_b.as_any().downcast_ref::<TypedColumn<B>>().unwrap();
        let typed_c = col_c.as_any().downcast_ref::<TypedColumn<C>>().unwrap();
        let typed_d = col_d.as_any().downcast_ref::<TypedColumn<D>>().unwrap();

        let alive = &self.alive;
        let len = typed_a
            .slots
            .len()
            .min(typed_b.slots.len())
            .min(typed_c.slots.len())
            .min(typed_d.slots.len())
            .min(alive.len());
        let indices: Vec<usize> = (0..len)
            .filter(|&i| {
                if !alive[i] {
                    return false;
                }
                typed_a.slots[i]
                    .as_ref()
                    .and_then(|(ga, _)| {
                        typed_b.slots[i].as_ref().and_then(|(gb, _)| {
                            typed_c.slots[i].as_ref().and_then(|(gc, _)| {
                                typed_d.slots[i]
                                    .as_ref()
                                    .map(|(gd, _)| ga == gb && gb == gc && gc == gd)
                            })
                        })
                    })
                    .unwrap_or(false)
            })
            .collect();

        let box_a = self.columns.remove(&id_a).unwrap();
        let box_b = self.columns.remove(&id_b).unwrap();
        let box_c = self.columns.remove(&id_c).unwrap();
        let box_d = self.columns.remove(&id_d).unwrap();

        let any_a = box_a.into_any();
        let any_b = box_b.into_any();
        let any_c = box_c.into_any();
        let any_d = box_d.into_any();
        let typed_a = any_a.downcast::<TypedColumn<A>>().unwrap();
        let typed_b = any_b.downcast::<TypedColumn<B>>().unwrap();
        let typed_c = any_c.downcast::<TypedColumn<C>>().unwrap();
        let typed_d = any_d.downcast::<TypedColumn<D>>().unwrap();

        Some(QueryMut4Guard {
            world: self,
            col_a: Some(typed_a),
            col_b: Some(typed_b),
            col_c: Some(typed_c),
            col_d: Some(typed_d),
            indices,
        })
    }

    /// Mutably query five component types simultaneously.
    /// Returns a guard that owns the mutably borrowed columns, allowing safe iteration and lookup.
    pub fn query_mut5<A, B, C, D, E>(&mut self) -> Option<QueryMut5Guard<'_, A, B, C, D, E>>
    where
        A: 'static + Send + Sync,
        B: 'static + Send + Sync,
        C: 'static + Send + Sync,
        D: 'static + Send + Sync,
        E: 'static + Send + Sync,
    {
        let id_a = TypeId::of::<A>();
        let id_b = TypeId::of::<B>();
        let id_c = TypeId::of::<C>();
        let id_d = TypeId::of::<D>();
        let id_e = TypeId::of::<E>();
        if id_a == id_b
            || id_a == id_c
            || id_a == id_d
            || id_a == id_e
            || id_b == id_c
            || id_b == id_d
            || id_b == id_e
            || id_c == id_d
            || id_c == id_e
            || id_d == id_e
        {
            return None;
        }

        let has_a = self.columns.contains_key(&id_a);
        let has_b = self.columns.contains_key(&id_b);
        let has_c = self.columns.contains_key(&id_c);
        let has_d = self.columns.contains_key(&id_d);
        let has_e = self.columns.contains_key(&id_e);
        if !has_a || !has_b || !has_c || !has_d || !has_e {
            return None;
        }

        let col_a = self.columns.get(&id_a).unwrap();
        let col_b = self.columns.get(&id_b).unwrap();
        let col_c = self.columns.get(&id_c).unwrap();
        let col_d = self.columns.get(&id_d).unwrap();
        let col_e = self.columns.get(&id_e).unwrap();
        let typed_a = col_a.as_any().downcast_ref::<TypedColumn<A>>().unwrap();
        let typed_b = col_b.as_any().downcast_ref::<TypedColumn<B>>().unwrap();
        let typed_c = col_c.as_any().downcast_ref::<TypedColumn<C>>().unwrap();
        let typed_d = col_d.as_any().downcast_ref::<TypedColumn<D>>().unwrap();
        let typed_e = col_e.as_any().downcast_ref::<TypedColumn<E>>().unwrap();

        let alive = &self.alive;
        let len = typed_a
            .slots
            .len()
            .min(typed_b.slots.len())
            .min(typed_c.slots.len())
            .min(typed_d.slots.len())
            .min(typed_e.slots.len())
            .min(alive.len());
        let indices: Vec<usize> = (0..len)
            .filter(|&i| {
                if !alive[i] {
                    return false;
                }
                typed_a.slots[i]
                    .as_ref()
                    .and_then(|(ga, _)| {
                        typed_b.slots[i].as_ref().and_then(|(gb, _)| {
                            typed_c.slots[i].as_ref().and_then(|(gc, _)| {
                                typed_d.slots[i].as_ref().and_then(|(gd, _)| {
                                    typed_e.slots[i]
                                        .as_ref()
                                        .map(|(ge, _)| ga == gb && gb == gc && gc == gd && gd == ge)
                                })
                            })
                        })
                    })
                    .unwrap_or(false)
            })
            .collect();

        let box_a = self.columns.remove(&id_a).unwrap();
        let box_b = self.columns.remove(&id_b).unwrap();
        let box_c = self.columns.remove(&id_c).unwrap();
        let box_d = self.columns.remove(&id_d).unwrap();
        let box_e = self.columns.remove(&id_e).unwrap();

        let any_a = box_a.into_any();
        let any_b = box_b.into_any();
        let any_c = box_c.into_any();
        let any_d = box_d.into_any();
        let any_e = box_e.into_any();
        let typed_a = any_a.downcast::<TypedColumn<A>>().unwrap();
        let typed_b = any_b.downcast::<TypedColumn<B>>().unwrap();
        let typed_c = any_c.downcast::<TypedColumn<C>>().unwrap();
        let typed_d = any_d.downcast::<TypedColumn<D>>().unwrap();
        let typed_e = any_e.downcast::<TypedColumn<E>>().unwrap();

        Some(QueryMut5Guard {
            world: self,
            col_a: Some(typed_a),
            col_b: Some(typed_b),
            col_c: Some(typed_c),
            col_d: Some(typed_d),
            col_e: Some(typed_e),
            indices,
        })
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
        let type_id = TypeId::of::<R>();
        let prev = self.resources.insert(type_id, Box::new(resource));
        self.resource_ticks.insert(type_id, self.change_tick);
        prev.and_then(|boxed| boxed.downcast::<R>().ok().map(|b| *b))
    }

    /// Fetch a mutable resource, inserting `Default` if missing.
    pub fn resource_or_insert<R: 'static + Send + Sync + Default>(&mut self) -> &mut R {
        self.resource_or_insert_with(R::default)
    }

    /// Fetch a mutable resource, inserting the closure result if missing.
    pub fn resource_or_insert_with<R, F>(&mut self, f: F) -> &mut R
    where
        R: 'static + Send + Sync,
        F: FnOnce() -> R,
    {
        let type_id = TypeId::of::<R>();
        match self.resources.entry(type_id) {
            Entry::Vacant(entry) => {
                entry.insert(Box::new(f()));
                self.resource_ticks.insert(type_id, self.change_tick);
            }
            Entry::Occupied(_) => {}
        }
        self.resources
            .get_mut(&type_id)
            .and_then(|boxed| boxed.downcast_mut::<R>())
            .expect("resource entry missing or wrong type")
    }

    pub fn resource<R: 'static + Send + Sync>(&self) -> Option<&R> {
        self.resources.get(&TypeId::of::<R>())?.downcast_ref::<R>()
    }

    pub fn has_resource<R: 'static + Send + Sync>(&self) -> bool {
        self.resources.contains_key(&TypeId::of::<R>())
    }

    pub fn resource_mut<R: 'static + Send + Sync>(&mut self) -> Option<&mut R> {
        let type_id = TypeId::of::<R>();
        if self.resources.contains_key(&type_id) {
            self.resource_ticks.insert(type_id, self.change_tick);
        }
        self.resources.get_mut(&type_id)?.downcast_mut::<R>()
    }

    /// Returns the change tick when the resource of type `R` was last modified.
    ///
    /// Returns `None` if the resource does not exist.
    pub fn resource_tick<R: 'static + Send + Sync>(&self) -> Option<u64> {
        self.resource_ticks.get(&TypeId::of::<R>()).copied()
    }

    /// Returns `true` if the resource of type `R` has been modified since `last_tick`.
    ///
    /// If the resource does not exist, returns `false`.
    pub fn is_resource_dirty<R: 'static + Send + Sync>(&self, last_tick: u64) -> bool {
        self.resource_tick::<R>()
            .map(|tick| tick > last_tick)
            .unwrap_or(false)
    }

    /// Get a shared reference to a resource. Panics if the resource is missing.
    ///
    /// Use [`Self::resource`] if the resource might not be present.
    pub fn res<R: 'static + Send + Sync>(&self) -> &R {
        self.resource::<R>()
            .unwrap_or_else(|| panic!("Resource {} missing from world", std::any::type_name::<R>()))
    }

    /// Get a unique reference to a resource. Panics if the resource is missing.
    ///
    /// Use [`Self::resource_mut`] if the resource might not be present.
    pub fn res_mut<R: 'static + Send + Sync>(&mut self) -> &mut R {
        let name = std::any::type_name::<R>();
        self.resource_mut::<R>()
            .unwrap_or_else(|| panic!("Resource {} missing from world", name))
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

/// A guard that holds mutable borrows of two component columns, allowing safe concurrent iteration and mutation.
pub struct QueryMut2Guard<'a, A, B>
where
    A: 'static + Send + Sync,
    B: 'static + Send + Sync,
{
    world: &'a mut World,
    col_a: Option<Box<TypedColumn<A>>>,
    col_b: Option<Box<TypedColumn<B>>>,
    indices: Vec<usize>,
}

impl<'a, A, B> QueryMut2Guard<'a, A, B>
where
    A: 'static + Send + Sync,
    B: 'static + Send + Sync,
{
    pub fn for_each<F>(&mut self, mut f: F)
    where
        F: FnMut(Entity, &mut A, &mut B),
    {
        let col_a = self.col_a.as_mut().unwrap();
        let col_b = self.col_b.as_mut().unwrap();
        for &idx in &self.indices {
            let gen = self.world.generations[idx];
            let entity = Entity {
                index: idx as u32,
                generation: gen,
            };
            let a = &mut col_a.slots[idx].as_mut().unwrap().1;
            let b = &mut col_b.slots[idx].as_mut().unwrap().1;
            f(entity, a, b);
        }
    }

    pub fn get_mut(&mut self, entity: Entity) -> Option<(&mut A, &mut B)> {
        if !self.world.is_alive(entity) {
            return None;
        }
        let idx = entity.index as usize;
        let col_a = self.col_a.as_mut()?;
        let col_b = self.col_b.as_mut()?;
        let slot_a = col_a.slots.get_mut(idx)?.as_mut()?;
        let slot_b = col_b.slots.get_mut(idx)?.as_mut()?;
        if slot_a.0 == entity.generation && slot_b.0 == entity.generation {
            Some((&mut slot_a.1, &mut slot_b.1))
        } else {
            None
        }
    }
}

impl<'a, A, B> Drop for QueryMut2Guard<'a, A, B>
where
    A: 'static + Send + Sync,
    B: 'static + Send + Sync,
{
    fn drop(&mut self) {
        if let Some(col_a) = self.col_a.take() {
            self.world.columns.insert(TypeId::of::<A>(), col_a);
        }
        if let Some(col_b) = self.col_b.take() {
            self.world.columns.insert(TypeId::of::<B>(), col_b);
        }
    }
}

/// A guard that holds mutable borrows of three component columns, allowing safe concurrent iteration and mutation.
pub struct QueryMut3Guard<'a, A, B, C>
where
    A: 'static + Send + Sync,
    B: 'static + Send + Sync,
    C: 'static + Send + Sync,
{
    world: &'a mut World,
    col_a: Option<Box<TypedColumn<A>>>,
    col_b: Option<Box<TypedColumn<B>>>,
    col_c: Option<Box<TypedColumn<C>>>,
    indices: Vec<usize>,
}

impl<'a, A, B, C> QueryMut3Guard<'a, A, B, C>
where
    A: 'static + Send + Sync,
    B: 'static + Send + Sync,
    C: 'static + Send + Sync,
{
    pub fn for_each<F>(&mut self, mut f: F)
    where
        F: FnMut(Entity, &mut A, &mut B, &mut C),
    {
        let col_a = self.col_a.as_mut().unwrap();
        let col_b = self.col_b.as_mut().unwrap();
        let col_c = self.col_c.as_mut().unwrap();
        for &idx in &self.indices {
            let gen = self.world.generations[idx];
            let entity = Entity {
                index: idx as u32,
                generation: gen,
            };
            let a = &mut col_a.slots[idx].as_mut().unwrap().1;
            let b = &mut col_b.slots[idx].as_mut().unwrap().1;
            let c = &mut col_c.slots[idx].as_mut().unwrap().1;
            f(entity, a, b, c);
        }
    }

    pub fn get_mut(&mut self, entity: Entity) -> Option<(&mut A, &mut B, &mut C)> {
        if !self.world.is_alive(entity) {
            return None;
        }
        let idx = entity.index as usize;
        let col_a = self.col_a.as_mut()?;
        let col_b = self.col_b.as_mut()?;
        let col_c = self.col_c.as_mut()?;
        let slot_a = col_a.slots.get_mut(idx)?.as_mut()?;
        let slot_b = col_b.slots.get_mut(idx)?.as_mut()?;
        let slot_c = col_c.slots.get_mut(idx)?.as_mut()?;
        if slot_a.0 == entity.generation
            && slot_b.0 == entity.generation
            && slot_c.0 == entity.generation
        {
            Some((&mut slot_a.1, &mut slot_b.1, &mut slot_c.1))
        } else {
            None
        }
    }
}

impl<'a, A, B, C> Drop for QueryMut3Guard<'a, A, B, C>
where
    A: 'static + Send + Sync,
    B: 'static + Send + Sync,
    C: 'static + Send + Sync,
{
    fn drop(&mut self) {
        if let Some(col_a) = self.col_a.take() {
            self.world.columns.insert(TypeId::of::<A>(), col_a);
        }
        if let Some(col_b) = self.col_b.take() {
            self.world.columns.insert(TypeId::of::<B>(), col_b);
        }
        if let Some(col_c) = self.col_c.take() {
            self.world.columns.insert(TypeId::of::<C>(), col_c);
        }
    }
}

/// An iterator over four-component query results.
pub struct Query4<'a, A, B, C, D> {
    iter: std::vec::IntoIter<(Entity, &'a A, &'a B, &'a C, &'a D)>,
}

impl<'a, A, B, C, D> Iterator for Query4<'a, A, B, C, D> {
    type Item = (Entity, &'a A, &'a B, &'a C, &'a D);
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a, A, B, C, D> ExactSizeIterator for Query4<'a, A, B, C, D> {}

/// A guard that holds mutable borrows of four component columns, allowing safe concurrent iteration and mutation.
pub struct QueryMut4Guard<'a, A, B, C, D>
where
    A: 'static + Send + Sync,
    B: 'static + Send + Sync,
    C: 'static + Send + Sync,
    D: 'static + Send + Sync,
{
    world: &'a mut World,
    col_a: Option<Box<TypedColumn<A>>>,
    col_b: Option<Box<TypedColumn<B>>>,
    col_c: Option<Box<TypedColumn<C>>>,
    col_d: Option<Box<TypedColumn<D>>>,
    indices: Vec<usize>,
}

impl<'a, A, B, C, D> QueryMut4Guard<'a, A, B, C, D>
where
    A: 'static + Send + Sync,
    B: 'static + Send + Sync,
    C: 'static + Send + Sync,
    D: 'static + Send + Sync,
{
    pub fn for_each<F>(&mut self, mut f: F)
    where
        F: FnMut(Entity, &mut A, &mut B, &mut C, &mut D),
    {
        let col_a = self.col_a.as_mut().unwrap();
        let col_b = self.col_b.as_mut().unwrap();
        let col_c = self.col_c.as_mut().unwrap();
        let col_d = self.col_d.as_mut().unwrap();
        for &idx in &self.indices {
            let gen = self.world.generations[idx];
            let entity = Entity {
                index: idx as u32,
                generation: gen,
            };
            let a = &mut col_a.slots[idx].as_mut().unwrap().1;
            let b = &mut col_b.slots[idx].as_mut().unwrap().1;
            let c = &mut col_c.slots[idx].as_mut().unwrap().1;
            let d = &mut col_d.slots[idx].as_mut().unwrap().1;
            f(entity, a, b, c, d);
        }
    }

    pub fn get_mut(&mut self, entity: Entity) -> Option<(&mut A, &mut B, &mut C, &mut D)> {
        if !self.world.is_alive(entity) {
            return None;
        }
        let idx = entity.index as usize;
        let col_a = self.col_a.as_mut()?;
        let col_b = self.col_b.as_mut()?;
        let col_c = self.col_c.as_mut()?;
        let col_d = self.col_d.as_mut()?;
        let slot_a = col_a.slots.get_mut(idx)?.as_mut()?;
        let slot_b = col_b.slots.get_mut(idx)?.as_mut()?;
        let slot_c = col_c.slots.get_mut(idx)?.as_mut()?;
        let slot_d = col_d.slots.get_mut(idx)?.as_mut()?;
        if slot_a.0 == entity.generation
            && slot_b.0 == entity.generation
            && slot_c.0 == entity.generation
            && slot_d.0 == entity.generation
        {
            Some((&mut slot_a.1, &mut slot_b.1, &mut slot_c.1, &mut slot_d.1))
        } else {
            None
        }
    }
}

impl<'a, A, B, C, D> Drop for QueryMut4Guard<'a, A, B, C, D>
where
    A: 'static + Send + Sync,
    B: 'static + Send + Sync,
    C: 'static + Send + Sync,
    D: 'static + Send + Sync,
{
    fn drop(&mut self) {
        if let Some(col_a) = self.col_a.take() {
            self.world.columns.insert(TypeId::of::<A>(), col_a);
        }
        if let Some(col_b) = self.col_b.take() {
            self.world.columns.insert(TypeId::of::<B>(), col_b);
        }
        if let Some(col_c) = self.col_c.take() {
            self.world.columns.insert(TypeId::of::<C>(), col_c);
        }
        if let Some(col_d) = self.col_d.take() {
            self.world.columns.insert(TypeId::of::<D>(), col_d);
        }
    }
}

/// An iterator over five-component query results.
pub struct Query5<'a, A, B, C, D, E> {
    iter: std::vec::IntoIter<(Entity, &'a A, &'a B, &'a C, &'a D, &'a E)>,
}

impl<'a, A, B, C, D, E> Iterator for Query5<'a, A, B, C, D, E> {
    type Item = (Entity, &'a A, &'a B, &'a C, &'a D, &'a E);
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a, A, B, C, D, E> ExactSizeIterator for Query5<'a, A, B, C, D, E> {}

/// A guard that holds mutable borrows of five component columns, allowing safe concurrent iteration and mutation.
pub struct QueryMut5Guard<'a, A, B, C, D, E>
where
    A: 'static + Send + Sync,
    B: 'static + Send + Sync,
    C: 'static + Send + Sync,
    D: 'static + Send + Sync,
    E: 'static + Send + Sync,
{
    world: &'a mut World,
    col_a: Option<Box<TypedColumn<A>>>,
    col_b: Option<Box<TypedColumn<B>>>,
    col_c: Option<Box<TypedColumn<C>>>,
    col_d: Option<Box<TypedColumn<D>>>,
    col_e: Option<Box<TypedColumn<E>>>,
    indices: Vec<usize>,
}

impl<'a, A, B, C, D, E> QueryMut5Guard<'a, A, B, C, D, E>
where
    A: 'static + Send + Sync,
    B: 'static + Send + Sync,
    C: 'static + Send + Sync,
    D: 'static + Send + Sync,
    E: 'static + Send + Sync,
{
    pub fn for_each<F>(&mut self, mut f: F)
    where
        F: FnMut(Entity, &mut A, &mut B, &mut C, &mut D, &mut E),
    {
        let col_a = self.col_a.as_mut().unwrap();
        let col_b = self.col_b.as_mut().unwrap();
        let col_c = self.col_c.as_mut().unwrap();
        let col_d = self.col_d.as_mut().unwrap();
        let col_e = self.col_e.as_mut().unwrap();
        for &idx in &self.indices {
            let gen = self.world.generations[idx];
            let entity = Entity {
                index: idx as u32,
                generation: gen,
            };
            let a = &mut col_a.slots[idx].as_mut().unwrap().1;
            let b = &mut col_b.slots[idx].as_mut().unwrap().1;
            let c = &mut col_c.slots[idx].as_mut().unwrap().1;
            let d = &mut col_d.slots[idx].as_mut().unwrap().1;
            let e = &mut col_e.slots[idx].as_mut().unwrap().1;
            f(entity, a, b, c, d, e);
        }
    }

    pub fn get_mut(&mut self, entity: Entity) -> Option<(&mut A, &mut B, &mut C, &mut D, &mut E)> {
        if !self.world.is_alive(entity) {
            return None;
        }
        let idx = entity.index as usize;
        let col_a = self.col_a.as_mut()?;
        let col_b = self.col_b.as_mut()?;
        let col_c = self.col_c.as_mut()?;
        let col_d = self.col_d.as_mut()?;
        let col_e = self.col_e.as_mut()?;
        let slot_a = col_a.slots.get_mut(idx)?.as_mut()?;
        let slot_b = col_b.slots.get_mut(idx)?.as_mut()?;
        let slot_c = col_c.slots.get_mut(idx)?.as_mut()?;
        let slot_d = col_d.slots.get_mut(idx)?.as_mut()?;
        let slot_e = col_e.slots.get_mut(idx)?.as_mut()?;
        if slot_a.0 == entity.generation
            && slot_b.0 == entity.generation
            && slot_c.0 == entity.generation
            && slot_d.0 == entity.generation
            && slot_e.0 == entity.generation
        {
            Some((
                &mut slot_a.1,
                &mut slot_b.1,
                &mut slot_c.1,
                &mut slot_d.1,
                &mut slot_e.1,
            ))
        } else {
            None
        }
    }
}

impl<'a, A, B, C, D, E> Drop for QueryMut5Guard<'a, A, B, C, D, E>
where
    A: 'static + Send + Sync,
    B: 'static + Send + Sync,
    C: 'static + Send + Sync,
    D: 'static + Send + Sync,
    E: 'static + Send + Sync,
{
    fn drop(&mut self) {
        if let Some(col_a) = self.col_a.take() {
            self.world.columns.insert(TypeId::of::<A>(), col_a);
        }
        if let Some(col_b) = self.col_b.take() {
            self.world.columns.insert(TypeId::of::<B>(), col_b);
        }
        if let Some(col_c) = self.col_c.take() {
            self.world.columns.insert(TypeId::of::<C>(), col_c);
        }
        if let Some(col_d) = self.col_d.take() {
            self.world.columns.insert(TypeId::of::<D>(), col_d);
        }
        if let Some(col_e) = self.col_e.take() {
            self.world.columns.insert(TypeId::of::<E>(), col_e);
        }
    }
}

/// Component identifying the parent of an entity in a hierarchy.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Parent(pub Entity);

/// Component identifying the children of an entity in a hierarchy.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Children(pub Vec<Entity>);

impl World {
    /// Sets the parent of a child entity. If the child already had a parent, it is
    /// removed from the old parent's child list first.
    pub fn set_parent(&mut self, child: Entity, parent: Entity) {
        if !self.is_alive(child) || !self.is_alive(parent) {
            return;
        }

        // If the child already has a parent, remove it first
        if let Some(old_parent) = self.get::<Parent>(child).map(|p| p.0) {
            if old_parent == parent {
                return; // Already the parent
            }
            if let Some(children) = self.get_mut::<Children>(old_parent) {
                children.0.retain(|&c| c != child);
            }
        }

        // Set the new parent
        self.insert(child, Parent(parent));

        // Add to the new parent's children list
        if let Some(children) = self.get_mut::<Children>(parent) {
            if !children.0.contains(&child) {
                children.0.push(child);
            }
        } else {
            self.insert(parent, Children(vec![child]));
        }
    }

    /// Removes the parent link from a child entity, also removing it from the parent's children list.
    pub fn remove_parent(&mut self, child: Entity) {
        if !self.is_alive(child) {
            return;
        }

        if let Some(parent) = self.remove::<Parent>(child) {
            if let Some(children) = self.get_mut::<Children>(parent.0) {
                children.0.retain(|&c| c != child);
            }
        }
    }

    /// Despawns an entity and all of its descendants recursively.
    /// Returns the total number of entities despawned.
    pub fn despawn_recursive(&mut self, entity: Entity) -> usize {
        if !self.is_alive(entity) {
            return 0;
        }

        // First remove from parent to prevent dangling reference in parent
        self.remove_parent(entity);

        let mut count = 0;
        let mut to_despawn = vec![entity];
        let mut idx = 0;

        // BFS/DFS to collect all descendants
        while idx < to_despawn.len() {
            let current = to_despawn[idx];
            idx += 1;

            if let Some(children) = self.get::<Children>(current) {
                for &child in &children.0 {
                    if self.is_alive(child) && !to_despawn.contains(&child) {
                        to_despawn.push(child);
                    }
                }
            }
        }

        // Despawn all collected entities
        for ent in to_despawn {
            if self.despawn(ent) {
                count += 1;
            }
        }

        count
    }

    /// Spawn an entity with a specific string tag.
    pub fn spawn_with_tag(&mut self, name: &str) -> Entity {
        let e = self.spawn();
        self.insert(e, crate::tag::Tag::new(name));
        e
    }

    /// Check if a specific entity has the given tag.
    pub fn has_tag(&self, entity: Entity, name: &str) -> bool {
        self.get::<crate::tag::Tag>(entity)
            .is_some_and(|t| t.is(name))
    }

    /// Retrieve all entities that have a Tag matching the given name.
    pub fn find_entities_with_tag(&self, name: &str) -> Vec<Entity> {
        let mut out = Vec::new();
        for (e, tag) in self.query::<crate::tag::Tag>() {
            if tag.is(name) {
                out.push(e);
            }
        }
        out
    }

    /// Despawns all entities that have a Tag matching the given name.
    pub fn despawn_all_with_tag(&mut self, name: &str) -> usize {
        let targets = self.find_entities_with_tag(name);
        let count = targets.len();
        for e in targets {
            self.despawn(e);
        }
        count
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

    #[derive(Debug, PartialEq, Clone)]
    struct Pos(i32, i32);

    #[derive(Debug, PartialEq)]
    struct Tag(&'static str);

    #[derive(Debug, PartialEq, Clone, Default)]
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
    fn resource_or_insert_creates_default_once() {
        let mut world = World::new();
        let counter = world.resource_or_insert::<Counter>();
        assert_eq!(counter, &Counter(0));
        counter.0 = 7;
        assert_eq!(world.resource::<Counter>(), Some(&Counter(7)));
    }

    #[test]
    fn resource_or_insert_with_keeps_existing_resource() {
        use std::cell::Cell;

        let mut world = World::new();
        world.insert_resource(Counter(5));
        let called = Cell::new(false);
        let counter = world.resource_or_insert_with(|| {
            called.set(true);
            Counter(99)
        });
        assert!(!called.get());
        assert_eq!(counter, &Counter(5));
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

    #[test]
    fn entity_display() {
        let e = Entity {
            index: 3,
            generation: 2,
        };
        assert_eq!(format!("{}", e), "3#2");
    }

    #[test]
    fn entity_is_invalid() {
        assert!(Entity::INVALID.is_invalid());
        let e = Entity {
            index: 0,
            generation: 1,
        };
        assert!(!e.is_invalid());
    }

    #[test]
    fn query_mut_returns_mutable_refs() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        world.insert(a, Counter(10));
        world.insert(b, Counter(20));

        for (_, c) in world.query_mut::<Counter>() {
            c.0 *= 2;
        }

        assert_eq!(world.get::<Counter>(a).unwrap().0, 20);
        assert_eq!(world.get::<Counter>(b).unwrap().0, 40);
    }

    #[test]
    fn query_mut_excludes_dead_entities() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        world.insert(a, Counter(1));
        world.insert(b, Counter(2));
        world.despawn(b);

        let results = world.query_mut::<Counter>();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1 .0, 1);
    }

    #[test]
    fn query_mut_empty_when_no_column() {
        let mut world = World::new();
        world.spawn();
        assert!(world.query_mut::<Counter>().is_empty());
    }

    #[test]
    fn world_despawn_all() {
        let mut world = World::new();
        world.spawn_with1(Counter(10));
        world.spawn_with2(Counter(20), Pos(1, 2));
        assert_eq!(world.entity_count(), 2);

        let removed = world.despawn_all();
        assert_eq!(removed, 2);
        assert_eq!(world.entity_count(), 0);
    }

    #[test]
    fn world_clear_resets_everything() {
        let mut world = World::new();
        world.spawn_with1(Counter(10));
        world.insert_resource(Pos(3, 4));
        assert_eq!(world.entity_count(), 1);
        assert!(world.resource::<Pos>().is_some());

        world.clear();
        assert_eq!(world.entity_count(), 0);
        assert!(world.resource::<Pos>().is_none());
    }

    #[test]
    fn world_spawn_with_helpers() {
        let mut world = World::new();
        let e1 = world.spawn_with1(Counter(100));
        let e2 = world.spawn_with2(Counter(200), Pos(5, 6));

        assert_eq!(world.get::<Counter>(e1), Some(&Counter(100)));
        assert_eq!(world.get::<Counter>(e2), Some(&Counter(200)));
        assert_eq!(world.get::<Pos>(e2), Some(&Pos(5, 6)));
    }

    #[test]
    fn world_tuple_retrieval_helpers() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Pos(10, 20));
        world.insert(e, Tag("hello"));
        world.insert(e, Counter(42));

        // Test get2 and get3
        let (pos, tag) = world.get2::<Pos, Tag>(e).unwrap();
        assert_eq!(pos.0, 10);
        assert_eq!(tag.0, "hello");

        let (pos, tag, counter) = world.get3::<Pos, Tag, Counter>(e).unwrap();
        assert_eq!(pos.0, 10);
        assert_eq!(tag.0, "hello");
        assert_eq!(counter.0, 42);

        // Test with_mut2
        let result = world
            .with_mut2::<Pos, Tag, _, i32>(e, |p, t| {
                p.0 += 5;
                t.0 = "world";
                99
            })
            .unwrap();
        assert_eq!(result, 99);
        assert_eq!(world.get::<Pos>(e).unwrap().0, 15);
        assert_eq!(world.get::<Tag>(e).unwrap().0, "world");

        // Test with_mut3
        world
            .with_mut3::<Pos, Tag, Counter, _, ()>(e, |p, t, c| {
                p.0 += 5;
                t.0 = "test";
                c.0 += 10;
            })
            .unwrap();
        assert_eq!(world.get::<Pos>(e).unwrap().0, 20);
        assert_eq!(world.get::<Tag>(e).unwrap().0, "test");
        assert_eq!(world.get::<Counter>(e).unwrap().0, 52);
    }

    #[test]
    fn query_mut2_and_mut3_guards() {
        let mut world = World::new();
        let e1 = world.spawn();
        world.insert(e1, Pos(1, 2));
        world.insert(e1, Tag("e1"));
        world.insert(e1, Counter(10));

        let e2 = world.spawn();
        world.insert(e2, Pos(3, 4));
        world.insert(e2, Tag("e2"));

        // QueryMut2Guard
        {
            let mut guard = world.query_mut2::<Pos, Tag>().unwrap();
            guard.for_each(|entity, pos, tag| {
                if entity == e1 {
                    assert_eq!(pos.0, 1);
                    assert_eq!(tag.0, "e1");
                    pos.0 += 10;
                    tag.0 = "e1_mod";
                } else if entity == e2 {
                    assert_eq!(pos.0, 3);
                    assert_eq!(tag.0, "e2");
                    pos.0 += 20;
                    tag.0 = "e2_mod";
                }
            });

            let (p1, _t1) = guard.get_mut(e1).unwrap();
            assert_eq!(p1.0, 11);
            p1.0 += 100;
        }

        // Verify values are updated and columns are restored in the world
        assert_eq!(world.get::<Pos>(e1).unwrap().0, 111);
        assert_eq!(world.get::<Tag>(e1).unwrap().0, "e1_mod");
        assert_eq!(world.get::<Pos>(e2).unwrap().0, 23);
        assert_eq!(world.get::<Tag>(e2).unwrap().0, "e2_mod");

        // QueryMut3Guard
        {
            let mut guard = world.query_mut3::<Pos, Tag, Counter>().unwrap();
            guard.for_each(|entity, pos, tag, counter| {
                assert_eq!(entity, e1);
                assert_eq!(pos.0, 111);
                assert_eq!(tag.0, "e1_mod");
                assert_eq!(counter.0, 10);
                pos.0 += 1000;
                tag.0 = "e1_final";
                counter.0 += 90;
            });

            let (p, t, c) = guard.get_mut(e1).unwrap();
            assert_eq!(p.0, 1111);
            assert_eq!(t.0, "e1_final");
            assert_eq!(c.0, 100);
            p.0 += 5;
        }

        // Verify values are updated and columns are restored in the world
        assert_eq!(world.get::<Pos>(e1).unwrap().0, 1116);
        assert_eq!(world.get::<Tag>(e1).unwrap().0, "e1_final");
        assert_eq!(world.get::<Counter>(e1).unwrap().0, 100);
    }

    #[test]
    fn query4_and_query_mut4_guards() {
        #[derive(Debug, PartialEq, Eq)]
        struct Extra(i32);
        let mut world = World::new();
        let e1 = world.spawn();
        world.insert(e1, Pos(1, 2));
        world.insert(e1, Tag("e1"));
        world.insert(e1, Counter(10));
        world.insert(e1, Extra(42));

        // Test query4_iter
        let q = world
            .query4_iter::<Pos, Tag, Counter, Extra>()
            .collect::<Vec<_>>();
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].1 .0, 1);
        assert_eq!(q[0].2 .0, "e1");
        assert_eq!(q[0].3 .0, 10);
        assert_eq!(q[0].4 .0, 42);

        // Test QueryMut4Guard
        {
            let mut guard = world.query_mut4::<Pos, Tag, Counter, Extra>().unwrap();
            guard.for_each(|entity, pos, tag, counter, extra| {
                assert_eq!(entity, e1);
                pos.0 += 100;
                tag.0 = "e1_mutated";
                counter.0 += 5;
                extra.0 += 8;
            });

            let (p, t, c, ex) = guard.get_mut(e1).unwrap();
            assert_eq!(p.0, 101);
            assert_eq!(t.0, "e1_mutated");
            assert_eq!(c.0, 15);
            assert_eq!(ex.0, 50);
            p.0 += 10;
        }

        // Verify values are updated and columns are restored
        assert_eq!(world.get::<Pos>(e1).unwrap().0, 111);
        assert_eq!(world.get::<Tag>(e1).unwrap().0, "e1_mutated");
        assert_eq!(world.get::<Counter>(e1).unwrap().0, 15);
        assert_eq!(world.get::<Extra>(e1).unwrap().0, 50);
    }

    #[test]
    fn query5_and_query_mut5_guards() {
        #[derive(Debug, PartialEq, Eq)]
        struct Extra(i32);
        #[derive(Debug, PartialEq, Eq)]
        struct Flag;
        let mut world = World::new();
        let e1 = world.spawn();
        world.insert(e1, Pos(1, 2));
        world.insert(e1, Tag("e1"));
        world.insert(e1, Counter(10));
        world.insert(e1, Extra(42));
        world.insert(e1, Flag);

        // Test query5_iter
        let q = world
            .query5_iter::<Pos, Tag, Counter, Extra, Flag>()
            .collect::<Vec<_>>();
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].1 .0, 1);
        assert_eq!(q[0].2 .0, "e1");
        assert_eq!(q[0].3 .0, 10);
        assert_eq!(q[0].4 .0, 42);
        assert_eq!(q[0].5, &Flag);

        // Test QueryMut5Guard
        {
            let mut guard = world
                .query_mut5::<Pos, Tag, Counter, Extra, Flag>()
                .unwrap();
            guard.for_each(|entity, pos, tag, counter, extra, _flag| {
                assert_eq!(entity, e1);
                pos.0 += 100;
                tag.0 = "e1_mutated5";
                counter.0 += 5;
                extra.0 += 8;
            });

            let (p, t, c, ex, _fl) = guard.get_mut(e1).unwrap();
            assert_eq!(p.0, 101);
            assert_eq!(t.0, "e1_mutated5");
            assert_eq!(c.0, 15);
            assert_eq!(ex.0, 50);
            p.0 += 10;
        }

        // Verify values are updated and columns are restored
        assert_eq!(world.get::<Pos>(e1).unwrap().0, 111);
        assert_eq!(world.get::<Tag>(e1).unwrap().0, "e1_mutated5");
        assert_eq!(world.get::<Counter>(e1).unwrap().0, 15);
        assert_eq!(world.get::<Extra>(e1).unwrap().0, 50);
        assert!(world.get::<Flag>(e1).is_some());
    }

    #[test]
    fn test_parent_child_hierarchy() {
        let mut world = World::new();
        let parent = world.spawn();
        let child1 = world.spawn();
        let child2 = world.spawn();

        world.set_parent(child1, parent);
        world.set_parent(child2, parent);

        assert_eq!(world.get::<Parent>(child1).unwrap().0, parent);
        assert_eq!(world.get::<Parent>(child2).unwrap().0, parent);

        let children = world.get::<Children>(parent).unwrap();
        assert_eq!(children.0.len(), 2);
        assert!(children.0.contains(&child1));
        assert!(children.0.contains(&child2));

        // Test remove_parent
        world.remove_parent(child1);
        assert!(world.get::<Parent>(child1).is_none());
        let children = world.get::<Children>(parent).unwrap();
        assert_eq!(children.0.len(), 1);
        assert!(!children.0.contains(&child1));
        assert!(children.0.contains(&child2));

        // Re-add and test recursive despawn
        world.set_parent(child1, parent);
        let sub_child = world.spawn();
        world.set_parent(sub_child, child1);

        assert_eq!(world.despawn_recursive(parent), 4); // parent, child1, child2, sub_child
        assert!(!world.is_alive(parent));
        assert!(!world.is_alive(child1));
        assert!(!world.is_alive(child2));
        assert!(!world.is_alive(sub_child));
    }
}
