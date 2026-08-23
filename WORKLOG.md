# Verryte Worklog

## 2026-08-23 - turn-committed floor modifiers

**Goal.** Stop scheduled floor modifiers from consuming duration and applying
periodic damage at render-frame frequency.

**Accomplishments.** `ActiveFloorModifiers` now records the last processed game
turn with backward-compatible serialization. The modifier schedule only runs in
the player phase and only once per turn, so Elemental Storm consumes one RNG
roll/damage pulse and every active modifier loses one duration point per turn
instead of per frame. Modifier selection and console injection reset the marker
for their new set, malformed duration vectors are normalized defensively, and
save/load preserves the committed turn so loading cannot repeat an already
applied storm pulse.

**Verification.** Focused regressions cover repeated schedule calls in one turn,
the next-turn pulse/countdown, older serialized resources, and save/load
idempotence. `cargo fmt --all --check`, `cargo test --workspace`,
`cargo clippy --workspace --all-targets -- -D warnings`, and `git diff --check`
pass.

**Next Steps.** Make Frenzy's temporary stat adjustment explicitly reversible
on expiry or reroll without losing base DEF for zero-defense entities.

## 2026-08-23 - recording and replay history integrity

**Goal.** Preserve replayable action boundaries and outcome metadata across
recording controls, nested replay steps, headless turns, and state replacement.

**Accomplishments.** Action-history records are now appended after their action
completes, so `Load` can replace the world without losing its own structured
record and nested replay actions keep their outcomes instead of having them
overwritten by the `StepReplay` wrapper. Recording start/stop controls no longer
enter the persisted gameplay history, and stopping uses the authoritative
structured action history instead of first writing the router's incompatible
queued-action representation. A replayed headless `EndTurn` whose recorded
outcome is `TurnAdvanced` now runs the same bounded schedule-settling path before
verification while temporarily suppressing recursive replay auto-step. Replay
loading also filters recording controls from legacy traces created before this
fix.

**Verification.** Focused regressions reproduced both failures before the fix.
The complete 83-test integration suite passes, including save/load history,
recording boundaries, replay wrapper metadata, and headless end-turn parity. The
same full workspace formatting, test, warning-denied clippy, and diff checks
recorded above pass. Script and Agent smoke runs reach turn 2/player phase with
`TurnAdvanced`; Agent frames contain exactly 24 and 40 lines at 80x24 and
120x40.

**Next Steps.** Consider surfacing nested replayed game events on the outer
`StepReplay` report without applying their battle-stat and threat side effects a
second time.

## 2026-08-23 - headless end-turn schedule parity

**Goal.** Repair the script/agent path where `end` queued a transition but
never ran turn management or enemy AI.

**Accomplishments.** `run_pending_reports()` now advances `Action::EndTurn`
through the same schedule used by the TTY, with a defensive tick bound, until
control returns on the next player turn or the game ends. The returned
`StepReport` includes the final snapshot, `TurnAdvanced` outcome, both phase
changes, enemy events, schedule diagnostics, and corrected history metadata.
Scheduled event side effects such as threat and battle-stat updates are applied
once during real-time updates and tracked until the event queue is drained;
already-processed phase/combat events no longer contaminate the next action's
outcome or report. A parity regression compares a Script-sourced `end` with the
normal interactive action plus schedule ticks.

**Verification.** `cargo fmt --all --check`, `cargo test --workspace`, and
`cargo clippy --workspace --all-targets -- -D warnings` pass. The JSON script
runner reaches turn 2/player phase with `TurnAdvanced`; the Agent runner reports
the same Agent-sourced outcome with exact 80x24 and 120x40 frame heights.

**Next Steps.** Consider exposing schedule-settling policy as a small reusable
input-runner primitive only if another prototype needs discrete headless turns;
otherwise keep this game-specific pacing policy in Wuthering Terminal.

## 2026-08-23 - turn-committed weather safety

**Goal.** Extend the shared safety planner to weather without preserving the
weather system's frame-driven side effects.

