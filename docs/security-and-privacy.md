# Security and Privacy

Vigil applies a basic redaction pass before sending an `EvidencePacket` to Cloudflare AI Gateway.

It masks:

```text
common password fields
common secret fields
common token fields
common API key fields
some token-like string values
common inline secret assignments such as `api_token=...`
```

The redaction report is stored in the evidence packet and trajectory. Redaction is best-effort and is not a substitute for reviewing inputs before sending them to an LLM.

Raw supplied evidence can still contain untrusted text after secret redaction. Vigil treats that text as evidence, not instructions, and validates recommended checks before rendering, but operators should review generated outputs before sharing them.

Vigil does not:

```text
execute shell commands
SSH into hosts
run target-side agents
mutate production
perform remediation
execute unregistered tools
store Cloudflare credentials in outputs
read raw environment variables into evidence packets
```

Target and alert investigation may execute registered read-only adapter capabilities. Every planned call is policy-validated against the local capability registry and recorded in the trajectory. `--plan-only` prints the proposed read-only calls without executing adapters.

Adapter bearer tokens are configured by environment variable name through `bearer_token_env`. Vigil reads the token when making the request, but records only the environment variable name, not the token value.

Invalid LLM responses are rejected before use when they fail `ToolPlan` or `ReasoningResult` schema validation, reference unregistered capabilities, mark recommended checks as non-read-only, or include obvious runnable shell-command text.
