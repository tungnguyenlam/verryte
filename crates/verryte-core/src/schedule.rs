//! A minimal ordered system list.
//!
//! `Schedule` is intentionally small: it holds `fn(&mut World)` pointers and
//! runs them in order. This is enough for a turn-based game loop where a tick
//! is "translate input -> apply game systems -> snapshot state". Stages,
//! parallelism, and richer run conditions can grow on top of this once a real
//! game pulls on the API.

use crate::world::World;

pub type System = fn(&mut World);

/// A predicate that gates whether a system should run this tick.
///
/// The condition receives a shared reference to the world so it can inspect
/// resources and component state without mutating anything.
pub type RunCondition = fn(&World) -> bool;

/// A system with an optional name and optional run condition.
pub struct NamedSystem {
    pub name: &'static str,
    pub func: System,
    pub condition: Option<RunCondition>,
}

impl NamedSystem {
    pub fn new(name: &'static str, func: System) -> Self {
        Self {
            name,
            func,
            condition: None,
        }
    }

    /// Create a named system where the name is derived from the function
    /// pointer's debug representation.
    pub fn auto(func: System) -> Self {
        Self {
            name: "<unnamed>",
            func,
            condition: None,
        }
    }

    /// Create a system that only runs when the condition returns `true`.
    pub fn conditional(name: &'static str, func: System, condition: RunCondition) -> Self {
        Self {
            name,
            func,
            condition: Some(condition),
        }
    }
}

pub struct Schedule {
    systems: Vec<NamedSystem>,
}

impl Schedule {
    pub fn new() -> Self {
        Self {
            systems: Vec::new(),
        }
    }

    /// Add an unnamed system. The system will appear as `<unnamed>` in logs.
    pub fn add(&mut self, system: System) -> &mut Self {
        self.systems.push(NamedSystem::auto(system));
        self
    }

    /// Add a system with a name for debugging and logging.
    pub fn add_named(&mut self, name: &'static str, system: System) -> &mut Self {
        self.systems.push(NamedSystem::new(name, system));
        self
    }

    /// Add a system that only runs when `condition` returns `true`.
    ///
    /// Useful for debug systems, toggleable features, or systems that should
    /// only run when a specific resource flag is set.
    pub fn add_conditional(
        &mut self,
        name: &'static str,
        system: System,
        condition: RunCondition,
    ) -> &mut Self {
        self.systems
            .push(NamedSystem::conditional(name, system, condition));
        self
    }

    /// Returns the name and function of each system in order.
    pub fn systems(&self) -> &[NamedSystem] {
        &self.systems
    }

    pub fn len(&self) -> usize {
        self.systems.len()
    }

    pub fn is_empty(&self) -> bool {
        self.systems.is_empty()
    }

    /// Run all systems in order, skipping those whose condition is not met.
    pub fn run(&self, world: &mut World) {
        for system in &self.systems {
            if let Some(cond) = system.condition {
                if !cond(world) {
                    continue;
                }
            }
            (system.func)(world);
        }
    }

    /// Run all systems in order, calling `on_system` with each system's name
    /// before it executes. Systems whose condition is not met are skipped
    /// without calling the hook.
    pub fn run_with_hook<F>(&self, world: &mut World, mut on_system: F)
    where
        F: FnMut(&str),
    {
        for system in &self.systems {
            if let Some(cond) = system.condition {
                if !cond(world) {
                    continue;
                }
            }
            on_system(system.name);
            (system.func)(world);
        }
    }

    /// Remove all systems from the schedule.
    pub fn clear(&mut self) {
        self.systems.clear();
    }

    /// Remove the first system with the given name. Returns `true` if a system
    /// was found and removed.
    ///
    /// Useful for hot-reloading systems or toggling debug/profiling systems
    /// at runtime.
    pub fn remove_by_name(&mut self, name: &str) -> bool {
        if let Some(pos) = self.systems.iter().position(|s| s.name == name) {
            self.systems.remove(pos);
            true
        } else {
            false
        }
    }

    /// Insert a system at a specific position, shifting existing systems at
    /// and after that position to the right.
    ///
    /// Panics if `index > self.len()`. Use `add` or `add_named` for appending
    /// to the end.
    pub fn insert_at(&mut self, index: usize, name: &'static str, system: System) {
        self.systems.insert(index, NamedSystem::new(name, system));
    }

    /// Replace the first system with the given name, returning `true` if found.
    ///
    /// The replacement keeps the same position in the schedule, preserving
    /// execution order relative to other systems.
    pub fn replace_by_name(&mut self, name: &str, new_name: &'static str, new_system: System) -> bool {
        if let Some(pos) = self.systems.iter().position(|s| s.name == name) {
            self.systems[pos] = NamedSystem::new(new_name, new_system);
            true
        } else {
            false
        }
    }

