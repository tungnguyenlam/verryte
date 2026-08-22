//! The shared event/script funnel: [`InputRouter`] and [`BindingsGuard`].
//!
//! This module provides the central queue where all input (from the terminal,
//! automated test scripts, and replay traces) converges.
//!
//! # Example
//!
//! ```rust
//! use verryte_input::{ActionSource, Bindings, InputRouter, Key};
//!
//! #[derive(Clone, PartialEq, Debug)]
//! enum Action { MoveUp }
//!
//! let mut bindings = Bindings::new();
//! bindings.bind(Key::Up, Action::MoveUp);
//!
//! let mut router = InputRouter::new(bindings);
//!
//! // Terminal input
//! router.handle(verryte_input::InputEvent::Key {
//!     key: Key::Up,
//!     kind: verryte_input::KeyEventKind::Press,
//! });
//!
//! // Script injection
//! router.inject_from(Action::MoveUp, ActionSource::Script);
//!
//! let trace = router.drain_trace();
//! let actions = trace.steps();
//! assert_eq!(actions.len(), 2);
//! assert_eq!(actions[0].action, Action::MoveUp);
//! assert_eq!(actions[0].source, ActionSource::Terminal);
//! assert_eq!(actions[1].action, Action::MoveUp);
//! assert_eq!(actions[1].source, ActionSource::Script);
//! ```

use std::collections::{vec_deque, VecDeque};

use crate::action::{ActionSource, QueuedAction};

use crate::bindings::{Bindings, CommandBindings, CommandParseError};
use crate::key::{InputEvent, Key, KeyEventKind, MouseButton, ScrollDirection};
use crate::trace::ActionTrace;
use crate::RepeatConfig;

type InterceptorFn<A> = std::sync::Arc<dyn Fn(&A) -> bool + Send + Sync>;

/// The shared event/script funnel.
///
/// Holds the active [`Bindings`] and a queue of pending actions. Frontends
/// feed terminal events in with [`handle`](Self::handle); test harnesses and
/// agents inject actions with [`inject`](Self::inject). Game loops then drain
/// the queue and apply each action through the same systems.
pub struct InputRouter<A: Clone> {
    bindings: Bindings<A>,
    pending: VecDeque<QueuedAction<A>>,
    delayed: Vec<DelayedAction<A>>,
    total_queued: usize,
    context_stack: Vec<Bindings<A>>,
    history: Vec<QueuedAction<A>>,
    history_limit: Option<usize>,
    recording_path: Option<std::path::PathBuf>,
    recorded_actions: Vec<QueuedAction<A>>,
    repeat_config: RepeatConfig,
    held_key: Option<(Key, ActionSource)>,
    held_time: f32,
    last_repeat_time: f32,
    profiles: crate::bindings::BindingsProfileRegistry<A>,
    interceptor: Option<InterceptorFn<A>>,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DelayedAction<A: Clone> {
    pub action: A,
    pub source: ActionSource,
    pub delay: f32,
}

impl<A: Clone> InputRouter<A> {
    pub fn new(bindings: Bindings<A>) -> Self {
        let mut profiles = crate::bindings::BindingsProfileRegistry::new();
        profiles.register("default", bindings.clone());
        profiles.switch_profile("default");
        Self {
            bindings,
            pending: VecDeque::new(),
            delayed: Vec::new(),
            total_queued: 0,
            context_stack: Vec::new(),
            history: Vec::new(),
            history_limit: Some(1000),
            recording_path: None,
            recorded_actions: Vec::new(),
            repeat_config: RepeatConfig::default(),
            held_key: None,
            held_time: 0.0,
            last_repeat_time: 0.0,
            profiles,
            interceptor: None,
        }
    }

    /// Register a named keymap/bindings profile.
    pub fn register_profile(&mut self, name: &str, bindings: Bindings<A>) {
        self.profiles.register(name, bindings);
    }

    /// Get the name of the currently active profile, if any.
    pub fn active_profile_name(&self) -> Option<&str> {
        self.profiles.active_profile_name()
    }

    /// Switch to a registered keymap profile. Returns true if successful.
    pub fn switch_profile(&mut self, name: &str) -> bool {
        if self.profiles.switch_profile(name) {
            if let Some(active) = self.profiles.active_bindings() {
                self.bindings = active.clone();
                return true;
            }
        }
        false
    }

