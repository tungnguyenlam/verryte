//! Terminal-agnostic input model.
//!
//! `verryte-input` exists to enforce the most important shape in the engine:
//!
//! ```text
//! terminal event -> game action -> game system -> observable state
//! script command -> game action -> game system -> observable state
//! ```
//!
//! Both halves go through the same [`InputRouter`]:
//!
//! * Interactive frontends translate their native key/mouse events into the
//!   neutral [`InputEvent`] type and feed them through [`InputRouter::handle`].
//!   Simple key and mouse-button bindings can queue actions directly.
//! * Scripts, tests, and agents push fully-formed actions through
//!   [`InputRouter::inject`].
//!
//! Downstream, the game just drains the action queue. It cannot tell — and
//! does not need to tell — whether an action came from a keypress or a script.
//! If a harness wants that information for logs, replays, or debugging, it can
//! drain [`QueuedAction`] values and read their [`ActionSource`].
//!
//! The router is generic over the game's action enum, so games define their
//! own action vocabulary without giving up the shared dispatch path.

use std::collections::{vec_deque, HashMap, VecDeque};

/// Neutral terminal-side key identifier.
///
/// Frontends (crossterm, termion, custom) translate their native key types
/// into this enum so [`Bindings`] doesn't depend on any particular backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Char(char),
    Enter,
    Esc,
    Tab,
    Backspace,
    Space,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    F(u8),
}

/// Mouse buttons in a terminal-friendly shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// A coarse mouse binding key.
///
/// The trigger intentionally ignores the terminal cell position. Games that
/// need position-aware mouse behavior can inspect [`InputEvent::Mouse`] before
/// routing it, while simple controls can still enter the shared action queue.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MouseTrigger {
    pub button: MouseButton,
    pub pressed: bool,
}

impl MouseTrigger {
    pub fn new(button: MouseButton, pressed: bool) -> Self {
        Self { button, pressed }
    }
}

/// One discrete input event. Frontends emit these; the router converts them
/// (when bound) into game actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputEvent {
    Key(Key),
    Mouse {
        x: u16,
        y: u16,
        button: MouseButton,
        pressed: bool,
    },
    /// A fixed-cadence pulse useful for real-time games; ignored by default.
    Tick,
    /// A platform-level resize notification.
    Resize {
        width: u16,
        height: u16,
    },
}

/// Where a queued action came from before entering the shared game-action path.
///
/// This is metadata only: games should still apply the contained action through
/// the same systems no matter who produced it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ActionSource {
    Terminal,
    Script,
    Agent,
    Replay,
    Test,
}

/// One pending game action plus its control-plane provenance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueuedAction<A> {
    pub action: A,
    pub source: ActionSource,
}

impl<A> QueuedAction<A> {
    pub fn new(action: A, source: ActionSource) -> Self {
        Self { action, source }
    }

    pub fn into_action(self) -> A {
        self.action
    }
}

/// A replayable sequence of sourced actions.
///
/// Traces keep control-plane provenance attached to actions while still
/// replaying through [`InputRouter`]'s normal pending queue. This is useful for
/// recording a terminal session, storing an agent plan, or turning a failing
/// test into a reproducible script without creating another action path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionTrace<A> {
    steps: Vec<QueuedAction<A>>,
}

impl<A> ActionTrace<A> {
    pub fn new() -> Self {
        Self { steps: Vec::new() }
    }

    pub fn from_steps<I: IntoIterator<Item = QueuedAction<A>>>(steps: I) -> Self {
        Self {
            steps: steps.into_iter().collect(),
        }
    }

    pub fn push(&mut self, action: A, source: ActionSource) {
        self.steps.push(QueuedAction::new(action, source));
    }

    pub fn len(&self) -> usize {
        self.steps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, QueuedAction<A>> {
        self.steps.iter()
    }

    pub fn extend(&mut self, other: ActionTrace<A>) {
        self.steps.extend(other.steps);
    }

    pub fn into_steps(self) -> Vec<QueuedAction<A>> {
        self.steps
    }
}

impl<A: Clone> ActionTrace<A> {
    pub fn from_actions<I: IntoIterator<Item = A>>(actions: I, source: ActionSource) -> Self {
        Self {
            steps: actions
                .into_iter()
                .map(|action| QueuedAction::new(action, source))
                .collect(),
        }
    }

