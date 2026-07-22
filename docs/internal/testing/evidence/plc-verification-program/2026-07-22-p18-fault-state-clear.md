# P18 fault-state clear slice

Date: 2026-07-22

Status: exact authority, mapping, strengthened legacy unit lock, generated
decision-table trace, and producer-authentic targeted behavior-lock proof
complete. Source-bound assertion reachability is also complete. Formal mutation
sensitivity, fact-ledger refresh, and broad remote proof remain open. No Phase
18 global row or stop gate is closed by this slice.

## Scope and authority

- Phase 18 slice:
  `P18-CODE-CODE_E58D6E19401CE00AC50751C139562696AB92E09720F9EBA17F89934B01312CC0`
- Exact code fact:
  `CODE_E58D6E19401CE00AC50751C139562696AB92E09720F9EBA17F89934B01312CC0`
- Exact symbol: `trust_runtime_core::watchdog::FaultSubsystem::clear`
- Reviewed body digest:
  `sha256:dd89ef12fe046aa819fb8eae590250daf5b865acf6f9f1bf1037adccc045aec5`
- Oracle: `SPEC_RUNTIME_ENGINE_001#fault-state-clear-lifecycle`
- Invariant: `RT_FAULT_STATE_CLEAR_001`

IEC 61131-3 defines resource and task execution but does not define truST's
portable host-side `FaultSubsystem` holder or its clear method. `DEV-059`
therefore owns the product boundary: clear atomically removes the fault flag
and last error, preserves `FaultPolicy`, is idempotent while healthy, and does
not restart execution, resume resources, reset storage, prove repair, or
authorize recovery. The production body did not change.

## Executable contract

The existing `watchdog_and_fault_subsystems_preserve_state_contracts` unit test
keeps discovery identity `DISC_DBFE8331AF87D467BFB3` and is now cataloged as
`TEST_RUNTIME_FAULT_STATE_CLEAR_001`. It verifies recorded-fault clearing,
`SafeHalt` preservation, and a second healthy clear.

`TEST_RUNTIME_FAULT_STATE_CLEAR_TRACE_001`, discovery identity
`DISC_ABF461B986A8F002436E`, executes both reviewed partitions through the
shared `verification-cases` artifact contract. Its generated case file is
`verification/cases/runtime_safety/RT_FAULT_STATE_CLEAR_001.toml`, digest
`sha256:eba7f46c61be3a09205fe289f6de07cf92ebce3953cd849a5b60b66c854e8f0f`.
It regenerates exactly with `gen_cases_v2.py v1`. The full scanner found 4,050
test facts with zero warnings and both expected stable identities.

## Producer-authentic lock proof

The isolated builder checkout was
`trust-builder:/home/johannes/projects/trust-platform-p18-substrate-186b9359`.
Canonical `/home/johannes/projects/trust-platform/AGENTS.md` and the complete
`.codex/skills/**` tree were manually copied and read before each HEAD
transition. Source and destination hashes matched as
`d0b35bc3edf19dc1d276202b6058ac6ad2835c6552fafbe71c04eb262a88530e`
and `a595ab028d85241bc031148f44604245e027d9c320d858150bc460aaf740b25f`.

The baseline ran from clean source commit
`c8fe5d17b27709d9305da302715e4280f9331a1e`; the comparison ran from clean
descendant commit `8b76d5d63d9e07547181818a8be15244304219db`.

- Baseline: `EVID_TEST_RUNTIME_FAULT_STATE_CLEAR_TRACE_001_LOCK_BASELINE`
- Comparison: `EVID_TEST_RUNTIME_FAULT_STATE_CLEAR_TRACE_001_LOCK_COMPARE`
- Proof contract digest:
  `sha256:6e4e0d2a22bd1c6b16feb79bd9787574c86bbdd78e071d79b2da8b1807378f57`
- Stable case-result digest:
  `sha256:f4d221f790444ff75fcf9f6d9efae2a48aa6e13fe6f928081e7263f15075e04c`
- Baseline artifact digest:
  `sha256:1f883247e9112fba77c2fc68240b30386722a7de2072cecefc40cf0390ea99b1`
- Comparison artifact digest:
  `sha256:da2f88e8bca33e2a43978afb0401959deeaa06cd7b3831047ac93600c0341479`
- Result: both generated partitions passed in both runs; no case was blocked,
  skipped, filtered out, or failed.

The first formal baseline attempt was rejected because the shared target cache
contained `verification-cases` metadata with an absolute path from another
worktree. Cleaning only that generated helper package removed 16 MiB and
forced the accepted repository-relative case path. The rejected attempt
produced no evidence record.

## Source-bound assertion reachability

The cumulative Phase 18 producer ran from clean source commit
`5a29b349ef545fd7c0f709669bd5d1144c7d99bf` in the isolated builder checkout.
The reviewed shard applies the exact generated source mutation
`MUTANT_FAULT_STATE_CLEAR_REPLACE_WITH_UNIT`, which replaces
`FaultSubsystem::clear` with `()` at `watchdog.rs:263:9`. This is the smallest
viable adversarial change that preserves the method signature while removing
both required state updates.

Both the strengthened unit lock and the generated trace pass against the clean
body and independently fail against that compiled no-op body. They are
therefore classified `proven` rather than merely executed.

- Evidence: `EVID_RT_FAULT_STATE_CLEAR_ASSERTION_REACHABILITY_001`
- Canonical artifact: `verification/p18/assertion-reachability.json`
- Artifact digest:
  `sha256:69eb25a7b6e6508bace3ff1d877f987db5fea35807189872046b66bd00da1cff`
- Input-closure digest:
  `sha256:4b514c409b1aa292eb42d5ce61f641101fa1eaea2742e0021a7ed48e40458da1`
- Cumulative result: 16/279 catalog assertions proven, sixteen caught mutants,
  zero survivors, unviable mutants, timeouts, or harness failures.

The remaining 263 assertions stay explicitly open. This slice does not use the
cumulative result to waive any other test or the separate formal mutation
budget in `VERIF-P18-009`.

Focused x86_64 Linux proof passed the exact strengthened unit test, the exact
generated trace command, case regeneration, formatting, and 921-record
pre-lock metadata validation. Full `clippy` and `test-all` were not run; the
active board reserves them for the one final `VERIF-P18-016` candidate.

## Exact risk acknowledgement

`RISK_ACK_E58D6E19401CE00AC50751C139562696AB92E09720F9EBA17F89934B01312CC0`
acknowledges the scanner's `state_mutation` signal for the exact unchanged body
digest. It is bound to `DEV-059`, the fault-state lifecycle specification, the
invariant, strengthened unit lock, and producer-authentic generated trace pair.
It is also bound to the source-bound assertion-reachability artifact. It does
not suppress the signal or claim the still-open mutation budget, refreshed
ledger, or broad-gate proof.

## Architecture and remaining debt

The fault holder remains in `trust-runtime-core`; the host runtime continues to
own restart, resource recovery, safe outputs, and storage lifecycle. The new
trace adapter reuses `verification-cases` and introduces no production
ownership, data-flow, public API, or diagram change. This satisfies the
SOLID/KISS/DRY acceptance check: one normative decision table, one strengthened
unit consumer, and one generated trace at the owning boundary.

Remaining exact debt:

- mutation sensitivity and zero viable survivors under `VERIF-P18-009`;
- refreshed fact-ledger accounting for the exact code and test identities;
- causal broad evidence and final frozen-candidate gates under
  `VERIF-P18-016`;
- all other code/test facts and every global Phase 18 equation.