    /// Register a closure to intercept, validate, or filter actions.
    pub fn set_interceptor<F>(&mut self, interceptor: F)
    where
        F: Fn(&A) -> bool + Send + Sync + 'static,
    {
        self.interceptor = Some(std::sync::Arc::new(interceptor));
    }

    /// Clear the active action interceptor hook.
    pub fn clear_interceptor(&mut self) {
        self.interceptor = None;
    }

    /// Helper to queue a pending action, routing it through the interceptor if present.
    /// Returns true if the action was successfully queued (not filtered).
    fn push_pending(&mut self, qa: QueuedAction<A>, front: bool) -> bool {
        if let Some(ref interceptor) = self.interceptor {
            if !interceptor(&qa.action) {
                return false;
            }
        }
        self.record_if_active(&qa);
        if front {
            self.pending.push_front(qa);
        } else {
            self.pending.push_back(qa);
        }
        self.total_queued += 1;
        true
    }

    /// Set the maximum number of actions to keep in history.
    ///
    /// If `None`, history grows indefinitely (not recommended for long sessions).
    pub fn set_history_limit(&mut self, limit: Option<usize>) {
        self.history_limit = limit;
        self.apply_history_limit();
    }

    fn apply_history_limit(&mut self) {
        if let Some(limit) = self.history_limit {
            if self.history.len() > limit {
                let to_remove = self.history.len() - limit;
                self.history.drain(0..to_remove);
            }
        }
    }

    /// Start recording all actions to a file on disk.
    ///
    /// If a recording is already active, it is stopped first. The file is
    /// overwritten. Actions are collected in memory and flushed to disk when
    /// [`stop_recording`](Self::stop_recording) is called.
    pub fn start_recording<P: Into<std::path::PathBuf>>(&mut self, path: P) {
        if self.recording_path.is_some() {
            self.recording_path = None;
            self.recorded_actions.clear();
        }
        self.recording_path = Some(path.into());
        self.recorded_actions.clear();
    }

    /// Stop the current recording and flush collected actions to disk.
    #[cfg(feature = "serde")]
    pub fn stop_recording(&mut self)
    where
        A: serde::Serialize + serde::de::DeserializeOwned,
    {
        if let Some(path) = self.recording_path.take() {
            if let Ok(json) = serde_json::to_string_pretty(&self.recorded_actions) {
                let _ = std::fs::write(&path, json);
            }
            self.recorded_actions.clear();
        }
    }

    /// Stop the current recording without writing to disk.
    #[cfg(not(feature = "serde"))]
    pub fn stop_recording(&mut self) {
        self.recording_path = None;
        self.recorded_actions.clear();
    }

    /// Returns `true` if a recording is currently active.
    pub fn is_recording(&self) -> bool {
        self.recording_path.is_some()
    }

    /// Returns the number of actions recorded since recording started.
    pub fn recorded_count(&self) -> usize {
        self.recorded_actions.len()
    }

    /// Convert recorded actions into an [`ActionTrace`] without stopping the recording.
    ///
    /// Returns `None` if no recording is active or no actions have been recorded.
    /// The recording continues — this is a snapshot of the current recorded actions.
    pub fn recorded_as_trace(&self) -> Option<ActionTrace<A>>
    where
        A: Clone,
    {
        if self.recorded_actions.is_empty() {
            return None;
        }
        Some(ActionTrace::from_steps(self.recorded_actions.clone()))
    }

    /// Stop recording and return the collected actions as an [`ActionTrace`].
    ///
    /// Unlike [`stop_recording`](Self::stop_recording), this does not write to
    /// disk. The recording is stopped and the trace is returned for programmatic
    /// use (replay, analysis, agent replay).
    pub fn take_recording(&mut self) -> ActionTrace<A> {
        self.recording_path = None;
        let actions = std::mem::take(&mut self.recorded_actions);
        ActionTrace::from_steps(actions)
    }

