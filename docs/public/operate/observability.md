# Observability

## What to monitor

- historian file recording
- Prometheus metrics export
- allowlist-based variable recording
- web-path exposure of observability endpoints

## Good first checks

- confirm the endpoint exposing metrics is reachable
- confirm only the expected variables are recorded or exposed
- confirm historian retention/output paths are explicit

## Worked tutorial

--8<-- "examples/tutorials/23_observability_historian_prometheus/README.md"