    /// Run the first system with the given name, if it exists.
    ///
    /// Returns `true` if a system was found and executed. Systems with
    /// unmet conditions are skipped and return `false`.
    ///
    /// Useful for debugging individual systems, triggering specific behavior
    /// on demand, or running systems outside the normal schedule order.
    pub fn run_system_by_name(&self, name: &str, world: &mut World) -> bool {
        for system in &self.systems {
            if system.name == name {
                if let Some(cond) = system.condition {
                    if !cond(world) {
                        return false;
                    }
                }
                (system.func)(world);
                return true;
            }
        }
        false
    }
}

impl Default for Schedule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Counter(u32);

    #[derive(Default)]
    struct DebugMode(bool);

    fn bump(world: &mut World) {
        let counter = world.resource_mut::<Counter>().expect("counter resource");
        counter.0 += 1;
    }

    fn double(world: &mut World) {
        let counter = world.resource_mut::<Counter>().expect("counter resource");
        counter.0 *= 2;
    }

    fn debug_bump(world: &mut World) {
        let counter = world.resource_mut::<Counter>().expect("counter resource");
        counter.0 += 100;
    }

    #[test]
    fn schedule_runs_systems_in_order() {
        let mut world = World::new();
        world.insert_resource(Counter(1));
        let mut schedule = Schedule::new();
        schedule.add(bump).add(double); // (1 + 1) * 2 = 4
        schedule.run(&mut world);
        assert_eq!(world.resource::<Counter>().unwrap().0, 4);
        schedule.run(&mut world); // (4 + 1) * 2 = 10
        assert_eq!(world.resource::<Counter>().unwrap().0, 10);
    }

    #[test]
    fn named_systems_track_their_names() {
        let mut schedule = Schedule::new();
        schedule.add_named("bump", bump);
        schedule.add_named("double", double);
        schedule.add(bump);

        let systems = schedule.systems();
        assert_eq!(systems[0].name, "bump");
        assert_eq!(systems[1].name, "double");
        assert_eq!(systems[2].name, "<unnamed>");
    }

    #[test]
    fn run_with_hook_calls_callback_per_system() {
        let mut world = World::new();
        world.insert_resource(Counter(0));
        let mut schedule = Schedule::new();
        schedule.add_named("first", bump);
        schedule.add_named("second", double);

        let mut names = Vec::new();
        schedule.run_with_hook(&mut world, |name| names.push(name.to_string()));

        assert_eq!(names, vec!["first".to_string(), "second".to_string()]);
        assert_eq!(world.resource::<Counter>().unwrap().0, 2);
    }

    #[test]
    fn clear_removes_all_systems() {
        let mut schedule = Schedule::new();
        schedule.add_named("a", bump);
        schedule.add_named("b", double);
        assert_eq!(schedule.len(), 2);

        schedule.clear();
        assert_eq!(schedule.len(), 0);
        assert!(schedule.is_empty());
    }

    #[test]
    fn remove_by_name_removes_first_matching_system() {
        let mut schedule = Schedule::new();
        schedule.add_named("alpha", bump);
        schedule.add_named("beta", double);
        schedule.add_named("alpha", bump);

        assert!(schedule.remove_by_name("alpha"));
        assert_eq!(schedule.len(), 2);
        assert_eq!(schedule.systems()[0].name, "beta");
        assert_eq!(schedule.systems()[1].name, "alpha");

        assert!(!schedule.remove_by_name("gamma"));
        assert_eq!(schedule.len(), 2);
    }

    fn is_debug_enabled(world: &World) -> bool {
        world.resource::<DebugMode>().map_or(false, |mode| mode.0)
    }

    #[test]
    fn conditional_system_runs_when_condition_is_met() {
        let mut world = World::new();
        world.insert_resource(Counter(0));
        world.insert_resource(DebugMode(true));

        let mut schedule = Schedule::new();
        schedule.add_named("bump", bump);
        schedule.add_conditional("debug", debug_bump, is_debug_enabled);

        schedule.run(&mut world);
        assert_eq!(world.resource::<Counter>().unwrap().0, 101);
    }

    #[test]
    fn conditional_system_skips_when_condition_is_not_met() {
        let mut world = World::new();
        world.insert_resource(Counter(0));
        world.insert_resource(DebugMode(false));

        let mut schedule = Schedule::new();
        schedule.add_named("bump", bump);
        schedule.add_conditional("debug", debug_bump, is_debug_enabled);

        schedule.run(&mut world);
        assert_eq!(world.resource::<Counter>().unwrap().0, 1);
    }

    #[test]
    fn conditional_system_skips_when_resource_missing() {
        let mut world = World::new();
        world.insert_resource(Counter(0));
        // No DebugMode resource.

        let mut schedule = Schedule::new();
        schedule.add_named("bump", bump);
        schedule.add_conditional("debug", debug_bump, is_debug_enabled);

        schedule.run(&mut world);
        assert_eq!(world.resource::<Counter>().unwrap().0, 1);
    }

    #[test]
    fn conditional_system_with_hook_skips_without_callback() {
        let mut world = World::new();
        world.insert_resource(Counter(0));
        world.insert_resource(DebugMode(false));

        let mut schedule = Schedule::new();
        schedule.add_named("bump", bump);
        schedule.add_conditional("debug", debug_bump, is_debug_enabled);

        let mut names = Vec::new();
        schedule.run_with_hook(&mut world, |name| names.push(name.to_string()));

        assert_eq!(names, vec!["bump".to_string()]);
        assert_eq!(world.resource::<Counter>().unwrap().0, 1);
    }

    #[test]
    fn conditional_system_field_is_set() {
        let mut schedule = Schedule::new();
        schedule.add_named("bump", bump);
        schedule.add_conditional("debug", debug_bump, is_debug_enabled);

        let systems = schedule.systems();
        assert!(systems[0].condition.is_none());
        assert!(systems[1].condition.is_some());
    }

    #[test]
    fn run_system_by_name_executes_matching_system() {
        let mut world = World::new();
        world.insert_resource(Counter(0));
        let mut schedule = Schedule::new();
        schedule.add_named("bump", bump);
        schedule.add_named("double", double);

        assert!(schedule.run_system_by_name("double", &mut world));
        assert_eq!(world.resource::<Counter>().unwrap().0, 0); // 0 * 2 = 0

        schedule.run_system_by_name("bump", &mut world);
        assert_eq!(world.resource::<Counter>().unwrap().0, 1);
    }

    #[test]
    fn run_system_by_name_returns_false_for_unknown_name() {
        let mut world = World::new();
        world.insert_resource(Counter(0));
        let schedule = Schedule::new();

        assert!(!schedule.run_system_by_name("nonexistent", &mut world));
    }

    #[test]
    fn run_system_by_name_respects_condition() {
        let mut world = World::new();
        world.insert_resource(Counter(0));
        world.insert_resource(DebugMode(false));
        let mut schedule = Schedule::new();
        schedule.add_conditional("debug", debug_bump, is_debug_enabled);

        assert!(!schedule.run_system_by_name("debug", &mut world));
        assert_eq!(world.resource::<Counter>().unwrap().0, 0);

        world.insert_resource(DebugMode(true));
        assert!(schedule.run_system_by_name("debug", &mut world));
        assert_eq!(world.resource::<Counter>().unwrap().0, 100);
    }

    #[test]
    fn insert_at_places_system_at_position() {
        let mut schedule = Schedule::new();
        schedule.add_named("first", bump);
        schedule.add_named("third", double);
        schedule.insert_at(1, "second", bump);

        let systems = schedule.systems();
        assert_eq!(systems[0].name, "first");
        assert_eq!(systems[1].name, "second");
        assert_eq!(systems[2].name, "third");
    }

    #[test]
    fn insert_at_affects_execution_order() {
        let mut world = World::new();
        world.insert_resource(Counter(1));
        let mut schedule = Schedule::new();
        schedule.add_named("double", double);
        schedule.insert_at(0, "bump", bump);

        schedule.run(&mut world);
        assert_eq!(world.resource::<Counter>().unwrap().0, 4);
    }

    #[test]
    fn replace_by_name_swaps_system_at_same_position() {
        let mut schedule = Schedule::new();
        schedule.add_named("first", bump);
        schedule.add_named("second", double);
        schedule.add_named("third", bump);

        assert!(schedule.replace_by_name("second", "replaced", debug_bump));
        let systems = schedule.systems();
        assert_eq!(systems[0].name, "first");
        assert_eq!(systems[1].name, "replaced");
        assert_eq!(systems[2].name, "third");
    }

    #[test]
    fn replace_by_name_returns_false_for_unknown() {
        let mut schedule = Schedule::new();
        schedule.add_named("alpha", bump);
        assert!(!schedule.replace_by_name("beta", "gamma", bump));
    }

    #[test]
    fn replace_by_name_preserves_execution_order() {
        let mut world = World::new();
        world.insert_resource(Counter(1));
        let mut schedule = Schedule::new();
        schedule.add_named("double", double);
        schedule.add_named("bump", bump);

        schedule.replace_by_name("double", "bump2", bump);
        schedule.run(&mut world);
        assert_eq!(world.resource::<Counter>().unwrap().0, 3);
    }
}
