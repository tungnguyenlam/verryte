# Verryte

Verryte is a modular Rust engine for rich terminal games. The current workspace
is intentionally small and vertical: engine crates provide core ECS storage,
input routing, spatial maps, and terminal-cell rendering; Ash Courier exercises
those pieces as the first proving game.

## Workspace

- `crates/verryte-core` - generational entities, component/resource storage,
  event queues, and a minimal ordered schedule.
- `crates/verryte-input` - terminal-neutral input events, key bindings, script
  command bindings, sourced queued actions, and the shared action queue.
- `crates/verryte-map` - reusable grid/spatial primitives: `Point`,
  `Direction`, `Size`, typed `TileGrid<T>`, cardinal neighbors, and distance
  helpers.
- `crates/verryte-terminal` - terminal-cell data structures: colors, cells,
  grids, clipping, borders, blitting, and plain-text snapshots.
- `prototype/ash-courier` - a small turn-based roguelike prototype that proves
  the engine path through movement, pickup, hazards, win/loss state, rendering,
  and observable snapshots.

## Control Model

Verryte keeps interactive input and automation on the same path:

```text
terminal event -> game action -> game system -> observable state
script command -> game action -> game system -> observable state
```

In practice:

- terminal frontends translate keys/mouse into `InputEvent` and call
  `InputRouter::handle`;
- scripts and agents parse command text with `CommandBindings` and inject the
  resulting actions into the same `InputRouter`;
- games drain actions and apply normal systems;
- snapshots and per-step reports expose observable state, action source, and
  action result for tests, scripts, and future tooling.

## Ash Courier Script Runner

Ash Courier includes a tiny non-TTY runner:

```sh
cargo run -p ash-courier --bin ash-courier-script -- "eeesss,nnneeeesssssss"
```

`verryte-input` command bindings accept both command words and compact glyphs:
`north`, `south`, `east`, `west`, `wait`, `pickup`, `quit`, plus `n`, `s`, `e`,
`w`, `.`, `,`, and `q`. The runner prints the rendered frame, state summary,
source, and action result after each action.

## Verification

The normal check for the workspace is:

```sh
cargo fmt --check
cargo test
```

This environment must have the Rust toolchain on `PATH` for those commands.
