# Verryte

Verryte is a modular Rust engine for rich terminal games. The current workspace
is intentionally small and vertical: engine crates provide core ECS storage,
input routing, spatial maps, and terminal-cell rendering; the prototypes exercise
those pieces.

## Workspace

- `crates/verryte-core` - generational entities, component/resource storage,
  event queues, and a minimal ordered schedule. Includes `Query`, `Query2`,
  `Query3`, `Query4`, `Query5` iterators (with `ExactSizeIterator` support),
  `World::query2_iter` / `World::query3_iter` / `Query4` / `Query5` for lazy multi-component
  iteration, `World::has_resource` and `World::contains` for safe resource
  and component existence checks, `World::for_each2_mut` / `World::for_each3_mut` / `World::for_each4_mut`
  for mutable two-, three-, and four-component iteration, `World::despawn_with` for bulk
  entity removal, `World::retain` for predicate-based entity filtering,
  `World::query3` / `World::query4` / `World::query5` for multi-component queries,
  `World::get_or_insert` /
  `World::get_or_insert_with` for lazy component initialization,
  `World::resource_or_insert` / `World::resource_or_insert_with` for lazy
  resource setup,
  `World::entities()` for iterating all live entities, `World::spawn_batch` for
  bulk entity creation with shared components, `World::query_mut` for mutable
  single-component queries, `Entity::is_invalid` for sentinel checks,
  `Entity` with `Display` (`index#generation`), `Schedule::clear`,
  `Schedule::remove_by_name`, `Schedule::insert_at`, and `Schedule::replace_by_name` for runtime
  schedule management and selective execution, `Schedule::add_conditional` for
  systems gated by a `RunCondition` predicate, `Schedule::add_stage` /
  `Schedule::run_stage` / `Schedule::run_stage_with_hook` / `Schedule::run_all_stages` for named execution
  phases with optional per-system observability hooks, `Schedule::run_profiling`
  for zero-effort diagnostics (auto-inserts `Diagnostics` resource), `Events::peek` /
  `Events::last` for non-consuming event inspection, bounded
  `MessageLog::with_max`, a `Tag` marker component for entity grouping and
  filtering, `Rng` (seeded xorshift64 RNG) for reproducible randomness in
  tests, replays, and procedural generation (including `weighted_pick` for
  weighted random selection),   `GameClock` for tracking elapsed ticks,
  pause state, and real-time duration, `FixedTime` with `reset()` for fixed-timestep
  accumulation, `Diagnostics` with `Display`, `reset()`,
  `clear()`, `remove_system()`, per-system `avg_duration()`, `min_duration`,
  `sorted_by_duration()`, `sorted_by_max_duration()`, `sorted_by_avg_duration()`,
  `sorted_by_call_count()`, `system_count()`, `total_calls()`, `total_duration()`,
  `snapshot()` for serializable performance summaries, `Schedule` with `Display`,
  O(1) `entity_count()`, `Events` with `Clone`, and state machine management
  (`State<S>`, `StateTransitionEvent<S>`). `AudioEvents` type alias for
  ergonomic event channel usage.
  Optional `serde` feature enables `Serialize`/`Deserialize` on `Entity` and `DiagnosticsSnapshot`.
