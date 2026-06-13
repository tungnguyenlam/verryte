# Verryte Worklog

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
