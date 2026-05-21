# Verryte Goal

Verryte is a modular Rust engine for building rich terminal games.

Its final form is a small but capable foundation for games that belong in the terminal: roguelikes, tactics games, simulations, interactive fiction, and strange hybrids that treat the terminal as a real creative medium rather than a fallback display.

The engine should feel direct, inspectable, and easy to extend. A game built with Verryte should be able to start simple, grow in complexity, and remain understandable as new systems, content, tools, and presentation layers are added.

---

## Finished Shape

A completed Verryte game runs in a real terminal, presents a coherent 2D world, accepts keyboard and mouse input, updates through a predictable game loop, and exposes enough of itself to be tested, scripted, debugged, and driven by tools.

The player experience should feel immediate and intentional: crisp cells, expressive glyphs, readable color, responsive input, clear UI, and game state that behaves consistently across interactive play and automated runs.

The developer experience should feel lightweight: bring the engine into `main`, compose the pieces the game needs, define the game's data and systems, and let Verryte handle the terminal concerns around rendering, input, timing, maps, assets, and testability.

---

## Core Direction

Verryte is ECS-oriented, data-first, and terminal-native.

The engine is built around the idea that a terminal cell is the basic visual unit. Every part of the design should respect that constraint and turn it into an advantage: readable grids, layered presentation, clear spatial reasoning, compact state, and visual identity that belongs to the terminal instead of apologizing for it.

Game behavior should be expressed through ordinary Rust data and systems. The engine can provide structure, scheduling, storage, and conventions, but the user's game should remain visible and debuggable.

---

## Visual Direction

Verryte should make terminal games look intentional, not symbolic by default. A player character should not have to be `@`, a wall should not have to be `#`, and a monster should not have to be a single letter unless that is the game's chosen art direction. The engine should support games whose characters, terrain, objects, effects, and UI are recognizable terminal art with shape, color, animation, lighting, and style.

The cornerstone is a data-driven terminal graphics system:

```text
game state -> semantic visual id -> terminal sprite/layer -> cell grid
```

Gameplay remains semantic: an entity is still a courier, gate, hazard, relic, or map tile. Rendering maps that meaning to visual assets. This keeps gameplay, scripts, tests, replays, and agents independent from the exact glyphs used on screen, while still allowing the player-facing TUI to look rich.

Verryte should support several levels of terminal-native presentation:

- **Single-cell glyphs** for compact maps, debug views, logs, tests, and fallback modes.
- **Multi-cell ASCII sprites** for recognizable silhouettes, props, architecture, effects, and title art.
- **Unicode block sprites** using block elements such as half-blocks and full blocks when a game opts into higher-fidelity terminal graphics.
- **TrueColor palettes** for terminals that support 24-bit color, with graceful degradation to simpler palettes when needed.
- **Layered rendering** for terrain, actors, lighting, fog, particles, targeting cursors, inspection overlays, and UI.
- **Animation frames** for movement, smoke, impact, doors, weather, status effects, and other short-lived presentation details.

Image-derived or prompt-derived visuals are allowed, but they should become inspectable assets before runtime. A tool may convert a source image, sprite sheet, sketch, or user description into terminal sprites by using density gradients, edge-aware ASCII, dithering, or Unicode half-block cells where one terminal cell represents a top and bottom color pair. The result should be baked into plain asset data that the engine can load, diff, test, snapshot, and render deterministically.

For example, a high-fidelity block sprite may encode two vertical source pixels into one terminal cell:

```text
top pixel color    -> foreground color
bottom pixel color -> background color
glyph              -> upper half block
```

This is still terminal-native rendering. It is not GPU graphics, a GUI window, or a hidden image layer. The final frame is always a grid of cells with glyphs, foreground colors, background colors, and attributes.

### Adaptive Resolution Sprite System

Verryte supports an adaptive sprite resolution system that scales visual
fidelity to match the user's terminal dimensions. At startup (and on terminal
resize), the engine queries the terminal size in columns and rows and selects
the highest resolution tier that fits:

| Tier   | Sprite Size | Min Terminal |
|--------|------------|--------------|
| TINY   | 6×8        | 60×20        |
| SMALL  | 8×12       | 80×24        |
| MEDIUM | 12×16      | 100×30       |
| LARGE  | 16×20      | 120×36       |
| XLARGE | 20×24      | 140×42       |
| ULTRA  | 28×32      | 160×48       |

Tier selection is based entirely on terminal columns and rows, not monitor
resolution or pixel density. A maximized terminal on a small laptop and a
small floating window on a 4K monitor may select different tiers despite
running on the same display hardware.

Sprite assets are compiled at build time into static Rust arrays at all
resolution tiers. At runtime, the engine selects the appropriate tier with
zero image processing or file I/O. The build-time pipeline converts source
PNG artwork into `[[(u8, u8, u8); W]; H]` arrays using half-block sub-pixel
packing, with white and transparent pixels keyed to the background color.

Games can also respond to terminal resize events by re-selecting the tier
mid-session, allowing the player to resize their window and immediately see
the visual fidelity adjust.

The visual system should stay honest about terminal constraints. It should provide fallbacks for plain ASCII, low-color terminals, small terminal sizes, and test output. Rich rendering should improve the player experience without making the game state opaque or splitting visual behavior away from the shared engine model.

---

## Modularity

