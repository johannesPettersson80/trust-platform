# P18 integer i64 normalization slice

Date: 2026-07-22

Status: exact authority, mapping, executable boundary matrix, and targeted
behavior-lock proof complete. Dynamic assertion reachability, mutation
sensitivity, and broad remote proof remain open. No Phase 18 global row or stop
gate is closed by this slice.

## Scope and authority

- Phase 18 slice:
  `P18-CODE-CODE_48DB0C5559CB11931C99FDD5BA0812B4045ACFFE01AF8DDB1E3BA4BA16F4087D`
- Exact code fact:
  `CODE_48DB0C5559CB11931C99FDD5BA0812B4045ACFFE01AF8DDB1E3BA4BA16F4087D`
- Exact symbol: `trust_runtime_core::numeric::to_i64`
- Reviewed body digest:
  `sha256:022801157617e00f8f815c4d1bfc518d9a9f063bf76ee0053c1c0197757e9540`
- Oracle:
  `SPEC_RUNTIME_SEMANTICS_001#6-9-integer-runtime-normalization-to-i64`
- Invariant: `RT_INTEGER_I64_NORMALIZATION_001`

The reviewed truST contract requires exact normalization for every integer
runtime tag representable by `i64`, `RuntimeError::Overflow` for `ULINT` above
`i64::MAX`, and `RuntimeError::TypeMismatch` for non-integer runtime tags. This
is an internal runtime-operational contract recorded as `DEV-055`, not an IEC
source-language conversion rule. The production body did not change in this
slice; the work replaces a historical gap with reviewed authority and
executable proof rather than deriving expected behavior from the code.

## Executable contract

Two mapped tests cover the closest owning boundary:

- `TEST_RUNTIME_INTEGER_I64_NORMALIZATION_001` directly checks every signed
  width at both boundaries, every unsigned width at its representable maximum,
  the `ULINT` overflow edge and unsigned maximum, and representative
  non-integer runtime categories.
- `TEST_RUNTIME_INTEGER_I64_NORMALIZATION_TRACE_001` executes the four reviewed
  behavior partitions through the shared `verification-cases` artifact
  contract. Its generated case file is
  `verification/cases/runtime_safety/RT_INTEGER_I64_NORMALIZATION_001.toml`
  with digest
  `sha256:76a97c9098d244c7305fdc620e304fdfa96dc1406282001718d0a041fcead2c4`.

The case file regenerates exactly with `gen_cases_v2.py v1`. The scanner found
the direct test as `DISC_69CC0C415ACEA7836310` and the integration test as
`DISC_4BCDECA148555EE6800D`; both committed denominator rows map to their
catalog tests without an ignore or nonmapping disposition. The complete scan
contains 4,044 facts.

## Producer-authentic lock proof

The isolated builder checkout was
`trust-builder:/home/johannes/projects/trust-platform-p18-substrate-186b9359`.
Before each branch transition, canonical
`/home/johannes/projects/trust-platform/AGENTS.md` and the complete
`.codex/skills/**` tree were manually copied to the destination. Their hashes
matched as `d0b35bc3edf19dc1d276202b6058ac6ad2835c6552fafbe71c04eb262a88530e`
and `a595ab028d85241bc031148f44604245e027d9c320d858150bc460aaf740b25f`.

The baseline ran from clean source commit
`4017add56e961317abb3395539f64c82933c86be`; the comparison ran from clean
descendant commit `b4349925c0859cb7e311ad4385d32a6b942dd0b6`.

- Baseline:
  `EVID_TEST_RUNTIME_INTEGER_I64_NORMALIZATION_TRACE_001_LOCK_BASELINE`
- Comparison:
  `EVID_TEST_RUNTIME_INTEGER_I64_NORMALIZATION_TRACE_001_LOCK_COMPARE`
- Proof contract digest:
  `sha256:30f923bfcdc24545332233465f06adcc0bc4cb621cd92ad8b6fad1fbf47766c6`
- Stable case-result digest:
  `sha256:98c24419a0045c7f94d4f5a8b901b04ee49ec71ae1a547911269078bc6af3dd1`
- Result: all four generated partitions passed in both runs; no case was
  blocked, skipped, filtered out, or failed.

Focused x86_64 Linux proof at exact source `4017add56e...` also passed the exact
direct unit command and the exact case-backed integration command. Local static
validation passed case regeneration, 893-record metadata validation, and the
275-to-4,044 catalog staleness join. Full `clippy` and `test-all` were not run;
the active board reserves them for the one final `VERIF-P18-016` candidate.

## Exact risk acknowledgement

`RISK_ACK_48DB0C5559CB11931C99FDD5BA0812B4045ACFFE01AF8DDB1E3BA4BA16F4087D`
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
