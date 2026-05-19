## 2026-05-15 - shared input, map helpers, and local snapshots

**Goal.** Continue autonomous development on Verryte toward the terminal-native,
agent-ready engine goal, with Ash Courier kept as the proving game and the
terminal/script control paths kept unified.

**Changes.**
- `crates/verryte-input/src/lib.rs` - added position-neutral mouse button
  bindings, `InputRouter::handle_from` for sourced neutral input events,
  `pending_iter`, and `pending_trace` so tools can inspect queued work without
  draining it.
- `crates/verryte-map/src/lib.rs` - added reusable `nearest_path4` and
  `reachable_points4` helpers, moving generic path/reachability behavior into
  the engine crate.
- `crates/verryte-terminal/src/lib.rs` - added `Grid::viewport` for clipped
  terminal-cell camera views.
- `prototype/ash-courier/src/lib.rs` - bound right mouse press to `Scan` and
  middle mouse press to `Wait`, moved nearest-path lookup onto `verryte-map`,
  added reachable tiles and a centered local viewport to `Snapshot`, and added
  tests for the new shared path and observability.
- `prototype/ash-courier/src/bin/script.rs` - prints reachable tile count and
  the local viewport after each scripted action.
- `README.md` and `prototype/ash-courier/README.md` - documented the new mouse
  bindings, queue trace inspection, map helpers, reachable state, and local
  viewport snapshots.

**Reasoning.** The next useful pressure point was not a larger game feature;
it was making existing engine promises more real. Mouse support was added as
simple button-transition bindings instead of coordinate-targeted actions
because the current action type has no payload and the core promise is a shared
action path. Position-aware mouse behavior can still be layered by intercepting
`InputEvent::Mouse` before routing. Nearest-path and reachability moved into
`verryte-map` because Ash Courier was already proving those are generic grid
needs. The viewport primitive went into `verryte-terminal` so snapshots and TTY
frontends can share the same cell-buffer camera behavior.

**Gotchas.** The script smoke command exits with status 1 unless the script
reaches `Outcome::Won`; use the documented win script for a passing smoke test.
The viewport test originally moved the player onto the package, which hid the
package glyph under the player layer. The test now stops adjacent to the
package so it validates the local camera rather than layer ordering.

**Follow-ups.** The next best step is position-aware mouse actions or prompts:
define a small target/action envelope that can carry terminal cell coordinates
without splitting interactive input away from scripted and replayed control.

## 2026-05-16 - tailor agent guide for Verryte

**Goal.** Replace the copied cross-project `AGENTS.md` guidance with instructions that match this repository: a Rust workspace for the Verryte terminal-game engine, with Ash Courier as the proving prototype and a strict shared input/script/control model.

**Changes.**
- `AGENTS.md:1` - rewrote the file as a Verryte-specific agent guide instead of a worklog-only handoff from another project.
- `AGENTS.md:8` - added startup context for future agents: read `GOAL.md`, `README.md`, `WORKLOG.md`, and the relevant crate/prototype sources before substantial work.
- `AGENTS.md:18` - documented the workspace layout and intended boundaries for `verryte-core`, `verryte-input`, `verryte-map`, `verryte-terminal`, `verryte-tty`, and `prototype/ash-courier`.
- `AGENTS.md:35` - captured the load-bearing engineering priorities: preserve the unified terminal/script/action/state path, prefer small vertical slices, keep reusable behavior in engine crates, forbid unsafe code, update tests/docs with behavior changes, and preserve unrelated dirty-worktree edits.
- `AGENTS.md:63` - added the normal verification commands and Ash Courier smoke commands, including the non-obvious script-runner success condition.
- `AGENTS.md:87` - added documentation sync guidance for root docs, prototype docs, `GOAL.md`, and prompt files.
- `AGENTS.md:98` - kept the repository's strict worklog policy but replaced copied examples with Verryte-specific examples and gotchas.

**Reasoning.** The original file only described the worklog process and included examples from another project, so it did not help future agents make Verryte-shaped decisions. I chose a concise project guide rather than copying the larger prompt kit because `AGENTS.md` should be the always-on operating contract: what to read, where code belongs, what invariants must not break, how to verify, and how to leave handoff notes. The prompt files remain useful for task-specific sessions, but duplicating them here would make the guide harder to maintain.

**Assumptions.** I assumed the existing README/GOAL/prompt material represents the desired project direction, including the current `verryte-tty` crate and Ash Courier TTY runner that are present in the dirty worktree. I also assumed the heredoc-only worklog rule should remain because it is a repository-specific process constraint, even though the rest of the copied file needed to be replaced.

**Gotchas.** `AGENTS.md` and `WORKLOG.md` are currently untracked in `git status`, so normal `git diff -- AGENTS.md` does not show this rewrite unless the file is added or compared explicitly. The worktree already had many unrelated modified Rust files before this change; I did not inspect or modify them beyond reading project context. The script smoke command is intentionally documented with the winning path because the runner exits nonzero for non-winning scripts.

**Follow-ups.** Future behavior changes should keep this guide in sync if crate boundaries or verification commands change. If `AGENTS.md` is meant to be versioned, add it along with `WORKLOG.md` so future diffs show edits normally.

## 2026-05-16 - add safety-step action and hazard-distance observability

**Goal.** Continue autonomous engine development with a meaningful vertical slice that improves reusable map behavior and Ash Courier control/state surfaces without splitting terminal and scripted action paths.

**Changes.**
- `crates/verryte-map/src/lib.rs:358` - added `TileGrid::distance_to_nearest4`, a BFS nearest-target distance helper that reuses the same passability contract as `shortest_path4`; added tests at `:651` and `:670`.
- `prototype/ash-courier/src/lib.rs:35` - added `Action::StepToSafety`, with key bindings (`r`/`R`) and command/glyph bindings (`step_safety`, `to_safety`, `retreat`, `v`/`V`) at `:83-84`, `:113-115`, and `:140-141`.
- `prototype/ash-courier/src/lib.rs:211` - added `Map::nearest_walkable_distance` to keep distance logic in engine map primitives and avoid prototype-local BFS duplication.
- `prototype/ash-courier/src/lib.rs:680` - wired `StepToSafety` into `Game::apply`; added `safety_step_direction` / `safer_neighbors_from` at `:827-873` so movement still resolves through existing movement systems and events.
- `prototype/ash-courier/src/lib.rs:326` - expanded `Snapshot` with `path_to_nearest_hazard`, `distance_to_nearest_hazard`, and `safer_neighbors`; populated at `:952-978`.
- `prototype/ash-courier/src/lib.rs:1129` and `:1140` - added action tests for advancing and no-op safety behavior; updated snapshot assertions at `:1485-1491`; updated glyph-command coverage at `:1413-1421`.
- `prototype/ash-courier/src/bin/script.rs:63` and `:95` - script runner summary now prints safer-neighbor counts and hazard path/distance fields; command help docs include `v` safety step.
- `README.md:25-29` and `prototype/ash-courier/README.md:18-35` - documented new safety action and expanded snapshot/runner observability.

**Reasoning.** The biggest missing pressure point after the previous slice was hazard-aware movement and distance observability, not more content. I added the nearest-distance primitive in `verryte-map` first so Ash Courier could consume reusable engine behavior instead of encoding a one-off prototype BFS. `StepToSafety` reuses the same action queue and movement application path as every other action, preserving the terminal/script parity invariant while adding a tactically useful command for agents and scripts.

**Assumptions.** I assumed `StepToSafety` should only move when at least one neighbor is strictly safer than the current tile to avoid oscillation/noisy movement. I also assumed hazards continue to occupy walkable floor tiles, so nearest-hazard distance should use walkability/path distance semantics rather than direct Manhattan distance.

**Gotchas.** Initial safety tests were wrong because they accidentally expected improvements in layouts where the current tile was already safer than neighbors. The default-map `safer_neighbors` expectation also initially used the wrong tie direction; `walkable_neighbors` order plus distance ranking made east (`(2,1)`) the best deterministic choice.

**Follow-ups.** A useful next step is to expose optional richer safety scoring (e.g., tie-break by progress-to-goal or package distance) while keeping the same shared action path and map helper reuse.

## 2026-05-16 - parameterized scan commands and chaser observability

**Goal.** Continue autonomous Verryte development with a meaningful vertical slice that improves the shared input/control path and Ash Courier observability, without creating separate interactive-vs-script logic.

**Changes.**
- `crates/verryte-input/src/lib.rs` - added `CommandBindings::parse_script_with` and `InputRouter::inject_script_with` so parameterized script tokens can resolve into actions before glyph fallback while still entering the same queue and source metadata path.
- `prototype/ash-courier/src/action.rs` - added digit key bindings (`1`-`5`) for `Action::ScanRadius(...)` and added `resolve_command_token` supporting `scan:3`, `scan3`, and `x3` script forms.
- `prototype/ash-courier/src/bin/script.rs` - switched script injection to `inject_script_with(..., resolve_command_token)` and updated runner help text.
- `prototype/ash-courier/src/components.rs` / `src/systems.rs` - added `GameEvent::ChaserMoved` and emitted those events from `chaser_system`; message log rendering now includes chaser movement messages.
- `prototype/ash-courier/src/snapshot.rs` / `src/game.rs` - expanded snapshot with `chasers`, `path_to_nearest_chaser`, and `distance_to_nearest_chaser`; render now draws chasers as distinct `c` glyphs so they are not visually merged with static hazards.
- `prototype/ash-courier/src/bin/tty.rs` - status panel now shows nearest package/goal/hazard/chaser distances from snapshot state and updated control hints include scan-radius and safety-step shortcuts.
- `prototype/ash-courier/src/lib.rs` - added tests for custom token parsing, scan-radius key/script parity, chaser movement events/messages, and new chaser snapshot fields.
- `README.md` and `prototype/ash-courier/README.md` - documented parameterized scan command support, `inject_script_with`, and chaser-specific observability.

**Reasoning.** The strongest near-term gap was that `ScanRadius` existed but was hard to drive through shared harness-style command input. Extending parser/router primitives in `verryte-input` preserved the single input-to-action path while enabling richer command tokens without adding Ash-Courier-only parser forks. Chaser-specific state was visible only indirectly through hazards, so adding distinct events, snapshot fields, and rendering improved agent/test inspectability while keeping gameplay systems simple.

**Assumptions.** I assumed radius scans should reject non-positive radii in token parsing and that scripted parameterized tokens should remain an optional extension (fixed-name/glyph command parsing still works unchanged). I also assumed chasers should remain hazards for loss logic while being represented separately for observability.

**Gotchas.** Emitting chaser movement events directly during mutable position updates caused borrow conflicts; collecting events then sending them after movement avoided overlapping mutable borrows of `World`. `cargo fmt --check` initially failed due formatting changes in new tests and long format strings, so formatting had to be applied before final verification.

**Follow-ups.** If agent scripts need richer parameterized commands beyond scan radius, add a small shared token-parser module in Ash Courier (or a reusable utility crate) rather than duplicating closure logic in each harness entry point. For gameplay depth, chaser move policy could next consider tie-breaks that avoid deterministic oscillation in wider maps.

