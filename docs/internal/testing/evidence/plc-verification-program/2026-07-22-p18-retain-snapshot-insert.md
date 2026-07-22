# P18 retain snapshot insertion slice

Date: 2026-07-22

Status: exact authority, mapping, executable transition matrix, targeted
behavior-lock proof, source-bound assertion reachability, and fact-ledger
accounting complete. Mutation sensitivity and broad remote proof remain open.
No Phase 18 global row or stop gate is closed by this slice.

## Scope and authority

- Phase 18 slice:
  `P18-CODE-CODE_F4ABB9ACC5495574EA09AD38CFC84F2BDE1D0BACA961B3CEAF9D9E44992E3F53`
- Exact code fact:
  `CODE_F4ABB9ACC5495574EA09AD38CFC84F2BDE1D0BACA961B3CEAF9D9E44992E3F53`
- Exact symbol: `trust_runtime_core::retain::RetainSnapshot::insert`
- Reviewed body digest:
  `sha256:4ccfcc38579fe7e1d3e7e4f0be7e9c0e58696a0ad8718fe4e286e0a5029443e8`
- Oracle: `SPEC_RUNTIME_ENGINE_001#retain-snapshot-identity-and-order`
- Invariant: `RT_RETAIN_SNAPSHOT_INSERT_001`

IEC 61131-3 section 6.5.6 defines which variable classes are retentive, not
the duplicate-key, ordering, or validation semantics of an internal snapshot
container. The reviewed truST policy in `DEV-057` therefore owns this boundary:
the first insertion appends the exact key/value, reinserting the same resolved
key replaces only its value at the original position, and a distinct supplied
key appends without hidden normalization. Insertion is not persistence or
reload acceptance; those later boundaries retain declared-type, finiteness,
migration, and atomic-apply validation. The production body did not change.

## Executable contract

Two mapped tests cover the closest portable boundary:

- `TEST_RUNTIME_RETAIN_SNAPSHOT_INSERT_001` directly proves exact-value append,
  same-key replacement without duplicate/cardinality/order drift, unchanged
  sibling state, and distinct supplied-key append.
- `TEST_RUNTIME_RETAIN_SNAPSHOT_INSERT_TRACE_001` executes the three reviewed
  partitions through the shared `verification-cases` artifact contract and
  records the retained snapshot transition.

The generated case file is
`verification/cases/runtime_safety/RT_RETAIN_SNAPSHOT_INSERT_001.toml`, digest
`sha256:d49c380eed600a367208997ef2b31482526eaa802e121e14a63cbd185fad014d`.
It regenerates exactly with `gen_cases_v2.py v1`. The scanner found the direct
test as `DISC_DECD7AFB66FB10F373C0` and the integration test as
`DISC_4E22986F2396DB8BFAC1`; both committed denominator rows map to their
catalog tests. The full scan contains 4,048 facts, and the staleness join
validates 279 committed catalog records against that denominator.

## Producer-authentic lock proof

The isolated builder checkout was
`trust-builder:/home/johannes/projects/trust-platform-p18-substrate-186b9359`.
Before each branch transition, canonical
`/home/johannes/projects/trust-platform/AGENTS.md` and the complete
`.codex/skills/**` tree were manually copied to the destination. Their hashes
matched as `d0b35bc3edf19dc1d276202b6058ac6ad2835c6552fafbe71c04eb262a88530e`
and `a595ab028d85241bc031148f44604245e027d9c320d858150bc460aaf740b25f`.

The baseline ran from clean source commit
`693306ae1a4cafef662a72d742f64bc968189462`; the comparison ran from clean
descendant commit `e1b2c2f18a630348b36229f2cddc90c8431e92e2`.

- Baseline:
  `EVID_TEST_RUNTIME_RETAIN_SNAPSHOT_INSERT_TRACE_001_LOCK_BASELINE`
- Comparison:
  `EVID_TEST_RUNTIME_RETAIN_SNAPSHOT_INSERT_TRACE_001_LOCK_COMPARE`
- Proof contract digest:
  `sha256:11b528f935e5af337381dd4f4adce026cafab5c91c446d740c726f1b92af5a8a`
- Stable case-result digest:
  `sha256:1c1fbca44cf401cb3b69537a53f5f5ba1d751b7ca97f90549df828b766747132`
- Result: all three generated partitions passed in both runs; no case was
  blocked, skipped, filtered out, or failed.