    pub fn replay_into(&self, router: &mut InputRouter<A>) {
        for step in &self.steps {
            router.inject_from(step.action.clone(), step.source);
        }
    }
}

impl<A> Default for ActionTrace<A> {
    fn default() -> Self {
        Self::new()
    }
}

/// A keyboard-to-action map. Generic over the game's action type so the engine
/// never has to know what actions exist.
#[derive(Clone)]
pub struct Bindings<A: Clone> {
    by_key: HashMap<Key, A>,
    by_mouse: HashMap<MouseTrigger, A>,
}

impl<A: Clone> Bindings<A> {
    pub fn new() -> Self {
        Self {
            by_key: HashMap::new(),
            by_mouse: HashMap::new(),
        }
    }

    /// Bind a key to an action. If the key was already bound, the new action
    /// wins; the previous action is returned.
    pub fn bind(&mut self, key: Key, action: A) -> Option<A> {
        self.by_key.insert(key, action)
    }

    pub fn unbind(&mut self, key: Key) -> Option<A> {
        self.by_key.remove(&key)
    }

    /// Bind a mouse button transition to an action.
    ///
    /// This is deliberately position-neutral. It is useful for commands like
    /// scan, wait, or confirm. Cell-targeted actions can be layered on later
    /// without creating a separate control path.
    pub fn bind_mouse(&mut self, button: MouseButton, pressed: bool, action: A) -> Option<A> {
        self.by_mouse
            .insert(MouseTrigger::new(button, pressed), action)
    }

    pub fn unbind_mouse(&mut self, button: MouseButton, pressed: bool) -> Option<A> {
        self.by_mouse.remove(&MouseTrigger::new(button, pressed))
    }

    pub fn translate(&self, key: Key) -> Option<A> {
        self.by_key.get(&key).cloned()
    }

    pub fn translate_mouse(&self, button: MouseButton, pressed: bool) -> Option<A> {
        self.by_mouse
            .get(&MouseTrigger::new(button, pressed))
            .cloned()
    }

    pub fn translate_event(&self, event: InputEvent) -> Option<A> {
        match event {
            InputEvent::Key(key) => self.translate(key),
            InputEvent::Mouse {
                button, pressed, ..
            } => self.translate_mouse(button, pressed),
            InputEvent::Tick | InputEvent::Resize { .. } => None,
        }
    }

    pub fn len(&self) -> usize {
        self.by_key.len() + self.by_mouse.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_key.is_empty() && self.by_mouse.is_empty()
    }

    /// Merge `other` bindings into `self`. Bindings in `other` overwrite
    /// existing bindings in `self` for the same key or mouse trigger.
    ///
    /// Useful for layering input contexts: start with base game bindings,
    /// then merge context-specific bindings (menus, dialogs, etc.) on top.
    pub fn merge(&mut self, other: Bindings<A>) {
        for (key, action) in other.by_key {
            self.by_key.insert(key, action);
        }
        for (trigger, action) in other.by_mouse {
            self.by_mouse.insert(trigger, action);
        }
    }
}

impl<A: Clone> Default for Bindings<A> {
    fn default() -> Self {
        Self::new()
    }
}

/// Script/agent command bindings for a game's action vocabulary.
///
/// `Bindings` maps neutral terminal events to actions. `CommandBindings` maps
/// textual commands to the same actions, so a harness can parse input like
/// `"north pickup east"` or compact glyph scripts like `"ne,."` and inject the
/// resulting actions into [`InputRouter`].
pub struct CommandBindings<A: Clone> {
    by_name: HashMap<String, A>,
    by_glyph: HashMap<char, A>,
}

impl<A: Clone> CommandBindings<A> {
    pub fn new() -> Self {
        Self {
            by_name: HashMap::new(),
            by_glyph: HashMap::new(),
        }
    }

    /// Bind a case-sensitive command name to an action.
    pub fn bind_name<S: Into<String>>(&mut self, name: S, action: A) -> Option<A> {
        self.by_name.insert(name.into(), action)
    }

