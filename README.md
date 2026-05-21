# Verryte

Verryte is a modular Rust engine for rich terminal games. The current workspace
is intentionally small and vertical: engine crates provide core ECS storage,
input routing, spatial maps, and terminal-cell rendering; Ash Courier exercises
those pieces as the first proving game.

## Workspace

- `crates/verryte-core` - generational entities, component/resource storage,
  event queues, and a minimal ordered schedule. Includes `Query`, `Query2`,
  and `Query3` iterators (with `ExactSizeIterator` support),
  `World::query2_iter` / `World::query3_iter` for lazy multi-component
  iteration, `World::has_resource` and `World::contains` for safe resource
  and component existence checks, `World::for_each2_mut` / `World::for_each3_mut`
  for mutable two- and three-component iteration, `World::despawn_with` for bulk
  entity removal, `World::retain` for predicate-based entity filtering,
  `World::query3` for three-component queries, `World::get_or_insert` /
  `World::get_or_insert_with` for lazy component initialization,
  `World::resource_or_insert` / `World::resource_or_insert_with` for lazy
  resource setup,
  `World::entities()` for iterating all live entities, `World::spawn_batch` for
  bulk entity creation with shared components, `World::query_mut` for mutable
  single-component queries, `Entity::is_invalid` for sentinel checks,
  `Entity` with `Display` (`index#generation`), `Schedule::clear`,
  `Schedule::remove_by_name`, and `Schedule::run_system_by_name` for runtime
  schedule management and selective execution, `Schedule::add_conditional` for
  systems gated by a `RunCondition` predicate, `Schedule::add_stage` /
  `Schedule::run_stage` / `Schedule::run_stage_with_hook` for named execution
  phases with optional per-system observability hooks, `Events::peek` /
  `Events::last` for non-consuming event inspection, bounded
  `MessageLog::with_max`, a `Tag` marker component for entity grouping and
  filtering, `Rng` (seeded xorshift64 RNG) for reproducible randomness in
  tests, replays, and procedural generation (including `weighted_pick` for
  weighted random selection), and `GameClock` for tracking elapsed ticks,
  pause state, and real-time duration.
- `crates/verryte-input` - terminal-neutral input events, key/mouse/scroll
  bindings, script command bindings, sourced queued actions, replayable
  `ActionTrace`s, router-level script injection, pending queue snapshots and
  drain traces, the shared action queue, input context switching via
  `set_bindings` and `bindings_guard`, a context stack via
  `InputRouter::push_bindings` / `pop_bindings` for nested modal input, batch
  event processing (`handle_batch`, `handle_batch_with`), custom event
  translation (`handle_with`) for position-aware inputs,
  `Bindings::merge` for layering keymaps,
  `CommandBindings::merge` for layering command sets,
  `Bindings::iter_keys` / `iter_mouse` and `CommandBindings::iter_names` /
  `iter_glyphs` for binding inspection,
  `Bindings::clear` / `CommandBindings::clear` for removing all bindings,
  `InputRouter::total_actions_queued()` for lifetime action metrics,
  `TextInput` for terminal text entry (prompts, naming, chat) with cursor
  movement, insertion, deletion, max length, dirty tracking, and Ctrl shortcut
  editing (A/E/B/F/U/W/K), and
  `ActionSource` with `Display`/`FromStr` for serialization and debugging.
  `Key`, `MouseButton`, and `ScrollDirection` have `Display` for logging.
