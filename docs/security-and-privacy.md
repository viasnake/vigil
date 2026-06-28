# Security and Privacy

Vigil applies a basic redaction pass before sending an `EvidencePacket` to Cloudflare AI Gateway.

It masks:

```text
common password fields
common secret fields
common token fields
common API key fields
some token-like string values
```

The redaction report is stored in the evidence packet and trajectory. Redaction is best-effort and is not a substitute for reviewing inputs before sending them to an LLM.

Vigil does not:

```text
execute shell commands
SSH into hosts
run target-side agents
mutate production
perform remediation
store Cloudflare credentials in outputs
read raw environment variables into evidence packets
```

Invalid LLM responses are rejected before rendering when they fail schema validation, mark recommended checks as non-read-only, or include obvious runnable shell-command text in recommended checks.
