# Configuration

Vigil resolves configuration in this order:

```text
CLI flag > environment variable > config file > default
```

Required for LLM-assisted investigation:

```text
CLOUDFLARE_ACCOUNT_ID
CLOUDFLARE_API_TOKEN
VIGIL_CLOUDFLARE_GATEWAY_ID
```

Optional:

```text
VIGIL_LLM_MODEL
VIGIL_CLOUDFLARE_ENDPOINT
```

The default model is `openai/gpt-4.1`, routed through Cloudflare AI Gateway.
The default endpoint is `rest`. Set `VIGIL_CLOUDFLARE_ENDPOINT=gateway` to use Cloudflare's `gateway.ai.cloudflare.com` provider-native path, which is useful for authenticated Gateway tokens and Workers AI models such as `@cf/meta/llama-3.1-8b-instruct-fast`.

TOML config files are supported:

```toml
[cloudflare]
account_id = "..."
api_token = "..."
gateway_id = "..."
model = "openai/gpt-4.1"
endpoint = "rest"
request_timeout_secs = 30
retry_count = 1

[output]
format = "markdown"

[investigation]
max_iterations = 2
max_tool_calls = 8
max_duration_secs = 60

[sources.inventory.local]
path = "examples/minimal/inventory.yaml"

[sources.runbook.local]
dir = "examples/minimal/runbooks"

[sources.alertmanager.prod]
url = "https://alertmanager.example.com"
bearer_token_env = "ALERTMANAGER_TOKEN"

[sources.prometheus.prod]
url = "https://prometheus.example.com"
bearer_token_env = "PROMETHEUS_TOKEN"

[sources.github.main]
repo = "example/web"
bearer_token_env = "GITHUB_TOKEN"

[sources.http.web]
url = "https://web.example.com/health"

[sources.dns.default]

[sources.loki.prod]
url = "https://loki.example.com"
bearer_token_env = "LOKI_TOKEN"

[sources.grafana.prod]
url = "https://grafana.example.com"
bearer_token_env = "GRAFANA_TOKEN"

[sources.kubernetes.prod]
url = "https://kubernetes.example.com"
namespace = "default"
bearer_token_env = "KUBERNETES_TOKEN"
```

Check configuration:

```bash
vigil config check --config vigil.toml
```

`--api-token` is supported for completeness, but environment variables or local config files are usually safer than shell history.

Source configuration is used by target and alert investigation. `inventory` and `runbook` sources read local files. Network-backed adapters use configured URLs only and perform read-only GET requests. `fixture_path` remains supported for every external adapter where local or test data is preferable.

Use `bearer_token_env` to name an environment variable that contains an adapter bearer token. Vigil stores the environment variable name in trajectory metadata, not the token value.

Use `--source <kind:name>` to limit an investigation to selected configured sources, for example:

```bash
vigil investigate service:web --since 30m --source prometheus:prod
```
