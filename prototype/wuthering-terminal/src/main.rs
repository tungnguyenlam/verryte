use std::time::{Duration, Instant};
use verryte_input::{ActionSource, InputEvent};
use verryte_terminal::Grid;
use verryte_tty::{init, poll_event, render, render_diff};
use wuthering_terminal::{Action, Game, Outcome};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut game = Game::new();
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
            if let InputEvent::Resize {
                width: _,
                height: _,
            } = event
            {
                prev_frame = None;
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
        let _dt = now.duration_since(last_tick).as_secs_f32();
        last_tick = now;

        // Render
        let grid = game.render();
        let (gw, gh) = (grid.width(), grid.height());

        // Center the grid in the terminal
        let x_off = (w.saturating_sub(gw) / 2) as u16;
        let y_off = (h.saturating_sub(gh) / 2) as u16;

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

    Ok(())
}