- `crates/verryte-map` - reusable grid/spatial primitives: `Point`
  (with `Display`, `From<(i16,i16)>`), `Direction` (with `Display`,
  `From<Direction> for Direction8`), `Direction8` (with `Display`,
  `TryFrom<Direction8> for Direction`, both with `from_offset` for converting
  deltas to directions), `Size` (with `Display`, `From<(u16,u16)>`), typed `TileGrid<T>`, cardinal and
  8-directional neighbors, line tracing (both `Vec`-returning `line_between`
  and lazy `LineIter`), visibility queries, line-of-sight checks
  (`is_line_of_sight_clear`), recursive shadowcasting field-of-view
  (`TileGrid::field_of_view`), shortest/nearest cardinal and 8-directional
  paths, reachable regions (4 and 8-directional), distance helpers (Manhattan,
  Chebyshev), flood-fill for connected-component detection, region counting,
  hazard-distance safety scoring (`safer_neighbors4`), random-walk dungeon
  generation (`TileGrid::random_walk_fill4`), BSP dungeon generation
  (`TileGrid::generate_bsp_dungeon`), `TileGrid::count_matching`,
  `TileGrid::find_matching`, `TileGrid::points_in`,
  `TileGrid::points_matching`, and `TileGrid::density` for map analysis,
  `TileGrid::bounds` and
  `TileGrid::bounding_box_of` with `Bounds` (with `Display`) / `Bounds::clamp_point` plus
  `Bounds::intersects` / `Bounds::intersection` for spatial framing, `SpatialHash<T>`
  for efficient proximity queries on grid-based
  entities, cellular automata cave
  generation
  (`TileGrid::cellular_automata_cave`) for organic procedural maps,
  `TileGrid::from_ascii` for constructing grids from multi-line string
  literals, `TileGrid::map_tiles` for transforming tile types, and
  `TileGrid::crop` for extracting rectangular sub-regions as new grids.
- `crates/verryte-terminal` - terminal-cell data structures: colors, cells,
  grids, clipping, borders, line drawing, blitting, viewports, frame diffs,
  plain-text snapshots, ANSI-colored output (`Grid::to_ansi_string`), HTML
  output (`Grid::to_html_string`) for web/debug viewing, circle drawing and
  filling (`Grid::draw_circle`, `Grid::fill_circle`), diamond/rhombus shapes
  (`Grid::draw_diamond`, `Grid::fill_diamond`), Unicode box-drawing borders
  (`draw_border_rounded`, `draw_rounded_panel`, `draw_text_box`), horizontal/vertical lines
  (`draw_hline`, `draw_vline`), progress bars (`Grid::draw_progress_bar`),
  text wrapping utilities (`wrap_text`, `write_wrapped_text`, `write_lines`),
  `Grid::transform` and `Grid::map` for bulk cell modification, row/column helpers
  (`Grid::row_mut`, `Grid::fill_row`, `Grid::fill_col`), `Rect::inset` for
  padded layouts, `Grid::resize` for dynamic grid sizing on terminal resize,
  `Grid::scroll_up` and
  `Grid::scroll_down` for scrolling content within a grid, a `Layer` system
  for compositing named, ordered rendering layers (map, entities, UI), a
  `Layers` collection for managed layer lifecycle, `ColorPalette` with built-in
  themes (dark dungeon, light classic, amber terminal, cyberpunk) for consistent
  game theming, `Sprite` and `SpriteSheet` for frame-based terminal animation,
  and `draw_sparkline` for inline data visualization with Unicode block
  characters.
  `CellAttrs` supports all attribute combinations (bold, dim, italic, underline,
  blink, reverse) with correct ANSI escape code generation, plus
  inspection getters (`is_bold`, `is_underline`, `is_dim`, `is_italic`,
  `is_reverse`, `is_blink`, `is_empty`). `Color` has `Display` (`#RRGGBB`),
  `From<(u8,u8,u8)>`. `Rect` has `Display` and `From<(u16,u16,u16,u16)>`. `Grid::fill_background`
  sets the background color across all cells without changing glyphs.
- `crates/verryte-tty` - TTY frontend using crossterm: alternate screen,
  input polling with full modifier key passthrough (Ctrl, Alt, Shift produce
  `Key::Modified` events), Grid rendering with ANSI colors, incremental
  diff-based rendering (`render_diff`) with automatic full-render fallback
  on terminal resize, and
  `terminal_size()` for querying the current terminal dimensions.