Verryte should be assembled from focused parts rather than delivered as one indivisible framework.

Core engine behavior, terminal rendering, input, maps, assets, audio, tooling, and testing support should be separable enough that games can adopt what they need and ignore what they do not. The facade should make the common path pleasant, while the internals remain accessible for games that need unusual control.

Modules should communicate through clear data boundaries. Features should compose without requiring a game to accept a large preset architecture.

---

## Extensibility

Verryte should invite extension at the places where terminal games naturally differ.

Games should be able to define their own components, resources, systems, maps, actions, rendering layers, asset formats, game states, plugins, and test hooks. Built-in pieces should be useful defaults, not hard limits.

The engine should make common patterns easy while leaving enough room for experiments: custom field-of-view, alternate input schemes, nonstandard UI, generated worlds, simulation-heavy systems, content pipelines, and agent-driven play.

---

## Input and Control

Input is one of Verryte's central promises.

A finished game should not have to treat terminal events, player commands, test input, and agent control as separate worlds. Keyboard and mouse events should be captured reliably, translated into named game actions, and passed through the same game logic that automated tools can drive.

The engine should support both direct interactive play and discrete command injection. Turn-based games should be able to queue input between ticks without dropping intent. Real-time games should be able to sample input predictably. Games should be able to define their own action maps, text prompts, menus, mouse behavior, and input contexts without rewriting the terminal plumbing.

The important shape is simple:

```text
terminal event -> game action -> game system -> observable state
script command -> game action -> game system -> observable state
```

That shared path is what keeps play, debugging, testing, replays, and agent control from drifting apart.

---

## Agent-Ready by Default

Every Verryte game should be observable and controllable outside the interactive TUI. This is not a separate testing mode; it is part of the engine's design. A game that can be played by a person should also be understandable to tools.

The shared control path makes this possible:

```text
terminal event -> game action -> game system -> observable state
script command -> game action -> game system -> observable state
agent command -> game action -> game system -> observable state
```

All input sources converge into the same action queue, pass through the same game logic, and produce the same observable state. No source gets a privileged or degraded path.

### Runners

A finished Verryte game should ship with two runners:

**Interactive TUI** - a terminal frontend that renders the game to a real terminal, handles keyboard and mouse input, and presents the game's visual output through incremental cell diffs. This is the player-facing runner.

**Script / CI runner** - a non-interactive runner that accepts a script of commands, applies them step by step, and exits with a pass/fail result. This runner requires no terminal, produces plain-text output suitable for logs, and is the primary smoke-test and regression tool. It should be usable from CI with no special setup.

Both runners drive the same game logic. A script that wins in the CI runner should win identically in the interactive TUI.

The command-line interface for these runners should stay simple. A user or tool invokes the runner with straightforward arguments — a script string, a seed, a layout flag — and the engine handles all parsing, execution, state management, and output formatting internally. Complex logic lives in the engine, not in the command that starts it.

### Observability

A game's state should be fully observable at any point during execution. The engine should provide:

- **Step reports** - after each action, a report containing the action taken, its source, the result, whether the turn advanced, and before/after snapshots of changed state.
- **Snapshots** - a complete picture of observable game state at a moment in time: positions, inventories, map state, visibility, outcomes, scores, and any game-specific data the game chooses to expose.
- **Action provenance** - every action should carry metadata indicating its source (terminal, script, agent, replay, test) so that tools can distinguish human play from automated runs without affecting game behavior.

Observability is not an afterthought. Game systems should be designed so that meaningful state is accessible to tests, scripts, and tools without requiring internal access.

### Replay

The engine should support recording and replaying sessions. A sequence of sourced actions should be capturable as a trace, serializable, and replayable through the same action queue that handles live input. Replay should reproduce identical game state when given the same initial conditions and RNG seed.

Replay serves debugging (reproduce a failure), testing (assert on recorded sessions), and demonstration (show how a game was played).

### Agent Control

A tool should be able to start a game from a known state, inject actions step by step, inspect the resulting state, and reset for another attempt. The exact protocol can evolve, but the capability should remain clear:

- **Reset** - return the game to its initial state, reusing the same structures.
- **Inject** - place actions into the queue with an agent source tag.
- **Observe** - read structured state after each step or at any point.
- **Batch** - drain and apply all pending actions, receiving a report for each.

This enables agents, bots, and external tools to drive games through the same path as human players, without requiring Rust-level access to the engine internals.

---

## Design Principles

1. **Terminal first** - embrace cells, glyphs, color, constrained space, and terminal-native visual style
2. **Expressive by default** - support recognizable sprites, palettes, layers, animation, and fallbacks instead of forcing symbolic placeholders
3. **Data first** - keep game state, visual mappings, and assets explicit, inspectable, and efficient
4. **Composable parts** - prefer focused modules that work together cleanly
5. **Extensible defaults** - provide useful behavior without closing escape hatches
6. **No hidden ownership** - the engine supports the game; it does not swallow it
7. **Agent-ready** - games should be scriptable, testable, and reproducible
8. **Rust-native** - APIs should feel natural, honest, and boring in the best way

---

## Boundaries

Verryte is not a general-purpose game engine.

It does not aim to support 3D, GPU rendering, GUI windows, or a broad application framework. It should stay focused on terminal-native 2D games and the tools needed to build, run, test, and extend them well.
