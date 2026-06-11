``# Verryte Agent Guide

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
  real-time 30 FPS game loop. Loads PNG character sprites (Kael, Mira,
  Blight Sovereign) from `wuthering-terminal/assets/` via `image_to_grid()`
  with chroma-key transparency. Run with `cargo run -p vfx-demo`.

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
- **TTY frontend** (`verryte-tty`): crossterm integration, real-time input translation, incremental cell-diff rendering.
- **Audio** (`verryte-audio`): spatial/panned audio playback via rodio, with `AudioEvent` integration from `verryte-core` and volume/pan controls.
- **Adaptive resolution sprites**: build-time PNG-to-Rust compilation pipeline (`scratch/png_to_ansi.py`) that bakes chibi pixel art into static `[[(u8, u8, u8); W]; H]` arrays at 6 resolution tiers (TINY through ULTRA). At runtime, `crossterm::terminal::size()` selects the best tier purely by terminal cols×rows.
- **Wuthering Terminal prototype** (`prototype/wuthering-terminal`): tactical RPG prototype, multi-character teams, high-fidelity image-based sprites, larger tile grids, complex turn-phase scheduling. (Steps 1-8 Complete: grid scene, turn system, combat, team-swap QTE, telegraphed attacks, Echo absorption, sprite pipeline, boss fight.)
  - Observability surface for agents and replays: `ActionOutcome` enum on every `StepReport` (`Hit`, `Healed`, `Moved`, `BossPhaseChanged`, `Failed`, `GameOver`, etc.), `Snapshot::reachable_tiles` and `targetable_tiles` for the selected unit, `Game::last_outcome` and `boss_just_transitioned()`. Computed outcomes are serialized into each `ActionRecord`'s metadata under the `"outcome"` key. Boss phase 2 is data-driven via `BossConfig` and applies a configurable `ElementalShield` on threshold cross.
  - Multi-floor progression: dynamic procedural BSP dungeon generation for Floor 2+, stair tile interaction via `Action::NextFloor` or keyboard `>`. Floor-scaling difficulty: enemy stats increase +15% per floor (HP, ATK, DEF), boss shields scale +50 per floor, elite encounters on Floor 3+, bonus loot on deeper floors. XP awards scale +15% per floor depth.
  - Item alchemy/crafting: combining items via recipe matches using `Action::CraftItem(idx1, idx2)`, replacing items in inventory and triggering visual bloom VFX and cleansed audio. Supports advanced recipes like `Divine Remedy` and `Elixir of the Gods` utilizing `CleanseAndHeal` effects.
  - Level-up system (`progression.rs`): XP from combat scales by floor depth (+15% per floor). Level-ups grant +10 HP, +2 ATK, +1 DEF, a skill point, and gold VFX (particles, shake, flash, floating text). Prestige classes unlock at kill/damage/healing milestones.
  - Hero passive traits: Warrior (`SwiftFoot` for +1 bonus AP on turn start), Mage (`StormChaser` for +10 Lightning shatter damage), and Healer (`PurifyingTouch` for 50% cleanse chance on heal).
  - Advanced map/rendering capabilities: Walkable `Tile::Mud` terrain (costs 3 AP) with custom VFX, waypoint-based grid pathfinding, Dijkstra map-based fleeing paths, and decorative/overlay grid rendering helpers (`draw_crosshair`, `draw_diagonal_crosshair`, etc.).
  - Equipment system (`equipment.rs`): Weapon/Armor/Accessory slots with stat bonuses (ATK, DEF, HP, SPD) and special effects (Lifesteal, CritBoost, ElementalDamage, HpRegen). 13 predefined items with class-appropriate starter gear. `EquippedItems` component on player characters.
  - Tactical AI (`ai.rs`): `TacticalAI` with battlefield assessment, cover detection (Bresenham LOS), flanking position calculation, threat maps (ATK-weighted Manhattan distance), archetype-specific strategies (Chaser with flanking, Cleric heal-first, Coward retreat-under-30%), focus-fire targeting.
  - Environmental hazards (`hazards.rs`): 6 new tile types (SpikeTrap, PoisonCloud, HealingSpring, CrackedFloor, PressurePlate, ThornBush). `HazardSystem` with procedural hazard population, trigger processing, cracked floor destruction, and trigger-count exhaustion cleanup.
  - Skill upgrade trees (`skill_tree.rs`): Per-class branching skill trees (Warrior, Mage, Healer) with 4 branches × 3 tiers each. Skill point economy on level-up, prerequisite enforcement, damage/range/AoE/heal bonus aggregation, passive stat bonuses. `UpgradeSkill` and `ToggleSkillTree` actions.
  - Battle preview (`battle_preview.rs`): Damage preview (ATK/DEF/level/elemental formula, min/max/expected, hit/crit chance, can-kill flag), AoE preview (Circle/Square/Cross/Line/Cone shapes with entity classification), turn order display (SPD-descending), enemy intent prediction (archetype-based).
  - Hazard terrain: 6 additional tile types in `map.rs` with walkability, movement cost, and ASCII mapping support.