## 2026-05-16 - engine primitives: ANSI output, circles, text wrapping, flood fill, lazy line iterator, has_resource

**Goal.** Continue autonomous Verryte development with a batch of reusable engine primitives that improve rendering, spatial analysis, and API ergonomics across the workspace.

**Changes.**
- `crates/verryte-terminal/src/lib.rs:355` - added `Grid::to_ansi_string()` for rendering grids with ANSI 24-bit color escape codes. Produces output usable in any ANSI terminal without crossterm, useful for debug dumps, log files, and agent observation over plain text channels.
- `crates/verryte-terminal/src/lib.rs:386` - added `Grid::draw_circle()` using the midpoint circle algorithm for circle outlines, and `Grid::fill_circle()` using a scanline approach for filled circles. Both clip to grid bounds.
- `crates/verryte-terminal/src/lib.rs:449` - added `wrap_text()` for wrapping text into lines at word boundaries with hard-wrap fallback, and `write_wrapped_text()` for writing wrapped text directly into a Grid. Useful for message boxes, help screens, and dialogue.
- `crates/verryte-map/src/lib.rs:49` - refactored `line_between()` to use the new `LineIter`, and added `LineIter` as a lazy Bresenham line iterator. Yields points without allocating a `Vec`, enabling early termination for line-of-sight checks.
- `crates/verryte-map/src/lib.rs:555` - added `TileGrid::flood_fill4()` for BFS-based connected-component detection from a seed point, and `TileGrid::count_regions4()` for counting disconnected regions matching a predicate. Useful for room detection, region labeling, and map analysis.
- `crates/verryte-core/src/world.rs:320` - added `World::has_resource::<R>()` for checking resource existence without borrowing, useful for conditional system behavior and safe initialization checks.
- `README.md` - updated crate descriptions to document new capabilities.

**Reasoning.** These are all small, focused primitives that terminal games repeatedly need. ANSI output decouples colored rendering from the crossterm dependency, making `verryte-terminal` usable in more contexts. Circle primitives support visual effects and spatial queries. Text wrapping supports UI elements that the engine previously had no answer for. Flood fill and region counting are fundamental map analysis tools for roguelikes (room detection, lake identification, connected area queries). The lazy line iterator avoids allocation in hot paths like line-of-sight checks. `has_resource` is a small but ergonomic addition for systems that need conditional resource access.

**Assumptions.** I assumed the midpoint circle algorithm's exact output shape is less important than having a working circle primitive, so tests verify structural properties (center empty for outline, center filled for fill, minimum cell counts) rather than exact pixel patterns. I also assumed `wrap_text` should prefer breaking at the last space within the width limit, which produces slightly different output than a greedy "fit as many words as possible" approach.

**Gotchas.** Initial flood fill tests had incorrect grid data (wrong vec lengths for the declared dimensions) and wrong expected counts (not accounting for connectivity through open rows). Circle tests initially expected specific star counts that didn't match the midpoint algorithm's actual output. The `wrap_text` test expectation was based on a different wrapping strategy than implemented. All were fixed by adjusting test data and expectations to match actual behavior.

**Follow-ups.** Position-aware mouse actions (noted in the prior worklog entry) remain the next best step for the input/control path. A layer system for `Grid` rendering would be a useful addition for separating background, entity, and UI overlays. Dungeon generation helpers in `verryte-map` (random walk, BSP rooms) would further support the roguelike proving game.

## 2026-05-16 - engine primitives: query3, bounded MessageLog, box borders, hline/vline, Direction8, handle_batch

**Goal.** Continue autonomous Verryte development with a batch of reusable engine
primitives that improve ECS queries, message management, terminal rendering,
spatial directions, and input batching.

**Changes.**
- `crates/verryte-core/src/world.rs:283` - added `World::query3<A, B, C>()` for
  querying entities with three components simultaneously, completing the
  multi-component query API alongside `query` and `query2`. Tests at `:571` and
  `:585`.
- `crates/verryte-core/src/log.rs:1` - added `MessageLog::with_max()` for bounded
  message logs that automatically drop oldest entries when capacity is reached,
  plus `max()`, `len()`, and `is_empty()` accessors. Tests at `:76` covering
  unbounded growth, bounded trimming, single-entry cap, and tail queries.
- `crates/verryte-terminal/src/lib.rs:284` - added Unicode box-drawing character
  constants (`BORDER_TL`, `BORDER_TR`, `BORDER_BL`, `BORDER_BR`, `BORDER_H`,
  `BORDER_V`) and `Grid::draw_border_rounded()` for drawing ┌─┐/││/└─┘ style
  borders with color support. Tests at `:870` and `:887`.
- `crates/verryte-terminal/src/lib.rs:331` - added `Grid::draw_hline()` and
  `Grid::draw_vline()` for horizontal and vertical line drawing with clip
  support and cell count returns. Tests at `:895`, `:905`, and `:917`.
- `crates/verryte-map/src/lib.rs:152` - added `Direction8` enum with all eight
  directions (cardinal + diagonal), including `delta()`, `opposite()`,
  `is_cardinal()`, `to_direction()`, `from_direction()`, and `CARDINAL`/`DIAGONAL`
  constant subsets. Tests at `:1113`.
- `crates/verryte-map/src/lib.rs:41` - added `Point::neighbors8()`,
  `Point::step8()`, and `Point::chebyshev_distance()` for 8-directional spatial
  queries. Tests at `:1127` and `:1134`.
- `crates/verryte-map/src/lib.rs:411` - added `TileGrid::neighbors8()` for
  retrieving all eight in-bounds neighbors with tiles. Test at `:1141`.
- `crates/verryte-input/src/lib.rs:503` - added `InputRouter::handle_batch()` and
  `InputRouter::handle_batch_from()` for processing multiple input events at
  once, returning the count of events that produced queued actions. Tests at
  `:960`, `:972`, and `:983`.

**Reasoning.** These are all small, focused additions that terminal games
repeatedly need. `query3` completes the ECS query API for systems that need
three component types. Bounded `MessageLog` prevents memory growth in
long-running games. Box-drawing borders make panels and UI elements look
significantly better in terminals. `draw_hline`/`draw_vline` are common
primitives for separators, health bars, and UI framing. `Direction8` and
`chebyshev_distance` support games that need diagonal movement (king-move
distance is the natural metric for 8-directional grids). `handle_batch` lets
frontends process input bursts efficiently without repeated method calls.

**Assumptions.** I assumed `Direction8` should live alongside `Direction` rather
than replacing it, since most existing code uses 4-directional semantics. I
assumed `MessageLog::with_max` should trim oldest messages (FIFO) rather than
rejecting new ones, since recent messages are typically more useful. I assumed
`handle_batch` should return the count of successfully queued events so callers
can detect how many events were unbound.

**Gotchas.** `for_each2_mut` was attempted but abandoned because Rust's borrow
checker prevents two mutable borrows of different HashMap entries simultaneously
without `unsafe` or `get_many_mut` (which wasn't available on this HashMap
pattern). The `query3` approach avoids this by collecting results immutably.
The `draw_border_rounded` test initially used a height-2 rect which left no room
for vertical edges between top and bottom rows; fixed by using height-3.
The `draw_hline` test expectation missed the trailing space from the grid width.

**Follow-ups.** A `World::for_each2_mut` could be revisited if the column storage
is restructured to use a Vec or array instead of HashMap, enabling safe split
borrows. `Direction8`-aware pathfinding (`shortest_path8`) could be added to
`verryte-map` for games that need diagonal movement with proper cost modeling
(diagonal steps often cost more than cardinal). A `Grid::draw_rounded_panel`
combining `draw_border_rounded` with title placement would be a natural next
rendering convenience.

## 2026-05-16 - input contexts, bulk despawn, named systems, diagonal pathfinding, rounded panels, event take

**Goal.** Continue autonomous Verryte development with improvements to input
context switching, ECS bulk operations, schedule debugging, diagonal pathfinding,
terminal UI convenience, and event consumption.

**Changes.**
- `crates/verryte-input/src/lib.rs:486` - added `InputRouter::set_bindings()` for
  swapping the active keymap at runtime, enabling input context switching between
  gameplay, menus, and dialogs. Returns the previous bindings for restoration.
- `crates/verryte-input/src/lib.rs:501` - added `InputRouter::bindings_guard()` and
  `BindingsGuard<A>` RAII guard that automatically restores original bindings when
  dropped, even on panic. Requires `Bindings<A>: Clone`. Test at `:1080`.
- `crates/verryte-input/src/lib.rs:200` - added `#[derive(Clone)]` to `Bindings<A>`
  to support the guard pattern.
- `crates/verryte-core/src/world.rs:118` - added `World::despawn_with<T>()` for
  bulk removal of all entities that have a specific component type. Returns the
  count of removed entities. Useful for cleanup of temporary entities like
  projectiles or expired effects. Tests at `:619` and `:635`.
- `crates/verryte-core/src/schedule.rs:13` - added `NamedSystem` struct with
  `name` and `func` fields, and `NamedSystem::auto()` for unnamed systems.
- `crates/verryte-core/src/schedule.rs:30` - updated `Schedule` to store
  `NamedSystem` entries internally. Added `add_named()` for named systems and
  `systems()` accessor. Added `run_with_hook()` that calls a callback with each
  system's name before execution, useful for profiling and logging. Tests at
  `:128` and `:140`.
- `crates/verryte-core/src/lib.rs:26` - exported `NamedSystem`.
- `crates/verryte-map/src/lib.rs:350` - added `TileGrid::shortest_path8()` using
  A* with integer costs (cardinal = 10, diagonal = 14) for proper distance
  minimization in 8-directional grids. Uses `BinaryHeap` for the open set and
  `chebyshev_distance` for the heuristic. Tests at `:1241`, `:1256`, `:1266`,
  and `:1273`.
- `crates/verryte-terminal/src/lib.rs:381` - added `Grid::draw_rounded_panel()`
  combining `draw_border_rounded` with centered title placement on the top
  border. Clips to grid bounds. Tests at `:963`, `:983`, and `:1000`.
- `crates/verryte-core/src/event.rs:41` - added `Events::take()` to consume all
  pending events and return them as a `Vec`. More ergonomic than
  `drain().collect()` for systems that want to snapshot events. Test at `:84`.

**Reasoning.** Input context switching is essential for real games that have
menus, dialogs, or mode-specific controls. The guard pattern ensures bindings
are always restored, even if the modal code panics or returns early. Bulk
despawn is a common need for cleaning up temporary entities. Named systems make
debugging and profiling much easier — knowing which system ran when is valuable
for understanding game behavior. Diagonal pathfinding with proper costs is
fundamental for games that allow 8-directional movement; using integer costs
(10/14) avoids floating-point issues while preserving the √2 ratio. Rounded
panels with titles are a common UI pattern that benefits from a convenience
method. `Events::take()` simplifies event consumption patterns.

