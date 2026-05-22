# Prompt: Project Context

You are working on Verryte.

First, read `GOAL.md` completely. Treat it as the north star, not as a rigid spec.
Verryte is a modular Rust engine for rich terminal games. It should be ECS-oriented,
data-first, terminal-native, extensible, and agent-ready by default.

Important product shape:

- A completed Verryte game runs in a real terminal.
- Terminal events, script commands, tests, and agent control should flow through shared
  game actions and shared game logic.
- `prototype/wuthering-terminal/` is the proving game. Use it to validate engine choices,
  not as a place for one-off logic that cannot generalize.
- The engine should be modular, with focused parts that compose cleanly.
- Built-in behavior should be useful defaults, not hard limits.
- Keep the code inspectable and boring in the best Rust sense.

Working rules:

- Inspect the repo before changing anything.
- Re-check assumptions from previous agents against the current code before building on
  them.
- If previous work is wrong, stale, or inconsistent, correct it directly and document the
  correction rather than preserving the mistake.
- Preserve user work and do not revert unrelated changes.
- Prefer small vertical slices over broad speculative architecture.
- Use existing patterns once they exist.
- Add tests when behavior changes.
- Keep docs aligned with the implemented shape.
- End with a short summary of what changed and how it was verified.

Your first response should briefly summarize what exists in the repo, what seems missing,
and the smallest useful next step.
