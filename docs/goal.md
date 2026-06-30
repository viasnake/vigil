# Vigil Goal

## Project

Vigil is an open-source, read-only SRE investigation agent.

Repository:

```text
github.com/viasnake/vigil
```

Vigil helps operators start from a target, alert, incident, or operational question; collect bounded read-only context from configured sources; and produce structured, evidence-backed investigation briefs.

Vigil is an agent only in the investigation sense. It is not a general-purpose AI agent, a coding agent, a ChatOps bot, or an autonomous remediation system. It is focused on the first and most important phase of SRE work: understanding what is happening, what evidence exists, what may have changed, and what should be checked next.

## Background and Rationale

Vigil is designed as an AI-assisted SRE investigation tool because SRE work is becoming increasingly constrained by operational complexity, fragmented evidence, and accelerated change velocity.

Modern AI coding and automation tools can increase the speed at which software and infrastructure changes are produced. This makes purely manual investigation workflows less scalable. When incidents occur, operators must quickly understand what changed, what is affected, what evidence exists, and what should be verified next.

Vigil focuses on that initial investigation phase.

It does not attempt to become a general-purpose agent. It does not attempt to automate remediation. It does not attempt to replace SRE judgment.

The core design is based on five assumptions:

1. Incident investigation starts with evidence, not action.
2. LLMs are useful for summarization, hypothesis generation, and missing-check discovery.
3. LLMs must not be trusted as execution engines.
4. Production action should be separated from reasoning.
5. Investigation history should become structured operational memory.

Vigil 1.0 therefore aims to produce an evidence-backed investigation brief, not an automated fix.

### Why Evidence Briefs

The first useful output in an incident is a concise operational brief that answers:

* What is affected?
* What evidence exists?
* What changed recently?
* What hypotheses are plausible?
* What is still unknown?
* What should be checked next?

This is intentionally narrower than full incident management.

Vigil does not replace monitoring, logging, tracing, ticketing, ChatOps, or postmortem systems. It sits between those inputs and the human operator.

### Why Assisted Investigation First

Vigil starts with assisted investigation because this is the safest and most generally useful entry point for AI in SRE workflows.

Investigation work benefits from LLMs because it involves summarizing partial evidence, connecting symptoms with known failure modes, and proposing next checks. However, execution and remediation require stronger safety controls, validation, authorization, and rollback semantics.

Vigil 1.0 does not implement production execution or remediation. It may execute bounded, policy-validated, read-only collection through registered capabilities.

This keeps the initial product useful without creating unnecessary production risk.

### Why Cloudflare AI Gateway First

Vigil 1.0 supports Cloudflare AI Gateway as the only LLM provider.

This keeps the implementation small while still allowing users to route requests through a managed gateway that can provide centralized model access, logging, caching, rate limiting, and related gateway controls.

Vigil keeps an internal provider boundary so that additional providers can be added later, but 1.0 should not implement multiple providers.

The goal is to avoid early provider sprawl.

### Why Not ChatGPT or Codex Subscription Reuse

Vigil 1.0 does not support ChatGPT or Codex subscription reuse.

Codex authentication and ChatGPT sign-in are suitable for Codex workflows, but Vigil should not treat subscription sign-in as a general-purpose OpenAI API replacement.

Vigil should use documented API or gateway paths only.

This avoids depending on private endpoints, undocumented token reuse, or provider policy ambiguity.

### Why Rust

Vigil is implemented in Rust because it should be a lightweight, reliable, distributable operational tool.

Rust is suitable for this project because Vigil should:

* ship as a practical CLI,
* avoid heavy runtime dependencies,
* model operational data strictly,
* handle errors predictably,
* support future standalone components if needed,
* remain suitable for SRE environments where reliability and distribution matter.

Vigil should avoid heavy agent frameworks in 1.0.

The core value is not an agent framework.
The core value is the bounded read-only investigation pipeline:

```text
target or alert
  -> read-only context collection
  -> LLM-assisted investigation planning
  -> policy-validated read-only tool calls
  -> evidence collection
  -> hypothesis update
  -> EvidenceBrief
  -> trajectory record
```

### Why Trajectory Recording

Incident investigation should produce reusable operational memory.

Vigil records a trajectory so that investigations can be reviewed, tested, improved, and eventually used for regression evaluation.

The trajectory is not a transcript. It is a structured record of:

