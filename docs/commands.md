# Commands

## `vigil case init`

Creates a case workspace.

```bash
vigil case init web-5xx \
  --target service:web \
  --severity page \
  --summary "Web service 5xx responses are above threshold"
```

It creates:

```text
web-5xx/vigil.yaml
web-5xx/evidence/
web-5xx/runbooks/
web-5xx/output/
```

Use `--force` to overwrite the case manifest in an existing case directory.

## `vigil evidence add`

Adds a metric, log, operator observation, or other evidence item to a case.

```bash
vigil evidence add web-5xx \
  --kind metric \
  --summary "HTTP 5xx rate increased from 0.2% to 8.4%" \
  --source prometheus \
  --url "https://grafana.example.com/d/web"
```

File-backed evidence is also supported:

```bash
vigil evidence add web-5xx \
  --kind log \
  --summary "Backend timeout errors increased around the alert window" \
  --source loki \
  --file ./timeout-snippet.txt
```

## `vigil change add`

Adds change evidence to a case.

```bash
vigil change add web-5xx \
  --summary "Caddy upstream timeout setting changed before the alert" \
  --source github \
  --url "https://github.com/example/repo/pull/123"
```

## `vigil runbook add`

Validates a runbook and copies it into the case.

```bash
vigil runbook add web-5xx examples/minimal/runbooks/web-5xx.yaml
```

## `vigil investigate`

Investigates a case:

```bash
vigil investigate web-5xx
```

By default, case investigation writes:

```text
web-5xx/output/brief.md
web-5xx/output/brief.json
web-5xx/output/trajectory.json
```

The file-based workflow remains available:

```bash
vigil investigate \
  --alert examples/minimal/alert.yaml \
  --inventory examples/minimal/inventory.yaml \
  --runbook-dir examples/minimal/runbooks \
  --output brief.md
```

Shared options:

```text
--output <PATH>
--json-output <PATH>
--trajectory-output <PATH>
--config <PATH>
--model <MODEL>
--endpoint <ENDPOINT>
--gateway-id <GATEWAY_ID>
--account-id <ACCOUNT_ID>
--api-token <TOKEN>
--request-timeout-secs <SECONDS>
--retry-count <COUNT>
--dry-run
--no-llm
```

File-mode options:

```text
--alert <PATH>
--inventory <PATH>
--runbook <PATH>
--runbook-dir <PATH>
TARGET
```

If the positional argument is a case directory, do not combine it with file-mode flags.

`--no-llm` produces deterministic output for tests and local verification. `--dry-run` also avoids an LLM request.

`--endpoint` accepts `rest` or `gateway`. `rest` is the default Cloudflare REST API path. `gateway` uses the documented `gateway.ai.cloudflare.com` provider-native path.

## `vigil config check`

Validates Cloudflare settings and optional input/output paths.

## `vigil validate`

Parses and validates alert, inventory, and runbook files for the file-based workflow.

## `vigil render`

Renders a Markdown brief from a saved trajectory JSON file.

## `vigil version`

Prints the CLI version.
