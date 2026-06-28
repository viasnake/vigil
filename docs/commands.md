# Commands

## `vigil investigate`

```bash
vigil investigate \
  --alert examples/minimal/alert.yaml \
  --inventory examples/minimal/inventory.yaml \
  --runbook-dir examples/minimal/runbooks \
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
--account-id <ACCOUNT_ID>
--api-token <TOKEN>
--request-timeout-secs <SECONDS>
--retry-count <COUNT>
--dry-run
--no-llm
TARGET
```

`--no-llm` produces deterministic output for tests and local smoke runs. `--dry-run` also avoids an LLM request.

## `vigil config check`

Validates Cloudflare settings and optional input/output paths.

## `vigil validate`

Parses and validates alert, inventory, and runbook files.

## `vigil render`

Renders a Markdown brief from a saved trajectory JSON file.

## `vigil version`

Prints the CLI version.
