//! Scripted Wuthering Terminal runner.
//!
//! Reads a sequence of commands from argv[1], injects them via InputRouter,
//! runs the game, prints the ANSI rendering and state reports, and returns
//! exit codes based on game outcome.
//!
//! Supports `--verify <expected.json>` to compare actual outcomes against an
//! expected JSON file for CI regression testing.

use verryte_input::ActionSource;
use wuthering_terminal::{default_commands, resolve_command_token, ActionOutcome, Game, Outcome};

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

        let mut verify_path: Option<String> = None;
        let remaining: Vec<String> = args.collect();
        let mut i = 0;
        while i < remaining.len() {
            if remaining[i] == "--verify" && i + 1 < remaining.len() {
                verify_path = Some(remaining[i + 1].clone());
                i += 2;
            } else {
                i += 1;
            }
        }

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

        let mut actual_outcomes: Vec<ActionOutcome> = Vec::new();

        for (i, report) in game.run_pending_reports().into_iter().enumerate() {
            print_report(i, &report);
            print_frame(&game);
            actual_outcomes.push(report.outcome.clone());
            if !matches!(report.after.outcome, Outcome::Playing) {
                break;
            }
        }

        let diag = game.diagnostics();
        println!("--- diagnostics ---");
        println!(
            "alive={} dead={} floor={} turn={} phase={:?} weather={} combo={} energy={}",
            diag.alive_entities,
            diag.dead_entities,
            diag.floor,
            diag.turn,
            diag.current_phase,
            diag.weather,
            diag.combo_count,
            diag.concert_energy
        );
        for ch in &diag.characters {
            println!(
                "  {}: hp={}/{} ap={}/{} status={} alive={}",
                ch.name, ch.hp, ch.max_hp, ch.ap, ch.max_ap, ch.status, ch.alive
            );
        }

        if let Some(path) = verify_path {
            match run_verification(&path, &actual_outcomes) {
                Ok(()) => {
                    println!("--- verification PASSED ---");
                    match game.outcome() {
                        Outcome::Victory | Outcome::Playing => std::process::exit(0),
                        Outcome::Defeat | Outcome::Quit => std::process::exit(1),
                    }
                }
                Err(diff) => {
                    eprintln!("--- verification FAILED ---");
                    eprintln!("{}", diff);
                    std::process::exit(1);
                }
            }
        } else {
            match game.outcome() {
                Outcome::Victory | Outcome::Playing => std::process::exit(0),
                Outcome::Defeat | Outcome::Quit => std::process::exit(1),
            }
        }
    }
}

fn run_verification(expected_path: &str, actual_outcomes: &[ActionOutcome]) -> Result<(), String> {
    let expected_json = std::fs::read_to_string(expected_path)
        .map_err(|e| format!("Failed to read expected file '{}': {}", expected_path, e))?;
    let expected: Vec<ActionOutcome> = serde_json::from_str(&expected_json)
        .map_err(|e| format!("Failed to parse expected JSON: {}", e))?;

    if expected.len() != actual_outcomes.len() {
        return Err(format!(
            "Outcome count mismatch: expected {} steps, got {} steps",
            expected.len(),
            actual_outcomes.len()
        ));
    }

    let mut diffs = Vec::new();
    for (i, (exp, act)) in expected.iter().zip(actual_outcomes.iter()).enumerate() {
        if exp != act {
            diffs.push(format!("  step {}: expected {:?}, got {:?}", i, exp, act));
        }
    }

    if diffs.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} outcome diff(s):\n{}",
            diffs.len(),
            diffs.join("\n")
        ))
    }
}

fn run_repl() {
    use std::io::{self, BufRead, Write};

    let mut game = Game::new();
    println!("=== Wuthering Terminal Interactive Shell ===");
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
        } else if line == "history" {
            let history = game
                .world
                .resource::<verryte_input::ActionHistory<wuthering_terminal::Action>>()
                .unwrap();
            println!("--- Action History ({} records) ---", history.len());
            for (i, record) in history.iter().enumerate() {
                println!("  {:3}: {:?} source={:?}", i, record.action, record.source);
            }
            if let Err(e) = history.save_to_file("history.json") {
                println!("Error saving history: {}", e);
            } else {
                println!("Saved history to history.json");
            }
        } else if line == "load-history" {
            match verryte_input::ActionHistory::<wuthering_terminal::Action>::load_from_file(
                "history.json",
            ) {
                Ok(history) => {
                    println!("Loaded {} records from history.json", history.len());
                    for (i, record) in history.iter().enumerate() {
                        println!("  {:3}: {:?} source={:?}", i, record.action, record.source);
                    }
                }
                Err(e) => println!("Error loading history: {}", e),
            }
        } else if line == "diagnostics" {
            let diag = game.diagnostics();
            println!("--- Diagnostics ---");
            println!(
                "alive={} dead={} floor={} turn={} phase={:?} weather={} combo={} energy={}",
                diag.alive_entities,
                diag.dead_entities,
                diag.floor,
                diag.turn,
                diag.current_phase,
                diag.weather,
                diag.combo_count,
                diag.concert_energy
            );
            for ch in &diag.characters {
                println!(
                    "  {}: hp={}/{} ap={}/{} status={} alive={}",
                    ch.name, ch.hp, ch.max_hp, ch.ap, ch.max_ap, ch.status, ch.alive
                );
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
                            print_frame(&game);
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
    println!("  q / quit         - Quit the game");
    println!("  diagnostics      - Show game diagnostics");
    println!();
    println!("Action scripts (can combine multiple, e.g. 'e e e s c'):");
    println!("  n / s / e / w    - Move cursor");
    println!("  .                - Wait");
    println!("  c                - Confirm action/selection");
    println!("  x                - Cancel selection/targeting");
    println!("  >                - Next character");
    println!("  <                - Prev character");
    println!("  1                - Select skill 1");
    println!("  2                - Select skill 2");
    println!("  3                - Select skill 3 / QTE Swap");
    println!("  q                - Quit");
    println!("  e                - End turn");
    println!("  inspect:x,y      - Inspect coordinate (x,y)");
}

fn print_frame(game: &Game) {
    let snap = game.snapshot();
    let grid = game.render();
    println!("{}", grid.to_ansi_string());
    println!(
        "turn={} outcome={:?} phase={:?} cursor={},{}",
        snap.turn, snap.outcome, snap.phase, snap.cursor.x, snap.cursor.y
    );
    if let Some(log) = game.world.resource::<verryte_core::MessageLog>() {
        for msg in log.messages() {
            println!("  log: {}", msg);
        }
    }
}

fn print_report(i: usize, report: &wuthering_terminal::StepReport) {
    println!(
        "--- step {i:>3}: {:?} source={:?} events={} outcome={:?} ---",
        report.action,
        report.source,
        report.events.len(),
        report.outcome
    );
    for event in &report.events {
        println!("  event: {:?}", event);
    }
}
