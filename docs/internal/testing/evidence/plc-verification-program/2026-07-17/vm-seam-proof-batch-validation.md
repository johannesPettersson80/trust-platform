# VM seam case-backed proof batch validation

Date: 2026-07-17

Case and runner foundation checkpoint:
`5ac4a12b581ab60e6453ee067df2194fa3159453`

Final proof and promotion checkpoint:
`a308633477c2c99f0d3a646504ebd9c042a117e0`

Final report source checkpoint:
`76b7a7fc03e71a8b4b6b19daa9bf8a06c783b3fd`

## Outcome

This batch converted seven bytecode-VM seam contracts from written metadata
without executable proof into case-backed targeted proof:

- declared-type conversion policy: 14 cases;
- bounded STRING behavior: 5 cases;
- subrange behavior: 5 cases;
- encoder fail-closed behavior: 3 cases;
- frame/instance owner selection: 2 cases;
- reference-escape rejection: 2 cases; and
- bytecode-validator rejection before partial apply: 7 cases.

All 38 case rows passed against unchanged product behavior. The batch found
missing executable assertions, not a new product defect, so it did not
manufacture a red result or modify runtime/VM product code. A separate positive
integration regression also proves that an STBC v1 unknown section with flags
zero is raw-preserved, accepted by validation, and ignored by product apply as
specified in `docs/specs/12-bytecode.md`.

The seven VM invariants are now `implemented` at G1. The existing precedence
and PLCopen import case contracts were rebound through new authentic lock pairs
after their generated-case provenance migrated. The register now contains 54
invariants: 19 at S0, 26 at G1, and 9 at G2. The bytecode-VM area contains seven
G1 invariants and one G2 invariant.

## Tests and catalog binding

The new and existing case-backed runners are cataloged as:

- `TEST_VM_DECLARED_TYPE_TRACE_001`;
- `TEST_VM_STRING_BOUND_TRACE_001`;
- `TEST_VM_SUBRANGE_TRACE_001`;
- `TEST_VM_ENCODER_FAIL_CLOSED_TRACE_001`;
- `TEST_VM_OWNER_TRACE_001`;
- `TEST_VM_REF_ESCAPE_TRACE_001`; and
- `TEST_BYTECODE_VALIDATOR_CASES_001`.

The optional-section integration regression is cataloged as
`TEST_BYTECODE_UNKNOWN_OPTIONAL_SECTION_001`. Its final ownership is the
dedicated `bytecode_optional_sections` test binary. Keeping that positive
runtime-apply contract out of the negative malformed-container test file also
preserves the existing reviewed refactor assessment without weakening its
mixed-purpose rule.

The stable VM error-model gap is closed against the previously committed exact
identifier tests. The gap register now contains 24 closed and 11 open records;
the eleven unresolved specification questions remain visible under
`VERIF-P16-002`.

## Authentic proof records

`prove.py v1` wrote nine clean lock baselines followed by nine clean descendant
lock comparisons. The seven VM pairs, the replacement `IEC_PREC_001` pair, and
the replacement `PLCO_IMPORT_001` pair have distinct run IDs, matching
case-file and execution-contract digests, and identical all-passing per-case
summaries. No broad-gate evidence was relabeled as targeted proof, and no
invariant was promoted beyond the evidence-supported G1 level by this batch.

## Defects exposed and fixed

The batch exposed and fixed two verification-tooling defects:

- generated-case `source_digest` included invariant proof lifecycle fields,
  causing the evidence update authorized by a passing run to invalidate that
  run's own case provenance; the digest now binds the invariant execution
  contract while still changing for executable behavior changes; and
- `prove.py` lock evidence IDs were permanently single-use, so an honest
  proof-contract migration collided with historical evidence; the narrow
  `--rerun-label` option now versions only replacement lock pairs and rejects
  invalid or mismatched labels.

Focused regression tests were added before each tooling fix. The case work also
caught and corrected two fixture defects before proof: owner/reference expected
detail text did not match the written contract, and the optional-section
fixture initially used a module that product apply could not resolve. Neither
fixture correction changed product behavior.

## Mutation evidence

The bytecode-validator mutation shard was rerun on `trust-builder` from clean
checkpoint `5ac4a12b581ab60e6453ee067df2194fa3159453` using cargo-mutants
27.0.0. Both mutants were caught; there were zero survivors, unviable mutants,
timeouts, or infrastructure errors. The committed report digest is
`sha256:ea49674613d05b9272d5bd9eadfe2fdc46bb249607b2ce9e56da86d14326ee11`,
and it binds the final validator case-file digest
`sha256:442c2bbb5d72ef43dd82fb20f23ee423bf9a44440801f6bd4829b81ecf4d9643`.

