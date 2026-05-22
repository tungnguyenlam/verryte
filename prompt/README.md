# Verryte Prompt Kit

Reusable prompts for continuing Verryte development across terminal coding harnesses.

Each file is designed to be copied into a fresh agent session. Start with
`00-project-context.md` when the agent has no context, then choose the prompt that matches
the kind of work you want next.

The `prototype/wuthering-terminal/` folder describes the proving game. Agents should use
that prototype to validate engine behavior, especially the shared input/control path.

## Suggested Loop

1. Paste `00-project-context.md`.
2. Paste one task prompt, such as `02-implement-next-slice.md`.
3. Let the harness inspect the repo and make a small, tested change.
4. Paste `06-review-and-harden.md` before moving on.
5. Repeat with a narrower prompt as the codebase grows.

## Prompts

- `00-project-context.md` - baseline orientation for any harness
- `01-architecture-pass.md` - reason about structure before implementation
- `02-implement-next-slice.md` - pick and build the next useful vertical slice
- `03-input-control-contract.md` - protect the core input and agent-control model
- `04-modularity-pass.md` - keep crates/modules extensible and composable
- `05-testing-and-harness.md` - add or improve automated checks
- `06-review-and-harden.md` - review recent work for bugs and missing coverage
- `07-docs-sync.md` - keep docs aligned with the current implementation
- `08-fresh-session-bootstrap.md` - compact prompt for quick repeated use
- `09-autonomous-engine-run.md` - longer autonomous run prompt for sustained progress
- `10-tactical-rpg.md` - autonomous run for the tactical RPG prototype: VFX extraction, grid battlefield, turn system, combat, team swap, boss fight
