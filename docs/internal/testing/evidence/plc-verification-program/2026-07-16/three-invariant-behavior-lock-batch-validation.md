# Three-invariant behavior-lock batch validation

Date: 2026-07-16

Case-backed runner checkpoint:
`96ead60003d39c7f35def130896284e861823fac`

Stable case-provenance checkpoint:
`adfbf1b342676b80c8ba5c75b9213ce8acfe5ae7`

Stable lock-compare checkpoint:
`497411c8c312d9fb70ab0fae3ce24dbb84191e41`

G1 promotion and Rust validation checkpoint:
`12a41da548d7ffde5290fdc6b5a237de4bdaac7a`

Final report source checkpoint: pending final verification tripwire refresh.

Report refresh and final verification checkpoint: pending.

## Outcome

This batch added the missing case-backed tests for three already-written
contracts:

- `IEC_PREC_001`: eight expression-precedence and associativity cases;
- `PLCO_IMPORT_001`: seven supported and unsupported executable-body import
  cases; and
- `PROTO_OPCUA_001`: six OPC UA client lifecycle cases.

All 21 cases passed against unchanged product code. The batch therefore found
missing executable proof coverage, not a product defect. No runtime, parser,
or PLCopen importer behavior was changed, and no red result was manufactured.

## Tests and contract binding

The hand-owned catalog now maps:

- `TEST_IEC_PRECEDENCE_TRACE_001` to
  `verification/cases/compiler_iec/IEC_PREC_001.toml`;
- `TEST_PLCOPEN_IMPORT_TRACE_001` to
  `verification/cases/plcopen_devtools/PLCO_IMPORT_001.toml`; and
- `TEST_OPCUA_CLIENT_LIFECYCLE_TRACE_001` to
  `verification/cases/protocols/PROTO_OPCUA_001.toml`.

The IEC and PLCopen definitions use the existing generated-case runner through
the new `gen_cases.py v2` contract. The OPC UA definition uses the established
hand-authored state-machine mode. Each runner emits the standard stamped case
artifact consumed by `prove.py`.

## Lifecycle-stable generated cases

The first v2 implementation bound generated cases to the complete invariant
file. That made an honest proof-reference or proof-level update change the case
digest, which in turn invalidated the proof that authorized the update. The
batch caught this verification-tooling defect before promotion.

The corrected v2 generator binds `source_digest` to an invariant execution
contract: behavior rows, oracle semantics, and other executable inputs remain
bound, while lifecycle outputs such as status, proof level, evidence references,
and coverage state are excluded. Regression tests prove that lifecycle-only
changes preserve the digest and behavior changes alter it. The original
`gen_cases.py v1` bytecode contract and all existing bytecode cases remain
byte-stable.

The final committed case-file digests are:

- `IEC_PREC_001`: `sha256:c23709e713899ce7926b7a04a60c3f3fdcc0cb6407b54440d211746b79791ea7`;
- `PLCO_IMPORT_001`: `sha256:1a44c22e641c00fdc7ec9a0f0ddb3cdf1cec59b3626d22558cc3731e7914101e`;
  and
- `PROTO_OPCUA_001`: `sha256:d5f1740836a29c1a0742f3040e5060a01fc78f07e99870a8c139f27c41191e7d`.

## Producer-authentic proof

`prove.py v1` recorded clean lock baselines at
`adfbf1b342676b80c8ba5c75b9213ce8acfe5ae7` and clean descendant lock compares
at `497411c8c312d9fb70ab0fae3ce24dbb84191e41`. Each pair has distinct run IDs,
the same case-file and proof-contract digests, and identical passing per-case
result digests. The three invariants are consequently `implemented` at G1.

No approved broad-gate evidence was created. The canonical builder checkout
contains unrelated user work, so the batch used an isolated clean worktree for
validation and did not relabel that manual result as causal G2 evidence.

## Remote validation

Focused validation on the clean `trust-builder` worktree included:

- `cargo test -p verification-cases generated_v2_provenance_is_accepted`;
- `cargo test -p trust-runtime --test iec_precedence_trace_cases
  expression_precedence_trace_cases -- --exact`;
- `cargo test -p trust-runtime --test plcopen_import_trace_cases
  plcopen_import_trace_cases -- --exact`;
- `cargo test -p trust-runtime --lib
  opcua::lifecycle_cases::opcua_client_lifecycle_trace_cases -- --exact`;
- focused v2 generator, case-trace, metadata, and seed-lifecycle Python tests;
- `python3 scripts/validate_verification_metadata.py`; and
- v1 and v2 generated-case checks.

The final broad Rust gates ran once at the end against clean checkpoint
`12a41da548d7ffde5290fdc6b5a237de4bdaac7a`:

- `just fmt`: passed;
- `just clippy`: passed; and
- `CARGO_INCREMENTAL=0 just test-all`: passed with no failures.

The initial cold `test-all` attempt exhausted its batch-owned 59 GiB target
before a test result was available. Following the builder rule, all remaining
build processes were stopped, only that generated target was removed, disk
capacity was rechecked, and the clean gate was rerun with incremental artifacts
disabled. The successful target was then removed, restoring 63 GiB free.

## Final verification validation

Pending the one-time report refresh and final metadata validation.

## Generated report refresh

Pending generation from the final clean report source checkpoint.

## Honest remaining posture

- The invariant register contains 53 records: 36 at S0, 8 at G1, and 9 at G2.
- Coverage contains 24 covered cells, 20 `gap_open` cells, and 24 `spec_gap`
  cells.
- The gap register remains 18 closed, 15 open, and 1 `spec_updated`; this batch
  closes no specification gap.
- `PLCO_IMPORT_001` still requires real-vendor corpus evidence in addition to a
  causal broad gate; the other two invariants still require a causal broad gate.
- The hand-owned catalog contains 190 records, and the broader Phase 16 mapping
  denominator remains open.
- CI, workflows, suites, approved proof producers, and enforcement posture are
  unchanged.
