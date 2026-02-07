# HMI Reliability Bundle Fixture

This project folder is the tracked fixture for long read-only HMI soak runs.

Intended usage:

- `scripts/runtime_soak_test.sh` with `HMI_POLL_ENABLED=true`
- `.github/workflows/hmi-long-soak.yml`

The soak harness builds `program.stbc` from `sources/` before execution.
