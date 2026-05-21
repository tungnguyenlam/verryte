//! TTY frontend for Verryte.
//!
//! This crate bridges the gap between the engine's neutral [`Grid`] representation
//! and an actual terminal. It uses [`crossterm`] to:
//!
//! * Enter raw mode and alternate screen buffer
//! * Translate terminal input events into engine [`InputEvent`]s
//! * Render a [`Grid`] to the terminal with ANSI colors
//! * Clean up on exit

use crossterm::{
    cursor::MoveTo,
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event as TermEvent, KeyCode, KeyModifiers,
        MouseButton, MouseEventKind,
    },
    execute,
    style::Color as TermColor,
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, SetTitle},
};
use std::io::{self, Write};
use verryte_input::{InputEvent, Key, MouseButton as InputMouseButton, ScrollDirection};

pub use verryte_input;
pub use verryte_terminal;

/// Initialize the terminal for game rendering.
/// Returns a guard that will restore the terminal on drop.
pub fn init() -> io::Result<AlternateScreen> {
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    if let Err(error) = execute!(
        stdout,
        EnterAlternateScreen,
        crossterm::cursor::Hide,
        EnableMouseCapture,
        SetTitle("Verryte Game"),
        Clear(ClearType::All),
    ) {
        let _ = terminal::disable_raw_mode();
        return Err(error);
    }
    Ok(AlternateScreen { active: true })
}

