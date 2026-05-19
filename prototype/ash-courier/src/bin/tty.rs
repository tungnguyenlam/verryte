//! Interactive TTY frontend for Ash Courier.
//!
//! Run with: `cargo run -p ash-courier --bin ash-courier-tty`

use ash_courier::{Action, Game, Outcome, Position};
use verryte_input::{InputEvent, MouseButton};
use verryte_terminal::{Alignment, Cell, ColorPalette, Grid, Rect};
use verryte_tty as tty;

fn main() {
    let _screen = match tty::init() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to initialize terminal: {}", e);
            std::process::exit(1);
        }
    };

    let mut game = Game::new();
    let mut term_size = terminal_size();

    // Initial render — full frame to establish the baseline.
    let mut prev_frame = render_game(&game, term_size);
    tty::home();
    tty::render(&prev_frame);

    loop {
        if let Some(event) = tty::read_event() {
            if let InputEvent::Resize { width, height } = event {
                term_size = (width, height);
            }

            let inspect_layout = InspectLayout::new(&game, term_size);
            let handled =
                game.handle_event_with(event, |event| inspect_layout.action_for_event(event));
            if handled {
                game.run_pending();
            }

            let next_frame = render_game(&game, term_size);
            tty::render_diff(&prev_frame, &next_frame);
            prev_frame = next_frame;

            if game.outcome() == Outcome::Quit {
                break;
            }
        }
    }
}

fn terminal_size() -> (u16, u16) {
    tty::terminal_size()
}

fn clamp_terminal((term_w, term_h): (u16, u16)) -> (u16, u16) {
    (term_w.max(40), term_h.max(16))
}

fn viewport_dimensions(width: u16, height: u16) -> (u16, u16) {
    let vp_w = (width / 2).saturating_sub(2).max(10);
    let vp_h = (height - 2).max(5);
    (vp_w, vp_h)
}

struct InspectLayout {
    origin: Position,
    inner: Rect,
}

impl InspectLayout {
    fn new(game: &Game, term_size: (u16, u16)) -> Self {
        let (term_w, term_h) = clamp_terminal(term_size);
        let (vp_w, vp_h) = viewport_dimensions(term_w, term_h);
        let map = game.map();
        let inner_w = vp_w.min(map.width);
        let inner_h = vp_h.min(map.height);
        let origin = game.viewport_origin(inner_w, inner_h);
        let inner = Rect::new(1, 1, inner_w, inner_h);
        Self { origin, inner }
    }

    fn action_for_event(&self, event: InputEvent) -> Option<Action> {
        match event {
            InputEvent::Mouse {
                x,
                y,
                button: MouseButton::Left,
                pressed: true,
            } if self.inner.contains(x, y) => {
                let dx = x.saturating_sub(self.inner.x) as i16;
                let dy = y.saturating_sub(self.inner.y) as i16;
                Some(Action::Inspect(Position {
                    x: self.origin.x + dx,
                    y: self.origin.y + dy,
                }))
            }
            _ => None,
        }
    }
}

