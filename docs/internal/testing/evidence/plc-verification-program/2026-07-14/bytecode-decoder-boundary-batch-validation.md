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

Clean source revision: `27694b329a62206b51cb8392378d6eb9ee0fd8e2`

Timestamp: `2026-07-14T23:30:00+02:00`

All fifteen generators ran from a pristine checkout on `trust-builder`; each
generated pair then passed its own at-rest validator. The source revision also
contains a tests-first correction for the specification-source audit: the
source-transformation topic no longer references the already-closed
`SPEC_GAP_VM_LOWERING_FAIL_CLOSED_001` gap.

| Report | JSON SHA-256 |
| --- | --- |
| Test-class completeness | `cf6a4fa0b360716a94967e710aebaa46bf1faaa06be95ee72904f0b4bcb55a81` |
| Coverage-matrix gaps | `0edb1ee5c55d19c9bc6db02c157e9b3eb71a4dcffc3e8ea32a3ef282f374ef19` |
| Malformed-input coverage | `a142d9d01fc27864b94e49c769803c6aed01e0b66e3fd1bb3fa464047a01f587` |
| Unmapped-test debt | `68b459824cf289fbb64c6b2616b8d5e72f16a066c34ce81927367e9030752056` |
| Test-refactor assessment | `5c133dc8e9f6b9d7b295b0dc1580b768e6e52b9a0c0db67ae086438c793af8e1` |
| Ignored-test inventory | `19e2ed2c41a70e92be363df798ea31610a18b5fc419fe07eef828e791a77277f` |
| Phase 5 suite audit | `8d2ee9c03f963b5c0dc04c286de1f14411360cf146ffc9ec812b0aa4b9c97629` |
| Invariant-seed audit | `86fb268f6c37856c3a9af47da3a236893ef8e3da91652926fc808c665526ae48` |
| Specification completeness | `2548fc2f97e0987070d304c709a89dd7e2593a5bc55a6238cb875505aa3ba791` |
| Requirement/oracle audit | `7349bb053fe5fda7beb24affe1023c4005f41f3a4a40b671afb6668b4f4406bc` |
| Conformance alignment | `592cc23d07015b4c9cc67bdac04f53b785a81025f03aaca1000bf485060b31fa` |
| Runtime-anomaly audit | `59a8c42a25bf3f02476a11bc2326fbf3bf4fddbc4502fc40629f0b7dc469df2d` |
| Fuzz-program audit | `0c20bd678d395e994b4208c0c645a37501ec0bbc4eec92cc48481ab71f94cd94` |
| Mutation program | `68ee736477932062ee66697ee70e409a992db146dc3f8607b121330226029db7` |
| Specification-source audit | `4e560e163d77733db0eac9b21d8351cae06129b3ce5c81d6e0704c53075b030c` |

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
