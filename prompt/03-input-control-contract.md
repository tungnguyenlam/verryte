# Prompt: Input and Control Contract

Read `GOAL.md`, then focus specifically on Verryte's input and control model.

This is one of the most important parts of the engine. Do not let interactive play,
tests, scripts, and agent control become separate systems.

The intended shape is:

```text
terminal event -> game action -> game system -> observable state
script command -> game action -> game system -> observable state
```

Your task:

- inspect the current input/control code, or create the first minimal version if none
  exists
- ensure raw input can become named game actions
- ensure scripted or CLI-style input can inject the same named actions
- ensure state after meaningful changes can be inspected in a structured way
- add tests that prove interactive-style and script-style input share the same path

Keep it modular:

- games should define their own actions and mappings
- games should support text prompts, menus, mouse behavior, and input contexts over time
- turn-based games should be able to queue input without dropping intent
- real-time games should be able to sample input predictably

Avoid hard-coding one game's controls into the engine. Build the smallest reusable
contract that makes the shared path real.

