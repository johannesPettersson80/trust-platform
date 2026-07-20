# Phase 2 Report Replay Review Fixes

Date: `2026-07-10`

Implementation/source commit:
`3e207af163703fda7cbfae3c2bc7cbef9e643f87`

This evidence records the review hardening applied after the accepted
`VERIF-P2-007` through `VERIF-P2-010` slice. It is adequacy evidence with
`proof_kind = "none"`; it changes no coverage state, proof claim, spec gap, or
CI enforcement.

## Fixes

- The four report evidence commands now join generation and at-rest validation
  with `&&`. A generator refusal therefore cannot be hidden by validation of a
  previously generated artifact.
- `verification/README.md` states that each report regeneration needs a
  pristine source tree. Multiple reports must use separate clean worktrees or
  restore the previous report outputs before the next generator starts.
- The unmapped-debt canonical command now rejects a missing timestamp instead
  of exposing an unreachable timestamp-free shape.
- The report input path guard no longer carries a no-op symlink flag; the
  existing fixtures continue to require both symlink and workspace-escape
  diagnostics where applicable.

## Tests First

`test_default_command_rejects_missing_timestamp` failed before the CLI change:

```text
AssertionError: ValueError not raised
```

After implementation:

```text
python3 -m unittest \
  scripts.verification.test_catalog_debt_tests \
  scripts.verification.report_input_contract_tests
```

Result: 16/16 passed in 43.559 seconds.

The review's dirty-worktree replay probe was also repeated with the corrected
coverage command. The generator refused the dirty commit and the `&&` chain
returned nonzero before the validator could read the existing artifact:

```text
coverage-matrix gap report failed: commit must identify a clean full Git SHA
```

## Clean Report Refresh

All reports were generated with timestamp `2026-07-10T10:31:03Z` in a clean
detached worktree at the source commit above. After each report was generated,
validated, and copied to staging, its tracked Markdown and ignored JSON output
were restored or removed. The next generator therefore began from the same
pristine commit.

| Report | Generated JSON SHA-256 |
| --- | --- |
| Test-class completeness | `06b4834fbfa719359bccdc30646df4edef7f90cd4f8217f7538f39ca8a683264` |
| Coverage-matrix gaps | `cad5cd5859aca7d0605d86c678ad7b33c926ffcd4447ed96deba1cfa374f5ff8` |
| Malformed-input coverage | `cfac77685745f269b5bddab1c2f54cbff25a214c766984cc33efc9919e45e14a` |
| Unmapped-test debt | `37f0bed416996ce1868c16c8df40da67784e7d8a2202d179ac019006b4c8cc3d` |

Semantic results are unchanged: 1/3,816 scanner facts classified, 2/5
required test classes complete, 64/80 coverage slots missing, malformed-input
states 1 covered / 2 gap_open / 25 spec_gap, and 3,815/3,816 scanner facts in
report-only unmapped debt.

## Preserved Boundaries

- All 10 spec gaps remain open.
- `VM_SEAM_VALID_001` remains `spec_gap` at proof level `S0`.
- `VERIF-P1B-012`, `VERIF-P1B-014`, `VERIF-P2A-001` onward,
  `VERIF-P10-001`, and `VERIF-P10-003` remain open.
- No runtime, product, workflow, skill, agent-rule, or enforcement file changed.
