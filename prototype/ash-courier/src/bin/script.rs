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
//! * `t` / `step_cursor` — step one tile toward the inspection cursor
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
    let first_arg = args.next();

    let interactive = match &first_arg {
        None => true,
        Some(s) if s == "-i" || s == "--interactive" => true,
        _ => false,
    };

    if interactive {
        run_repl();
    } else {
        let script = first_arg.unwrap();
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
            print_report(i, &report);
            if !matches!(report.after.outcome, Outcome::Playing) {
                break;
            }
        }

        match game.outcome() {
            Outcome::Won => std::process::exit(0),
            Outcome::Lost | Outcome::Quit | Outcome::Playing => std::process::exit(1),
        }
    }
}

fn run_repl() {
    use std::io::{self, BufRead, Write};

    let mut game = Game::new();
    println!("=== Ash Courier Interactive Shell ===");
    println!("Type 'help' for a list of commands.");
    print_frame(&game);

    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    let mut step_count = 0;

    print!("> ");
    io::stdout().flush().unwrap();

    while let Some(Ok(line)) = lines.next() {
        let line = line.trim();
        if line.is_empty() {
            print!("> ");
            io::stdout().flush().unwrap();
            continue;
        }

        if line == "help" {
            print_repl_help();
        } else if line == "q" || line == "quit" {
            println!("Quitting...");
            break;
        } else if let Some(stripped) = line.strip_prefix("save ") {
            let path = stripped.trim();
            if path.is_empty() {
                println!("error: save requires a path. e.g. save game.save");
            } else {
                match game.save_to_file(path) {
                    Ok(_) => println!("Game successfully saved to '{}'", path),
                    Err(e) => println!("error saving game: {}", e),
                }
            }
        } else if let Some(stripped) = line.strip_prefix("load ") {
            let path = stripped.trim();
            if path.is_empty() {
                println!("error: load requires a path. e.g. load game.save");
            } else {
                match game.load_from_file(path) {
                    Ok(_) => {
                        println!("Game successfully loaded from '{}'", path);
                        print_frame(&game);
                    }
                    Err(e) => println!("error loading game: {}", e),
                }
            }
        } else {
            match game.router.inject_script_with(
                &default_commands(),
                line,
                ActionSource::Script,
                resolve_command_token,
            ) {
                Ok(count) => {
                    if count > 0 {
                        let reports = game.run_pending_reports();
                        for report in reports {
                            print_report(step_count, &report);
                            step_count += 1;
                            if !matches!(report.after.outcome, Outcome::Playing) {
                                break;
                            }
                        }
                    } else {
                        println!("No actions parsed from input.");
                    }
                }
                Err(e) => {
                    println!("error: {}", e);
                }
            }
        }

        if !matches!(game.outcome(), Outcome::Playing) {
            println!("Game ended with outcome: {:?}", game.outcome());
            break;
        }

        print!("> ");
        io::stdout().flush().unwrap();
    }
}

fn print_repl_help() {
    println!("Commands:");
    println!("  help             - Show this help message");
    println!("  save <path>      - Save the game state to the specified path");
    println!("  load <path>      - Load the game state from the specified path");
    println!("  q / quit         - Quit the game");
    println!();
    println!("Action scripts (can combine multiple, e.g. 'e e e s p'):");
    println!("  n / s / e / w    - Move north/south/east/west");
    println!("  .                - Wait one turn");
    println!("  x                - Scan visible tiles");
    println!("  scan:<r> / x<r>  - Scan with explicit radius r");
    println!("  inspect:x,y      - Inspect tile at (x,y) and update cursor");
    println!("  c                - Clear the inspection cursor");
    println!("  p                - Step one tile toward nearest package");
    println!("  o                - Step one tile toward nearest goal");
    println!("  v                - Step one tile toward safest neighbor away from hazards");
    println!("  t                - Step one tile toward inspection cursor");
    println!("  ,                - Pick up package");
    println!("  !                - Drop carried package");
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

fn print_report(i: usize, report: &ash_courier::StepReport) {
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
