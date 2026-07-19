# VM fixed resource-limit batch validation

Date: 2026-07-15

Final report source checkpoint: `1242961436eeadfb52cd34328e2ca75de2fb6cb9`

Report refresh commit: `78928c667912b307d4782f69c596c3798f544bb7`

## Outcome

This batch converted `SPEC_GAP_VM_DETERMINISM_RESOURCE_LIMITS_001` from an
unspecified resource-limit gap into written product contracts, committed trace
cases, authentic red and green proof, and a causal broad-gate record. The gap
is closed and `VM_SEAM_DETERMINISM_LIMITS_001` is implemented at G2.

The red run found six missing product defenses: decoded instruction count,
reference count, POU local count, POU parameter count, operand-stack depth,
and executed instructions per top-level invocation. The encoded-container and
active-call-depth defenses already existed, and the within-limit OSCAT case
passed before and after the fix.

The fixed STBC version 1.x limits are:

| Resource | Maximum |
| --- | ---: |
| Encoded container | 67,108,864 bytes (64 MiB) |
| Decoded module instructions | 1,000,000 |
| Module references | 65,536 |
| POU locals | 65,536 |
| POU parameters | 1,024 |
| Native-call arguments | 1,024 |
| Operand-stack values | 16,384 |
| Active call frames | 1,024 |
| Executed original bytecode instructions per top-level invocation | 1,000,000 |

These are implementer product limits, not IEC deviations. Stable public error
identifiers remain explicitly open under `SPEC_GAP_VM_ERROR_MODEL_001`.
Deadline and watchdog timing also remain outside this proof.

## Tests first and proof chain

- Commit `700a3ad21633810ab79d4467a8403a323a80d860` added the hand-authored
  nine-case trace contract and cataloged runner.
- `EVID_TEST_VM_RESOURCE_LIMIT_CASES_001_RED`, produced by `prove.py v1`,
  recorded six genuine failing cases at that clean commit.
- Commit `4bb981280b85c233376e050e531ead70f19ff58d` added the shared limits and
  enforcement in decode, validation, materialization, and all VM backends.
- `EVID_TEST_VM_RESOURCE_LIMIT_CASES_001_GREEN`, also produced by
  `prove.py v1`, recorded all nine cases passing with the same case-file and
  proof-contract digests as the red run.
- `EVID_BROAD_REMOTE_PR_20260715_81EA8F2854DB` records the causal clean
  trust-builder descendant `786576bcdd346362274133e2476e2dfec1f987a4`.
- Commit `4c832eff` promoted `VM_SEAM_DETERMINISM_LIMITS_001` to G2 after the
  broad record existed. No invariant was promoted beyond its evidence.

The stack, register, and tier-1 VMs share one original-bytecode instruction
budget. Nested calls consume the same top-level budget, and fused or expanded
register IR cannot reduce the charged original-bytecode cost.

## Defects exposed by validation

The broad and focused gates also caught three non-product issues, which were
fixed before final evidence was recorded:

- A pre-existing nested-POU section-bounds test used a count above the new
  fixed parameter cap and therefore no longer reached the section-bounds
  condition it intended to test. Commit `786576bc` moved its fixture to the
  allowed maximum without weakening either contract.
- The generic mutation-program selector duplicated a source line that shifted
  after the resource-limit validation code was added. Commit `dc9ea688`
  rebound the reviewed selector to the live cargo-mutants location.
- Five reviewed live-census assertions still described the pre-batch test and
  oracle populations. Commit `12429614` refreshed only those measured
  tripwires after the canonical focused suite exposed them.

## Validation

On the clean trust-builder product checkpoint:

- `just fmt` passed.
- `just clippy` passed.
- `just test-all` passed.
- `cargo test -p trust-runtime --test vm_resource_limit_cases
  vm_resource_limit_trace_cases -- --exact` passed all nine trace cases in the
  same broad run.
- `cargo test -p trust-runtime --test api_smoke` passed 3/3.
- `cargo test -p trust-runtime --test debug_control` passed 20/20.
- `cargo test -p trust-runtime --test complete_program` passed 1/1.
- `cargo test -p trust-runtime --test runtime_reliability` passed 4/4.

