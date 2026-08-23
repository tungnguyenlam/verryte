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

The [prompt/](prompt/) directory contains reusable continuation prompts. It is
project context, not runtime code.

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
  real-time 30 FPS game loop. Refactored to use the engine's `VfxSystem`. Run with `cargo run -p vfx-demo`.

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

## Current Engine Capabilities

As of the latest commits, Verryte has:

- **ECS core** (`verryte-core`): entities, components, resources, events, queries, schedules.
- **Input system** (`verryte-input`): unified input events (keyboard, mouse, scroll), action bindings, command parsing, action queues, replay traces, `ActionSource` for origin tracking.
- **Map & geometry** (`verryte-map`): grid, bounds, distance, visibility, reachability, pathfinding, `TileGrid` with iterators.
- **Terminal rendering** (`verryte-terminal`): cell, color, grid, clipping, viewport, diff, line, border, text rendering, batch write helpers.
- **VFX System** (`verryte-terminal::vfx`): High-performance terminal VFX with particles (fire, ice, lightning, slash, burst, heal, bloom, shatter), eased screen shake, regional and full-screen flashes, floating text with velocity and easing, and AoE rings.
- **TTY frontend** (`verryte-tty`): crossterm integration, real-time input translation, incremental cell-diff rendering.
- **Audio** (`verryte-audio`): spatial/panned audio playback via rodio, with `AudioEvent` integration from `verryte-core` and volume/pan controls.
- **Adaptive resolution sprites**: build-time PNG-to-Rust compilation pipeline (`scratch/png_to_ansi.py`) that bakes chibi pixel art into static `[[(u8, u8, u8); W]; H]` arrays at 6 resolution tiers (TINY through ULTRA). At runtime, `crossterm::terminal::size()` selects the best tier purely by terminal cols×rows.
- **Wuthering Terminal prototype** (`prototype/wuthering-terminal`): tactical RPG prototype, multi-character teams, high-fidelity image-based sprites, larger tile grids, complex turn-phase scheduling. (Steps 1-8 Complete: grid scene, turn system, combat, team-swap QTE, telegraphed attacks, Echo absorption, sprite pipeline, boss fight.)
  - Enhanced with high-quality VFX feedback: hit sparks, shatter crits, regional flashes, and eased floating damage numbers.
  - Observability surface for agents and replays: `ActionOutcome` enum on every `StepReport`, `Snapshot::reachable_tiles` and `targetable_tiles` for the selected unit, `Game::last_outcome` and `boss_just_transitioned()`. Computed outcomes are serialized into each `ActionRecord`'s metadata under the `"outcome"` key. Boss phase 2 is data-driven via `BossConfig` and applies a configurable `ElementalShield` on threshold cross.
  - Multi-floor progression: dynamic procedural BSP dungeon generation for Floor 2+, stair tile interaction via `Action::NextFloor` or keyboard `>`. Floor-scaling difficulty: enemy stats increase +15% per floor (HP, ATK, DEF), boss shields scale +50 per floor, elite encounters on Floor 3+, bonus loot on deeper floors. XP awards scale +15% per floor depth.
  - Dynamic floor events: Floor 2+ schedules seed-driven modifier surges or mini-boss incursions every three turns. Events are telegraphed one player turn ahead with orange danger tiles, then resolve unless answered. `Action::RespondToFloorEvent` (`brace` / `intercept` / `embrace` / `purify` / `bolster` / `channel`) and crafted Ward Charms share the same action path. Existing inventory items answer events without new recipes (Cleanse Remedy, Aegis Elixir, Energy Elixir, Healing Potion). Mini-boss telegraphs use `verryte-map::TileShape` patterns (`cross` / `ring`). Snapshots expose `pending_floor_event*` fields plus `next_floor_event_turn` and `recent_floor_events`.
  - Incursion mini-boss attacks: spawned Void Terror and Frozen Sentinel incursions arm persistent, one-turn `cross` / `ring` attacks through normal enemy AI. Intercepting the arrival disrupts only the first attack (`short-cross` / `broken-ring`). Exact tiles and the `intercepted` flag are exposed through `incursion_attacks`, enemy intents, saves, and structured events.
  - Item alchemy/crafting: combining items via recipe matches using `Action::CraftItem(idx1, idx2)`, replacing items in inventory and triggering visual bloom VFX and cleansed audio. Supports advanced recipes like `Divine Remedy` and `Elixir of the Gods` utilizing `CleanseAndHeal` effects.
  - Level-up system (`progression.rs`): XP from combat scales by floor depth (+15% per floor). Level-ups grant +10 HP, +2 ATK, +1 DEF, a skill point, and gold VFX (particles, shake, flash, floating text). Prestige classes unlock at kill/damage/healing milestones.
  - Hero passive traits: Warrior (`SwiftFoot` for +1 bonus AP on turn start), Mage (`StormChaser` for +10 Lightning shatter damage), and Healer (`PurifyingTouch` for 50% cleanse chance on heal).
  - Advanced map/rendering capabilities: Walkable `Tile::Mud` terrain (costs 3 AP) with custom VFX, waypoint-based grid pathfinding, Dijkstra map-based fleeing paths, and decorative/overlay grid rendering helpers (`draw_crosshair`, `draw_diagonal_crosshair`, etc.).
  - Equipment system (`equipment.rs`): Weapon/Armor/Accessory slots with stat bonuses (ATK, DEF, HP, SPD) and special effects (Lifesteal, CritBoost, ElementalDamage, HpRegen). 13 predefined items with class-appropriate starter gear. `EquippedItems` component on player characters.
  - Tactical AI (`ai.rs`): `TacticalAI` with battlefield assessment, cover detection (Bresenham LOS), flanking position calculation, threat maps (ATK-weighted Manhattan distance), archetype-specific strategies (Chaser with flanking, Cleric heal-first, Coward retreat-under-30%, Assassin with Shadow Step teleportation, Defender with protect-ally-priority), focus-fire targeting.
  - New Characters: Added `VoidTerror` (Assassin archetype) and `FrozenSentinel` (Defender archetype).
  - Elemental Reactions: Expanded with `Melt` (Fire + Ice -> bonus damage + terrain melting) and `Combustion` (Fire + Nature -> damage + refreshed fire). `ElementalStatus::Fire` added to the core status effects.
  - Environmental hazards (`hazards.rs`): 6 new tile types (SpikeTrap, PoisonCloud, HealingSpring, CrackedFloor, PressurePlate, ThornBush). `HazardSystem` with procedural hazard population, trigger processing, cracked floor destruction, and trigger-count exhaustion cleanup.
  - Skill upgrade trees (`skill_tree.rs`): Per-class branching skill trees (Warrior, Mage, Healer) with 4 branches × 3 tiers each. Skill point economy on level-up, prerequisite enforcement, damage/range/AoE/heal bonus aggregation, passive stat bonuses. `UpgradeSkill` and `ToggleSkillTree` actions.
  - Battle preview (`battle_preview.rs`): Damage preview (ATK/DEF/level/elemental formula, min/max/expected, hit/crit chance, can-kill flag), AoE preview (Circle/Square/Cross/Line/Cone shapes with entity classification), turn order display (SPD-descending), enemy intent prediction (archetype-based).
  - Hazard terrain: 6 additional tile types in `map.rs` with walkability, movement cost, and ASCII mapping support.
- **Terminal VFX demo** (`prototype/vfx-demo`): interactive demo proving real-time terminal animation at 30 FPS. Refactored to use the engine's `VfxSystem`. Particle system (fire, ice, lightning, slash, burst, heal, bloom, shatter), screen shake, flash overlays, floating damage text, expanding ring indicators, diff-based rendering. Loads PNG character sprites (Kael, Mira, Blight Sovereign) from `wuthering-terminal/assets/` via `image_to_grid()` with chroma-key transparency. Run with `cargo run -p vfx-demo`.

**Key architectural invariant:** all gameplay paths (terminal input, scripted commands, tests, replays, agent injection) converge on the same `Action` enum and `apply_action()` function. Do not split this path.
