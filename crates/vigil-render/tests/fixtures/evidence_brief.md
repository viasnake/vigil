# Investigation Brief: web

Elevated 5xx responses are affecting the web service.

## Affected Targets

- `service:web` - service - web - env: prod - criticality: high
## Observed Evidence

- `alert:web-5xx` - Alert: 5xx rate is above threshold (source: file:alert (alert.yaml)), target: `service:web`
## Hypotheses

- `hyp-1` - Dependency errors (confidence 0.50): A downstream dependency may be failing.
  - Supporting evidence: alert:web-5xx
  - Risk if wrong: The team may inspect the wrong dependency.
## Missing Checks

- No missing checks were identified.
## Recommended Read-Only Checks

- `check-1` - Review dependency dashboard: target: `service:web`; Compare dependency errors with the alert window. Reason: The check is read-only and validates the hypothesis.. Source: test. Not executed by Vigil.
## Risk Notes

- Hypotheses are not facts.
## Warnings

- Recommended checks were not executed.
