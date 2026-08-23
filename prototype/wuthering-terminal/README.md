# Wuthering Terminal

A turn-based tactical RPG prototype built on the Verryte terminal-game engine.

## Overview

Wuthering Terminal validates that Verryte can support complex, visually rich
games — not just small roguelikes. It stress-tests multi-character teams,
real-time VFX overlays, telegraphed enemy AI, absorbable ability systems,
phase-based boss encounters, and adaptive-resolution sprite rendering.

A corruption spreads across the land, twisting creatures into violent husks.
Three heroes descend into the heart of the corruption to seal its source.

## Characters

| Name | Role | Element | Weapon |
|------|------|---------|--------|
| Kael | Melee DPS / Tank | Ice | Greatsword |
| Lyra | Ranged DPS | Lightning | Floating catalyst |
| Mira | Healer / Support | Nature | Staff |
| Glacial Golem | Floor 2 Heavy Enemy | Ice | Fists |

## Passive Traits

Characters can have unique traits that modify gameplay:
- **SwiftFoot** (Kael): Starts each turn with 3 AP instead of 2.
- **StormChaser** (Lyra): Lightning reactions deal +10 extra Shatter/Overgrowth damage.
- **PurifyingTouch** (Mira): 50% chance to cleanse negative status effects when healing.
- **IceWalker** (Glacial Golem / Custom): Prevents sliding on Ice terrain.

## Combat

- **Turn-based** with action points (2 per character per turn)
- **Elemental reactions**: Ice+Lightning=Shatter, Lightning+Nature=Overgrowth, Nature+Ice=Bloom
- **Telegraphed attacks**: enemy danger zones shown on the grid before executing
- **Assassin Teleport**: Enemies with the Assassin archetype (like VoidTerror) can use Shadow Step to teleport to flanking positions for sudden attacks.
- **Echo absorption**: defeated enemies drop equippable abilities (Swift, Frostbite, Thorns)
- **QTE team swap**: spend concert energy for instant swap with intro skill
- **Boss phases**: multi-phase fight with stat boosts and new attack patterns
- **Inventory**: healing potions, energy elixirs, cleanse remedies, aegis elixirs
- **Equipment**: class starter gear, enemy-awarded set gear, upgrade kits, stat bonuses, lifesteal, and per-turn HP regeneration. Equipment upgrades and set rewards are exposed as structured `ActionOutcome` values for scripts and replays. Upgrade Kits are used through `equip_upgrade:<slot>` and are not consumed by direct inventory use.
- **Alchemy crafting**: combine two items in inventory (e.g. 2x Healing Potion -> 1x Mega Potion, Potion + Elixir -> Elixir of Life, Cleanse Remedy + Energy Elixir -> Ward Charm) using `Action::CraftItem`. Successful crafts report `ActionOutcome::Crafted`; invalid recipes and slots report structured failures. Ward Charm uses `ItemEffect::EventWard` to Brace a telegraphed floor event without spending AP.
- **Floor progression**: stairs spawn upon defeating enemies, triggering descent to deeper floors (using `Action::NextFloor` or keyboard key `>`). Floor 2+ features a procedural BSP dungeon layout, scaled enemy stats (+15% per floor), bonus loot, and an elite Glacial Golem encounter on Floor 3+. Boss shields scale with floor depth.
- **Dynamic floor events**: beginning on Floor 2, seed-driven modifier surges or mini-boss incursions are telegraphed one turn before they resolve. Danger tiles, resolve turn, and any player response are exposed in snapshots. Players can `brace`, `intercept`, or `embrace` through the shared action path; a crafted `Ward Charm` (Cleanse Remedy + Energy Elixir) auto-braces without spending AP. Existing inventory items also answer events: Cleanse Remedy purifies a surge, Aegis Elixir bolsters an incursion, Energy Elixir intercepts from anywhere, and a Healing Potion channels a Healing Surge into extra party healing. Mini-boss telegraphs use class-specific patterns (`cross` for Void Terror, `ring` for Frozen Sentinel) from `verryte-map::TileShape`. Intercepting an incursion also disrupts its first post-spawn attack. Triggers still emit structured game events and action outcomes.
- **Incursion attack warnings**: once an event mini-boss has spawned, it keeps
  using its class shape for committed attacks. Void Terror charges a cross-shaped
  Void Rend; Frozen Sentinel charges a ring-shaped Glacial Lock. These attacks
  spend the mini-boss phase to arm, remain avoidable for the next player turn,
  then resolve once through normal enemy AI. Defeating the owner removes its
  warning immediately. An Intercept response shortens Void Terror's first cross
  to radius one or removes the escape-side tile from Frozen Sentinel's first
  ring; later attacks return to their normal patterns.
- **Elemental shields**: absorb damage before HP
- **Level-up system**: defeating enemies awards XP (scaled by floor depth). Level-ups grant +10 HP, +2 ATK, +1 DEF, and a skill point. Prestige classes (BladeMaster, Archmage, DivineHealer) unlock at milestones with VFX feedback.
- **Combo system**: consecutive hits on enemies increment the combo counter, boosting damage (+5% per combo point starting from the second hit), granting healing (+5 HP) and concert energy (+10 CE) every 3 combo points; combo resets on turn change or action failure
- **Battle stats**: tracks total damage dealt, taken, healing done, kills, swaps, turns, and maximum combo reached, serialized in snapshots for agent observability.

## Controls

### Movement & Actions

