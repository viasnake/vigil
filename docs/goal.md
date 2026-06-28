# Vigil Goal

## Project

Vigil is an open-source, AI-assisted SRE investigation tool.

Repository:

```text
github.com/viasnake/vigil
```

Vigil helps operators convert alerts, incidents, and operational questions into structured, evidence-backed investigation briefs.

Vigil is not a general-purpose AI agent, a coding agent, a ChatOps bot, or an autonomous remediation system. It is focused on the first and most important phase of SRE work: understanding what is happening, what evidence exists, what may have changed, and what should be checked next.

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

Vigil 1.0 does not implement execution.

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
The core value is the structured pipeline:

```text
EvidencePacket
  -> Cloudflare AI Gateway
  -> ReasoningResult
  -> validation
  -> EvidenceBrief
  -> Trajectory
```

### Why Trajectory Recording

Incident investigation should produce reusable operational memory.

Vigil records a trajectory so that investigations can be reviewed, tested, improved, and eventually used for regression evaluation.

The trajectory is not a transcript. It is a structured record of:

* inputs,
* resolved targets,
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

Vigil 1.0 must be a reliable Rust CLI that can investigate an operational issue using structured inputs, read-only integrations, and Cloudflare AI Gateway.

At 1.0, a user should be able to run:

```bash
vigil investigate \
  --alert examples/minimal/alert.yaml \
  --inventory examples/minimal/inventory.yaml \
  --runbook-dir examples/minimal/runbooks \
  --output brief.md
```

and receive a useful investigation brief containing:

* affected target summary,
* relevant evidence,
* recent changes,
* LLM-assisted hypotheses,
* missing checks,
* recommended read-only checks,
* uncertainty,
* source references,
* structured JSON output,
* and a trajectory record suitable for later review.

Vigil 1.0 must be useful for a real SRE investigation without executing production actions.

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

Given an alert file or alert source, generate an investigation brief.

Example:

```bash
vigil investigate --alert alert.yaml --inventory inventory.yaml
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

Given a service or host target, generate an operational investigation brief.

Example:

```bash
vigil investigate target service:web --inventory inventory.yaml
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

* input files,
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

The Cloudflare provider receives an `EvidencePacket` and returns a `ReasoningResult`.

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

Recommended checks are not executed by Vigil 1.0.

## Data Flow

The 1.0 investigation workflow is:

```text
Input files / read-only adapters
        ↓
Target resolution
        ↓
Evidence collection
        ↓
EvidencePacket construction
        ↓
Redaction / normalization
        ↓
Cloudflare AI Gateway
        ↓
ReasoningResult
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

#### `vigil investigate`

Main command.

Required behavior:

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
```

`--no-llm` exists only for testing and deterministic rendering. It is not the primary assistant experience.

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
inventory
alert
runbook
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
* file-based input workflow,
* LLM-assisted investigation brief,
* schema-validated reasoning output,
* Markdown and JSON output,
* trajectory recording,
* user-facing documentation,
* tests and CI.

1.0 is complete when Vigil can be used as a practical SRE investigation assistant without production mutation.

## Acceptance Criteria for 1.0

Vigil 1.0 is acceptable only when all of the following are true:

* `vigil investigate` works with the minimal example.
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

Vigil 1.0 is a Rust-based, Cloudflare-AI-Gateway-backed SRE investigation CLI.

It turns structured operational inputs into evidence-backed investigation briefs.

It helps operators understand incidents faster.

It does not fix production automatically.

It does not execute commands.

It does not replace SRE judgment.

It provides a reliable foundation for future SRE AI tooling.
