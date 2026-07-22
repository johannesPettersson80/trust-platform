# P18 task readiness slice

Date: 2026-07-22

Status: exact authority, mapping, strengthened legacy unit locks, generated
decision-table trace, and producer-authentic targeted behavior-lock proof
complete. Source-bound assertion reachability, formal mutation sensitivity,
fact-ledger refresh, and broad remote proof remain open. No Phase 18 global row
or stop gate is closed by this slice.

## Scope and authority

- Phase 18 slice:
  `P18-CODE-CODE_9081A7CEC84BFEF134E282B9DE3838334C15CBA557479C25AF9F230C205C2A73`
- Exact code fact:
  `CODE_9081A7CEC84BFEF134E282B9DE3838334C15CBA557479C25AF9F230C205C2A73`
- Exact symbol: `trust_runtime_core::task::evaluate_task_readiness`
- Reviewed body digest:
  `sha256:c374b4c941bff296e1358893263d6c8edea1502000ca6b53241845b78b2aa358`
- Oracle: `SPEC_RUNTIME_ENGINE_001#task-readiness-and-overrun-accounting`
- Invariant: `RT_TASK_READINESS_001`

IEC 61131-3 Edition 3 section 6.8.2 a-b requires one scheduling event for a
`SINGLE` rising edge and periodic scheduling for non-zero `INTERVAL` while
`SINGLE` is zero. It leaves interval resolution and deadline information
implementer-specific and does not define truST's due timestamp, missed count,
clock-discontinuity behavior, or cumulative overrun arithmetic. `DEV-058`
therefore owns that product boundary: event samples do not move the periodic
baseline, non-positive portable intervals are disabled, backward samples leave
state unchanged, and forward jumps coalesce to one activation with exact
missed-period and saturating-overrun accounting. The production body did not
change.

## Executable contract

Five pre-existing unit tests moved from `reviewed_nonmapping` to explicit
catalog contracts and were strengthened without renaming their discovery
identity:

- `TEST_RUNTIME_TASK_READINESS_PERIODIC_OVERRUN_001` covers exact periodic due,
  multi-interval coalescing, and zero/negative interval disablement.
- `TEST_RUNTIME_TASK_READINESS_EVENT_EDGE_001` covers rising, held, falling
  rearm, and second rising samples.
- `TEST_RUNTIME_TASK_READINESS_EVENT_SUPPRESSION_001` proves that a high
  `SINGLE` sample at and after the deadline suppresses periodic baseline and
  overrun mutation.
- `TEST_RUNTIME_TASK_READINESS_BACKWARD_CLOCK_001` covers backward, baseline,
  pre-deadline, and exact next-deadline samples.
- `TEST_RUNTIME_TASK_READINESS_FORWARD_COALESCING_001` covers one activation,
  first nominal due time, exact skipped intervals, baseline advancement, and
  cumulative `u64::MAX` saturation.

`TEST_RUNTIME_TASK_READINESS_TRACE_001` executes the same reviewed boundary
through the shared `verification-cases` artifact contract. Its generated case
file is `verification/cases/runtime_safety/RT_TASK_READINESS_001.toml`, digest
`sha256:ee200d7df31c1f80acbd550a76fc28ac80ffb638339383f46b51e86e2aeaa8fe`.
It regenerates exactly with `gen_cases_v2.py v1`.

The scanner identities are `DISC_6F2849AC1484DA9AB8A8`,
`DISC_F9FC5A9157982659A367`, `DISC_A8E6873E91C94109A6C0`,
`DISC_0D4BF89810AFBF92D9ED`, `DISC_CD6A594B4D0B29C8C3AD`, and
`DISC_FAE2E483B2B2C80C04A2`. The full scan contains 4,049 facts; the
staleness join validates 285 committed catalog records against that
denominator.

## Producer-authentic lock proof

The isolated builder checkout was
`trust-builder:/home/johannes/projects/trust-platform-p18-substrate-186b9359`.
Before each HEAD transition, canonical
`/home/johannes/projects/trust-platform/AGENTS.md` and the complete
`.codex/skills/**` tree were manually copied to the destination. Their hashes
matched as `d0b35bc3edf19dc1d276202b6058ac6ad2835c6552fafbe71c04eb262a88530e`
and `a595ab028d85241bc031148f44604245e027d9c320d858150bc460aaf740b25f`.

The baseline ran from clean source commit
`326fa56d87b8b354b1402a96962f545b2a4d1762`; the comparison ran from clean
descendant commit `bb89c04ddff107bf3e7993d41d3da34af101c1ce`.

- Baseline: `EVID_TEST_RUNTIME_TASK_READINESS_TRACE_001_LOCK_BASELINE`
- Comparison: `EVID_TEST_RUNTIME_TASK_READINESS_TRACE_001_LOCK_COMPARE`
- Proof contract digest:
  `sha256:f31a77ed4844a1d2cfabde248e63df9c34e0d4ec372b000e2f982622230f33b9`
- Stable case-result digest:
  `sha256:62ca60da30ba6b56eff4ed0106e3e91e6201644732e54b3a4d19f1c51134a1bc`
- Baseline artifact digest:
  `sha256:18446efb8da969bc8264376b7d4b92190867aa760a4a16e3746e192790c23157`
- Comparison artifact digest:
  `sha256:c8737fdf12af8c3106baed23591355d096bcfd29c84d1148f37a429bbf2408a7`
- Result: all seven generated partitions passed in both runs; no case was
  blocked, skipped, filtered out, or failed.

The first formal baseline attempt was rejected because the reused target cache
contained `verification-cases` metadata with an absolute worktree case path.
Cleaning only that helper package removed 16 MiB of generated cache and forced
the accepted repository-relative artifact path. The rejected attempt produced
no evidence record.

Focused x86_64 Linux proof also passed all five exact task-readiness unit tests,
the exact generated trace command, case regeneration, 915-record pre-lock
metadata validation, and the 285-to-4,049 catalog staleness join. Full `clippy`
and `test-all` were not run; the active board reserves them for the one final
`VERIF-P18-016` candidate.

## Exact risk acknowledgement

`RISK_ACK_9081A7CEC84BFEF134E282B9DE3838334C15CBA557479C25AF9F230C205C2A73`
acknowledges the scanner's `state_mutation` signal for the exact unchanged body
digest. It is bound to IEC trigger authority, `DEV-058`, the invariant, five
strengthened unit locks, and the producer-authentic generated trace pair. It
does not suppress the signal or claim the still-open assertion reachability,
mutation budget, refreshed ledger, or broad-gate proof.

## Architecture and remaining debt

The portable readiness function remains in `trust-runtime-core`; the host
scheduler continues to own clock injection, task ordering, metrics, and
execution. The new trace adapter reuses `verification-cases` and introduces no
production ownership, data-flow, public API, or diagram change. This satisfies
the SOLID/KISS/DRY acceptance check: one normative decision table, five existing
focused consumers, and one generated trace at the owning boundary.

Remaining exact debt:

- source-bound assertion reachability under global `VERIF-P18-007`;
- mutation sensitivity and zero viable survivors under `VERIF-P18-009`;
- refreshed fact-ledger accounting for the exact code and test identities;
- causal broad evidence and final frozen-candidate gates under
  `VERIF-P18-016`;
- all other code/test facts and every global Phase 18 equation.
