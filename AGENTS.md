``# Verryte Agent Guide

This repository is a Rust workspace for **Verryte**, a modular terminal-game
engine. Treat [GOAL.md](GOAL.md) as the north star: Verryte should stay
terminal-native, data-first, modular, extensible, and observable enough for
tests, scripts, replays, and agents.

Before doing substantial work, read:

- [GOAL.md](GOAL.md) for the engine direction and boundaries.
- [README.md](README.md) for the current workspace shape and commands.
- [WORKLOG.md](WORKLOG.md) for recent decisions and handoff notes.
- The relevant crate or prototype README/source for the slice you are touching.

The [prompt/](prompt/) directory contains reusable continuation prompts. It is
project context, not runtime code.

## Workspace Map

- `crates/verryte-core` - ECS-style entities, component/resource storage,
  events, queries, and schedules. Keep it terminal- and input-agnostic.
- `crates/verryte-input` - neutral input events, action bindings, command
  parsing, action queues, sourced actions, and replay traces. This crate protects
  the shared control path.
- `crates/verryte-map` - reusable grid, geometry, distance, visibility,
  reachability, and pathfinding primitives.
- `crates/verryte-terminal` - terminal cell, color, grid, clipping, viewport,
  diff, line, border, and text rendering primitives.
- `crates/verryte-tty` - crossterm frontend that translates real terminal input
  into `verryte-input` events and renders `verryte-terminal::Grid`.
- `prototype/ash-courier` - the first proving game. Use it to validate engine
  behavior through a small turn-based roguelike instead of inventing abstract
  engine features in isolation.

## Engineering Priorities

The key architectural promise is:

```text
terminal event -> game action -> game system -> observable state
script command -> game action -> game system -> observable state
```

Do not split interactive play, scripts, tests, replays, and agent control into
separate gameplay paths. Add metadata such as `ActionSource` when useful, but
keep action application shared.

Prefer the smallest useful vertical slice. When Ash Courier exposes a reusable
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

## Current Engine Capabilities

As of the latest commits, Verryte has:

- **ECS core** (`verryte-core`): entities, components, resources, events, queries, schedules.
- **Input system** (`verryte-input`): unified input events (keyboard, mouse, scroll), action bindings, command parsing, action queues, replay traces, `ActionSource` for origin tracking.
- **Map & geometry** (`verryte-map`): grid, bounds, distance, visibility, reachability, pathfinding, `TileGrid` with iterators.
- **Terminal rendering** (`verryte-terminal`): cell, color, grid, clipping, viewport, diff, line, border, text rendering, batch write helpers.
- **TTY frontend** (`verryte-tty`): crossterm integration, real-time input translation, incremental cell-diff rendering.
- **Ash Courier proving game** (`prototype/ash-courier`): turn-based roguelike, cursor control, step-to-target navigation, score/win/loss outcomes, batch input, replay support, script runner.

**Key architectural invariant:** all gameplay paths (terminal input, scripted commands, tests, replays, agent injection) converge on the same `Action` enum and `apply_action()` function. Do not split this path.

## Verification

Normal workspace verification:

```sh
cargo fmt --check
cargo test
```

Useful Ash Courier smoke commands:

```sh
cargo run -p ash-courier --bin ash-courier-script -- "eeesss,nnneeeesssssss"
cargo run -p ash-courier --bin ash-courier-tty
```

The script runner returns success only when the run reaches `Outcome::Won`; use
the documented winning script above for a passing smoke test. The TTY runner
needs a real terminal and is not a CI-style check.

If a Rust toolchain is unavailable or a command cannot be run in the current
environment, say so in the final response and record the limitation in the
worklog for non-trivial work.

## Documentation

Keep these docs aligned when their subject changes:

- Root [README.md](README.md) for workspace capabilities and common commands.
- [prototype/ash-courier/README.md](prototype/ash-courier/README.md) for the
  proving game, its controls, harnesses, and current scope.
- [GOAL.md](GOAL.md) only when the project direction itself changes.
- Prompt files under [prompt/](prompt/) only when reusable agent instructions
  need to change.

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

The worklog captures what is not recoverable from `git diff` or `git log`:
reasoning, rejected alternatives, assumptions, constraints, gotchas, and
follow-ups. Prefer too much context over too little.

Every non-trivial entry should usually cover:

1. **Goal** - restate what the user asked for.
2. **Changes** - concrete edits with `path:line` references where useful.
3. **Reasoning** - why this approach, alternatives rejected, and trade-offs.
4. **Assumptions** - what you took as given that a future agent might challenge.
5. **Gotchas** - subtle findings, footguns, or things that nearly broke.
6. **Follow-ups** - what remains or what should be verified later.

Use this exact append style:

```bash
cat >> WORKLOG.md <<'EOF'

## YYYY-MM-DD - one-line summary

**Goal.** Restate the request in 1-2 sentences.

**Changes.**
- `path/to/file.rs:42` - what changed and why it matters.
- `README.md` - documentation updated to match behavior.

**Reasoning.** Explain why this shape of solution fits Verryte. Mention
alternatives considered, such as putting behavior in Ash Courier only versus
moving a reusable primitive into an engine crate.

**Assumptions.** List any assumptions that are not obvious from the diff.

**Gotchas.** Capture non-obvious findings, such as script smoke tests only
passing on a win, TTY behavior requiring a real terminal, or a shared input path
that must not fork into test-only logic.

**Follow-ups.** Note what should be done or verified next.
EOF
```

Use the literal quoted delimiter `'EOF'` so backticks and `$` are not expanded.
Skip the worklog for genuinely trivial edits such as typo fixes or a single
user-dictated config line. For everything else, leave enough context that the
next agent does not need to re-derive your reasoning.
