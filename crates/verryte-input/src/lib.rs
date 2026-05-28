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
//!
//! # Example
//!
//! ```rust
//! use verryte_input::{Bindings, InputRouter, Key, InputEvent, KeyEventKind};
//!
//! #[derive(Clone, Debug, PartialEq)]
//! enum Action {
//!     MoveUp,
//!     MoveDown,
//!     Quit,
//! }
//!
//! let mut bindings = Bindings::new();
//! bindings.bind(Key::Char('w'), Action::MoveUp);
//! bindings.bind(Key::Char('s'), Action::MoveDown);
//! bindings.bind(Key::Esc, Action::Quit);
//!
//! let mut router = InputRouter::new(bindings);
//!
//! // Simulate a key press
//! router.handle(InputEvent::Key { key: Key::Char('w'), kind: KeyEventKind::Press });
//!
//! // Game loop consumes the actions
//! let mut actions: Vec<_> = router.drain().collect();
//! assert_eq!(actions, vec![Action::MoveUp]);
//! ```

use std::collections::{vec_deque, HashMap, VecDeque};

/// Configuration for action repeating when a key is held down.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RepeatConfig {
    /// Initial delay in seconds before repeating starts.
    pub delay: f32,
    /// Interval in seconds between repeated actions.
    pub interval: f32,
}

impl Default for RepeatConfig {
    fn default() -> Self {
        Self {
            delay: 0.25,
            interval: 0.05,
        }
    }
}

/// Neutral terminal-side key identifier.
///
/// Frontends (crossterm, termion, custom) translate their native key types
/// into this enum so [`Bindings`] doesn't depend on any particular backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    /// A character key with modifier flags.
    ///
    /// Use this for bindings like Ctrl+C, Alt+X, Shift+Tab, etc.
    /// The plain `Char` variant is for unmodified character input.
    Modified {
        char: char,
        ctrl: bool,
        alt: bool,
        shift: bool,
    },
}

impl Key {
    /// Create a modified key with the given flags.
    pub fn modified(ch: char, ctrl: bool, alt: bool, shift: bool) -> Self {
        Key::Modified {
            char: ch,
            ctrl,
            alt,
            shift,
        }
    }

    /// Returns `true` if this key has any modifier flags set.
    pub fn is_modified(&self) -> bool {
        match self {
            Key::Modified {
                ctrl, alt, shift, ..
            } => *ctrl || *alt || *shift,
            _ => false,
        }
    }
}

impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Key::Char(c) => write!(f, "{c}"),
            Key::Enter => write!(f, "Enter"),
            Key::Esc => write!(f, "Esc"),
            Key::Tab => write!(f, "Tab"),
            Key::Backspace => write!(f, "Backspace"),
            Key::Space => write!(f, "Space"),
            Key::Up => write!(f, "Up"),
            Key::Down => write!(f, "Down"),
            Key::Left => write!(f, "Left"),
            Key::Right => write!(f, "Right"),
            Key::Home => write!(f, "Home"),
            Key::End => write!(f, "End"),
            Key::PageUp => write!(f, "PageUp"),
            Key::PageDown => write!(f, "PageDown"),
            Key::Insert => write!(f, "Insert"),
            Key::Delete => write!(f, "Delete"),
            Key::F(n) => write!(f, "F{n}"),
            Key::Modified {
                char: c,
                ctrl,
                alt,
                shift,
            } => {
                let mut parts = Vec::new();
                if *ctrl {
                    parts.push("Ctrl");
                }
                if *alt {
                    parts.push("Alt");
                }
                if *shift {
                    parts.push("Shift");
                }
                parts.push("");
                write!(f, "{}{c}", parts.join("+"))
            }
        }
    }
}

/// Mouse buttons in a terminal-friendly shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

impl std::fmt::Display for MouseButton {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MouseButton::Left => write!(f, "Left"),
            MouseButton::Right => write!(f, "Right"),
            MouseButton::Middle => write!(f, "Middle"),
        }
    }
}

/// Mouse wheel scroll direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

impl std::fmt::Display for ScrollDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScrollDirection::Up => write!(f, "Up"),
            ScrollDirection::Down => write!(f, "Down"),
            ScrollDirection::Left => write!(f, "Left"),
            ScrollDirection::Right => write!(f, "Right"),
        }
    }
}

/// A coarse mouse binding key.
///
/// The trigger intentionally ignores the terminal cell position. Games that
/// need position-aware mouse behavior can inspect [`InputEvent::Mouse`] before
/// routing it, while simple controls can still enter the shared action queue.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MouseTrigger {
    pub button: MouseButton,
    pub pressed: bool,
}

impl MouseTrigger {
    pub fn new(button: MouseButton, pressed: bool) -> Self {
        Self { button, pressed }
    }
}

/// The kind of key event (press, repeat, or release).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum KeyEventKind {
    Press,
    Repeat,
    Release,
}

impl std::fmt::Display for KeyEventKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeyEventKind::Press => write!(f, "Press"),
            KeyEventKind::Repeat => write!(f, "Repeat"),
            KeyEventKind::Release => write!(f, "Release"),
        }
    }
}

/// One discrete input event. Frontends emit these; the router converts them
/// (when bound) into game actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum InputEvent {
    Key {
        key: Key,
        kind: KeyEventKind,
    },
    Mouse {
        x: u16,
        y: u16,
        button: MouseButton,
        pressed: bool,
    },
    /// A mouse scroll event at a terminal cell position.
    MouseScroll {
        x: u16,
        y: u16,
        direction: ScrollDirection,
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ActionSource {
    Terminal,
    Script,
    Agent,
    Replay,
    Test,
}

impl std::fmt::Display for ActionSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActionSource::Terminal => write!(f, "Terminal"),
            ActionSource::Script => write!(f, "Script"),
            ActionSource::Agent => write!(f, "Agent"),
            ActionSource::Replay => write!(f, "Replay"),
            ActionSource::Test => write!(f, "Test"),
        }
    }
}

impl std::str::FromStr for ActionSource {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "terminal" => Ok(ActionSource::Terminal),
            "script" => Ok(ActionSource::Script),
            "agent" => Ok(ActionSource::Agent),
            "replay" => Ok(ActionSource::Replay),
            "test" => Ok(ActionSource::Test),
            other => Err(format!("unknown ActionSource: {other}")),
        }
    }
}

/// One pending game action plus its control-plane provenance.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(bound = "A: serde::Serialize + serde::de::DeserializeOwned")
)]
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

/// A permanent record of an action that was applied to the game.
///
/// Unlike [`QueuedAction`], which is for pending work, [`ActionRecord`]
/// stores the action alongside its source, a timestamp, and optional
/// metadata (like turn number or phase) for replay and analysis.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(bound = "A: serde::Serialize + serde::de::DeserializeOwned")
)]
pub struct ActionRecord<A> {
    pub action: A,
    pub source: ActionSource,
    pub timestamp: f32,
    pub metadata: std::collections::HashMap<String, String>,
}

impl<A> ActionRecord<A> {
    pub fn new(action: A, source: ActionSource, timestamp: f32) -> Self {
        Self {
            action,
            source,
            timestamp,
            metadata: std::collections::HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }
}

/// A linear history of all actions applied during a game session.
///
/// This provides the data foundation for replays, undo/redo (if actions are
/// reversible), and agent performance analysis.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(bound = "A: serde::Serialize + serde::de::DeserializeOwned")
)]
pub struct ActionHistory<A> {
    pub records: Vec<ActionRecord<A>>,
}

impl<A> Default for ActionHistory<A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A> ActionHistory<A> {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    pub fn push(&mut self, record: ActionRecord<A>) {
        self.records.push(record);
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn clear(&mut self) {
        self.records.clear();
    }
}

/// A replayable sequence of sourced actions.
///
/// Traces keep control-plane provenance attached to actions while still
/// replaying through [`InputRouter`]'s normal pending queue. This is useful for
/// recording a terminal session, storing an agent plan, or turning a failing
/// test into a reproducible script without creating another action path.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(bound = "A: serde::Serialize + serde::de::DeserializeOwned")
)]
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

    pub fn steps(&self) -> &[QueuedAction<A>] {
        &self.steps
    }

    pub fn extend(&mut self, other: ActionTrace<A>) {
        self.steps.extend(other.steps);
    }

    pub fn into_steps(self) -> Vec<QueuedAction<A>> {
        self.steps
    }

    /// Create a trace from a list of records.
    pub fn from_records(records: &[ActionRecord<A>]) -> Self
    where
        A: Clone,
    {
        Self {
            steps: records
                .iter()
                .map(|r| QueuedAction::new(r.action.clone(), r.source))
                .collect(),
        }
    }

    /// Create a trace from a history of queued actions.
    pub fn from_history(history: &[QueuedAction<A>]) -> Self
    where
        A: Clone,
    {
        Self {
            steps: history.to_vec(),
        }
    }

    /// Serialize the action trace to a detailed string using a custom formatter for action values.
    ///
    /// Each step is written on its own line in the format: `Source:ActionString`.
    pub fn to_detailed_string<F>(&self, mut format_action: F) -> String
    where
        F: FnMut(&A) -> String,
    {
        let mut out = String::new();
        for step in &self.steps {
            out.push_str(&format!(
                "{}:{}\n",
                step.source,
                format_action(&step.action)
            ));
        }
        out
    }

    /// Deserialize an action trace from a detailed string using a custom action parser.
    ///
    /// The string should have one action per line in the format: `Source:ActionString`.
    /// Empty lines and lines starting with `#` are ignored as comments.
    pub fn from_detailed_string<F>(s: &str, mut parse_action: F) -> Result<Self, String>
    where
        F: FnMut(&str) -> Option<A>,
    {
        let mut steps = Vec::new();
        for (line_idx, line) in s.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let (source_str, action_str) = trimmed.split_once(':').ok_or_else(|| {
                format!("invalid line {} (missing ':'): {:?}", line_idx + 1, trimmed)
            })?;
            let source = source_str
                .trim()
                .parse::<ActionSource>()
                .map_err(|e| format!("invalid source on line {}: {}", line_idx + 1, e))?;
            let action = parse_action(action_str.trim()).ok_or_else(|| {
                format!(
                    "unrecognized action on line {}: {:?}",
                    line_idx + 1,
                    action_str
                )
            })?;
            steps.push(QueuedAction::new(action, source));
        }
        Ok(Self { steps })
    }

    /// Save the action trace to a file on disk.
    pub fn save_to_file<P, F>(&self, path: P, format_action: F) -> Result<(), String>
    where
        P: AsRef<std::path::Path>,
        F: FnMut(&A) -> String,
    {
        let path = path.as_ref();
        let s = self.to_detailed_string(format_action);
        std::fs::write(path, s)
            .map_err(|e| format!("failed to write action trace to {:?}: {}", path, e))
    }

    /// Load an action trace from a file on disk.
    pub fn load_from_file<P, F>(path: P, parse_action: F) -> Result<Self, String>
    where
        P: AsRef<std::path::Path>,
        F: FnMut(&str) -> Option<A>,
    {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read action trace from {:?}: {}", path, e))?;
        Self::from_detailed_string(&content, parse_action)
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
    by_scroll: HashMap<ScrollDirection, A>,
}

