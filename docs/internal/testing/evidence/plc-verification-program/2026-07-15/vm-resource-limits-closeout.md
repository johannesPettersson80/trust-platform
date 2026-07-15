# VM Fixed Resource-Limit Closeout

Date: 2026-07-15

## Scope

This closes `SPEC_GAP_VM_DETERMINISM_RESOURCE_LIMITS_001` for the fixed STBC
version 1.x resource limits written in `docs/specs/12-bytecode.md` and the
per-invocation instruction-budget semantics written in
`docs/specs/10-runtime-semantics.md`. The limits cover encoded container size,
decoded module instructions, module references, POU locals and parameters,
native-call arguments, operand-stack values, active call frames, and executed
instructions per top-level invocation.

This closeout does not define stable public error identifiers. That remains
open under `SPEC_GAP_VM_ERROR_MODEL_001`. Deadline and watchdog timing
interactions also remain explicit invariant debt; they are not inferred from
the fixed instruction-budget proof.

## Product Evidence

- Red source commit: `700a3ad21633810ab79d4467a8403a323a80d860`.
- Red proof: `EVID_TEST_VM_RESOURCE_LIMIT_CASES_001_RED` recorded six genuine
  failures for instruction, reference, local, parameter, operand-stack, and
  executed-instruction limits. The pre-existing container and call-depth
  defenses and the within-limit OSCAT fixture passed.
- Product implementation: `4bb981280b85c233376e050e531ead70f19ff58d`.
- Green proof: `EVID_TEST_VM_RESOURCE_LIMIT_CASES_001_GREEN` recorded all nine
  committed cases passing with the same case-file digest and proof-contract
  digest as the red run.
- Stack, register, and tier-1 backends share original-bytecode instruction
  accounting, including nested calls and fused/expanded register IR.

## Metadata Closeout

- `REQ_SPEC_VM_DETERMINISM_AND_RESOURCE_LIMITS` now resolves to
  `SPEC_BYTECODE_FORMAT_001`.
- The eight affected catalog tests resolve directly to written oracle
  sections, with no live reference to the closed gap.
- The five resource-limit malformed-input classes are `required` and map to
  `SPEC_BYTECODE_FORMAT_001#fixed-resource-limits`.
- `VM_SEAM_DETERMINISM_LIMITS_001` is `implemented` at `G1` with the resource
  cell covered. Promotion to `G2` remains blocked until a causal broad remote
  gate is recorded.

## Validation

Focused product validation on `trust-builder` passed:

- `cargo test -p trust-runtime-core vm` (`9/9`).
- `cargo test -p trust-runtime bytecode::validate::resource_limits` (`2/2`).
- `cargo test -p trust-runtime runtime::vm::register_ir` (`102/102`).
- `cargo test -p trust-runtime --test vm_resource_limit_cases` (`1/1`, nine
  trace cases).

The metadata validator, spec-gap closure fixtures, promotion-evidence fixtures,
and malformed-input coverage fixtures pass against this closeout. Broad remote
gates and the resulting `G2` promotion are recorded separately so the causal
evidence chain remains reviewable.