    /// Bind a single compact script glyph to an action.
    pub fn bind_glyph(&mut self, glyph: char, action: A) -> Option<A> {
        self.by_glyph.insert(glyph, action)
    }

    pub fn translate_name(&self, name: &str) -> Option<A> {
        self.by_name.get(name).cloned()
    }

    pub fn translate_glyph(&self, glyph: char) -> Option<A> {
        self.by_glyph.get(&glyph).cloned()
    }

    /// Parse whitespace-separated command names into actions.
    pub fn parse_words(&self, script: &str) -> Result<Vec<A>, CommandParseError> {
        let mut out = Vec::new();
        for word in script.split_whitespace() {
            let action = self
                .translate_name(word)
                .ok_or_else(|| CommandParseError::UnknownCommand(word.to_owned()))?;
            out.push(action);
        }
        Ok(out)
    }

    /// Parse a compact glyph script into actions, ignoring whitespace.
    pub fn parse_glyphs(&self, script: &str) -> Result<Vec<A>, CommandParseError> {
        let mut out = Vec::new();
        for (index, glyph) in script.chars().enumerate() {
            if glyph.is_whitespace() {
                continue;
            }
            let action = self
                .translate_glyph(glyph)
                .ok_or(CommandParseError::UnknownGlyph { glyph, index })?;
            out.push(action);
        }
        Ok(out)
    }

    /// Parse a script that may mix command words and compact glyph runs.
    ///
    /// Each non-whitespace token first tries to resolve as a named command. If
    /// no name matches, the token is parsed as one or more glyph commands. This
    /// lets harnesses accept both `"east pickup"` and `"e,"` without choosing
    /// a separate code path. Unbound `,` and `;` act as separators, and `#`
    /// starts an inline comment that continues until newline.
    pub fn parse_script(&self, script: &str) -> Result<Vec<A>, CommandParseError> {
        self.parse_script_with(script, |_| None)
    }

    /// Parse a mixed script and allow custom token resolution before glyph
    /// fallback.
    ///
    /// This keeps dynamic command forms (for example `scan:3`) on the same
    /// parsing path as regular command names and glyph runs.
    pub fn parse_script_with<F>(
        &self,
        script: &str,
        mut resolve_token: F,
    ) -> Result<Vec<A>, CommandParseError>
    where
        F: FnMut(&str) -> Option<A>,
    {
        let mut out = Vec::new();
        let is_separator =
            |ch: char| (ch == ',' || ch == ';') && self.translate_glyph(ch).is_none();
        let chars: Vec<(usize, usize, char)> = script
            .char_indices()
            .enumerate()
            .map(|(char_index, (byte_index, ch))| (char_index, byte_index, ch))
            .collect();

        let mut i = 0;
        while i < chars.len() {
            let (_, _, ch) = chars[i];
            if ch.is_whitespace() || is_separator(ch) {
                i += 1;
                continue;
            }
            if ch == '#' {
                i += 1;
                while i < chars.len() && chars[i].2 != '\n' {
                    i += 1;
                }
                continue;
            }

            let (char_start, byte_start, _) = chars[i];
            i += 1;
            while i < chars.len() {
                let next = chars[i].2;
                if next.is_whitespace() || is_separator(next) || next == '#' {
                    break;
                }
                i += 1;
            }
            let byte_end = if i < chars.len() {
                chars[i].1
            } else {
                script.len()
            };
            let token = &script[byte_start..byte_end];

            if let Some(action) = self.translate_name(token) {
                out.push(action);
            } else if let Some(action) = resolve_token(token) {
                out.push(action);
            } else {
                for (offset, glyph) in token.chars().enumerate() {
                    let action =
                        self.translate_glyph(glyph)
                            .ok_or(CommandParseError::UnknownGlyph {
                                glyph,
                                index: char_start + offset,
                            })?;
                    out.push(action);
                }
            }
        }
        Ok(out)
    }

    pub fn name_count(&self) -> usize {
        self.by_name.len()
    }

    pub fn glyph_count(&self) -> usize {
        self.by_glyph.len()
    }

