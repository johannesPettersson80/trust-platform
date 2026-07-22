# P18 fault-policy decision slice

Date: 2026-07-22

Status: exact authority, mapping, strengthened core and host locks, generated
decision-table trace, and producer-authentic targeted behavior-lock proof
complete. Source-bound assertion reachability, formal mutation sensitivity,
fact-ledger refresh, and broad remote proof remain open. No Phase 18 global row
or stop gate is closed by this slice.

## Scope and authority

- Phase 18 slice:
  `P18-CODE-CODE_E78D5BAEDF1B5EE0A6C2DEA95C1E40ADF75CADF7F3FE427F6BDA5738D8453BC8`
- Exact code fact:
  `CODE_E78D5BAEDF1B5EE0A6C2DEA95C1E40ADF75CADF7F3FE427F6BDA5738D8453BC8`
- Exact symbol: `trust_runtime_core::watchdog::FaultSubsystem::decision`
- Reviewed body digest:
  `sha256:b4bdf762fb3e51e108aa743d98300a524045d5bb39bd50c304d73ee47c142113`
- Oracle: `SPEC_RUNTIME_ENGINE_001#fault-policy-decision-projection`
- Invariant: `RT_FAULT_POLICY_DECISION_001`

IEC 61131-3 defines resource and task execution but does not define truST's
portable host-side `FaultPolicy` or `FaultDecision` API. `DEV-060` therefore
owns the product boundary: Halt projects to Halt with no safe-state request,
SafeHalt projects to SafeHalt with a safe-state request, and Restart projects
to Restart with no safe-state request. Projection reads only configured policy,
does not depend on the fault flag or stored error, mutates no holder state, and
does not execute output, recovery, restart, clear, or storage effects. The
production body did not change.

## Executable contract

The existing core unit and host integration tests retain discovery identities
`DISC_4C7C595026E1FD708546` and `DISC_04CF6B406AB66EB7B40B`. They are now
cataloged as `TEST_RUNTIME_FAULT_POLICY_DECISION_CORE_001` and
`TEST_RUNTIME_FAULT_POLICY_DECISION_HOST_001`. Each executes all three policies
before and after recording `WatchdogTimeout`, verifies the exact decision, and
proves the holder observations remain unchanged by projection.

`TEST_RUNTIME_FAULT_POLICY_DECISION_TRACE_001`, discovery identity
`DISC_42530353D93187E17303`, executes all three reviewed partitions through the
shared `verification-cases` artifact contract. Its generated case file is
`verification/cases/runtime_safety/RT_FAULT_POLICY_DECISION_001.toml`, digest
`sha256:0a1b09240a7a0b3ea38a922dcaee0b09c7eb9879b08bcb7cd29087f02099354a`.
It regenerates exactly with `gen_cases_v2.py v1`. The live scanner found 4,051
test facts and reconciled all three stable identities.

## Producer-authentic lock proof

The isolated builder checkout was
`trust-builder:/home/johannes/projects/trust-platform-p18-substrate-186b9359`.
Canonical `/home/johannes/projects/trust-platform/AGENTS.md` and the complete
`.codex/skills/**` tree were manually copied before each HEAD transition.
Source and destination hashes matched as
`d0b35bc3edf19dc1d276202b6058ac6ad2835c6552fafbe71c04eb262a88530e`
and `a595ab028d85241bc031148f44604245e027d9c320d858150bc460aaf740b25f`.

The baseline ran from clean source commit
`91109873a25fc43969a91ca382c429ba2fd45c9e`; the comparison ran from clean
descendant commit `3552414410d84cefad693089346923776cdda9c8`.

- Baseline: `EVID_TEST_RUNTIME_FAULT_POLICY_DECISION_TRACE_001_LOCK_BASELINE`
- Comparison: `EVID_TEST_RUNTIME_FAULT_POLICY_DECISION_TRACE_001_LOCK_COMPARE`
- Proof contract digest:
  `sha256:84383289be47200b7ddbe8333515bf6dd97b46f212d2618415bf0c268e1ee2c0`
- Stable case-result digest:
  `sha256:b876cd363c1ae13a3c34373f61ca5d9fd11c091369478ef936289fd599e013ce`
- Baseline artifact digest:
  `sha256:05c0923655e076f5f86555a4b2bf168134b3e0b304a7b7e2430e338f9d88392b`
- Comparison artifact digest:
  `sha256:5658fecba06e24e41d6c44827471f74b5981398fe4c6efce3602af027da706c1`
- Result: all three policy partitions passed in healthy and recorded-fault
  states in both runs; no case was blocked, skipped, filtered out, or failed.

The first formal baseline attempt was rejected because the shared target cache
contained `verification-cases` compile-time workspace metadata from another
worktree. Cleaning only that generated helper package removed 25.4 MiB and
forced the accepted repository-relative case identity. The rejected attempt
produced no evidence record.

Focused x86_64 Linux proof passed formatting, the exact core unit, host
integration, and generated trace commands, case regeneration, 928-record
pre-lock metadata validation, and catalog staleness against all 4,051 scanner
facts. Full `clippy` and `test-all` were not run; the active board reserves them
for the one final `VERIF-P18-016` candidate.

## Exact risk acknowledgement

`RISK_ACK_E78D5BAEDF1B5EE0A6C2DEA95C1E40ADF75CADF7F3FE427F6BDA5738D8453BC8`
acknowledges the scanner's `fault_mapping` signal for the exact unchanged body
digest. It is bound to `DEV-060`, the fault-policy projection specification,
the invariant, strengthened core and host locks, and producer-authentic
generated trace pair. It does not suppress the signal or claim the still-open
assertion reachability, mutation budget, refreshed ledger, or broad-gate proof.

## Architecture and remaining debt

The portable holder continues to own only policy and fault observations; the
host runtime continues to own output application, restart, resource recovery,
and storage lifecycle. The new trace adapter reuses `verification-cases` and
introduces no production ownership, data-flow, public API, or diagram change.
This satisfies the SOLID/KISS/DRY acceptance check: one normative decision
table, two strengthened boundary consumers, and one generated trace at the
portable owner.

Remaining exact debt:

- source-bound assertion reachability under global `VERIF-P18-007`;
- mutation sensitivity and zero viable survivors under `VERIF-P18-009`;
- refreshed fact-ledger accounting for the exact code and test identities;
- causal broad evidence and final frozen-candidate gates under
  `VERIF-P18-016`;
- all other code/test facts and every global Phase 18 equation.