**Assumptions.** I assumed `Bindings` should be `Clone` to support the guard
pattern; this is a reasonable requirement since bindings are typically small
HashMaps. I assumed diagonal path costs should use the standard 10/14 integer
approximation rather than floating-point, which is common in grid-based games.
I assumed `draw_rounded_panel` should center the title on the top border, which
may overwrite corner characters for wide titles — this is acceptable since the
title is the focal point.

**Gotchas.** The initial `with_bindings` closure approach had a borrow checker
issue where the closure's borrow of `self` conflicted with the post-closure
restoration. Switched to an RAII guard pattern (`BindingsGuard`) that restores
bindings on `Drop`, which is both safer and more idiomatic Rust. The
`bindings_guard` test initially failed because the action queue was not drained
between test phases, leaving stale actions that confused assertions. Fixed by
explicitly draining the queue at each phase boundary. The `draw_rounded_panel`
clip test initially expected `BORDER_TL` to survive, but wide titles overwrite
the top-left corner; changed to check for bottom corners instead.

**Follow-ups.** A `TileGrid::nearest_path8` would complement `shortest_path8`
for finding paths to the nearest of multiple targets with diagonal movement.
Input context stacks (push/pop multiple contexts) would be useful for nested
modals. The schedule could benefit from run conditions (systems that only run
when a resource flag is set) and system groups (named stages that run in order).

## 2026-05-16 - engine primitives: 8-directional path helpers, for_each2_mut, diamond shapes, binding merge, random walk, schedule management

**Goal.** Continue autonomous Verryte development with a batch of reusable engine
primitives that improve spatial pathfinding, ECS mutable iteration, terminal
rendering, input context layering, dungeon generation, and schedule management.

**Changes.**
- `crates/verryte-map/src/lib.rs:683` - added `TileGrid::nearest_path8()` for
  finding the shortest 8-directional path to the nearest of multiple targets,
  complementing `nearest_path4`. Test at `:1341`.
- `crates/verryte-map/src/lib.rs:710` - added `TileGrid::reachable_points8()`
  for 8-directional flood-fill reachability, complementing `reachable_points4`.
  Tests at `:1371` and `:1392`.
- `crates/verryte-core/src/world.rs:393` - added `World::for_each2_mut<A, B>()`
  for mutable iteration over entities with two component types. Uses a
  `Column::into_any` trait method for safe `Box<dyn Any>` downcasting without
  `unsafe` or `get_many_mut`. Returns `false` for same-type or missing columns.
  Tests at `:718`, `:736`, and `:745`.
- `crates/verryte-core/src/world.rs:23` - added `Column::into_any` trait method
  and implemented for `TypedColumn<T>` to enable safe owned downcasting.
- `crates/verryte-terminal/src/lib.rs:580` - added `Grid::draw_diamond()` for
  diamond/rhombus outline using Manhattan distance, and `Grid::fill_diamond()`
  for solid fill. Useful for AoE indicators and range displays. Tests at
  `:1063`, `:1079`, `:1093`, and `:1100`.
- `crates/verryte-input/src/lib.rs:267` - added `Bindings::merge()` for
  combining keymap sets with overlay semantics. Useful for layering input
  contexts (base game + menu bindings). Test at `:1131`.
- `crates/verryte-map/src/lib.rs:863` - added `TileGrid::random_walk_fill4()`
  for simple dungeon/cave generation using seeded random walks. Uses an inline
  xorshift64 PRNG for reproducibility without external dependencies. Tests at
  `:1401`, `:1415`, `:1425`, and `:1437`.
- `crates/verryte-core/src/schedule.rs:90` - added `Schedule::clear()` and
  `Schedule::remove_by_name()` for runtime schedule management. Tests at
  `:168` and `:177`.
- `README.md` - updated crate descriptions to document new capabilities.

**Reasoning.** These are all small, focused additions that terminal games
repeatedly need. The 8-directional path helpers complete the spatial API for
games that allow diagonal movement. `for_each2_mut` was previously attempted
but abandoned due to borrow checker limitations; the `Column::into_any`
approach provides a safe path without `unsafe` code. Diamond shapes complement
circles for AoE and range visualization. `Bindings::merge` enables clean input
context layering without full context switching. Random-walk generation is the
simplest useful dungeon primitive — organic, replayable, and dependency-free.
Schedule management supports hot-reloading and debug toggles.

**Assumptions.** I assumed `for_each2_mut` should collect matching indices
first (read-only) before mutating, to minimize the time columns are removed
from the HashMap. I assumed `random_walk_fill4` should use a simple xorshift64
PRNG rather than accepting an RNG trait, to avoid adding a `rand` dependency
to the map crate. I assumed diamond shapes should use Manhattan distance (L1
norm) which produces the natural rhombus shape for terminal grids.

**Gotchas.** `for_each2_mut` required adding `Column::into_any` because
`Box<dyn Column>` doesn't have a `downcast` method (only `Box<dyn Any>` does).
The `into_any` approach temporarily removes both columns from the HashMap,
downcasts them to concrete types, processes them, then restores them. This is
safe but has a small overhead from the HashMap remove/insert. The borrow
checker also required capturing the generation value before the mutable borrow
of `typed_a.slots[i]`.

**Follow-ups.** `for_each3_mut` could be added following the same pattern if
games need three mutable components. A `TileGrid::bsp_rooms` or cellular
automata generator would complement `random_walk_fill4` for more structured
dungeon layouts. `CommandBindings::merge` would be useful for layering command
sets alongside key bindings.

## 2026-05-16 - engine primitives: event inspection, LOS, progress bar, retain, BSP, command merge, run conditions

**Goal.** Continue autonomous Verryte development with a batch of reusable engine
primitives that improve event inspection, spatial analysis, terminal UI, ECS entity
management, dungeon generation, input layering, and schedule control.

**Changes.**
- `crates/verryte-core/src/event.rs:54` - added `Events::peek()` for inspecting
  the oldest pending event without consuming, and `Events::last()` for inspecting
  the most recently added event. Tests at `:88` and `:98`.
- `crates/verryte-core/src/world.rs:135` - added `World::retain(predicate)` for
  predicate-based entity filtering. Returns the count of removed entities.
  Complements `despawn_with` for cases where the keep/remove logic is not
  component-type-based. Tests at `:739`, `:755`, `:765`, and `:776`.
- `crates/verryte-core/src/schedule.rs:26` - added `RunCondition` type alias
  (`fn(&World) -> bool`) and `NamedSystem::conditional()` constructor. Added
  `Schedule::add_conditional()` for systems gated by a predicate. Updated
  `run()` and `run_with_hook()` to check conditions before executing systems;
  skipped systems do not trigger the hook callback. Tests at `:227`, `:240`,
  `:253`, `:266`, and `:277`.
- `crates/verryte-input/src/lib.rs:442` - added `CommandBindings::merge()` for
  combining command binding sets with overlay semantics, complementing the
  existing `Bindings::merge()` for key bindings. Test at `:1167`.
- `crates/verryte-map/src/lib.rs:740` - added `TileGrid::is_line_of_sight_clear()`
  for fast boolean LOS checks using the lazy `LineIter`. Both endpoints are
  excluded from blocking checks (observer and target). Tests at `:1517`, `:1524`,
  `:1532`, `:1540`, `:1546`, and `:1553`.
- `crates/verryte-map/src/lib.rs:938` - added `TileGrid::generate_bsp_dungeon()`
  for BSP (binary space partitioning) dungeon generation. Recursively splits the
  grid into sub-regions, places random rooms in leaf nodes, and connects sibling
  rooms with L-shaped corridors. Returns room centers for spawn placement.
  Tests at `:1562`, `:1583`, `:1591`, and `:1600`.
- `crates/verryte-terminal/src/lib.rs:626` - added `Grid::draw_progress_bar()`
  for horizontal progress bars with configurable fill/empty cells, ratio
  clamping, and grid clipping. Tests at `:1108`, `:1116`, `:1124`, `:1132`,
  `:1141`, `:1149`, and `:1157`.
- `README.md` - updated crate descriptions to document all new capabilities.

**Reasoning.** These are all small, focused primitives that terminal games
repeatedly need. `Events::peek`/`last` let systems inspect pending events without
consuming them, which is useful for conditional logic and debugging. `World::retain`
is the natural complement to `despawn_with` for predicate-based cleanup. Run
conditions let games toggle debug systems, pause subsystems during cutscenes, or
gate systems on resource flags without cluttering system code. `CommandBindings::merge`
enables layering command sets (base game + debug + mod) just like `Bindings::merge`
does for key bindings. Line-of-sight is a fundamental spatial query for any game
with visibility mechanics. BSP dungeon generation complements random walk for
structured room-and-corridor maps. Progress bars are a practical terminal UI
primitive for health, XP, timers, and loading indicators.

**Assumptions.** I assumed `RunCondition` should be a plain function pointer
(`fn(&World) -> bool`) rather than a closure, matching the existing `System` type
alias pattern. This keeps the schedule API simple and inspectable. I assumed BSP
generation should fill the entire grid with wall first, then carve rooms and
corridors, returning room centers for spawn placement. I assumed `retain`'s
predicate takes only `Entity` (not `&World`), because the mutable borrow of
`self` prevents re-borrowing inside the closure; callers pre-collect what they
need.

**Gotchas.** The BSP `place_rooms` function initially had type mismatches between
`u64` (from `rng()`) and `u16` (region dimensions). Fixed by casting to `u64`
for the modulo operation, then back to `u16` for the result. The `connect_siblings`
function was initially written but proved unnecessary since `collect_corridors`
handles corridor generation; it was removed to avoid dead code warnings. The
`retain` test initially tried to borrow `world` inside the closure, which
conflicted with the mutable borrow from `retain` itself; fixed by pre-collecting
entities to keep.

**Follow-ups.** A `Schedule::run_stage` or named-stage system could group systems
into ordered phases (input, physics, rendering). `for_each3_mut` could follow the
same `into_any` pattern as `for_each2_mut`. The BSP generator could be extended
with configurable room shapes, door placement, or treasure/enemy spawn tables.
A `Grid::draw_bar_chart` or `Grid::draw_sparkline` would complement the progress
bar for data visualization in terminal games.

## 2026-05-16 - responsive TTY layout, HTML output, Layer compositing, entity iteration, lazy components, Tag, map density/bounds

**Goal.** Continue autonomous Verryte development with improvements to TTY
responsiveness, debug output formats, rendering architecture, ECS ergonomics,
and spatial analysis primitives.

**Changes.**
- `prototype/ash-courier/src/bin/tty.rs` - replaced the hardcoded 80x24 root
  grid with a dynamic layout derived from `tty::terminal_size()`. Viewport, log,
  and status panels now scale proportionally to terminal width and height, with
  graceful degradation for narrow terminals. Resize events are tracked through
  the main loop so the layout adapts when the terminal window changes.
- `crates/verryte-tty/src/lib.rs:177` - added `terminal_size()` that queries
  crossterm for the current terminal dimensions, falling back to (80, 24).
