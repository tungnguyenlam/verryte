//! A minimal ordered system list.
//!
//! `Schedule` is intentionally small: it holds `fn(&mut World)` pointers and
//! runs them in order. This is enough for a turn-based game loop where a tick
//! is "translate input -> apply game systems -> snapshot state". Stages,
//! parallelism, and run conditions can grow on top of this once a real game
//! pulls on the API.

use crate::world::World;

pub type System = fn(&mut World);

pub struct Schedule {
    systems: Vec<System>,
}

impl Schedule {
    pub fn new() -> Self {
        Self {
            systems: Vec::new(),
        }
    }

    pub fn add(&mut self, system: System) -> &mut Self {
        self.systems.push(system);
        self
    }

    pub fn len(&self) -> usize {
        self.systems.len()
    }

    pub fn is_empty(&self) -> bool {
        self.systems.is_empty()
    }

    pub fn run(&self, world: &mut World) {
        for system in &self.systems {
            system(world);
        }
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

    fn bump(world: &mut World) {
        let counter = world.resource_mut::<Counter>().expect("counter resource");
        counter.0 += 1;
    }

    fn double(world: &mut World) {
        let counter = world.resource_mut::<Counter>().expect("counter resource");
        counter.0 *= 2;
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
}