    /// Borrow the currently recorded actions without stopping the recording.
    ///
    /// Returns `None` if no recording is active. Useful for inspecting
    /// what has been recorded so far without consuming or stopping.
    pub fn recorded_actions(&self) -> Option<&[QueuedAction<A>]> {
        if self.recording_path.is_some() {
            Some(&self.recorded_actions)
        } else {
            None
        }
    }

    pub fn bindings(&self) -> &Bindings<A> {
        &self.bindings
    }

    pub fn bindings_mut(&mut self) -> &mut Bindings<A> {
        &mut self.bindings
    }

    /// Replace the active bindings with a new set, returning the previous set.
    ///
    /// Useful for switching input contexts (for example, from gameplay to a
    /// menu or dialog) without losing the pending action queue.
    pub fn set_bindings(&mut self, new: Bindings<A>) -> Bindings<A> {
        std::mem::replace(&mut self.bindings, new)
    }

    /// Temporarily swap in different bindings and restore the originals when
    /// the returned guard is dropped.
    ///
    /// This is convenient for modal input contexts where you want to ensure
    /// the original bindings are restored even if the modal exits early.
    pub fn bindings_guard(&mut self, temporary: Bindings<A>) -> BindingsGuard<'_, A> {
        let original = self.set_bindings(temporary);
        BindingsGuard {
            router: self,
            original,
        }
    }

    /// Push the current bindings onto the context stack and install `new`.
    ///
    /// This enables nested modal input (game → inventory → item detail).
    /// Each push saves the current bindings; [`Self::pop_bindings`] restores
    /// the most recently saved set.
    pub fn push_bindings(&mut self, new: Bindings<A>) {
        let current = std::mem::replace(&mut self.bindings, new);
        self.context_stack.push(current);
    }

    /// Pop the most recently pushed bindings from the context stack and
    /// restore them as the active set.
    ///
    /// Returns `true` if there was a saved context to restore, `false` if the
    /// stack was empty (no change).
    pub fn pop_bindings(&mut self) -> bool {
        if let Some(previous) = self.context_stack.pop() {
            self.bindings = previous;
            true
        } else {
            false
        }
    }

    /// The number of saved binding contexts on the stack.
    pub fn context_depth(&self) -> usize {
        self.context_stack.len()
    }

    pub fn repeat_config(&self) -> RepeatConfig {
        self.repeat_config
    }

    pub fn set_repeat_config(&mut self, config: RepeatConfig) {
        self.repeat_config = config;
    }

    /// Advance input timers.
    ///
    /// This should be called once per frame with the elapsed time in seconds.
    /// It handles repeating actions for held keys and processing delayed actions.
    pub fn tick(&mut self, dt: f32) {
        // Handle repeating keys
        if let Some((key, source)) = self.held_key {
            self.held_time += dt;

            if self.held_time >= self.repeat_config.delay {
                let time_since_last = self.held_time - self.last_repeat_time;
                if time_since_last >= self.repeat_config.interval {
                    if let Some(action) = self.bindings.translate(key) {
                        let qa = QueuedAction::new(action, source);
                        if self.push_pending(qa, false) {
                            self.last_repeat_time = self.held_time;
                        }
                    }
                }
            }
        }

        // Handle delayed actions
        if !self.delayed.is_empty() {
            let mut ready = Vec::new();
            self.delayed.retain_mut(|da| {
                da.delay -= dt;
                if da.delay <= 0.0 {
                    ready.push(QueuedAction::new(da.action.clone(), da.source));
                    false
                } else {
                    true
                }
            });

            for qa in ready {
                self.push_pending(qa, false);
            }
        }
    }

    fn record_if_active(&mut self, action: &QueuedAction<A>) {
        if self.recording_path.is_some() {
            self.recorded_actions.push(action.clone());
        }
    }
}

/// Restores the original bindings when dropped.
///
/// Created by [`InputRouter::bindings_guard`].
pub struct BindingsGuard<'a, A: Clone> {
    router: &'a mut InputRouter<A>,
    original: Bindings<A>,
}

impl<A: Clone> Drop for BindingsGuard<'_, A> {
    fn drop(&mut self) {
        self.router.set_bindings(self.original.clone());
    }
}

impl<A: Clone> std::ops::Deref for BindingsGuard<'_, A> {
    type Target = InputRouter<A>;
    fn deref(&self) -> &Self::Target {
        self.router
    }
}

