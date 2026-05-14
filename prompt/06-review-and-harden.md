# Prompt: Review and Harden

Read `GOAL.md`, inspect recent changes, and review the code like a senior engineer.

Prioritize:

- bugs
- broken or unclear public contracts
- behavior that drifts from the goal doc
- prototype code that has leaked one-off game assumptions into reusable engine APIs
- input/control paths that split interactive and scripted behavior
- stale comments, docs, or prior-agent assumptions that no longer match the code
- missing tests around changed behavior
- unnecessary coupling or premature abstraction
- error handling that will make terminal games hard to debug

If you find issues, fix the highest-value ones that are safe and local. If a concern is
real but too large for this pass, document it clearly as a follow-up.

Self-healing rule: do not preserve a bad pattern just because it already exists. Confirm
the intended behavior from `GOAL.md`, the Ash Courier prototype README, tests, and the
current code. If those sources conflict, make the smallest correction that restores a
coherent direction and leave a clear note for the next agent.

After changes:

- run relevant tests/checks
- summarize findings first
- summarize fixes second
- note remaining risk