* inputs,
* resolved targets,
* configured sources,
* registered capabilities,
* planned read-only tool calls,
* tool results,
* evidence,
* LLM request metadata,
* LLM response metadata,
* hypotheses,
* recommended checks,
* validation results,
* warnings,
* outputs.

This supports future evaluation without requiring Vigil 1.0 to implement a full evaluation framework.

## Reference-to-Decision Map

| Design Decision                               | Reason                                                                                              |
| --------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| Focus on assisted investigation               | AI can reduce initial investigation load without requiring unsafe production mutation.              |
| Produce Evidence Briefs                       | Operators need a compact, source-backed view before acting.                                         |
| Do not execute production actions             | Reasoning and actuation must remain separate until safety controls exist.                           |
| Execute only registered read-only collection  | Vigil needs operational context without becoming a remediation or shell execution system.           |
| Use Cloudflare AI Gateway only in 1.0         | Keeps provider integration small while allowing centralized AI gateway behavior.                    |
| Avoid ChatGPT/Codex subscription reuse        | Subscription sign-in is not a general-purpose API provider contract for Vigil.                      |
| Use Rust                                      | Supports lightweight, reliable, distributable CLI implementation.                                   |
| Record trajectories                           | Investigation history should become structured operational memory for future review and evaluation. |
| Keep user-facing docs implementation-accurate | Public docs should describe what exists, not speculative design intent.                             |

## References

The following references explain the design context behind Vigil.

### SRE and AI-assisted Operations

* Google SRE, “AI Engineering for Reliable Operations”
  https://sre.google/resources/practices-and-processes/ai-engineering-reliable-operations/
  Used as the primary reference for AI-assisted SRE operations, assisted investigation, operational context, safety boundaries, and progressive authorization.

* Google SRE Book, “Postmortem Culture: Learning from Failure”
  https://sre.google/sre-book/postmortem-culture/
  Used as a reference for treating incidents as learning opportunities and preserving operational knowledge.

### LLM Provider and Gateway

* Cloudflare AI Gateway, “REST API”
  https://developers.cloudflare.com/ai-gateway/usage/rest-api/
  Used as the implementation reference for Cloudflare AI Gateway as Vigil 1.0’s only LLM provider.

* Cloudflare AI Gateway, “Unified API / OpenAI compatibility”
  https://developers.cloudflare.com/ai-gateway/usage/chat-completion/
  Used as a reference for OpenAI-compatible gateway behavior through Cloudflare AI Gateway.

### Codex and Subscription Boundary

* OpenAI Developers, “Codex Authentication”
  https://developers.openai.com/codex/auth
  Used as the basis for excluding ChatGPT/Codex subscription reuse from Vigil 1.0.

* OpenAI Developers, “Codex CLI Reference”
  https://developers.openai.com/codex/cli/reference
  Used as background for understanding Codex as a separate runtime and not as Vigil’s general LLM provider.

### Implementation Language

* Rust Programming Language
  https://www.rust-lang.org/
  Used as the reference for Rust’s memory safety, thread safety, performance, and suitability for lightweight operational tooling.

### Related Tooling Context

* OpenClaw
  https://github.com/openclaw/openclaw
  Used as a contrast point. Vigil is intentionally not a general-purpose personal assistant or desktop automation agent.

* Model Context Protocol Specification
  https://modelcontextprotocol.io/specification/
  Used as future context only. Vigil 1.0 does not implement MCP.

## Reference Policy

This `goal.md` may cite external references because it explains why Vigil exists and why the 1.0 scope is constrained.

User-facing documentation must follow a stricter rule:

* It must describe implemented behavior.
* It must not present planned behavior as supported.
* It should avoid broad design discussion unless required to use the tool safely.

The goal document is allowed to describe the intended 1.0 target.
User-facing docs must describe the actual implementation.

## 1.0 Goal

Vigil 1.0 must be a reliable Rust CLI that can start from a target, alert, or incident case; collect bounded read-only operational context from configured sources; iteratively update hypotheses; and produce an evidence-backed investigation brief through Cloudflare AI Gateway or deterministic no-LLM mode.

The primary 1.0 workflow is:

```text
target or alert
  -> read-only context collection
  -> LLM-assisted investigation planning
  -> policy-validated read-only tool calls
  -> evidence collection
  -> hypothesis update
  -> evidence-backed brief
  -> trajectory record
```

The primary user commands are:

```bash
vigil investigate service:web --since 30m
vigil investigate alert WebHigh5xxRate --since 30m
vigil investigate service:web --since 30m --plan-only
```

