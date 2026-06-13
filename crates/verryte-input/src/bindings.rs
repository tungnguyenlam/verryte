//! Keyboard/mouse/scroll bindings and command bindings.
//!
//! This module provides mappings from input events or script commands to
//! a generic game action type.
//!
//! # Example
//!
//! ```rust
//! use verryte_input::{Bindings, Key};
//!
//! #[derive(Clone, PartialEq, Debug)]
//! enum Action {
//!     Up,
//!     Down,
//!     Attack,
//! }
//!
//! let mut bindings = Bindings::<Action>::new();
//! bindings.bind(Key::Up, Action::Up);
//! bindings.bind(Key::Char('w'), Action::Up);
//! bindings.bind(Key::Enter, Action::Attack);
//!
//! assert_eq!(bindings.translate(Key::Up), Some(Action::Up));
//! assert_eq!(bindings.translate(Key::Char('x')), None);
//! ```

use std::collections::HashMap;

use crate::key::{InputEvent, Key, KeyEventKind, MouseButton, MouseTrigger, ScrollDirection};

/// A keyboard-to-action map. Generic over the game's action type so the engine
/// never has to know what actions exist.
#[derive(Clone, Debug)]
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

    /// Get all keys bound to a specific action.
    pub fn get_keys_for_action(&self, action: &A) -> Vec<Key>
    where
        A: PartialEq,
    {
        let mut keys = Vec::new();
        for (&key, val) in &self.by_key {
            if val == action {
                keys.push(key);
            }
        }
        // Sort keys to have deterministic return values (Key doesn't implement Ord easily, but we can do a simple order or return them as they are)
        keys
    }

    /// Get all mouse triggers bound to a specific action.
    pub fn get_mouse_for_action(&self, action: &A) -> Vec<(MouseButton, bool)>
    where
        A: PartialEq,
    {
        let mut mouse = Vec::new();
        for (trigger, val) in &self.by_mouse {
            if val == action {
                mouse.push((trigger.button, trigger.pressed));
            }
        }
        mouse
    }

    /// Get all scroll directions bound to a specific action.
    pub fn get_scroll_for_action(&self, action: &A) -> Vec<ScrollDirection>
    where
        A: PartialEq,
    {
        let mut scroll = Vec::new();
        for (&dir, val) in &self.by_scroll {
            if val == action {
                scroll.push(dir);
            }
        }
        scroll
    }

    /// Check if a specific key has any action bound to it.
    pub fn is_key_bound(&self, key: Key) -> bool {
        self.by_key.contains_key(&key)
    }

    /// Check if a specific action is bound to any input trigger.
    pub fn is_action_bound(&self, action: &A) -> bool
    where
        A: PartialEq,
    {
        self.by_key.values().any(|val| val == action)
            || self.by_mouse.values().any(|val| val == action)
            || self.by_scroll.values().any(|val| val == action)
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
/// resulting actions into [`InputRouter`](super::InputRouter).
#[derive(Clone)]
pub struct CommandBindings<A: Clone> {
    by_name: HashMap<String, A>,
    by_glyph: HashMap<char, A>,
    aliases: HashMap<String, String>,
}

impl<A: Clone> CommandBindings<A> {
    pub fn new() -> Self {
        Self {
            by_name: HashMap::new(),
            by_glyph: HashMap::new(),
            aliases: HashMap::new(),
        }
    }

    /// Bind a case-sensitive command name to an action.
    pub fn bind_name<S: Into<String>>(&mut self, name: S, action: A) -> Option<A> {
        self.by_name.insert(name.into(), action)
    }

    /// Bind an alias to an existing command name.
    ///
    /// If the alias was already bound, the new target wins; the previous target
    /// is returned.
    pub fn bind_alias<S: Into<String>>(&mut self, alias: S, target: S) -> Option<String> {
        self.aliases.insert(alias.into(), target.into())
    }

    /// Bind a single compact script glyph to an action.
    pub fn bind_glyph(&mut self, glyph: char, action: A) -> Option<A> {
        self.by_glyph.insert(glyph, action)
    }

    pub fn translate_name(&self, name: &str) -> Option<A> {
        if let Some(target) = self.aliases.get(name) {
            self.by_name.get(target).cloned()
        } else {
            self.by_name.get(name).cloned()
        }
    }

    pub fn translate_glyph(&self, glyph: char) -> Option<A> {
        self.by_glyph.get(&glyph).cloned()
    }

    /// Suggests the closest registered command names (including aliases) to the given input,
    /// using Levenshtein distance. Returns up to 3 suggestions with distance <= 3, sorted by closeness.
    pub fn suggest_command(&self, input: &str) -> Vec<String> {
        let mut suggestions: Vec<(usize, String)> = self
            .by_name
            .keys()
            .chain(self.aliases.keys())
            .map(|name| (levenshtein_distance(input, name), name.clone()))
            .filter(|&(dist, _)| dist <= 3)
            .collect();
        suggestions.sort_by_key(|&(dist, _)| dist);
        suggestions
            .into_iter()
            .take(3)
            .map(|(_, name)| name)
            .collect()
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
        for (alias, target) in other.aliases {
            self.aliases.insert(alias, target);
        }
    }

    /// Remove all name, glyph, and alias command bindings.
    pub fn clear(&mut self) {
        self.by_name.clear();
        self.by_glyph.clear();
        self.aliases.clear();
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
        let mut state = serializer.serialize_struct("CommandBindings", 3)?;

        let names: Vec<(&String, &A)> = self.by_name.iter().collect();
        let glyphs: Vec<(&char, &A)> = self.by_glyph.iter().collect();
        let aliases: Vec<(&String, &String)> = self.aliases.iter().collect();

        state.serialize_field("by_name", &names)?;
        state.serialize_field("by_glyph", &glyphs)?;
        state.serialize_field("aliases", &aliases)?;
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
            #[serde(default)]
            aliases: Vec<(String, String)>,
        }

        let helper = CommandBindingsHelper::deserialize(deserializer)?;
        Ok(CommandBindings {
            by_name: helper.by_name.into_iter().collect(),
            by_glyph: helper.by_glyph.into_iter().collect(),
            aliases: helper.aliases.into_iter().collect(),
        })
    }
}

