# P18 integer i64 normalization slice

Date: 2026-07-22

Status: exact authority, mapping, executable boundary matrix, and targeted
behavior-lock and source-bound assertion-reachability proof complete. Mutation
sensitivity and broad remote proof remain open. No Phase 18 global row or stop
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

## Assertion reachability

The Phase 18 producer executed all three reviewed numeric shards in one
isolated run from clean source `811e94b04463217ef6976e90388c33b467e7b437` on
`trust-builder-linux-x86_64`. Both integer-normalization commands passed on the
unmodified source. It then deleted only the generated `Value::ULInt(v)` match
arm from `to_i64`; the mutant compiled, and each selected test failed at its
named assertion. The semantic validator reconstructed both results as
`caught`.

Canonical raw artifact:
`verification/p18/assertion-reachability.json`, SHA-256
`20bbf0d59c8dd4e14b272ff3de1f4670f32d3ea019d63f60021c204d956da246`, input
closure `sha256:3088cfaaec235b2449677fb7b32646fd164cdd72325d7b70b514055cdd46c5ba`.
Its report-only summary is 6/269 proven, 263 unrun debt, six caught mutants,
and zero survivors, unviable outcomes, timeouts, or harness failures. This
closes this invariant's `assertion_reachability` item only. The separate
`mutation_sensitivity` and formal `VERIF-P18-009` adequacy campaign remain open
and budget-gated.

## Refreshed slice accounting

The canonical fact ledger was regenerated on the isolated builder from clean
source `b58dfe73e5f3dda7bda12234cd59b9ae1a49eebf`, then committed and revalidated
from clean descendant `605919e0`. It contains exactly 15,298 code facts and
4,044 test facts with zero duplicate or omitted identities. The canonical
artifact SHA-256 is
`eb16de61807c386c1289844d683ba9d080b6d0a44988000340f56a52c3153aa3`
and its input closure is
`sha256:d5e748460eb17480df40b30a5c3c13299593ce8f80d79da511f5aa304ff4f7e9`.

The exact `to_i64` slice is now `specified` with both test IDs and the reviewed
invariant. Repository totals are 15,136 remaining `spec_gap` facts, 15,133
remaining `registered_backlog` facts, and 3,777 remaining
`reviewed_nonmapping` tests. The rebound report-only zero-debt gate has zero
contract findings; strict mode correctly remains `open_debt`. All nine focused
P18 substrate self-tests and 896-record metadata validation pass. No broad
`clippy` or `test-all` milestone ran.

## Exact risk acknowledgement

`RISK_ACK_48DB0C5559CB11931C99FDD5BA0812B4045ACFFE01AF8DDB1E3BA4BA16F4087D`
acknowledges the scanner's `fault_mapping` signal for the exact unchanged body
digest. It is bound to this reviewed specification, invariant, direct unit
matrix, and producer-authentic lock pair. It does not suppress the signal or
claim mutation adequacy or broad-gate proof; the source-bound catcher evidence
closes only assertion reachability for this exact slice.

## Architecture and remaining debt

The implementation reuses the existing `verification-cases` helper and keeps
the artifact adapter isolated in a `trust-runtime-core` integration test. The
direct matrix remains beside the pure numeric helper. No production ownership,
data flow, execution flow, public API, or diagram changed, so no product
architecture diagram update applies to this proof-only slice. This satisfies
the SOLID/KISS/DRY acceptance check: one existing oracle, one generated case
table, and two small consumers at the owning numeric boundary.

Remaining exact debt:

- mutation sensitivity and zero viable survivors under `VERIF-P18-009`;
- causal broad evidence and final frozen-candidate gates under
  `VERIF-P18-016`;
- all other code/test facts and every global Phase 18 equation.
