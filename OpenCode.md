# Verryte Agent Guide

This repository is a Rust workspace for **Verryte**, a modular terminal-game
engine. Treat [GOAL.md](GOAL.md) as the north star: Verryte should stay
terminal-native, data-first, modular, extensible, and observable enough for
tests, scripts, replays, and agents.

Before doing substantial work, read:

- [GOAL.md](GOAL.md) for the engine direction and boundaries.
- [README.md](README.md) for the current workspace shape and commands.
- [WORKLOG.md](WORKLOG.md) for recent decisions and handoff notes.
- The relevant crate or prototype README/source for the slice you are touching.

## CRITICAL: Task Continuity Rules

**NEVER stop mid-task. You MUST complete the full planned work before ending a
response.** Specifically:

1. **Do not stop after one edit.** If you planned 5 improvements, deliver all 5.
2. **Do not stop to "verify next."** Run the verification yourself.
3. **Do not stop after a compile error.** Fix it and re-verify.
4. **Do not stop after a test failure.** Debug and fix it.
5. **Do not summarize partial progress as a stopping point.** Only stop when all
   planned work is done AND verified.
6. **If you must pause** (tool limit, context limit), record the exact in-progress
   state in WORKLOG.md under a `## CONTINUE HERE` heading with: what was being
   edited, current compile/test state, and the next atomic action needed.
7. **Never ask "should I continue?"** — the answer is always yes.
8. **Never end with "would you like me to..."** — just do it.

## Workspace Map

- `crates/verryte-core` - ECS-style entities, component/resource storage,
  events, queries, and schedules. Keep it terminal- and input-agnostic.
- `crates/verryte-input` - neutral input events, action bindings, command
  parsing, action queues, sourced actions, and replay traces. Modularized
  into `key`, `action`, `bindings`, `trace`, `router`, `text_input`, and
  `replay` sub-modules. This crate protects the shared control path.
- `crates/verryte-map` - reusable grid, geometry, distance, visibility,
  reachability, and pathfinding primitives.
- `crates/verryte-terminal` - terminal cell, color, grid, clipping, viewport,
  diff, line, border, and text rendering primitives.
- `crates/verryte-tty` - crossterm frontend that translates real terminal input
  into `verryte-input` events and renders `verryte-terminal::Grid`.
- `crates/verryte-audio` - spatial/panned audio playback via rodio, with
  `AudioEvent` integration from `verryte-core` and volume/pan controls.
- `prototype/wuthering-terminal` - a 2D turn-based tactical RPG prototype.
  Validates the engine on complex mechanics: team swapping, Echo absorption,
  parry/dodge, and adaptive-resolution sprite rendering. Source PNG artwork
  lives in `prototype/wuthering-terminal/assets/` and is loaded at runtime
  via `image_to_grid()` with chroma-key transparency.
- `prototype/vfx-demo` - interactive terminal VFX demo proving particles,
  screen shake, flash overlays, floating damage text, AoE rings, and a
  real-time 30 FPS game loop. Loads PNG character sprites (Kael, Mira,
  Blight Sovereign) from `wuthering-terminal/assets/` via `image_to_grid()`
  with chroma-key transparency. Run with `cargo run -p vfx-demo`.

## Engineering Priorities

The key architectural promise is:

```text
terminal event -> game action -> game system -> observable state
script command -> game action -> game system -> observable state
```

Do not split interactive play, scripts, tests, replays, and agent control into
separate gameplay paths. Add metadata such as `ActionSource` when useful, but
keep action application shared.

Prefer the smallest useful vertical slice. When a prototype exposes a reusable
need, move the reusable part into the appropriate engine crate and keep
game-specific rules in the prototype. Avoid large speculative systems, content
volume, or architecture that only serves a hypothetical future game.

Keep APIs plain Rust and inspectable. The workspace forbids unsafe code through
the root lint configuration; do not introduce `unsafe`.

When behavior changes, update focused tests and docs in the same pass. Good
tests usually drive through the same public path as scripts or terminal input,
then assert observable state.

Preserve unrelated user changes. The worktree may already be dirty; inspect
before editing and do not revert work you did not make.

## Verification

Normal workspace verification:

```sh
cargo fmt --check
cargo test
```

Useful Wuthering Terminal smoke commands:

```sh
cargo run -p wuthering-terminal --bin wuthering-terminal-script -- "inspect:4,4 confirm inspect:4,5 confirm"
cargo run -p wuthering-terminal --bin wuthering-terminal
```

The script runner executes a sequence of action tokens for smoke testing. The TTY runner needs a real terminal.

If a Rust toolchain is unavailable or a command cannot be run in the current
environment, say so in the final response and record the limitation in the
worklog for non-trivial work.

## Documentation

Keep these docs aligned when their subject changes:

- Root [README.md](README.md) for workspace capabilities and common commands.
- [prototype/wuthering-terminal/README.md](prototype/wuthering-terminal/README.md) for the
  tactical RPG prototype and its current scope.
- [GOAL.md](GOAL.md) only when the project direction itself changes.
- [prompt/improve.md](prompt/improve.md) only when the long-running autonomous
  development instructions need to change.

## Committing

After successfully completing a task, updating the documentation, and verifying the workspace, you must commit your changes. Only commit once the job is complete and all tests pass.
- Use `git status` and `git diff HEAD` to review your work.
- Stage the specific files you modified or created using `git add <file>`.
- Use `git log -n 3` to match the project's commit message style.
- Create a concise commit message explaining the "why" of the changes.
- Do not push to a remote repository unless explicitly asked.

## Worklog

After finishing a non-trivial request, **append** a dated entry to the end of
[WORKLOG.md](WORKLOG.md) at the repo root. Entries are chronological: oldest
first, newest at bottom.

**Always append via a bash heredoc. Never edit or rewrite `WORKLOG.md` with
patch/edit tools.** This keeps earlier handoff notes byte-for-byte intact.

Every non-trivial entry should usually cover:

1. **Goal** - restate what the user asked for.
2. **Changes** - concrete edits with `path:line` references where useful.
3. **Reasoning** - why this approach, alternatives rejected, and trade-offs.
4. **Assumptions** - what you took as given that a future agent might challenge.
5. **Gotchas** - subtle findings, footguns, or things that nearly broke.
6. **Follow-ups** - what remains or what should be verified later.