- **Terminal VFX demo** (`prototype/vfx-demo`): interactive demo proving real-time terminal animation at 30 FPS. Particle system (fire, ice, lightning, slash, burst, heal, AoE), screen shake, flash overlays, floating damage text, expanding ring indicators, diff-based rendering. Loads PNG character sprites (Kael, Mira, Blight Sovereign) from `wuthering-terminal/assets/` via `image_to_grid()` with chroma-key transparency. Run with `cargo run -p vfx-demo`.

**Key architectural invariant:** all gameplay paths (terminal input, scripted commands, tests, replays, agent injection) converge on the same `Action` enum and `apply_action()` function. Do not split this path.

## Tactical RPG Direction

The next major prototype direction is a **turn-based tactical RPG** on a grid
battlefield. The VFX demo
(`prototype/vfx-demo`) proved that terminal-based animation (particles, screen
shake, flash, floating text, AoE rings) feels satisfying at 30 FPS with
diff-based rendering.

### Design Decisions (2026-05-22)

Three concerns were raised about using Wuthering Waves IP directly:

1. **Image model accuracy** — AI image generators cannot produce accurate WuWa
   characters. The game launched mid-2024 and is niche enough that training data
   is thin. Generated sprites look generic, not like actual characters.
2. **LLM knowledge** — Models have limited coverage of WuWa's specific systems
   (Forte, Concert, Echo absorption). Implementation would be more invention
   than adaptation.
3. **Legal risk** — Using copyrighted character names, designs, and trademarked
   mechanics from Kuro Games is a liability for a public GitHub repo, even for
   non-commercial use.

**Resolution:** Keep WuWa-inspired **mechanics** (team swap QTE, telegraphed
parry/dodge, absorption system) but apply them to **original characters** using
universally recognized RPG archetypes that every image model and LLM knows
perfectly:

| Archetype | Visual | Character | Why it works |
|-----------|--------|-----------|-------------|
| Warrior | Sword + heavy armor | Kael | Every image model produces crisp pixel art |
| Mage | Staff + robes + elemental effects | Lyra | Universally recognized silhouette |
| Healer/Cleric | White robes + holy light | Mira | Universally recognized |
| Boss: Dark Knight | Dark armor + greatsword | Blight Sovereign | Classic villain archetype |

This gives us: zero legal risk, accurate sprite generation, deep LLM knowledge
of mechanics, and the same gameplay depth.

### VFX Capabilities (proven in demo)

The VFX demo proves these terminal-native effects work:

- **Particle bursts** — colored glyphs (`*`, `·`, `✦`, `░`, `▓`) spawning from
  a point, drifting with velocity, fading over lifetime with color decay.
- **Screen shake** — sinusoidal viewport offset with intensity decay over time.
- **Flash overlay** — full-screen or region-based color blending with alpha decay.
- **Floating damage text** — rises upward, fades from bright to gone.
- **AoE ring indicators** — expanding circles drawn with `draw_circle`.
- **Elemental effects** — color-coded: fire=red/orange, ice=blue/white,
  lightning=yellow/cyan, heal=green, slash=white.
- **Combo system** — escalating damage and visual intensity per successive hit.
- **Real-time loop** — `poll_event` + `thread::sleep` at 30 FPS, with
  diff-based rendering for efficient terminal I/O.

### Implementation Roadmap

Build order for the tactical RPG prototype:

1. **Tactical grid scene** — grid-based battlefield, tile rendering, character
   placement, cursor movement. *(Complete)*
