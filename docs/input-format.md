# Input Format

Vigil supports YAML and JSON files for these input categories:

```text
alert
inventory
runbook
```

The file extension selects JSON parsing for `.json`; other supported example files use YAML.

## Alert

```yaml
id: web-5xx-rate
name: WebHigh5xxRate
severity: page
status: firing
summary: Web service 5xx responses are above the paging threshold.
target: service:web
started_at: "2026-06-29T00:00:00Z"
labels:
  service: web
annotations:
  dashboard: web-service-overview
source: example
```

## Inventory

```yaml
targets:
  - id: service:web
    kind: service
    name: web
    environment: production
    service: web
    criticality: high
```

## Runbook

```yaml
id: web-5xx
title: Web 5xx investigation
applies_to:
  - service:web
checks:
  - id: confirm-scope
    title: Review service-level error and latency dashboards
    description: Compare web error rate, latency, saturation, and request volume during the alert window.
    read_only: true
```

Runbook checks must be read-only. Vigil recommends them; it does not execute them.