fn render_game(game: &Game, (term_w, term_h): (u16, u16)) -> Grid {
    // Clamp to a minimum so panels still fit.
    let (w, h) = clamp_terminal((term_w, term_h));

    let mut root = Grid::new(w, h);
    let palette = ColorPalette::dark_dungeon();

    // Fill background.
    root.fill_rect(
        Rect::new(0, 0, w, h),
        Cell::new(' ').with_bg(palette.background),
    );

    // Derive layout from terminal size.
    // Viewport: left side, roughly half width, most of height.
    let (vp_w, vp_h) = viewport_dimensions(w, h);
    let vp_rect = Rect::new(0, 0, vp_w + 2, vp_h + 2);
    root.draw_rounded_panel(
        vp_rect,
        " VIEWPORT ",
        palette.ui_border,
        palette.ui_title,
        palette.background,
    );
    let viewport = game.render_viewport(vp_w, vp_h);
    root.blit(&viewport, 1, 1);

    // Right side: log on top, status below.
    let right_x = vp_w + 3;
    let right_w = w.saturating_sub(right_x);
    if right_w < 5 {
        // Terminal too narrow; skip right panels.
        return root;
    }

    let log_h = (h / 2).saturating_sub(1).max(3);
    let log_rect = Rect::new(right_x, 0, right_w, log_h + 2);
    root.draw_rounded_panel(
        log_rect,
        " LOG ",
        palette.ui_border,
        palette.ui_title,
        palette.background,
    );
    let msgs = game.messages();
    let max_log_lines = log_h.min(10) as usize;
    let display_msgs = if msgs.len() > max_log_lines {
        &msgs[msgs.len() - max_log_lines..]
    } else {
        &msgs[..]
    };
    for (i, msg) in display_msgs.iter().enumerate() {
        root.write_str(
            right_x + 2,
            1 + i as u16,
            msg,
            palette.ui_text,
            palette.background,
        );
    }

    // Status panel below log.
    let status_y = log_h + 2;
    let status_h = h.saturating_sub(status_y);
    if status_h >= 3 {
        let status_rect = Rect::new(right_x, status_y, right_w, status_h);
        root.draw_rounded_panel(
            status_rect,
            " STATUS ",
            palette.ui_border,
            palette.ui_title,
            palette.background,
        );

        let state = game.state();
        let snap = game.snapshot();
        let sy = status_y + 1;
        let inner_w = right_w.saturating_sub(4);
        root.write_aligned(
            right_x + 2,
            sy,
            inner_w,
            &format!("Turn:    {}", state.turn),
            Alignment::Left,
            palette.ui_text,
            palette.background,
        );
        if sy + 1 < h {
            root.write_aligned(
                right_x + 2,
                sy + 1,
                inner_w,
                &format!("Package: {}", if state.has_package { "YES" } else { "NO" }),
                Alignment::Left,
                if state.has_package {
                    palette.item
                } else {
                    palette.ui_dim
                },
                palette.background,
            );
        }
        if sy + 2 < h {
            root.write_aligned(
                right_x + 2,
                sy + 2,
                inner_w,
                &format!("Scans:   {}", state.scans),
                Alignment::Left,
                palette.ui_text,
                palette.background,
            );
        }
        if sy + 3 < h {
            root.write_aligned(
                right_x + 2,
                sy + 3,
                inner_w,
                &format!(
                    "Dist P/G/H/C: {}/{}/{}/{}",
                    maybe_distance(snap.distance_to_nearest_package),
                    maybe_distance(snap.distance_to_goal),
                    maybe_distance(snap.distance_to_nearest_hazard),
                    maybe_distance(snap.distance_to_nearest_chaser)
                ),
                Alignment::Left,
                palette.ui_text,
                palette.background,
            );
        }

        if sy + 4 < h {
            let cursor = snap.cursor.map_or_else(
                || "-".to_owned(),
                |point| {
                    let tile = snap
                        .cursor_tile
                        .map(|tile| format!("{tile:?}"))
                        .unwrap_or_else(|| "-".to_owned());
                    format!("{},{} ({tile})", point.x, point.y)
                },
            );
            root.write_aligned(
                right_x + 2,
                sy + 4,
                inner_w,
                &format!("Cursor:  {}", cursor),
                Alignment::Left,
                palette.ui_text,
                palette.background,
            );
        }
        let (outcome_str, outcome_color) = match state.outcome {
            Outcome::Playing => ("Playing", palette.ui_title),
            Outcome::Won => ("WON! Press Q to exit.", palette.goal),
            Outcome::Lost => ("LOST! Press Q to exit.", palette.hazard),
            Outcome::Quit => ("Quit", palette.ui_dim),
        };
        if sy + 5 < h {
            root.write_aligned(
                right_x + 2,
                sy + 5,
                inner_w,
                &format!("Outcome: {}", outcome_str),
                Alignment::Left,
                outcome_color,
                palette.background,
            );
        }
    }

    // Control hints at the bottom.
    if h >= 2 {
        let hint_y = h - 2;
        root.write_str(
            right_x + 2,
            hint_y,
            "Arrows/WASD: Move | SPACE: Wait | G: Pick | D: Drop | 1-5: ScanR",
            palette.ui_dim,
            palette.background,
        );
        if hint_y + 1 < h {
            root.write_str(
                right_x + 2,
                hint_y + 1,
                "X: Scan | R: Safety | P/O: Path steps | Mouse L: Inspect | Q: Quit",
                palette.ui_dim,
                palette.background,
            );
        }
    }

    root
}

fn maybe_distance(distance: Option<u16>) -> String {
    distance
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_owned())
}