2. **Turn system** — player phase → enemy phase, action points per character.
   *(Complete)*
3. **Basic combat** — attack ranges, damage calculation, HP bars (VFX already
   proven). *(Complete)*
4. **Team swap QTE** — swap between Kael, Lyra, and Mira mid-turn, cooldown
   timer. *(Complete)*
5. **Telegraphed attacks** — enemy shows attack zones (colored tiles), player
   can dodge/parry. *(Complete)*
6. **Echo absorption** — defeated enemies drop abilities the player can absorb.
   *(Complete)*
7. **Sprite pipeline** — PNG → Rust const arrays via `scratch/png_to_ansi.py`,
   adaptive resolution. *(Complete)*
8. **Boss fight** — Blight Sovereign with multi-phase patterns. *(Complete)*

The VFX system has been extracted into `verryte-terminal::vfx` and is reusable
across prototypes.

## Verification

Normal workspace verification:

```sh
cargo fmt --check
cargo test
```

Useful Wuthering Terminal smoke commands:

```sh
cargo run -p wuthering-terminal --bin wuthering-terminal-script -- "inspect:4,4 confirm inspect:4,5 confirm"
cargo run -p wuthering-terminal --bin wuthering-terminal
```

The script runner executes a sequence of action tokens for smoke testing. The TTY runner needs a real terminal.

If a Rust toolchain is unavailable or a command cannot be run in the current
environment, say so in the final response and record the limitation in the
worklog for non-trivial work.

## Documentation

Keep these docs aligned when their subject changes:

- Root [README.md](README.md) for workspace capabilities and common commands.
- [prototype/wuthering-terminal/README.md](prototype/wuthering-terminal/README.md) for the
  tactical RPG prototype and its current scope.
- [GOAL.md](GOAL.md) only when the project direction itself changes.
- Prompt files under [prompt/](prompt/) only when reusable agent instructions
  need to change.

## Committing

After successfully completing a task, updating the documentation, and verifying the workspace, you must commit your changes. Only commit once the job is complete and all tests pass.
- Use `git status` and `git diff HEAD` to review your work.
- Stage the specific files you modified or created using `git add <file>`.
- Use `git log -n 3` to match the project's commit message style.
- Create a concise commit message explaining the "why" of the changes.
- Do not push to a remote repository unless explicitly asked.

## Worklog

After finishing a non-trivial request, **append** a dated entry to the end of
[WORKLOG.md](WORKLOG.md) at the repo root. Entries are chronological: oldest
first, newest at bottom.

**Always append via a bash heredoc. Never edit or rewrite `WORKLOG.md` with
patch/edit tools.** This keeps earlier handoff notes byte-for-byte intact.

The worklog captures what is not recoverable from `git diff` or `git log`:
reasoning, rejected alternatives, assumptions, constraints, gotchas, and
follow-ups. Prefer too much context over too little.

Every non-trivial entry should usually cover:

1. **Goal** - restate what the user asked for.
2. **Changes** - concrete edits with `path:line` references where useful.
3. **Reasoning** - why this approach, alternatives rejected, and trade-offs.
4. **Assumptions** - what you took as given that a future agent might challenge.
5. **Gotchas** - subtle findings, footguns, or things that nearly broke.
6. **Follow-ups** - what remains or what should be verified later.

Use this exact append style:

```bash
cat >> WORKLOG.md <<'EOF'

## YYYY-MM-DD - one-line summary

**Goal.** Restate the request in 1-2 sentences.

**Changes.**
- `path/to/file.rs:42` - what changed and why it matters.
- `README.md` - documentation updated to match behavior.

**Reasoning.** Explain why this shape of solution fits Verryte. Mention
alternatives considered, such as putting behavior in Ash Courier only versus
moving a reusable primitive into an engine crate.

**Assumptions.** List any assumptions that are not obvious from the diff.

**Gotchas.** Capture non-obvious findings, such as script smoke tests only
passing on a win, TTY behavior requiring a real terminal, or a shared input path
that must not fork into test-only logic.

**Follow-ups.** Note what should be done or verified next.
EOF
```

Use the literal quoted delimiter `'EOF'` so backticks and `$` are not expanded.
Skip the worklog for genuinely trivial edits such as typo fixes or a single
user-dictated config line. For everything else, leave enough context that the
next agent does not need to re-derive your reasoning.
