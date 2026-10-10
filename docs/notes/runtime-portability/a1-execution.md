# A1 portable foundations: execution and closeout

A1 is verified for its checklist allocation, **not push-ready**. All 24 planned run-3 steps passed. Run 3 validated the then-uncommitted source on `feat/runtime-portability-a1`, based on `9a15065725c17da2c912055f1509368d3fd01d6c`; the same source was committed on that branch afterwards (see the checklist checkpoint). The primary checkout held older specifications and source at that time.

## Inspectable evidence

- [Batch ledger](a1-evidence/run-3-ledger.tsv), [exact commands](a1-evidence/run-3-commands.txt), [environment](a1-evidence/run-3-environment.txt), [result](a1-evidence/run-3-result.txt).
- [Runtime test output](a1-evidence/run-3-runtime-tests.txt) and [core test output](a1-evidence/run-3-core-tests.txt).
- [Frozen 73-file snapshot](a1-evidence/frozen-run3.json), [copied-artifact digests](a1-evidence/artifact-sha256.json), and [review messages](a1-evidence/review-record.md).
- [Evidence interpretation and limitations](a1-evidence/README.md).

Run 3 used Rust 1.95.0, edition 2021 and resolver 2 on the Linux x86_64 builder. MCU checks used separate `thumbv7em-none-eabihf` and `riscv32imac-unknown-none-elf` invocations. All protected files matched locally and remotely after that batch, **before the later documentation/metadata correction**. The retained manifest remains the tested identity; it is not a hash claim about the subsequently edited bookkeeping.

## Implemented foundations

| Area | Source and native evidence | Remaining qualification |
|---|---|---|
| Nominal deadlines | `trust-runtime-core/src/task/readiness.rs`; core readiness/case tests; `tasks::periodic_task_keeps_its_interval_when_cycles_start_late` and `tasks::periodic_task_non_multiple_interval_preserves_nominal_deadlines` pass. A 25 ms task on exact 10 ms logical samples activates 40 times through 1000 ms with zero overruns. | A4 fixture integration and physical/platform parity. |
| Portable dependencies/storage | Workspace std defaults/host opt-ins; core `collections::OrderedMap`; `portable_foundations`; both MCU core checks. | Portable loader/executor extraction, firmware linkage and bounded storage. |
| Numerics | Shared core `numeric::math` routes libm 0.2.16; `portable_numeric_contract`, standard-library suites and VM behavior locks pass. Duration/scalar lowering contexts are separate, including aliases and nested factors. | Full saved-artifact traces and target qualification; no universal bit-identity claim. |
| Layout | Host Value slot 40 bytes, alignment 8; MCU compile-time guards enforce <=32 bytes/alignment <=8. | Actual firmware memory/stack/scan measurements. |
| CI | Separate portable-core matrix jobs, required result markers and gate-inventory entry. | GitHub execution is unrun. |

EXP(1) differs by one binary64 ULP under libm within the specified tolerance; the other nine recorded f64 corpus values match the old std reference. Native controls and exceptional-value assertions passed. This finite corpus does not establish every mathematical input or physical target state.

## Batches and reviews

| Run | Disposition | Findings and resolution |
|---|---|---|
| 1 | [Failed ledger](a1-evidence/run-1-ledger.tsv) | Invalid signed-duration syntax in the fixture; existing MySQL lint warning; locked Salsa/rustls advisories; stale allocation metadata phrase. Corrected before the separately authorized second batch. Cargo stopped the hosted test invocation early; later selected suites were unrun. |
| 2 | [Failed ledger](a1-evidence/run-2-ledger.tsv) | Corrected literal exposed shared duration operand-context lowering defect. Three LSP lint warnings and advisory metadata wording failure remained. Corrected and independently reviewed before the separately authorized third batch. |
| 3 | [Passed ledger](a1-evidence/run-3-ledger.tsv) | All 23 required and one advisory steps pass. No automatic retry or fourth batch. |

The available independent-review messages and exact review identities are [retained separately](a1-evidence/review-record.md). The review before run 3 found no actionable issue; native run 3 then exercised that source. Initial-review transcript availability and author-recorded authorization provenance are explicitly limited there.

Run-3 native counts: core 130; no-default foundations 5; HIR/IDE/browser analysis 1446 (one pre-existing ignored doc test); runtime integrations 127 across 14 binaries; mesh 15; security configuration 24; LSP scope contracts 27. Four focused metadata tests passed; unchanged full-history unit assertions retain run 2's 22-test result. Clippy, host/Windows cross-target warnings, supply chain, architecture, diagrams, CI observability and target assembly all pass.

## Bookkeeping correction after closeout review

The first closeout incorrectly described the detailed requirement ledger as updated: only A1 task boxes and checkpoint text had been closed. Its batch-history section and native-assertion notes were stale. Those records are now corrected, preserving partial evidence and all future requirements as open.

The scheduling gap now references registered tests and a dedicated nominal-deadline invariant, with repository-file evidence linked. The old test identifier and readiness invariant existed only in test/case artifacts, not the registries, and were not valid references. The new CI job is inventoried. The invariant remains S0 and the evidence is `proof_kind = none`: no producer-authenticated red/green/lock proof or hardware proof was manufactured.

These documentation/metadata changes received independent source review with no unresolved findings after two CI inventory/suite binding corrections. They were not run through a validator or test batch. The full report-census suite has historical drift, and this note does not claim that all metadata is clean. A1's original bookkeeping also added errors, including the missing CI inventory and orphan gap references; these edits address those causes but are not a fresh validator result.

## Before commit or push

No commit, push or A2 implementation is authorized by this closeout. A1 did not run the runtime crate's full unit suite, complete LSP/debug suites, or `just test-all`. Commit/push preparation must review the full diff and current instructions, include all intended new files, and complete required pre-push/release checks. The first-party version change is unreleased.

The rustls security update also updated native `aws-lc-sys`. Native Windows/macOS builds remain unverified; the Windows cross-check is not a native-platform build/test result. Physical NUCLEO-F401RE/C6 tests, STBC 2.0 and compiler-free engine integration remain later scopes. A1 does not establish zero bugs or functional-safety certification.
