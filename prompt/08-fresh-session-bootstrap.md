# Prompt: Fresh Session Bootstrap

Read `GOAL.md`. Then inspect the repo and continue Verryte development.

Keep these priorities in mind:

- modular Rust engine for terminal games
- ECS-oriented, data-first, terminal-native
- extensible defaults, not hard limits
- no hidden ownership of the user's game
- Wuthering Terminal in `prototype/wuthering-terminal/` is the proving game
- input/control is central
- interactive input and scripted/agent input must share the same action path
- every meaningful slice should become testable
- previous-agent work is useful but fallible; verify assumptions before building on them

Do the smallest useful next thing. Prefer a working vertical slice over speculative
architecture. Preserve unrelated user changes. Add or update tests/docs when behavior
changes. If existing code or docs are stale, wrong, or blocking progress, self-heal them
with the smallest coherent correction and leave the next agent with fewer traps. End with
a concise summary and verification result.
