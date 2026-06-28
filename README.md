# Vigil

Vigil is a Rust CLI for producing evidence-backed SRE investigation briefs from structured local inputs.

The current implementation reads alert, inventory, and runbook files, builds a redacted evidence packet, asks Cloudflare AI Gateway for structured reasoning, validates the response, and renders Markdown, JSON, and trajectory output.

## Quick Start

Run the deterministic example without an LLM:

```bash
cargo run -p vigil-cli -- investigate \
  --alert examples/minimal/alert.yaml \
  --inventory examples/minimal/inventory.yaml \
  --runbook-dir examples/minimal/runbooks \
  --output brief.md \
  --json-output brief.json \
  --trajectory-output trajectory.json \
  --no-llm
```

Run with Cloudflare AI Gateway after configuration:

```bash
export CLOUDFLARE_ACCOUNT_ID=...
export CLOUDFLARE_API_TOKEN=...
export VIGIL_CLOUDFLARE_GATEWAY_ID=...
export VIGIL_LLM_MODEL=openai/gpt-4.1

cargo run -p vigil-cli -- investigate \
  --alert examples/minimal/alert.yaml \
  --inventory examples/minimal/inventory.yaml \
  --runbook-dir examples/minimal/runbooks \
  --output brief.md
```

## Commands

```text
vigil investigate
vigil config check
vigil validate
vigil render
vigil version
```

See [docs/commands.md](docs/commands.md) for command details.

## Safety Boundary

Vigil does not execute shell commands, SSH into hosts, run target-side agents, mutate production, or perform remediation. Recommended checks are rendered as advisory read-only descriptions only.

## Documentation

Start with [docs/getting-started.md](docs/getting-started.md), then see:

```text
docs/configuration.md
docs/cloudflare-ai-gateway.md
docs/input-format.md
docs/output-format.md
docs/security-and-privacy.md
docs/troubleshooting.md
```
