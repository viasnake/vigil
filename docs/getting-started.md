# Getting Started

Build and run the CLI from the workspace:

```bash
cargo run -p vigil-cli -- version
```

Validate the minimal example:

```bash
cargo run -p vigil-cli -- validate \
  --alert examples/minimal/alert.yaml \
  --inventory examples/minimal/inventory.yaml \
  --runbook-dir examples/minimal/runbooks
```

Generate a deterministic brief without Cloudflare credentials:

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

For LLM-assisted output, configure Cloudflare AI Gateway first, then omit `--no-llm`.
