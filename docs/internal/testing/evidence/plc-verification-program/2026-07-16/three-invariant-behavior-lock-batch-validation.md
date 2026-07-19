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

Final report source checkpoint:
`f71729e9ec9314de8dea09a7062d3546c81b546b`

Report refresh and final verification checkpoint:
`d3c6f5b8ba08f27e9d57724e2abaa1c9aec578da`

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

## Case-gate dispatch

The final metadata gate exposed a second verification-tooling defect: the gate
sent every generated case table to the bytecode-only `gen_cases.py v1`
entrypoint. The new v2 IEC and PLCopen tables therefore failed the production
gate even though their direct checks passed. A tests-first end-to-end fixture
now runs the gate against every committed generated-case version. The shell
gate dispatches by the committed `generator` field to `gen_cases.py v1` or
`gen_cases_v2.py v1` and rejects unknown generators. The regression test and
the complete metadata gate both pass at the final checkpoint.

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

The verification-only closure ran on `trust-builder` at
`d3c6f5b8ba08f27e9d57724e2abaa1c9aec578da` after the broad Rust gates, so no
second broad compile was performed:

- `python3 scripts/run_verification_focused_tests.py`: 807/807 passed;
- `python3 scripts/validate_verification_metadata.py`: 562 records validated;
- `scripts/verification_metadata_gate.sh`: passed, including mixed v1/v2 case
  dispatch and the Phase 16 product fence;
- `python3 scripts/check_verification_tooling_selftests.py --report
  docs/internal/testing/evidence/plc-verification-program/2026-07-11/p6a-tooling-selftest-fixture-report.md`:
  33/33 fixtures passed;
- ignored-test join: 29 discovered, 29 registered, 5 unknown, 0 catalog-mapped;
- catalog staleness join: 190 committed records against 3,970 scanner facts;
- VS Code registration: 456 facts, 38 files, 38 registrations;
- test-refactor proposals: 1 proposal, 0 redirects, 190 catalog records, 3,970
  scanner facts;
- v1 `VM_SEAM_VALID_001` and v2 `IEC_PREC_001` / `PLCO_IMPORT_001` generated
  case checks: passed;
- all 15 current report pairs: passed at-rest validation;
- `python3 scripts/check_diagram_drift.py`: passed;
- `git diff --check`: passed; and
- the retained remote worktree was clean at the exact checkpoint before and
  after validation.

## Generated report refresh

All current report pairs were regenerated from pristine isolated worktrees at
`f71729e9ec9314de8dea09a7062d3546c81b546b` with timestamp
`2026-07-16T16:12:00+02:00`. Each generator and its at-rest validator passed,
and each regenerated Markdown file is committed. The canonical JSON digests
are:

| Report | SHA-256 |
| --- | --- |
| Test-class completeness | `ec4eae998ae586f29e63db7ff6be1eace47ec3d6fc0677acf011e5e9223fa290` |
| Coverage-matrix gaps | `b109d5ae1f336d2acb51cae72aba6c841bda71888213bbc7cec047de27d303b3` |
| Malformed-input coverage | `4507b69e7994653ae60dc1c30ed6e040eae24be1136b3fda35e4ea85ff2c6e20` |
| Unmapped-test debt | `2904c58f6d37cb47f96abd1764393315297a0f5f0825ce959bb22ee098c770d2` |
| Test-refactor assessment | `70b1374c763f13dc5d54a5e352a23cd1e791a7848209ac78a37de71debeb991b` |
| Ignored-test inventory | `e890753d9e465e22921af6a2a4e96510878fe43b69c0a46e03ee05f50906a87a` |
| Phase 5 suite audit | `1464454a498c7816366a833f87e2845e8579c17f8f4bfd15480079c2576660a4` |
| Invariant-seed audit | `2848e31a65ce7cf7a4bd4b40a67c528f9381e6ad660308de88ba40454bfa2b50` |
| Specification completeness | `14e762668683c6d568e0ec870ee45c11c5847774ff2443053ca2ec79c11bffe9` |
| Requirement/oracle audit | `7f3406edc913a895d555ae279f1500f32586e663f9cb390dbad16d6f9724bb77` |
| Conformance alignment | `a2d9a15690fa4e2c56efe0645b0e85fca29cc24c6bafaa7ee7d219c717b52c34` |
| Runtime-anomaly audit | `eacc7370802733adee8ce87ee779174a3dbcfb2f8e85f6bc92a8c42bec1f148c` |
| Fuzz-program audit | `05cbe44018c18fc7b497db503dd39430bd179e4ffa3ec1a3dcbb9996f5d111d7` |
| Mutation-program report | `fc05a0ce5262888a88a6dd51f0be93eeb8415ed2c59af3842cb3f5e7d4d6b815` |
| Specification-source audit | `a00670373b58b9fbdd27e721723a4db83513c9556494b3d30d18b37bbf5e001e` |

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
