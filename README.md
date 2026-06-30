# Vigil

Vigil is a Rust CLI for bounded read-only SRE investigation.

The current implementation can start from a target or alert, plan registered read-only collection, collect file-backed/mock fixture evidence from configured sources, build a redacted evidence packet, ask Cloudflare AI Gateway for structured reasoning, validate the response, and render Markdown, JSON, and trajectory output.

## Quick Start

Inspect a target investigation plan without Cloudflare credentials:

```bash
cargo run -p vigil-cli -- investigate service:web \
  --since 30m \
  --plan-only \
  --no-llm
```

Run a deterministic target investigation:

```bash
cargo run -p vigil-cli -- investigate service:web \
  --since 30m \
  --no-llm
```

By default this writes:

```text
output/brief.md
output/brief.json
output/trajectory.json
```

Local case workspaces are still supported:

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

Vigil does not execute shell commands, SSH into hosts, run target-side agents, mutate production, or perform remediation. Agent tool calls are limited to registered read-only capabilities and are recorded in the trajectory.

Alertmanager, Prometheus, GitHub, HTTP, DNS, Loki, Grafana, and Kubernetes adapters are read-only. Network-backed adapters use configured URLs only; fixtures remain supported for tests and local dry runs.

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