impl<A: Clone> std::ops::DerefMut for BindingsGuard<'_, A> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.router
    }
}

impl<A: Clone> InputRouter<A> {
    /// Translate a terminal event into a game action and queue it.
    ///
    /// Returns `true` if the event mapped to an action and was queued. Events
    /// without a binding (or events like `Tick`/`Resize`) are dropped here —
    /// frontends that care about them can intercept before calling.
    pub fn handle(&mut self, event: InputEvent) -> bool {
        self.handle_from(event, ActionSource::Terminal)
    }

    pub fn handle_event(&mut self, event: InputEvent) -> bool {
        self.handle(event)
    }

    /// Simulate a key press event.
    pub fn simulate_key_press(&mut self, key: Key) -> bool {
        self.handle(InputEvent::Key {
            key,
            kind: KeyEventKind::Press,
        })
    }

    /// Simulate a key release event.
    pub fn simulate_key_release(&mut self, key: Key) -> bool {
        self.handle(InputEvent::Key {
            key,
            kind: KeyEventKind::Release,
        })
    }

    /// Simulate a mouse press event.
    pub fn simulate_mouse_press(&mut self, button: MouseButton, x: u16, y: u16) -> bool {
        self.handle(InputEvent::Mouse {
            x,
            y,
            button,
            pressed: true,
        })
    }

    /// Simulate a mouse release event.
    pub fn simulate_mouse_release(&mut self, button: MouseButton, x: u16, y: u16) -> bool {
        self.handle(InputEvent::Mouse {
            x,
            y,
            button,
            pressed: false,
        })
    }

    /// Simulate a scroll event.
    pub fn simulate_scroll(&mut self, direction: ScrollDirection, x: u16, y: u16) -> bool {
        self.handle(InputEvent::MouseScroll { x, y, direction })
    }

    /// Translate a terminal event into a game action using a custom translator
    /// before falling back to the current bindings.
    pub fn handle_with<F>(&mut self, event: InputEvent, translate: F) -> bool
    where
        F: FnOnce(InputEvent) -> Option<A>,
    {
        self.handle_with_from(event, ActionSource::Terminal, translate)
    }

    /// Translate an input event into a game action and queue it with explicit
    /// provenance.
    pub fn handle_from(&mut self, event: InputEvent, source: ActionSource) -> bool {
        match event {
            InputEvent::Key { key, kind } => match kind {
                KeyEventKind::Press => {
                    self.held_key = Some((key, source));
                    self.held_time = 0.0;
                    self.last_repeat_time = 0.0;

                    if let Some(action) = self.bindings.translate(key) {
                        let qa = QueuedAction::new(action, source);
                        self.push_pending(qa, false)
                    } else {
                        false
                    }
                }
                KeyEventKind::Release => {
                    if let Some((held, _)) = self.held_key {
                        if held == key {
                            self.held_key = None;
                        }
                    }
                    false
                }
                KeyEventKind::Repeat => false,
            },
            _ => {
                if let Some(action) = self.bindings.translate_event(event) {
                    let qa = QueuedAction::new(action, source);
                    self.push_pending(qa, false)
                } else {
                    false
                }
            }
        }
    }

    /// Translate an input event with a custom translator, falling back to
    /// bindings if the translator returns `None`.
    pub fn handle_with_from<F>(
        &mut self,
        event: InputEvent,
        source: ActionSource,
        translate: F,
    ) -> bool
    where
        F: FnOnce(InputEvent) -> Option<A>,
    {
        if let Some(action) = translate(event) {
            let qa = QueuedAction::new(action, source);
            self.push_pending(qa, false)
        } else {
            self.handle_from(event, source)
        }
    }

    /// Translate many input events at once. Returns the count of events that
    /// produced a queued action.
    pub fn handle_batch<I>(&mut self, events: I) -> usize
    where
        I: IntoIterator<Item = InputEvent>,
    {
        self.handle_batch_from(events, ActionSource::Terminal)
    }

