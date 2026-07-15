# Bytecode Validator Contract Closeout

- Date: 2026-07-15
- Implementation revision: `38b6d09592913e373441511ccc8b5411938dc63f`
- Evidence posture: specification closeout, focused characterization, and
  mutation adequacy; this is not producer-authentic red/green product proof.

## Closed Boundary

`docs/specs/12-bytecode.md#validator-before-apply` now specifies the complete
structural and semantic acceptance boundary before a candidate STBC module may
change runtime configuration or executable state. The contract covers required
sections, table and metadata references, owner and frame-local reference rules,
instruction decoding, calls, jumps, stack dataflow, and no-partial-apply
behavior.

The complete committed transform seed produces seven runnable negative cases.
A dedicated product-path runner checks each case through direct decode and
validation and through `Runtime::apply_bytecode_bytes`, including unchanged
observable runtime state after rejection. Eleven existing Phase 11 validator
tests were removed from quarantine and now run in the normal suite. Additional
focused tests cover missing sections, jump instruction boundaries, incomplete
reference owners, unsupported schema tags, checksum, version, call targets,
and standardized section shape.

The hand-owned catalog maps 26 test records to this closeout. The malformed
input audit measures 23 required classes covered by 32 explicit associations.
Only the five numeric resource-limit classes remain `spec_gap` under
`SPEC_GAP_VM_DETERMINISM_RESOURCE_LIMITS_001`.

## Results

The focused runtime set passed 93 tests on `trust-builder`:

- `bytecode_container`: 7 passed;
- `bytecode_decode_resource_bounds`: 7 passed;
- `bytecode_validation`: 13 passed;
- `bytecode_verification_cases`: 2 passed;
- `bytecode_vm_core`: 42 passed; and
- `phase11_seam_contract`: 22 passed, 0 ignored.

The focused verification-tooling set passed 119 tests. The catalog staleness
join and metadata validator then failed only on the deliberately stale prior
mutation artifact, as required before a clean rerun.

The bytecode-validator mutation shard ran from a clean archive of the
implementation revision with `cargo-mutants 27.0.0`. Both reviewed mutants
were caught: 2 caught, 0 survived, 0 unviable, 0 timeout, and 0 infrastructure
error. The regenerated machine report is
`docs/internal/testing/evidence/plc-verification-program/2026-07-08/p1b-bytecode-validator-mutation-report.json`
with SHA-256
`329a2d46a14eb0c1f29c9dfac6f94ba112529a0daecb3b48f47038f429db9dc8`.

## Product Finding

No bytecode-validator acceptance defect reproduced. Every new adversarial
input was already rejected by the shipped decoder or validator, including the
product apply path. This batch therefore changes specifications, tests, and
verification metadata, but does not alter runtime product behavior and does
not manufacture a red/fix/green proof chain.

## Preserved Gaps

`SPEC_GAP_VM_ERROR_MODEL_001` remains open because current Rust error variants
and diagnostic strings are not declared stable public identifiers. Numeric
container, instruction, stack, local, reference, call-depth, and execution
budgets remain open under
`SPEC_GAP_VM_DETERMINISM_RESOURCE_LIMITS_001`. No proof level, CI enforcement,
suite producer, skill, or agent instruction changes in this closeout.

