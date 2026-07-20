# VM and bytecode stable error-model batch validation

Date: 2026-07-16

Final report source checkpoint: `b2f0ce18257585e741dbfeadc9a6d9852cbf7cf4`

Report refresh commit: `61308e8ce52241f426275e756f4143daec17a887`

## Outcome

This batch replaced ad-hoc bytecode, VM-trap, runtime-value, and HMI admission
error matching with a closed stable machine-identifier model. Human-readable
diagnostic text remains available, but tests and control clients can now use
the lower-snake-case `error_code` contract without parsing that text.

The implementation provides:

- an exhaustive `StableErrorCode` enum in `trust-runtime-core`;
- a stable identifier for every `BytecodeError` variant;
- stable VM-trap identifiers, preserved when traps become `RuntimeError`;
- structured bytecode errors that preserve their original identifier through
  runtime bytecode apply; and
- typed HMI admission identifiers for type mismatch, bounded-string overflow,
  subrange violation, and non-finite numeric input.

Rejected HMI writes remain transactional: no rejected value is queued and the
existing human-readable error remains present beside `error_code`.

The written contracts live in `docs/specs/10-runtime-semantics.md`,
`docs/specs/11-runtime-engine.md`, and `docs/specs/12-bytecode.md`. The stable
identifiers are product interface contracts, not IEC deviations.

## Tests first and proof chain

- Commit `e9d7ee39` added the written stable-identifier contract and failing
  exact-code tests before product implementation.
- The first authentic red attempt exposed a verification-harness defect:
  case artifacts recorded an absolute temporary case-file path, so `prove.py`
  rejected an otherwise valid run. Commit `148d5271` made the artifact carry
  the committed workspace-relative case identity and added a regression test.
- `EVID_TEST_BYTECODE_VALIDATOR_CASES_001_RED`, produced by `prove.py v1` at
  clean commit `148d52715221bd585bb183b565a090e3832e4e61`, records all seven
  bytecode-validator cases failing their exact stable-code assertions. The run
  exits 101 and binds case-file digest
  `sha256:102262b53ce12ae6a7c2c18ffb52f8a1ae243b80d2baf6a64b3ce32c6df9de95`.
- Commit `60db827f9d26b09192a975429c3486eddd589bfc` implements the product
  error model without changing the committed case contract.
- `EVID_TEST_BYTECODE_VALIDATOR_CASES_001_GREEN`, also produced by
  `prove.py v1`, records all seven formerly red cases passing at the clean
  descendant `60db827f9d26b09192a975429c3486eddd589bfc`, with the same case-file
  and proof-contract digests.
- Commit `c7642476` adds exhaustive bytecode-error, VM-trap, runtime-conversion,
  HMI admission, and type-policy exact-code regression tests. Commit
  `f305dd45` maps the reviewed test identities into the catalog and gap record.

The red/green pair proves the seven committed bytecode-validator cases. The
additional exact-code tests are mapped regression coverage; this batch does
not mislabel them as proof created by the earlier case contract.

## Defects exposed and fixed

The batch found and fixed two product defects:

- bytecode and VM failures were collapsed into coarse runtime variants or
  exposed only through changeable display text, losing the originating machine
  category; and
- HMI write rejection responses omitted a stable error identifier even though
  they correctly rejected and did not queue invalid values.

Validation also exposed and fixed two test-infrastructure defects:

- the case runner emitted an absolute temporary case-file path, preventing
  authentic `prove.py` red evidence from binding to the committed case file;
  and
- the full Rust gate found one pre-existing unit test still asserting the old
  `RuntimeError::InvalidBytecode` shape. Commit `54504670` updates that test to
  assert the structured `RuntimeError::Bytecode { code, detail }` contract.

The canonical focused suite then detected five intentionally moving census
tripwires caused by the new Rust facts and catalog mappings. Commit `b2f0ce18`
refreshes only those measured counts: 3,961 scanner facts, 179 mapped facts,
3,782 unmapped facts, and 72 `trust-runtime-core` unit facts.

## Validation

On the clean trust-builder checkpoint
`54504670d2bad0c349bb4febff5b1140c08d5d6d`:

- `just fmt` passed.
- `just clippy` passed.
- `just test-all` passed: 3,195 passed, 27 ignored, 0 failed across 232
  reported result groups.
