# P18 unsigned integer materialization slice

Date: 2026-07-22

Status: exact authority, mapping, executable boundary matrix, and targeted
behavior-lock proof complete. Dynamic assertion reachability, mutation
sensitivity, and broad remote proof remain open. No Phase 18 global row or stop
gate is closed by this slice.

## Scope and authority

- Phase 18 slice:
  `P18-CODE-CODE_A438DB9396B2895A5E07F5891D9467E790F822347F344B1F6AF0AB99C342BE99`
- Exact code fact:
  `CODE_A438DB9396B2895A5E07F5891D9467E790F822347F344B1F6AF0AB99C342BE99`
- Exact symbol: `trust_runtime_core::numeric::unsigned_from_u128`
- Reviewed body digest:
  `sha256:ddc77b7d57970d598ae98c7ae7f80fb343928bd337199563ac333dba4ac3166f`
- Oracle:
  `SPEC_RUNTIME_SEMANTICS_001#6-10-unsigned-integer-result-materialization`
- Invariant: `RT_UNSIGNED_INTEGER_MATERIALIZATION_001`

The reviewed truST contract requires exact destination tags and values for
every representable `USINT`, `UINT`, `UDINT`, and `ULINT` result,
`RuntimeError::Overflow` for the first value above each destination maximum,
and `RuntimeError::TypeMismatch` for signed or floating destinations. This is
the fail-closed runtime policy recorded as `DEV-056`. The production body did
not change in this slice; the work replaces a historical gap with reviewed
authority and executable proof rather than deriving expected behavior from the
code.

## Executable contract

Two mapped tests cover the closest owning boundary:

- `TEST_RUNTIME_UNSIGNED_INTEGER_MATERIALIZATION_001` directly checks zero,
  maximum, and the first value above maximum for all four unsigned widths,
  including `u64::MAX + 1` represented in `u128`, plus every signed and
  floating destination category.
- `TEST_RUNTIME_UNSIGNED_INTEGER_MATERIALIZATION_TRACE_001` executes the three
  reviewed behavior partitions through the shared `verification-cases`
  artifact contract. Its generated case file is
  `verification/cases/runtime_safety/RT_UNSIGNED_INTEGER_MATERIALIZATION_001.toml`
  with digest
  `sha256:ddd10eed182da7caf71aba25d8ac01d3d3a5893380894fb33e8c51a6d7878de8`.

The case file regenerates exactly with `gen_cases_v2.py v1`. The scanner found
the direct test as `DISC_515CDC7490B537210498` and the integration test as
`DISC_83069BB55E723A690C01`; both committed denominator rows map to their
catalog tests without an ignore or nonmapping disposition. The complete scan
contains 4,046 facts, and the staleness join validates 277 committed catalog
records against that denominator.

## Producer-authentic lock proof

The isolated builder checkout was
`trust-builder:/home/johannes/projects/trust-platform-p18-substrate-186b9359`.
Before each branch transition, canonical
`/home/johannes/projects/trust-platform/AGENTS.md` and the complete
`.codex/skills/**` tree were manually copied to the destination. Their hashes
matched as `d0b35bc3edf19dc1d276202b6058ac6ad2835c6552fafbe71c04eb262a88530e`
and `a595ab028d85241bc031148f44604245e027d9c320d858150bc460aaf740b25f`.

The baseline ran from clean source commit
`de88ba354fa5703ec4f6f126428492b4f7e18719`; the comparison ran from clean
descendant commit `7427046cbdf42c79c5e12c5612af816ad24fba44`.

- Baseline:
  `EVID_TEST_RUNTIME_UNSIGNED_INTEGER_MATERIALIZATION_TRACE_001_LOCK_BASELINE`
- Comparison:
  `EVID_TEST_RUNTIME_UNSIGNED_INTEGER_MATERIALIZATION_TRACE_001_LOCK_COMPARE`
- Proof contract digest:
  `sha256:d77f3f5e6e208a457baa8eff123b7cc2feed4859d4b30d25df81a093dd5cd8dd`
- Stable case-result digest:
  `sha256:b8bd1075c088f15862e32fe2fc6e3277b0c7e19210300c729e6bd6652025e348`
- Result: all three generated partitions passed in both runs; no case was
  blocked, skipped, filtered out, or failed.

A reused builder target initially carried a `verification-cases` binary
compiled in an earlier mutation worktree, so its compile-time workspace root
made the first formal artifact absolute and the producer rejected it. Removing
only that package's 16 MiB of generated cache forced recompilation at the clean
source checkout; the accepted baseline and comparison both record the required
repository-relative case path. This failed attempt produced no evidence
record.

Focused x86_64 Linux proof at exact source `de88ba354f...` also passed the
exact direct unit command and exact case-backed integration command. Static
validation passed case regeneration, 901-record metadata validation, and the
277-to-4,046 catalog staleness join. Full `clippy` and `test-all` were not run;
the active board reserves them for the one final `VERIF-P18-016` candidate.

## Exact risk acknowledgement

`RISK_ACK_A438DB9396B2895A5E07F5891D9467E790F822347F344B1F6AF0AB99C342BE99`
acknowledges the scanner's `fault_mapping` signal for the exact unchanged body
digest. It is bound to this reviewed specification, invariant, direct unit
matrix, and producer-authentic lock pair. It does not suppress the signal or
claim the still-open assertion-reachability, mutation, or broad-gate proof.

## Architecture and remaining debt

The implementation reuses the existing `verification-cases` helper and keeps
the artifact adapter isolated in a `trust-runtime-core` integration test. The
direct matrix remains beside the pure numeric helper. No production ownership,
data flow, execution flow, public API, or diagram changed, so no product
architecture diagram update applies to this proof-only slice. This satisfies
the SOLID/KISS/DRY acceptance check: one existing oracle, one generated case
table, and two small consumers at the owning numeric boundary.

Remaining exact debt:

- source-bound assertion reachability under global `VERIF-P18-007`;
- mutation sensitivity and zero viable survivors under `VERIF-P18-009`;
- causal broad evidence and final frozen-candidate gates under
  `VERIF-P18-016`;
- all other code/test facts and every global Phase 18 equation.