Focused product runs also passed runtime-core VM tests (9/9), bytecode resource
validation tests (2/2), stack/register/tier-1 VM tests (102/102), and the
nine-case trace runner.

The bytecode-validator mutation shard was rerun against the clean
implementation: 2 caught, 0 survived, 0 unviable, 0 timeout, and 0
infrastructure error. The committed shard JSON digest is
`sha256:11e09fbe3c2a3c549b02a018811dc933c4b650ad6d6c45f8a8d8afa33c828069`.

The canonical focused Python suite passed 772/772 at the final report source
checkpoint. Metadata validation reported 521 records before this batch record
was indexed.

## Generated report refresh

All 15 installed report pairs were generated from the pristine detached
checkpoint `1242961436eeadfb52cd34328e2ca75de2fb6cb9` with timestamp
`2026-07-15T23:48:32+02:00`. Every generator and production at-rest validator
exited zero.

| Report | JSON SHA-256 |
| --- | --- |
| Test-class completeness | `db801d0424de86f19bc873d22625d9106a8e86b400555abd4565be9d4e4fb09d` |
| Coverage-matrix gaps | `62a54939261ff552c9ecbd4fe5dac4b7f63384d19d5ef3312fd6b28876dfd575` |
| Malformed-input coverage | `86f3beb4986c5c9b8cb8dbbcbb616c413e16f6a89b5d46222003f71271664332` |
| Unmapped-test debt | `2d48a2db2cbf3c941c4f4e234d9024cf74466e676e136c6371c953bae3fff2ff` |
| Test-refactor assessment | `a0d6783c45019e0d525a0ee6cab60cfc6d8739557eb23ef61de01fe2f908ce32` |
| Ignored-test inventory | `adb9fbcaae76c39ed329652db9e197078c6adc8e511c0cf2d22dea06ea90dbfa` |
| Phase 5 suite audit | `8640f46635d91c9656ab995e1cebee2f7634862ce1fd875c11e28b6b3dd8da8a` |
| Invariant-seed audit | `543757f6c99eeb072ae51895a20a18c164af565cf9b0573dfa31abdbe4c06df6` |
| Specification completeness | `28be9eec358dcf1725d16823ffe92bf7677b03e060fb928579099bd40c1d0707` |
| Requirement/oracle audit | `96df2977d1033cb961d5af33755366df0f0ebe9723cacb1684efed3e48d27df4` |
| Conformance alignment | `1c8b2940d6b6e755440e901a7166a6c75f21a9a518c02e8d9a79bf4e43b22643` |
| Runtime-anomaly audit | `1cab7b839d3b51965834f0dde16005c8d7a3d10381de731e4a443d2171abc018` |
| Fuzz-program audit | `86e1277204ef2f55b4d1b29578f467d2de358fd34d3d052159bb537819fa98fe` |
| Mutation program | `94a36e3d6fdee757856b707bf085175ab57ba6b20e2142cc3656d668033aca2b` |
| Specification-source audit | `9ec29608ad048cac46bce4782912163e126a83f41e916953de44bbec224d0700` |

## Honest remaining posture

- The gap register contains 16 open, 4 test-mapped, and 14 closed records.
- All 34 gap records retain the `spec_gap` semantic status; resolution state is
  tracked separately.
- The invariant register contains 53 records: 44 at S0 and 9 at G2.
- The coverage matrix still reports 63/80 required slots missing.
- The requirement/oracle audit still reports 25/53 invariants without an
  eligible oracle.
- The test catalog maps 175/3,957 scanner facts; unmapped-test debt remains
  3,782 facts.
- `VERIF-P16-002` remains open because this batch closes one gap, not all gaps.
- `SPEC_GAP_VM_ERROR_MODEL_001` remains open.
- CI, suites, workflows, and approved proof producers are unchanged.
- Version metadata is synchronized at 0.24.48; tagging and public release are
  deferred until the change reaches `main`.
