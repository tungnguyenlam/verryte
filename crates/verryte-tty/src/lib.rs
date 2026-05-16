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
        self, DisableMouseCapture, EnableMouseCapture, Event as TermEvent, KeyCode, MouseButton,
        MouseEventKind,
    },
    execute,
    style::Color as TermColor,
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, SetTitle},
};
use std::io::{self, Write};
use verryte_input::{InputEvent, Key, MouseButton as InputMouseButton};

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
        EnableMouseCapture,
        SetTitle("Verryte Game"),
    ) {
        let _ = terminal::disable_raw_mode();
        return Err(error);
    }
    Ok(AlternateScreen { active: true })
}

/// Exit alternate screen and restore terminal.
pub fn restore() -> io::Result<()> {
    let mut stdout = io::stdout();
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
    for y in 0..grid.height() {
        for x in 0..grid.width() {
            if let Some(cell) = grid.get(x, y) {
                let fg = to_crossterm_color(cell.fg);
                let bg = to_crossterm_color(cell.bg);
                print!("{}", crossterm::style::SetForegroundColor(fg));
                print!("{}", crossterm::style::SetBackgroundColor(bg));
                print!("{}", cell.glyph);
            }
        }
        if y + 1 < grid.height() {
            println!();
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
        TermEvent::Mouse(mouse) => {
            let button = match mouse.kind {
                MouseEventKind::Down(b) | MouseEventKind::Up(b) => Some(b),
                _ => None,
            }?;
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
        TermEvent::Resize(width, height) => Some(InputEvent::Resize { width, height }),
        _ => None,
    }
}

fn map_key(code: KeyCode, _modifiers: crossterm::event::KeyModifiers) -> Key {
    match code {
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::Enter => Key::Enter,
        KeyCode::Esc => Key::Esc,
        KeyCode::Tab => Key::Tab,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Insert => Key::Insert,
        KeyCode::Delete => Key::Delete,
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
