# Bytecode Decoder Boundary Batch Validation

Date: 2026-07-14

This record closes the validation batch for two independently reproduced
bytecode-container defects. It does not close either owning specification gap,
promote an invariant, or claim proof.

## Product Results

### Untrusted collection counts

Clean red revision: `7fe5015f8ae559a1f7862b7e88e3ea4cdfcac415`

`cargo test -p trust-runtime --test bytecode_container` produced seven failures.
Each new boundary test observed `UnexpectedEof` after the decoder had already
used an attacker-controlled count in `Vec::with_capacity`. The decoder had not
proved that the claimed collection could fit in the remaining section bytes.

Green product revision: `36837353b993e906c2b27d1cb0ad969bb7d1da86`

The decoder now checks the count with checked multiplication against the
minimum encoded entry width before allocating. The check covers top-level and
nested collections without introducing an arbitrary resource cap. The focused
builder run passed all seven new resource-bound tests and 54 adjacent bytecode
tests.

Cataloged regression tests:

- `TEST_BYTECODE_STRING_TABLE_COUNT_BOUND_001`
- `TEST_BYTECODE_TYPE_TABLE_COUNT_BOUND_001`
- `TEST_BYTECODE_FIXED_SECTION_COUNT_BOUND_001`
- `TEST_BYTECODE_NESTED_TYPE_COUNT_BOUND_001`
- `TEST_BYTECODE_NESTED_REFERENCE_COUNT_BOUND_001`
- `TEST_BYTECODE_NESTED_POU_COUNT_BOUND_001`
- `TEST_BYTECODE_NESTED_RESOURCE_COUNT_BOUND_001`

### Duplicate standardized sections

Clean red revision: `cf8987658931e5097ae890cf26f90bb9557afc3d`

`duplicate_standard_section_ids_are_rejected` failed because the decoder
accepted two standardized sections with the same identifier and later selected
the first occurrence. The table-driven test covers all twelve standardized
section identifiers.

Green product revision: `5fb897f7e36538c22cb1de97e231f613af9c6870`

The decoder now tracks standardized section identifiers in a set and rejects a
duplicate before section selection or execution. The focused builder run
passed 62 bytecode tests.

Cataloged regression test:

- `TEST_BYTECODE_CONTAINER_DUPLICATE_STANDARD_SECTION_001`

## Broad Builder Gate

Product/evidence checkpoint: `708fdd78caa8a202a095fa0315cec2132e09a8a4`

Remote checkout:
`$HOME/projects/trust-platform-bytecode-count-502fff48`

The following commands ran once on `trust-builder` with
`CARGO_TARGET_DIR=$HOME/.cache/codex-targets/trust-platform-gate` and exited
zero:

- `just fmt`
- `just clippy`
- `just test-all`
- `cargo test -p trust-runtime --test api_smoke` (`3/3`)
- `cargo test -p trust-runtime --test debug_control` (`20/20`)
- `cargo test -p trust-runtime --test complete_program` (`1/1`)
- `cargo test -p trust-runtime --test runtime_reliability` (`4/4`)

The bytecode-validator mutation shard was also rerun from clean revision
`8437e60842680bf07236fbd40f5ba23926463520`: two mutants were caught, with
zero survivors, unviable outcomes, timeouts, or infrastructure errors. The
committed shard report digest is
`sha256:6e2c0eeb9fd016823bc6c62561bf37680a1b84fdb303e3e3dec8077dafe44e38`.

## Report Refresh

Clean source revision: `86cda273f7cbfbf0b06b1eead5cb751dc77eb1a9`

Timestamp: `2026-07-14T23:07:00+02:00`

All fifteen generators ran from a pristine checkout on `trust-builder`; each
generated pair then passed its own at-rest validator. The source revision also
contains a tests-first correction for the specification-source audit: the
source-transformation topic no longer references the already-closed
`SPEC_GAP_VM_LOWERING_FAIL_CLOSED_001` gap.

| Report | JSON SHA-256 |
| --- | --- |
| Test-class completeness | `251b7c82282a9c81fe20e7284903886694cd1d90d2d020a41b7cdc5af30bb4bc` |
| Coverage-matrix gaps | `5f248d8199f9fe7d50d4fb0248f8a988aef9ccf013c46105934f3634a96387f3` |
| Malformed-input coverage | `4cbe700139f777e53411dc436afb5a833792f215527bd6ae44ab1880e48962e7` |
| Unmapped-test debt | `415bd9dad3b3ebf4f0f1cb8ff142f6ec976a857caaa3bb9c1fedd37c0ed161f9` |
| Test-refactor assessment | `f2ff86c6359ee7986d23b731d6687850e114a2078e45de6eedf8bda25d230aeb` |
| Ignored-test inventory | `98aee04acf3ac3292c3ecfab1f5fa0536679ed42ce0aaf88b3975113000ceb69` |
| Phase 5 suite audit | `456900252b6a4c88be658f5d4bb6eab0645c81cc79c2908dc42b43591e9f6497` |
| Invariant-seed audit | `814be3422cc2074883a9ae7cef267d0a3c157bfd7e8963899d016a5923ccd0af` |
| Specification completeness | `af8be5b60a6caaa36c996005fa7b92723e112527bfe4d8837db998473d23f8a4` |
| Requirement/oracle audit | `fc4ff3030c898104c092b33851da6865acea4b6d387dc98e8be7b9ae61177783` |
| Conformance alignment | `ab8f1d88da440b9e506cf6da1ff7e788adeed49ce0656154daf4374a2325dc3a` |
| Runtime-anomaly audit | `0a99891eba92b9c7fed87aeb8cb17e974abae85d63430ad2f9349c2261993283` |
| Fuzz-program audit | `3ef36d89a10f0ad1ae9604fac8d16b2447d33ca8e1cbd581d9a3016546d23637` |
| Mutation program | `f45dc9fa2be506877b9a8ae581d4bd158c88b5ebbca1e877cd8e60cd347f562c` |
| Specification-source audit | `e3d98647a8b25e3ef45b125706fd2a7a8bbf97d8406f9e5b6449054d93a2a756` |

Measured posture after the refresh:

- `128/3914` scanner facts are test-class classified.
- `2/28` malformed-input classes have explicit catalog mappings.
- `28/53` invariants remain insufficiently specified.
- `24/53` invariants have an eligible oracle; `29/53` do not.
- `21/21` conformance cases remain explicitly unlinked.
- The runtime-anomaly audit retains nine test gaps.
- The fuzz-program audit retains six surface gaps.

## Boundaries

`SPEC_GAP_VM_DETERMINISM_RESOURCE_LIMITS_001` and
`SPEC_GAP_BYTECODE_VALIDATOR_001` remain open. `VM_SEAM_DETERMINISM_LIMITS_001`
and `VM_SEAM_VALID_001` remain at `S0` with `spec_gap` status. No workflow,
suite, proof producer, or enforcement setting changed in this batch.
