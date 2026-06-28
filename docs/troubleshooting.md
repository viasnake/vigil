# Troubleshooting

## Missing Cloudflare Account ID

Set `CLOUDFLARE_ACCOUNT_ID`, add `cloudflare.account_id` to the TOML config file, or pass `--account-id`.

## Missing Cloudflare API Token

Set `CLOUDFLARE_API_TOKEN`, add `cloudflare.api_token` to the TOML config file, or pass `--api-token`.

## Missing Gateway ID

Set `VIGIL_CLOUDFLARE_GATEWAY_ID`, add `cloudflare.gateway_id` to the TOML config file, or pass `--gateway-id`.

## Invalid Input

Run:

```bash
vigil validate --alert alert.yaml --inventory inventory.yaml --runbook-dir runbooks
```

The error message includes the input category and file path.

## Invalid LLM Response

Vigil rejects model output that is not valid JSON, does not match the `ReasoningResult` schema, is not read-only, or includes obvious runnable shell-command text in recommended checks.

Use `--no-llm` to verify parsing and rendering without calling Cloudflare.
