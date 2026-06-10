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
- **Echo absorption**: defeated enemies drop equippable abilities (Swift, Frostbite, Thorns)
- **QTE team swap**: spend concert energy for instant swap with intro skill
- **Boss phases**: multi-phase fight with stat boosts and new attack patterns
- **Inventory**: healing potions, energy elixirs, cleanse remedies, aegis elixirs
- **Equipment**: class starter gear, enemy-awarded set gear, upgrade kits, stat bonuses, lifesteal, and per-turn HP regeneration. Equipment upgrades and set rewards are exposed as structured `ActionOutcome` values for scripts and replays. Upgrade Kits are used through `equip_upgrade:<slot>` and are not consumed by direct inventory use.
- **Alchemy crafting**: combine two items in inventory (e.g. 2x Healing Potion -> 1x Mega Potion, Potion + Elixir -> Elixir of Life) using `Action::CraftItem`. Successful crafts report `ActionOutcome::Crafted`; invalid recipes and slots report structured failures.
- **Floor progression**: stairs spawn upon defeating enemies, triggering descent to Floor 2 (using `Action::NextFloor` or keyboard key `>`) which features a procedural BSP dungeon layout
- **Elemental shields**: absorb damage before HP
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
Item use, crafting, equipment upgrades, set rewards, and invalid inventory or
floor-transition attempts are reported through structured `ActionOutcome`
values so scripts and replays do not need to scrape log text.

### Script tokens

Movement: `north`, `south`, `east`, `west` (or `n`, `s`, `e`, `w`)

Combat: `skill1`, `skill2`, `skill3`, `confirm`, `cancel`, `end`, `wait`, `craft:<idx1>,<idx2>` (1-indexed, e.g. `craft:1,2`), `use:<idx>` (1-indexed), `equip_upgrade:<weapon|armor|accessory>`

Swap: `swap1`, `swap2`, `swap3` (or `4`, `5`, `6`)

Other: `autobattle`, `safety`, `stairs`/`next_floor`/`>`, `quit`

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

Press F5 to save, F9 to load. Save files are stored in the `saves/` directory
as JSON. The full game state (entities, components, map, RNG, clock) is
serialized.

## Architecture

Uses all core Verryte engine crates:
- `verryte-core` — ECS entities, components, resources, events, schedules
- `verryte-input` — unified input/action path (terminal, script, replay)
- `verryte-map` — grid, pathfinding, visibility, spatial queries
- `verryte-terminal` — cell rendering, sprites, VFX, layers, viewport
- `verryte-tty` — crossterm frontend with diff-based rendering