The first formal baseline attempt was rejected because the reused target cache
contained `verification-cases` compiled in another worktree and therefore
emitted an absolute case path. Removing only that package's 16 MiB of generated
cache forced recompilation at the clean source checkout. The accepted baseline
and comparison both record the required repository-relative case path; the
rejected attempt produced no evidence record.

## Source-bound assertion reachability

The cumulative Phase 18 producer ran from clean source commit
`8e65a3997819cf6cffd56fa21719b2f9b024489f` in the isolated builder checkout.
It selected generated adversarial mutation
`MUTANT_RETAIN_SNAPSHOT_INSERT_REPLACE_WITH_UNIT` at exact function
`RetainSnapshot::insert`. The mutant compiled, and each mapped direct/trace
test independently failed against the changed production body, so both
assertions are classified `proven` rather than merely executed.

- Evidence: `EVID_RT_RETAIN_SNAPSHOT_INSERT_ASSERTION_REACHABILITY_001`
- Canonical artifact: `verification/p18/assertion-reachability.json`
- Artifact digest:
  `sha256:5aebc75976607070d52bdb9967504bc35f072b88e90690feb203fd47e2d4c280`
- Input-closure digest:
  `sha256:d3d804ff26d0aaccabd026ba200fbfab908ec1b57b92c481af918945607f8eff`
- Cumulative result: 8/271 catalog assertions proven, eight caught mutants,
  zero survivors, unviable mutants, timeouts, or harness failures.

The remaining 263 assertions stay explicitly open. This slice does not use
the cumulative result to waive any other test or the separate formal mutation
budget in `VERIF-P18-009`.

## Refreshed slice accounting

The deterministic fact-ledger producer ran in the isolated builder checkout
from clean source `fcbaf8141d699e60d891f8b3f455e0e3434286ee`. The generated
artifact was committed as `5726d23b359bd427c7f6bdeb6f8e6ae8e4982bed` and
revalidated from clean descendant
`2562a416bffc67ae7665d01cc74901df5d188c7c`. The canonical ledger contains
exactly 15,298 code facts and 4,048 test facts with zero duplicate, omitted, or
unowned identities.

- Burn-down artifact digest:
  `sha256:cc877807f4706fc491d566e00930d206c438618c8bc1853f87a57dc9c8529591`
- Burn-down input closure:
  `sha256:ece60ff87d80c3f87c0cbe1968dbf904f4bc8a3f80cb19f32a626da3af030a4c`
- Remaining `spec_gap`: 15,134
- Remaining `registered_backlog`: 15,131
- Remaining `reviewed_nonmapping`: 3,777
- Test mapping: 271/4,048, artifact digest
  `sha256:fbbe7e15f2aee39053ba8e555a3e6fb9a6163b510f120fd9dc74d409a0314af6`
- Top-level ledger closure:
  `sha256:afa5401d966750026fd743783fd696f32a81b285f114c70636997aa31e7bf8f2`

The report-only zero-debt gate has zero contract findings. Strict mode
correctly remains `open_debt`; all remaining populations and unmeasured proof
classes stay blocking. The nine focused P18 substrate tests pass, including
the four-shard assertion-plan join and known-bad strict-mode catchers.

Focused x86_64 Linux proof at exact source `693306ae...` also passed the exact
direct unit command and exact case-backed integration command. Static
validation passed case regeneration, 905-record metadata validation, and the
279-to-4,048 catalog staleness join. Full `clippy` and `test-all` were not run;
the active board reserves them for the one final `VERIF-P18-016` candidate.

## Exact risk acknowledgement

`RISK_ACK_F4ABB9ACC5495574EA09AD38CFC84F2BDE1D0BACA961B3CEAF9D9E44992E3F53`
acknowledges the scanner's `state_mutation` signal for the exact unchanged body
digest. It is bound to the reviewed specification, invariant, direct unit
matrix, and producer-authentic lock pair. It does not suppress the signal or
claim the still-open mutation-budget or broad-gate proof.

## Architecture and remaining debt

The implementation reuses the existing `verification-cases` helper and keeps
the artifact adapter isolated in a `trust-runtime-core` integration test. The
direct matrix remains beside the portable retain model. No production
ownership, data flow, execution flow, public API, or diagram changed, so no
product architecture diagram update applies to this proof-only slice. This
satisfies the SOLID/KISS/DRY acceptance check: one product oracle, one generated
case table, and two small consumers at the owning method boundary.

Remaining exact debt:

- mutation sensitivity and zero viable survivors under `VERIF-P18-009`;
- causal broad evidence and final frozen-candidate gates under
  `VERIF-P18-016`;
- all other code/test facts and every global Phase 18 equation.
