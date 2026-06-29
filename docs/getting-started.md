# Getting Started

Build and run the CLI from the workspace:

```bash
cargo run -p vigil-cli -- version
```

Create a local investigation case:

```bash
cargo run -p vigil-cli -- case init /tmp/web-5xx \
  --target service:web \
  --severity page \
  --summary "Web service 5xx responses are above threshold"
```

Add evidence:

```bash
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
```

Generate deterministic outputs without Cloudflare credentials:

```bash
cargo run -p vigil-cli -- investigate /tmp/web-5xx --no-llm
```

This writes:

```text
/tmp/web-5xx/output/brief.md
/tmp/web-5xx/output/brief.json
/tmp/web-5xx/output/trajectory.json
```

For LLM-assisted output, configure Cloudflare AI Gateway first, then omit `--no-llm`.

The file-based workflow remains available:

```bash
cargo run -p vigil-cli -- investigate \
  --alert examples/minimal/alert.yaml \
  --inventory examples/minimal/inventory.yaml \
  --runbook-dir examples/minimal/runbooks \
  --no-llm
```
