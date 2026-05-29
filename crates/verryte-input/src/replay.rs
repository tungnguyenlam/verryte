//! Stateful replay helpers: [`ActionReplayer`] and [`ReplayRunner`].

use crate::router::InputRouter;
use crate::trace::ActionTrace;

/// A stateful replayer that can feed an [`ActionTrace`] into an [`InputRouter`]
/// one step at a time.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(bound = "A: serde::Serialize + serde::de::DeserializeOwned")
)]
pub struct ActionReplayer<A> {
    trace: ActionTrace<A>,
    index: usize,
    pub auto_advance: bool,
}

impl<A> ActionReplayer<A> {
    pub fn new(trace: ActionTrace<A>) -> Self {
        Self {
            trace,
            index: 0,
            auto_advance: false,
        }
    }

    pub fn with_auto(mut self) -> Self {
        self.auto_advance = true;
        self
    }

    pub fn reset(&mut self) {
        self.index = 0;
    }

    pub fn is_finished(&self) -> bool {
        self.index >= self.trace.len()
    }

    pub fn current_index(&self) -> usize {
        self.index
    }

    pub fn total_steps(&self) -> usize {
        self.trace.len()
    }

    /// Pull the next action from the trace and inject it into the router.
    ///
    /// Returns `true` if an action was replayed, `false` if the trace is
    /// finished.
    pub fn step(&mut self, router: &mut InputRouter<A>) -> bool
    where
        A: Clone,
    {
        if let Some(step) = self.trace.steps().get(self.index) {
            router.inject_from(step.action.clone(), step.source);
            self.index += 1;
            true
        } else {
            false
        }
    }

    pub fn trace(&self) -> &ActionTrace<A> {
        &self.trace
    }
}

/// Automates replaying an [`ActionTrace`] into an [`InputRouter`].
pub struct ReplayRunner<A: Clone> {
    trace: ActionTrace<A>,
    index: usize,
}

impl<A: Clone> ReplayRunner<A> {
    pub fn new(trace: ActionTrace<A>) -> Self {
        Self { trace, index: 0 }
    }

    /// Advance the replay by one step, injecting the action into the router.
    ///
    /// Returns `true` if an action was injected, `false` if the end of the
    /// trace was reached.
    pub fn step(&mut self, router: &mut InputRouter<A>) -> bool {
        if let Some(step) = self.trace.steps().get(self.index) {
            router.inject_from(step.action.clone(), step.source);
            self.index += 1;
            true
        } else {
            false
        }
    }

    /// Inject all remaining actions into the router immediately.
    pub fn fast_forward(&mut self, router: &mut InputRouter<A>) {
        while self.step(router) {}
    }

    pub fn is_finished(&self) -> bool {
        self.index >= self.trace.steps().len()
    }

    pub fn current_step(&self) -> usize {
        self.index
    }

    pub fn total_steps(&self) -> usize {
        self.trace.steps().len()
    }

    pub fn reset(&mut self) {
        self.index = 0;
    }
}

/// Replay a trace's sourced actions through a router.
///
/// This is a free function to avoid circular dependency between trace and
/// router modules.
pub fn replay_trace<A: Clone>(trace: &ActionTrace<A>, router: &mut InputRouter<A>) {
    for step in trace.steps() {
        router.inject_from(step.action.clone(), step.source);
    }
}