**Accomplishments.** Weather effects now prepare at most once per game turn, so
cycle-boundary weather, snow status extension, terrain changes, RNG use, and
lightning warnings no longer repeat at render frequency. Lightning coordinates
are de-duplicated, remain committed until phase resolution, and damage only the
team whose phase is ending instead of every entity at both boundaries. Ambient
loops emit only when weather changes and restart after loading rather than on
every frame. `Action::StepToSafety` now combines committed lightning tiles with
boss and incursion telegraphs and retains its normal structured move/event/
history observability. Save/load preserves the committed turn and danger tiles,
while older serialized `Weather` resources receive compatible defaults. Each
resolved lightning hit now emits `GameEvent::WeatherHazardResolved` with typed
target/team/position/damage attribution and updates player damage-taken stats.

**Verification.** Focused weather, shared-action, structured-event, and
save/load regressions pass as part of the full formatting, workspace-test, and
warning-denied clippy gate recorded above.

**Next Steps.** Consider extracting the prototype's repeated danger-union
construction only when a second gameplay consumer needs the exact same set.

## 2026-08-23 - shared safety response for armed incursions

**Goal.** Let players, scripts, and agents react to already-armed incursion
attacks through an existing shared action with authoritative outcomes.

**Accomplishments.** `Action::StepToSafety` now treats the union of boss and
incursion attack telegraphs as danger, chooses a reachable tile outside every
armed pattern, and emits the normal structured `GameEvent::Moved` /
`ActionOutcome::Moved` surfaces. The action records the same outcome in action
history and reports missing selection, missing warnings, already-safe actors,
insufficient AP, and blocked escape routes as categorized failures instead of
silent `NoOp` results. Stale selected entities no longer panic this path.

