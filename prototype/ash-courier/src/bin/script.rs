//! Scripted Ash Courier runner.
//!
//! Reads a short action script from `argv[1]`, drives the game through the
//! same `InputRouter` an interactive frontend would use, and prints a plain
//! text render plus a one-line snapshot summary after each step. Useful for
//! smoke-testing the engine without a TTY.
//!
//! Accepts named commands (`east pickup`) and compact glyphs:
//! * `n` / `s` / `e` / `w` — move
//! * `.` — wait
//! * `,` — pick up
//! * `q` — quit
//!
//! Whitespace in the script is ignored.

use ash_courier::{default_commands, Game, Outcome};

fn main() {
    let mut args = std::env::args();
    let _program = args.next();
    let script = args.next().unwrap_or_else(|| {
        eprintln!("usage: ash-courier-script <action-string>");
        eprintln!("example: ash-courier-script \"sse.\"");
        std::process::exit(2);
    });

    let actions = match default_commands().parse_script(&script) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(2);
        }
    };

    let mut game = Game::new();
    println!("--- initial ---");
    print_frame(&game);

    for (i, action) in actions.into_iter().enumerate() {
        game.router.inject(action);
        let mut reports = game.run_pending_reports();
        let report = reports
            .pop()
            .expect("one injected action should produce one report while game is running");
        let snap = &report.after;
        println!(
            "--- step {i:>3}: {action:?} source={:?} result={:?} changed={} turn_advanced={} ---",
            report.source, report.result, report.changed, report.turn_advanced
        );
        println!("{}", snap.frame);
        println!(
            "turn={} outcome={:?} package={}",
            snap.turn, snap.outcome, snap.has_package
        );
        if game.is_over() {
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
        "turn={} outcome={:?} package={}",
        snap.turn, snap.outcome, snap.has_package
    );
}
