# Phase 16 Bounded Fuzz Campaign

- Source revision: `185806fc536a1fc8bb9bea70a5e544d06b408706`
- Platform: `linux-x86_64` (`trust-builder`)
- Started: `2026-07-17T23:21:13+00:00`
- Finished: `2026-07-17T23:24:35+00:00`
- Requested cargo-fuzz runs per target: `10000`
- Per-target cargo-fuzz wall-time bound: `120` seconds
- Per-input timeout: `10` seconds
- Durable JSON: `p16-fuzz-campaign.json`
- JSON SHA-256: `05f6a974c8f4f3e9a3f5d2c8cb80bae0a6057f1a5b33b010d85252b811c7650b`

## Result

The clean retained builder worktree ran all 17 registered targets in program
order. All 11 cargo-fuzz targets exited zero without a crash artifact. Ten
completed 10,000 executions; `FUZZ_TARGET_HIR_SEMANTIC` completed 4,388 within
its 120-second bound. All six bounded Rust smokes exited zero and the runner
confirmed that each exact Cargo filter executed one test.

| Targets | Passed | Infrastructure failures | Crash artifacts | Regression handoffs |
| ---: | ---: | ---: | ---: | ---: |
| 17 | 17 | 0 | 0 | 0 |

The committed `verification/fuzz-crash-regressions.toml` registry is empty
because the accepted campaign produced no artifact. The campaign contract
requires every future observed artifact to join that registry and a mapped
deterministic regression; an unregistered artifact makes the campaign fail.

## Defects Found Before Acceptance

1. `FUZZ_TARGET_ADS_COMMAND_DISPATCH` no longer compiled after the ADS symbol
   snapshot interface moved to `Arc<SymbolSnapshot>`. Commit `dd052c99` repaired
   the fuzz host and the target then completed 10,000 executions.
2. Four library smokes used package-wide Cargo filters, causing Cargo to build
   unrelated integration-test binaries and exhaust the builder target. Commit
   `e68fe2c8` narrowed each command to one `--lib` or `--test` binary.
3. The WAN allowlist command copied a stale module path from
   `runtime_comms_fuzz_gate.sh`; Cargo exited zero after running zero tests.
   Commit `e4629c9a` made zero-test results fatal, exposing the defect, and
   commit `185806fc` corrected the live module path in both the gate and the
   reviewed program binding.

The disk-exhausted attempt and the zero-test attempt are not accepted campaign
evidence. The generated target was stopped and cleaned under the remote-builder
procedure before the final clean run.

## Boundaries

- This is one bounded campaign, not proof of universal crash freedom.
- No corpus-completeness, coverage-percent, invariant-proof, or spec-gap claim
  is created by the zero-artifact result.
- `VERIF-P9-005` closes only the enforced crash-to-regression handoff mechanism.
- No product/runtime behavior changed in the final campaign closure; the ADS
  and WAN fixes repair verification harness coverage.
- CI enforcement and suite definitions are unchanged.

## Verification

```text
python3 scripts/run_fuzz_campaign.py \
  --validate-existing docs/internal/testing/evidence/plc-verification-program/2026-07-18/p16-fuzz-campaign.json
sha256sum docs/internal/testing/evidence/plc-verification-program/2026-07-18/p16-fuzz-campaign.json
jq -e '.source_commit == "185806fc536a1fc8bb9bea70a5e544d06b408706" and .summary == {"crash_artifacts":0,"infrastructure_failures":0,"passed":17,"regressions":0,"targets":17} and all(.results[]; .exit_status == 0 and (.timed_out | not) and .executions > 0 and (.artifact_files | length) == 0)' docs/internal/testing/evidence/plc-verification-program/2026-07-18/p16-fuzz-campaign.json
```