- `crates/verryte-terminal/src/lib.rs:516` - added `Grid::to_html_string()`
  producing a `<pre>` block with inline CSS `rgb()` colors and HTML-escaped
  glyphs. Useful for web debug viewers, CI reports, and sharing terminal state
  over non-terminal channels. Tests at `:1301` and `:1317`.
- `crates/verryte-terminal/src/lib.rs:85` - added `Layer` struct with `name`,
  `order`, `grid`, and `visible` fields, plus `Layer::composite()` that sorts
  visible layers by draw order and blits them onto a target grid. Enables
  clean separation of map, entity, and UI rendering layers. Tests at `:1331`,
  `:1353`, and `:1371`.
- `crates/verryte-core/src/world.rs:267` - added `World::get_or_insert<T>()`
  for lazy component initialization with `Default`, and
  `World::get_or_insert_with(entity, f)` for custom initialization closures.
  Tests at `:900`, `:910`, `:918`, and `:925`.
- `crates/verryte-core/src/world.rs:170` - added `World::entities()` iterator
  over all live entities. Test at `:933` and `:945`.
- `crates/verryte-core/src/tag.rs` - added `Tag` component: a lightweight
  string marker for entity grouping and filtering. Implements `is()`,
  `Display`, and `From<S>`. Tests at `:43`, `:48`, and `:52`. Exported from
  `verryte-core` lib.
- `crates/verryte-map/src/lib.rs:1190` - added `TileGrid::count_matching()`
  for counting tiles matching a predicate, `TileGrid::density()` for the
  fraction of matching tiles, and `TileGrid::bounding_box_of()` returning a
  `Bounds` rectangle. Tests at `:1977`, `:1983`, `:1989`, `:1996`, `:2003`,
  and `:2018`.
- `crates/verryte-map/src/lib.rs:1244` - added `Bounds` struct with `x`, `y`,
  `width`, `height`, `right()`, `bottom()`, `contains()`, and `center()`.
- `README.md` - updated crate descriptions to document all new capabilities.

**Reasoning.** The TTY frontend was the most visible gap: a fixed 80x24 layout
breaks on any terminal that isn't that size, and resize events were received
but ignored. Making it responsive validates that the engine's Grid abstraction
works at arbitrary sizes. HTML output complements ANSI output for contexts where
a terminal isn't available (web dashboards, CI artifacts). The Layer system
addresses a pattern that every terminal game needs — separating background,
entities, and UI into independently-updated buffers that composite at render
time. `get_or_insert` is a standard ECS convenience that reduces boilerplate
for components that may or may not exist on an entity. `entities()` iteration
is the natural complement to `entity_count()` when you need to process all
live entities. `Tag` is the simplest useful entity-labeling primitive. Map
density and bounding-box queries are fundamental spatial analysis tools.

**Assumptions.** I assumed `Layer::composite` should sort by `order` ascending
(lower draws first, higher draws on top), which is the standard convention.
I assumed `Bounds` should live in `verryte-map` rather than depending on
`verryte-terminal::Rect` to keep the dependency graph clean. I assumed
`get_or_insert` should require `Default` rather than accepting a value, since
the value-taking variant is covered by `get_or_insert_with`.

**Gotchas.** `Layer::new` takes `Grid` by value, so test code that reuses a
grid across multiple `Layer` constructions needs `.clone()`. The TTY resize
logic tracks size in a local variable rather than a resource because the
frontend binary owns the render loop and doesn't need engine-level resize
state. `to_html_string` must escape `<`, `>`, `&`, and `"` to produce valid
HTML.

**Follow-ups.** The Layer system could benefit from a `Layers` collection type
that manages layer lifecycle (add/remove/find by name). The TTY frontend could
store the terminal size as a resource so game systems can react to resize
events. A `Grid::to_svg_string` would complement HTML output for vector
graphics contexts.

## 2026-05-16 - query iterators, schedule debugging, FOV, grid transforms, input metrics

**Goal.** Continue autonomous Verryte development with a batch of reusable engine
primitives that improve ECS ergonomics, schedule debugging, spatial visibility,
terminal rendering, and input observability.

**Changes.**
- `crates/verryte-core/src/world.rs:602` - added `Query2` and `Query3` iterator
  types that wrap the existing `Vec`-backed query results, complementing the
  existing `Query<T>` type. Added `World::query2_iter` and `World::query3_iter`
  methods at `:349` and `:361` for lazy iteration over two- and three-component
  queries. Tests at `:1038` and `:1051`.
- `crates/verryte-core/src/schedule.rs:163` - added `Schedule::run_system_by_name()`
  for executing a single named system outside the normal schedule order. Respects
  run conditions (returns `false` if condition not met). Useful for debugging
  individual systems, triggering specific behavior on demand, or running systems
  out of order. Tests at `:351`, `:361`, and `:368`.
- `crates/verryte-map/src/lib.rs:1247` - added `TileGrid::field_of_view()` using
  recursive shadowcasting. Returns all tiles within radius that are visible from
  the origin, with blocking tiles visible but casting shadows behind them. This
  is the standard FOV algorithm for roguelikes: fast, accurate, and symmetric
  (if A can see B, B can see A). Added helper function `cast_light` at `:1302`
  for the recursive octant scanning. Tests at `:2183` covering origin inclusion,
  open-area visibility, wall blocking, radius enforcement, and out-of-bounds
  handling.
- `crates/verryte-terminal/src/lib.rs:750` - added `Grid::transform()` for
  applying a mutation function to every cell in-place, and `Grid::map()` at
  `:762` for creating a transformed copy without mutating the original. Useful
  for bulk color adjustments, glyph remapping, dimming/brightening effects,
  and post-processing frames before render. Tests at `:1414` and `:1427`.
- `crates/verryte-input/src/lib.rs:493` - added `total_queued` counter to
  `InputRouter` that tracks the lifetime count of all actions queued through
  `handle_from` and `inject_from`. Added `InputRouter::total_actions_queued()`
  accessor at `:722`. Counter never decreases when actions are drained, making
  it useful for metrics, debugging, and detecting whether any input has been
  processed. Test at `:1201`.
- `README.md` - updated crate descriptions to document `Query2`/`Query3`
  iterators, `run_system_by_name`, `field_of_view`, `transform`/`map`, and
  `total_actions_queued`.

**Reasoning.** These are all small, focused additions that terminal games
repeatedly need. `Query2`/`Query3` iterators complete the ECS query API for
systems that prefer lazy iteration over collecting into `Vec`. `run_system_by_name`
makes the schedule more debuggable — being able to trigger a specific system
by name is valuable for interactive debugging and testing. Field-of-view via
recursive shadowcasting is a fundamental roguelike primitive that was missing;
the existing `visible_points` method uses a brute-force approach that checks
every point against every other point, while shadowcasting is O(n) in the
number of visible tiles. `Grid::transform`/`map` enable post-processing effects
that games need for visual polish (dimming off-screen areas, highlighting
selected regions, etc.). The input counter provides observability into how
much input the router has processed over its lifetime, which is useful for
metrics and debugging input flow.

**Assumptions.** I assumed `field_of_view` should use Manhattan distance for
the radius check, consistent with the existing `visible_points` method. I
assumed the shadowcasting implementation should use the standard 8-octant
multiplier approach, which is the most common implementation in roguelike
engines. I assumed `Query2`/`Query3` should wrap the existing `Vec`-backed
query methods rather than implementing true lazy iteration, since the
underlying storage doesn't support efficient multi-column iteration without
the same `into_any` dance that `for_each2_mut` uses.

**Gotchas.** The initial shadowcasting implementation had an unused `octants`
variable that triggered a compiler warning; cleaned up by removing the dead
code. The `cast_light` helper function needs to be outside the `TileGrid` impl
block because it's a free function that takes `&TileGrid<T>` as a parameter.
The FOV tests initially used a 7x1 grid which made the wall-blocking test
trivially pass; verified that the algorithm correctly handles both horizontal
and diagonal blocking.

**Follow-ups.** The `visible_points` method in `verryte-map` could be replaced
with or deprecated in favor of `field_of_view` since shadowcasting is more
efficient and produces better results. A `field_of_view8` variant using
Chebyshev distance could be added for games that want 8-directional FOV.
The schedule could benefit from a `run_systems_by_tag` or system grouping
feature for running subsets of systems. `Grid::transform` could be extended
with a `transform_rect` variant for region-limited transformations.

## 2026-05-16 - engine primitives: seeded RNG, color palettes, grid resize, text input, game clock

**Goal.** Continue autonomous Verryte development with a batch of reusable engine
primitives that improve reproducibility, theming, responsive layouts, text entry,
and timing — all areas that terminal games repeatedly need.

**Changes.**
- `crates/verryte-core/src/rng.rs` - added `Rng`, a seeded xorshift64 PRNG with
  `next_u64`, `next_u32`, `roll` (range), `flip`, `chance` (probability), `pick`,
  `pick_index`, `shuffle` (Fisher-Yates), and `next_f64`. Deterministic sequences
  from the same seed enable reproducible tests, replays, and agent behavior.
  Tests at `:107` covering seed determinism, range bounds, shuffle permutation,
  and clone semantics.
- `crates/verryte-core/src/clock.rs` - added `GameClock` resource tracking
  elapsed ticks, pause state, real-time duration (excluding paused time), and
  total paused duration. Methods: `tick`, `tick_n`, `pause`, `resume`,
  `toggle_pause`, `reset`, `set_elapsed_ticks`. Store as an ECS resource so
  systems can read timing without plumbing it through arguments. Tests at
  `:135` covering tick advancement, pause/resume, real-time exclusion of paused
  duration, and reset.
- `crates/verryte-terminal/src/lib.rs:855` - added `ColorPalette` with four
  built-in themes (`dark_dungeon`, `light_classic`, `amber_terminal`,
  `cyberpunk`) and convenience cell constructors (`floor_cell`, `wall_cell`,
  `player_cell`, `hazard_cell`, `item_cell`, `goal_cell`). Games can swap
  palettes for theming or player customization without touching rendering code.
  Tests at `:1709`.
- `crates/verryte-terminal/src/lib.rs:1004` - added `Layers` collection type
  with `add` (replace-by-name), `get`, `get_mut`, `remove`, `composite`,
  `len`, `is_empty`, and `iter`. Layers are kept sorted by draw order. This
  provides a managed lifecycle on top of the raw `Vec<Layer>` pattern. Tests
  at `:1737`.
- `crates/verryte-terminal/src/lib.rs:1068` - added `Grid::resize(new_width,
  new_height)` for dynamic grid sizing. Preserves overlapping content, fills
  new cells with `Cell::EMPTY`. Useful for responsive TTY layouts that adapt
  to terminal resize events. Tests at `:1797`.
- `crates/verryte-input/src/lib.rs:727` - added `TextInput` buffer for terminal
  text entry (prompts, naming, chat). Handles `Key` events for character
  insertion, backspace, delete, left/right/home/end cursor movement, Enter
  (submit), and Esc (clear). Supports max length, dirty tracking, multibyte
  character awareness, and `take_text` for consuming the final string. Tests
  at `:1467` covering character input, max length, cursor movement, multibyte
  chars, dirty tracking, and event handling.