File-based and case-based investigation remain supported for scripting, tests, and manually curated evidence, but they are not the primary user experience.

Vigil 1.0 must support:

* case-based investigation,
* target-based investigation,
* alert-based investigation,
* configured read-only sources,
* a bounded investigation loop,
* LLM-assisted next-check planning,
* policy validation before every tool call,
* evidence collection from read-only adapters,
* evidence-backed brief generation,
* trajectory recording.

Required 1.0 source and capability concepts:

* `Source`: a configured read-only context source.
* `Capability`: a registered read-only operation available through a source.
* `ToolPlan`: proposed read-only collection steps.
* `ToolResult`: evidence and status from a read-only adapter.
* `InvestigationLoop`: bounded plan, validate, collect, update iterations.
* `InvestigationBudget`: `max_iterations`, `max_tool_calls`, and `max_duration_secs`.

Required 1.0 read-only adapters:

* `inventory-file`
* `runbook-file`
* `alertmanager`
* `prometheus`
* `github`
* `http`
* `dns`
* `loki`
* `grafana`
* `kubernetes`

External adapters may support fixtures for tests and local replay, but their interfaces must be real read-only adapter interfaces rather than ad hoc evidence fields.

Vigil may:

* choose read-only investigation steps,
* collect evidence from configured sources,
* update hypotheses,
* identify missing checks,
* produce briefs.

Vigil must not:

* execute shell commands,
* SSH into hosts,
* mutate production,
* restart services,
* apply remediations,
* open pull requests,
* change infrastructure,
* bypass policy validation,
* implement MCP in 1.0,
* reuse ChatGPT or Codex subscription authentication as an LLM provider.

Every tool call must be read-only, registered, policy-validated, and recorded in the trajectory.

## North Star

Reduce the time and cognitive load required to start a reliable incident investigation.

The first useful output is not an action.
The first useful output is an evidence-backed operational brief.

## Product Position

Vigil sits between raw operational signals and human SRE judgment.

```text
Alerts / logs / metrics / runbooks / inventory / changes
        ↓
Vigil
        ↓
Evidence Brief + Hypotheses + Missing Checks + Recommended Read-Only Checks
        ↓
Human SRE investigation
```

Vigil should help the operator move from “an alert fired” to “here is what is affected, what evidence exists, what changed, and what to verify next.”

## Core Use Cases

Vigil 1.0 must support these use cases.

### 1. Alert Investigation

Given an alert selector or alert source, collect read-only context and generate an investigation brief.

Example:

```bash
vigil investigate alert WebHigh5xxRate --since 30m
```

The result should answer:

* What alert fired?
* What target does it affect?
* What symptoms are known?
* What evidence is available?
* What hypotheses are plausible?
* What is still unknown?
* What read-only checks should be performed next?

### 2. Target Investigation

Given a service or host target, collect read-only context and generate an operational investigation brief.

Example:

```bash
vigil investigate service:web --since 30m
```

The result should answer:

* What is this target?
* What dependencies does it have?
* What runbooks match it?
* What related alerts or evidence exist?
* What should be checked first?

### 3. Runbook-Assisted Investigation

Given a runbook and a symptom, use the runbook to guide hypotheses and checks.

Vigil must not blindly follow the runbook. It should use the runbook as structured context and produce evidence-backed recommendations.

### 4. Evidence Brief Generation

Given structured evidence, produce a human-readable brief and a machine-readable output.

Supported outputs:

```text
Markdown
JSON
```

Markdown is for humans.
JSON is for tools, tests, and future integrations.

### 5. Investigation Trajectory Recording

Vigil must record the investigation trajectory.

The trajectory is not a chat log. It is structured operational memory.

It should include:

* input files or investigation selector,
* configured sources,
* registered capabilities,
* planned read-only tool calls,
* tool results,
* resolved targets,
* collected evidence,
* LLM request metadata,
* LLM response metadata,
* hypotheses,
* recommended checks,
* validation results,
* rendered outputs,
* errors or warnings.

This is required for debugging, regression testing, and future evaluation.

## Non-Goals for 1.0

Vigil 1.0 must not implement:

* autonomous remediation,
* production mutation,
* arbitrary shell execution,
* SSH execution on target hosts,
* target-host runner,
* ChatGPT subscription reuse,
* Codex subscription reuse,
* MCP server,
* desktop automation,
* ChatOps bot,
* ticketing system replacement,
* full incident management workflow,
* multi-agent orchestration,
* direct OpenAI API provider,
* Anthropic direct provider,
* Ollama provider,
* OpenRouter provider,
* LiteLLM provider.