impl<A: Clone> Bindings<A> {
    pub fn new() -> Self {
        Self {
            by_key: HashMap::new(),
            by_mouse: HashMap::new(),
            by_scroll: HashMap::new(),
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

    /// Bind a scroll direction to an action.
    pub fn bind_scroll(&mut self, direction: ScrollDirection, action: A) -> Option<A> {
        self.by_scroll.insert(direction, action)
    }

    pub fn unbind_scroll(&mut self, direction: ScrollDirection) -> Option<A> {
        self.by_scroll.remove(&direction)
    }

    pub fn translate(&self, key: Key) -> Option<A> {
        self.by_key.get(&key).cloned()
    }

    pub fn translate_mouse(&self, button: MouseButton, pressed: bool) -> Option<A> {
        self.by_mouse
            .get(&MouseTrigger::new(button, pressed))
            .cloned()
    }

    pub fn translate_scroll(&self, direction: ScrollDirection) -> Option<A> {
        self.by_scroll.get(&direction).cloned()
    }

    pub fn translate_event(&self, event: InputEvent) -> Option<A> {
        match event {
            InputEvent::Key { key, kind } => {
                if matches!(kind, KeyEventKind::Press | KeyEventKind::Repeat) {
                    self.translate(key)
                } else {
                    None
                }
            }
            InputEvent::Mouse {
                button, pressed, ..
            } => self.translate_mouse(button, pressed),
            InputEvent::MouseScroll { direction, .. } => self.translate_scroll(direction),
            InputEvent::Tick | InputEvent::Resize { .. } => None,
        }
    }

    pub fn len(&self) -> usize {
        self.by_key.len() + self.by_mouse.len() + self.by_scroll.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_key.is_empty() && self.by_mouse.is_empty() && self.by_scroll.is_empty()
    }

    /// Iterate over all key bindings as `(Key, &A)` pairs.
    pub fn iter_keys(&self) -> impl Iterator<Item = (Key, &A)> {
        self.by_key.iter().map(|(&k, a)| (k, a))
    }

    /// Iterate over all mouse bindings as `((MouseButton, bool), &A)` pairs.
    /// The bool is the `pressed` flag.
    pub fn iter_mouse(&self) -> impl Iterator<Item = ((MouseButton, bool), &A)> {
        self.by_mouse
            .iter()
            .map(|(t, a)| ((t.button, t.pressed), a))
    }

    /// Iterate over all scroll bindings as `(ScrollDirection, &A)` pairs.
    pub fn iter_scroll(&self) -> impl Iterator<Item = (ScrollDirection, &A)> {
        self.by_scroll.iter().map(|(&dir, a)| (dir, a))
    }

    /// Merge `other` bindings into `self`. Bindings in `other` overwrite
    /// existing bindings in `self` for the same key, mouse trigger, or scroll.
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
        for (direction, action) in other.by_scroll {
            self.by_scroll.insert(direction, action);
        }
    }

    /// Remove all key, mouse, and scroll bindings.
    pub fn clear(&mut self) {
        self.by_key.clear();
        self.by_mouse.clear();
        self.by_scroll.clear();
    }
}

impl<A: Clone> Default for Bindings<A> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "serde")]
impl<A: Clone + serde::Serialize> serde::Serialize for Bindings<A> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("Bindings", 3)?;

        let keys: Vec<(&Key, &A)> = self.by_key.iter().collect();
        let mouse: Vec<(&MouseTrigger, &A)> = self.by_mouse.iter().collect();
        let scroll: Vec<(&ScrollDirection, &A)> = self.by_scroll.iter().collect();

        state.serialize_field("by_key", &keys)?;
        state.serialize_field("by_mouse", &mouse)?;
        state.serialize_field("by_scroll", &scroll)?;
        state.end()
    }
}

#[cfg(feature = "serde")]
impl<'de, A: Clone + serde::Deserialize<'de>> serde::Deserialize<'de> for Bindings<A> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct BindingsHelper<A> {
            by_key: Vec<(Key, A)>,
            by_mouse: Vec<(MouseTrigger, A)>,
            by_scroll: Vec<(ScrollDirection, A)>,
        }

        let helper = BindingsHelper::deserialize(deserializer)?;
        Ok(Bindings {
            by_key: helper.by_key.into_iter().collect(),
            by_mouse: helper.by_mouse.into_iter().collect(),
            by_scroll: helper.by_scroll.into_iter().collect(),
        })
    }
}

/// Script/agent command bindings for a game's action vocabulary.
///
/// `Bindings` maps neutral terminal events to actions. `CommandBindings` maps
/// textual commands to the same actions, so a harness can parse input like
/// `"north pickup east"` or compact glyph scripts like `"ne,."` and inject the
/// resulting actions into [`InputRouter`].
#[derive(Clone)]
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

    /// Iterate over all name bindings as `(&str, &A)` pairs.
    pub fn iter_names(&self) -> impl Iterator<Item = (&str, &A)> {
        self.by_name.iter().map(|(k, a)| (k.as_str(), a))
    }

    /// Iterate over all glyph bindings as `(char, &A)` pairs.
    pub fn iter_glyphs(&self) -> impl Iterator<Item = (char, &A)> {
        self.by_glyph.iter().map(|(&k, a)| (k, a))
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

    /// Remove all name and glyph command bindings.
    pub fn clear(&mut self) {
        self.by_name.clear();
        self.by_glyph.clear();
    }
}

impl<A: Clone> Default for CommandBindings<A> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "serde")]
impl<A: Clone + serde::Serialize> serde::Serialize for CommandBindings<A> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("CommandBindings", 2)?;

        let names: Vec<(&String, &A)> = self.by_name.iter().collect();
        let glyphs: Vec<(&char, &A)> = self.by_glyph.iter().collect();

        state.serialize_field("by_name", &names)?;
        state.serialize_field("by_glyph", &glyphs)?;
        state.end()
    }
}

#[cfg(feature = "serde")]
impl<'de, A: Clone + serde::Deserialize<'de>> serde::Deserialize<'de> for CommandBindings<A> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct CommandBindingsHelper<A> {
            by_name: Vec<(String, A)>,
            by_glyph: Vec<(char, A)>,
        }

        let helper = CommandBindingsHelper::deserialize(deserializer)?;
        Ok(CommandBindings {
            by_name: helper.by_name.into_iter().collect(),
            by_glyph: helper.by_glyph.into_iter().collect(),
        })
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
    /// Returns `true` if an action was replayed, `false` if the trace is finished.
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
    context_stack: Vec<Bindings<A>>,
    history: Vec<QueuedAction<A>>,
    recording_path: Option<std::path::PathBuf>,
    repeat_config: RepeatConfig,
    held_key: Option<(Key, ActionSource)>,
    held_time: f32,
    last_repeat_time: f32,
}

impl<A: Clone> InputRouter<A> {
    pub fn new(bindings: Bindings<A>) -> Self {
        Self {
            bindings,
            pending: VecDeque::new(),
            total_queued: 0,
            context_stack: Vec::new(),
            history: Vec::new(),
            recording_path: None,
            repeat_config: RepeatConfig::default(),
            held_key: None,
            held_time: 0.0,
            last_repeat_time: 0.0,
        }
    }

    /// Start recording all actions to a file on disk.
    ///
    /// If a recording is already active, it is stopped first. The file is
    /// overwritten. Actions are written as they are popped from the queue.
    pub fn start_recording<P: Into<std::path::PathBuf>>(&mut self, path: P) {
        self.recording_path = Some(path.into());
    }

    /// Stop the current recording.
    pub fn stop_recording(&mut self) {
        self.recording_path = None;
    }