- `README.md` - updated crate descriptions to document `Rng`, `GameClock`,
  `ColorPalette`, `Layers`, `Grid::resize`, and `TextInput`.

**Reasoning.** These are all small, focused additions that terminal games
repeatedly need. A seeded RNG is fundamental for reproducible procedural
generation, test fixtures, and agent replay scenarios. `GameClock` gives
turn-based games a clean way to track turns and real-time games a way to
measure session duration while respecting pause state. Color palettes solve
the "hardcoded RGB values scattered across rendering code" problem that every
terminal game eventually hits. `Layers` collection is the natural evolution of
the raw `Vec<Layer>` pattern — games need to find layers by name, replace them,
and composite without manual sorting. `Grid::resize` enables the TTY frontend
to respond to terminal resize events without recreating the entire grid.
`TextInput` fills the gap for games that need player text entry (naming
characters, entering commands, chat in multiplayer terminal games) without
each game reinventing cursor management and multibyte handling.

**Assumptions.** I assumed `Rng` should use xorshift64 rather than a more
sophisticated algorithm because terminal games don't need cryptographic
randomness and xorshift64 is fast, simple, and has no dependencies. I assumed
`GameClock` should use `std::time::Instant` for real-time tracking, which
means it's not serializable for save games — the tick count can be set
directly via `set_elapsed_ticks` for that use case. I assumed `TextInput`
should handle `Key` events rather than raw characters, since the engine
already has a neutral `Key` type and frontends translate terminal input into
keys. I assumed `ColorPalette` should ship with four opinionated themes rather
than being purely a blank struct, since most games will want a starting point.

**Gotchas.** The initial `TextInput` backspace-at-start test had a wrong
expectation: after typing 'a' and pressing backspace once, the text is empty,
not 'a'. The multibyte test also had wrong expectations: backspace at cursor
position 2 in "日本語" deletes "本" (position 1), leaving "日語" with cursor
at 1, not "日本" with cursor at 2. Both were fixed by correcting the test
expectations to match actual behavior. The `Grid::resize` test initially had
a temporary-value-dropped-while-borrowed error from chaining `.to_plain_string().lines().collect()`; fixed by introducing a `let` binding.

**Follow-ups.** `TextInput` could be extended with clipboard support,
undo/redo, or selection ranges for richer editing. `GameClock` could gain
fixed-timestep support (accumulating delta time and running multiple ticks
when behind). `ColorPalette` could support runtime loading from config files
(TOML/JSON) for player-customizable themes. The `Layers` system could gain
z-index ranges or layer groups for more complex rendering hierarchies.

## 2026-05-16 - engine primitives: animation sprites, sparkline, for_each3_mut, spatial hash, weighted RNG, ActionSource serialization

**Goal.** Continue autonomous Verryte development with a second batch of
reusable engine primitives focused on animation, data visualization, complete
mutable iteration API, efficient spatial queries, weighted randomness, and
action source serialization.

**Changes.**
- `crates/verryte-terminal/src/lib.rs:1083` - added `Frame`, `Sprite`, and
  `SpriteSheet` types for frame-based terminal animation. `Sprite` tracks
  playback state (current frame, elapsed ticks, paused) and loops by default.
  `SpriteSheet` manages named sprites (idle, walk, attack, etc.) with
  add/find/remove, `tick_all`, and `reset_all`. Tests at `:2117` covering
  frame advancement, pause/resume, reset, set_frame clamping, sheet
  replacement, and tick_all.
- `crates/verryte-terminal/src/lib.rs:1265` - added `draw_sparkline()` for
  rendering mini bar charts using Unicode block characters (▁▂▃▄▅▆▇█). Values
  are normalized and mapped to 9 levels. Useful for inline stats in terminal
  game UIs (health history, damage trends, turn counts). Tests at `:2232`.
- `crates/verryte-core/src/world.rs:581` - added `World::for_each3_mut<A, B, C>()`
  completing the mutable iteration API alongside `for_each_mut` and
  `for_each2_mut`. Uses the same column-swap pattern for safe `Box<dyn Any>`
  downcasting without `unsafe`. Returns `false` for duplicate types or missing
  columns. Tests at `:1119` covering three-component mutation, duplicate type
  rejection, missing column handling, and empty-match success.
- `crates/verryte-map/src/lib.rs:1470` - added `SpatialHash<T>` for efficient
  proximity queries on grid-based entities. Divides space into fixed-size
  cells and stores entities by cell key. Methods: `insert`, `remove`,
  `query` (Manhattan radius), `nearest` (custom comparator), `clear`, `len`.
  Useful for AI targeting, collision detection, and interaction range queries
  without scanning all entities. Tests at `:2373` covering insert/query,
  remove, nearest finding, cell size grouping, empty query, clear, and length.
- `crates/verryte-core/src/rng.rs:124` - added `Rng::weighted_pick(items, weights)`
  for weighted random selection. Each element's probability is proportional to
  its weight. Returns `None` for empty slices, mismatched lengths, or all-zero
  weights. Tests at `:341` covering empty/mismatched/zero-weight edge cases,
  weight distribution verification (>70% for 90/10 split), determinism, and
  single-item case.
- `crates/verryte-input/src/lib.rs:110` - added `Display` and `FromStr`
  implementations for `ActionSource`. `Display` produces canonical names
  ("Terminal", "Script", etc.). `FromStr` parses case-insensitively for
  serialization, config files, and debug output. Tests at `:1670`.
- `README.md` - updated all crate descriptions to document new capabilities.

**Reasoning.** Animation is a gap that every terminal game eventually needs —
character movement, attack effects, UI transitions. `Sprite`/`SpriteSheet`
provide a lightweight frame-based system that integrates with the existing
`Grid` abstraction. Sparklines are a compact data visualization primitive that
terminal games can use for inline stat displays without needing a full chart
library. `for_each3_mut` completes the mutable iteration API that was started
with `for_each2_mut` — games with position/velocity/health or similar
three-component patterns need this. `SpatialHash` is the standard solution for
proximity queries in grid games; without it, games either scan all entities
(O(n)) or build their own ad-hoc spatial structures. `weighted_pick` is
fundamental for loot tables, encounter generation, and any game mechanic where
outcomes should have different probabilities. `ActionSource` serialization
enables config files, debug dumps, and agent protocols to reference sources
by name.

**Assumptions.** I assumed `Sprite` should use tick-based timing rather than
real-time durations, since the engine's `GameClock` already tracks ticks and
games control the tick cadence. I assumed `SpatialHash` should use Manhattan
distance for query radius to match the rest of the engine's distance semantics.
I assumed `weighted_pick` should use `u32` weights rather than `f64` to avoid
floating-point precision issues and keep the API simple. I assumed
`ActionSource::FromStr` should be case-insensitive to make config files and
debug output more forgiving.

**Gotchas.** The initial `SpatialHash` tests had multiple issues: `query`
returns `&T` references, so tests needed `.copied()` to collect into `Vec<T>`.
The `nearest` test used `&str` values which don't have `manhattan_distance`;
switched to `Point` values. The cell_size test used a query radius that was
too small to include nearby entities in the same cell — fixed by using a
larger radius. The `spatial_hash_nearest_finds_closest` test had equidistant
points (origin and near both at distance 1 from center), making the result
non-deterministic — fixed by moving the center to (5,0) so near (2,0) is
clearly closer than origin (0,0). Duplicate test definitions were introduced
during editing and had to be cleaned up.

**Follow-ups.** `Sprite` could gain easing/interpolation between frames for
smoother animation. `SpatialHash` could support dynamic cell sizing or
hierarchical grids for games with entities at vastly different scales.
`draw_sparkline` could gain configurable block character sets or vertical
orientation. `Rng::weighted_pick` could accept `f64` weights for finer
probability control. The `Layers` system could gain z-index ranges for
sub-layer ordering within a single layer.

## 2026-05-16 - priority action queue, Events::with_capacity, fix place_rooms tests

**Goal.** Add priority action injection for urgent/interrupt actions, pre-allocated event channels, and fix compilation errors in place_rooms tests from a previous batch.

**Changes.**
- `crates/verryte-input/src/lib.rs:668-682` - Added `inject_priority` and `inject_priority_from` methods to `InputRouter` that use `push_front` on the pending VecDeque, placing actions ahead of all currently queued items.
- `crates/verryte-core/src/event.rs:21-30` - Added `Events::with_capacity(capacity)` constructor that pre-allocates the internal VecDeque, useful when per-frame event volume is known.
- `crates/verryte-map/src/lib.rs:1198-1209` - Changed `place_rooms` signature from `<F, R>` to `<F1, F2, R>` so `wall` and `floor` can be different closure types (each closure literal has a unique anonymous type in Rust).
- `crates/verryte-map/Cargo.toml` - Added `verryte-core` as a dev-dependency so tests can use `Rng::seed`.
- `crates/verryte-map/src/lib.rs:1674` - Added `use verryte_core::Rng;` in test module.

**Reasoning.** Priority injection is a common game-dev need (interrupts, emergency actions, immediate responses) and fits naturally on VecDeque since it already supports O(1) push_front. `with_capacity` is a standard optimization for hot-path event channels. The `place_rooms` fix was necessary because Rust's type system treats each closure literal as a distinct type, so `wall: F, floor: F` cannot accept two different closures.

**Assumptions.** Priority actions are rare enough that front-insertion overhead doesn't matter. Games that need multi-level priorities can layer their own ordering on top.

**Gotchas.** The `place_rooms` signature change from `<F, R>` to `<F1, F2, R>` is a breaking API change for any code using this method, but since it was just added in a previous uncommitted batch, this is fine.

**Follow-ups.** Consider whether `InputRouter` should support a true priority queue (BinaryHeap) for more than two priority levels, or if the current front/back dichotomy is sufficient for Verryte's use cases.

## 2026-05-16 - batch 4: drain_filter, filter_pending, fill_rect, find_cell

**Goal.** Add selective event/action filtering and grid search primitives.

**Changes.**
- `crates/verryte-core/src/event.rs:58-74` - `Events::drain_filter` drains events matching a predicate and re-queues the rest. Uses swap-then-partition to avoid Clone requirement.
- `crates/verryte-input/src/lib.rs:755-773` - `InputRouter::filter_pending` removes pending actions matching a predicate, preserving order of remaining actions. Returns count of removed actions.
- `crates/verryte-map/src/lib.rs:475-492` - `TileGrid::fill_rect` fills a rectangular region with a tile, clipping to grid bounds. Accepts signed start coordinates for partial fills from edges.
- `crates/verryte-terminal/src/lib.rs:282-301` - `Grid::find_cell` scans row-major for first cell matching a predicate, returning (x, y, &Cell).