/// A named profile of key/mouse/scroll bindings.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BindingsProfile<A: Clone> {
    pub name: String,
    pub bindings: Bindings<A>,
}

/// A registry that stores named input binding profiles and allows switching between them.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BindingsProfileRegistry<A: Clone> {
    profiles: HashMap<String, Bindings<A>>,
    active_profile: Option<String>,
}

impl<A: Clone> BindingsProfileRegistry<A> {
    pub fn new() -> Self {
        Self {
            profiles: HashMap::new(),
            active_profile: None,
        }
    }

    pub fn register(&mut self, name: &str, bindings: Bindings<A>) {
        self.profiles.insert(name.to_string(), bindings);
    }

    pub fn get(&self, name: &str) -> Option<&Bindings<A>> {
        self.profiles.get(name)
    }

    pub fn active_profile_name(&self) -> Option<&str> {
        self.active_profile.as_deref()
    }

    pub fn active_bindings(&self) -> Option<&Bindings<A>> {
        self.active_profile.as_ref().and_then(|name| self.get(name))
    }

    pub fn switch_profile(&mut self, name: &str) -> bool {
        if self.profiles.contains_key(name) {
            self.active_profile = Some(name.to_string());
            true
        } else {
            false
        }
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

impl CommandParseError {
    /// If this is an `UnknownCommand` error, attempts to suggest corrections from the bindings.
    pub fn suggest_corrections<A: Clone>(
        &self,
        bindings: &CommandBindings<A>,
    ) -> Option<Vec<String>> {
        match self {
            CommandParseError::UnknownCommand(cmd) => {
                let suggestions = bindings.suggest_command(cmd);
                if suggestions.is_empty() {
                    None
                } else {
                    Some(suggestions)
                }
            }
            _ => None,
        }
    }
}

fn levenshtein_distance(a: &str, b: &str) -> usize {
    let b_len = b.chars().count();
    let mut dp: Vec<usize> = (0..=b_len).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = i;
        dp[0] = i + 1;
        for (j, cb) in b.chars().enumerate() {
            let temp = dp[j + 1];
            dp[j + 1] = if ca == cb {
                prev
            } else {
                1 + prev.min(dp[j]).min(dp[j + 1])
            };
            prev = temp;
        }
    }
    dp[b_len]
}
