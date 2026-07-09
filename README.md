# bide

A deterministic workflow engine that consults specialized agents when it needs
judgement.

*bide* means "path" in Basque — and that is what it is: an engine that drives a
task down an explicit path of states. The AI reasons; Rust controls.

```
Created → ContextReady → PlanReviewed → ChangesProduced → ChecksCompleted → Accepted
```

The core is deterministic Rust. Agents (Planner, Critic, Fix Planner, Reviewer)
produce analysis, plans and reviews — they do not control the system. The
Workflow Engine executes steps, applies retry limits and records artifacts.

See [docs/architecture.md](docs/architecture.md) for the full design.

## Development

Built test-first (strict TDD), minimal code, no speculative modules.

```sh
just test    # cargo test
just lint    # cargo clippy -D warnings
just check   # fmt + lint + test
```

## Status

Early. Implemented: the state machine (`WorkflowState`, `StepOutcome`, `Task`)
with its retry branch. Everything else grows one tested module at a time toward
the tree in `docs/architecture.md`.
