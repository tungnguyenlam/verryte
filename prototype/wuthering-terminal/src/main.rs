use std::time::{Duration, Instant};
use verryte_input::{ActionSource, InputEvent};
use verryte_terminal::Grid;
use verryte_tty::{init, poll_event, render, render_diff};
use wuthering_terminal::{action::Action, Game, Outcome};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut game = Game::new();
    game.trigger_intro_dialogue();
    let mut prev_frame: Option<Grid> = None;

    let _guard = init()?;

    let mut last_tick = Instant::now();
    let tick_rate = Duration::from_millis(33); // ~30 FPS

    loop {
        let (w, h) = verryte_tty::terminal_size();
        let outcome = game
            .world
            .resource::<wuthering_terminal::components::GameState>()
            .unwrap()
            .outcome;

        if outcome != Outcome::Playing {
            break;
        }

        // Handle input
        while let Some(event) = poll_event() {
            let ui_state = game
                .world
                .resource::<wuthering_terminal::components::GameState>()
                .unwrap()
                .ui_state;
            if ui_state == wuthering_terminal::components::UIState::Console {
                if let InputEvent::Key {
                    key,
                    kind: verryte_input::KeyEventKind::Press,
                } = event
                {
                    game.apply_action(Action::ConsoleKey(key), ActionSource::Terminal);
                }
                continue;
            }

            if let InputEvent::Resize {
                width: _,
                height: _,
            } = event
            {
                prev_frame = None;
                continue;
            }
            if let InputEvent::Mouse {
                x,
                y,
                button: verryte_input::MouseButton::Left,
                pressed: true,
            } = event
            {
                if game.handle_mouse_click(w, h, x, y) {
                    continue;
                }
            }
            if let InputEvent::MouseScroll {
                x: _,
                y: _,
                direction,
            } = event
            {
                match direction {
                    verryte_input::ScrollDirection::Up => {
                        game.apply_action(Action::ZoomIn, ActionSource::Terminal);
                    }
                    verryte_input::ScrollDirection::Down => {
                        game.apply_action(Action::ZoomOut, ActionSource::Terminal);
                    }
                    _ => {}
                }
                continue;
            }
            if game.router.handle(event) {
                while let Some(queued) = game.router.pop_action() {
                    game.apply_action(queued.action, queued.source);
                }
            }
        }

        // Update
        let now = Instant::now();
        let dt = now.duration_since(last_tick).as_secs_f32();
        last_tick = now;
        game.router.tick(dt);
        game.update(dt);

        // Render
        let grid = game.render();
        let (gw, gh) = (grid.width(), grid.height());

        // Center the grid in the terminal, applying shake offset
        let (shake_x, shake_y) = game.vfx().shake_offset();
        let x_off = (((w.saturating_sub(gw) / 2) as i32 + shake_x as i32).max(0) as u16).min(w);
        let y_off = (((h.saturating_sub(gh) / 2) as i32 + shake_y as i32).max(0) as u16).min(h);

        let mut root = Grid::new(w, h);
        root.blit(&grid, x_off as i32, y_off as i32);

        if let Some(prev) = prev_frame {
            render_diff(&prev, &root);
        } else {
            render(&root);
        }
        prev_frame = Some(root);

        // FPS cap
        let elapsed = now.elapsed();
        if elapsed < tick_rate {
            std::thread::sleep(tick_rate - elapsed);
        }
    }

    // Show battle summary screen
    let outcome = game
        .world
        .resource::<wuthering_terminal::components::GameState>()
        .unwrap()
        .outcome;
    let (w, h) = verryte_tty::terminal_size();
    let mut summary_grid = Grid::new(w, h);
    wuthering_terminal::ui::render_battle_summary(&mut summary_grid, &game.world, outcome, w, h);
    render(&summary_grid);

    // Wait for any keypress to exit
    loop {
        if let Some(InputEvent::Key { .. } | InputEvent::Mouse { pressed: true, .. }) = poll_event()
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(16));
    }

    Ok(())
}