- `cargo test -p trust-runtime --test api_smoke` passed 3/3.
- `cargo test -p trust-runtime --test debug_control` passed 20/20.
- `cargo test -p trust-runtime --test complete_program` passed 1/1.
- `cargo test -p trust-runtime --test runtime_reliability` passed 4/4.

The bytecode-validator mutation shard was rerun from clean commit
`f305dd45f98bdbbe282985dad4fcbe82c5132fb3` with cargo-mutants 27.0.0:
2 caught, 0 survived, 0 unviable, 0 timeout, and 0 infrastructure error. The
committed shard JSON digest is
`sha256:88e97c57ed47e698f5c3c8d33611e3d8f0a4cdae0135ee84451bfa10c00713a2`.

The canonical focused Python suite passed 772/772 at the final report source
checkpoint. Metadata validation reported 528 records before this batch record
was indexed.

## Generated report refresh

All 15 installed report pairs were generated from the pristine detached
checkpoint `b2f0ce18257585e741dbfeadc9a6d9852cbf7cf4` with timestamp
`2026-07-16T02:44:00+02:00`. Every generator and production at-rest validator
exited zero, and the detached worktree returned clean.

| Report | JSON SHA-256 |
| --- | --- |
| Test-class completeness | `f8ffaf8a86c886c17776ac77a22a3c2fe9b853faa102487e925fd1a27d3c0aae` |
| Coverage-matrix gaps | `14f997b2f49854973537ca82ef1b32ab479d015bc6fa2257c3d9b707daea09e3` |
| Malformed-input coverage | `2ae585a082fd7d4678dc8067a56e798aee7ce409bd92808eefefc94ad6df11f7` |
| Unmapped-test debt | `c22034e725be0eb30777d9201d1c457c044f752768c960b718e32ecbd75a4c78` |
| Test-refactor assessment | `bc444794744d294f2888bbe4e250afdacd7251b217b16a4e0335d6fb93d646ff` |
| Ignored-test inventory | `5b5ad834d33c9b38388abd22d684d637bb884ce753d1164358fa95521fe3980d` |
| Phase 5 suite audit | `70b82af53ee8d5d39f95c681e643ef647fcfefaa8f3659829e5a81ed8cebc9d7` |
| Invariant-seed audit | `3796a77f87e400bd23c4c6926019a6d275a82d6d7f40d27ad4d3da0c036fd16c` |
| Specification completeness | `9323f814e833662484c332b6cbd904e19c2310d27d8f15ddf155132b5fd0a65e` |
| Requirement/oracle audit | `ac5e1b32bd3a2709f13f14a0f99d21d1e3846d9f2ae89983ce85b912cb624923` |
| Conformance alignment | `4c1a6780a002c7a126fb005309c7ba12d3531126057d192946a7abe9e1b3401d` |
| Runtime-anomaly audit | `19fdbe74c0025c6d0ee875318dda24207ef8afa85cf9eaadd4304a84fc00dcf0` |
| Fuzz-program audit | `874ca49c47c65d987dc2f99e798797d3d3aabb8c02b1e27add56ffa92db09959` |
| Mutation program | `0ebef9c715a10a461e553e6a3a4382a7f65824e394e17aefd30e8d61702c4e15` |
| Specification-source audit | `f64c0f4f9e83b563bb17ca63308beb1c0c93986dc16c8f30bc2afeca8fbfb8e6` |

## Honest remaining posture

- `SPEC_GAP_VM_ERROR_MODEL_001` is `spec_updated`, not closed. The authentic
  red/green case proof predates the broader nine-test catalog contract. Closing
  the gap now would require rewriting a proof-bound contract after execution,
  which is forbidden. Closure therefore needs a separately reviewed contract
  migration or a new pre-bound execution contract; this batch does not bypass
  that requirement.
- The gap register contains 15 open, 4 test-mapped, 1 spec-updated, and 14
  closed records.
- The invariant register contains 53 records: 44 at S0 and 9 at G2. No
  invariant was promoted by this batch.
- The coverage matrix still reports 63/80 required slots missing.
- The requirement/oracle audit still reports 25/53 invariants without an
  eligible oracle.
- The test catalog maps 179/3,961 scanner facts; unmapped-test debt remains
  3,782 facts.
- Conformance alignment remains 0/21 explicitly linked.
- CI, workflows, suites, and approved proof producers are unchanged.
- Version metadata is synchronized at 0.24.49; tagging and public release are
  deferred until the change reaches `main`.
- No subsequent product batch was started before this one was closed out.
