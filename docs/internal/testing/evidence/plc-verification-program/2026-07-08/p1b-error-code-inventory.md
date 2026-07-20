# Phase 1B Error-Code Inventory

Date: 2026-07-09
Scope: `VERIF-P1B-003A`
Branch: `plc-verification-program`
Commit base: `1d9b3ec6a` with local verification metadata changes

## Commands

- `rg -n 'pub enum (BytecodeError|RuntimeError|VmTrap)|SubrangeViolation|InvalidBytecode|InvalidSection|normalize_vm_value_for_type|normalize_vm_value_for_ref|truncate_string_elements' crates/trust-runtime crates/trust-runtime-core`
- `nl -ba crates/trust-runtime-core/src/bytecode/mod.rs | sed -n '30,95p'`
- `nl -ba crates/trust-runtime-core/src/error.rs | sed -n '1,140p'`
- `nl -ba crates/trust-runtime-core/src/vm/errors.rs | sed -n '1,120p'`
- `nl -ba crates/trust-runtime/src/runtime/vm/type_policy.rs | sed -n '1,120p'`
- `nl -ba crates/trust-runtime/src/runtime/vm/dispatch_refs.rs | sed -n '110,155p'`
- `nl -ba crates/trust-runtime/src/runtime/vm/call/bindings.rs | sed -n '520,580p;680,725p;750,780p'`
- `nl -ba crates/trust-runtime/src/bytecode/validate/stack_shape.rs | sed -n '105,135p;145,205p;270,305p'`
- `nl -ba crates/trust-runtime/src/bytecode/validate/reference_escape.rs | sed -n '90,165p'`
- `nl -ba crates/trust-runtime/src/bytecode/validate/pou_and_instr.rs | sed -n '105,220p'`
- `nl -ba crates/trust-runtime/src/bytecode/validate/const_compat.rs | sed -n '132,155p'`
- `nl -ba crates/trust-runtime/src/bytecode/validate/param_direction.rs | sed -n '127,140p'`

## Findings

1. `BytecodeError` is a typed Rust enum, but several semantic validator failures still collapse to `InvalidSection(SmolStr)`.
   Stable examples exist for container-level failures: `InvalidMagic`, `UnsupportedVersion`, `InvalidHeader`, `InvalidChecksum`, `InvalidSectionTable`, `SectionOutOfBounds`, `SectionOverlap`, `SectionAlignment`, `UnexpectedEof`, `MissingSection`, `InvalidOpcode`, `InvalidJumpTarget`, `InvalidPouId`, and `InvalidIndex`.
   Semantic examples still use string payloads: stack-shape errors, local-reference escape errors, unsupported runtime opcode diagnostics, constant/store compatibility, and parameter-direction diagnostics.

2. `RuntimeError` is a typed Rust enum and now includes value-policy variants useful to the pilot, including `SubrangeViolation`, `TypeMismatch`, `Overflow`, `IndexOutOfBounds`, `InvalidBytecode`, and `InvalidBytecodeMetadata`.
   These variants are stable enough for low-level Rust assertions, but no committed contract maps them to stable case-level error codes for generated verification cases.

3. `VmTrap` is a typed internal VM trap enum, but `VmTrap::into_runtime_error` converts many VM failures into `RuntimeError::InvalidBytecode(SmolStr)` with formatted strings.
   That means generated tests must not match those display strings as stable public identifiers.

4. The store/copy-in surfaces are partially centralized:
   `store_ref` normalizes values through `normalize_vm_value_for_ref`, FB parameter binding and native call binding call `normalize_vm_value_for_type`, subranges can emit `RuntimeError::SubrangeViolation`, and bounded strings are truncated by primitive type policy.
   This is useful implementation evidence, but it does not close the written error-code contract gap.

## Decision

`SPEC_GAP_VM_ERROR_MODEL_001` remains open and still blocks `VERIF-P1B-004` behavior rows that would pin error codes such as `SubrangeOutOfBounds`.
The next phase may reference existing typed variants as candidate implementation surfaces, but expected generated-case outcomes must cite a written VM error model or remain `spec_gap`.

No product code was changed for this inventory.
