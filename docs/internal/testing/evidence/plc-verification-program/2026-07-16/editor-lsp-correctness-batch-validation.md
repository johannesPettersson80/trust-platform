# Editor and LSP correctness batch validation

Date: 2026-07-16

Product, trace, and proof checkpoint:
`2747e1154cb568d06c17e80bde8a22acca6b6b52`

Reviewed baseline and report source checkpoint:
`f9f416580a2e7b22a184bb856ab4a0847235d0f8`

Report binding checkpoint:
`71867015c0a5f87a92c02e5075f1e188a9abc32b`

## Outcome

This batch added five cataloged, hand-authored trace runners with 29 cases:

- `TEST_EDITOR_RENAME_LOCAL_TRACE_001`: six local collision and identifier cases;
- `TEST_EDITOR_RENAME_PROJECT_TRACE_001`: four project and cross-file cases;
- `TEST_LSP_POSITION_ENCODING_TRACE_001`: nine UTF-16 and line-ending cases;
- `TEST_LSP_DIAGNOSTIC_CANCELLATION_TRACE_001`: five push/pull cancellation cases; and
- `TEST_LSP_DOCUMENT_CLOSE_TRACE_001`: five disk-reload, cache, dependency, and pull-diagnostic cases.

The existing disk-reload test is also cataloged as
`TEST_LSP_DOCUMENT_CLOSE_DISK_RELOAD_001`. The batch closes
`SPEC_GAP_EDITOR_DOCUMENT_CLOSE_001` against `SPEC_LSP_CONTRACT_001` and
promotes `EDIT_RENAME_001`, `EDIT_RENAME_002`, `EDIT_LSP_POS_001`,
`EDIT_DIAG_CANCEL_001`, and `EDIT_DOC_CLOSE_001` from S0 to G1. No broad gate
is used as causal promotion evidence, so the invariants stop honestly at G1.

## Product defect and fix

The document-close trace produced an authentic red result at
`fef5ad88cb5a7439e0ccfcd28c3b762091494354`. Closing an unsaved but readable
file restored the on-disk text while retaining semantic-token and pull-
diagnostic cache entries derived from the discarded buffer. Only
`EDIT_DOC_CLOSE_001_CACHE_INVALIDATION` failed; the other four close cases
passed.

Commit `606666edbb886b0b62f91090ff3129d84d359d3a` evicts both URI-keyed caches
before invalidating the project generation. The paired green proof passes all
five cases with the same case-file and trace-definition digests. The four
surfaces that already matched their written contracts use producer-authentic
current-contract lock-baseline/lock-compare pairs; no failing result was
manufactured.

The final pre-push warning gate also exposed a test-harness portability issue:
Linux-only EtherCAT trace helpers were included on Windows even though their
own test function was target-gated. Commit `2747e115` gates the complete helper
include to `feature = "ethercat-wire"` and Unix. The host and Windows warning
checks then passed. This changes no shipped EtherCAT behavior.

## Focused verification validation

Local validation at the final report source checkpoint:

- `python3 scripts/run_verification_focused_tests.py`: 808/808 passed in
  1002.403 seconds;
- the six baseline modules that initially tripped on the new facts and
  lifecycle state: 100/100 passed after the reviewed expectation refresh;
- `python3 scripts/validate_verification_metadata.py`: 603 records before this
  batch-validation row was indexed;
- `scripts/verification_metadata_gate.sh`: passed; and
- `git diff --check`: passed.

The first complete focused run failed nine live-repository tripwires because
the batch intentionally added five Rust scanner facts, six generated-test
catalog mappings, one invariant, five covered cells, and four execution-ready
seed lifecycles. Commit `f9f41658` changes only those measured expectations and
the corresponding closed-schema lifecycle consts. No validation rule was
weakened.

## Generated report refresh

All 15 report pairs were generated one at a time from the pristine
`trust-builder` worktree at
`f9f416580a2e7b22a184bb856ab4a0847235d0f8` with timestamp
`2026-07-16T23:26:00+02:00`. Every generator and production at-rest validator
exited zero. Each generated Markdown file was imported byte-for-byte, and the
isolated worktree was restored to pristine before the next report.

