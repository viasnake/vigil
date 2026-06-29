# Implementation Notes

Concise implementation knowledge discovered while building Vigil.

## Current Status

* Rust workspace is implemented with the requested crate split: CLI, config, core workflow, LLM provider, shared models, and rendering.
* `docs/goal.md` is the single goal contract; the previous root-level `goal.md` duplicate was moved there.
* Minimal examples exist under `examples/minimal/` and still work with deterministic `--no-llm` file-based investigation.
* Case-based investigation is now implemented as the primary UX: `case init`, `evidence add`, `change add`, `runbook add`, and `investigate <case-dir>`.
* Required user-facing docs are present.
* CI workflow now exists at `.github/workflows/ci.yml`; before this readiness pass, implementation notes overstated CI readiness.

## Technical Findings

* Cloudflare AI Gateway integration uses the current REST chat-completions endpoint at `api.cloudflare.com/client/v4/accounts/{account_id}/ai/v1/chat/completions` with the `cf-aig-gateway-id` header.
* `ReasoningResult` is schema-validated and then semantically checked for non-empty required content, confidence ranges, read-only recommendations, and obvious runnable shell-command text in recommended checks.
* The core workflow builds a redacted `EvidencePacket` before any LLM call. Redaction masks common secret-like field names and some token-like values, but remains best-effort.
* Tests avoid real Cloudflare credentials by using deterministic `--no-llm` mode and a mock provider.
* CLI smoke tests cover `vigil version`, `vigil validate` with `examples/minimal`, `vigil investigate` with `examples/minimal --no-llm`, and rendering from a saved trajectory.
* Markdown rendering has a golden-style fixture at `crates/vigil-render/tests/fixtures/evidence_brief.md`.
* Case evidence intake writes existing `Evidence` model YAML under `<case>/evidence/`; change intake uses `kind: change`.
* Case investigation reads `<case>/vigil.yaml`, `<case>/evidence/`, and `<case>/runbooks/`, then writes default outputs under `<case>/output/`.
* `vigil investigate <case-dir>` rejects case directories combined with file-mode flags such as `--inventory`.

## Validation Notes

* `cargo fmt --check` passed after the case UX updates and temporary Cloudflare validation notes.
* `cargo clippy --workspace --all-targets -- -D warnings` passed after the case UX updates and temporary Cloudflare validation notes.
* `cargo test --workspace` passed after the case UX updates and temporary Cloudflare validation notes.
* Manual case flow passed with `/tmp/web-5xx`: `case init`, `evidence add --kind metric --url`, `change add --url`, `runbook add`, and `investigate /tmp/web-5xx --no-llm`.
* Manual log evidence intake passed with `evidence add --kind log --file /tmp/timeout-snippet.txt`.
* Manual file-based compatibility run passed with `examples/minimal` and explicit Markdown, JSON, and trajectory outputs.
* Manual ambiguous-input check passed: `vigil investigate /tmp/web-5xx --inventory examples/minimal/inventory.yaml --no-llm` failed with an actionable ambiguity error.
* Temporary live Cloudflare validation on 2026-06-29 did not produce an LLM response: `vigil investigate /tmp/web-5xx` against the REST endpoint returned HTTP 401, and a direct minimal REST request returned the same authentication error.
* A provider-native `gateway.ai.cloudflare.com` diagnostic using Gateway authentication reached Cloudflare AI Gateway but returned HTTP 402 `Insufficient wholesale credits`; live end-to-end LLM output remains unvalidated.

## Known Limitations

* Real Cloudflare requests were attempted with temporary credentials, but no successful LLM response was received because the REST request failed authentication and the provider-native diagnostic hit account credits.
* Redaction is intentionally basic and cannot guarantee perfect secret detection.
* File inputs cover alert, inventory, and runbook evidence; there are no log, metric, change, ticketing, or monitoring adapters.

## Open Questions

* None.
