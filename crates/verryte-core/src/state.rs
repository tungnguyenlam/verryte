//! Simple ECS State Machine.
//!
//! Provides a `State<S>` resource for managing application or gameplay states
//! (e.g., MainMenu, Playing, Paused, GameOver), and a `StateTransitionEvent<S>` event
//! triggered when transitions occur.

use crate::event::Events;
use crate::world::World;

/// A resource representing a state in a state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct State<S> {
    current: S,
    next: Option<S>,
}

impl<S> State<S> {
    /// Create a new state resource initialized to `initial`.
    pub fn new(initial: S) -> Self {
        State {
            current: initial,
            next: None,
        }
    }

    /// Get the current state value.
    pub fn current(&self) -> &S {
        &self.current
    }

    /// Get the pending next state value, if any.
    pub fn next(&self) -> Option<&S> {
        self.next.as_ref()
    }

    /// Set the next state to transition to.
    pub fn set(&mut self, next: S) {
        self.next = Some(next);
    }
}

/// Event emitted when a state transition occurs.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StateTransitionEvent<S> {
    pub from: S,
    pub to: S,
}

impl World {
    /// Initialize a state resource in the world, along with its transition event queue.
    pub fn init_state<S: Clone + PartialEq + Send + Sync + 'static>(&mut self, initial: S) {
        self.insert_resource(State::new(initial));
        self.insert_resource(Events::<StateTransitionEvent<S>>::new());
    }

    /// Apply any pending transitions for state type `S`, emitting a `StateTransitionEvent` if one occurred.
    ///
    /// Returns `true` if a transition actually occurred.
    pub fn apply_state_transitions<S: Clone + PartialEq + Send + Sync + 'static>(
        &mut self,
    ) -> bool {
        let mut transition = None;
        if let Some(state) = self.resource_mut::<State<S>>() {
            if let Some(next) = state.next.take() {
                if next != state.current {
                    let from = state.current.clone();
                    state.current = next;
                    transition = Some((from, state.current.clone()));
                }
            }
        }

        if let Some((from, to)) = transition {
            if let Some(events) = self.resource_mut::<Events<StateTransitionEvent<S>>>() {
                events.send(StateTransitionEvent { from, to });
            }
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EventReader;

    #[allow(dead_code)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum MyState {
        Menu,
        Playing,
        Paused,
    }

    #[test]
    fn test_state_basic_transitions() {
        let mut world = World::new();
        world.init_state(MyState::Menu);

        // Verify initial state
        {
            let state = world.resource::<State<MyState>>().unwrap();
            assert_eq!(state.current(), &MyState::Menu);
            assert_eq!(state.next(), None);
        }

        // Trigger transition
        {
            let state = world.resource_mut::<State<MyState>>().unwrap();
            state.set(MyState::Playing);
            assert_eq!(state.next(), Some(&MyState::Playing));
        }

        // Apply transitions
        let transitioned = world.apply_state_transitions::<MyState>();
        assert!(transitioned);

        // Verify state is updated
        {
            let state = world.resource::<State<MyState>>().unwrap();
            assert_eq!(state.current(), &MyState::Playing);
            assert_eq!(state.next(), None);
        }

        // Verify event was sent
        {
            let events = world
                .resource::<Events<StateTransitionEvent<MyState>>>()
                .unwrap();
            let mut reader = EventReader::new();
            let transition_events: Vec<_> = reader.read(events).collect();
            assert_eq!(transition_events.len(), 1);
            assert_eq!(
                transition_events[0],
                &StateTransitionEvent {
                    from: MyState::Menu,
                    to: MyState::Playing
                }
            );
        }
    }

    #[test]
    fn test_state_no_transition_on_same_state() {
        let mut world = World::new();
        world.init_state(MyState::Menu);

        {
            let state = world.resource_mut::<State<MyState>>().unwrap();
            state.set(MyState::Menu);
        }

        let transitioned = world.apply_state_transitions::<MyState>();
        assert!(!transitioned);

        let state = world.resource::<State<MyState>>().unwrap();
        assert_eq!(state.current(), &MyState::Menu);
    }
}