/// Exit alternate screen and restore terminal.
pub fn restore() -> io::Result<()> {
    let mut stdout = io::stdout();
    let _ = execute!(stdout, crossterm::cursor::Show);
    execute!(stdout, DisableMouseCapture, LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;
    Ok(())
}

/// Guard that restores terminal on drop.
pub struct AlternateScreen {
    active: bool,
}

impl Drop for AlternateScreen {
    fn drop(&mut self) {
        if self.active {
            let _ = restore();
        }
    }
}

/// Render a Grid to the terminal.
pub fn render(grid: &verryte_terminal::Grid) {
    let mut stdout = io::stdout();
    let _ = execute!(stdout, crossterm::cursor::Hide);
    for y in 0..grid.height() {
        let _ = execute!(stdout, MoveTo(0, y));
        for x in 0..grid.width() {
            if let Some(cell) = grid.get(x, y) {
                let fg = to_crossterm_color(cell.fg);
                let bg = to_crossterm_color(cell.bg);
                print!(
                    "{}{}{}",
                    crossterm::style::SetForegroundColor(fg),
                    crossterm::style::SetBackgroundColor(bg),
                    cell.glyph
                );
            }
        }
    }
    print!("{}", crossterm::style::ResetColor);
    let _ = stdout.flush();
}

/// Render only the cells that changed between `prev` and `next`.
///
/// This writes far fewer bytes than [`render`] when most of the frame is
/// unchanged — common in turn-based games where only a few cells move each
/// tick. The caller should pass the previous frame on each call and then
/// replace it with `next` for the next diff.
///
/// If the grids have different dimensions, a full [`render`] of `next` is
/// performed instead, since the diff cannot reliably clear stale cells at
/// edges that no longer exist in the new frame.
pub fn render_diff(prev: &verryte_terminal::Grid, next: &verryte_terminal::Grid) {
    if prev.width() != next.width() || prev.height() != next.height() {
        clear_screen();
        render(next);
        return;
    }
    let changes = prev.diff(next);
    if changes.is_empty() {
        return;
    }
    let mut stdout = io::stdout();
    let _ = execute!(stdout, crossterm::cursor::Hide);
    for change in &changes {
        if let Some(cell) = change.after {
            let _ = execute!(stdout, MoveTo(change.x, change.y));
            let fg = to_crossterm_color(cell.fg);
            let bg = to_crossterm_color(cell.bg);
            print!(
                "{}{}{}",
                crossterm::style::SetForegroundColor(fg),
                crossterm::style::SetBackgroundColor(bg),
                cell.glyph
            );
        }
    }
    print!("{}", crossterm::style::ResetColor);
    let _ = stdout.flush();
}

/// Clear the screen.
pub fn clear_screen() {
    let mut stdout = io::stdout();
    let _ = execute!(stdout, Clear(ClearType::All));
    let _ = stdout.flush();
}

/// Move cursor to top-left (for repainting).
pub fn home() {
    let mut stdout = io::stdout();
    let _ = execute!(stdout, MoveTo(0, 0));
    let _ = stdout.flush();
}

/// Poll for a single input event. Returns `None` if no input is available.
pub fn poll_event() -> Option<InputEvent> {
    if event::poll(std::time::Duration::from_secs(0)).ok()? {
        event::read().ok().and_then(translate_event)
    } else {
        None
    }
}

/// Read a single input event, blocking until one is available.
pub fn read_event() -> Option<InputEvent> {
    loop {
        match event::read() {
            Ok(evt) => {
                if let Some(translated) = translate_event(evt) {
                    return Some(translated);
                }
            }
            Err(_) => return None,
        }
    }
}

fn translate_event(term_evt: TermEvent) -> Option<InputEvent> {
    match term_evt {
        TermEvent::Key(key) => Some(InputEvent::Key(map_key(key.code, key.modifiers))),
        TermEvent::Mouse(mouse) => match mouse.kind {
            MouseEventKind::Down(button) | MouseEventKind::Up(button) => {
                let input_button = match button {
                    MouseButton::Left => InputMouseButton::Left,
                    MouseButton::Right => InputMouseButton::Right,
                    MouseButton::Middle => InputMouseButton::Middle,
                };
                let pressed = matches!(mouse.kind, MouseEventKind::Down(_));
                Some(InputEvent::Mouse {
                    x: mouse.column,
                    y: mouse.row,
                    button: input_button,
                    pressed,
                })
            }
            MouseEventKind::ScrollUp => Some(InputEvent::MouseScroll {
                x: mouse.column,
                y: mouse.row,
                direction: ScrollDirection::Up,
            }),
            MouseEventKind::ScrollDown => Some(InputEvent::MouseScroll {
                x: mouse.column,
                y: mouse.row,
                direction: ScrollDirection::Down,
            }),
            MouseEventKind::ScrollLeft => Some(InputEvent::MouseScroll {
                x: mouse.column,
                y: mouse.row,
                direction: ScrollDirection::Left,
            }),
            MouseEventKind::ScrollRight => Some(InputEvent::MouseScroll {
                x: mouse.column,
                y: mouse.row,
                direction: ScrollDirection::Right,
            }),
            _ => None,
        },
        TermEvent::Resize(width, height) => Some(InputEvent::Resize { width, height }),
        _ => None,
    }
}

fn map_key(code: KeyCode, modifiers: KeyModifiers) -> Key {
    let ctrl = modifiers.contains(KeyModifiers::CONTROL);
    let alt = modifiers.contains(KeyModifiers::ALT);
    let shift = modifiers.contains(KeyModifiers::SHIFT);
    let has_mod = ctrl || alt;
    match code {
        KeyCode::Char(c) if has_mod => Key::modified(c.to_ascii_lowercase(), ctrl, alt, shift),
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::Enter if has_mod => Key::modified('\r', ctrl, alt, shift),
        KeyCode::Enter => Key::Enter,
        KeyCode::Tab if has_mod || shift => Key::modified('\t', ctrl, alt, shift),
        KeyCode::Tab => Key::Tab,
        KeyCode::Backspace if has_mod => Key::modified('\x08', ctrl, alt, shift),
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Esc if has_mod => Key::modified('\x1b', ctrl, alt, shift),
        KeyCode::Esc => Key::Esc,
        KeyCode::Up if has_mod => Key::modified('↑', ctrl, alt, shift),
        KeyCode::Up => Key::Up,
        KeyCode::Down if has_mod => Key::modified('↓', ctrl, alt, shift),
        KeyCode::Down => Key::Down,
        KeyCode::Left if has_mod => Key::modified('←', ctrl, alt, shift),
        KeyCode::Left => Key::Left,
        KeyCode::Right if has_mod => Key::modified('→', ctrl, alt, shift),
        KeyCode::Right => Key::Right,
        KeyCode::Home if has_mod => Key::modified('H', ctrl, alt, shift),
        KeyCode::Home => Key::Home,
        KeyCode::End if has_mod => Key::modified('E', ctrl, alt, shift),
        KeyCode::End => Key::End,
        KeyCode::PageUp if has_mod => Key::modified('U', ctrl, alt, shift),
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown if has_mod => Key::modified('D', ctrl, alt, shift),
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Insert if has_mod => Key::modified('I', ctrl, alt, shift),
        KeyCode::Insert => Key::Insert,
        KeyCode::Delete if has_mod => Key::modified('\x7f', ctrl, alt, shift),
        KeyCode::Delete => Key::Delete,
        KeyCode::F(f) if has_mod => Key::modified((b'0' + f) as char, ctrl, alt, shift),
        KeyCode::F(f) => Key::F(f),
        _ => Key::Esc,
    }
}

fn to_crossterm_color(color: verryte_terminal::Color) -> TermColor {
    let verryte_terminal::Color(r, g, b) = color;
    TermColor::Rgb { r, g, b }
}

/// Query the current terminal size (columns, rows).
///
/// Returns `(80, 24)` as a fallback if the size cannot be determined.
pub fn terminal_size() -> (u16, u16) {
    crossterm::terminal::size().unwrap_or((80, 24))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn km_ctrl() -> KeyModifiers {
        KeyModifiers::CONTROL
    }

    fn km_alt() -> KeyModifiers {
        KeyModifiers::ALT
    }

    fn km_shift() -> KeyModifiers {
        KeyModifiers::SHIFT
    }

    fn km_ctrl_shift() -> KeyModifiers {
        KeyModifiers::CONTROL | KeyModifiers::SHIFT
    }

    fn km_none() -> KeyModifiers {
        KeyModifiers::NONE
    }

    #[test]
    fn map_key_char_no_modifiers() {
        assert_eq!(map_key(KeyCode::Char('a'), km_none()), Key::Char('a'));
        assert_eq!(map_key(KeyCode::Char('Z'), km_none()), Key::Char('Z'));
    }

    #[test]
    fn map_key_ctrl_produces_modified() {
        let key = map_key(KeyCode::Char('c'), km_ctrl());
        assert_eq!(
            key,
            Key::Modified {
                char: 'c',
                ctrl: true,
                alt: false,
                shift: false,
            }
        );
    }

    #[test]
    fn map_key_alt_produces_modified() {
        let key = map_key(KeyCode::Char('x'), km_alt());
        assert_eq!(
            key,
            Key::Modified {
                char: 'x',
                ctrl: false,
                alt: true,
                shift: false,
            }
        );
    }

    #[test]
    fn map_key_ctrl_shift_produces_modified() {
        let key = map_key(KeyCode::Char('a'), km_ctrl_shift());
        assert_eq!(
            key,
            Key::Modified {
                char: 'a',
                ctrl: true,
                alt: false,
                shift: true,
            }
        );
    }

    #[test]
    fn map_key_ctrl_uppercase_lowercased() {
        let key = map_key(KeyCode::Char('C'), km_ctrl());
        assert_eq!(
            key,
            Key::Modified {
                char: 'c',
                ctrl: true,
                alt: false,
                shift: false,
            }
        );
    }

    #[test]
    fn map_key_special_keys_without_modifiers() {
        assert_eq!(map_key(KeyCode::Enter, km_none()), Key::Enter);
        assert_eq!(map_key(KeyCode::Esc, km_none()), Key::Esc);
        assert_eq!(map_key(KeyCode::Tab, km_none()), Key::Tab);
        assert_eq!(map_key(KeyCode::Backspace, km_none()), Key::Backspace);
        assert_eq!(map_key(KeyCode::Up, km_none()), Key::Up);
        assert_eq!(map_key(KeyCode::Down, km_none()), Key::Down);
        assert_eq!(map_key(KeyCode::Left, km_none()), Key::Left);
        assert_eq!(map_key(KeyCode::Right, km_none()), Key::Right);
        assert_eq!(map_key(KeyCode::Home, km_none()), Key::Home);
        assert_eq!(map_key(KeyCode::End, km_none()), Key::End);
        assert_eq!(map_key(KeyCode::F(5), km_none()), Key::F(5));
    }

    #[test]
    fn map_key_arrow_with_ctrl_produces_modified() {
        let key = map_key(KeyCode::Up, km_ctrl());
        assert_eq!(
            key,
            Key::Modified {
                char: '↑',
                ctrl: true,
                alt: false,
                shift: false,
            }
        );
    }

    #[test]
    fn map_key_tab_with_shift_produces_modified() {
        let key = map_key(KeyCode::Tab, km_shift());
        assert_eq!(
            key,
            Key::Modified {
                char: '\t',
                ctrl: false,
                alt: false,
                shift: true,
            }
        );
    }

    #[test]
    fn map_key_enter_with_ctrl_produces_modified() {
        let key = map_key(KeyCode::Enter, km_ctrl());
        assert_eq!(
            key,
            Key::Modified {
                char: '\r',
                ctrl: true,
                alt: false,
                shift: false,
            }
        );
    }

    #[test]
    fn map_key_f_key_with_alt_produces_modified() {
        let key = map_key(KeyCode::F(3), km_alt());
        assert_eq!(
            key,
            Key::Modified {
                char: '3',
                ctrl: false,
                alt: true,
                shift: false,
            }
        );
    }
}
