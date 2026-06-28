# Output Format

`vigil investigate` can write three outputs:

```text
--output <PATH>              Markdown brief
--json-output <PATH>         JSON brief
--trajectory-output <PATH>   JSON trajectory
```

The Markdown brief includes:

```text
affected targets
observed evidence
hypotheses
missing checks
recommended read-only checks
risk notes
source references
warnings
```

The JSON brief is the serialized `EvidenceBrief` model.

The trajectory records the investigation inputs, resolved targets, redacted evidence packet, validated reasoning result, final brief, LLM metadata when used, warnings, and errors. It does not contain Cloudflare credentials.

Render a Markdown brief from a saved trajectory:

```bash
vigil render --trajectory trajectory.json --output brief.md
```
