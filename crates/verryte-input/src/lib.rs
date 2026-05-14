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

/// A keyboard-to-action map. Generic over the game's action type so the engine
/// never has to know what actions exist.
pub struct Bindings<A: Clone> {
    by_key: HashMap<Key, A>,
}

impl<A: Clone> Bindings<A> {
    pub fn new() -> Self {
        Self {
            by_key: HashMap::new(),
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

    pub fn translate(&self, key: Key) -> Option<A> {
        self.by_key.get(&key).cloned()
    }

    pub fn len(&self) -> usize {
        self.by_key.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_key.is_empty()
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
    /// a separate code path.
    pub fn parse_script(&self, script: &str) -> Result<Vec<A>, CommandParseError> {
        let mut out = Vec::new();
        let mut search_start = 0;
        for token in script.split_whitespace() {
            let relative_start = script[search_start..]
                .find(token)
                .expect("split_whitespace token came from the script");
            let byte_start = search_start + relative_start;
            let char_start = script[..byte_start].chars().count();
            search_start = byte_start + token.len();

            if let Some(action) = self.translate_name(token) {
                out.push(action);
                continue;
            }
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
        Ok(out)
    }

    pub fn name_count(&self) -> usize {
        self.by_name.len()
    }

    pub fn glyph_count(&self) -> usize {
        self.by_glyph.len()
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
}

impl<A: Clone> InputRouter<A> {
    pub fn new(bindings: Bindings<A>) -> Self {
        Self {
            bindings,
            pending: VecDeque::new(),
        }
    }

    pub fn bindings(&self) -> &Bindings<A> {
        &self.bindings
    }

    pub fn bindings_mut(&mut self) -> &mut Bindings<A> {
        &mut self.bindings
    }

    /// Translate a terminal event into a game action and queue it.
    ///
    /// Returns `true` if the event mapped to an action and was queued. Events
    /// without a binding (or non-key events like `Tick`/`Resize`) are dropped
    /// here — frontends that care about them can intercept before calling.
    pub fn handle(&mut self, event: InputEvent) -> bool {
        match event {
            InputEvent::Key(key) => {
                if let Some(action) = self.bindings.translate(key) {
                    self.pending
                        .push_back(QueuedAction::new(action, ActionSource::Terminal));
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
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

    pub fn clear(&mut self) {
        self.pending.clear();
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
}