    /// Returns `true` if a recording is currently active.
    pub fn is_recording(&self) -> bool {
        self.recording_path.is_some()
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
    ///
    /// A depth of 0 means only the current bindings are active. A depth of 2
    /// means two levels of nested modals have been pushed.
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
    /// It handles repeating actions for held keys.
    pub fn tick(&mut self, dt: f32) {
        if let Some((key, source)) = self.held_key {
            self.held_time += dt;

            if self.held_time >= self.repeat_config.delay {
                let time_since_last = self.held_time - self.last_repeat_time;
                if time_since_last >= self.repeat_config.interval {
                    if let Some(action) = self.bindings.translate(key) {
                        self.pending.push_back(QueuedAction::new(action, source));
                        self.total_queued += 1;
                        self.last_repeat_time = self.held_time;
                    }
                }
            }
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

    /// Translate a terminal event into a game action using a custom translator
    /// before falling back to the current bindings.
    ///
    /// This is useful for position-aware mouse actions or other bespoke logic
    /// while still keeping the shared action queue.
    pub fn handle_with<F>(&mut self, event: InputEvent, translate: F) -> bool
    where
        F: FnOnce(InputEvent) -> Option<A>,
    {
        self.handle_with_from(event, ActionSource::Terminal, translate)
    }

    /// Translate an input event into a game action and queue it with explicit
    /// provenance. This keeps replayed or synthetic input events on the same
    /// path as real terminal input while preserving useful report metadata.
    pub fn handle_from(&mut self, event: InputEvent, source: ActionSource) -> bool {
        match event {
            InputEvent::Key { key, kind } => {
                match kind {
                    KeyEventKind::Press => {
                        self.held_key = Some((key, source));
                        self.held_time = 0.0;
                        self.last_repeat_time = 0.0;

                        if let Some(action) = self.bindings.translate(key) {
                            self.pending.push_back(QueuedAction::new(action, source));
                            self.total_queued += 1;
                            true
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
                    KeyEventKind::Repeat => {
                        // We ignore frontend repeats and use our own timer in tick()
                        // for consistent behavior across platforms.
                        false
                    }
                }
            }
            _ => {
                if let Some(action) = self.bindings.translate_event(event) {
                    self.pending.push_back(QueuedAction::new(action, source));
                    self.total_queued += 1;
                    true
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
            self.pending.push_back(QueuedAction::new(action, source));
            self.total_queued += 1;
            true
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
                self.pending.push_back(QueuedAction::new(action, source));
                self.total_queued += 1;
                count += 1;
            } else if let Some(action) = self.bindings.translate_event(event) {
                self.pending.push_back(QueuedAction::new(action, source));
                self.total_queued += 1;
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

    /// Inject a high-priority action at the front of the queue.
    ///
    /// Use this for interrupting or urgent actions that should be processed
    /// before any currently pending actions. The action still shares the same
    /// drain path and is applied through the same systems.
    pub fn inject_priority(&mut self, action: A) {
        self.inject_priority_from(action, ActionSource::Script);
    }

    /// Inject a high-priority action at the front of the queue with explicit
    /// provenance.
    pub fn inject_priority_from(&mut self, action: A, source: ActionSource) {
        self.pending.push_front(QueuedAction::new(action, source));
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
        let action = self.pending.pop_front();
        if let Some(ref act) = action {
            self.history.push(act.clone());
        }
        action
    }

    pub fn pop_action(&mut self) -> Option<QueuedAction<A>> {
        self.next_queued()
    }

    pub fn drain(&mut self) -> impl Iterator<Item = A> + '_ {
        self.history.extend(self.pending.iter().cloned());
        self.pending.drain(..).map(QueuedAction::into_action)
    }

    pub fn drain_queued(&mut self) -> vec_deque::Drain<'_, QueuedAction<A>> {
        self.history.extend(self.pending.iter().cloned());
        self.pending.drain(..)
    }

    /// Drain the pending queue into a replayable trace, preserving sources.
    pub fn drain_trace(&mut self) -> ActionTrace<A> {
        self.history.extend(self.pending.iter().cloned());
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
    ///
    /// Useful for detecting a completely fresh router or one that has been
    /// fully drained and unbound.
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
        let history = serde_json::from_str(&content)?;
        Ok(history)
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
    /// Useful for canceling queued actions when game state changes (for
    /// example, removing all movement actions when the player enters a menu).
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
    ///
    /// Like [`Self::filter_pending`] but returns the drained actions instead of
    /// just a count. Useful for logging canceled actions or re-routing them.
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
    ///
    /// This counter never decreases, even when actions are drained. Useful
    /// for metrics, debugging, and detecting whether any input has been
    /// processed.
    pub fn total_actions_queued(&self) -> usize {
        self.total_queued
    }
}

/// A text input buffer for terminal text entry (prompts, naming, chat, etc.).
///
/// Handles key events and produces a plain-text string. Supports cursor
/// movement, insertion, deletion, and a configurable max length, plus common
/// Ctrl shortcuts (A/E/B/F/U/W/K) for navigation and deletion.
///
/// This is separate from the action router because text entry is a continuous
/// editing state, not a discrete action. Games can render the buffer's current
/// value and cursor position each frame, then submit the final string when
/// the player confirms.
///
/// # Example
///
/// ```ignore
/// let mut input = TextInput::with_max(32);
/// input.handle_key(Key::Char('h'));
/// input.handle_key(Key::Char('i'));
/// assert_eq!(input.text(), "hi");
///
/// input.handle_key(Key::Backspace);
/// assert_eq!(input.text(), "h");
///
/// if input.handle_key(Key::Enter) {
///     let submitted = input.take_text();
///     // use submitted text...
/// }
/// ```
#[derive(Clone, Debug)]
pub struct TextInput {
    text: String,
    cursor: usize,
    max_len: usize,
    dirty: bool,
    history: Vec<String>,
    history_index: Option<usize>,
    max_history: usize,
    undo_stack: Vec<(String, usize)>,
    redo_stack: Vec<(String, usize)>,
    autocomplete_matches: Option<Vec<String>>,
    autocomplete_index: usize,
    original_text_before_autocomplete: String,
    autocomplete_start_char: usize,
    autocomplete_end_char: usize,
}

impl TextInput {
    /// Create a text input with no maximum length.
    pub fn new() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            max_len: usize::MAX,
            dirty: false,
            history: Vec::new(),
            history_index: None,
            max_history: 50,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            autocomplete_matches: None,
            autocomplete_index: 0,
            original_text_before_autocomplete: String::new(),
            autocomplete_start_char: 0,
            autocomplete_end_char: 0,
        }
    }

    /// Create a text input with a maximum character length.
    pub fn with_max(max_len: usize) -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            max_len,
            dirty: false,
            history: Vec::new(),
            history_index: None,
            max_history: 50,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            autocomplete_matches: None,
            autocomplete_index: 0,
            original_text_before_autocomplete: String::new(),
            autocomplete_start_char: 0,
            autocomplete_end_char: 0,
        }
    }

    /// Set the maximum number of history entries to keep.
    pub fn with_max_history(mut self, max: usize) -> Self {
        self.max_history = max;
        self
    }

    /// Handle a key event. Returns `true` if the input was submitted (Enter pressed).
    pub fn handle_key(&mut self, key: Key) -> bool {
        if key != Key::Tab {
            self.autocomplete_matches = None;
        }
        match key {
            Key::Char(ch) => {
                if ch.is_control() || ch == '\n' || ch == '\r' {
                    return false;
                }
                if self.text.chars().count() >= self.max_len {
                    return false;
                }
                self.record_state();
                let byte_pos = self.char_to_byte(self.cursor);
                self.text.insert(byte_pos, ch);
                self.cursor += 1;
                self.dirty = true;
                false
            }
            Key::Modified {
                char, ctrl: true, ..
            } => {
                let ch = char.to_ascii_lowercase();
                match ch {
                    'a' if self.cursor != 0 => {
                        self.cursor = 0;
                        self.dirty = true;
                    }
                    'e' => {
                        let len = self.text.chars().count();
                        if self.cursor != len {
                            self.cursor = len;
                            self.dirty = true;
                        }
                    }
                    'b' if self.cursor > 0 => {
                        self.cursor -= 1;
                        self.dirty = true;
                    }
                    'f' if self.cursor < self.text.chars().count() => {
                        self.cursor += 1;
                        self.dirty = true;
                    }
                    'u' if self.cursor > 0 => {
                        self.record_state();
                        self.delete_to_start();
                    }
                    'k' if self.cursor < self.text.chars().count() => {
                        self.record_state();
                        self.delete_to_end();
                    }
                    'w' if self.cursor > 0 => {
                        self.record_state();
                        self.delete_word_left();
                    }
                    'z' => {
                        self.undo();
                    }
                    'y' | 'r' => {
                        self.redo();
                    }
                    '←' => {
                        self.cursor = self.word_start_left();
                        self.dirty = true;
                    }
                    '→' => {
                        self.cursor = self.word_start_right();
                        self.dirty = true;
                    }
                    _ => {}
                }
                false
            }
            Key::Backspace => {
                if self.cursor > 0 {
                    self.record_state();
                    let byte_pos = self.char_to_byte(self.cursor);
                    let prev = self.text[..byte_pos]
                        .char_indices()
                        .next_back()
                        .map(|(i, _c)| i)
                        .unwrap_or(byte_pos);
                    self.text.drain(prev..byte_pos);
                    self.cursor -= 1;
                    self.dirty = true;
                }
                false
            }
            Key::Delete => {
                if self.cursor < self.text.chars().count() {
                    self.record_state();
                    let byte_pos = self.char_to_byte(self.cursor);
                    let char_len = self.text[byte_pos..]
                        .chars()
                        .next()
                        .map(|c| c.len_utf8())
                        .unwrap_or(0);
                    self.text.drain(byte_pos..byte_pos + char_len);
                    self.dirty = true;
                }
                false
            }
            Key::Left => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.dirty = true;
                }
                false
            }
            Key::Right => {
                if self.cursor < self.text.chars().count() {
                    self.cursor += 1;
                    self.dirty = true;
                }
                false
            }
            Key::Home => {
                if self.cursor > 0 {
                    self.cursor = 0;
                    self.dirty = true;
                }
                false
            }
            Key::End => {
                let len = self.text.chars().count();
                if self.cursor < len {
                    self.cursor = len;
                    self.dirty = true;
                }
                false
            }
            Key::Enter => {
                if !self.text.is_empty() {
                    self.history.push(self.text.clone());
                    if self.history.len() > self.max_history {
                        self.history.remove(0);
                    }
                }
                self.history_index = None;
                true
            }
            Key::Up => {
                if self.history.is_empty() {
                    return false;
                }
                if self.history_index.is_none() {
                    self.history_index = Some(self.history.len());
                }
                if let Some(idx) = self.history_index {
                    if idx > 0 {
                        let new_idx = idx - 1;
                        self.history_index = Some(new_idx);
                        self.set_text(self.history[new_idx].clone());
                    }
                }
                false
            }
            Key::Down => {
                if let Some(idx) = self.history_index {
                    if idx + 1 < self.history.len() {
                        let new_idx = idx + 1;
                        self.history_index = Some(new_idx);
                        self.set_text(self.history[new_idx].clone());
                    } else {
                        self.history_index = None;
                        self.clear();
                    }
                }
                false
            }
            Key::Esc => {
                if !self.text.is_empty() {
                    self.record_state();
                }
                self.history_index = None;
                self.text.clear();
                self.cursor = 0;
                self.dirty = true;
                false
            }
            _ => false,
        }
    }

    /// Handle an InputEvent. Only Key events are processed.
    /// Returns `true` if the input was submitted (Enter pressed).
    pub fn handle_event(&mut self, event: InputEvent) -> bool {
        if let InputEvent::Key { key, kind } = event {
            if matches!(kind, KeyEventKind::Press | KeyEventKind::Repeat) {
                return self.handle_key(key);
            }
        }
        false
    }

    /// Get the current text content.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Take ownership of the text and reset the buffer.
    pub fn take_text(&mut self) -> String {
        let text = std::mem::take(&mut self.text);
        self.cursor = 0;
        self.dirty = false;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.autocomplete_matches = None;
        text
    }

    /// Set the text content directly (e.g., for pre-filling or programmatic edits).
    pub fn set_text(&mut self, text: String) {
        self.record_state();
        self.text = text;
        self.cursor = self.text.chars().count().min(self.max_len);
        // Trim if over max.
        if self.text.chars().count() > self.max_len {
            self.text = self.text.chars().take(self.max_len).collect();
            self.cursor = self.max_len;
        }
        self.autocomplete_matches = None;
        self.dirty = true;
    }

    /// Insert a string at the current cursor position.
    ///
    /// Truncates the inserted text if it would exceed the maximum length.
    /// The cursor advances by the number of characters actually inserted.
    pub fn insert_str(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let current_len = self.text.chars().count();
        if current_len >= self.max_len {
            return;
        }
        let available = self.max_len - current_len;
        let to_insert: String = text.chars().take(available).collect();
        if to_insert.is_empty() {
            return;
        }
        self.record_state();
        let byte_pos = self.char_to_byte(self.cursor);
        self.text.insert_str(byte_pos, &to_insert);
        self.cursor += to_insert.chars().count();
        self.autocomplete_matches = None;
        self.dirty = true;
    }

    /// Get the current cursor position (in character units, not bytes).
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Set the cursor position (in character units). Clamped to valid range.
    pub fn set_cursor(&mut self, pos: usize) {
        let max = self.text.chars().count();
        self.cursor = pos.min(max);
        self.autocomplete_matches = None;
        self.dirty = true;
    }

    /// Get the maximum length.
    pub fn max_len(&self) -> usize {
        self.max_len
    }

    /// Check if the buffer has been modified since the last clear_dirty call.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Clear the dirty flag.
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    /// Check if the input is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        if !self.text.is_empty() {
            self.record_state();
        }
        self.text.clear();
        self.cursor = 0;
        self.autocomplete_matches = None;
        self.dirty = true;
    }

