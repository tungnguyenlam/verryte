//! Scripted Ash Courier runner.
//!
//! Reads a short action script from `argv[1]`, drives the game through the
//! same `InputRouter` an interactive frontend would use, and prints a plain
//! text render, local viewport, and one-line snapshot summary after each step.
//! Useful for smoke-testing the engine without a TTY.
//!
//! Accepts named commands (`east pickup`) and compact glyphs:
//! * `n` / `s` / `e` / `w` — move
//! * `.` — wait
//! * `x` — scan visible tiles
//! * `scan:3`, `scan3`, `x3` — scan with explicit radius
//! * `inspect:3,4` / `look:3,4` — inspect a tile and update the cursor
//! * `c` / `clear_cursor` — clear the inspection cursor
//! * `p` — step one tile toward nearest package
//! * `o` — step one tile toward nearest goal
//! * `v` — step one tile toward the safest neighbor away from hazards
//! * `,` — pick up
//! * `!` — drop carried package
//! * `q` — quit
//!
//! Whitespace is ignored. `;` can separate commands, and `#` starts a comment
//! that runs to end-of-line.

use ash_courier::{default_commands, resolve_command_token, Game, Outcome, Position, Tile};
use verryte_input::ActionSource;

fn main() {
    let mut args = std::env::args();
    let _program = args.next();
    let script = args.next().unwrap_or_else(|| {
        eprintln!("usage: ash-courier-script <action-string>");
        eprintln!("example: ash-courier-script \"sse.\"");
        std::process::exit(2);
    });

    let mut game = Game::new();
    let queued = match game.router.inject_script_with(
        &default_commands(),
        &script,
        ActionSource::Script,
        resolve_command_token,
    ) {
        Ok(count) => count,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(2);
        }
    };

    println!("--- initial ---");
    print_frame(&game);
    println!("queued_actions={queued}");

    for (i, report) in game.run_pending_reports().into_iter().enumerate() {
        let snap = &report.after;
        println!(
            "--- step {i:>3}: {:?} source={:?} result={:?} changed={} turn_advanced={} events={} ---",
            report.action,
            report.source,
            report.result,
            report.changed,
            report.turn_advanced,
            report.events.len()
        );
        println!("{}", snap.frame);
        println!(
            "turn={} outcome={:?} package={} scans={} visible_tiles={} visible_hazards={} reachable_tiles={} chasers={} safer_neighbors={} cursor={} cursor_tile={} path_to_cursor={} distance_to_cursor={} path_to_package={} path_to_goal={} path_to_hazard={} path_to_chaser={} distance_to_package={} distance_to_goal={} distance_to_hazard={} distance_to_chaser={}",
            snap.turn,
            snap.outcome,
            snap.has_package,
            snap.scans,
            snap.visible_tiles.len(),
            snap.visible_hazards.len(),
            snap.reachable_tiles.len(),
            snap.chasers.len(),
            snap.safer_neighbors.len(),
            maybe_point(snap.cursor),
            maybe_tile(snap.cursor_tile),
            path_len(&snap.path_to_cursor),
            maybe_distance(snap.distance_to_cursor),
            path_len(&snap.path_to_nearest_package),
            path_len(&snap.path_to_goal),
            path_len(&snap.path_to_nearest_hazard),
            path_len(&snap.path_to_nearest_chaser),
            maybe_distance(snap.distance_to_nearest_package),
            maybe_distance(snap.distance_to_goal),
            maybe_distance(snap.distance_to_nearest_hazard),
            maybe_distance(snap.distance_to_nearest_chaser)
        );
        println!("local:\n{}", snap.local_frame);
        if !matches!(snap.outcome, Outcome::Playing) {
            break;
        }
    }

    match game.outcome() {
        Outcome::Won => std::process::exit(0),
        Outcome::Lost | Outcome::Quit | Outcome::Playing => std::process::exit(1),
    }
}

fn print_frame(game: &Game) {
    let snap = game.snapshot();
    println!("{}", snap.frame);
    println!(
        "turn={} outcome={:?} package={} scans={} visible_tiles={} visible_hazards={} reachable_tiles={} chasers={} safer_neighbors={} cursor={} cursor_tile={} path_to_cursor={} distance_to_cursor={} path_to_package={} path_to_goal={} path_to_hazard={} path_to_chaser={} distance_to_package={} distance_to_goal={} distance_to_hazard={} distance_to_chaser={}",
        snap.turn,
        snap.outcome,
        snap.has_package,
        snap.scans,
        snap.visible_tiles.len(),
        snap.visible_hazards.len(),
        snap.reachable_tiles.len(),
        snap.chasers.len(),
        snap.safer_neighbors.len(),
        maybe_point(snap.cursor),
        maybe_tile(snap.cursor_tile),
        path_len(&snap.path_to_cursor),
        maybe_distance(snap.distance_to_cursor),
        path_len(&snap.path_to_nearest_package),
        path_len(&snap.path_to_goal),
        path_len(&snap.path_to_nearest_hazard),
        path_len(&snap.path_to_nearest_chaser),
        maybe_distance(snap.distance_to_nearest_package),
        maybe_distance(snap.distance_to_goal),
        maybe_distance(snap.distance_to_nearest_hazard),
        maybe_distance(snap.distance_to_nearest_chaser)
    );
    println!("local:\n{}", snap.local_frame);
}

fn path_len(path: &Option<Vec<ash_courier::Position>>) -> String {
    path.as_ref()
        .map(|path| path.len().to_string())
        .unwrap_or_else(|| "-".to_owned())
}

fn maybe_point(point: Option<Position>) -> String {
    point
        .map(|point| format!("{},{}", point.x, point.y))
        .unwrap_or_else(|| "-".to_owned())
}

fn maybe_tile(tile: Option<Tile>) -> String {
    tile.map(|tile| format!("{tile:?}"))
        .unwrap_or_else(|| "-".to_owned())
}

fn maybe_distance(distance: Option<u16>) -> String {
    distance
        .map(|distance| distance.to_string())
        .unwrap_or_else(|| "-".to_owned())
}
