//! Agent-facing interactive REPL for Wuthering Terminal.
//!
//! Designed for AI agents that drive the game step-by-step over stdin/stdout.
//! Each action produces a structured JSON response containing the rendered
//! frame (as plain text), the game snapshot, step report, and log messages.
//!
//! ## Protocol
//!
//! The binary reads one line at a time from stdin. Each line is either a
//! built-in meta-command or one or more space-separated action tokens
//! (same syntax as the script runner).
//!
//! After processing, a single JSON object is printed to stdout terminated
//! by a newline, followed by a `READY` sentinel line so the driving tool
//! knows it can send the next command.
//!
//! ### Meta-commands
//!
//! - `help`          — list available action tokens
//! - `snapshot`      — print current snapshot without advancing state
//! - `diagnostics`   — print detailed character diagnostics
//! - `reset`         — reset the game to initial state
//! - `quit` / `q`    — exit
//!
//! ### Response JSON shape
//!
//! ```json
//! {
//!   "ok": true,
//!   "step": 0,
//!   "frame": "<plain-text grid>",
//!   "snapshot": { ... },
//!   "reports": [ { "action": "...", "outcome": "...", "events": [...] } ],
//!   "logs": ["..."],
//!   "game_over": false
//! }
//! ```
//!
//! On parse error or unknown command the response has `"ok": false` with
//! an `"error"` field.
//!
//! ## Usage
//!
//! ```sh
//! cargo run -p wuthering-terminal --bin wuthering-terminal-agent
//! ```
//!
//! Or pipe commands:
//! ```sh
//! echo -e "north\nconfirm\nsnapshot" | cargo run -p wuthering-terminal --bin wuthering-terminal-agent
//! ```

use std::io::{self, BufRead, Write};
use verryte_input::ActionSource;
use wuthering_terminal::{default_commands, resolve_command_token, Game, Outcome};

/// Virtual terminal size for headless rendering.
const TERM_W: u16 = 120;
const TERM_H: u16 = 36;

fn main() {
    // Parse optional flags.
    let mut args = std::env::args().skip(1);
    let mut cols = TERM_W;
    let mut rows = TERM_H;
    let mut seed: Option<u64> = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--size" => {
                if let Some(val) = args.next() {
                    if let Some((w, h)) = val.split_once('x') {
                        cols = w.parse().unwrap_or(TERM_W);
                        rows = h.parse().unwrap_or(TERM_H);
                    }
                }
            }
            "--seed" => {
                if let Some(val) = args.next() {
                    seed = val.parse().ok();
                }
            }
            _ => {}
        }
    }

    let mut game = Game::new();

    if let Some(s) = seed {
        *game.world.resource_mut::<verryte_core::Rng>().unwrap() = verryte_core::Rng::seed(s);
    }

    let mut step_count: u64 = 0;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Emit initial frame so the agent sees the starting state.
    emit_state(&game, &mut out, step_count, cols, rows, &[]);
    step_count += 1;

    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();

    while let Some(Ok(line)) = lines.next() {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        match line.as_str() {
            "q" | "quit" => {
                write_json(
                    &mut out,
                    &serde_json::json!({
                        "ok": true,
                        "event": "quit"
                    }),
                );
                break;
            }

            "help" => {
                write_json(
                    &mut out,
                    &serde_json::json!({
                        "ok": true,
                        "event": "help",
                        "actions": action_help(),
                    }),
                );
            }

            "snapshot" => {
                let snap = game.snapshot();
                write_json(
                    &mut out,
                    &serde_json::json!({
                        "ok": true,
                        "event": "snapshot",
                        "snapshot": snap,
                    }),
                );
            }

            "diagnostics" => {
                let diag = game.diagnostics();
                write_json(
                    &mut out,
                    &serde_json::json!({
                        "ok": true,
                        "event": "diagnostics",
                        "diagnostics": diag,
                    }),
                );
            }

            "reset" => {
                game = Game::new();
                if let Some(s) = seed {
                    *game.world.resource_mut::<verryte_core::Rng>().unwrap() =
                        verryte_core::Rng::seed(s);
                }
                step_count = 0;
                emit_state(&game, &mut out, step_count, cols, rows, &[]);
                step_count += 1;
            }

            _ => {
                // Try to parse as action tokens.
                match game.router.inject_script_with(
                    &default_commands(),
                    &line,
                    ActionSource::Script,
                    resolve_command_token,
                ) {
                    Ok(count) if count > 0 => {
                        let reports = game.run_pending_reports();
                        let report_json: Vec<serde_json::Value> = reports
                            .iter()
                            .map(|r| {
                                serde_json::json!({
                                    "action": format!("{:?}", r.action),
                                    "source": format!("{:?}", r.source),
                                    "outcome": format!("{:?}", r.outcome),
                                    "events": r.events.iter()
                                        .map(|e| format!("{:?}", e))
                                        .collect::<Vec<_>>(),
                                })
                            })
                            .collect();

                        emit_state(&game, &mut out, step_count, cols, rows, &report_json);
                        step_count += 1;

                        if !matches!(game.outcome(), Outcome::Playing) {
                            // Game ended — emit final state with game_over flag
                            // (already included in emit_state via snapshot).
                        }
                    }
                    Ok(_) => {
                        write_json(
                            &mut out,
                            &serde_json::json!({
                                "ok": false,
                                "error": "no actions parsed from input",
                                "input": line,
                            }),
                        );
                    }
                    Err(e) => {
                        write_json(
                            &mut out,
                            &serde_json::json!({
                                "ok": false,
                                "error": format!("{}", e),
                                "input": line,
                            }),
                        );
                    }
                }
            }
        }
    }
}

