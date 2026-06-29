# Troubleshooting

## Missing Cloudflare Account ID

Set `CLOUDFLARE_ACCOUNT_ID`, add `cloudflare.account_id` to the TOML config file, or pass `--account-id`.

## Missing Cloudflare API Token

Set `CLOUDFLARE_API_TOKEN`, add `cloudflare.api_token` to the TOML config file, or pass `--api-token`.

## Missing Gateway ID

Set `VIGIL_CLOUDFLARE_GATEWAY_ID`, add `cloudflare.gateway_id` to the TOML config file, or pass `--gateway-id`.

## Cloudflare 401 With Gateway Tokens

The default endpoint is `rest`, which calls `api.cloudflare.com` with `Authorization: Bearer ...`.

If your token is for an authenticated AI Gateway on `gateway.ai.cloudflare.com`, set:

```bash
export VIGIL_CLOUDFLARE_ENDPOINT=gateway
```

or pass `--endpoint gateway`.

## Invalid Input

Run:

```bash
vigil validate --alert alert.yaml --inventory inventory.yaml --runbook-dir runbooks
```

The error message includes the input category and file path.

## Existing Case Directory

`vigil case init` refuses to overwrite an existing case directory unless `--force` is supplied.

## Missing Case Manifest

`vigil investigate <case-dir>` expects `<case-dir>/vigil.yaml`. Create the case with `vigil case init` first.

## Ambiguous Investigation Input

Do not combine a case directory with file-mode flags such as `--alert` or `--inventory`.

Use one of these forms:

```bash
vigil investigate web-5xx
vigil investigate --alert alert.yaml --inventory inventory.yaml
```

## Invalid LLM Response

Vigil rejects model output that is not valid JSON, does not match the `ReasoningResult` schema, is not read-only, or includes obvious runnable shell-command text in recommended checks.

Use `--no-llm` to verify parsing and rendering without calling Cloudflare.
