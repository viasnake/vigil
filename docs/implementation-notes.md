# Implementation Notes

Concise implementation knowledge discovered while building Vigil.

## Current Status

* Rust workspace is implemented with the requested crate split: CLI, config, core workflow, LLM provider, shared models, and rendering.
* `docs/goal.md` is the single goal contract; the previous root-level `goal.md` duplicate was moved there.
* Minimal examples exist under `examples/minimal/` and work with deterministic `--no-llm` investigation.
* Required user-facing docs are present.
* CI workflow now exists at `.github/workflows/ci.yml`; before this readiness pass, implementation notes overstated CI readiness.

## Technical Findings

* Cloudflare AI Gateway integration uses the current REST chat-completions endpoint at `api.cloudflare.com/client/v4/accounts/{account_id}/ai/v1/chat/completions` with the `cf-aig-gateway-id` header.
* `ReasoningResult` is schema-validated and then semantically checked for non-empty required content, confidence ranges, read-only recommendations, and obvious runnable shell-command text in recommended checks.
* The core workflow builds a redacted `EvidencePacket` before any LLM call. Redaction masks common secret-like field names and some token-like values, but remains best-effort.
* Tests avoid real Cloudflare credentials by using deterministic `--no-llm` mode and a mock provider.
* CLI smoke tests cover `vigil version`, `vigil validate` with `examples/minimal`, `vigil investigate` with `examples/minimal --no-llm`, and rendering from a saved trajectory.
* Markdown rendering has a golden-style fixture at `crates/vigil-render/tests/fixtures/evidence_brief.md`.

## Validation Notes

* `cargo fmt --check` passed after the CI, docs, smoke-test, and renderer fixture updates.
* `cargo clippy --workspace --all-targets -- -D warnings` passed after the CI, docs, smoke-test, and renderer fixture updates.
* `cargo test --workspace` passed after the CI, docs, smoke-test, and renderer fixture updates.
* Direct smoke run passed: `cargo run -q -p vigil-cli -- investigate --alert examples/minimal/alert.yaml --inventory examples/minimal/inventory.yaml --runbook-dir examples/minimal/runbooks --output <tmp>/brief.md --json-output <tmp>/brief.json --trajectory-output <tmp>/trajectory.json --no-llm`.

## Known Limitations

* Real Cloudflare requests were not executed in this environment because no credentials were provided.
* Redaction is intentionally basic and cannot guarantee perfect secret detection.
* File inputs cover alert, inventory, and runbook evidence; there are no log, metric, change, ticketing, or monitoring adapters.

## Open Questions

* None.
