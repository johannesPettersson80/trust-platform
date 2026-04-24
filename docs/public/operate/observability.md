# Observability

## What to monitor

- historian file recording
- Prometheus metrics export
- allowlist-based variable recording
- web-path exposure of observability endpoints

Use this page when you need operational evidence over time instead of a single
runtime status snapshot. The tutorial below shows the first historian/metrics
path; production use still needs a site retention and access policy.

## Good first checks

- confirm the endpoint exposing metrics is reachable
- confirm only the expected variables are recorded or exposed
- confirm historian retention/output paths are explicit

Success means the site can answer which signals are recorded, where the data is
retained, who may read it, and how the evidence helps diagnose a runtime issue
after the fact.

## Worked tutorial

--8<-- "examples/tutorials/23_observability_historian_prometheus/README.md:3"
