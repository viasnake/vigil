# Vigil

Vigil is a Rust CLI for producing evidence-backed SRE investigation briefs from local case workspaces and structured input files.

The current implementation lets operators create a local case, add metric/log/change evidence and runbooks, build a redacted evidence packet, ask Cloudflare AI Gateway for structured reasoning, validate the response, and render Markdown, JSON, and trajectory output.

## Quick Start

Run a deterministic case investigation without Cloudflare credentials:

```bash
cargo run -p vigil-cli -- case init /tmp/web-5xx \
  --target service:web \
  --severity page \
  --summary "Web service 5xx responses are above threshold"

cargo run -p vigil-cli -- evidence add /tmp/web-5xx \
  --kind metric \
  --summary "HTTP 5xx rate increased from 0.2% to 8.4%" \
  --source prometheus \
  --url "https://grafana.example.com/d/web"

cargo run -p vigil-cli -- change add /tmp/web-5xx \
  --summary "Caddy upstream timeout setting changed before the alert" \
  --source github \
  --url "https://github.com/example/repo/pull/123"

cargo run -p vigil-cli -- runbook add /tmp/web-5xx examples/minimal/runbooks/web-5xx.yaml

cargo run -p vigil-cli -- investigate /tmp/web-5xx \
  --no-llm
```

Run with Cloudflare AI Gateway after configuration:

```bash
export CLOUDFLARE_ACCOUNT_ID=...
export CLOUDFLARE_API_TOKEN=...
export VIGIL_CLOUDFLARE_GATEWAY_ID=...
export VIGIL_LLM_MODEL=openai/gpt-4.1

cargo run -p vigil-cli -- investigate /tmp/web-5xx
```

For authenticated Gateway tokens or Workers AI models, use the Gateway endpoint:

```bash
export VIGIL_CLOUDFLARE_ENDPOINT=gateway
export VIGIL_LLM_MODEL=@cf/meta/llama-3.1-8b-instruct-fast

cargo run -p vigil-cli -- investigate /tmp/web-5xx
```

## Commands

```text
vigil investigate
vigil case init
vigil evidence add
vigil change add
vigil runbook add
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
