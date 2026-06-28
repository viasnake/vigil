# AGENTS.md

## Primary Contract

`docs/goal.md` is the single authoritative product and implementation contract for this repository.

Do not duplicate the goal, scope, roadmap, architecture, provider policy, safety boundary, milestone list, or acceptance criteria from `docs/goal.md` into this file.

When product or implementation intent is unclear, read `docs/goal.md` first.

## Project Memory

Maintain `docs/implementation-notes.md` while working.

Use it to record important knowledge discovered during implementation, including:

* current implementation status,
* relevant technical findings,
* important constraints discovered from code or tests,
* validation results,
* known limitations,
* unresolved questions,
* decisions made during implementation that are not already stated in `docs/goal.md`.

Do not use `docs/implementation-notes.md` to restate `docs/goal.md`.

Keep notes concise, factual, and current.

If information in `docs/implementation-notes.md` conflicts with `docs/goal.md`, `docs/goal.md` wins. Resolve the conflict immediately by updating or removing the conflicting note.

If implementation reality shows that `docs/goal.md` is wrong or impossible, stop and explain the conflict instead of silently changing the project direction.

## No Contradictions or Duplicates

Do not create overlapping project documents with competing definitions.

Do not add new goal, roadmap, architecture, provider, runtime, safety, or milestone documents unless explicitly requested.

Do not duplicate the same rule in multiple places.

When adding documentation, prefer one authoritative location and link to it.

## Implementation Rules

Follow `docs/goal.md` until the 1.0 acceptance criteria are met.

Do not implement features excluded by `docs/goal.md`.

Do not add unsupported LLM providers.

Do not add ChatGPT or Codex subscription reuse.

Do not add command execution, SSH execution, target-host runners, MCP, autonomous remediation, or production mutation.

Prefer explicit Rust data models.

Prefer deterministic validation over trusting LLM output.

Keep provider abstraction minimal.

Keep user-facing errors actionable.

Never commit secrets.

## Documentation Rules

`docs/goal.md` may describe the intended 1.0 target.

User-facing documentation must describe implemented behavior only.

Do not document planned behavior as supported.

When implementation changes user-visible behavior, update the relevant user-facing documentation.

When implementation reveals important project knowledge, update `docs/implementation-notes.md`.

## Validation

Before finishing a work session, run:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

If a command cannot be run, record the reason in the final response and, when relevant, in `docs/implementation-notes.md`.

## Completion Standard

Work is complete only when:

* the requested goal has been implemented or the remaining blocker is clearly explained,
* relevant tests exist,
* required checks pass or failures are explained,
* `docs/implementation-notes.md` is up to date,
* no duplicated or contradictory project documentation was introduced,
* the final response summarizes changed files, validation results, and remaining risks.