    /// Cycle through autocomplete matches for the word before the cursor.
    pub fn cycle_autocomplete(&mut self, dictionary: &[&str]) {
        if let Some(ref matches) = self.autocomplete_matches {
            if matches.is_empty() {
                self.autocomplete_matches = None;
                return;
            }
            self.autocomplete_index = (self.autocomplete_index + 1) % matches.len();
        } else {
            let cursor_char = self.cursor;
            let chars: Vec<char> = self.text.chars().collect();
            let mut start_char = cursor_char;
            while start_char > 0 && !chars[start_char - 1].is_whitespace() {
                start_char -= 1;
            }
            let prefix: String = chars[start_char..cursor_char].iter().collect();
            if prefix.is_empty() {
                return;
            }

            let matches: Vec<String> = dictionary
                .iter()
                .filter(|word| word.starts_with(&prefix))
                .map(|word| word.to_string())
                .collect();

            if matches.is_empty() {
                return;
            }

            self.original_text_before_autocomplete = self.text.clone();
            self.autocomplete_matches = Some(matches);
            self.autocomplete_index = 0;
            self.autocomplete_start_char = start_char;
            self.autocomplete_end_char = cursor_char;
        }

        let matches = self.autocomplete_matches.as_ref().unwrap();
        let match_word = &matches[self.autocomplete_index];

        let orig_chars: Vec<char> = self.original_text_before_autocomplete.chars().collect();
        let mut new_chars = Vec::new();
        new_chars.extend_from_slice(&orig_chars[..self.autocomplete_start_char]);
        new_chars.extend(match_word.chars());
        new_chars.extend_from_slice(&orig_chars[self.autocomplete_end_char..]);

        self.text = new_chars.into_iter().collect();
        self.cursor = self.autocomplete_start_char + match_word.chars().count();
        if self.text.chars().count() > self.max_len {
            self.text = self.text.chars().take(self.max_len).collect();
            self.cursor = self.cursor.min(self.max_len);
        }
        self.dirty = true;
    }

    /// Get the number of history entries.
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Get a history entry by index (0 = oldest).
    pub fn history_get(&self, index: usize) -> Option<&str> {
        self.history.get(index).map(|s| s.as_str())
    }

    /// Clear the history.
    pub fn clear_history(&mut self) {
        self.history.clear();
        self.history_index = None;
    }

    fn delete_range(&mut self, start: usize, end: usize) {
        let len = self.text.chars().count();
        let start = start.min(len);
        let end = end.min(len);
        if start >= end {
            return;
        }
        let start_byte = self.char_to_byte(start);
        let end_byte = self.char_to_byte(end);
        self.text.drain(start_byte..end_byte);
        if self.cursor >= end {
            self.cursor -= end - start;
        } else if self.cursor > start {
            self.cursor = start;
        }
        self.dirty = true;
    }

    fn delete_to_start(&mut self) {
        if self.cursor > 0 {
            let cursor = self.cursor;
            self.delete_range(0, cursor);
        }
    }

    fn delete_to_end(&mut self) {
        let len = self.text.chars().count();
        if self.cursor < len {
            let cursor = self.cursor;
            self.delete_range(cursor, len);
        }
    }

    fn delete_word_left(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let start = self.word_start_left();
        let cursor = self.cursor;
        self.delete_range(start, cursor);
    }

    fn word_start_left(&self) -> usize {
        let chars: Vec<char> = self.text.chars().collect();
        let mut idx = self.cursor.min(chars.len());
        while idx > 0 && chars[idx - 1].is_whitespace() {
            idx -= 1;
        }
        while idx > 0 && !chars[idx - 1].is_whitespace() {
            idx -= 1;
        }
        idx
    }

    fn word_start_right(&self) -> usize {
        let chars: Vec<char> = self.text.chars().collect();
        let len = chars.len();
        let mut idx = self.cursor;
        while idx < len && !chars[idx].is_whitespace() {
            idx += 1;
        }
        while idx < len && chars[idx].is_whitespace() {
            idx += 1;
        }
        idx
    }

    fn char_to_byte(&self, char_index: usize) -> usize {
        self.text
            .char_indices()
            .nth(char_index)
            .map(|(i, _)| i)
            .unwrap_or(self.text.len())
    }

    /// Record the current text and cursor state to the undo stack.
    /// This also clears the redo stack.
    pub fn record_state(&mut self) {
        let state = (self.text.clone(), self.cursor);
        if self.undo_stack.last() != Some(&state) {
            self.undo_stack.push(state);
            self.redo_stack.clear();
        }
    }

    /// Undo the last modification. Returns `true` if successful.
    pub fn undo(&mut self) -> bool {
        if let Some(prev) = self.undo_stack.pop() {
            let current = (self.text.clone(), self.cursor);
            self.redo_stack.push(current);
            self.text = prev.0;
            self.cursor = prev.1;
            self.dirty = true;
            true
        } else {
            false
        }
    }

    /// Redo the last undone modification. Returns `true` if successful.
    pub fn redo(&mut self) -> bool {
        if let Some(next) = self.redo_stack.pop() {
            let current = (self.text.clone(), self.cursor);
            self.undo_stack.push(current);
            self.text = next.0;
            self.cursor = next.1;
            self.dirty = true;
            true
        } else {
            false
        }
    }
}