| Key | Action |
|-----|--------|
| Arrow keys / WASD | Move character |
| Space | Wait |
| Enter | Confirm / Attack |
| Esc | Cancel |
| Tab | Cycle character |
| E | End turn |

### Combat

| Key | Action |
|-----|--------|
| 1 | Skill 1 |
| 2 | Skill 2 |
| 3 | Skill 3 (QTE swap) |
| 4/5/6 | Swap to character 0/1/2 |
| I | Toggle inventory |

### Debug / Tools

| Key | Action |
|-----|--------|
| F | Brace a telegraphed floor event |
| N / [ | Intercept / Embrace a floor event |
| B | Toggle auto-battle |
| R | Step to safety |
| F3 | Toggle performance overlay |
| F5 | Save game |
| F9 | Load game |
| F10 | Toggle recording |
| F11 | Toggle replay |
| F12 | Step replay |
| P | Toggle auto-replay |
| Q | Quit |

## Runners

### Interactive TTY

```sh
cargo run -p wuthering-terminal --bin wuthering-terminal
```

Requires a real terminal. Renders at 30 FPS with diff-based updates.

### Script Runner (non-interactive, for tests/CI)

```sh
cargo run -p wuthering-terminal --bin wuthering-terminal-script -- "confirm skill1 confirm end"
```

The script runner accepts action tokens separated by spaces. It prints
rendered frames, state summaries, and event outcomes after each action.
Item use, crafting, equipment upgrades, set rewards, echo absorption, boss phase
transitions, save/load, action recording, replay mode changes, replay steps,
UI/tool toggles, prestige status views, rest recovery, floor modifier rerolls,
and invalid inventory, floor-transition, or replay attempts are reported through
structured `ActionOutcome` values so scripts and replays do not need to scrape
log text.

The agent runner uses the same command parser and `Game::apply_action` path but
labels queued actions as `Agent`. Its JSON snapshots also include
`active_modifier_durations` and `weather_danger_zones`, allowing an external
controller to reason about timed floor effects and imminent lightning strikes.
`next_floor_event_turn` and `recent_floor_events` expose dynamic event timing
and history without requiring frame or log scraping. When an event is
telegraphed, `pending_floor_event`, `pending_floor_event_resolves_on`,
`pending_floor_event_tiles`, `pending_floor_event_response`,
`pending_floor_event_pattern`, and `pending_floor_event_item_offers` describe the
warning, its danger tiles and telegraph pattern, any Brace/Intercept/Embrace/item
answer already committed, and which existing inventory items can still answer it.
After an incursion spawns, `incursion_attacks` continues that observability with
structured attacker, pattern, origin, tile, predicted-damage, and resolve-turn
fields plus an `intercepted` flag. Modified patterns are named `short-cross` or
`broken-ring`, and their exact safe tiles remain authoritative. The same attacks
appear in `enemy_intents` and as structured `IncursionAttackTelegraphed` /
`IncursionAttackResolved` game events.

### Script tokens

Movement: `north`, `south`, `east`, `west` (or `n`, `s`, `e`, `w`)

Combat: `skill1`, `skill2`, `skill3`, `confirm`, `cancel`, `end`, `wait`, `craft:<idx1>,<idx2>` (1-indexed, e.g. `craft:1,2`), `use:<idx>` (1-indexed), `equip_upgrade:<weapon|armor|accessory>`

Swap: `swap1`, `swap2`, `swap3` (or `4`, `5`, `6`)

Other: `autobattle`, `inventory`, `bestiary`/`lore`, `prestige`, `reroll`, `rest`, `safety`, `save`/`quicksave`, `load`/`quickload`, `record`/`recording`, `replay`, `replay_auto`/`auto_replay`, `step_replay`/`replay_step`, `stairs`/`next_floor`/`>`, `brace`/`fortify`, `intercept`/`disrupt`, `embrace`/`accept_event`, `purify`/`cleanse_event`, `bolster`, `channel`/`channel_surge`, `quit`

## Adaptive Sprites

Character sprites are compiled from PNG source art into static Rust arrays at
build time. At runtime, the engine selects the highest resolution tier that
fits the current terminal size:

| Tier | Sprite Size | Min Terminal |
|------|-------------|-------------|
| TINY | 6x8 | 60x20 |
| SMALL | 8x12 | 80x24 |
| MEDIUM | 12x16 | 100x30 |
| LARGE | 16x20 | 120x36 |
| XLARGE | 20x24 | 140x42 |
| ULTRA | 28x32 | 160x48 |

Resize your terminal window to see the visual fidelity adjust in real time.

## VFX

The prototype uses the `verryte-terminal::vfx` system for:

- Particle bursts (fire, ice, lightning, slash, heal, bloom, shatter)
- Screen shake on impacts
- Flash overlays for critical hits and phase transitions
- Floating damage numbers
- AoE ring indicators for telegraphed attacks
- Combo counter with escalating visual intensity

## Save/Load

Press F5 to save, F9 to load, or use script tokens `save` and `load`. Save
files are stored in the `saves/` directory as JSON. The full game state
(entities, components, map, RNG, clock) is serialized, and save/load actions
report `ActionOutcome::GameSaved` or `ActionOutcome::GameLoaded` with the
quicksave path.

## Architecture

Uses all core Verryte engine crates:
- `verryte-core` — ECS entities, components, resources, events, schedules
- `verryte-input` — unified input/action path (terminal, script, replay)
- `verryte-map` — grid, pathfinding, visibility, spatial queries
- `verryte-terminal` — cell rendering, sprites, VFX, layers, viewport
- `verryte-tty` — crossterm frontend with diff-based rendering
