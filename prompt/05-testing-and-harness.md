# Prompt: Testing and Harness

Read `GOAL.md` and inspect the repository.

Improve Verryte's automated development story. The goal is that a game can be tested,
scripted, debugged, and eventually driven by agents without needing a live TUI.

Choose the smallest useful testing/harness improvement:

- add unit tests for existing behavior
- add an integration test for input-to-action flow
- add a tiny scripted run of an example game
- add state snapshot serialization for a narrow piece of state
- add a smoke test command
- document how a harness should drive the game

Important:

- tests should exercise public or near-public behavior where possible
- agent/script control should share logic with normal gameplay
- avoid building a fake parallel implementation just for tests
- keep output structured when state is inspected

Run the relevant checks and report what passed.