impl Default for TextInput {
    fn default() -> Self {
        Self::new()
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
    /// Returns `true` if an action was injected, `false` if the end of the trace was reached.
    pub fn step(&mut self, router: &mut InputRouter<A>) -> bool {
        if let Some(step) = self.trace.steps.get(self.index) {
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
        self.index >= self.trace.steps.len()
    }

    pub fn current_step(&self) -> usize {
        self.index
    }

    pub fn total_steps(&self) -> usize {
        self.trace.steps.len()
    }

    pub fn reset(&mut self) {
        self.index = 0;
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
    fn test_key_repeat_config_and_ticking() {
        let mut router = bound_router();
        router.set_repeat_config(RepeatConfig {
            delay: 0.1,
            interval: 0.05,
        });

        // Key Press
        router.handle_from(
            InputEvent::Key {
                key: Key::Up,
                kind: KeyEventKind::Press,
            },
            ActionSource::Terminal,
        );

        // First action is immediately queued
        assert_eq!(router.total_actions_queued(), 1);
        assert_eq!(router.next_action(), Some(Move::North));

        // Advance by 0.05s (less than delay) -> no extra repeat
        router.tick(0.05);
        assert_eq!(router.total_actions_queued(), 1); // no action added yet

        // Advance by another 0.06s (total 0.11s, >= delay 0.1s, interval >= 0.05s) -> 1 action repeated
        router.tick(0.06);
        assert_eq!(router.total_actions_queued(), 2);
        assert_eq!(router.next_action(), Some(Move::North));

        // Advance by another 0.06s -> another action repeated
        router.tick(0.06);
        assert_eq!(router.total_actions_queued(), 3);
        assert_eq!(router.next_action(), Some(Move::North));

        // Release Key
        router.handle_from(
            InputEvent::Key {
                key: Key::Up,
                kind: KeyEventKind::Release,
            },
            ActionSource::Terminal,
        );

        // Advance by 0.1s -> no actions queued since key was released
        router.tick(0.1);
        assert_eq!(router.total_actions_queued(), 3);
    }

    #[test]
    fn test_router_history_tracking() {
        let mut router = bound_router();
        assert!(router.history().is_empty());

        router.inject_from(Move::North, ActionSource::Terminal);
        router.inject_from(Move::Wait, ActionSource::Script);
        assert!(router.history().is_empty()); // not popped yet

        let a1 = router.next_queued().unwrap();
        assert_eq!(a1.action, Move::North);
        assert_eq!(router.history().len(), 1);
        assert_eq!(router.history()[0].action, Move::North);

        let a2 = router.next_queued().unwrap();
        assert_eq!(a2.action, Move::Wait);
        assert_eq!(router.history().len(), 2);
        assert_eq!(router.history()[1].action, Move::Wait);

        router.clear_history();
        assert!(router.history().is_empty());
    }

    #[test]
    fn action_trace_serialization_round_trip() {
        let mut trace = ActionTrace::new();
        trace.push(Move::North, ActionSource::Terminal);
        trace.push(Move::Wait, ActionSource::Script);
        trace.push(Move::Scan(3), ActionSource::Agent);

        let format_move = |m: &Move| match m {
            Move::North => "north".to_owned(),
            Move::South => "south".to_owned(),
            Move::East => "east".to_owned(),
            Move::West => "west".to_owned(),
            Move::Wait => "wait".to_owned(),
            Move::Scan(r) => format!("scan:{}", r),
        };

        let parse_move = |s: &str| match s {
            "north" => Some(Move::North),
            "south" => Some(Move::South),
            "east" => Some(Move::East),
            "west" => Some(Move::West),
            "wait" => Some(Move::Wait),
            other => {
                if let Some(r_str) = other.strip_prefix("scan:") {
                    r_str.parse::<u16>().ok().map(Move::Scan)
                } else {
                    None
                }
            }
        };

        let serialized = trace.to_detailed_string(format_move);
        assert_eq!(serialized, "Terminal:north\nScript:wait\nAgent:scan:3\n");

        let deserialized = ActionTrace::from_detailed_string(&serialized, parse_move).unwrap();
        assert_eq!(deserialized, trace);

        // Check comments and blank lines are ignored
        let comment_str = "# this is a comment\n\nTerminal:north\n  # inner comment\nScript:wait\n";
        let parsed_comments = ActionTrace::from_detailed_string(comment_str, parse_move).unwrap();
        assert_eq!(parsed_comments.len(), 2);
        assert_eq!(parsed_comments.into_steps()[0].action, Move::North);
    }

    #[test]
    fn action_trace_file_round_trip() {
        let mut trace = ActionTrace::new();
        trace.push(Move::South, ActionSource::Terminal);
        trace.push(Move::Wait, ActionSource::Script);

        let format_move = |m: &Move| match m {
            Move::North => "north".to_owned(),
            Move::South => "south".to_owned(),
            Move::East => "east".to_owned(),
            Move::West => "west".to_owned(),
            Move::Wait => "wait".to_owned(),
            Move::Scan(r) => format!("scan:{}", r),
        };

        let parse_move = |s: &str| match s {
            "north" => Some(Move::North),
            "south" => Some(Move::South),
            "east" => Some(Move::East),
            "west" => Some(Move::West),
            "wait" => Some(Move::Wait),
            other => {
                if let Some(r_str) = other.strip_prefix("scan:") {
                    r_str.parse::<u16>().ok().map(Move::Scan)
                } else {
                    None
                }
            }
        };

        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_action_trace.txt");

        trace.save_to_file(&path, format_move).unwrap();
        let loaded = ActionTrace::<Move>::load_from_file(&path, parse_move).unwrap();
        assert_eq!(loaded, trace);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn key_event_translates_to_action() {
        let mut router = bound_router();
        assert!(router.handle(InputEvent::Key {
            key: Key::Up,
            kind: KeyEventKind::Press
        }));
        assert_eq!(router.next_action(), Some(Move::North));
        assert!(router.is_idle());
    }

    #[test]
    fn unbound_key_is_dropped() {
        let mut router = bound_router();
        assert!(!router.handle(InputEvent::Key {
            key: Key::Char('z'),
            kind: KeyEventKind::Press
        }));
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
        assert!(!router.handle(InputEvent::MouseScroll {
            x: 1,
            y: 1,
            direction: ScrollDirection::Up,
        }));
        assert!(router.is_idle());
    }

    #[test]
    fn is_empty_checks_both_bindings_and_pending() {
        let router: InputRouter<Move> = InputRouter::new(Bindings::new());
        assert!(router.is_empty());

        let mut router = bound_router();
        assert!(!router.is_empty()); // has bindings

        router.bindings_mut().unbind(Key::Up);
        router.bindings_mut().unbind(Key::Down);
        router.bindings_mut().unbind(Key::Left);
        router.bindings_mut().unbind(Key::Right);
        router.bindings_mut().unbind(Key::Char('.'));
        assert!(router.is_empty()); // no bindings, no pending

        router.inject(Move::Wait);
        assert!(!router.is_empty()); // has pending
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
    fn scroll_bindings_enter_the_same_action_queue() {
        let mut router = bound_router();
        router
            .bindings_mut()
            .bind_scroll(ScrollDirection::Down, Move::Scan(1));

        assert!(router.handle(InputEvent::MouseScroll {
            x: 2,
            y: 3,
            direction: ScrollDirection::Down,
        }));
        assert_eq!(
            router.next_queued(),
            Some(QueuedAction::new(Move::Scan(1), ActionSource::Terminal))
        );
    }

    #[test]
    fn handle_with_prefers_custom_translation() {
        let mut router = bound_router();
        let handled = router.handle_with(
            InputEvent::Key {
                key: Key::Up,
                kind: KeyEventKind::Press,
            },
            |_| Some(Move::Scan(2)),
        );
        assert!(handled);
        assert_eq!(
            router.next_queued(),
            Some(QueuedAction::new(Move::Scan(2), ActionSource::Terminal))
        );
    }

    #[test]
    fn handle_with_falls_back_to_bindings() {
        let mut router = bound_router();
        let handled = router.handle_with(
            InputEvent::Key {
                key: Key::Right,
                kind: KeyEventKind::Press,
            },
            |_| None,
        );
        assert!(handled);
        assert_eq!(
            router.next_queued(),
            Some(QueuedAction::new(Move::East, ActionSource::Terminal))
        );
    }

    #[test]
    fn input_events_can_be_queued_with_explicit_source() {
        let mut router = bound_router();
        assert!(router.handle_from(
            InputEvent::Key {
                key: Key::Right,
                kind: KeyEventKind::Press
            },
            ActionSource::Replay
        ));
        assert_eq!(
            router.next_queued(),
            Some(QueuedAction::new(Move::East, ActionSource::Replay))
        );
    }

    #[test]
    fn injected_actions_share_queue_with_translated_events() {
        let mut router = bound_router();
        router.handle(InputEvent::Key {
            key: Key::Up,
            kind: KeyEventKind::Press,
        });
        router.inject(Move::Wait);
        router.handle(InputEvent::Key {
            key: Key::Right,
            kind: KeyEventKind::Press,
        });
        let drained: Vec<Move> = router.drain().collect();
        assert_eq!(drained, vec![Move::North, Move::Wait, Move::East]);
    }

    #[test]
    fn queued_actions_track_source_without_changing_order() {
        let mut router = bound_router();
        router.handle(InputEvent::Key {
            key: Key::Up,
            kind: KeyEventKind::Press,
        });
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
    fn drain_trace_preserves_sources_and_clears_queue() {
        let mut router = bound_router();
        router.handle(InputEvent::Key {
            key: Key::Up,
            kind: KeyEventKind::Press,
        });
        router.inject_from(Move::Wait, ActionSource::Agent);
        router.handle_from(
            InputEvent::Key {
                key: Key::Right,
                kind: KeyEventKind::Press,
            },
            ActionSource::Replay,
        );

        let trace = router.drain_trace();
        assert!(router.is_idle());

        let steps = trace.into_steps();
        assert_eq!(
            steps,
            vec![
                QueuedAction::new(Move::North, ActionSource::Terminal),
                QueuedAction::new(Move::Wait, ActionSource::Agent),
                QueuedAction::new(Move::East, ActionSource::Replay),
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
        router.handle(InputEvent::Key {
            key: Key::Up,
            kind: KeyEventKind::Press,
        });
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
        router.handle(InputEvent::Key {
            key: Key::Right,
            kind: KeyEventKind::Press,
        });

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
            InputEvent::Key {
                key: Key::Up,
                kind: KeyEventKind::Press,
            },
            InputEvent::Key {
                key: Key::Right,
                kind: KeyEventKind::Press,
            },
            InputEvent::Key {
                key: Key::Down,
                kind: KeyEventKind::Press,
            },
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
            InputEvent::Key {
                key: Key::Up,
                kind: KeyEventKind::Press,
            },
            InputEvent::Key {
                key: Key::Char('z'),
                kind: KeyEventKind::Press,
            },
            InputEvent::Tick,
            InputEvent::Key {
                key: Key::Left,
                kind: KeyEventKind::Press,
            },
        ]);
        assert_eq!(count, 2);
        assert_eq!(router.pending(), 2);
    }

    #[test]
    fn handle_batch_from_preserves_source() {
        let mut router = bound_router();
        router.handle_batch_from(
            [
                InputEvent::Key {
                    key: Key::Up,
                    kind: KeyEventKind::Press,
                },
                InputEvent::Key {
                    key: Key::Right,
                    kind: KeyEventKind::Press,
                },
            ],
            ActionSource::Agent,
        );
        let queued: Vec<QueuedAction<Move>> = router.drain_queued().collect();
        assert_eq!(queued.len(), 2);
        assert!(queued.iter().all(|q| q.source == ActionSource::Agent));
    }

    #[test]
    fn handle_batch_with_prefers_custom_translation() {
        let mut router = bound_router();
        let count = router.handle_batch_with(
            [
                InputEvent::Key {
                    key: Key::Up,
                    kind: KeyEventKind::Press,
                },
                InputEvent::Key {
                    key: Key::Right,
                    kind: KeyEventKind::Press,
                },
            ],
            |event| match event {
                InputEvent::Key {
                    key: Key::Up,
                    kind: KeyEventKind::Press,
                } => Some(Move::Scan(2)),
                _ => None,
            },
        );
        assert_eq!(count, 2);
        assert_eq!(
            router.drain_queued().collect::<Vec<_>>(),
            vec![
                QueuedAction::new(Move::Scan(2), ActionSource::Terminal),
                QueuedAction::new(Move::East, ActionSource::Terminal),
            ]
        );
    }

    #[test]
    fn handle_batch_with_from_preserves_source() {
        let mut router = bound_router();
        let count = router.handle_batch_with_from(
            [
                InputEvent::Key {
                    key: Key::Up,
                    kind: KeyEventKind::Press,
                },
                InputEvent::Key {
                    key: Key::Down,
                    kind: KeyEventKind::Press,
                },
            ],
            ActionSource::Replay,
            |_| None,
        );
        assert_eq!(count, 2);
        let queued: Vec<QueuedAction<Move>> = router.drain_queued().collect();
        assert!(queued.iter().all(|q| q.source == ActionSource::Replay));
    }

    #[test]
    fn set_bindings_swaps_keymap_and_returns_old() {
        let mut router = bound_router();
        assert!(router.handle(InputEvent::Key {
            key: Key::Up,
            kind: KeyEventKind::Press
        }));
        assert_eq!(router.next_action(), Some(Move::North));

        let mut menu_bindings = Bindings::new();
        menu_bindings.bind(Key::Enter, Move::Wait);
        menu_bindings.bind(Key::Esc, Move::Wait);

        let old = router.set_bindings(menu_bindings);
        assert!(old.translate(Key::Up).is_some());
        assert!(!router.handle(InputEvent::Key {
            key: Key::Up,
            kind: KeyEventKind::Press
        }));
        assert!(router.handle(InputEvent::Key {
            key: Key::Enter,
            kind: KeyEventKind::Press
        }));
        assert_eq!(router.next_action(), Some(Move::Wait));

        router.set_bindings(old);
        assert!(router.handle(InputEvent::Key {
            key: Key::Up,
            kind: KeyEventKind::Press
        }));
        assert_eq!(router.next_action(), Some(Move::North));
    }

    #[test]
    fn bindings_guard_restores_on_drop() {
        let mut router = bound_router();
        let mut menu_bindings = Bindings::new();
        menu_bindings.bind(Key::Enter, Move::Wait);

        // Verify original bindings work.
        assert!(router.handle(InputEvent::Key {
            key: Key::Up,
            kind: KeyEventKind::Press
        }));
        assert_eq!(router.next_action(), Some(Move::North));

        {
            let mut guard = router.bindings_guard(menu_bindings);
            assert!(guard.handle(InputEvent::Key {
                key: Key::Enter,
                kind: KeyEventKind::Press
            }));
            assert_eq!(guard.next_action(), Some(Move::Wait));
            assert!(!guard.handle(InputEvent::Key {
                key: Key::Up,
                kind: KeyEventKind::Press
            }));
        }

        // Verify original bindings are restored.
        assert!(router.handle(InputEvent::Key {
            key: Key::Up,
            kind: KeyEventKind::Press
        }));
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

        assert!(router.handle(InputEvent::Key {
            key: Key::Up,
            kind: KeyEventKind::Press
        }));
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

        router.handle(InputEvent::Key {
            key: Key::Up,
            kind: KeyEventKind::Press,
        });
        assert_eq!(router.total_actions_queued(), 1);

        router.inject(Move::Wait);
        assert_eq!(router.total_actions_queued(), 2);

        router.inject_all([Move::East, Move::South]);
        assert_eq!(router.total_actions_queued(), 4);

        // Draining does not decrease the counter.
        let _: Vec<Move> = router.drain().collect();
        assert_eq!(router.total_actions_queued(), 4);

        router.handle(InputEvent::Key {
            key: Key::Down,
            kind: KeyEventKind::Press,
        });
        assert_eq!(router.total_actions_queued(), 5);
    }

    #[test]
    fn text_input_accepts_characters() {
        let mut input = TextInput::new();
        input.handle_key(Key::Char('h'));
        input.handle_key(Key::Char('i'));
        assert_eq!(input.text(), "hi");
    }

    #[test]
    fn text_input_respects_max_length() {
        let mut input = TextInput::with_max(3);
        input.handle_key(Key::Char('a'));
        input.handle_key(Key::Char('b'));
        input.handle_key(Key::Char('c'));
        input.handle_key(Key::Char('d'));
        assert_eq!(input.text(), "abc");
    }

    #[test]
    fn text_input_backspace_deletes_before_cursor() {
        let mut input = TextInput::new();
        input.handle_key(Key::Char('a'));
        input.handle_key(Key::Char('b'));
        input.handle_key(Key::Char('c'));
        input.handle_key(Key::Backspace);
        assert_eq!(input.text(), "ab");
        assert_eq!(input.cursor(), 2);
    }

    #[test]
    fn text_input_backspace_at_start_does_nothing() {
        let mut input = TextInput::new();
        input.handle_key(Key::Char('a'));
        input.handle_key(Key::Backspace); // deletes 'a', cursor at 0
        assert_eq!(input.text(), "");
        input.handle_key(Key::Backspace); // does nothing, already at start
        assert_eq!(input.text(), "");
    }

    #[test]
    fn text_input_delete_deletes_at_cursor() {
        let mut input = TextInput::new();
        input.handle_key(Key::Char('a'));
        input.handle_key(Key::Char('b'));
        input.handle_key(Key::Char('c'));
        input.handle_key(Key::Left);
        input.handle_key(Key::Left);
        input.handle_key(Key::Delete);
        assert_eq!(input.text(), "ac");
    }

    #[test]
    fn text_input_cursor_movement() {
        let mut input = TextInput::new();
        input.handle_key(Key::Char('a'));
        input.handle_key(Key::Char('b'));
        input.handle_key(Key::Char('c'));
        assert_eq!(input.cursor(), 3);

        input.handle_key(Key::Left);
        assert_eq!(input.cursor(), 2);

        input.handle_key(Key::Right);
        assert_eq!(input.cursor(), 3);

        input.handle_key(Key::Home);
        assert_eq!(input.cursor(), 0);

        input.handle_key(Key::End);
        assert_eq!(input.cursor(), 3);
    }

    #[test]
    fn text_input_ctrl_navigation_moves_cursor() {
        let mut input = TextInput::new();
        input.set_text("hello".to_owned());
        assert_eq!(input.cursor(), 5);

        input.handle_key(Key::modified('b', true, false, false));
        assert_eq!(input.cursor(), 4);

        input.handle_key(Key::modified('f', true, false, false));
        assert_eq!(input.cursor(), 5);

        input.handle_key(Key::modified('a', true, false, false));
        assert_eq!(input.cursor(), 0);

        input.handle_key(Key::modified('e', true, false, false));
        assert_eq!(input.cursor(), 5);
    }

    #[test]
    fn text_input_ctrl_w_deletes_word_left() {
        let mut input = TextInput::new();
        input.set_text("hello world".to_owned());

        input.handle_key(Key::modified('w', true, false, false));
        assert_eq!(input.text(), "hello ");
        assert_eq!(input.cursor(), 6);
    }

    #[test]
    fn text_input_ctrl_u_deletes_to_start() {
        let mut input = TextInput::new();
        input.set_text("hello world".to_owned());
        input.set_cursor(5);

        input.handle_key(Key::modified('u', true, false, false));
        assert_eq!(input.text(), " world");
        assert_eq!(input.cursor(), 0);
    }

    #[test]
    fn text_input_ctrl_k_deletes_to_end() {
        let mut input = TextInput::new();
        input.set_text("hello world".to_owned());
        input.set_cursor(6);

        input.handle_key(Key::modified('k', true, false, false));
        assert_eq!(input.text(), "hello ");
        assert_eq!(input.cursor(), 6);
    }

    #[test]
    fn text_input_inserts_at_cursor() {
        let mut input = TextInput::new();
        input.handle_key(Key::Char('a'));
        input.handle_key(Key::Char('c'));
        input.handle_key(Key::Left);
        input.handle_key(Key::Char('b'));
        assert_eq!(input.text(), "abc");
    }

    #[test]
    fn text_input_enter_returns_true() {
        let mut input = TextInput::new();
        input.handle_key(Key::Char('h'));
        input.handle_key(Key::Char('i'));
        assert!(input.handle_key(Key::Enter));
    }

    #[test]
    fn text_input_take_text_clears_buffer() {
        let mut input = TextInput::new();
        input.handle_key(Key::Char('h'));
        input.handle_key(Key::Char('i'));
        let text = input.take_text();
        assert_eq!(text, "hi");
        assert_eq!(input.text(), "");
        assert_eq!(input.cursor(), 0);
    }

    #[test]
    fn text_input_esc_clears_all() {
        let mut input = TextInput::new();
        input.handle_key(Key::Char('h'));
        input.handle_key(Key::Char('i'));
        input.handle_key(Key::Esc);
        assert_eq!(input.text(), "");
        assert_eq!(input.cursor(), 0);
    }

    #[test]
    fn text_input_dirty_tracking() {
        let mut input = TextInput::new();
        assert!(!input.is_dirty());
        input.handle_key(Key::Char('a'));
        assert!(input.is_dirty());
        input.clear_dirty();
        assert!(!input.is_dirty());
    }

    #[test]
    fn text_input_set_text() {
        let mut input = TextInput::with_max(5);
        input.set_text("hello world".to_owned());
        assert_eq!(input.text(), "hello");
        assert_eq!(input.cursor(), 5);
    }

    #[test]
    fn text_input_is_empty() {
        let mut input = TextInput::new();
        assert!(input.is_empty());
        input.handle_key(Key::Char('a'));
        assert!(!input.is_empty());
    }

    #[test]
    fn text_input_clear() {
        let mut input = TextInput::new();
        input.handle_key(Key::Char('a'));
        input.clear();
        assert!(input.is_empty());
        assert_eq!(input.cursor(), 0);
    }

    #[test]
    fn text_input_handles_multibyte_characters() {
        let mut input = TextInput::new();
        input.handle_key(Key::Char('日'));
        input.handle_key(Key::Char('本'));
        input.handle_key(Key::Char('語'));
        assert_eq!(input.text(), "日本語");
        assert_eq!(input.cursor(), 3);

        input.handle_key(Key::Left);
        assert_eq!(input.cursor(), 2);
        input.handle_key(Key::Backspace);
        // Backspace at cursor 2 deletes "本" (position 1), leaving "日語"
        assert_eq!(input.text(), "日語");
        assert_eq!(input.cursor(), 1);
    }

    #[test]
    fn text_input_control_chars_ignored() {
        let mut input = TextInput::new();
        input.handle_key(Key::Char('\n'));
        input.handle_key(Key::Char('\r'));
        input.handle_key(Key::Char('\t'));
        assert_eq!(input.text(), "");
    }

    #[test]
    fn text_input_handle_event_only_processes_keys() {
        let mut input = TextInput::new();
        assert!(!input.handle_event(InputEvent::Tick));
        assert!(!input.handle_event(InputEvent::Resize {
            width: 80,
            height: 24
        }));
        assert!(!input.handle_event(InputEvent::Mouse {
            x: 0,
            y: 0,
            button: MouseButton::Left,
            pressed: true,
        }));
        assert_eq!(input.text(), "");
    }

    #[test]
    fn action_source_display_roundtrips() {
        assert_eq!(ActionSource::Terminal.to_string(), "Terminal");
        assert_eq!(ActionSource::Script.to_string(), "Script");
        assert_eq!(ActionSource::Agent.to_string(), "Agent");
        assert_eq!(ActionSource::Replay.to_string(), "Replay");
        assert_eq!(ActionSource::Test.to_string(), "Test");
    }

    #[test]
    fn action_source_from_str_parses() {
        assert_eq!(
            "Terminal".parse::<ActionSource>().unwrap(),
            ActionSource::Terminal
        );
        assert_eq!(
            "script".parse::<ActionSource>().unwrap(),
            ActionSource::Script
        );
        assert_eq!(
            "AGENT".parse::<ActionSource>().unwrap(),
            ActionSource::Agent
        );
    }

    #[test]
    fn action_source_case_insensitive_parsing() {
        assert_eq!(
            "terminal".parse::<ActionSource>().unwrap(),
            ActionSource::Terminal
        );
        assert_eq!(
            "SCRIPT".parse::<ActionSource>().unwrap(),
            ActionSource::Script
        );
        assert_eq!(
            "agent".parse::<ActionSource>().unwrap(),
            ActionSource::Agent
        );
        assert_eq!(
            "replay".parse::<ActionSource>().unwrap(),
            ActionSource::Replay
        );
        assert_eq!("TEST".parse::<ActionSource>().unwrap(), ActionSource::Test);
    }

    #[test]
    fn action_source_invalid_returns_error() {
        assert!("unknown".parse::<ActionSource>().is_err());
        assert!("".parse::<ActionSource>().is_err());
    }

    #[test]
    fn text_input_history_records_on_enter() {
        let mut input = TextInput::new();
        input.handle_key(Key::Char('h'));
        input.handle_key(Key::Char('i'));
        assert!(input.handle_key(Key::Enter));
        assert_eq!(input.history_len(), 1);
        assert_eq!(input.history_get(0), Some("hi"));
    }

    #[test]
    fn text_input_history_navigate_up() {
        let mut input = TextInput::new();
        input.set_text("first".to_owned());
        input.handle_key(Key::Enter);
        input.set_text("second".to_owned());
        input.handle_key(Key::Enter);

        input.handle_key(Key::Up);
        assert_eq!(input.text(), "second");
        input.handle_key(Key::Up);
        assert_eq!(input.text(), "first");
    }

    #[test]
    fn text_input_history_navigate_down() {
        let mut input = TextInput::new();
        input.set_text("first".to_owned());
        input.handle_key(Key::Enter);

        input.handle_key(Key::Up);
        assert_eq!(input.text(), "first");
        input.handle_key(Key::Down);
        assert!(input.is_empty());
    }

    #[test]
    fn text_input_empty_text_not_added_to_history() {
        let mut input = TextInput::new();
        assert!(input.handle_key(Key::Enter));
        assert_eq!(input.history_len(), 0);
    }

    #[test]
    fn text_input_history_respects_max() {
        let mut input = TextInput::with_max(10).with_max_history(2);
        input.set_text("a".to_owned());
        input.handle_key(Key::Enter);
        input.set_text("b".to_owned());
        input.handle_key(Key::Enter);
        input.set_text("c".to_owned());
        input.handle_key(Key::Enter);

        assert_eq!(input.history_len(), 2);
        assert_eq!(input.history_get(0), Some("b"));
        assert_eq!(input.history_get(1), Some("c"));
    }

    #[test]
    fn text_input_clear_history() {
        let mut input = TextInput::new();
        input.set_text("test".to_owned());
        input.handle_key(Key::Enter);
        assert_eq!(input.history_len(), 1);
        input.clear_history();
        assert_eq!(input.history_len(), 0);
    }

    #[test]
    fn text_input_insert_str_at_cursor() {
        let mut input = TextInput::new();
        input.handle_key(Key::Char('a'));
        input.handle_key(Key::Char('c'));
        input.handle_key(Key::Left);
        input.insert_str("b");
        assert_eq!(input.text(), "abc");
    }

    #[test]
    fn text_input_insert_str_respects_max_length() {
        let mut input = TextInput::with_max(5);
        input.set_text("hel".to_owned());
        input.insert_str("lo world");
        assert_eq!(input.text(), "hello");
    }

    #[test]
    fn text_input_insert_str_empty_is_noop() {
        let mut input = TextInput::new();
        input.handle_key(Key::Char('a'));
        input.insert_str("");
        assert_eq!(input.text(), "a");
    }

    #[test]
    fn text_input_insert_str_at_end() {
        let mut input = TextInput::new();
        input.set_text("hello".to_owned());
        input.insert_str("!");
        assert_eq!(input.text(), "hello!");
    }

    #[test]
    fn test_text_input_undo_redo() {
        let mut input = TextInput::new();
        input.handle_key(Key::Char('a'));
        input.handle_key(Key::Char('b'));
        input.handle_key(Key::Char('c'));
        assert_eq!(input.text(), "abc");

        // Undo last character insertion ('c')
        assert!(input.undo());
        assert_eq!(input.text(), "ab");

        // Undo 'b'
        assert!(input.undo());
        assert_eq!(input.text(), "a");

        // Undo 'a'
        assert!(input.undo());
        assert_eq!(input.text(), "");

        // No more undo
        assert!(!input.undo());

        // Redo 'a'
        assert!(input.redo());
        assert_eq!(input.text(), "a");

        // Redo 'b'
        assert!(input.redo());
        assert_eq!(input.text(), "ab");

        // Redo 'c'
        assert!(input.redo());
        assert_eq!(input.text(), "abc");

        // No more redo
        assert!(!input.redo());

        // Type 'd' -> should clear redo stack
        input.handle_key(Key::Char('d'));
        assert_eq!(input.text(), "abcd");
        assert!(!input.redo());

        // Test ctrl-z/ctrl-y keys via handle_key
        input.handle_key(Key::Modified {
            char: 'z',
            ctrl: true,
            alt: false,
            shift: false,
        });
        assert_eq!(input.text(), "abc");

        input.handle_key(Key::Modified {
            char: 'y',
            ctrl: true,
            alt: false,
            shift: false,
        });
        assert_eq!(input.text(), "abcd");

        // Test clear()/Esc undoable
        input.clear();
        assert_eq!(input.text(), "");
        assert!(input.undo());
        assert_eq!(input.text(), "abcd");
    }

    #[test]
    fn test_text_input_autocomplete() {
        let mut input = TextInput::new();
        input.set_text("run in".to_owned());

        let dict = vec!["inspect", "confirm", "info", "input", "init"];

        // Cycle 1: matches "inspect", "info", "input", "init". First match should be "inspect".
        input.cycle_autocomplete(&dict);
        assert_eq!(input.text(), "run inspect");
        assert_eq!(input.cursor(), 11);

        // Cycle 2: next match is "info"
        input.cycle_autocomplete(&dict);
        assert_eq!(input.text(), "run info");
        assert_eq!(input.cursor(), 8);

        // Cycle 3: next match is "input"
        input.cycle_autocomplete(&dict);
        assert_eq!(input.text(), "run input");
        assert_eq!(input.cursor(), 9);

        // Cycle 4: next match is "init"
        input.cycle_autocomplete(&dict);
        assert_eq!(input.text(), "run init");
        assert_eq!(input.cursor(), 8);

        // Cycle 5: wraps around back to "inspect"
        input.cycle_autocomplete(&dict);
        assert_eq!(input.text(), "run inspect");

        // Typing a character should break the autocomplete cycle
        input.handle_key(Key::Char('r'));
        assert_eq!(input.text(), "run inspectr");

        // Now if we hit tab/autocomplete again with "inspectr", no match.
        input.cycle_autocomplete(&dict);
        assert_eq!(input.text(), "run inspectr");
    }

    #[test]
    fn test_text_input_word_jumps() {
        let mut input = TextInput::new();
        input.set_text("hello brave new world".to_owned());
        assert_eq!(input.cursor(), 21); // at the end

        // Ctrl-Left to jump to "world" start
        input.handle_key(Key::Modified {
            char: '←',
            ctrl: true,
            alt: false,
            shift: false,
        });
        assert_eq!(input.cursor(), 16); // start of "world"

        // Ctrl-Left to jump to "new" start
        input.handle_key(Key::Modified {
            char: '←',
            ctrl: true,
            alt: false,
            shift: false,
        });
        assert_eq!(input.cursor(), 12); // start of "new"

        // Ctrl-Right to jump to start of next word "world"
        input.handle_key(Key::Modified {
            char: '→',
            ctrl: true,
            alt: false,
            shift: false,
        });
        assert_eq!(input.cursor(), 16); // start of "world"

        // Ctrl-Right to end (no next word)
        input.handle_key(Key::Modified {
            char: '→',
            ctrl: true,
            alt: false,
            shift: false,
        });
        assert_eq!(input.cursor(), 21);
    }

    #[test]
    fn inject_priority_puts_action_at_front() {
        let mut router = bound_router();
        router.inject(Move::North);
        router.inject(Move::South);
        router.inject_priority(Move::Wait);

        assert_eq!(router.pending(), 3);
        assert_eq!(router.next_action(), Some(Move::Wait));
        assert_eq!(router.next_action(), Some(Move::North));
        assert_eq!(router.next_action(), Some(Move::South));
    }

    #[test]
    fn inject_priority_from_preserves_source() {
        let mut router = bound_router();
        router.inject(Move::North);
        router.inject_priority_from(Move::Wait, ActionSource::Agent);

        let queued: Vec<QueuedAction<Move>> = router.drain_queued().collect();
        assert_eq!(
            queued,
            vec![
                QueuedAction::new(Move::Wait, ActionSource::Agent),
                QueuedAction::new(Move::North, ActionSource::Script),
            ]
        );
    }

    #[test]
    fn inject_priority_counts_toward_total() {
        let mut router = bound_router();
        router.inject(Move::North);
        router.inject_priority(Move::Wait);
        assert_eq!(router.total_actions_queued(), 2);
    }

    #[test]
    fn filter_pending_removes_matching_actions() {
        let mut router = bound_router();
        router.inject(Move::North);
        router.inject(Move::South);
        router.inject(Move::East);

        let removed = router.filter_pending(|qa| matches!(qa.action, Move::North | Move::South));
        assert_eq!(removed, 2);
        assert_eq!(router.pending(), 1);
        assert_eq!(router.next_action(), Some(Move::East));
    }

    #[test]
    fn filter_pending_preserves_order_of_remaining() {
        let mut router = bound_router();
        router.inject(Move::North);
        router.inject(Move::South);
        router.inject(Move::East);
        router.inject(Move::West);

        router.filter_pending(|qa| matches!(qa.action, Move::South));
        let drained: Vec<Move> = router.drain().collect();
        assert_eq!(drained, vec![Move::North, Move::East, Move::West]);
    }

    #[test]
    fn filter_pending_empty_queue_returns_zero() {
        let mut router = bound_router();
        let removed = router.filter_pending(|_| true);
        assert_eq!(removed, 0);
        assert!(router.is_idle());
    }

    #[test]
    fn bindings_iter_keys_yields_all_key_bindings() {
        let b = bound_router();
        let keys: Vec<Key> = b.bindings().iter_keys().map(|(k, _)| k).collect();
        assert_eq!(keys.len(), 5);
        assert!(keys.contains(&Key::Up));
        assert!(keys.contains(&Key::Down));
        assert!(keys.contains(&Key::Left));
        assert!(keys.contains(&Key::Right));
        assert!(keys.contains(&Key::Char('.')));
    }

    #[test]
    fn bindings_iter_mouse_yields_mouse_bindings() {
        let mut b = Bindings::new();
        b.bind_mouse(MouseButton::Left, true, Move::North);
        b.bind_mouse(MouseButton::Right, false, Move::South);
        let mouse: Vec<_> = b.iter_mouse().collect();
        assert_eq!(mouse.len(), 2);
    }

    #[test]
    fn command_bindings_iter_names_and_glyphs() {
        let mut c = CommandBindings::new();
        c.bind_name("north", Move::North);
        c.bind_name("south", Move::South);
        c.bind_glyph('e', Move::East);
        c.bind_glyph('w', Move::West);

        let names: Vec<&str> = c.iter_names().map(|(n, _)| n).collect();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"north"));
        assert!(names.contains(&"south"));

        let glyphs: Vec<char> = c.iter_glyphs().map(|(g, _)| g).collect();
        assert_eq!(glyphs.len(), 2);
        assert!(glyphs.contains(&'e'));
        assert!(glyphs.contains(&'w'));
    }

    #[test]
    fn push_bindings_saves_and_switches_context() {
        let mut router = bound_router();
        assert_eq!(router.context_depth(), 0);

        let mut menu = Bindings::new();
        menu.bind(Key::Enter, Move::Wait);
        router.push_bindings(menu);

        assert_eq!(router.context_depth(), 1);
        assert!(router.handle(InputEvent::Key {
            key: Key::Enter,
            kind: KeyEventKind::Press
        }));
        assert_eq!(router.next_action(), Some(Move::Wait));
        assert!(!router.handle(InputEvent::Key {
            key: Key::Up,
            kind: KeyEventKind::Press
        }));
    }

    #[test]
    fn pop_bindings_restores_previous_context() {
        let mut router = bound_router();

        let mut menu = Bindings::new();
        menu.bind(Key::Enter, Move::Wait);
        router.push_bindings(menu);

        assert!(router.pop_bindings());
        assert_eq!(router.context_depth(), 0);
        assert!(router.handle(InputEvent::Key {
            key: Key::Up,
            kind: KeyEventKind::Press
        }));
        assert_eq!(router.next_action(), Some(Move::North));
        assert!(!router.handle(InputEvent::Key {
            key: Key::Enter,
            kind: KeyEventKind::Press
        }));
    }

    #[test]
    fn pop_bindings_returns_false_on_empty_stack() {
        let mut router = bound_router();
        assert!(!router.pop_bindings());
        assert_eq!(router.context_depth(), 0);
    }

    #[test]
    fn nested_push_bindings_supports_multiple_levels() {
        let mut router = bound_router();

        let mut level1 = Bindings::new();
        level1.bind(Key::Char('a'), Move::North);
        router.push_bindings(level1);
        assert_eq!(router.context_depth(), 1);

        let mut level2 = Bindings::new();
        level2.bind(Key::Char('b'), Move::South);
        router.push_bindings(level2);
        assert_eq!(router.context_depth(), 2);

        // Level 2 is active.
        assert!(router.handle(InputEvent::Key {
            key: Key::Char('b'),
            kind: KeyEventKind::Press
        }));
        assert!(!router.handle(InputEvent::Key {
            key: Key::Char('a'),
            kind: KeyEventKind::Press
        }));

        // Pop to level 1.
        router.pop_bindings();
        assert!(router.handle(InputEvent::Key {
            key: Key::Char('a'),
            kind: KeyEventKind::Press
        }));
        assert!(!router.handle(InputEvent::Key {
            key: Key::Char('b'),
            kind: KeyEventKind::Press
        }));

        // Pop back to original.
        router.pop_bindings();
        assert!(router.handle(InputEvent::Key {
            key: Key::Up,
            kind: KeyEventKind::Press
        }));
        assert!(!router.handle(InputEvent::Key {
            key: Key::Char('a'),
            kind: KeyEventKind::Press
        }));
    }

    #[test]
    fn key_display() {
        assert_eq!(format!("{}", Key::Char('a')), "a");
        assert_eq!(format!("{}", Key::Enter), "Enter");
        assert_eq!(format!("{}", Key::F(5)), "F5");
        assert_eq!(
            format!("{}", Key::modified('x', true, false, false)),
            "Ctrl+x"
        );
        assert_eq!(
            format!("{}", Key::modified('a', true, true, false)),
            "Ctrl+Alt+a"
        );
    }

    #[test]
    fn mouse_button_display() {
        assert_eq!(format!("{}", MouseButton::Left), "Left");
        assert_eq!(format!("{}", MouseButton::Right), "Right");
    }

    #[test]
    fn scroll_direction_display() {
        assert_eq!(format!("{}", ScrollDirection::Up), "Up");
        assert_eq!(format!("{}", ScrollDirection::Down), "Down");
    }

    #[test]
    fn bindings_clear_removes_all() {
        let mut bindings = Bindings::new();
        bindings.bind(Key::Char('a'), 1);
        bindings.bind(Key::Char('b'), 2);
        bindings.bind_mouse(MouseButton::Left, true, 3);
        bindings.bind_scroll(ScrollDirection::Up, 4);
        assert_eq!(bindings.len(), 4);

        bindings.clear();
        assert!(bindings.is_empty());
        assert_eq!(bindings.len(), 0);
        assert_eq!(bindings.translate(Key::Char('a')), None);
    }

    #[test]
    fn command_bindings_clear_removes_all() {
        let mut cmds = CommandBindings::new();
        cmds.bind_name("north", 1);
        cmds.bind_glyph('n', 1);
        assert_eq!(cmds.name_count(), 1);
        assert_eq!(cmds.glyph_count(), 1);

        cmds.clear();
        assert_eq!(cmds.name_count(), 0);
        assert_eq!(cmds.glyph_count(), 0);
        assert_eq!(cmds.translate_name("north"), None);
        assert_eq!(cmds.translate_glyph('n'), None);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_roundtrip_action_source() {
        let sources = [
            ActionSource::Terminal,
            ActionSource::Script,
            ActionSource::Agent,
            ActionSource::Replay,
            ActionSource::Test,
        ];
        for source in sources {
            let json = serde_json::to_string(&source).unwrap();
            let roundtrip: ActionSource = serde_json::from_str(&json).unwrap();
            assert_eq!(source, roundtrip);
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_roundtrip_key() {
        let keys = [
            Key::Char('a'),
            Key::Enter,
            Key::Up,
            Key::F(5),
            Key::Modified {
                char: 'c',
                ctrl: true,
                alt: false,
                shift: false,
            },
        ];
        for key in keys {
            let json = serde_json::to_string(&key).unwrap();
            let roundtrip: Key = serde_json::from_str(&json).unwrap();
            assert_eq!(key, roundtrip);
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_roundtrip_input_event() {
        let events = [
            InputEvent::Key {
                key: Key::Char('x'),
                kind: KeyEventKind::Press,
            },
            InputEvent::Mouse {
                x: 10,
                y: 5,
                button: MouseButton::Left,
                pressed: true,
            },
            InputEvent::MouseScroll {
                x: 0,
                y: 0,
                direction: ScrollDirection::Up,
            },
            InputEvent::Resize {
                width: 80,
                height: 24,
            },
        ];
        for event in events {
            let json = serde_json::to_string(&event).unwrap();
            let roundtrip: InputEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(event, roundtrip);
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_roundtrip_bindings() {
        #[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
        enum DummyAction {
            MoveUp,
            Fire,
        }

        let mut bindings = Bindings::new();
        bindings.bind(Key::Char('w'), DummyAction::MoveUp);
        bindings.bind_mouse(MouseButton::Left, true, DummyAction::Fire);
        bindings.bind_scroll(ScrollDirection::Up, DummyAction::MoveUp);

        let json = serde_json::to_string(&bindings).unwrap();
        let roundtrip: Bindings<DummyAction> = serde_json::from_str(&json).unwrap();
        assert_eq!(
            roundtrip.translate(Key::Char('w')),
            Some(DummyAction::MoveUp)
        );
        assert_eq!(
            roundtrip.translate_mouse(MouseButton::Left, true),
            Some(DummyAction::Fire)
        );
        assert_eq!(
            roundtrip.translate_scroll(ScrollDirection::Up),
            Some(DummyAction::MoveUp)
        );

        let mut cmd_bindings = CommandBindings::new();
        cmd_bindings.bind_name("up", DummyAction::MoveUp);
        cmd_bindings.bind_glyph('f', DummyAction::Fire);

        let cmd_json = serde_json::to_string(&cmd_bindings).unwrap();
        let cmd_roundtrip: CommandBindings<DummyAction> = serde_json::from_str(&cmd_json).unwrap();
        assert_eq!(
            cmd_roundtrip.translate_name("up"),
            Some(DummyAction::MoveUp)
        );
        assert_eq!(cmd_roundtrip.translate_glyph('f'), Some(DummyAction::Fire));
    }
}

/// A buffer that can throttle actions based on per-action cooldowns.
#[derive(Clone, Debug)]
pub struct ActionBuffer<A: Clone + Eq + std::hash::Hash> {
    cooldowns: HashMap<A, f32>,
    timers: HashMap<A, f32>,
}

impl<A: Clone + Eq + std::hash::Hash> ActionBuffer<A> {
    pub fn new() -> Self {
        Self {
            cooldowns: HashMap::new(),
            timers: HashMap::new(),
        }
    }

    pub fn set_cooldown(&mut self, action: A, duration: f32) {
        self.cooldowns.insert(action, duration);
    }

    pub fn update(&mut self, dt: f32) {
        for timer in self.timers.values_mut() {
            *timer = (*timer - dt).max(0.0);
        }
    }

    pub fn can_perform(&self, action: &A) -> bool {
        self.timers.get(action).is_none_or(|&t| t <= 0.0)
    }

    pub fn perform(&mut self, action: A) -> bool {
        if self.can_perform(&action) {
            if let Some(&duration) = self.cooldowns.get(&action) {
                self.timers.insert(action, duration);
            }
            true
        } else {
            false
        }
    }

    pub fn remaining_cooldown(&self, action: &A) -> f32 {
        self.timers.get(action).cloned().unwrap_or(0.0)
    }

    pub fn clear_cooldowns(&mut self) {
        self.timers.clear();
    }
}

impl<A: Clone + Eq + std::hash::Hash> Default for ActionBuffer<A> {
    fn default() -> Self {
        Self::new()
    }
}