- `crates/verryte-input` - terminal-neutral input events, key/mouse/scroll
  bindings, script command bindings, sourced queued actions, replayable
  `ActionTrace`s, router-level script injection, pending queue snapshots and
  drain traces, the shared action queue, input context switching via
  `set_bindings` and `bindings_guard`, a context stack via
  `InputRouter::push_bindings` / `pop_bindings` for nested modal input, batch
  event processing (`handle_batch`, `handle_batch_with`), custom event
  translation (`handle_with` for position-aware inputs,
  `Bindings::merge` for layering keymaps,
  `CommandBindings::merge` for layering command sets,
  `Bindings::iter_keys` / `iter_mouse` / `iter_scroll` and
  `CommandBindings::iter_names` / `iter_glyphs` for binding inspection,
  `Bindings::clear` / `CommandBindings::clear` for removing all bindings,
  `InputRouter::total_actions_queued()` for lifetime action metrics,
  `InputRouter::start_recording` / `stop_recording` for action recording to
  disk (actions collected in memory, flushed via serde on stop),
  `InputRouter::recorded_as_trace` / `take_recording` for converting recordings
  into `ActionTrace` without disk I/O,
  `InputRouter::recorded_actions` for borrowing recorded actions without stopping,
  `Bindings::get_keys_for_action` for retrieving key mappings bound to an action,
  `ActionRecord::metadata_value` / `parse_metadata` for inspecting replay and
  agent metadata without depending on the backing map layout,
  `TextInput` for terminal text entry (prompts, naming, chat) with cursor
  movement, insertion, deletion, max length, dirty tracking, undo/redo,
  autocomplete cycling, word jumps (Ctrl+Left/Right), word deletion
  (Ctrl+Backspace/Delete), and Ctrl shortcut editing (A/E/B/F/U/W/K/Z/Y),
  `ActionHistory` with `iter`, `get`, `last`, `by_source`, `filter`, and
  `time_range` for recorded action analysis, and
  `ActionSource` with `Display`/`FromStr` for serialization and debugging.
  `Key`, `MouseButton`, and `ScrollDirection` have `Display` for logging.
  Exposes event simulation helpers (`simulate_key_press`, `simulate_mouse_press`, etc.)
  on `InputRouter` for ergonomic interactive testing and scripting.
  Modularized into focused sub-modules (`key`, `action`, `bindings`,
  `trace`, `router`, `text_input`, `replay`) for maintainability.
  Optional `serde` feature enables `Serialize`/`Deserialize` on `Key`,
  `MouseButton`, `ScrollDirection`, `MouseTrigger`, `InputEvent`,
  `ActionSource`, and `QueuedAction<A>`.
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
  hazard-distance safety scoring (`safer_neighbors4`), `DijkstraMap::find_all_within_range`,
  `DijkstraMap::chase_path_to_range` for tactical approach and retreat patterns,
  random-walk dungeon
  generation (`TileGrid::random_walk_fill4`),  BSP dungeon generation (`TileGrid::generate_bsp_dungeon`),
  weighted predicate pathfinding (`shortest_path_to_predicate4_weighted`,
  `shortest_path_to_predicate8_weighted`), `TileGrid::count_matching`,
  `TileGrid::find_matching`, `TileGrid::points_in`,
  `TileGrid::points_matching`, and `TileGrid::density` for map analysis,
  `TileGrid::bounds` and
  `TileGrid::bounding_box_of` with `Bounds` (with `Display`) / `Bounds::clamp_point` plus
  `Bounds::intersects` / `Bounds::intersection` for spatial framing,
  `Rect::contains_rect` for full containment checks, `SpatialHash<T>`
  with `Display` for debug/logging output, for efficient proximity queries on grid-based
  entities, cellular automata cave
  generation
  (`TileGrid::cellular_automata_cave`) for organic procedural maps,
  `TileGrid::from_ascii` for constructing grids from multi-line string
  literals, `TileGrid::map_tiles` for transforming tile types, and
  `TileGrid::crop` for extracting rectangular sub-regions as new grids.
- `crates/verryte-terminal` - terminal-cell data structures: colors, cells,
  grids, clipping, borders, line drawing, blitting, viewports, frame diffs,
  plain-text snapshots, ANSI-colored output (`Grid::to_ansi_string` with
  optimized `write!` rendering), HTML
  output (`Grid::to_html_string`) for web/debug viewing, circle drawing and
  filling (`Grid::draw_circle`, `Grid::fill_circle`), circular sector filling (`Grid::fill_sector`), diamond/rhombus shapes
  (`Grid::draw_diamond`, `Grid::fill_diamond`), Unicode box-drawing borders
  (`draw_border_rounded`, `draw_rounded_panel`, `draw_text_box`), horizontal/vertical lines
  (`draw_hline`, `draw_vline`), progress bars (`Grid::draw_progress_bar`),
  text wrapping utilities (`wrap_text`, `write_wrapped_text`, `write_lines`),
  `Grid::transform`, `Grid::transform_rect`, `Grid::map`, and grid transformations
  (flip, rotate clockwise/counter-clockwise via `flipped_horizontally`,
  `flipped_vertically`, `rotated_90`, `rotated_180`, `rotated_270`) for bulk cell modification
  and layout adjustments, row/column helpers
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
  Widgets:  `MenuView` with scroll support for long option lists,
  `VerticalProgressBar` for bottom-to-top fills, `Tooltip` for floating
  context hints, `PerformanceOverlay`, and `MessageLogView`.
  `Camera::follow()` for smooth entity tracking, `Camera::zoom_in` / `zoom_out` with clamps,
  and `Camera::focus_on_points` to dynamically frame multiple targets.
  `image_to_grid` converts PNG images to half-block terminal grids, and
  `image_to_grid_with_chroma_key` adds transparency support for sprite loading.
  The `vfx` module provides a reusable visual effects system: particles
  (fire, ice, lightning, slash, heal, burst, bloom, shatter, shockwave, vortex), screen shake,
  flash overlays, floating damage text, AoE ring indicators, shockwave/vortex triggers
  (`trigger_shockwave`, `trigger_vortex`), and clear operations (`VfxSystem::clear`) — all rendered
  directly into a `Grid` with emitter presets. All VFX types (`ScreenShake`,
  `FloatingText`, `AoeRing`, `VfxSystem`, `blend_color`, emit functions) are
  re-exported at the crate root for convenience.
- `crates/verryte-tty` - TTY frontend using crossterm: alternate screen,
  input polling with full modifier key passthrough (Ctrl, Alt, Shift produce
  `Key::Modified` events), Grid rendering with ANSI colors and cell attributes
  (bold, dim, italic, underline, blink, reverse), incremental
  diff-based rendering (`render_diff`) with automatic full-render fallback
  on terminal resize, and
  `terminal_size()` for querying the current terminal dimensions.
- `prototype/wuthering-terminal` - a 2D turn-based tactical RPG prototype.
  Features team-swapping, Echo absorption, telegraphed enemy attacks with
  parry/dodge, an alchemy crafting system for item combinations, equipment
  upgrades, enemy-awarded set gear, and special effects (lifesteal and per-turn HP regeneration),
  floor-by-floor progression with procedural BSP dungeon generation, and an adaptive resolution sprite system that scales visual
  fidelity to the user's terminal size. Sprites are compiled from PNG pixel
  art into static Rust arrays at build time using half-block sub-pixel packing.
- `prototype/vfx-demo` - interactive terminal VFX demo proving real-time
  animation at 30 FPS. Features a particle system (fire, ice, lightning,
  slash, burst, heal, AoE), screen shake, flash overlays, floating damage
  text, expanding ring indicators, combo counter, and diff-based rendering.
  Loads PNG character sprites (Kael, Mira, Blight Sovereign) via `image_to_grid()`
  with chroma-key transparency. Run with `cargo run -p vfx-demo`.

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
- action histories can attach string metadata such as serialized outcomes, and
  replay/agent tools can inspect those values through `ActionRecord` helpers.

## Wuthering Terminal

Wuthering Terminal includes two runners:

**Script runner** (non-TTY, for tests/CI):
```sh
cargo run -p wuthering-terminal --bin wuthering-terminal-script -- "inspect:4,4 confirm inspect:4,5 confirm"
```

**Agent runner** (line-oriented JSON protocol):
```sh
printf 'snapshot\nnorth\n' | cargo run -p wuthering-terminal --bin wuthering-terminal-agent
```

Agent commands use `ActionSource::Agent` in their step reports, keeping agent
control distinguishable from scripts without changing the shared action path.
Snapshots expose active modifier names alongside their remaining turns and
weather danger-zone coordinates so a headless controller can plan from state
rather than scrape the rendered frame.

**Interactive TTY** (real terminal):
```sh
cargo run -p wuthering-terminal --bin wuthering-terminal
```

`verryte-input` command bindings accept action tokens, e.g. for team swapping, skills, item use, crafting, equipment upgrades, save/load, recording, replay controls, UI/tool toggles, and target selections. The script runner parses these commands and validates game logic with structured outcomes, including item use, crafting, equipment upgrade, set reward, echo absorption, boss phase transition, save/load, recording/replay state changes, replay steps, rest recovery, floor modifier rerolls, scheduled deeper-floor events, status views, and precise failure reports for invalid inventory/crafting/floor/replay actions.
The script runner prints the rendered frame, viewport, state summary, source, action result, and event outcomes after each action.

## Verification

The normal check for the workspace is:

```sh
cargo fmt --check
cargo test
```

This environment must have the Rust toolchain on `PATH` for those commands.
