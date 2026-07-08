# Conformance Contract

This document defines what the conformance suite asserts and how pass/fail is
reported.

## Assertion Scope

Each conformance case asserts deterministic behavior for one category.

### v1 Categories

The v1 summary contract is frozen to:

- `timers`: TON/TOF/TP timing semantics under fixed scan cycles.
- `edges`: rising/falling edge detection behavior.
- `scan_cycle`: scan ordering and update visibility across cycles.
- `init_reset`: initialization and reset behavior (including retentive checks).
- `arithmetic`: numeric corner-case behavior for supported operations.
- `memory_map`: mapped address behavior and visibility rules.

### v2 Categories

The v2 summary contract keeps all v1 categories and adds:

- `strings`: STRING/WSTRING-compatible runtime value behavior covered by the
  suite.
- `arrays`: fixed-bound ARRAY indexing, mutation, and whole-value encoding.
- `structs`: STRUCT initializers, field reads, and field writes.
- `enums`: ENUM initializers, assignments, comparisons, and value encoding.
- `nested_values`: nested STRUCT/ARRAY access paths and encoding.
- `oop_dispatch`: method dispatch, interface dispatch, inheritance overrides,
  and `SUPER` calls.
- `references`: `REF_TO` initialization, dereference reads, and dereference
  writes.
- `retain_matrix`: cold/warm/hot/fault/download restart labels mapped to the
  runtime's implemented retain behavior.
- `scheduler`: deterministic task scheduling under scripted virtual time.
- `comms_determinism`: simulated connector status transitions projected
  through the shared connector status model. These cases are loopback/simulated
  only and never use live sockets or hardware.

## Pass/Fail Rules

- `passed`: runtime output/state exactly matches the expected artifact.
- `failed`: runtime executed but output/state differs from expected artifact.
- `error`: case could not be executed or evaluated (compile/runtime/harness error).
- `skipped`: case intentionally not executed by the runner (reserved for matrix runs).

Determinism requirement:

- Runner ordering is stable and deterministic (`case_id` ascending).
- Re-running the same input set with the same runtime and config must produce
  identical result ordering and equivalent status classification.

## Summary JSON Contract

The machine-readable summary is JSON and must validate against:

- `conformance/schemas/summary-v1.schema.json` for v1 summaries
- `conformance/schemas/summary-v2.schema.json` for v2 summaries

Core fields:

- `version`: fixed integer (`1` or `2`)
- `profile`: fixed string (`trust-conformance-v1` or `trust-conformance-v2`)
- `generated_at_utc`: RFC3339 timestamp
- `ordering`: fixed string (`case_id_asc`)
- `runtime`: runtime metadata (`name`, `version`, optional `target`)
- `summary`: totals (`total`, `passed`, `failed`, `errors`, `skipped`)
- `results`: per-case outcomes with deterministic ordering

Per-case required fields:

- `case_id`
- `category`
- `status`
- `expected_ref`

Per-case optional fields:

- `actual_ref`
- `duration_ms`
- `cycles`
- `reason` (`code`, `message`, optional `details`)

`reason.code` values are fixed and machine-parseable in v1 and v2:

- `expected_missing`
- `expected_mismatch`
- `expected_read_error`
- `expected_write_error`
- `case_execution_error`

Failure semantics:

- `failed` means the case executed and an expected artifact exists, but the
  actual artifact does not match expected (`expected_mismatch`).
- `error` means the case or expected-artifact handling could not be completed
  deterministically.

See `conformance/failure-taxonomy.md` for details.

## Compatibility

- `summary-v1.schema.json` is not mutated by v2 expansion.
- A suite containing only v1 categories emits `version = 1` and
  `profile = "trust-conformance-v1"`.
- A suite containing any v2-only category emits `version = 2` and
  `profile = "trust-conformance-v2"`.
- Generated human and machine reports are CI artifacts; committed expected
  artifacts remain under `conformance/expected/`.

## Example Summary

```json
{
  "version": 1,
  "profile": "trust-conformance-v1",
  "generated_at_utc": "2026-02-10T12:00:00Z",
  "ordering": "case_id_asc",
  "runtime": {
    "name": "trust-runtime",
    "version": "0.4.0"
  },
  "summary": {
    "total": 2,
    "passed": 1,
    "failed": 1,
    "errors": 0,
    "skipped": 0
  },
  "results": [
    {
      "case_id": "cfm_timers_ton_basic_delay_001",
      "category": "timers",
      "status": "passed",
      "expected_ref": "expected/timers/cfm_timers_ton_basic_delay_001.json",
      "duration_ms": 1,
      "cycles": 12
    },
    {
      "case_id": "cfm_timers_ton_reset_mid_cycle_002",
      "category": "timers",
      "status": "failed",
      "expected_ref": "expected/timers/cfm_timers_ton_reset_mid_cycle_002.json",
      "actual_ref": "reports/actual/cfm_timers_ton_reset_mid_cycle_002.json",
      "duration_ms": 1,
      "cycles": 12,
      "reason": {
        "code": "expected_mismatch",
        "message": "Q output mismatched at cycle 9"
      }
    }
  ]
}
```