    /// Translate many input events at once using a custom translator, falling
    /// back to the current bindings for events the translator ignores.
    pub fn handle_batch_with<I, F>(&mut self, events: I, translate: F) -> usize
    where
        I: IntoIterator<Item = InputEvent>,
        F: FnMut(InputEvent) -> Option<A>,
    {
        self.handle_batch_with_from(events, ActionSource::Terminal, translate)
    }

    /// Translate many input events at once with explicit provenance.
    pub fn handle_batch_from<I>(&mut self, events: I, source: ActionSource) -> usize
    where
        I: IntoIterator<Item = InputEvent>,
    {
        let mut count = 0;
        for event in events {
            if self.handle_from(event, source) {
                count += 1;
            }
        }
        count
    }

    /// Translate many input events at once with a custom translator and
    /// explicit provenance.
    pub fn handle_batch_with_from<I, F>(
        &mut self,
        events: I,
        source: ActionSource,
        mut translate: F,
    ) -> usize
    where
        I: IntoIterator<Item = InputEvent>,
        F: FnMut(InputEvent) -> Option<A>,
    {
        let mut count = 0;
        for event in events {
            if let Some(action) = translate(event) {
                let qa = QueuedAction::new(action, source);
                if self.push_pending(qa, false) {
                    count += 1;
                }
            } else if let Some(action) = self.bindings.translate_event(event) {
                let qa = QueuedAction::new(action, source);
                if self.push_pending(qa, false) {
                    count += 1;
                }
            }
        }
        count
    }

    /// Inject an action directly. This is the path scripts, tests, and agents
    /// use; it shares the queue with `handle`, so order is preserved across
    /// both paths.
    pub fn inject(&mut self, action: A) {
        self.inject_from(action, ActionSource::Script);
    }

    /// Inject an action with explicit provenance for reports, replays, or
    /// agent drivers.
    pub fn inject_from(&mut self, action: A, source: ActionSource) {
        let qa = QueuedAction::new(action, source);
        self.push_pending(qa, false);
    }

    /// Inject a high-priority action at the front of the queue.
    pub fn inject_priority(&mut self, action: A) {
        self.inject_priority_from(action, ActionSource::Script);
    }

    /// Inject a high-priority action at the front of the queue with explicit
    /// provenance.
    pub fn inject_priority_from(&mut self, action: A, source: ActionSource) {
        let qa = QueuedAction::new(action, source);
        self.push_pending(qa, true);
    }

    /// Inject an action that will be queued after a delay.
    pub fn inject_delayed(&mut self, action: A, delay: f32) {
        self.inject_delayed_from(action, ActionSource::Script, delay);
    }

    /// Inject an action that will be queued after a delay with explicit provenance.
    pub fn inject_delayed_from(&mut self, action: A, source: ActionSource, delay: f32) {
        self.delayed.push(DelayedAction {
            action,
            source,
            delay,
        });
    }

    /// Inject many actions in order. Convenience for scripted runs.
    pub fn inject_all<I: IntoIterator<Item = A>>(&mut self, actions: I) {
        for action in actions {
            self.inject(action);
        }
    }

    /// Inject many actions in order with the same explicit source.
    pub fn inject_all_from<I: IntoIterator<Item = A>>(&mut self, actions: I, source: ActionSource) {
        for action in actions {
            self.inject_from(action, source);
        }
    }

    /// Parse a textual script with command bindings and enqueue every parsed
    /// action with the same source.
    pub fn inject_script(
        &mut self,
        commands: &CommandBindings<A>,
        script: &str,
        source: ActionSource,
    ) -> Result<usize, CommandParseError> {
        let actions = commands.parse_script(script)?;
        let count = actions.len();
        self.inject_all_from(actions, source);
        Ok(count)
    }

    /// Parse a script with command bindings and a custom token resolver, then
    /// enqueue every parsed action with the same source.
    pub fn inject_script_with<F>(
        &mut self,
        commands: &CommandBindings<A>,
        script: &str,
        source: ActionSource,
        resolve_token: F,
    ) -> Result<usize, CommandParseError>
    where
        F: FnMut(&str) -> Option<A>,
    {
        let actions = commands.parse_script_with(script, resolve_token)?;
        let count = actions.len();
        self.inject_all_from(actions, source);
        Ok(count)
    }

    pub fn next_action(&mut self) -> Option<A> {
        self.next_queued().map(QueuedAction::into_action)
    }