    /// Merge `other` command bindings into `self`. Bindings in `other`
    /// overwrite existing bindings in `self` for the same name or glyph.
    ///
    /// Useful for layering command sets: base game commands, then
    /// context-specific commands (debug, admin, mod) on top.
    pub fn merge(&mut self, other: CommandBindings<A>) {
        for (name, action) in other.by_name {
            self.by_name.insert(name, action);
        }
        for (glyph, action) in other.by_glyph {
            self.by_glyph.insert(glyph, action);
        }
    }
}

impl<A: Clone> Default for CommandBindings<A> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandParseError {
    UnknownCommand(String),
    UnknownGlyph { glyph: char, index: usize },
}

impl std::fmt::Display for CommandParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandParseError::UnknownCommand(command) => {
                write!(f, "unknown command {command:?}")
            }
            CommandParseError::UnknownGlyph { glyph, index } => {
                write!(f, "unknown action glyph {glyph:?} at character {index}")
            }
        }
    }
}

impl std::error::Error for CommandParseError {}

/// The shared event/script funnel.
///
/// Holds the active [`Bindings`] and a queue of pending actions. Frontends
/// feed terminal events in with [`handle`](Self::handle); test harnesses and
/// agents inject actions with [`inject`](Self::inject). Game loops then drain
/// the queue and apply each action through the same systems.
pub struct InputRouter<A: Clone> {
    bindings: Bindings<A>,
    pending: VecDeque<QueuedAction<A>>,
    total_queued: usize,
}

