//! Terminal text entry buffer with undo/redo and autocomplete.

use crate::key::{InputEvent, Key, KeyEventKind};

/// A text input buffer for terminal text entry (prompts, naming, chat, etc.).
///
/// Handles key events and produces a plain-text string. Supports cursor
/// movement, insertion, deletion, and a configurable max length, plus common
/// Ctrl shortcuts (A/E/B/F/U/W/K) for navigation and deletion.
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
                    '\u{1b}' => {
                        // Ctrl+[ is Escape
                        self.history_index = None;
                        self.text.clear();
                        self.cursor = 0;
                        self.dirty = true;
                    }
                    '\u{12}' => {
                        // Ctrl+R is redo
                        self.redo();
                    }
                    '\u{2190}' => {
                        // Ctrl+Left: word left
                        self.cursor = self.word_start_left();
                        self.dirty = true;
                    }
                    '\u{2192}' => {
                        // Ctrl+Right: word right
                        self.cursor = self.word_start_right();
                        self.dirty = true;
                    }
                    '\x08' if self.cursor > 0 => {
                        // Ctrl+Backspace: delete word left
                        self.record_state();
                        let start = self.word_start_left();
                        let byte_start = self.char_to_byte(start);
                        let byte_end = self.char_to_byte(self.cursor);
                        self.text.drain(byte_start..byte_end);
                        self.cursor = start;
                        self.dirty = true;
                    }
                    '\x7f' if self.cursor < self.text.chars().count() => {
                        // Ctrl+Delete: delete word right
                        self.record_state();
                        let end = self.word_start_right();
                        let byte_start = self.char_to_byte(self.cursor);
                        let byte_end = self.char_to_byte(end);
                        self.text.drain(byte_start..byte_end);
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

    /// Cycle through autocomplete matches for the word before the cursor,
    /// including text input history as candidates.
    pub fn cycle_autocomplete_with_history(&mut self, dictionary: &[&str]) {
        let history: Vec<String> = self.history.clone();
        let mut combined = dictionary.to_vec();
        for h in &history {
            if !combined.contains(&h.as_str()) {
                combined.push(h.as_str());
            }
        }
        self.cycle_autocomplete(&combined);
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

    /// Check if there are states available on the undo stack.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Check if there are states available on the redo stack.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
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
