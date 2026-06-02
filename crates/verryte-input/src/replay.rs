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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::ActionSource;
    use crate::bindings::Bindings;
    use crate::router::InputRouter;
    use crate::trace::ActionTrace;

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum Act {
        A,
        B,
        C,
    }

    fn make_trace() -> ActionTrace<Act> {
        ActionTrace::from_actions(vec![Act::A, Act::B, Act::C], ActionSource::Script)
    }

    #[test]
    fn action_replayer_new_starts_at_zero() {
        let r = ActionReplayer::new(make_trace());
        assert_eq!(r.current_index(), 0);
        assert_eq!(r.total_steps(), 3);
        assert!(!r.is_finished());
    }

    #[test]
    fn action_replayer_with_auto_sets_flag() {
        let r = ActionReplayer::new(make_trace()).with_auto();
        assert!(r.auto_advance);
    }

    #[test]
    fn action_replayer_step_feeds_into_router() {
        let trace = make_trace();
        let mut replayer = ActionReplayer::new(trace);
        let mut router: InputRouter<Act> = InputRouter::new(Bindings::new());

        assert!(replayer.step(&mut router));
        assert_eq!(replayer.current_index(), 1);
        let action = router.pop_action().unwrap();
        assert_eq!(action.action, Act::A);
        assert_eq!(action.source, ActionSource::Script);

        assert!(replayer.step(&mut router));
        assert_eq!(replayer.current_index(), 2);
        let action = router.pop_action().unwrap();
        assert_eq!(action.action, Act::B);
        assert_eq!(action.source, ActionSource::Script);

        assert!(replayer.step(&mut router));
        assert_eq!(replayer.current_index(), 3);
        assert!(replayer.is_finished());

        assert!(!replayer.step(&mut router));
    }

    #[test]
    fn action_replayer_reset_returns_to_start() {
        let trace = make_trace();
        let mut replayer = ActionReplayer::new(trace);
        let mut router: InputRouter<Act> = InputRouter::new(Bindings::new());

        replayer.step(&mut router);
        replayer.step(&mut router);
        assert_eq!(replayer.current_index(), 2);

        replayer.reset();
        assert_eq!(replayer.current_index(), 0);
        assert!(!replayer.is_finished());
    }

    #[test]
    fn action_replayer_trace_returns_reference() {
        let trace = make_trace();
        let replayer = ActionReplayer::new(trace);
        assert_eq!(replayer.trace().len(), 3);
    }

    #[test]
    fn replay_runner_new_starts_at_zero() {
        let runner = ReplayRunner::new(make_trace());
        assert_eq!(runner.current_step(), 0);
        assert_eq!(runner.total_steps(), 3);
        assert!(!runner.is_finished());
    }

    #[test]
    fn replay_runner_step_feeds_into_router() {
        let trace = make_trace();
        let mut runner = ReplayRunner::new(trace);
        let mut router: InputRouter<Act> = InputRouter::new(Bindings::new());

        assert!(runner.step(&mut router));
        assert_eq!(runner.current_step(), 1);
        let action = router.pop_action().unwrap();
        assert_eq!(action.action, Act::A);

        assert!(runner.step(&mut router));
        assert!(runner.step(&mut router));
        assert!(runner.is_finished());
        assert!(!runner.step(&mut router));
    }

    #[test]
    fn replay_runner_fast_forward_drains_all() {
        let trace = make_trace();
        let mut runner = ReplayRunner::new(trace);
        let mut router: InputRouter<Act> = InputRouter::new(Bindings::new());

        runner.fast_forward(&mut router);
        assert!(runner.is_finished());
        assert_eq!(runner.current_step(), 3);
        assert_eq!(router.pending(), 3);
    }

    #[test]
    fn replay_runner_reset_returns_to_start() {
        let trace = make_trace();
        let mut runner = ReplayRunner::new(trace);
        let mut router: InputRouter<Act> = InputRouter::new(Bindings::new());

        runner.fast_forward(&mut router);
        assert!(runner.is_finished());

        runner.reset();
        assert_eq!(runner.current_step(), 0);
        assert!(!runner.is_finished());
    }

    #[test]
    fn replay_trace_feeds_all_actions() {
        let trace = make_trace();
        let mut router: InputRouter<Act> = InputRouter::new(Bindings::new());
        replay_trace(&trace, &mut router);
        assert_eq!(router.pending(), 3);

        let a1 = router.pop_action().unwrap();
        assert_eq!(a1.action, Act::A);
        let a2 = router.pop_action().unwrap();
        assert_eq!(a2.action, Act::B);
        let a3 = router.pop_action().unwrap();
        assert_eq!(a3.action, Act::C);
    }

    #[test]
    fn replay_empty_trace_is_noop() {
        let trace = ActionTrace::<Act>::from_actions(vec![], ActionSource::Script);
        let mut replayer = ActionReplayer::new(trace.clone());
        let mut runner = ReplayRunner::new(trace.clone());
        let mut router: InputRouter<Act> = InputRouter::new(Bindings::new());

        assert!(replayer.is_finished());
        assert!(!replayer.step(&mut router));

        assert!(runner.is_finished());
        assert!(!runner.step(&mut router));

        replay_trace(&trace, &mut router);
        assert_eq!(router.pending(), 0);
    }
}
