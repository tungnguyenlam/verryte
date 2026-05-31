//! Neutral terminal-side input event types.
//!
//! These are leaf types with no dependencies on other modules in this crate.

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
/// into this enum so [`Bindings`](super::Bindings) doesn't depend on any
/// particular backend.
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
                if parts.is_empty() {
                    write!(f, "{c}")
                } else {
                    parts.push("");
                    write!(f, "{}{c}", parts.join("+"))
                }
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
