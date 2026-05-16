//! Interactive TTY frontend for Ash Courier.
//!
//! Run with: `cargo run -p ash-courier --bin ash-courier-tty`

use ash_courier::{Game, Outcome};
use verryte_terminal::{Cell, Color, Grid, Rect};
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

    // Initial render
    render_game(&game, term_size);

    loop {
        if let Some(event) = tty::read_event() {
            // Track resize events so the layout adapts.
            if let verryte_input::InputEvent::Resize { width, height } = event {
                term_size = (width, height);
            }

            let handled = game.handle_event(event);
            if handled {
                game.run_pending();
            }

            render_game(&game, term_size);

            if game.outcome() == Outcome::Quit {
                break;
            }
        }
    }
}

fn terminal_size() -> (u16, u16) {
    tty::terminal_size()
}

fn render_game(game: &Game, (term_w, term_h): (u16, u16)) {
    // Clamp to a minimum so panels still fit.
    let w = term_w.max(40);
    let h = term_h.max(16);

    let mut root = Grid::new(w, h);

    // Derive layout from terminal size.
    // Viewport: left side, roughly half width, most of height.
    let vp_w = (w / 2).saturating_sub(2).max(10);
    let vp_h = (h - 2).max(5);
    let vp_rect = Rect::new(0, 0, vp_w + 2, vp_h + 2);
    root.draw_panel(
        vp_rect,
        " VIEWPORT ",
        Cell::new('#').with_fg(Color::DARK_GREY),
        Color::CYAN,
    );
    let viewport = game.render_viewport(vp_w, vp_h);
    root.blit(&viewport, 1, 1);

    // Right side: log on top, status below.
    let right_x = vp_w + 3;
    let right_w = w.saturating_sub(right_x);
    if right_w < 5 {
        // Terminal too narrow; skip right panels.
        tty::home();
        tty::render(&root);
        return;
    }

    let log_h = (h / 2).saturating_sub(1).max(3);
    let log_rect = Rect::new(right_x, 0, right_w, log_h + 2);
    root.draw_panel(
        log_rect,
        " LOG ",
        Cell::new('#').with_fg(Color::DARK_GREY),
        Color::YELLOW,
    );
    let msgs = game.messages();
    let max_log_lines = log_h.min(10) as usize;
    let display_msgs = if msgs.len() > max_log_lines {
        &msgs[msgs.len() - max_log_lines..]
    } else {
        &msgs[..]
    };
    for (i, msg) in display_msgs.iter().enumerate() {
        root.write_str(right_x + 2, 1 + i as u16, msg, Color::GREY, Color::BLACK);
    }

    // Status panel below log.
    let status_y = log_h + 2;
    let status_h = h.saturating_sub(status_y);
    if status_h >= 3 {
        let status_rect = Rect::new(right_x, status_y, right_w, status_h);
        root.draw_panel(
            status_rect,
            " STATUS ",
            Cell::new('#').with_fg(Color::DARK_GREY),
            Color::GREEN,
        );

        let state = game.state();
        let snap = game.snapshot();
        let sy = status_y + 1;
        root.write_str(
            right_x + 2,
            sy,
            &format!("Turn:    {}", state.turn),
            Color::WHITE,
            Color::BLACK,
        );
        if sy + 1 < h {
            root.write_str(
                right_x + 2,
                sy + 1,
                &format!("Package: {}", if state.has_package { "YES" } else { "NO" }),
                Color::WHITE,
                Color::BLACK,
            );
        }
        if sy + 2 < h {
            root.write_str(
                right_x + 2,
                sy + 2,
                &format!("Scans:   {}", state.scans),
                Color::WHITE,
                Color::BLACK,
            );
        }
        if sy + 3 < h {
            root.write_str(
                right_x + 2,
                sy + 3,
                &format!(
                    "Dist P/G/H/C: {}/{}/{}/{}",
                    maybe_distance(snap.distance_to_nearest_package),
                    maybe_distance(snap.distance_to_goal),
                    maybe_distance(snap.distance_to_nearest_hazard),
                    maybe_distance(snap.distance_to_nearest_chaser)
                ),
                Color::WHITE,
                Color::BLACK,
            );
        }

        let (outcome_str, outcome_color) = match state.outcome {
            Outcome::Playing => ("Playing", Color::CYAN),
            Outcome::Won => ("WON! Press Q to exit.", Color::GREEN),
            Outcome::Lost => ("LOST! Press Q to exit.", Color::RED),
            Outcome::Quit => ("Quit", Color::GREY),
        };
        if sy + 4 < h {
            root.write_str(
                right_x + 2,
                sy + 4,
                &format!("Outcome: {}", outcome_str),
                outcome_color,
                Color::BLACK,
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
            Color::DARK_GREY,
            Color::BLACK,
        );
        if hint_y + 1 < h {
            root.write_str(
                right_x + 2,
                hint_y + 1,
                "X: Scan | R: Safety | P/O: Path steps | Q: Quit",
                Color::DARK_GREY,
                Color::BLACK,
            );
        }
    }

    tty::home();
    tty::render(&root);
}

fn maybe_distance(distance: Option<u16>) -> String {
    distance
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_owned())
}
