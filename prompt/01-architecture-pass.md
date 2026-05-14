# Prompt: Architecture Pass

Read `GOAL.md` and inspect the current repository structure.

Reason about the architecture Verryte needs next. Do not over-design the whole engine.
Your job is to identify the next structural decision that will make future development
easier while keeping the project modular and extensible.

Focus on:

- clean boundaries between core engine behavior, terminal rendering, input/control, maps,
  assets, tooling, and tests
- APIs that let games define their own data, systems, actions, states, and rendering
  behavior
- avoiding a monolithic framework that swallows user code
- keeping the input and agent-control path central

Deliverable:

- propose the smallest architecture slice worth implementing now
- explain why it helps
- implement it if it is safe and local
- add or update tests/docs as appropriate

Avoid:

- locking in detailed APIs too early
- adding crates/modules with no immediate use
- making the engine depend on a single sample game shape

