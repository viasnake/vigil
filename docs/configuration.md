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
```

The default model is `openai/gpt-4.1`, routed through Cloudflare AI Gateway.

TOML config files are supported:

```toml
[cloudflare]
account_id = "..."
api_token = "..."
gateway_id = "..."
model = "openai/gpt-4.1"
request_timeout_secs = 30
retry_count = 1

[output]
format = "markdown"
```

Check configuration:

```bash
vigil config check --config vigil.toml
```

`--api-token` is supported for completeness, but environment variables or local config files are usually safer than shell history.
