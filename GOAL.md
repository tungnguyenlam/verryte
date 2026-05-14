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

The engine is built around the idea that a terminal cell is the basic visual unit. Every part of the design should respect that constraint and turn it into an advantage: readable grids, layered presentation, clear spatial reasoning, and compact state.

Game behavior should be expressed through ordinary Rust data and systems. The engine can provide structure, scheduling, storage, and conventions, but the user's game should remain visible and debuggable.

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

Every Verryte game should be observable and controllable outside the interactive TUI.

The finished engine should make it natural to reset a game, inspect state, inject actions, enter text, run tests, and reproduce behavior from scripts or CI. This is not a separate testing mode; it is part of the engine's design. A game that can be played by a person should also be understandable to tools.

The exact commands can evolve, but the capability should remain clear: a tool should be able to start from a known state, apply input step by step, and receive structured state back after meaningful changes.

---

## Design Principles

1. **Terminal first** - embrace cells, glyphs, color, and constrained space
2. **Data first** - keep game state explicit, inspectable, and efficient
3. **Composable parts** - prefer focused modules that work together cleanly
4. **Extensible defaults** - provide useful behavior without closing escape hatches
5. **No hidden ownership** - the engine supports the game; it does not swallow it
6. **Agent-ready** - games should be scriptable, testable, and reproducible
7. **Rust-native** - APIs should feel natural, honest, and boring in the best way

---

## Boundaries

Verryte is not a general-purpose game engine.

It does not aim to support 3D, GPU rendering, GUI windows, or a broad application framework. It should stay focused on terminal-native 2D games and the tools needed to build, run, test, and extend them well.
