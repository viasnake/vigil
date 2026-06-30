# Implementation Notes

Concise implementation knowledge discovered while building Vigil.

## Current Status

* Rust workspace is implemented with the requested crate split: CLI, config, core workflow, LLM provider, shared models, and rendering.
* `docs/goal.md` is the single goal contract; the previous root-level `goal.md` duplicate was moved there.
* Minimal examples exist under `examples/minimal/` and still work with deterministic `--no-llm` file-based investigation.
* Target- and alert-based investigation are now implemented as the primary UX: `investigate service:web --since 30m`, `investigate alert WebHigh5xxRate --since 30m`, and `--plan-only`.
* Case-based investigation remains implemented: `case init`, `evidence add`, `change add`, `runbook add`, and `investigate <case-dir>`.
* Required user-facing docs are present.
* CI workflow now exists at `.github/workflows/ci.yml`; before this readiness pass, implementation notes overstated CI readiness.

## Technical Findings

* Cloudflare AI Gateway integration defaults to the current REST chat-completions endpoint at `api.cloudflare.com/client/v4/accounts/{account_id}/ai/v1/chat/completions` with the `cf-aig-gateway-id` header.
* The Cloudflare provider also supports the documented `gateway.ai.cloudflare.com` provider-native path through `endpoint = "gateway"` / `VIGIL_CLOUDFLARE_ENDPOINT=gateway`; Workers AI `@cf/` models use `workers-ai/v1/chat/completions` with `cf-aig-authorization`.
* `ReasoningResult` is schema-validated and then semantically checked for non-empty required content, confidence ranges, read-only recommendations, and obvious runnable shell-command text in recommended checks.
* The core workflow builds a redacted `EvidencePacket` before any LLM call. Redaction masks common secret-like field names, token-like values, and common inline secret assignments such as `api_token=...`, but remains best-effort.
* The LLM prompt was simplified after live testing with smaller Workers AI models; the provider also normalizes common small-model shape drift before applying the same schema and semantic validation.
* Tests avoid real Cloudflare credentials by using deterministic `--no-llm` mode and a mock provider.
* CLI smoke tests cover `vigil version`, `vigil validate` with `examples/minimal`, target/alert `--plan-only`, target `--no-llm` output, file-based `examples/minimal --no-llm`, and rendering from a saved trajectory.
* Core tests cover live local-HTTP collection for required Alertmanager, Prometheus, and GitHub adapters plus optional HTTP, DNS, Loki, Grafana, and Kubernetes adapter evidence collection.
* Markdown rendering has a golden-style fixture at `crates/vigil-render/tests/fixtures/evidence_brief.md`.
* Case evidence intake writes existing `Evidence` model YAML under `<case>/evidence/`; change intake uses `kind: change`.
* Case investigation reads `<case>/vigil.yaml`, `<case>/evidence/`, and `<case>/runbooks/`, then writes default outputs under `<case>/output/`.
* `vigil investigate <case-dir>` rejects case directories combined with file-mode flags such as `--inventory`.
* Agent investigation registers `Source`, `Capability`, `ToolPlan`, `ToolResult`, `InvestigationLoop`, and `InvestigationBudget` models in trajectory output.
* Agent investigation supports read-only source configs for `inventory-file`, `runbook-file`, `alertmanager`, `prometheus`, `github`, `http`, `dns`, `loki`, `grafana`, and `kubernetes`.
* `inventory-file` and `runbook-file` adapters read local files. External adapters support configured read-only network calls and fixture-backed local collection.
* LLM planning now requests and validates a dedicated `ToolPlan` schema instead of deriving plans only from `ReasoningResult.recommended_checks`.
* `--plan-only` prints a policy-validated read-only collection plan without executing adapters or sending an LLM request.
* Agent investigations write `output/brief.md`, `output/brief.json`, and `output/trajectory.json` by default when explicit output paths are not supplied.

## Validation Notes

* During the read-only agent implementation, `cargo fmt` passed.
* During the read-only agent implementation, `cargo check --workspace` passed.
* During the read-only agent implementation, `cargo fmt --check` passed.
* During the read-only agent implementation, `cargo clippy --workspace --all-targets -- -D warnings` passed.
* During the read-only agent implementation, `cargo test --workspace` initially failed in existing `vigil-llm` localhost HTTP tests with sandbox `Operation not permitted`, then passed when rerun with the required permission.
* After completing live adapters and ToolPlan planning, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` passed.
* `cargo fmt --check` passed after the live product-evaluation fixes.
* `cargo clippy --workspace --all-targets -- -D warnings` passed after the live product-evaluation fixes.
* `cargo test --workspace` passed after the live product-evaluation fixes.
* Manual case flow passed with `/tmp/web-5xx`: `case init`, `evidence add --kind metric --url`, `change add --url`, `runbook add`, and `investigate /tmp/web-5xx --no-llm`.
* Manual log evidence intake passed with `evidence add --kind log --file /tmp/timeout-snippet.txt`.
* Manual file-based compatibility run passed with `examples/minimal` and explicit Markdown, JSON, and trajectory outputs.
* Manual ambiguous-input check passed: `vigil investigate /tmp/web-5xx --inventory examples/minimal/inventory.yaml --no-llm` failed with an actionable ambiguity error.
* Temporary live Cloudflare validation on 2026-06-29 did not produce an LLM response: `vigil investigate /tmp/web-5xx` against the REST endpoint returned HTTP 401, and a direct minimal REST request returned the same authentication error.
* A provider-native `gateway.ai.cloudflare.com` diagnostic using Gateway authentication reached Cloudflare AI Gateway for an OpenAI-compatible model but returned HTTP 402 `Insufficient wholesale credits`.
* Workers AI lightweight model validation on 2026-06-29 used `@cf/meta/llama-3.2-1b-instruct`: direct Workers AI REST, AI Gateway REST, and `vigil investigate /tmp/web-5xx` all returned HTTP 401 with the temporary token.
* The same Workers AI model returned HTTP 200 for a simple prompt through `gateway.ai.cloudflare.com/.../workers-ai/v1/chat/completions` with Gateway authentication, confirming the model/gateway path worked before Vigil supported that endpoint style.
* Live product evaluation on 2026-06-30 succeeded through `VIGIL_CLOUDFLARE_ENDPOINT=gateway` with `@cf/meta/llama-3.1-8b-instruct-fast` for web 5xx, checkout latency, and payment auth-failure case workflows.
* Live evaluation showed `@cf/meta/llama-3.2-1b-instruct` is too weak for Vigil's structured `ReasoningResult` contract: it can answer simple prompts, but did not produce schema-valid investigation output.
* Live evaluation found and fixed a redaction gap where inline secrets inside log file content, such as `api_token=...`, reached brief JSON and trajectory output.

## Known Limitations

* Redaction is intentionally basic and cannot guarantee perfect secret detection.
* Prompt-injection text inside supplied evidence remains visible as evidence content after secret redaction; validation prevents generated recommended checks from becoming shell commands, but raw evidence should still be reviewed before sharing output.
* CloudWatch, ticketing, and tracing adapters are not implemented.
* Kubernetes support is direct read-only API access from a configured URL; Vigil does not require Kubernetes and does not run a target-host agent.

## Open Questions

* None.
