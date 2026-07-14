# Bytecode lowering fail-closed specification closeout

Date: 2026-07-14

Focused source: `c0c8540e644dfa8df842cf41f24c391e6c36eb15`

## Missing contract and test activation

`SPEC_GAP_VM_LOWERING_FAIL_CLOSED_001` did not say which source-to-bytecode
boundary must reject an unsupported construct or how a valid emitted prefix is
handled. The contract is now written in `docs/specs/12-bytecode.md` under
"Source-to-Bytecode Fail-Closed Boundary".

The focused coverage binds three real scanner facts:

- supported `EXIT` and `CONTINUE` loop-control lowering and execution;
- rejection of an analyzed source `JMP` statement whose bytecode lowering is
  unsupported; and
- rejection of an analyzed executable array-initializer assignment after an
  otherwise supported assignment, proving that no partial module is returned.

The array-initializer test was previously quarantined. It was reactivated and
strengthened so the fixture must first build the runtime model successfully and
then fail bytecode-module construction. This prevents a parser or semantic
error from falsely satisfying the lowering test.

A proposed structure-assignment fixture was removed after this stronger
boundary check proved that its syntax was rejected before lowering. It is not
counted as evidence.

## Focused result

All three lowering partitions passed against the unchanged product
implementation on `trust-builder`:

```text
CARGO_TARGET_DIR=$HOME/.cache/codex-targets/trust-platform-gate \
  cargo test -p trust-runtime --test bytecode_vm_core \
  vm_lowering_supports_exit_and_continue_in_loop_stmt_paths \
  -- --exact --nocapture
# 1 passed; 0 failed; 41 filtered out

CARGO_TARGET_DIR=$HOME/.cache/codex-targets/trust-platform-gate \
  cargo test -p trust-runtime --test phase11_seam_contract \
  unsupported_array_initializer_assignment_fails_build_instead_of_nop \
  -- --exact --nocapture
# 1 passed; 0 failed; 21 filtered out

CARGO_TARGET_DIR=$HOME/.cache/codex-targets/trust-platform-gate \
  cargo test -p trust-runtime --test bytecode_vm_core \
  vm_lowering_rejects_unsupported_c5_edge_case_stmt_paths -- --nocapture
# 1 passed; 0 failed; 41 filtered out
```

## Result and posture

No product defect reproduced. The existing encoder already fails closed for
the reviewed unsupported statement and expression partitions. This closeout
changes a product specification, test activation/assertion strength, and
verification metadata; encoder implementation is unchanged.

`VM_SEAM_ENC_001` remains `spec_gap/S0` because the separate stable typed error
identifier contract is still open under `SPEC_GAP_VM_ERROR_MODEL_001`.
Ordinary focused tests are not producer-authentic red/green proof, and no broad
gate was rerun for this no-product-change closeout.

## Boundaries

- No runtime implementation, validator, schema, suite, workflow, approved
  proof producer, or CI enforcement changed.
- No product bug, proof, invariant promotion, or public claim is asserted.
- The Phase 3 ignored-test report and other commit-bound reports are refreshed
  only at the next product-change batch milestone.
