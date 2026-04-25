# Observability

## What to monitor

- historian file recording
- Prometheus metrics export
- allowlist-based variable recording
- web-path exposure of observability endpoints

Operational evidence over time: historian files, metrics, signal allowlists,
retention, and access policy.

## Good first checks

- confirm the endpoint exposing metrics is reachable
- confirm only the expected variables are recorded or exposed
- confirm historian retention/output paths are explicit

Success means the site can answer which signals are recorded, where the data is
retained, who may read it, and how the evidence helps diagnose a runtime issue
after the fact.

## Worked tutorial

--8<-- "examples/tutorials/23_observability_historian_prometheus/README.md:3"