The only supported LLM provider for 1.0 is Cloudflare AI Gateway.

## Implementation Language

Vigil must be implemented in Rust.

Reasons:

* lightweight distribution,
* single-binary CLI,
* reliable data modeling,
* low runtime dependency footprint,
* suitable for operational tooling,
* suitable for future target-side components if needed,
* strong compile-time guarantees.

## Runtime Form

Vigil 1.0 is a local CLI.

It must not require a daemon.

It must not require a background service.

It must not require a target-host agent.

It must not require Kubernetes.

It must not require Docker.

It may later gain a server mode, but 1.0 must be useful as a standalone CLI.

## LLM Provider

Vigil 1.0 supports Cloudflare AI Gateway only.

The implementation must keep an internal provider boundary so that additional providers can be added later, but no other provider should be implemented for 1.0.

### Provider Responsibilities

The Cloudflare provider receives an `EvidencePacket` and returns schema-validated `ToolPlan` and `ReasoningResult` responses for planning and final reasoning.

The provider must not:

* collect evidence,
* execute commands,
* perform policy decisions,
* mutate production,
* bypass validation,
* return unstructured output as authoritative result.

### Required Configuration

Vigil must support configuration for:

* Cloudflare account ID,
* Cloudflare API token,
* AI Gateway ID,
* model name,
* request timeout,
* retry count,
* output format.

Configuration may come from:

* CLI flags,
* environment variables,
* config file.

Precedence:

```text
CLI flag > environment variable > config file > default
```

### Cloudflare Configuration Environment Variables

Vigil should support:

```text
CLOUDFLARE_ACCOUNT_ID
CLOUDFLARE_API_TOKEN
VIGIL_CLOUDFLARE_GATEWAY_ID
VIGIL_LLM_MODEL
```

## Safety Boundary

Vigil 1.0 is an investigation assistant.

Hard boundaries:

* No production mutation.
* No command execution.
* No SSH execution.
* No shell command generation for direct execution.
* No autonomous remediation.
* No target-host runner.
* No background monitoring loop.
* No credential scraping.
* No use of undocumented ChatGPT, Codex, or provider-private APIs.

LLM output is advisory.

Vigil must validate LLM output before rendering it.

Vigil must clearly distinguish:

* observed evidence,
* inferred hypothesis,
* missing check,
* recommended check,
* unsupported assumption.

Hypotheses are not facts.

LLM recommended checks are not executed directly. Vigil may convert them into registered read-only `ToolPlan` calls, validate them against policy, and execute only those read-only adapter calls.

## Data Flow

The 1.0 investigation workflow is:

```text
Target / alert / case / file inputs
        ↓
Target or alert resolution
        ↓
Initial read-only context collection
        ↓
EvidencePacket construction
        ↓
Redaction / normalization
        ↓
Cloudflare AI Gateway planning or deterministic planning
        ↓
ToolPlan
        ↓
Policy validation
        ↓
Registered read-only adapter calls
        ↓
ToolResult evidence
        ↓
ReasoningResult over expanded EvidencePacket
        ↓
Schema validation
        ↓
EvidenceBrief
        ↓
Markdown / JSON output
        ↓
Trajectory record
```

## Core Data Models

Vigil must define stable Rust data models for the following.

### Target

Represents an operational target.

Fields should include:

```text
id
kind
name
environment
service
host
labels
criticality
metadata
```

Target kinds:

```text
service
host
component
endpoint
unknown
```

### Alert

Represents an alert or incident input.

Fields should include:

```text
id
name
severity
status
summary
description
target
started_at
ended_at
labels
annotations
source
```

### Inventory

Represents known operational targets and their relationships.

Fields should include:

```text
targets
services
hosts
dependencies
labels
metadata
```

### Runbook

Represents structured investigation guidance.

Fields should include:

```text
id
title
applies_to
symptoms
checks
notes
references
```

Runbook checks in 1.0 are recommendations only. They are not executed.

### Evidence

Represents a single observed or supplied fact.

Fields should include:

```text
id
kind
summary
source
target
timestamp
confidence
data
references
```

Evidence kinds:

```text
alert
metric
log
change
runbook
inventory
operator_input
external
```

### Source

Represents a configured read-only context source.