| Report | JSON SHA-256 |
| --- | --- |
| Test-class completeness | `5bddae5dca0e353f33952bc0f27a6a96bc06281d852175bee8b5fed315c58804` |
| Coverage-matrix gaps | `3688f33bb9741a6fefab9884817937f5ce10813545acbd4a0c1623e83b5298fc` |
| Malformed-input coverage | `2058b526f289a0e333f2d61f59ce46daf9cb1d5a0f118e397f9766a4596d8c1e` |
| Unmapped-test debt | `a1885ea9adfacfe761960c581346a0b2021594694db2c6b3a8d268cd3df64ff7` |
| Test-refactor assessment | `6eb523fe2007b100533df4f8e897131a2a03afcac8ff96d2f3abfc258ef2d565` |
| Ignored-test inventory | `4e48cbc2ae857a4f4a58b3ab3873bd3f07cb68e365530711f8ed4ed33a41a817` |
| Phase 5 suite audit | `ca897efd0f0067960134f0b2640dbfdc3b9a13f0bea44d8d0d1e7b6d4ec8b574` |
| Invariant-seed audit | `c2905f86242f22a4871f8d8e67dc4ac065205f1f1f0e28a49d4520571b31103e` |
| Specification completeness | `2d045ad525e1231f1fdfe2ecab9b2a569a79bc92b053fe35da5944d46243f4f8` |
| Requirement/oracle audit | `2afa1ff167dcd22c655ecd4e12d3be10e446fea71ceed49dcf5142b9e3256b86` |
| Conformance alignment | `fcc5a7b378a46b0af6c51b0f10042d26a662db0947cbf5e1ba417dbb2deee130` |
| Runtime-anomaly audit | `bd39717f495edb062bf06711b9e4b332eefc912a4a42deab88bc49feafd0e1f2` |
| Fuzz-program audit | `a0f38699aa0f29fbaea9399c6f3ea274ba7ca0a5a78790dc396f3f825504ac5d` |
| Mutation program | `50e7cd0aeef8da996dd50dc816efa794c6257b191fd68fd164de01609d54952f` |
| Specification-source audit | `acecb25208073f6ec4c9160a98c63426f8e7a82b45d8d2b320b78eaf0aff6ec4` |

## Remote product validation

The final heavy checkpoint ran on the clean `trust-builder` worktree at
`2747e1154cb568d06c17e80bde8a22acca6b6b52`:

- `just fmt`: passed;
- `just clippy`: passed;
- `./scripts/prepush_ci_gate.sh`: passed, including the host/Windows warning
  checks, trust-lsp tests, path hygiene, and eight runtime mesh/TLS iterations;
- `just test-all`: passed on its clean retry with no failed test result;
- `cargo build -p trust-lsp`: passed; and
- `xvfb-run ... npm test` in `editors/vscode`, with the exact built
  `trust-lsp`: 456/456 passed.

The first `just test-all` attempt exhausted the generated target filesystem
while writing an incremental query cache after fmt, clippy, and pre-push had
passed. It produced no test assertion failure. Compiler processes were stopped,
disk usage was audited, only generated target output was removed, and the gate
was rerun from a clean target. The successful retry is the accepted result.
The first extension attempts also failed before test execution because the
isolated worktree lacked lockfile dependencies and then lacked an X display;
`npm ci` and `xvfb-run` corrected those environment prerequisites before the
456-test green run.

## Honest remaining posture

- The gap register contains 23 closed, 11 open, and 1 `spec_updated` record.
- The invariant register contains 54 records: 26 at S0, 19 at G1, and 9 at G2.
- Specification completeness still reports 14/54 invariants not specified and
  18 specification-gap matrix cells.
- The hand-owned catalog contains 199 records against 3,978 scanner facts;
  194 scanner facts are mapped and 3,784 remain unmapped.
- Conformance alignment remains 0/21 explicitly linked, and the specification-
  source audit still reports 14,415 unreviewed public prose blocks.
- No invariant was promoted beyond G1 by this batch, no hardware result was
  inferred, and no CI, workflow, suite, approved-proof-producer, validator, or
  lifecycle change was made.
- Version metadata is synchronized at 0.24.52; tagging and public release are
  deferred until the change reaches `main`.