impl<A: Clone> InputRouter<A> {
    pub fn new(bindings: Bindings<A>) -> Self {
        Self {
            bindings,
            pending: VecDeque::new(),
            total_queued: 0,
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
    ///
    /// # Example
    ///
    /// ```ignore
    /// let guard = router.bindings_guard(menu_bindings);
    /// // Use router with menu bindings...
    /// // Original bindings restored when guard drops.
    /// ```
    pub fn bindings_guard(&mut self, temporary: Bindings<A>) -> BindingsGuard<'_, A> {
        let original = self.set_bindings(temporary);
        BindingsGuard {
            router: self,
            original,
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

    /// Translate an input event into a game action and queue it with explicit
    /// provenance. This keeps replayed or synthetic input events on the same
    /// path as real terminal input while preserving useful report metadata.
    pub fn handle_from(&mut self, event: InputEvent, source: ActionSource) -> bool {
        if let Some(action) = self.bindings.translate_event(event) {
            self.pending.push_back(QueuedAction::new(action, source));
            self.total_queued += 1;
            true
        } else {
            false
        }
    }

    /// Translate many input events at once. Returns the count of events that
    /// produced a queued action.
    pub fn handle_batch<'a, I>(&mut self, events: I) -> usize
    where
        I: IntoIterator<Item = InputEvent>,
    {
        self.handle_batch_from(events, ActionSource::Terminal)
    }

    /// Translate many input events at once with explicit provenance.
    pub fn handle_batch_from<'a, I>(&mut self, events: I, source: ActionSource) -> usize
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

    /// Inject an action directly. This is the path scripts, tests, and agents
    /// use; it shares the queue with `handle`, so order is preserved across
    /// both paths.
    pub fn inject(&mut self, action: A) {
        self.inject_from(action, ActionSource::Script);
    }

    /// Inject an action with explicit provenance for reports, replays, or
    /// agent drivers. The source does not affect game behavior.
    pub fn inject_from(&mut self, action: A, source: ActionSource) {
        self.pending.push_back(QueuedAction::new(action, source));
        self.total_queued += 1;
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
        self.pending.pop_front()
    }

    pub fn drain(&mut self) -> impl Iterator<Item = A> + '_ {
        self.drain_queued().map(QueuedAction::into_action)
    }

    pub fn drain_queued(&mut self) -> vec_deque::Drain<'_, QueuedAction<A>> {
        self.pending.drain(..)
    }

    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    pub fn is_idle(&self) -> bool {
        self.pending.is_empty()
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

    /// Total number of actions queued over the lifetime of this router.
    ///
    /// This counter never decreases, even when actions are drained. Useful
    /// for metrics, debugging, and detecting whether any input has been
    /// processed.
    pub fn total_actions_queued(&self) -> usize {
        self.total_queued
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Move {
        North,
        South,
        East,
        West,
        Wait,
        Scan(u16),
    }

    fn bound_router() -> InputRouter<Move> {
        let mut bindings = Bindings::new();
        bindings.bind(Key::Up, Move::North);
        bindings.bind(Key::Down, Move::South);
        bindings.bind(Key::Left, Move::West);
        bindings.bind(Key::Right, Move::East);
        bindings.bind(Key::Char('.'), Move::Wait);
        InputRouter::new(bindings)
    }

    #[test]
    fn key_event_translates_to_action() {
        let mut router = bound_router();
        assert!(router.handle(InputEvent::Key(Key::Up)));
        assert_eq!(router.next_action(), Some(Move::North));
        assert!(router.is_idle());
    }

    #[test]
    fn unbound_key_is_dropped() {
        let mut router = bound_router();
        assert!(!router.handle(InputEvent::Key(Key::Char('z'))));
        assert!(router.is_idle());
    }

    #[test]
    fn non_key_events_are_ignored_by_default() {
        let mut router = bound_router();
        assert!(!router.handle(InputEvent::Tick));
        assert!(!router.handle(InputEvent::Resize {
            width: 80,
            height: 24
        }));
        assert!(router.is_idle());
    }

    #[test]
    fn mouse_bindings_enter_the_same_action_queue() {
        let mut router = bound_router();
        router
            .bindings_mut()
            .bind_mouse(MouseButton::Right, true, Move::Wait);

        assert!(router.handle(InputEvent::Mouse {
            x: 12,
            y: 4,
            button: MouseButton::Right,
            pressed: true,
        }));
        assert_eq!(
            router.next_queued(),
            Some(QueuedAction::new(Move::Wait, ActionSource::Terminal))
        );
    }

    #[test]
    fn input_events_can_be_queued_with_explicit_source() {
        let mut router = bound_router();
        assert!(router.handle_from(InputEvent::Key(Key::Right), ActionSource::Replay));
        assert_eq!(
            router.next_queued(),
            Some(QueuedAction::new(Move::East, ActionSource::Replay))
        );
    }

    #[test]
    fn injected_actions_share_queue_with_translated_events() {
        let mut router = bound_router();
        router.handle(InputEvent::Key(Key::Up));
        router.inject(Move::Wait);
        router.handle(InputEvent::Key(Key::Right));
        let drained: Vec<Move> = router.drain().collect();
        assert_eq!(drained, vec![Move::North, Move::Wait, Move::East]);
    }

    #[test]
    fn queued_actions_track_source_without_changing_order() {
        let mut router = bound_router();
        router.handle(InputEvent::Key(Key::Up));
        router.inject_from(Move::Wait, ActionSource::Agent);
        router.inject_all_from([Move::East, Move::South], ActionSource::Replay);

        let queued: Vec<QueuedAction<Move>> = router.drain_queued().collect();
        assert_eq!(
            queued,
            vec![
                QueuedAction::new(Move::North, ActionSource::Terminal),
                QueuedAction::new(Move::Wait, ActionSource::Agent),
                QueuedAction::new(Move::East, ActionSource::Replay),
                QueuedAction::new(Move::South, ActionSource::Replay),
            ]
        );
    }

    #[test]
    fn inject_all_preserves_order() {
        let mut router = bound_router();
        router.inject_all([Move::North, Move::North, Move::East]);
        assert_eq!(router.pending(), 3);
        assert_eq!(router.next_action(), Some(Move::North));
        assert_eq!(router.next_action(), Some(Move::North));
        assert_eq!(router.next_action(), Some(Move::East));
        assert_eq!(router.next_action(), None);
    }

    #[test]
    fn rebinding_replaces_action() {
        let mut router = bound_router();
        let prev = router.bindings_mut().bind(Key::Up, Move::Wait);
        assert_eq!(prev, Some(Move::North));
        router.handle(InputEvent::Key(Key::Up));
        assert_eq!(router.next_action(), Some(Move::Wait));
    }

    #[test]
    fn command_words_parse_to_actions() {
        let mut commands = CommandBindings::new();
        commands.bind_name("north", Move::North);
        commands.bind_name("wait", Move::Wait);

        let parsed = commands.parse_words("north wait north").unwrap();
        assert_eq!(parsed, vec![Move::North, Move::Wait, Move::North]);
    }

    #[test]
    fn command_glyphs_parse_to_actions_and_ignore_whitespace() {
        let mut commands = CommandBindings::new();
        commands.bind_glyph('n', Move::North);
        commands.bind_glyph('.', Move::Wait);

        let parsed = commands.parse_glyphs("n . n").unwrap();
        assert_eq!(parsed, vec![Move::North, Move::Wait, Move::North]);
    }

    #[test]
    fn mixed_scripts_parse_words_and_glyph_runs() {
        let mut commands = CommandBindings::new();
        commands.bind_name("north", Move::North);
        commands.bind_name("wait", Move::Wait);
        commands.bind_glyph('e', Move::East);
        commands.bind_glyph('w', Move::West);
        commands.bind_glyph('.', Move::Wait);

        let parsed = commands.parse_script("north ew wait .").unwrap();
        assert_eq!(
            parsed,
            vec![Move::North, Move::East, Move::West, Move::Wait, Move::Wait]
        );
    }

    #[test]
    fn mixed_scripts_accept_commas_semicolons_and_comments() {
        let mut commands = CommandBindings::new();
        commands.bind_name("north", Move::North);
        commands.bind_glyph('e', Move::East);
        commands.bind_glyph('.', Move::Wait);

        let parsed = commands
            .parse_script("north,e.; # stop parsing this line\ne")
            .unwrap();
        assert_eq!(
            parsed,
            vec![Move::North, Move::East, Move::Wait, Move::East]
        );
    }

    #[test]
    fn mixed_scripts_can_use_custom_token_resolver() {
        let mut commands = CommandBindings::new();
        commands.bind_name("north", Move::North);
        commands.bind_glyph('e', Move::East);

        let parsed = commands
            .parse_script_with("north scan:3 ee x2", |token| {
                token
                    .strip_prefix("scan:")
                    .or_else(|| token.strip_prefix('x'))
                    .and_then(|digits| digits.parse::<u16>().ok())
                    .map(Move::Scan)
            })
            .unwrap();
        assert_eq!(
            parsed,
            vec![
                Move::North,
                Move::Scan(3),
                Move::East,
                Move::East,
                Move::Scan(2)
            ]
        );
    }

    #[test]
    fn command_parse_errors_identify_unknown_input() {
        let commands = CommandBindings::<Move>::new();
        assert_eq!(
            commands.parse_words("north").unwrap_err(),
            CommandParseError::UnknownCommand("north".to_owned())
        );
        assert_eq!(
            commands.parse_glyphs("x").unwrap_err(),
            CommandParseError::UnknownGlyph {
                glyph: 'x',
                index: 0
            }
        );
    }

    #[test]
    fn router_can_parse_and_enqueue_script_actions_with_source() {
        let mut commands = CommandBindings::new();
        commands.bind_name("north", Move::North);
        commands.bind_glyph('e', Move::East);

        let mut router = bound_router();
        let count = router
            .inject_script(&commands, "north ee", ActionSource::Agent)
            .unwrap();

        assert_eq!(count, 3);
        assert_eq!(
            router.drain_queued().collect::<Vec<_>>(),
            vec![
                QueuedAction::new(Move::North, ActionSource::Agent),
                QueuedAction::new(Move::East, ActionSource::Agent),
                QueuedAction::new(Move::East, ActionSource::Agent),
            ]
        );
    }

    #[test]
    fn router_can_enqueue_script_actions_with_custom_token_resolver() {
        let mut commands = CommandBindings::new();
        commands.bind_name("north", Move::North);

        let mut router = bound_router();
        let count = router
            .inject_script_with(&commands, "north scan:4", ActionSource::Agent, |token| {
                token
                    .strip_prefix("scan:")
                    .and_then(|digits| digits.parse::<u16>().ok())
                    .map(Move::Scan)
            })
            .unwrap();

        assert_eq!(count, 2);
        assert_eq!(
            router.drain_queued().collect::<Vec<_>>(),
            vec![
                QueuedAction::new(Move::North, ActionSource::Agent),
                QueuedAction::new(Move::Scan(4), ActionSource::Agent),
            ]
        );
    }

    #[test]
    fn action_trace_replays_sourced_actions_through_router() {
        let mut trace = ActionTrace::new();
        trace.push(Move::North, ActionSource::Terminal);
        trace.push(Move::Wait, ActionSource::Agent);
        assert_eq!(trace.len(), 2);

        let mut router = bound_router();
        trace.replay_into(&mut router);

        assert_eq!(
            router.drain_queued().collect::<Vec<_>>(),
            vec![
                QueuedAction::new(Move::North, ActionSource::Terminal),
                QueuedAction::new(Move::Wait, ActionSource::Agent),
            ]
        );
    }

    #[test]
    fn pending_trace_snapshots_queue_without_draining() {
        let mut router = bound_router();
        router.inject_from(Move::North, ActionSource::Agent);
        router.handle(InputEvent::Key(Key::Right));

        let trace = router.pending_trace();
        assert_eq!(router.pending(), 2);
        assert_eq!(
            trace.into_steps(),
            vec![
                QueuedAction::new(Move::North, ActionSource::Agent),
                QueuedAction::new(Move::East, ActionSource::Terminal),
            ]
        );
    }

    #[test]
    fn action_trace_can_be_built_from_unsourced_action_runs() {
        let trace = ActionTrace::from_actions([Move::East, Move::East], ActionSource::Replay);

        assert_eq!(
            trace.into_steps(),
            vec![
                QueuedAction::new(Move::East, ActionSource::Replay),
                QueuedAction::new(Move::East, ActionSource::Replay),
            ]
        );
    }

    #[test]
    fn action_trace_can_extend() {
        let mut trace1 = ActionTrace::from_actions([Move::North], ActionSource::Test);
        let trace2 = ActionTrace::from_actions([Move::South], ActionSource::Agent);
        trace1.extend(trace2);
        assert_eq!(
            trace1.into_steps(),
            vec![
                QueuedAction::new(Move::North, ActionSource::Test),
                QueuedAction::new(Move::South, ActionSource::Agent),
            ]
        );
    }

    #[test]
    fn handle_batch_queues_multiple_events() {
        let mut router = bound_router();
        let count = router.handle_batch([
            InputEvent::Key(Key::Up),
            InputEvent::Key(Key::Right),
            InputEvent::Key(Key::Down),
        ]);
        assert_eq!(count, 3);
        assert_eq!(router.pending(), 3);
        let drained: Vec<Move> = router.drain().collect();
        assert_eq!(drained, vec![Move::North, Move::East, Move::South]);
    }

    #[test]
    fn handle_batch_skips_unbound_events() {
        let mut router = bound_router();
        let count = router.handle_batch([
            InputEvent::Key(Key::Up),
            InputEvent::Key(Key::Char('z')),
            InputEvent::Tick,
            InputEvent::Key(Key::Left),
        ]);
        assert_eq!(count, 2);
        assert_eq!(router.pending(), 2);
    }

    #[test]
    fn handle_batch_from_preserves_source() {
        let mut router = bound_router();
        router.handle_batch_from(
            [InputEvent::Key(Key::Up), InputEvent::Key(Key::Right)],
            ActionSource::Agent,
        );
        let queued: Vec<QueuedAction<Move>> = router.drain_queued().collect();
        assert_eq!(queued.len(), 2);
        assert!(queued.iter().all(|q| q.source == ActionSource::Agent));
    }

    #[test]
    fn set_bindings_swaps_keymap_and_returns_old() {
        let mut router = bound_router();
        assert!(router.handle(InputEvent::Key(Key::Up)));
        assert_eq!(router.next_action(), Some(Move::North));

        let mut menu_bindings = Bindings::new();
        menu_bindings.bind(Key::Enter, Move::Wait);
        menu_bindings.bind(Key::Esc, Move::Wait);

        let old = router.set_bindings(menu_bindings);
        assert!(old.translate(Key::Up).is_some());
        assert!(!router.handle(InputEvent::Key(Key::Up)));
        assert!(router.handle(InputEvent::Key(Key::Enter)));
        assert_eq!(router.next_action(), Some(Move::Wait));

        router.set_bindings(old);
        assert!(router.handle(InputEvent::Key(Key::Up)));
        assert_eq!(router.next_action(), Some(Move::North));
    }

    #[test]
    fn bindings_guard_restores_on_drop() {
        let mut router = bound_router();
        let mut menu_bindings = Bindings::new();
        menu_bindings.bind(Key::Enter, Move::Wait);

        // Verify original bindings work.
        assert!(router.handle(InputEvent::Key(Key::Up)));
        assert_eq!(router.next_action(), Some(Move::North));

        {
            let mut guard = router.bindings_guard(menu_bindings);
            assert!(guard.handle(InputEvent::Key(Key::Enter)));
            assert_eq!(guard.next_action(), Some(Move::Wait));
            assert!(!guard.handle(InputEvent::Key(Key::Up)));
        }

        // Verify original bindings are restored.
        assert!(router.handle(InputEvent::Key(Key::Up)));
        assert_eq!(router.next_action(), Some(Move::North));
    }

    #[test]
    fn bindings_guard_restores_even_on_panic() {
        let mut router = bound_router();
        let mut menu_bindings = Bindings::new();
        menu_bindings.bind(Key::Enter, Move::Wait);

        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = router.bindings_guard(menu_bindings);
            panic!("intentional");
        }));

        assert!(router.handle(InputEvent::Key(Key::Up)));
        assert_eq!(router.next_action(), Some(Move::North));
    }