Fields should include:

```text
id
kind
name
read_only
config
```

### Capability

Represents a registered read-only operation exposed by a source.

Fields should include:

```text
id
kind
source_id
adapter
read_only
description
input_schema
risk
```

### ToolPlan

Represents proposed read-only collection steps.

Fields should include:

```text
id
rationale
calls
```

### ToolResult

Represents the result of a policy-validated read-only adapter call.

Fields should include:

```text
call_id
capability_id
source_id
status
started_at
completed_at
evidence
error
```

### InvestigationBudget

Represents loop limits.

Fields should include:

```text
max_iterations
max_tool_calls
max_duration_secs
```

### EvidencePacket

The exact structured input sent to the LLM provider.

Fields should include:

```text
investigation_id
question
targets
alerts
evidence
runbooks
constraints
redaction
metadata
```

The `EvidencePacket` must be serializable to JSON.

### ReasoningResult

The structured LLM response.

Fields should include:

```text
summary
hypotheses
missing_checks
recommended_checks
risk_notes
operator_notes
confidence_notes
```

The `ReasoningResult` must be schema-validated.

### Hypothesis

Represents a candidate explanation.

Fields should include:

```text
id
title
description
confidence
supporting_evidence_ids
contradicting_evidence_ids
missing_checks
risk_if_wrong
```

### RecommendedCheck

Represents a suggested next check.

Fields should include:

```text
id
title
description
target
reason
read_only
source
related_evidence_ids
```

In 1.0, recommended checks are not executed.
They may inform later `ToolPlan` proposals, but only registered read-only capabilities can be executed.

### EvidenceBrief

The final human-facing investigation brief.

Fields should include:

```text
title
summary
targets
evidence
hypotheses
missing_checks
recommended_checks
risk_notes
references
warnings
```

### Trajectory

Represents the recorded investigation process.

Fields should include:

```text
id
started_at
completed_at
inputs
sources
capabilities
investigation_loop
resolved_targets
evidence_packet
reasoning_result
brief
warnings
errors
```

## CLI Requirements

The primary binary is:

```text
vigil
```

### Required Commands for 1.0

#### `vigil case init`

Creates a local investigation case.

Example:

```bash
vigil case init web-5xx \
  --target service:web \
  --severity page \
  --summary "Web service 5xx responses are above threshold"
```

#### `vigil evidence add`

Adds structured evidence to a case.

Example:

```bash
vigil evidence add web-5xx \
  --kind metric \
  --summary "HTTP 5xx rate increased from 0.2% to 8.4%" \
  --source prometheus \
  --url "https://grafana.example.com/d/web"
```

#### `vigil change add`

Adds change evidence to a case.

Example:

```bash
vigil change add web-5xx \
  --summary "Caddy upstream timeout setting changed before the alert" \
  --source github \
  --url "https://github.com/example/repo/pull/123"
```

#### `vigil runbook add`

Validates and copies a runbook into a case.

Example:

```bash
vigil runbook add web-5xx examples/minimal/runbooks/web-5xx.yaml
```

#### `vigil investigate`

Main command.

Primary target behavior:

```bash
vigil investigate service:web --since 30m
```

Primary alert behavior:

```bash
vigil investigate alert WebHigh5xxRate --since 30m
```

Plan-only behavior:

```bash
vigil investigate service:web --since 30m --plan-only
```

Case compatibility behavior:

```bash
vigil investigate web-5xx
```

Required file-based compatibility behavior:

```bash
vigil investigate \
  --alert alert.yaml \
  --inventory inventory.yaml \
  --runbook-dir runbooks \
  --output brief.md
```

Options:

```text
--alert <PATH>
--inventory <PATH>
--runbook <PATH>
--runbook-dir <PATH>
--output <PATH>
--json-output <PATH>
--trajectory-output <PATH>
--config <PATH>
--model <MODEL>
--gateway-id <GATEWAY_ID>
--dry-run
--no-llm
--since <DURATION>
--plan-only
--source <SOURCE>
--max-iterations <COUNT>
--max-tool-calls <COUNT>
--max-duration-secs <SECONDS>
<TARGET_OR_CASE>
```

`--no-llm` exists only for testing and deterministic rendering. It is not the primary assistant experience.
`--plan-only` prints proposed read-only tool calls without executing adapters.

#### `vigil config check`

Validates configuration.

It should check:

* required Cloudflare settings,
* readable input files,
* output path validity,
* basic model configuration.

