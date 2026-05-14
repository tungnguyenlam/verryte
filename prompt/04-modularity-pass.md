# Prompt: Modularity Pass

Read `GOAL.md` and inspect the current code.

Improve Verryte's modularity without making the project abstract for its own sake.
The engine should be assembled from focused parts, with a pleasant facade for common
usage and accessible internals for unusual games.

Look for one concrete improvement:

- separate a boundary that is already becoming tangled
- introduce a small trait or data type that enables extension
- move terminal-specific logic out of core game logic
- make a sample/example depend on public APIs instead of internals
- reduce coupling between input, state, rendering, and loop code

Rules:

- do not split modules just to create folders
- do not add plugin systems before there is something useful to plug in
- keep changes small enough to review
- preserve existing behavior
- add tests or compile checks where practical

End by explaining what boundary became clearer and what should remain flexible.