**Verification.** Targeted shared-action regressions pass. `cargo fmt --all
--check`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets
-- -D warnings` pass. The agent runner reported the expected Agent-sourced
failure at both 80x24 and 120x40.

**Next Steps.** Consider extending the safety planner to other committed damage
surfaces such as weather danger zones while retaining one authoritative union
of unsafe tiles and normal movement observability.

## 2026-08-23 - unified autonomous improvement prompt

**Goal.** Replace the fragmented continuation prompt kit with one durable prompt
for continuous autonomous development, optimization, review, and bug fixing
without routine human intervention.

**Accomplishments.** Consolidated the project context, architecture, vertical
slice, shared-control, modularity, testing, hardening, documentation, bootstrap,
autonomous-run, and tactical-RPG guidance into `prompt/improve.md`. The new
prompt uses a repeating evidence-driven work loop, makes each completed batch a
checkpoint rather than a stop condition, defines autonomous decision and
blocker-pivot rules, and distinguishes long-running maintenance from unbounded
or destructive commands. Removed the superseded prompt files and updated the
agent guides to reference the single source.

**Verification.** Documentation links, prompt inventory, stale prompt-name
references, and whitespace were checked after consolidation.

**Next Steps.** Use `prompt/improve.md` as the only continuation prompt and
revise it in place when the autonomous operating policy changes.

## 2026-08-23 - intercepted incursion openings

**Goal.** Make the existing `Intercept` response change the spawned
mini-boss's first attack without introducing a separate combat or agent path.

**Accomplishments.**
- Successful incursion Intercepts now leave a saveable,
  `IncursionFirstAttackDisrupted` marker on the spawned mini-boss. Arming the
  first attack consumes the marker, so subsequent attacks remain class-normal.
- Void Terror's intercepted Void Rend uses a radius-one `short-cross`; Frozen
  Sentinel's intercepted Glacial Lock removes the escape-side tile from its
  ring and reports a `broken-ring`.
- `IncursionAttackTelegraph` and agent snapshot previews expose an
  `intercepted` flag alongside the authoritative modified tiles. Battle-preview
  intents name the same modified patterns, and existing rendering consumes the
  modified tile list directly.
- The Intercept action outcome now records that the first attack was disrupted,
  keeping action-history and replay metadata informative. The pending marker
  and armed telegraph both survive save/load.
- Added end-to-end coverage through `Game::apply_action()` for both class
  patterns, one-shot consumption, action metadata, intents, and save/load.

**Verification.** Targeted intercepted-incursion and save/load tests pass.
`cargo fmt --all --check` and `cargo test --workspace` pass. The agent runner
produces exact 80x24 and 120x40 frames with the `incursion_attacks` JSON field
present at both sizes.

**Next Steps.** Consider a shared action for reacting to already-armed attacks
(guard or reposition assists) while keeping the committed telegraph tiles as
the authoritative resolution surface.

## 2026-08-23 - persistent incursion attack telegraphs

**Goal.** Continue the dynamic-floor-event slice after spawn so mini-boss
attacks remain avoidable, visible, and agent-readable instead of falling back
to ordinary immediate attacks.

**Accomplishments.**
- Event-spawned mini-bosses now carry a saveable `IncursionMiniBoss` marker and
  arm one-turn `IncursionAttackTelegraph` records during the normal enemy AI
  phase. Void Terror uses cross-shaped Void Rend; Frozen Sentinel uses
  ring-shaped Glacial Lock.
- Arrival and attack geometry share the same `incursion_shape()` mapping backed
  by `verryte-map::TileShape`. Armed attacks render as colored warning tiles,
  resolve once on the following enemy phase, emit structured telegraph/resolve
  events, and disappear immediately if their owner is defeated.
- Snapshots and the agent JSON expose `incursion_attacks` with source, attacker,
  pattern, origin, tiles, predicted damage, and resolve turn. Battle-preview
  enemy intents describe the same committed attacks.
- Registered the marker and attack queue for save/load, cleared the queue on
  floor transition, and added a fallback for older saves that lack the resource.
- Added focused integration coverage for spawn marking, both class shapes,
  arming/intent observability, single resolution, owner cleanup, save/load, and
  older-save compatibility.

**Verification.** Targeted incursion tests passed. Full workspace verification
is recorded in the final handoff for this run.

**Next Steps.** Let `Intercept` choice at the arrival warning influence the
first attack (for example shortening cross arms or opening a safe gap in the
ring), with the modified tiles kept in the same snapshot and replay surfaces.

## 2026-08-22 - event items, incursion patterns, and map shapes

**Goal.** Keep deeper-floor events on the shared action path while giving
agents distinct telegraph patterns and inventory answers that do not need
new recipes.

**Accomplishments.**
- Extracted origin-centered `TileShape` primitives into `verryte-map`
  (`Disk`, `Square`, `Cross`, `Line`, `Cone`, `ManhattanRing`, `Diamond`)
  with `TileGrid::clip_points` / `points_in_shape`. Battle preview AoE now
  uses those engine helpers instead of prototype-local tile math.
- Mini-boss telegraphs reuse that surface: Void Terror paints a `cross`,
  Frozen Sentinel paints a `ring` plus spawn origin. Snapshots expose
  `pending_floor_event_pattern` and `pending_floor_event_item_offers`.
- Existing inventory items answer pending events through `UseItem` and
  `Action::RespondToFloorEvent`: Cleanse Remedy purifies a surge, Aegis
  Elixir bolsters an incursion, Energy Elixir intercepts without standing
  on danger tiles, and a Healing Potion channels a Healing Surge into
  extra party healing. Script tokens `purify` / `bolster` / `channel`
  consume the matching item on the same path.

**Verification.** Targeted `verryte-map` and `wuthering-terminal` tests,
then workspace verification.

**Next Steps.** Add event-specific mini-boss attack telegraphs that reuse
`TileShape` after spawn, or let intercepting from a ring vs cross arm
change the incursion AI.

## 2026-08-22 - floor event telegraphs and player responses

## 2026-08-22 - floor event telegraphs and player responses

**Goal.** Give deeper-floor events a one-turn telegraph and player choices
without splitting the shared action path.

**Accomplishments.**
- Floor 2+ events now warn one player turn before they resolve. Mini-boss
  incursions paint orange danger tiles; snapshots expose `pending_floor_event`,
  resolve turn, tiles, and any committed response for agents and scripts.
- Added `Action::RespondToFloorEvent` (`brace` / `intercept` / `embrace`) on
  the same `apply_action()` path as terminal, script, and agent control.
  Brace weakens the event, Intercept delays it from a telegraphed tile, and
  Embrace resolves it immediately for Concert Energy.
- Added a `Ward Charm` recipe (Cleanse Remedy + Energy Elixir) whose
  `EventWard` effect auto-braces a pending event without spending AP.
- Structured `GameEvent` / `ActionOutcome` variants cover telegraph, response,
  and trigger; pending state is saveable with a missing-field default.

**Verification.** `cargo test -p wuthering-terminal` and `cargo test --workspace`
plus `cargo fmt --check`. Agent runner checked at `--size 80x24` and
`--size 120x40` with `brace` / `snapshot` tokens.

**Next Steps.** Add event-specific player choices that consume items already
in inventory (without a new recipe), or unique mini-boss patterns that reuse
the pending-tile telegraph surface.


## 2026-08-12 - dynamic floor events

**Goal.** Add observable mid-floor events to the tactical RPG without creating
a separate control or simulation path.

**Accomplishments.**
- Added a serializable `DynamicFloorEvents` resource that schedules one
  seed-driven event every three turns on Floor 2+: either a timed modifier
  surge or a scaled mini-boss incursion on a deterministic free tile.
- Reused the existing floor-modifier, spawn-scaling, RNG, message-log, and event
  systems. Events emit `GameEvent::FloorEventTriggered` and are promoted to a
  structured `ActionOutcome::FloorEventTriggered` on the shared action-report
  path.
- Extended snapshots and agent JSON with `next_floor_event_turn` and a bounded
  `recent_floor_events` history. Registered the resource for save/load and added
  missing-resource fallback for older saves.
- Added focused coverage for once-per-turn scheduling, snapshot/report
  observability, outcome serialization, and save/load preservation.

**Verification.** `cargo test --workspace` passes, including 270 Wuthering unit
tests, 58 integration tests, 18 mechanics tests, and 42 save/load tests. The
agent runner produced correctly sized 80x24 and 120x40 frames with the new
snapshot fields. `cargo fmt --check` remains blocked by pre-existing formatting
drift in unrelated dirty-worktree edits; new sections were kept rustfmt-clean.

**Next Steps.** Add event-specific telegraphs or player choices while retaining
the same event, action, and snapshot surfaces.

## 2026-08-12 - autonomous engine run

**Goal.** Strengthen the shared control and observability path while removing
prototype-local movement logic.

**Accomplishments.**
- Fixed `verryte-input` recording so an action is captured once when queued,
  rather than duplicated when it is later drained. Added regression coverage
  for single-action, batch-drain, and trace behavior.
- Updated the headless Wuthering agent runner to tag injected commands as
  `ActionSource::Agent`, preserving provenance without creating a second game
  path.
- Extended Wuthering snapshots with timed floor-modifier durations and active
  weather danger zones for structured agent planning.
- Added deterministic cost/row/column ordering to
  `verryte-map::ReachabilityMap::points` and migrated Wuthering's reachable-tile
  calculation to that reusable weighted primitive.
- Removed the level-editor unreachable fallback and fixed stale test warnings.

**Verification.** Targeted input, map, and Wuthering snapshot tests pass. Full
workspace verification remains the final step for this run.

**Next Steps.** Implement a small dynamic floor-event resource that can emit
structured events (modifier changes or mini-boss spawns) through the existing
action reports, using the new snapshot surfaces for agent control.

## 2026-06-18

**Goal.** Enhance tactical RPG prototype with Advanced AI and Reactive Environments.

**Accomplishments.**
- **New Character Class**: Added `FrozenSentinel`, a heavily armored defender-type enemy.
- **Defender AI Archetype**: Implemented a new AI strategy where units prioritize staying near and protecting high-value allies (like the Boss or Clerics).
- **Fire Elemental Status**: Introduced `Fire` as a new elemental status that deals damage over time and enables new reactions.
- **Reactive Environments**:
  - **Melt Reaction**: Fire + Ice on an entity deals bonus damage and cleanses both. If the entity is on an `Ice` tile, the tile melts into `Water`.
  - **Combustion Reaction**: Fire + Nature deals bonus damage and refreshes the Fire duration.
- **Verification**: Added 18 tests to `new_mechanics.rs` and updated existing bestiary tests to account for the new character class. Verified the shared control path via `wuthering-terminal-agent`.

**Next Steps.**
- Implement **Dynamic Floor Events**: Random events that change floor modifiers or spawn mini-bosses mid-floor.
- Expand **Equipment Crafting**: Add more recipes and a dedicated UI for the crafting system.
- Refine **Level Editor**: Add more tile types and entity placement options.

**Goal.** Continue expanding tactical RPG prototype content and mechanics, picking up from previous completion of the core roadmap.

**Accomplishments.**
- **Assassin Teleport AI**: Upgraded the `Assassin` AI archetype (used by VoidTerror) to possess a "Shadow Step" ability. If a player is within 5 tiles but not adjacent, and the enemy has enough AP, it will actively teleport to a flanking position (or any valid adjacent tile) before attacking.
- This teleport dynamically utilizes the engine's VFX system (`emit_burst` and `Flash::region` in purple colors) to visually telegraph the mechanic.
- Fixed minor state synchronization issues within `enemy_ai_system` when `Position` is updated during enemy turns to ensure the combat and VFX system target the correct coordinates.

**Verification**: Ran all workspace unit and integration tests successfully (`cargo test --workspace`).

**Next Steps**: Further expansions of character rosters, unique boss encounters, or deeper level editor mechanics.

## 2026-06-14 (Part 2)

**Goal.** Since the initial roadmap for `wuthering-terminal` was fully complete, expand content and add tooling as suggested by the last worklog entry.

**Accomplishments.**
- **Dedicated Level Editor**: Built `prototype/wuthering-terminal/src/bin/editor.rs`, a standalone interactive TUI editor using `verryte-tty` and `verryte-terminal::Grid`. It provides a canvas to paint `TacticalMap` tiles with a selectable palette (Wall, Grass, Water, Lava, etc.) and camera offset logic.
- **VoidTerror Content Expansion**: Fully integrated the `VoidTerror` enemy into the game.
  - Assigned it to the `Assassin` AI archetype to allow flanking and pursuit behavior.
  - Added its Bestiary entry detailing its lore and drop table (`Void Core`, `Dark Essence`).
  - Increased its spawn frequency significantly for procedural dungeon generation on Floor 2 and beyond.
- **Cleanup**: Fixed missing or failing assertions in tests related to Bestiary totals and eliminated several unused import warnings using `cargo fix`.

**Verification**: Confirmed all 360+ workspace tests pass. Verified the `editor` binary compiles successfully without warnings.

## 2026-06-14

### Context: Tactical RPG Prototype & VFX System Integration
- **VFX System Extraction**: Verified that `verryte-terminal::vfx` already contains the comprehensive VFX system (particles, flashes, shakes, floating text, AoE rings).
- **VFX Demo Refactoring**: Updated `prototype/vfx-demo/src/main.rs` to use the library's `VfxSystem` and emitters. Removed redundant local code, simplifying the demo and ensuring it serves as a clean reference for the engine's modular design.
- **Combat Feedback Enhancements**: Significantly improved the "juice" of the Tactical RPG prototype (`prototype/wuthering-terminal`).
    - Added `emit_burst` sparks on all hits for better impact feedback.
    - Added `emit_shatter` crystalline effects on critical hits.
    - Implemented brief regional flashes at the hit location using `trigger_flash_region`.
    - Enhanced floating damage text with randomized horizontal velocity and `QuadOut` easing, making numbers "jump" out of characters.
- **Verification**: Confirmed all 360+ workspace tests pass (270 unit, 56 integration, 40 save/load). Verified `vfx-demo` compiles and aligns with the refactored engine crates.
- **Next Steps**: The prototype is mechanically complete and visually polished. Future work could focus on expanding the content (more characters, floors, or complex enemy AI patterns) or implementing a dedicated Level Editor.

## 2026-05-22 - tactical RPG initialization and sprite rendering

**Goal.** Start the next Verryte prototype: a turn-based tactical RPG on a grid
battlefield, validating multi-character teams and high-fidelity sprites.

**Accomplishments.**
- Added `wuthering-terminal` to workspace members.
- Initialized `prototype/wuthering-terminal` with crate structure, `Action`
  funnel, and `Game` state.
- Implemented **Tactical grid scene** (Step 1 of roadmap):
    - Multi-cell tile rendering (8x4 cells per tile).
    - Image-based character sprites (Kael, Lyra, Mira, Blight Sovereign) loaded
      from PNG assets with chroma-key transparency.
    - Centered character rendering within tiles.
    - Camera system integration for cursor following.
- Self-healed `ash-courier` and `verryte-input`:
    - Standardized `InputRouter` methods (added `handle_event` and `pop_action`
      aliases).
    - Fixed `TileGrid` method calls (`cellular_automata_cave`, `points_matching`).
    - Restored `run_pending_reports` in `ash-courier` for script validation.
- Added baseline unit test for `wuthering-terminal` initialization.

**Next Steps.**
- Implement the **Turn system** (Step 2): player phase → enemy phase, AP tracking.
- Add basic combat mechanics (ranges, stats).
- Integrate VFX system into tactical animations.

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