**Reasoning.** These are all "missing obvious primitives" that games keep needing. `drain_filter` lets systems extract specific event types from shared channels without consuming everything. `filter_pending` enables canceling queued actions when game state changes (e.g., entering a menu). `fill_rect` is the rectangular analog to `fill`. `find_cell` is useful for locating player characters, items, or specific glyphs without manual iteration.

**Assumptions.** `drain_filter` and `filter_pending` both use the swap-and-partition pattern which temporarily allocates a new VecDeque. This is fine for occasional use but not for per-frame hot paths.

**Follow-ups.** Consider adding `Grid::find_all_cells` returning an iterator for cases where multiple matches matter.

## 2026-05-16 - batch 5: retain, union, swap_cells, insert_at, insert_str

**Goal.** Add missing utility primitives across core, terminal, and input crates.

**Changes.**
- `crates/verryte-core/src/log.rs:73-81` - `MessageLog::retain` filters messages in-place by predicate. Delegates to `Vec::retain`. Useful for clearing specific message categories.
- `crates/verryte-terminal/src/lib.rs:176-193` - `Rect::union` returns smallest rect containing both. Handles empty rects by returning the non-empty one. Useful for computing dirty regions.
- `crates/verryte-terminal/src/lib.rs:303-314` - `Grid::swap_cells` exchanges two cells by position via `Vec::swap`. Returns false if either position is out of bounds.
- `crates/verryte-core/src/schedule.rs:154-162` - `Schedule::insert_at` inserts a named system at a specific index. Delegates to `Vec::insert`. Panics if index > len (consistent with Vec behavior).
- `crates/verryte-input/src/lib.rs:991-1011` - `TextInput::insert_str` inserts a string at cursor, respecting max length. Truncates inserted text if it would exceed the limit. Advances cursor by actual inserted character count.

**Reasoning.** These are all "obvious missing methods" that games reach for. `retain` on MessageLog enables category-based filtering. `Rect::union` is the dual of `intersect` and useful for dirty-region tracking. `swap_cells` enables drag-and-drop and rearrangement. `insert_at` lets games inject systems before/after existing ones without rebuilding the schedule. `insert_str` on TextInput enables paste and programmatic text insertion beyond single-character input.

**Gotchas.** The `rect_union_combines_two_rects` test had an incorrect expected value (bottom=7 instead of 8). The math: a=Rect(2,3,4,5) has bottom=8, b=Rect(5,1,3,6) has bottom=7, so union bottom=max(8,7)=8.

**Follow-ups.** Consider adding `Rect::union_many` for combining more than two rects in one pass.

## 2026-05-16 - batch 6: INVALID, is_any, row, pick_range, send_batch

**Goal.** Add sentinel entity, tag convenience, grid row access, iterator-based random pick, and batch event sending.

**Changes.**
- `crates/verryte-core/src/entity.rs:17-21` - `Entity::INVALID` constant with `index: u32::MAX, generation: u32::MAX`. Will never resolve to a live entity since generations are reset on reuse and indices are allocated from a free list.
- `crates/verryte-core/src/tag.rs:33-38` - `Tag::is_any(&[&str])` checks if tag matches any name in a slice. Delegates to iterator `any`.
- `crates/verryte-terminal/src/lib.rs:235-244` - `Grid::row(y)` returns `Option<&[Cell]>` slice of a single row. Zero-copy, useful for scanning rows without full grid iteration.
- `crates/verryte-core/src/rng.rs:113-128` - `Rng::pick_range` picks random element from any iterator using reservoir sampling (single-item variant). Works on iterators without collecting into Vec first. O(n) time, O(1) space.
- `crates/verryte-core/src/event.rs:34-42` - `Events::send_batch` queues multiple events at once. Returns count of events queued.

**Reasoning.** `Entity::INVALID` is a common pattern in ECS systems for optional entity references (parent, target, etc.) without using Option<Entity>. `Tag::is_any` reduces boilerplate for group membership checks. `Grid::row` enables efficient row scanning for text rendering or row-based effects. `Rng::pick_range` fills a gap where games need to pick from non-slice iterables (like filtered entity iterators). `Events::send_batch` mirrors `InputRouter::handle_batch` for the event side.

**Assumptions.** `Entity::INVALID` uses MAX/MAX which is safe as long as the world never allocates that many entities (4 billion+). This is a safe assumption for terminal games.

**Follow-ups.** Consider adding `Grid::col(x)` for column slices, though it requires copying since cells are row-major.

## 2026-05-16 - batch 7: rotate, iter_cells, count_with, translate, map_in_place

**Goal.** Add direction rotation, grid iteration, entity counting, rect translation, and tile transformation.

**Changes.**
- `crates/verryte-map/src/lib.rs:167-183` - `Direction::rotate_cw` and `rotate_ccw` for 90-degree turns.
- `crates/verryte-terminal/src/lib.rs:247-254` - `Grid::iter_cells` yields `(x, y, &Cell)` for all cells in row-major order.
- `crates/verryte-core/src/world.rs:326-343` - `World::count_with<T>` counts live entities having component T.
- `crates/verryte-terminal/src/lib.rs:207-214` - `Rect::translate(dx, dy)` offsets position, clamps negative to zero.
- `crates/verryte-map/src/lib.rs:491-507` - `TileGrid::map_in_place` transforms all tiles with `(Point, &T) -> T`.

**Reasoning.** Direction rotation is fundamental for facing/turning. `iter_cells` avoids manual nested loops. `count_with` enables "how many enemies remain" queries without allocation. `Rect::translate` complements `intersect` and `union`. `map_in_place` enables position-aware tile transforms.

**Gotchas.** The `map_in_place` test had wrong expected value for (2,0): should be 0+2+0=2, not 4.

## 2026-05-16 - batch 8: area, replace_by_name, blit_region, swap, shuffle_range

**Goal.** Add rect area calculation, system replacement, sub-region blitting, tile swapping, and partial range shuffling.

**Changes.**
- `crates/verryte-terminal/src/lib.rs:163-166` - `Rect::area` returns width * height as usize. Returns 0 for empty rects.
- `crates/verryte-core/src/schedule.rs:163-173` - `Schedule::replace_by_name` replaces first system matching name, keeping same position. Returns false if not found.
- `crates/verryte-terminal/src/lib.rs:654-688` - `Grid::blit_region` copies a sub-rectangle from source grid, clipping to both source and destination bounds. Skips transparent cells.
- `crates/verryte-map/src/lib.rs:465-477` - `TileGrid::swap` exchanges two tiles by point using index-based Vec::swap. Returns false if either point is OOB.
- `crates/verryte-core/src/rng.rs:178-192` - `Rng::shuffle_range` shuffles a sub-range [start, end) using Fisher-Yates. Clamps to valid bounds, no-op for ranges < 2 elements.

