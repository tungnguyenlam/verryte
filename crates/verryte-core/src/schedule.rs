//! A minimal ordered system list.
//!
//! `Schedule` is intentionally small: it holds `fn(&mut World)` pointers and
//! runs them in order. This is enough for a turn-based game loop where a tick
//! is "translate input -> apply game systems -> snapshot state". Stages,
//! parallelism, and richer run conditions can grow on top of this once a real
//! game pulls on the API.

use crate::diagnostics::Diagnostics;
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
    /// Stage markers: (name, index_of_first_system_in_stage).
    stage_markers: Vec<(&'static str, usize)>,
}

impl Schedule {
    pub fn new() -> Self {
        Self {
            systems: Vec::new(),
            stage_markers: Vec::new(),
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
            let start = std::time::Instant::now();
            (system.func)(world);
            let elapsed = start.elapsed();
            if let Some(diags) = world.resource_mut::<Diagnostics>() {
                diags.record(system.name, elapsed);
            }
        }
    }

    /// Run all systems in order, ensuring a `Diagnostics` resource exists.
    ///
    /// This is a convenience method that inserts a default `Diagnostics`
    /// resource if one is not already present, then runs all systems.
    /// Metrics are automatically recorded and can be inspected afterward
    /// via `world.resource::<Diagnostics>()`.
    pub fn run_profiling(&self, world: &mut World) {
        if !world.has_resource::<Diagnostics>() {
            world.insert_resource(Diagnostics::new());
        }
        self.run(world);
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
            let start = std::time::Instant::now();
            (system.func)(world);
            let elapsed = start.elapsed();
            if let Some(diags) = world.resource_mut::<Diagnostics>() {
                diags.record(system.name, elapsed);
            }
        }
    }

    /// Remove all systems and stage markers from the schedule.
    pub fn clear(&mut self) {
        self.systems.clear();
        self.stage_markers.clear();
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
    pub fn replace_by_name(
        &mut self,
        name: &str,
        new_name: &'static str,
        new_system: System,
    ) -> bool {
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
                let start = std::time::Instant::now();
                (system.func)(world);
                let elapsed = start.elapsed();
                if let Some(diags) = world.resource_mut::<Diagnostics>() {
                    diags.record(system.name, elapsed);
                }
                return true;
            }
        }
        false
    }

    /// Begin a new named stage. All systems added after this call belong to
    /// this stage until the next `add_stage` call.
    ///
    /// Stages group systems into logical phases (e.g., "input", "ai",
    /// "resolve", "render") while keeping a flat execution order. Use
    /// [`run_stage`](Self::run_stage) to execute only the systems in one stage.
    pub fn add_stage(&mut self, name: &'static str) {
        self.stage_markers.push((name, self.systems.len()));
    }

    /// Run only the systems belonging to the named stage.
    ///
    /// Returns `true` if the stage was found. Systems within the stage respect
    /// their run conditions as usual.
    pub fn run_stage(&self, name: &str, world: &mut World) -> bool {
        let stage_idx = self.stage_markers.iter().position(|(n, _)| *n == name);
        let stage_idx = match stage_idx {
            Some(i) => i,
            None => return false,
        };
        let start = self.stage_markers[stage_idx].1;
        let end = if stage_idx + 1 < self.stage_markers.len() {
            self.stage_markers[stage_idx + 1].1
        } else {
            self.systems.len()
        };
        for system in &self.systems[start..end] {
            if let Some(cond) = system.condition {
                if !cond(world) {
                    continue;
                }
            }
            let sys_start = std::time::Instant::now();
            (system.func)(world);
            let elapsed = sys_start.elapsed();
            if let Some(diags) = world.resource_mut::<Diagnostics>() {
                diags.record(system.name, elapsed);
            }
        }
        true
    }

    /// Return the names of all defined stages, in order.
    pub fn stage_names(&self) -> Vec<&'static str> {
        self.stage_markers.iter().map(|(name, _)| *name).collect()
    }

    /// Run only the systems belonging to the named stage, calling the hook
    /// with each system's name before execution.
    ///
    /// Skipped systems (due to run conditions) do not trigger the hook.
    /// Returns `true` if the stage was found.
    pub fn run_stage_with_hook<F>(&self, name: &str, world: &mut World, mut on_system: F) -> bool
    where
        F: FnMut(&str),
    {
        let stage_idx = self.stage_markers.iter().position(|(n, _)| *n == name);
        let stage_idx = match stage_idx {
            Some(i) => i,
            None => return false,
        };
        let start = self.stage_markers[stage_idx].1;
        let end = if stage_idx + 1 < self.stage_markers.len() {
            self.stage_markers[stage_idx + 1].1
        } else {
            self.systems.len()
        };
        for system in &self.systems[start..end] {
            if let Some(cond) = system.condition {
                if !cond(world) {
                    continue;
                }
            }
            on_system(system.name);
            let sys_start = std::time::Instant::now();
            (system.func)(world);
            let elapsed = sys_start.elapsed();
            if let Some(diags) = world.resource_mut::<Diagnostics>() {
                diags.record(system.name, elapsed);
            }
        }
        true
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
        world.resource::<DebugMode>().is_some_and(|mode| mode.0)
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

    #[test]
    fn stage_names_tracks_added_stages() {
        let mut schedule = Schedule::new();
        schedule.add_stage("input");
        schedule.add_named("read_keys", bump);
        schedule.add_stage("logic");
        schedule.add_named("update", double);
        schedule.add_named("resolve", bump);
        schedule.add_stage("render");
        schedule.add_named("draw", bump);

        assert_eq!(schedule.stage_names(), vec!["input", "logic", "render"]);
    }

    #[test]
    fn run_stage_executes_only_that_stage() {
        let mut world = World::new();
        world.insert_resource(Counter(0));
        let mut schedule = Schedule::new();
        schedule.add_stage("first");
        schedule.add_named("bump", bump);
        schedule.add_stage("second");
        schedule.add_named("double", double);

        assert!(schedule.run_stage("first", &mut world));
        assert_eq!(world.resource::<Counter>().unwrap().0, 1);

        assert!(schedule.run_stage("second", &mut world));
        assert_eq!(world.resource::<Counter>().unwrap().0, 2);
    }

    #[test]
    fn run_stage_returns_false_for_unknown_stage() {
        let mut world = World::new();
        world.insert_resource(Counter(0));
        let mut schedule = Schedule::new();
        schedule.add_stage("alpha");
        schedule.add_named("bump", bump);

        assert!(!schedule.run_stage("beta", &mut world));
        assert_eq!(world.resource::<Counter>().unwrap().0, 0);
    }

    #[test]
    fn run_stage_respects_conditions() {
        let mut world = World::new();
        world.insert_resource(Counter(0));
        world.insert_resource(DebugMode(false));
        let mut schedule = Schedule::new();
        schedule.add_stage("logic");
        schedule.add_named("bump", bump);
        schedule.add_conditional("debug", debug_bump, is_debug_enabled);

        schedule.run_stage("logic", &mut world);
        assert_eq!(world.resource::<Counter>().unwrap().0, 1);
    }

    #[test]
    fn run_stages_are_independent_of_full_run() {
        let mut world = World::new();
        world.insert_resource(Counter(1));
        let mut schedule = Schedule::new();
        schedule.add_stage("a");
        schedule.add_named("bump", bump);
        schedule.add_stage("b");
        schedule.add_named("double", double);

        schedule.run_stage("b", &mut world);
        assert_eq!(world.resource::<Counter>().unwrap().0, 2);

        // Running "a" after "b" still works independently.
        schedule.run_stage("a", &mut world);
        assert_eq!(world.resource::<Counter>().unwrap().0, 3);
    }

    #[test]
    fn clear_removes_stage_markers() {
        let mut schedule = Schedule::new();
        schedule.add_stage("a");
        schedule.add_named("bump", bump);
        schedule.add_stage("b");
        schedule.add_named("double", double);
        assert_eq!(schedule.stage_names().len(), 2);

        schedule.clear();
        assert!(schedule.stage_names().is_empty());
        assert!(schedule.is_empty());
    }

    #[test]
    fn systems_without_stages_still_run_normally() {
        let mut world = World::new();
        world.insert_resource(Counter(1));
        let mut schedule = Schedule::new();
        schedule.add_named("bump", bump);
        schedule.add_named("double", double);

        // No stages defined — run() still works.
        schedule.run(&mut world);
        assert_eq!(world.resource::<Counter>().unwrap().0, 4);
    }

    #[test]
    fn run_stage_with_hook_calls_hook_for_executed_systems() {
        let mut world = World::new();
        world.insert_resource(Counter(0));
        let mut schedule = Schedule::new();
        schedule.add_stage("first");
        schedule.add_named("bump", bump);
        schedule.add_stage("second");
        schedule.add_named("double", double);

        let mut names = Vec::new();
        schedule.run_stage_with_hook("first", &mut world, |name| names.push(name.to_string()));
        assert_eq!(names, vec!["bump"]);
        assert_eq!(world.resource::<Counter>().unwrap().0, 1);
    }

    #[test]
    fn run_stage_with_hook_skips_conditional_systems() {
        let mut world = World::new();
        world.insert_resource(Counter(0));
        world.insert_resource(DebugMode(false));
        let mut schedule = Schedule::new();
        schedule.add_stage("logic");
        schedule.add_named("bump", bump);
        schedule.add_conditional("debug", debug_bump, is_debug_enabled);

        let mut names = Vec::new();
        schedule.run_stage_with_hook("logic", &mut world, |name| names.push(name.to_string()));
        assert_eq!(names, vec!["bump"]);
        assert_eq!(world.resource::<Counter>().unwrap().0, 1);
    }

    #[test]
    fn run_stage_with_hook_returns_false_for_unknown_stage() {
        let mut world = World::new();
        world.insert_resource(Counter(0));
        let mut schedule = Schedule::new();
        schedule.add_stage("alpha");
        schedule.add_named("bump", bump);

        let mut names = Vec::new();
        assert!(!schedule
            .run_stage_with_hook("beta", &mut world, |name| { names.push(name.to_string()) }));
        assert!(names.is_empty());
        assert_eq!(world.resource::<Counter>().unwrap().0, 0);
    }

    #[test]
    fn diagnostics_profiling_records_metrics() {
        let mut world = World::new();
        world.insert_resource(Diagnostics::new());
        world.insert_resource(Counter(0));

        let mut schedule = Schedule::new();
        schedule.add_named("bump", bump);

        schedule.run(&mut world);

        let diags = world.resource::<Diagnostics>().unwrap();
        let metrics = diags.systems.get("bump").unwrap();
        assert_eq!(metrics.call_count, 1);
        assert!(metrics.total_duration >= std::time::Duration::from_nanos(0));
    }

    #[test]
    fn run_profiling_inserts_diagnostics_automatically() {
        let mut world = World::new();
        world.insert_resource(Counter(0));

        let mut schedule = Schedule::new();
        schedule.add_named("bump", bump);

        // No Diagnostics resource inserted — run_profiling should add one.
        assert!(!world.has_resource::<Diagnostics>());
        schedule.run_profiling(&mut world);

        let diags = world.resource::<Diagnostics>().unwrap();
        let metrics = diags.systems.get("bump").unwrap();
        assert_eq!(metrics.call_count, 1);
    }

    #[test]
    fn run_stage_records_diagnostics() {
        let mut world = World::new();
        world.insert_resource(Diagnostics::new());
        world.insert_resource(Counter(0));

        let mut schedule = Schedule::new();
        schedule.add_stage("logic");
        schedule.add_named("bump", bump);
        schedule.add_named("double", double);

        schedule.run_stage("logic", &mut world);

        let diags = world.resource::<Diagnostics>().unwrap();
        assert_eq!(diags.systems.get("bump").unwrap().call_count, 1);
        assert_eq!(diags.systems.get("double").unwrap().call_count, 1);
    }

    #[test]
    fn run_system_by_name_records_diagnostics() {
        let mut world = World::new();
        world.insert_resource(Diagnostics::new());
        world.insert_resource(Counter(0));

        let mut schedule = Schedule::new();
        schedule.add_named("bump", bump);
        schedule.add_named("double", double);

        schedule.run_system_by_name("bump", &mut world);

        let diags = world.resource::<Diagnostics>().unwrap();
        assert_eq!(diags.systems.get("bump").unwrap().call_count, 1);
        // double was not run
        assert!(!diags.systems.contains_key("double"));
    }
}
