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

The trajectory records the investigation inputs, configured sources, registered capabilities, investigation loop, planned read-only tool calls, tool results, resolved targets, redacted evidence packet, validated reasoning result, final brief, LLM metadata when used, warnings, and errors. It does not contain Cloudflare credentials.

For case investigation, default output paths are:

```text
<case-dir>/output/brief.md
<case-dir>/output/brief.json
<case-dir>/output/trajectory.json
```

Explicit output flags override these defaults.

For target and alert investigation, default output paths are:

```text
output/brief.md
output/brief.json
output/trajectory.json
```

`--plan-only` prints a Markdown plan and does not write output files.

Render a Markdown brief from a saved trajectory:

```bash
vigil render --trajectory trajectory.json --output brief.md
```