/// Emit a full state response: frame + snapshot + reports + logs.
fn emit_state(
    game: &Game,
    out: &mut impl Write,
    step: u64,
    cols: u16,
    rows: u16,
    reports: &[serde_json::Value],
) {
    let snap = game.snapshot();
    let grid = game.render_sized(cols, rows);
    let frame = grid.to_plain_string();

    let logs: Vec<String> = game
        .world
        .resource::<verryte_core::MessageLog>()
        .map(|log| log.messages().to_vec())
        .unwrap_or_default();

    let game_over = !matches!(snap.outcome, Outcome::Playing);

    write_json(
        out,
        &serde_json::json!({
            "ok": true,
            "step": step,
            "frame": frame,
            "snapshot": snap,
            "reports": reports,
            "logs": logs,
            "game_over": game_over,
        }),
    );
}

/// Write a JSON value followed by a newline and a READY sentinel.
fn write_json(out: &mut impl Write, value: &serde_json::Value) {
    let _ = serde_json::to_writer(&mut *out, value);
    let _ = writeln!(out);
    let _ = writeln!(out, "READY");
    let _ = out.flush();
}

fn action_help() -> Vec<&'static str> {
    vec![
        "n / north       - Move cursor north",
        "s / south       - Move cursor south",
        "e / east        - Move cursor east",
        "w / west        - Move cursor west",
        ". / wait        - Wait",
        "c / confirm     - Confirm action/selection",
        "x / cancel      - Cancel selection/targeting",
        "> / next        - Next character",
        "< / prev        - Previous character",
        "1               - Skill 1",
        "2               - Skill 2",
        "3               - Skill 3 / QTE Swap",
        "end             - End turn",
        "inspect:x,y     - Inspect coordinate (x,y)",
        "use:N           - Use inventory item N (1-indexed)",
        "craft:N,M       - Craft items N and M together",
        "inventory       - Toggle inventory",
        "minimap         - Toggle minimap",
        "stairs / >      - Descend stairs",
        "save            - Save game",
        "load            - Load game",
        "undo            - Undo last action",
        "redo            - Redo undone action",
        "skilltree       - Toggle skill tree",
        "upgrade:slot_t  - Upgrade skill (e.g. upgrade:0_1)",
        "bestiary        - Toggle bestiary",
        "prestige        - View prestige",
        "rest            - Rest to recover",
        "weather:type    - Change weather",
        "snapshot        - Show current game state (meta)",
        "diagnostics     - Show character diagnostics (meta)",
        "reset           - Reset game to initial state (meta)",
        "quit / q        - Exit (meta)",
    ]
}
