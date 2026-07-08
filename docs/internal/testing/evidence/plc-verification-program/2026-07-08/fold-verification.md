# Fold Verification (VERIF-P0-008)

Date: 2026-07-08. Performed by the reviewer after the fold pass.

## Method

Re-read all seven documents post-fold, mapped every review finding
(V-01..V-15, see `review-verdict.md`) to its folded location, applied residual
fixes where the fold missed a detail, and ran doc-consistency checks.

## Residual Fixes Applied During Verification

1. `metadata-model.md`: invariant example TOML reordered. `proof_level`,
   `source_refs`, `tests`, `gates`, `evidence_refs`, `failure_modes`, and
   `missing` moved above `[spec]`/`[oracle]`/`[[coverage.cells]]`; the previous
   layout nested them under the most recently opened table, so every fixture
   copied from the example would have been missing its required top-level
   fields. A scoping note was added below the example.
2. `wrong_result` added to the validated/coverage blocking rules in
   `metadata-model.md` (fail-closed list), `test-taxonomy.md` (coverage rules),
   and `implementation-board.md` (`VERIF-P6-007`); the risk value existed but
   the three blocking rules still listed only safety-critical,
   silent-corruption, and false-status.
3. `metadata-model.md`: evidence `commit` format normalized to
   `dirty:<base-commit>`; evidence rules added (`lab_report` requires
   `device_model`, `firmware`, `topology`, `env_vars`; `committed_file` paths
   must be git-tracked; `ci_artifact` refs name workflow/run/artifact and note
   retention).
4. `test-taxonomy.md`: `time_or_clock_fault` added to the coverage dimensions
   (the fault-injection taxonomy in `VERIF-P8-001` had clock classes, but the
   per-invariant matrix could not track them).
5. `verification-areas.md`: seam Owns list punctuation fixed (mid-list period
   after "runtime value semantics").
6. `implementation-board.md`: `VERIF-P3-006` and `VERIF-P6-007` now reference
   the grace-period definition row `VERIF-P14-000`.

## Consistency Checks

- Duplicate checkbox IDs: none. `VERIF-REVIEW-004` appears twice by design -
  once as the status-header cross-reference, once as the row definition.
- Duplicate invariant seed IDs in `verification-areas.md`: none.
- Evidence root gitignore: `docs/internal/testing/evidence/plc-verification-program/**`
  is not ignored (negations at `.gitignore:147-148`); `git check-ignore`
  exits 1 for files under it.
- Program document tree is un-ignored (`.gitignore` `!docs/internal/testing/checklists/plc-verification-program/` block)
  and currently untracked (`??`) pending first commit.
- Link targets verified to exist: `docs/specs/12-bytecode.md`,
  `docs/internal/architecture/runtime-safety-fail-closed-contract.md`,
  `conformance/contract.md`, `../plc-verification-program-checklist.md`;
  referenced test files (`bytecode_validation`, `runtime_safety_fail_closed`,
  `retain_integrity`, `runtime_restart`, `process_image`, `io_cycle`,
  `io_multidriver_live`, `opcua_integration`, `opcua_client_runtime`,
  `ads_cli_command`, `ads_web_api`, `ethercat_driver`, `modbus_driver`,
  `hmi_readonly_integration`, `api_smoke`, `complete_program`,
  `runtime_reliability`, `debug_control`, `debug_stepping`, `hot_reload`,
  `iec_timers`); gate scripts (`runtime_comms_conformance_gate.sh`,
  `prepush_ci_gate.sh`, `runtime_device_in_loop_gate.sh`,
  `runtime_vm_malformed_bytecode_fuzz_gate.sh`,
  `runtime_vm_determinism_reliability_gate.sh`).
- Machine-status vocabularies (record statuses, `source_status`,
  `resolution_status`, coverage cell states, ignored-test classes) are each
  defined once in `metadata-model.md`; no hyphenated status values remain in
  TOML examples.
- `scripts/validate_verification_metadata.py` does not exist yet - expected;
  it is `VERIF-P1-011`.

## Post-Fold Line Counts

| Document | Lines |
| --- | --- |
| README.md | 126 |
| policy.md | 366 |
| metadata-model.md | 678 |
| test-taxonomy.md | 328 |
| verification-areas.md | 383 |
| implementation-board.md | 469 |
| fable-review-brief.md | 103 |
| ../plc-verification-program-checklist.md | 77 |
| Total | 2530 |

## Result

All fifteen findings folded and verified. `VERIF-REVIEW-002` satisfied;
`VERIF-REVIEW-004` may be cleared.
