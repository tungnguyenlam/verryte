# Prompt: Tactical RPG Prototype

You are an autonomous software engineer building the next Verryte prototype: a
turn-based tactical RPG on a grid battlefield, using original characters with
classic RPG archetypes.

Read these before starting:

- [GOAL.md](../../GOAL.md) — engine direction and boundaries.
- [AGENTS.md](../../AGENTS.md) — workspace map, engineering priorities, tactical
  RPG direction, VFX capabilities, and implementation roadmap.
- [WORKLOG.md](../../WORKLOG.md) — recent decisions and handoff notes.

## Context

The Verryte engine is mature enough for a complex prototype. The VFX demo
(`prototype/vfx-demo`) proved that terminal-based animation (particles, screen
shake, flash overlays, floating damage text, AoE rings) feels satisfying at 30
FPS with diff-based rendering.

The tactical RPG prototype lives in `prototype/wuthering-terminal/`. Its purpose
is to validate the engine on complex mechanics that Ash Courier cannot stress:
multi-character teams, real-time VFX overlays, area-of-effect targeting, and
turn-phase scheduling.

## Critical Control Shape

All gameplay paths must converge on the same action system. Do not split
interactive, scripted, test, or agent control into separate paths.

```text
terminal event -> game action -> game system -> observable state
script command -> game action -> game system -> observable state
```

## Characters

Use original characters with classic RPG archetypes (zero legal risk, every
image model and LLM knows them):

| Character | Role | Visual | Sprite source |
|-----------|------|--------|---------------|
| Kael | DPS / Tank | Silver-white hair, dark armor, blue glow greatsword | `assets/kael.png` |
| Lyra | Ranged DPS | Dark robes, purple accents, arcane circles | `assets/lyra.png` |
| Mira | Healer / Support | White and gold robes, green healing glow | `assets/mira.png` |
| Blight Sovereign | Boss | Dark armor, red-black corruption aura, horns | `assets/blight-sovereign.png` |

Sprites are 1024×1024 JPEGs (saved as `.png`). Use `image::io::Reader` with
`with_guessed_format()` for content-based detection. Resize to square dimensions
for half-block rendering (e.g. 12×12 → 12×6 terminal cells). Chroma-key
near-white pixels to transparent.

## VFX System

The VFX demo (`prototype/vfx-demo/src/main.rs`) contains a working VFX system:
particle emitters, screen shake, flash overlays, floating text, AoE rings. Before
building the tactical prototype, extract the reusable VFX primitives into either
`verryte-terminal` or a new `verryte-vfx` crate. Keep game-specific effect
presets (fire, ice, lightning) in the prototype.

## Implementation Roadmap

Build in this order. Each step should be a working vertical slice with tests
where practical.

1. **Tactical grid scene** — grid-based battlefield, tile rendering, character
   placement, cursor movement. Use existing `verryte-map` grid and
   `verryte-terminal` rendering primitives.

2. **Turn system** — player phase → enemy phase. Action points per character.
   Phase transitions with visible indicators.

3. **Basic combat** — attack ranges, damage calculation, HP bars, hit-flash via
   VFX. Characters have stats (HP, ATK, DEF, SPD).

4. **Team swap QTE** — swap between 2-3 characters mid-turn. Cooldown timer.
   Swap triggers a brief VFX burst.

5. **Telegraphed attacks** — enemy shows attack zones (colored tiles on the
   grid) before executing. Player can move characters out of danger zones.

6. **Echo absorption** — defeated enemies drop abilities the player can absorb,
   adding new skills to the active character.

7. **Boss fight** — Blight Sovereign with multi-phase patterns. Phase transitions
   trigger screen shake + flash + particle bursts. Telegraphed AoE attacks.

8. **Script runner** — non-interactive binary that runs a sequence of actions
   and reports outcome (win/loss/detailed state). Validates the shared control
   path.

## Autonomy Rules

1. Do not stop after one small task.
2. Do not ask whether to continue.
3. If something fails, debug it and make that failure the next task.
4. Prefer working vertical slices over broad abstract scaffolding.
5. Preserve unrelated user changes.
6. Keep implementation modular — engine primitives go in engine crates,
   game-specific logic stays in the prototype.
7. Update AGENTS.md, README.md, and WORKLOG.md when behavior or project shape
   changes.
8. Add tests for new behavior whenever practical.
9. The VFX demo (`prototype/vfx-demo`) is a standalone reference. Do not break
   it. If you extract VFX primitives, update the demo to use the extracted
   crate.
10. Run `cargo fmt --check` and `cargo test --workspace` after each batch.

## Phase 1: Understand

Before editing:

- read `GOAL.md`, `AGENTS.md`, `WORKLOG.md`
- read `prototype/wuthering-terminal/` source files
- read `prototype/vfx-demo/src/main.rs` for the VFX system to extract
- read `crates/verryte-terminal/src/lib.rs` for rendering primitives
- read `crates/verryte-map/src/lib.rs` for grid/spatial primitives
- run `cargo test --workspace` to confirm baseline

## Phase 2: Plan

Choose 2-3 steps from the roadmap to build in this session. Prioritize:

1. extract VFX system if not done yet
2. tactical grid scene (the foundation everything else depends on)
3. turn system and basic combat

Write a short plan, then execute.

## Phase 3: Execute

Work in vertical slices. Each slice should:

- compile and run
- have tests where practical
- preserve the unified action path
- use engine primitives, not game-specific hacks
- update docs if the shape changes

## Phase 4: Verify

Before finishing:

- `cargo fmt --check`
- `cargo test --workspace`
- confirm `cargo run -p vfx-demo` still works (if VFX extraction happened)
- update WORKLOG.md with what was done and what comes next

## Done Condition

You are done when:

- at least 2 roadmap steps are complete and tested
- the project builds cleanly
- docs and worklog are updated
- the next agent can pick up where you left off

## Final Response

Summarize:

- improvements completed
- files changed
- verification results
- the next roadmap step for the following agent

Begin now with Phase 1.