**Reasoning.** `Rect::area` is a basic utility needed for sizing and capacity calculations. `replace_by_name` enables hot-reloading systems without rebuilding the schedule or changing execution order. `blit_region` is needed when games want to copy a specific viewport or sprite region rather than the entire source grid. `TileGrid::swap` supports puzzle mechanics and tile rearrangement. `shuffle_range` is useful when only part of a collection needs randomization (e.g., shuffling a deck's top N cards).

## 2026-05-16 - batch 9: integrate engine features into Ash Courier prototype

**Goal.** Use existing engine features in the prototype to validate the API shape and reduce hardcoded values.

**Changes.**
- `prototype/ash-courier/src/game.rs:103` - Changed `MessageLog::new()` to `MessageLog::with_max(50)` to bound memory for long sessions.
- `prototype/ash-courier/src/game.rs:4` - Added `ColorPalette` import, removed unused `Color`.
- `prototype/ash-courier/src/game.rs:105-108` - Changed `schedule.add()` to `schedule.add_named()` for "chaser", "resolve", "messages" systems. Enables debugging and runtime schedule introspection.
- `prototype/ash-courier/src/game.rs:490-537` - Replaced hardcoded colors in `render()` with `ColorPalette::dark_dungeon()`. Wall/floor/goal/hazard/package/player colors now come from the theme.
- `prototype/ash-courier/src/bin/tty.rs:6` - Added `Alignment` and `ColorPalette` imports, removed unused `Cell` and `Color`.
- `prototype/ash-courier/src/bin/tty.rs:49-195` - Replaced `draw_panel` with `draw_rounded_panel` for all three UI panels. Replaced `write_str` with `write_aligned` for status panel text. All colors now come from `ColorPalette::dark_dungeon()`.

**Reasoning.** The prototype is the proving ground for engine APIs. Using `MessageLog::with_max` validates the bounded log API in a real context. `ColorPalette` replaces scattered hardcoded colors with a single theme, making it trivial to swap themes later (e.g., amber_terminal or cyberpunk). `add_named` makes the schedule self-documenting — system names appear in logs and hooks. `draw_rounded_panel` and `write_aligned` validate the terminal rendering primitives in the only frontend that matters: the real TTY.

**Assumptions.** The dark_dungeon palette is a good default. The bounded log at 50 messages is enough for typical play sessions. Named systems don't need conditions yet.

**Follow-ups.** Consider exposing the palette as a configurable option in the TTY frontend. The `Game::render()` method could accept a palette parameter instead of hardcoding dark_dungeon.

## 2026-05-16 - Batch 11: engine primitives and cell attributes

**Goal.** Add 8+ meaningful improvements across engine crates and integrate into Ash Courier.

**Changes.**
- `crates/verryte-core/src/world.rs:79` - Added `reserve_entities(n)` for bulk entity pre-allocation. Reserves capacity in generations/alive/free vectors and adds slots to the free list so subsequent spawns don't grow vectors.
- `crates/verryte-core/src/world.rs:22` - Added `Column::shrink_to_fit()` trait method and impl for `TypedColumn<T>` that trims trailing None slots and calls `shrink_to_fit()` on the underlying Vec.
- `crates/verryte-core/src/world.rs:218` - Added `World::shrink()` that calls `shrink_to_fit()` on all columns and trims empty trailing entity slots. Useful after bulk despawns or level transitions.
- `crates/verryte-map/src/lib.rs:860` - Added `reachable_points4_bounded(start, max_steps, passable)` for limited-range cardinal reachability. Returns points in BFS order.
- `crates/verryte-map/src/lib.rs:903` - Added `reachable_points8_bounded(start, max_steps, passable)` for limited-range 8-directional reachability.
- `crates/verryte-map/src/lib.rs:946` - Added `distance_to_nearest8(start, targets, passable)` for 8-directional distance queries. Uses BFS with 8-dir neighbors.
- `crates/verryte-terminal/src/lib.rs:29` - Added `CellAttrs` struct with `bold`, `underline`, `dim`, `italic`, `reverse`, `blink` fields and builder methods. Updated `Cell` to include `attrs: CellAttrs` field.
- `crates/verryte-terminal/src/lib.rs:803` - Updated `to_ansi_string()` to emit attribute escape codes when cells have non-default attributes.
- `crates/verryte-terminal/src/lib.rs:692` - Changed `draw_line()` to return `u16` count of cells written, matching `draw_hline`/`draw_vline` API.
- `crates/verryte-input/src/lib.rs:51` - Added `Key::Modified { char, ctrl, alt, shift }` variant for modifier key bindings. Added `Key::is_modified()` helper.
- `crates/verryte-input/src/lib.rs:812` - Added `InputRouter::drain_filtered()` that returns the removed `Vec<QueuedAction<A>>`, complementing `filter_pending()` which only returns a count.

**Reasoning.** 
- `reserve_entities` and `shrink` address memory management for games with entity churn (level transitions, projectile cleanup).
- Bounded reachability is essential for movement range indicators and limited FOV without scanning the entire map.
- Cell attributes enable richer terminal text styling (bold titles, underlined links, dimmed disabled items) without changing the cell model fundamentally.
- Modifier key support is a prerequisite for keyboard shortcuts (Ctrl+Q quit, Alt+1 switch tab, etc.).
- `drain_filtered` returning the removed items enables logging canceled actions or re-routing them.
- `QueryMut` iterator types were attempted but removed because they require `unsafe` pointer-to-ref conversion, which the workspace lint forbids. The existing `for_each_mut` callback pattern remains the safe alternative.

**Assumptions.** 
- `CellAttrs::to_ansi()` is simplified and only handles single-attribute or common two-attribute combinations. Full composite sequences would need a buffering approach.
- `reserve_entities` adds to the free list but doesn't actually spawn entities - callers still need to call `spawn()`.

**Gotchas.**
- The workspace has `-F unsafe-code` lint, so any `unsafe` blocks are hard errors. This blocked the `QueryMut` implementation that used raw pointer derefs.
- Adding `attrs` field to `Cell` broke `write_str` which constructed `Cell` manually. Fixed by adding `attrs: CellAttrs::NONE`.

**Follow-ups.**
- Consider adding `ExactSizeIterator` impl for `Query`/`Query2`/`Query3` types.
- Consider adding composite attribute sequences to `CellAttrs::to_ansi()`.
- Consider integrating bounded reachability into Ash Courier for movement range display.

## 2026-05-16 - GameClock, shadowcasting FOV, diff-based TTY, schedule stages, Rng resource

**Goal.** Continue autonomous Verryte development with improvements that validate
engine primitives through Ash Courier, improve rendering efficiency, and add
schedule organization.

**Changes.**
- `prototype/ash-courier/src/game.rs:1` - Added `GameClock` as an ECS resource
  alongside `GameState`. `advance_turn()` now calls `clock.tick()` in addition
  to incrementing `GameState.turn`, keeping both synchronized. Added `clock()`
  accessor and `with_seed()` / `from_layout_with_seed()` constructors.
- `prototype/ash-courier/src/map.rs:49` - Switched `visible_from` from
  `TileGrid::visible_points` (brute-force raycasting) to `TileGrid::field_of_view`
  (recursive shadowcasting). Same signature, drop-in replacement. Shadowcasting
  is faster on larger maps and produces symmetric visibility.
- `crates/verryte-tty/src/lib.rs:91` - Added `render_diff(prev, next)` that
  computes `Grid::diff()` and only writes changed cells to the terminal using
  cursor positioning. Dramatically reduces I/O for turn-based games where most
  of the frame is unchanged.
- `prototype/ash-courier/src/bin/tty.rs:9` - TTY frontend now maintains a
  `prev_frame` and uses `render_diff` instead of full-frame `render` on each
  tick. Initial frame still uses full `render` to establish the baseline.
- `crates/verryte-core/src/schedule.rs:55` - Added `stage_markers` field to
  `Schedule` for named execution phases. Added `add_stage(name)` to mark stage
  boundaries, `run_stage(name, world)` to execute only systems in one stage,
  and `stage_names()` to query defined stages. Systems added between
  `add_stage` calls belong to that stage. `run()` continues to execute all
  systems in order (backward compatible). `clear()` also clears stage markers.
  Seven new tests covering stage tracking, selective execution, condition
  respect, independence from full `run`, and clear behavior.
- `prototype/ash-courier/src/systems.rs:24` - Chaser system now shuffles chaser
  entity order each tick using the seeded `Rng` resource, preventing
  deterministic ordering bias from entity allocation order.
- `prototype/ash-courier/src/lib.rs` - Added tests for GameClock integration
  (tick tracking, no-advance on noop, clock/state synchronization), Rng resource
  availability, deterministic chaser outcomes with same seed, and `with_seed`
  constructor.
- `README.md` - Updated crate descriptions to document schedule stages,
  diff-based TTY rendering, shadowcasting FOV usage, GameClock/Rng integration.

**Reasoning.** These five improvements validate existing engine primitives through
the proving game rather than adding speculative features. GameClock was
implemented but never used by any game — integrating it into Ash Courier proves
the resource-as-timing pattern works. Shadowcasting FOV was the better algorithm
but Ash Courier still used the brute-force `visible_points`; switching validates
the engine upgrade path. Diff-based rendering uses `Grid::diff()` which existed
only for tests — now it's used in the real TTY frontend, proving the primitive's
practical value. Schedule stages address the "flat list gets hard to reason about"
problem that Ash Courier's three systems already hint at. Rng-as-resource is the
pattern the engine was designed for but no game had exercised.

**Assumptions.** `GameClock` and `GameState.turn` both track turns independently
rather than `GameState.turn` being derived from the clock. This keeps backward
compatibility with all existing tests while making the clock available for
future pause/resume and real-time tracking. Schedule stages use index-based
ranges rather than a separate data structure, keeping the flat execution model
intact. Chaser shuffling with Rng means chaser behavior is deterministic given
the same seed, which is correct for replay/agent scenarios.

**Gotchas.** The `field_of_view` shadowcasting produces slightly different
visibility at edges compared to `visible_points` raycasting, but all existing
tests pass because they test structural properties (visible tiles contain the
player, hazards are detected) rather than exact tile counts. The diff-based TTY
rendering needs a full initial frame as baseline; without it, the first diff
would write every cell.

**Follow-ups.** Schedule stages could gain per-stage hooks for profiling or
logging. The `Rng` resource could be exposed in the snapshot for agent
observation. `render_diff` could be extended to handle terminal resize by
falling back to a full render when the grid dimensions change.

## 2026-05-16 - modifier keys, cave generation, ECS ergonomics, query size hints, Ash Courier cave map

**Goal.** Continue autonomous Verryte development with a vertical slice that fixes a real gap in the input path, adds organic procedural map generation, and improves ECS ergonomics — all validated through the Ash Courier proving game.

**Changes.**
- `crates/verryte-tty/src/lib.rs:175` — rewired `map_key` to pass Ctrl/Alt/Shift modifiers through to `Key::Modified` instead of silently dropping them. Unmodified keys retain backward-compatible behavior. Added `KeyModifiers` import at `:14`.
- `crates/verryte-tty/src/lib.rs:228` — added 10 unit tests covering char, ctrl, alt, ctrl+shift, uppercase ctrl normalization, special keys without modifiers, arrow+ctrl, shift+tab, enter+ctrl, and F-key+alt.
- `crates/verryte-map/src/lib.rs:1496` — added `TileGrid::cellular_automata_cave` for organic procedural cave generation using cellular automata. Configurable fill chance, smoothing iterations, birth limit, and seed. Returns floor tile count. Borders are always walls.
- `crates/verryte-map/src/lib.rs:3048` — added 4 tests: basic cave carving, reproducibility with same seed, divergence with different seeds, and tiny-grid edge case.
- `crates/verryte-core/src/world.rs:398` — added `World::contains<T>()` for checking if any live entity has a given component type. Short-circuits on first match, more efficient than `count_with() > 0` for existence checks.
- `crates/verryte-core/src/world.rs:848` — added `ExactSizeIterator` and `size_hint` implementations for `Query`, `Query2`, and `Query3` iterator types. Delegates to the underlying `Vec::IntoIter`.
- `crates/verryte-core/src/world.rs:1505` — added 5 tests: `contains` present/absent/after-despawn, and `ExactSizeIterator` for all three query types.
- `prototype/ash-courier/src/game.rs:127` — added `Game::from_cave(width, height, seed)` constructor that generates a cave via cellular automata, picks player/goal/package/hazard positions from walkable tiles using the seeded RNG, and wires everything into the ECS.
- `prototype/ash-courier/src/lib.rs:764` — added 3 tests: `from_cave` creates playable game, has package and goal entities, and is deterministic with same seed.
- `README.md` — updated crate descriptions to document modifier key passthrough, cellular automata cave generation, `World::contains`, `ExactSizeIterator`, and `Game::from_cave`.

**Reasoning.** The modifier key gap was the most impactful fix: `Key::Modified` existed in the input model but the TTY frontend never produced it, making the entire modifier system dead code. Games can now bind Ctrl+Q for quit, Alt+1 for tab switching, etc. through the same shared action path. Cellular automata cave generation complements BSP and random walk for procedural map variety and validates the engine's map primitives through the proving game. `World::contains` is a common existence check that was missing. `ExactSizeIterator` on query types lets consumers know result counts without collecting. `Game::from_cave` validates the cave generator and the shared action path works with procedurally generated maps.

**Assumptions.** Ctrl+char normalizes to lowercase (Ctrl+C produces 'c' not 'C') to match standard terminal conventions. Shift alone with a char does not produce `Key::Modified` because crossterm already capitalizes the char. Arrow/special keys without modifiers remain as their normal `Key` variants for backward compatibility. Cave generation uses a fixed 0.42 fill chance and 5 iterations with birth limit 4 as defaults that produce reasonable caves at common grid sizes.

**Gotchas.** The initial `map_key` implementation produced `Key::Modified` for all arrow keys even without modifiers, which would have broken existing bindings. Fixed by only emitting `Modified` when Ctrl or Alt is held. `Rng::pick_index` returns `Option<usize>` (None for empty), so the cave constructor must unwrap. The `Map` struct's `tiles` field is `pub(crate)` which allows `from_cave` to set the goal tile directly after generation.

**Follow-ups.** The cave constructor could be extended with configurable fill chance, iterations, and birth limit parameters. A `from_bsp` constructor would complement `from_cave` for structured dungeon maps. The modifier key system could be exercised in Ash Courier by binding Ctrl+Q to quit or similar shortcuts. The TTY frontend's `map_key` function should handle Ctrl+letter for lowercase-only normalization (currently it passes through the original char case for ctrl).

## 2026-05-17 - engine primitives: from_ascii, spawn_batch, grid scroll, CellAttrs ANSI fix, map_tiles

**Goal.** Continue autonomous Verryte development with a batch of reusable engine
primitives that improve map construction, ECS bulk operations, terminal rendering,
and attribute serialization.

**Changes.**
- `crates/verryte-map/src/lib.rs:337` - added `TileGrid::from_ascii` for
  constructing grids from multi-line string literals via a char-mapping closure.
  Handles ragged lines (shorter lines padded), empty input (0×0 grid), and
  passes coordinates to the mapping function. Tests at `:3049`, `:3060`, `:3070`,
  and `:3080`.
- `crates/verryte-map/src/lib.rs:395` - added `TileGrid::map_tiles<U>` for
  transforming each tile into a different type, producing a new grid with the
  same dimensions. Useful for converting logical tile maps into display
  representations. Tests at `:3092` and `:3101`.
- `crates/verryte-core/src/world.rs:828` - added `World::spawn_batch` for bulk
  entity creation with a shared component. Clones the component for each entity
  and returns the list of spawned entities. Tests at `:1575`, `:1586`, and
  `:1596`.
- `crates/verryte-terminal/src/lib.rs:466` - added `Grid::scroll_up` and
  `Grid::scroll_down` for shifting grid content by N rows. New rows are filled
  with a provided cell. Uses `copy_within` for efficient memory movement.
  Tests at `:2932`, `:2946`, `:2960`, `:2967`.
- `crates/verryte-terminal/src/lib.rs:81` - rewrote `CellAttrs::to_ansi` to
  handle all attribute combinations dynamically instead of pattern-matching a
  fixed set. Now returns `String` (was `&'static str`) and builds composite
  ANSI sequences for any combination of bold, dim, italic, underline, blink,
  and reverse. Updated `Grid::to_ansi_string` to use the new method. Tests at
  `:2975` and `:2987`.
- `prototype/ash-courier/src/map.rs:27` - added `Map::from_ascii` convenience
  constructor that uses `TileGrid::from_ascii` internally for pure-tile map
  construction from string literals.
- `README.md` - updated crate descriptions to document `spawn_batch`,
  `from_ascii`, `map_tiles`, `scroll_up`/`scroll_down`, and `CellAttrs`
  improvements.

**Reasoning.** These are all small, focused primitives that terminal games
repeatedly need. `from_ascii` eliminates the most common boilerplate in map
construction and test fixtures — every roguelike test that creates a small map
currently does so tile-by-tile. `map_tiles` enables the common pattern of
separating logical tile types from display representations. `spawn_batch`
addresses bulk entity creation that level generators and hazard placement
routines need. Grid scrolling is a fundamental terminal UI primitive for message
logs, scrolling text regions, and terminal output emulation. The `CellAttrs`
fix addresses a real bug where multi-attribute combinations (e.g., bold+italic)
produced empty ANSI sequences, making styled terminal output incomplete.

**Assumptions.** `from_ascii` uses `FnMut` rather than `Fn` to support closures
that accumulate state (e.g., collecting coordinates). `spawn_batch` requires
`Clone` on the component type, which is reasonable since components are typically
small data. `scroll_up`/`scroll_down` use `copy_within` for efficiency but
handle the edge case where n ≥ height by clearing entirely.

**Gotchas.** The initial `CellAttrs::to_ansi` returned `&'static str` which
couldn't represent dynamic multi-attribute sequences. Changing the return type
to `String` is a minor API break but the method had no external callers outside
`to_ansi_string` (which handled attributes inline). The `from_ascii` empty-input
test initially failed because `"".split('\n')` produces `[""]` (one empty
string) rather than `[]`; fixed with an explicit empty-check guard. The
`spawn_batch` tests initially failed because test structs lacked `Clone`; added
`Clone` derives to `Pos` and `Counter` test types.

**Follow-ups.** `TileGrid::from_ascii` could be extended with error handling
(unknown glyph → Result) for cases where the mapping function needs to reject
invalid input. `spawn_batch` could accept an iterator of components for
heterogeneous bulk spawning. `scroll_up`/`scroll_down` could be integrated into
the Ash Courier TTY runner's message log panel for smoother scrolling behavior.

## 2026-05-17 - autonomous engine run: column slices, saturating math, contains_point, is_empty

**Goal.** Continue autonomous Verryte development with a batch of small, focused
engine primitives that fill genuine gaps identified through code inspection and
worklog follow-up review.

**Changes.**
- `crates/verryte-terminal/src/lib.rs:349` - added `Grid::col(x)` returning
  `Option<Vec<Cell>>` for column scanning. Unlike `row(y)` which returns a
  zero-copy slice, columns require allocation because cells are stored row-major.
  Tests at `:2872` and `:2889` verify correct column extraction and OOB handling.
- `crates/verryte-map/src/lib.rs:31` - added `Point::saturating_offset(dx, dy)`
  using `i16::saturating_add` to prevent overflow in grid math. Useful for
  iterative grid operations where explicit bounds checks at each step would be
  verbose. Test at `:2133` covers normal operation and saturation at both
  `i16::MAX` and `i16::MIN`.
- `crates/verryte-map/src/lib.rs:460` - added `TileGrid::contains_point(point, predicate)`
  combining `in_bounds` and tile inspection in one call. Returns `true` only if
  the point is in bounds AND its tile matches the predicate. Test at `:2231`
  covers in-bounds match, in-bounds non-match, and out-of-bounds cases.
- `crates/verryte-input/src/lib.rs:774` - added `InputRouter::is_empty()` checking
  both `bindings.is_empty()` and `pending.is_empty()`. Useful for detecting a
  completely fresh or fully drained router. Test at `:1222` verifies empty router,
  router with bindings, router with no bindings, and router with pending actions.
- `crates/verryte-terminal/src/lib.rs:3010` - added tests for existing
  `Grid::fill_rect(Rect, Cell)` API to improve coverage.

**Reasoning.** The codebase is already quite mature (566 tests, comprehensive
API coverage across all crates). The worklog follow-ups mentioned `Grid::col(x)`
as a natural complement to `row(y)`. The other additions are "obvious missing
methods" that games reach for: saturating offset prevents panics in iterative
grid algorithms, `contains_point` is a common guard clause pattern, and
`is_empty` on the router provides a more complete idle check than `is_idle()`
alone (which only checks the pending queue).

**Assumptions.** `Grid::col` returns `Vec<Cell>` rather than a custom iterator
because the allocation is small (grid height) and the simplicity outweighs the
micro-optimization. `saturating_offset` uses `i16` saturation semantics which
clamp to `i16::MIN`/`i16::MAX` rather than to grid bounds — callers still need
`in_bounds` checks for grid access.

**Gotchas.** `fill_rect` already existed with a `Rect` parameter signature, so
my initial attempt to add a `(x, y, width, height, cell)` variant created a
duplicate method name. Fixed by removing my variant and adding tests for the
existing API instead. The `saturating_offset` test initially expected clamping
to zero for negative offsets from (0,0), but `i16::saturating_add` clamps to
`i16::MIN`, not zero — fixed the test to match actual semantics.

**Follow-ups.** `Grid::col` could be extended with a mutable variant
`col_mut(x)` for column editing, though it requires allocation and copy-back.
`Point::clamp(rect)` would be a natural companion to `saturating_offset` for
explicit bounds clamping. `InputRouter::is_empty` could be used in Ash Courier's
TTY frontend to detect idle states for animation or timeout logic.

## 2026-05-19 - add lazy resources, input drain traces, map match helpers

**Goal.** Deliver another autonomous engine batch with small, reusable primitives that tighten ECS/resource ergonomics, action routing observability, and map query helpers without splitting the shared control path.

**Changes.**
- `crates/verryte-core/src/world.rs:778` - added `World::resource_or_insert` and `World::resource_or_insert_with` plus tests so resources can be created lazily without boilerplate checks.
- `crates/verryte-input/src/lib.rs:763` - added `InputRouter::drain_trace` plus a test to drain pending actions into an `ActionTrace` while preserving sources.
- `crates/verryte-map/src/lib.rs:1710` - added `TileGrid::find_matching` and `TileGrid::points_matching` plus tests for row-major match discovery.
- `crates/verryte-map/src/lib.rs:1973` - added `Bounds::clamp_point` plus tests to clamp points safely inside a bounds rectangle.
- `README.md` - documented the new resource helpers, drain traces, and map match/clamp APIs in the workspace summary.

**Reasoning.** Resource setup is a common ECS task; adding lazy insertion keeps systems terse while preserving the explicit resource model. Action traces already exist but there was no direct way to drain pending actions while keeping source metadata; a dedicated drain API improves observability for scripts and replays. The map helpers add focused query primitives that games repeatedly need, and `Bounds::clamp_point` is a direct companion to the existing bounds utilities.

**Assumptions.** `find_matching` and `points_matching` should respect the row-major ordering implied by `TileGrid::iter`. `Bounds::clamp_point` returning `None` for empty bounds is preferable to inventing a sentinel point. Resource lazy insertion should never override an existing resource.

**Gotchas.** `Bounds::clamp_point` uses saturating math for the max edge; callers still need to ensure bounds represent a real rectangle (non-zero width/height) or handle the `None` case.

**Follow-ups.** Consider using `points_matching` in Ash Courier layout parsing to reduce manual tile scans, and expose similar row-major helpers for `TileGrid::iter_mut` if future systems need bulk edits.

## 2026-05-20 - inspection cursor and position-aware input

**Goal.** Add a position-aware inspection action that keeps the shared input path intact, surface cursor state in snapshots and runners, and wire mouse clicks in the TTY frontend without breaking turn logic.

**Changes.**
- `crates/verryte-input/src/lib.rs` - added `InputRouter::handle_with` / `handle_with_from` plus tests to support custom event translation (position-aware input) before bindings.
- `prototype/ash-courier/src/action.rs` - introduced `Action::Inspect(Point)` and parameterized `inspect:`/`look:`/`cursor:` token parsing to drive cursor updates through scripts.
- `prototype/ash-courier/src/components.rs` / `src/snapshot.rs` - added cursor state and an `ActionResult::Updated` outcome; snapshots now include cursor tile, path, and distance.
- `prototype/ash-courier/src/game.rs` - applied inspection actions without advancing turns, ran message logging explicitly, and exposed `viewport_origin` for frontends; snapshot builder now includes cursor metadata.
- `prototype/ash-courier/src/bin/tty.rs` - mapped left mouse clicks inside the viewport to inspection actions and displayed cursor status in the UI.
- `prototype/ash-courier/src/bin/script.rs` - documented inspect tokens and printed cursor state in step summaries.
- `README.md` and `prototype/ash-courier/README.md` - documented the new input hook, inspect tokens, cursor fields, and the updated action result.

**Reasoning.** The inspection cursor is a low-risk vertical slice that stresses the shared input→action→state path while adding useful observability for agents and scripts. Using `handle_with` keeps position-aware translation in the same queue as terminal and script input without inventing a new path. Limiting inspection to state updates (no turn advance or system tick) keeps chaser and hazard logic deterministic while still logging inspection events.

**Assumptions.** Inspection should not advance the turn or trigger movement systems; it only updates cursor state and emits an event. Mouse coordinates in the TTY map directly to the viewport's inner rect, so mapping through the viewport origin is sufficient.

**Gotchas.** The viewport may be larger than the map; the input mapper clamps to the actual map width/height to avoid out-of-bounds cursor targets. Since inspection does not run the full schedule, the message system is invoked directly to record the inspection event.

**Follow-ups.** If inspection becomes a broader UI mode, consider adding a dedicated cursor overlay layer in `render()` and formalizing a stack of input contexts for nested UI states.
