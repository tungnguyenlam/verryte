# Ash Courier Prototype

Ash Courier is the first proving game for Verryte.

It is a small turn-based terminal roguelike about crossing a ruined city, carrying a
package through tight maps, simple hazards, and readable tactical choices. The game
should stay modest: it exists to prove the engine's shape, not to become a content-heavy
project too early.

## Current Shape (implemented)

The first slice is in place and is driven exclusively through Verryte's shared
input/script path. Highlights:

- An ASCII level loader (`Game::from_layout`) recognising `#` walls, `.` floor,
  `@` player spawn, `p` package, `h` hazard, `c` chaser, `G` goal.
- Procedural map generation: `Game::from_cave` (cellular automata) and
  `Game::from_bsp` (BSP dungeon) with seeded RNG for reproducibility.
- Agent-ready restart: `Game::reset`, `reset_from_layout`, `reset_from_cave`
  for reusing the same Game instance across multiple runs.
- An `Action` enum covering `MoveNorth/South/East/West`, `StepToPackage`,
  `StepToGoal`, `StepToSafety`, `Wait`, `Scan`, `ScanRadius(u16)`,
  `Inspect(Point)`, `PickUp`, `Drop`, and `Quit`,
  bound to arrow keys, WASD, vi keys, plus `.` / `x` / `p` / `o` / `r` / `,` /
  `D` / `q` / `Esc`, plus `1`-`5` for fixed-radius scans.
  Left mouse press sets an inspection cursor, right mouse press maps to `Scan`,
  and middle mouse press maps to `Wait` through the same router path.
- A single `Game::apply(action)` spine. Terminal events, scripted injections,
  and tests all converge here — there is no separate test-only code path.
- `GameState` resource (turn counter, outcome, package flag, scan count,
  inspection cursor), a
  per-action `Events<GameEvent>` resource, and an entity for the player, each
  package, and each hazard.
- A layered `render()` that walks the ECS to produce a `Grid` (walls / goal,
  then hazards, then packages, then player on top).
- A structured `Snapshot { turn, outcome, has_package, scans, player, cursor,
  cursor_tile, path_to_cursor, distance_to_cursor,
  packages, hazards, chasers, visible_tiles, visible_hazards, map_width,
  map_height, reachable_tiles, tile_under_player, walkable_neighbors,
  path_to_nearest_package, path_to_goal, path_to_nearest_hazard,
  path_to_nearest_chaser, distance_to_nearest_package, distance_to_goal,
  distance_to_nearest_hazard, distance_to_nearest_chaser, safer_neighbors,
  frame, local_frame }` for tests and agents.
- A scripted-run binary (`ash-courier-script`) and tests covering wall
  blocking, hazard loss, goal win, terminal-event parity, command parsing,
  step reports, and ignored post-game actions.
- Per-step reports include the action source (`Terminal`, `Script`, `Agent`,
  `Replay`, or `Test`), an explicit action result (`NoOp`, `Updated`,
  `Advanced`, `Ended`, or `IgnoredGameOver`), and the `GameEvent` list emitted
  by that action.

## How to drive it

Interactive (future TTY frontend):

```rust
let mut game = ash_courier::Game::new();
game.handle_event(InputEvent::Key(Key::Right));
game.run_pending();
println!("{}", game.snapshot().frame);
```

Scripted / test / agent:

```rust
let mut game = ash_courier::Game::new();
game.router.inject_all([Action::MoveEast, Action::PickUp, Action::MoveSouth]);
game.run_pending();
assert_eq!(game.outcome(), Outcome::Won);
```

Replay trace:

```rust
let trace = verryte_input::ActionTrace::from_actions(
    [Action::MoveEast, Action::PickUp, Action::Drop],
    verryte_input::ActionSource::Replay,
);
trace.replay_into(&mut game.router);
let reports = game.run_pending_reports();
```

CLI smoke test:

```sh
cargo run -p ash-courier --bin ash-courier-script -- "eeesss,nnneeeesssssss"
```

The script runner accepts named commands and compact glyphs in the same input.
Named commands are useful for readable tests; glyph runs are useful for compact
replay strings. Scripts also accept `;` separators and `#` inline comments.
Parameterized scan tokens (`scan:3`, `scan3`, `x3`) and inspect tokens
(`inspect:3,4`, `look:3,4`) are supported through the same shared
`InputRouter` queue path.

## Why This Prototype Exists

Ash Courier should force Verryte to support the important path:

```text
terminal event -> game action -> game system -> observable state
script command -> game action -> game system -> observable state
```

If a feature only works in the interactive TUI and cannot be driven by scripts or tests,
the prototype has exposed an engine problem.

## Initial Playable Shape

The first complete version should include:

- a small grid map rendered in a real terminal
- a player that can move by named actions
- walls or blocked tiles
- at least one pickup, key, package, or delivery objective
- at least one simple enemy or hazard
- a win condition and a loss condition
- observable state after each meaningful action
- tests or scripted commands that can drive the same logic as a person

## Engine Pressure Points

Use this prototype to validate:

- ECS/data modeling for entities, components, resources, and systems
- input mapping and action dispatch
- turn-based game loop behavior
- map and collision primitives
- terminal rendering layers
- state snapshots for testing and agents
- modular boundaries between engine code and game code

## Scope Discipline

Prefer engine-revealing features over content volume.

Good next additions:

- one new action
- one new component
- one new system
- one better state snapshot
- one clearer test
- one rendering improvement that exercises the engine

## Current Harness Shape

Ash Courier currently exposes:

- `default_bindings()` for terminal-style key input
- `default_commands()` for named and compact script commands
- `InputRouter<Action>` as the single queue for both paths
- `InputRouter::handle_with` / `Game::handle_event_with` for custom
  position-aware input translation
- `InputRouter::inject_script` / `inject_script_with` for sourced
  script/agent injection, including parameterized command tokens
- `ActionTrace` replay for recorded or planned sourced action runs
- `Game::run_pending_reports()` for per-action before/after snapshots
- `Game::snapshot()` for agent/test-readable state, local map context,
  visibility, reachability, package/goal/hazard/chaser path hints, hazard-safe
  neighboring moves, a plain rendered frame, and a local viewport frame

The map uses `verryte-map` grid/path primitives and rendering uses
`verryte-terminal` cell grids, keeping game rules in the prototype and reusable
behavior in engine crates.

Avoid early rabbit holes:

- large procedural world generation
- deep combat math
- complex inventory
- story systems
- real-time action
- custom engine architecture that only Ash Courier can use