    pub fn next_queued(&mut self) -> Option<QueuedAction<A>> {
        let action = self.pending.pop_front();
        if let Some(ref act) = action {
            self.history.push(act.clone());
            self.apply_history_limit();
        }
        action
    }

    pub fn pop_action(&mut self) -> Option<QueuedAction<A>> {
        self.next_queued()
    }

    pub fn drain(&mut self) -> impl Iterator<Item = A> + '_ {
        for act in &self.pending {
            self.history.push(act.clone());
        }
        self.apply_history_limit();
        self.pending.drain(..).map(QueuedAction::into_action)
    }

    pub fn drain_queued(&mut self) -> vec_deque::Drain<'_, QueuedAction<A>> {
        for act in &self.pending {
            self.history.push(act.clone());
        }
        self.apply_history_limit();
        self.pending.drain(..)
    }

    /// Drain the pending queue into a replayable trace, preserving sources.
    pub fn drain_trace(&mut self) -> ActionTrace<A> {
        for act in &self.pending {
            self.history.push(act.clone());
        }
        self.apply_history_limit();
        ActionTrace::from_steps(self.pending.drain(..))
    }

    pub fn history(&self) -> &[QueuedAction<A>] {
        &self.history
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    pub fn is_idle(&self) -> bool {
        self.pending.is_empty()
    }

    /// Returns `true` if there are no bindings and no pending actions.
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty() && self.pending.is_empty()
    }

    /// Save the current action history to a file as JSON.
    #[cfg(feature = "serde")]
    pub fn save_history_to_file<P: AsRef<std::path::Path>>(
        &self,
        path: P,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        A: serde::Serialize + serde::de::DeserializeOwned,
    {
        let json = serde_json::to_string_pretty(&self.history)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load action history from a JSON file.
    #[cfg(feature = "serde")]
    pub fn load_history_from_file<P: AsRef<std::path::Path>>(
        path: P,
    ) -> Result<Vec<QueuedAction<A>>, Box<dyn std::error::Error>>
    where
        A: serde::Serialize + serde::de::DeserializeOwned,
    {
        let content = std::fs::read_to_string(path)?;
        let history: crate::action::ActionHistory<A> = serde_json::from_str(&content)?;
        let steps = history
            .records
            .into_iter()
            .map(|r| QueuedAction::new(r.action, r.source))
            .collect();
        Ok(steps)
    }

    pub fn peek(&self) -> Option<&QueuedAction<A>> {
        self.pending.front()
    }

    pub fn pending_iter(&self) -> vec_deque::Iter<'_, QueuedAction<A>> {
        self.pending.iter()
    }

    /// Snapshot the pending queue as a replayable trace without draining it.
    pub fn pending_trace(&self) -> ActionTrace<A> {
        ActionTrace::from_steps(self.pending.iter().cloned())
    }

    pub fn clear(&mut self) {
        self.pending.clear();
    }

    /// Remove pending actions matching a predicate, keeping the rest.
    ///
    /// Useful for canceling queued actions when game state changes.
    /// Returns the count of removed actions.
    pub fn filter_pending<F>(&mut self, mut predicate: F) -> usize
    where
        F: FnMut(&QueuedAction<A>) -> bool,
    {
        let mut queue = VecDeque::new();
        std::mem::swap(&mut self.pending, &mut queue);
        let mut removed = 0;
        for action in queue {
            if predicate(&action) {
                removed += 1;
            } else {
                self.pending.push_back(action);
            }
        }
        removed
    }

    /// Remove pending actions matching a predicate, returning the removed items.
    pub fn drain_filtered<F>(&mut self, mut predicate: F) -> Vec<QueuedAction<A>>
    where
        F: FnMut(&QueuedAction<A>) -> bool,
    {
        let mut queue = VecDeque::new();
        std::mem::swap(&mut self.pending, &mut queue);
        let mut removed = Vec::new();
        for action in queue {
            if predicate(&action) {
                removed.push(action);
            } else {
                self.pending.push_back(action);
            }
        }
        removed
    }

    /// Total number of actions queued over the lifetime of this router.
    pub fn total_actions_queued(&self) -> usize {
        self.total_queued
    }
}
