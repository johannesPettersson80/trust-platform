# Eighteenth Review Findings Intake

Date: 2026-07-18
Reviewed checkpoint: `7dc47e6523d96c4f7fcbec76be97793860027991`

This intake reopens `VERIF-P16-002`. It records findings before product fixes;
it is not proof that any finding is resolved.

## Registered product gaps

- H1: `SPEC_GAP_WATCHDOG_PARTIAL_SAFE_STATE_001` records pending-output
  leakage when only part of the safe-state image is configured.
- H2: `SPEC_GAP_RUNTIME_INTERNAL_NONFINITE_CONVERSION_001` records internal
  NaN/infinity synthesis through explicit numeric conversions.
- H3: `SPEC_GAP_EDITOR_FIELD_RENAME_CROSS_FILE_001` records the merged-project
  field-collision bypass.
- M1: `SPEC_GAP_UI_PEER_STATUS_FAILURE_001` records peer-topology validation
  failures that can disappear without a visible per-peer error.
- M3: `SPEC_GAP_DEV_COMMIT_ATOMIC_SCOPE_001` records canonical-path fail-open
  behavior and the check-to-commit Git-index race.
- M5: `SPEC_GAP_EDITOR_DOCUMENT_CLOSE_RACE_001` records stale in-flight cache
  publication after `didClose`.
- LOW simulation clock: `SPEC_GAP_SIM_CLOCK_OVERFLOW_001` records the undefined
  overflow policy.
- LOW LSP edit boundaries: `SPEC_GAP_LSP_INVALID_CHANGE_RANGE_001` records
  over-EOF incremental edits and range-format line-boundary consistency.
- LOW OPC UA server access: `SPEC_GAP_OPCUA_SERVER_WRITE_EXPOSURE_001` records
  variables that advertise `CurrentWrite` without a write-back contract.

## Reviewed dispositions

- M6 is not registered as a defect gap. The current runtime contract explicitly
  specifies bounded read-lane capacity and fail-visible `server_busy` overload;
  it does not promise wait-free reads. A stronger availability guarantee would
  be a separately reviewed product requirement.
- M2 is historical process debt. Do not rewrite history. Add the missing
  changelog entry for the debug mutation lifecycle fix and keep future product
  fixes tests-first and independently revertible.
- M4 is historical proof-process debt. Do not rewrite history. Future red proof
  must use an isolated red commit or worktree and must not temporarily revert a
  shipped safety fix on the main development line.
- M7 is a tooling defect: remove invocation-context bytes from the mutation
  report payload and apply one output-containment rule to every report family.
- `denial_code` versus `error_code` is retained as a surface-specific naming
  distinction until a shared public error-envelope requirement is reviewed.

No CI, suite, approved-proof-producer, skill, or agent-rule change is authorized
by this intake.
