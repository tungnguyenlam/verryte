# Wuthering Terminal — Prototype Goal

A turn-based tactical RPG on a grid battlefield, built as a terminal-native
Verryte prototype. Inspired by Wuthering Waves combat mechanics, using original
characters and a universal dark-fantasy plot structure that every language model
and image model knows instinctively.

## Purpose

This prototype exists to prove that Verryte can support complex, visually
rich games — not just the Ash Courier roguelike. It stress-tests:

- Multi-character team management (swap, cooldowns, synergies)
- Real-time VFX overlays on a turn-based grid (particles, shake, flash, AoE)
- Telegraphed enemy AI with readable attack patterns
- Absorbable ability systems (defeat → gain)
- Phase-based boss encounters
- Adaptive-resolution sprite rendering
- The shared control path (terminal, script, agent) under complex mechanics

## Plot

A corruption spreads across the land, twisting creatures into violent husks.
Three heroes descend into the heart of the corruption to seal its source.

### World

The world is not named. It does not need to be. The setting is a generic
dark-fantasy realm where:

- An ancient civilization once wielded powerful magic
- That civilization fell, leaving behind ruins and sealed evils
- A creeping corruption (called "the Blight") now warps wildlife and people
- Blight-touched creatures become aggressive, losing their minds
- Small settlements survive on the edges, sending expeditions into the Blight
- Deep within the Blight lies the Sealed Throne — the source of the corruption

This structure is universal. Every model knows it. Every image generator can
produce it. No IP is involved.

### Characters

Three heroes form the active party. Each has a distinct combat role, visual
identity, and personality archetype that every model recognizes instantly.

**Kael — The Vanguard**
- Role: Melee DPS / Tank
- Weapon: Greatsword
- Visual: Silver-white hair, dark fitted armor, blue glow on weapon
- Element: Ice
- Personality: Stoic, protective, speaks few words but acts decisively
- Archetype: The lone swordsman with a mysterious past (Cloud, Sephiroth, Dante)

**Lyra — The Arcanist**
- Role: Ranged DPS / Elemental reactions
- Weapon: Floating catalyst (orb)
- Visual: Dark robes with purple accents, glowing eyes, arcane circles
- Element: Lightning
- Personality: Curious, analytical, speaks in precise sentences
- Archetype: The battle mage (Vivi, Y'shtola, Mona)

**Mira — The Warden**
- Role: Healer / Support
- Weapon: Staff with a luminous tip
- Visual: White and gold robes, gentle expression, green healing glow
- Element: Nature
- Personality: Compassionate, determined, quietly fierce
- Archetype: The white mage / cleric (Aerith, Yuna, Rem)

### Antagonist

**The Blight Sovereign**
- The final boss, sealed within the Sealed Throne
- Multi-phase encounter
- Visual: Dark armor, massive frame, red-black corruption aura, horns
- Personality: Speaks in riddles, claims to be a victim not a villain
- Archetype: The dark lord / demon king (Ganondorf, Chaos, Lavos)

### Progression

The party moves through three zones of increasing depth:

1. **The Fringe** — light corruption, basic enemies, tutorial mechanics
2. **The Depths** — heavier corruption, elite enemies, telegraphed AoE attacks
3. **The Sealed Throne** — boss arena, multi-phase fight

Each zone has 3-5 encounters. Defeated Blight-touched enemies may drop
absorbable echoes — fragments of power the party can equip for new abilities.

### Tone

Dark but hopeful. The world is broken but worth saving. Characters support each
other. Victory feels earned. The terminal aesthetic (half-block sprites, colored
glyphs, particle bursts) gives it a unique retro-modern feel that no other
engine produces.

## Combat Design

### Elemental System

Three elements with interaction:

| Combo | Reaction | Effect |
|-------|----------|--------|
| Ice + Lightning | Shatter | Bonus damage, AoE frost burst |
| Lightning + Nature | Overgrowth | Root enemies in place for 1 turn |
| Nature + Ice | Bloom | Healing zone on the ground for 2 turns |

Elemental reactions reward team composition and turn order planning.

### Turn System

- **Player Phase**: each character gets 2 action points. Move costs 1, attack
  costs 1-2, swap costs 1, use ability costs 1-2.
- **Enemy Phase**: each enemy executes telegraphed actions. Attack zones were
  shown on the grid during the player phase.
- **Swap**: spend 1 AP to swap the active character. The swapped-in character
  gets a brief invulnerability window (intro mechanic). Cooldown: 2 turns.

### Telegraphed Attacks

Before each enemy acts, the grid tiles it will attack are highlighted in red.
The player can see the danger zones and move characters out of the way, or
accept the hit if positioning matters more.

This is the core tactical loop: read the enemy, position your team, exploit
elemental reactions, manage action points.

### Echo Absorption

Defeated enemies have a chance to drop an Echo — a passive or active ability.
The party can equip up to 3 echoes total. Examples:

- **Frostbite Echo**: basic attacks have a 20% chance to apply Ice
- **Thorn Echo**: reflect 10% of damage taken back to the attacker
- **Swift Echo**: +1 action point for the equipped character

Echoes add build variety without complicating the core combat loop.

## Technical Requirements

### Rendering

- Grid-based battlefield rendered with `verryte-terminal` primitives
- Half-block sprite rendering for characters (`image_to_grid`)
- Chroma-key transparency for sprite backgrounds
- Adaptive resolution: sprites scale to terminal size
- VFX overlay layer: particles, shake, flash, floating text, AoE rings
- Diff-based rendering at 30 FPS for smooth animation during VFX sequences

### Engine Integration

- Uses `verryte-core` for ECS (entities, components, events, schedules)
- Uses `verryte-input` for action bindings and the unified control path
- Uses `verryte-map` for grid, pathfinding, bounds, visibility
- Uses `verryte-terminal` for cell, grid, sprite, viewport, diff rendering
- Uses `verryte-tty` for crossterm frontend

### Runners

Every gameplay path must converge on the same action system:

- **TTY runner**: interactive play with keyboard controls
- **Script runner**: non-interactive, runs action sequences, reports outcome
- Both use the same `Action` enum and `apply_action()` function

### Assets

- Character sprites: `assets/kael.png`, `assets/lyra.png`, `assets/mira.png`,
  `assets/blight-sovereign.png` (loaded at runtime via `image_to_grid()`,
  resized and chroma-keyed on startup. The `image` crate is a runtime
  dependency. Build-time const array compilation via `scratch/png_to_ansi.py`
  is the long-term plan but not needed for the prototype.)
- No external runtime dependencies — all rendering is terminal-native

## What This Prototype Does NOT Do

- It is not a complete RPG. It is a vertical slice.
- It does not have a save system, inventory, or overworld.
- It does not use Genshin's or WuWa's IP. Characters and world are original.
- It does not replace Ash Courier. Both prototypes coexist.
- It does not add new engine crates without justification. Reuse existing ones.

## Success Criteria

The prototype is successful when:

1. A player can control a 3-character team on a grid battlefield
2. Turn-based combat with action points works end-to-end
3. Elemental reactions trigger with visible VFX (particles, flash, shake)
4. Enemy attacks are telegraphed on the grid before executing
5. The script runner can play through an encounter and report win/loss
6. The boss fight has at least 2 phases with distinct attack patterns
7. Echo absorption adds at least 3 equippable abilities
8. The whole thing runs in a terminal at 30 FPS with diff-based rendering