## Validation cadence

Focused checks ran continuously before the final heavy checkpoint, including
the seven case-backed VM runners, the migrated precedence and PLCopen runners,
51 prover tests, 78 mutation/metadata tests, 72 live-census tests, metadata
validation, case regeneration checks, and the mutation shard. The final moved
optional-section regression passed 1/1 on the clean builder checkpoint
`76b7a7fc03e71a8b4b6b19daa9bf8a06c783b3fd`.

The only broad checkpoint ran on `trust-builder` at
`a308633477c2c99f0d3a646504ebd9c042a117e0`:

- `just fmt`: passed;
- `just clippy`: passed;
- `just verification-veryquick`: passed, including 812/812 focused Python
  tests and metadata validation of 632 records; and
- `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=4 just test-all`: passed with no test
  failures.

The first `test-all` attempt stopped before a test result because the generated
target exhausted builder disk. Following `AGENTS.md`, no result was counted;
remaining build processes were checked, only generated target/cache output was
removed, disk capacity was rechecked, and the command was retried once with
incremental compilation disabled. The successful generated target was then
removed, restoring approximately 62 GiB free. No broad gate was rerun after the
test-only ownership move or the report/evidence-only closure.

## Generated report refresh

All 15 installed report pairs were regenerated from independent pristine
worktrees at `76b7a7fc03e71a8b4b6b19daa9bf8a06c783b3fd` with timestamp
`2026-07-17T02:39:00+02:00`. Every generator and production at-rest validator
exited zero.

| Report | JSON SHA-256 |
| --- | --- |
| Test-class completeness | `b8c8e81536dca1225076a2d5895eac8500f217f83ce1d91cf62b0043472b1082` |
| Coverage-matrix gaps | `d556f5bd456e33bf706b36ccd5585d735d5dc50ead5845a2198795faff9859e9` |
| Malformed-input coverage | `aef09ed546294b874c724024d07f05b45981de387fe073169e1d1ed9244a66b0` |
| Unmapped-test debt | `2afad40234fc8534dc681346961960abdd58db48ded121ff8df9ad230b835822` |
| Test-refactor assessment | `b5743549c7cd4085a255c87f7c9201093f5ed49777d01fdb6975848ab94c9e06` |
| Ignored-test inventory | `be8bb1ed013d9fc0bbb0b1cfc440c0f22e73906fa0b33e0615969e0916bd694f` |
| Phase 5 suite audit | `a817cbcf1981e24107a8508981e6ee7c2c9f064fcfcfba6b1f1647f119882425` |
| Invariant-seed audit | `0f1cd857e6ab88729568e13378e56f7a42c2fc4db5a48035b89ce57444b14b93` |
| Specification completeness | `0238b7941b32ba4ae641a44d66f7828f1b612b06bc4bd7ef1e6c49dc05a9d6cd` |
| Requirement/oracle audit | `ed41f7fef3a6be4fe669c86edeb61057cb2e790623c7036a63ab19948ee64b48` |
| Conformance alignment | `bf114c7a114505055bbcf6855ddc0ebba72b63323f6aee2462338ed5f3adf89b` |
| Runtime-anomaly audit | `eaa3b9f9b1c42938dc8a847f41f4e60439eb61a9b4aba0d6829c6d7ed2ff492b` |
| Fuzz-program audit | `7b4af469f859218ea38a774a95e271adfe2a56a945b2b0647690a612e1abcaf6` |
| Mutation program | `595c4efc26950f5bf7d1b39f40bbaa3bfa9e6dff8f78ca862b31c24edc509660` |
| Specification-source audit | `9c1a0ddf8512c1c1b6c668e2dedf492303d9f39fce86ab73e6c8dcfcaf726668` |

## Honest remaining posture

- Eleven specification gaps remain open.
- Fourteen of 54 invariants still lack an eligible specification/oracle
  binding, and 19 invariants remain at S0.
- The required coverage matrix still has 63 of 80 slots missing.
- The hand-owned catalog maps 201 of 3,985 scanner facts; the remaining 3,784
  facts are catalog debt, not automatically missing product tests.
- Conformance alignment remains 0 of 21 explicitly linked.
- CI, workflows, suites, approved proof producers, and enforcement posture are
  unchanged.