    #[test]
    fn bindings_merge_combines_key_and_mouse_maps() {
        let mut base = Bindings::new();
        base.bind(Key::Up, Move::North);
        base.bind(Key::Down, Move::South);

        let mut overlay = Bindings::new();
        overlay.bind(Key::Down, Move::Wait);
        overlay.bind(Key::Left, Move::West);
        overlay.bind_mouse(MouseButton::Right, true, Move::Scan(1));

        base.merge(overlay);

        assert_eq!(base.translate(Key::Up), Some(Move::North));
        assert_eq!(base.translate(Key::Down), Some(Move::Wait));
        assert_eq!(base.translate(Key::Left), Some(Move::West));
        assert_eq!(
            base.translate_mouse(MouseButton::Right, true),
            Some(Move::Scan(1))
        );
        assert_eq!(base.len(), 4);
    }

    #[test]
    fn command_bindings_merge_combines_names_and_glyphs() {
        let mut base = CommandBindings::new();
        base.bind_name("north", Move::North);
        base.bind_glyph('e', Move::East);

        let mut overlay = CommandBindings::new();
        overlay.bind_name("north", Move::Wait); // overwrite
        overlay.bind_name("south", Move::South);
        overlay.bind_glyph('w', Move::West);

        base.merge(overlay);

        assert_eq!(base.translate_name("north"), Some(Move::Wait));
        assert_eq!(base.translate_name("south"), Some(Move::South));
        assert_eq!(base.translate_glyph('e'), Some(Move::East));
        assert_eq!(base.translate_glyph('w'), Some(Move::West));
        assert_eq!(base.name_count(), 2);
        assert_eq!(base.glyph_count(), 2);
    }

    #[test]
    fn total_actions_queued_tracks_lifetime_count() {
        let mut router = bound_router();
        assert_eq!(router.total_actions_queued(), 0);

        router.handle(InputEvent::Key(Key::Up));
        assert_eq!(router.total_actions_queued(), 1);

        router.inject(Move::Wait);
        assert_eq!(router.total_actions_queued(), 2);

        router.inject_all([Move::East, Move::South]);
        assert_eq!(router.total_actions_queued(), 4);

        // Draining does not decrease the counter.
        let _: Vec<Move> = router.drain().collect();
        assert_eq!(router.total_actions_queued(), 4);

        router.handle(InputEvent::Key(Key::Down));
        assert_eq!(router.total_actions_queued(), 5);
    }
}