#### `vigil validate`

Validates input files.

Example:

```bash
vigil validate --inventory inventory.yaml --alert alert.yaml
```

#### `vigil render`

Renders a brief from a saved trajectory or JSON result.

Example:

```bash
vigil render --trajectory trajectory.json --output brief.md
```

#### `vigil version`

Prints version information.

## Input Formats

Vigil 1.0 must support YAML and JSON input where practical.

Required input categories:

```text
case manifest
case evidence
inventory
alert
runbook
source fixtures
```

Examples must be included under:

```text
examples/minimal/
```

Minimum example files:

```text
examples/minimal/inventory.yaml
examples/minimal/alert.yaml
examples/minimal/runbooks/web-5xx.yaml
```

## Output Formats

Vigil 1.0 must support:

```text
Markdown brief
JSON brief
JSON trajectory
```

Case investigation writes these outputs under `<case>/output/` by default.
Target and alert investigation write these outputs under `output/` by default.

The Markdown brief must be readable without any external UI.

The JSON output must be stable enough for tests.

## Redaction

Vigil 1.0 must include basic redaction before sending an `EvidencePacket` to Cloudflare AI Gateway.

Minimum redaction behavior:

* mask common token-like values,
* mask obvious password fields,
* mask obvious secret fields,
* avoid sending raw environment variables,
* avoid sending raw credentials,
* mark whether redaction was applied.

Vigil must not claim perfect secret detection.

If redaction is incomplete or disabled, the output must include a warning.

## Error Handling

Vigil must produce clear errors for:

* missing Cloudflare account ID,
* missing Cloudflare API token,
* missing gateway ID,
* unreadable input file,
* invalid YAML,
* invalid JSON,
* invalid inventory schema,
* invalid alert schema,
* Cloudflare request failure,
* timeout,
* invalid LLM response,
* schema validation failure,
* output write failure.

Errors should be actionable.

Bad:

```text
request failed
```

Good:

```text
Cloudflare AI Gateway request failed: received HTTP 401. Check CLOUDFLARE_API_TOKEN.
```

## Crate Structure

Use a Rust workspace.

Initial crate layout:

```text
crates/
  vigil-cli/
  vigil-core/
  vigil-model/
  vigil-llm/
  vigil-render/
  vigil-config/
```

### `vigil-cli`

CLI entrypoint.

Responsibilities:

* parse arguments,
* load config,
* call core workflows,
* print user-facing errors.

### `vigil-core`

Investigation workflow.

Responsibilities:

* orchestrate investigation,
* resolve targets,
* register read-only sources and capabilities,
* build and validate tool plans,
* execute read-only adapter calls,
* build evidence packets,
* call LLM provider,
* validate reasoning result,
* build final brief,
* save trajectory.

### `vigil-model`

Shared data models.

Responsibilities:

* define Target,
* define Alert,
* define Evidence,
* define Source,
* define Capability,
* define ToolPlan,
* define ToolResult,
* define InvestigationLoop,
* define InvestigationBudget,
* define EvidencePacket,
* define ReasoningResult,
* define EvidenceBrief,
* define Trajectory,
* provide serialization and schema support.

### `vigil-llm`

LLM provider implementation.

Responsibilities:

* define provider trait,
* implement Cloudflare AI Gateway provider,
* build request payload,
* parse response,
* handle timeouts and provider errors.

### `vigil-render`

Output rendering.

Responsibilities:

* render Markdown,
* render JSON,
* manage templates if needed.

### `vigil-config`

Configuration loading and validation.

Responsibilities:

* config file loading,
* environment variable loading,
* CLI override merging,
* validation.

## Suggested Dependencies

Use lightweight dependencies.

Suggested workspace dependencies:

```toml
anyhow = "1"
thiserror = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
toml = "0.8"
schemars = { version = "1", features = ["derive"] }
jsonschema = "0.18"
clap = { version = "4", features = ["derive", "env"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros", "time"] }
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
tracing = "0.1"
tracing-subscriber = "0.3"
minijinja = "2"
uuid = { version = "1", features = ["serde", "v7"] }
chrono = { version = "0.4", features = ["serde"] }
```

Avoid large agent frameworks.

Avoid provider SDK lock-in unless clearly necessary.

Prefer direct HTTP integration for Cloudflare AI Gateway.

## Testing Requirements

Vigil 1.0 must include tests for:

```text
model serialization
input parsing
config loading
EvidencePacket construction
read-only capability registration
ToolPlan policy validation
fixture-backed adapter collection
bounded investigation loop
redaction
ReasoningResult validation
Markdown rendering
JSON output
Cloudflare provider request construction
Cloudflare provider mock response handling
CLI smoke tests
```

The test suite must not require real Cloudflare credentials.

Use mock HTTP server or mocked provider for tests.

Golden tests should be used for rendered Markdown output.

## Development Quality Requirements

Before 1.0, the following must pass:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The CLI must provide useful `--help`.

The examples must run against a configured Cloudflare AI Gateway.

The repository must not include real credentials.

## Documentation Requirements for 1.0

Project documentation should be user-facing and implementation-accurate.

Do not document unimplemented behavior as supported.

Required documents for 1.0:

```text
README.md
docs/getting-started.md
docs/configuration.md
docs/cloudflare-ai-gateway.md
docs/input-format.md
docs/output-format.md
docs/commands.md
docs/security-and-privacy.md
docs/troubleshooting.md
```

This `goal.md` is the implementation contract. It may describe the 1.0 target even before all behavior is implemented.

User-facing docs must describe only implemented behavior.

## Milestones

### v0.1: Skeleton

Deliver:

* Rust workspace,
* CLI skeleton,
* core model crate,
* minimal README,
* example directory,
* basic CI.

Completion:

* `cargo test --workspace` passes.
* `vigil version` works.
* no LLM call yet.

### v0.2: Models and Input

Deliver:

* Target model,
* Alert model,
* Inventory model,
* Runbook model,
* Evidence model,
* YAML parsing,
* validation command.

Completion:

* example input files parse successfully.
* invalid input produces clear errors.

### v0.3: Evidence Packet

Deliver:

* target resolution,
* evidence construction,
* EvidencePacket JSON output,
* redaction pass.

Completion:

* `vigil investigate --no-llm` can produce a deterministic draft brief and EvidencePacket.

### v0.4: Cloudflare AI Gateway

Deliver:

* Cloudflare provider,
* configuration loading,
* request construction,
* timeout handling,
* mock tests.

Completion:

* with valid Cloudflare configuration, Vigil can send an EvidencePacket and receive a response.
* tests do not require real credentials.

### v0.5: Structured Reasoning

Deliver:

* ReasoningResult schema,
* response parsing,
* response validation,
* invalid response handling.

Completion:

* LLM output is never used unless it validates.
* failures produce actionable errors.

### v0.6: Brief Rendering

Deliver:

* Markdown Evidence Brief,
* JSON Evidence Brief,
* output file support,
* golden tests.

Completion:

* example investigation produces a useful Markdown brief.

### v0.7: Trajectory

Deliver:

* trajectory model,
* trajectory output,
* render from trajectory.

Completion:

* `vigil render --trajectory trajectory.json --output brief.md` works.

### v0.8: Polish and UX

Deliver:

* improved CLI help,
* better error messages,
* config check command,
* troubleshooting docs,
* example improvements.

Completion:

* a new user can run the minimal example by following docs.

### v0.9: Release Candidate

Deliver:

* stable CLI surface,
* stable input/output schemas,
* complete user docs,
* CI release workflow,
* prebuilt binaries if feasible.

Completion:

* no known blocking bugs.
* user-facing docs match implemented behavior.

### v1.0: Stable Initial Release

Deliver:

* stable Rust CLI,
* Cloudflare AI Gateway provider,
* target-based investigation workflow,
* alert-based investigation workflow,
* bounded read-only investigation loop,
* source and capability registry,
* policy-validated tool planning,
* fixture-backed or live read-only adapters for required 1.0 source types,
* case-based input workflow,
* file-based input workflow for compatibility,
* LLM-assisted investigation brief,
* schema-validated reasoning output,
* Markdown and JSON output,
* trajectory recording,
* user-facing documentation,
* tests and CI.

1.0 is complete when Vigil can be used as a practical SRE investigation assistant without production mutation.

## Acceptance Criteria for 1.0

Vigil 1.0 is acceptable only when all of the following are true:

* `vigil investigate service:web --since 30m` works.
* `vigil investigate alert WebHigh5xxRate --since 30m` works.
* `vigil investigate service:web --since 30m --plan-only` works.
* `vigil investigate` works with a case directory.
* `vigil investigate` works with the minimal file-based example.
* Every agent tool call is read-only, registered, policy-validated, and recorded in the trajectory.
* Cloudflare AI Gateway is the only implemented LLM provider.
* LLM responses are schema-validated.
* Invalid LLM responses are rejected or handled safely.
* Markdown output is readable and useful.
* JSON output is machine-readable and tested.
* Trajectory output is generated.
* No production action is executed.
* No shell command is executed.
* No target-host runner exists.
* Tests pass in CI without real credentials.
* User-facing docs describe implemented behavior.
* Error messages are actionable.
* The project builds as a Rust workspace.
* The repository contains no secrets.

## Implementation Rules for Codex

When implementing Vigil, follow these rules.

1. Build the smallest correct version for each milestone.
2. Do not implement features outside the current milestone unless required by the goal.
3. Do not add ChatGPT, Codex, OpenAI direct, Anthropic direct, Ollama, OpenRouter, LiteLLM, MCP, or runner support before 1.0.
4. Keep Cloudflare AI Gateway as the only LLM provider for 1.0.
5. Keep provider abstraction internal and minimal.
6. Do not create a daemon.
7. Do not execute shell commands.
8. Do not mutate production state.
9. Do not store credentials in output, logs, examples, tests, or trajectory files.
10. Write tests for new behavior.
11. Keep user-facing docs aligned with implemented behavior.
12. Prefer clear data models over clever agent abstractions.
13. Prefer deterministic validation over trusting LLM output.
14. Prefer explicit errors over silent fallback.
15. Keep the CLI useful without requiring external services for tests.

## Final Product Statement

Vigil 1.0 is a Rust-based, Cloudflare-AI-Gateway-backed, read-only SRE investigation agent.

It starts from a target or alert, collects bounded read-only context from configured sources, and turns the resulting evidence into investigation briefs.

It helps operators understand incidents faster.

It does not fix production automatically.

It does not execute shell commands.

It does not replace SRE judgment.

It provides a reliable foundation for future SRE AI tooling.

## 1.0 UX Contract

Vigil 1.0 must be usable during an actual SRE investigation.

The primary workflow is target- or alert-based read-only investigation.

### Primary Workflow

```text
target or alert
  -> resolve context from configured sources
  -> plan read-only checks
  -> validate the plan against registered capabilities
  -> collect evidence through read-only adapters
  -> update hypotheses
  -> render brief, JSON, and trajectory
```

### Required User Experience

An operator must be able to start from a real target, alert, or symptom without hand-writing a complete EvidencePacket.

The user should be able to:

1. Run target investigation with `vigil investigate service:web --since 30m`.
2. Run alert investigation with `vigil investigate alert WebHigh5xxRate --since 30m`.
3. Preview read-only collection with `--plan-only`.
4. Configure local inventory and runbook sources.
5. Configure read-only Alertmanager, Prometheus, GitHub, HTTP, DNS, Loki, Grafana, and Kubernetes sources.
6. Run LLM-assisted investigation through Cloudflare AI Gateway.
7. Receive a Markdown brief suitable for incident notes.
8. Receive JSON output suitable for tooling.
9. Receive a trajectory suitable for replay, debugging, and future evaluation.
10. Use case and file workflows for scripting, tests, and manually curated evidence.
11. Do all of the above without production mutation, shell execution, SSH, or remediation.

### Required Commands

Vigil 1.0 should provide these user-facing commands:

```text
vigil investigate
vigil case init
vigil evidence add
vigil change add
vigil runbook add
vigil render
vigil validate
vigil config check
vigil version
```

### Agent Outputs

Target and alert investigation should write by default:

```text
output/brief.md
output/brief.json
output/trajectory.json
```

`--plan-only` should print planned read-only collection and should not collect evidence.

### Case Compatibility

A Vigil case directory remains supported:

```text
web-5xx/
  vigil.yaml
  evidence/
  runbooks/
  output/
    brief.md
    brief.json
    trajectory.json
```

Case workflows are useful for manually curated evidence, regression tests, and scripted local investigations, but they are not the primary 1.0 UX.

### File Compatibility

The file-based workflow should remain available for scripting:

```bash
vigil investigate \
  --alert alert.yaml \
  --inventory inventory.yaml \
  --runbook-dir runbooks \
  --output brief.md
```

### Non-Goals

The agent workflow must not introduce:

* shell command execution,
* SSH execution,
* target-host runners,
* production mutation,
* autonomous remediation,
* ChatOps bot behavior,
* MCP,
* background monitoring.
