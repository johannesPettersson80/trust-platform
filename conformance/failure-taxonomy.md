# Conformance Failure Taxonomy

`trust-runtime conformance` emits stable reason codes for machine parsing.

The v1 and v2 summary schemas share the same reason-code set.

## Codes

- `expected_missing`
  - Expected artifact file is missing for a discovered case.
  - Case status: `error`.

- `expected_mismatch`
  - Case executed, but actual artifact differs from expected artifact.
  - Case status: `failed`.

- `expected_read_error`
  - Expected artifact exists but cannot be read or parsed as JSON.
  - Case status: `error`.

- `expected_write_error`
  - Runner could not write expected artifacts in `--update-expected` mode.
  - Case status: `error`.

- `case_execution_error`
  - Case could not execute due to manifest/source/runtime execution failure.
  - Case status: `error`.

## Status Mapping

- `failed`: deterministic mismatch between expected and actual.
- `error`: execution or artifact handling failure.