- `prototype/ash-courier` - a small turn-based roguelike prototype that proves
  the engine path through movement, pickup, hazards, win/loss state, rendering,
  scan/visibility state (using recursive shadowcasting FOV), inspection cursor
  state with highlight rendering, event reports, package drop/re-pickup, path hints, hazard-distance
  safety hints, reachable-state hints, local viewport snapshots, observable
  snapshots,
  `GameClock` for turn tracking, `Rng` for reproducible chaser AI,
  diff-based TTY rendering, procedural cave map generation via
  `Game::from_cave` using cellular automata, BSP dungeon generation via
  `Game::from_bsp`, `Map::from_ascii` for convenient map construction,
  `Game::reset` / `reset_from_cave` / `reset_from_bsp` / `reset_from_layout` for agent-ready
  restart, `Game::render_with_palette` for theme-configurable rendering,
  and `PreviousPosition` for chaser anti-oscillation tie-breaking.
- `prototype/wuthering-terminal` - a 2D turn-based tactical RPG prototype
  inspired by Wuthering Waves. Features 3-resonator QTE swapping, Echo
  absorption, telegraphed enemy attacks with parry/dodge, and an adaptive
  resolution chibi sprite system that scales visual fidelity to the user's
  terminal size. Sprites are compiled from PNG pixel art into static Rust
  arrays at build time using half-block sub-pixel packing.
- `prototype/vfx-demo` - interactive terminal VFX demo proving real-time
  animation at 30 FPS. Features a particle system (fire, ice, lightning,
  slash, burst, heal, AoE), screen shake, flash overlays, floating damage
  text, expanding ring indicators, combo counter, and diff-based rendering.
  Run with `cargo run -p vfx-demo`.

## Control Model

Verryte keeps interactive input and automation on the same path:

```text
terminal event -> game action -> game system -> observable state
script command -> game action -> game system -> observable state
```

In practice:

- terminal frontends translate keys/mouse into `InputEvent` and call
  `InputRouter::handle`;
- games can bind simple mouse button transitions to actions, or translate
  position-aware mouse events via `InputRouter::handle_with` before routing;
- scripts and agents can parse command text with `CommandBindings` or call
  `InputRouter::inject_script` / `InputRouter::inject_script_with`, which
  inject the resulting actions into the same queue with an explicit
  `ActionSource`;
- tests and replay tools can route neutral input events with
  `InputRouter::handle_from`, or snapshot queued work with
  `InputRouter::pending_trace`;
- recorded or planned runs can be replayed with `ActionTrace`, preserving each
  action's source while still using the same router queue;
- games drain actions and apply normal systems;
- snapshots and per-step reports expose observable state, action source, action
  result, and game events for tests, scripts, and future tooling.

## Ash Courier

Ash Courier includes two runners:

**Script runner** (non-TTY, for tests/CI):
```sh
cargo run -p ash-courier --bin ash-courier-script -- "eeesss,nnneeeesssssss"
```

**Interactive TTY** (real terminal):
```sh
cargo run -p ash-courier --bin ash-courier-tty
```

`verryte-input` command bindings accept both command words and compact glyphs:
`north`, `south`, `east`, `west`, `wait`, `scan`, `step_package`, `step_goal`,
`step_safety`, `step_cursor`, `pickup`, `drop`, `clear_cursor`, `quit`, plus `n`,
`s`, `e`, `w`, `.`, `x`, `p`, `o`, `v`, `t`, `,`, `!`, `c`, and `q`. Scripts can
mix whitespace with `;` separators and `#` line comments. Ash Courier's script
runner also accepts parameterized scan tokens (`scan:3`, `scan3`, `x3`) and
inspect tokens (`inspect:3,4`, `look:3,4`) through `inject_script_with`.
The script runner prints the rendered frame, local viewport, state summary,
source, action result, event count, visible/reachable tile counts, path lengths,
shortest distances to package/goal/hazards/chasers, and safer-neighbor counts
after each action, plus cursor state when inspection is used.

## Verification

The normal check for the workspace is:

```sh
cargo fmt --check
cargo test
```

This environment must have the Rust toolchain on `PATH` for those commands.
