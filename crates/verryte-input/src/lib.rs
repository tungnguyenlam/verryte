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
            InputEvent::Key(key) => self.translate(key),
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
    context_stack: Vec<Bindings<A>>,
    history: Vec<QueuedAction<A>>,
}

impl<A: Clone> InputRouter<A> {
    pub fn new(bindings: Bindings<A>) -> Self {
        Self {
            bindings,
            pending: VecDeque::new(),
            total_queued: 0,
            context_stack: Vec::new(),
            history: Vec::new(),
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
        if let Some(action) = self.bindings.translate_event(event) {
            self.pending.push_back(QueuedAction::new(action, source));
            self.total_queued += 1;
            true
        } else {
            false
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
        }
    }

    /// Set the maximum number of history entries to keep.
    pub fn with_max_history(mut self, max: usize) -> Self {
        self.max_history = max;
        self
    }

    /// Handle a key event. Returns `true` if the input was submitted (Enter pressed).
    pub fn handle_key(&mut self, key: Key) -> bool {
        match key {
            Key::Char(ch) => {
                if ch.is_control() || ch == '\n' || ch == '\r' {
                    return false;
                }
                if self.text.chars().count() >= self.max_len {
                    return false;
                }
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
                    'u' => self.delete_to_start(),
                    'k' => self.delete_to_end(),
                    'w' => self.delete_word_left(),
                    _ => {}
                }
                false
            }
            Key::Backspace => {
                if self.cursor > 0 {
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
        if let InputEvent::Key(key) = event {
            self.handle_key(key)
        } else {
            false
        }
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
        text
    }

    /// Set the text content directly (e.g., for pre-filling or programmatic edits).
    pub fn set_text(&mut self, text: String) {
        self.text = text;
        self.cursor = self.text.chars().count().min(self.max_len);
        // Trim if over max.
        if self.text.chars().count() > self.max_len {
            self.text = self.text.chars().take(self.max_len).collect();
            self.cursor = self.max_len;
        }
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
        let byte_pos = self.char_to_byte(self.cursor);
        self.text.insert_str(byte_pos, &to_insert);
        self.cursor += to_insert.chars().count();
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
        self.text.clear();
        self.cursor = 0;
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

    fn char_to_byte(&self, char_index: usize) -> usize {
        self.text
            .char_indices()
            .nth(char_index)
            .map(|(i, _)| i)
            .unwrap_or(self.text.len())
    }
}

impl Default for TextInput {
    fn default() -> Self {
        Self::new()
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
        let handled = router.handle_with(InputEvent::Key(Key::Up), |_| Some(Move::Scan(2)));
        assert!(handled);
        assert_eq!(
            router.next_queued(),
            Some(QueuedAction::new(Move::Scan(2), ActionSource::Terminal))
        );
    }

    #[test]
    fn handle_with_falls_back_to_bindings() {
        let mut router = bound_router();
        let handled = router.handle_with(InputEvent::Key(Key::Right), |_| None);
        assert!(handled);
        assert_eq!(
            router.next_queued(),
            Some(QueuedAction::new(Move::East, ActionSource::Terminal))
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
    fn drain_trace_preserves_sources_and_clears_queue() {
        let mut router = bound_router();
        router.handle(InputEvent::Key(Key::Up));
        router.inject_from(Move::Wait, ActionSource::Agent);
        router.handle_from(InputEvent::Key(Key::Right), ActionSource::Replay);

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
    fn handle_batch_with_prefers_custom_translation() {
        let mut router = bound_router();
        let count = router.handle_batch_with(
            [InputEvent::Key(Key::Up), InputEvent::Key(Key::Right)],
            |event| match event {
                InputEvent::Key(Key::Up) => Some(Move::Scan(2)),
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
            [InputEvent::Key(Key::Up), InputEvent::Key(Key::Down)],
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
        assert!(router.handle(InputEvent::Key(Key::Enter)));
        assert_eq!(router.next_action(), Some(Move::Wait));
        assert!(!router.handle(InputEvent::Key(Key::Up)));
    }

    #[test]
    fn pop_bindings_restores_previous_context() {
        let mut router = bound_router();

        let mut menu = Bindings::new();
        menu.bind(Key::Enter, Move::Wait);
        router.push_bindings(menu);

        assert!(router.pop_bindings());
        assert_eq!(router.context_depth(), 0);
        assert!(router.handle(InputEvent::Key(Key::Up)));
        assert_eq!(router.next_action(), Some(Move::North));
        assert!(!router.handle(InputEvent::Key(Key::Enter)));
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
        assert!(router.handle(InputEvent::Key(Key::Char('b'))));
        assert!(!router.handle(InputEvent::Key(Key::Char('a'))));

        // Pop to level 1.
        router.pop_bindings();
        assert!(router.handle(InputEvent::Key(Key::Char('a'))));
        assert!(!router.handle(InputEvent::Key(Key::Char('b'))));

        // Pop back to original.
        router.pop_bindings();
        assert!(router.handle(InputEvent::Key(Key::Up)));
        assert!(!router.handle(InputEvent::Key(Key::Char('a'))));
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
}
