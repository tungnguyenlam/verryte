# Prompt: Implement Next Slice

Read `GOAL.md`, inspect the repo, and continue Verryte by implementing the smallest
useful vertical slice.

Pick work that moves the engine toward its final shape:

- ECS/data model foundations
- input-to-action plumbing
- observable state snapshots
- terminal rendering primitives
- game loop scaffolding
- modular plugin or extension points
- Ash Courier prototype work that proves the engine shape without hard-coding the engine
  around one game
- tests that make future changes safer

Before editing, state the slice you chose and why. Then implement it.

Implementation constraints:

- keep the change narrow and real
- prefer ordinary Rust APIs over magic
- keep user-facing APIs inspectable
- keep terminal-specific concerns explicit
- do not build speculative systems that no code uses
- add tests for behavior, especially input/control behavior
- verify prior assumptions against the current code; fix stale or incorrect previous
  agent work before layering new work on top

After implementation:

- run the most relevant checks available
- summarize files changed
- mention any remaining gap or next obvious slice
