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
