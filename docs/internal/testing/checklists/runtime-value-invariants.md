# Runtime Value Invariants Checklist

Status: Done

Scope: close silent wrong-value construction paths for runtime compound values without adding a global value factory, runtime type table, or separate validator phase.

## Architecture Rules

- [x] `RTVALUE-INV-01` Keep enum identity owned by `EnumValue`: registry-backed construction, alias canonicalization, numeric/variant validation, explicit equality, and private fields.
- [x] `RTVALUE-INV-02` Move struct identity and field-shape validation into `StructValue`: private fields, read accessors, canonical construction from `TypeRegistry`, declared field names/order, and declared field value-type checks.
- [x] `RTVALUE-INV-03` Move array shape validation into `ArrayValue`: private fields, read accessors, dimension validation, element-count validation, and declared element value-type checks.
- [x] `RTVALUE-INV-04` Route raw entry points through validating construction where type context is available: defaults, executable lowering, retain apply, protocol decode, bytecode constants, and test helpers.
- [x] `RTVALUE-INV-05` Preserve the failure policy: validation failure never substitutes a fallback value; defaults return `DefaultValueError`, executable lowering returns `CompileError`, retain load/apply fails with diagnostics, protocol decode rejects the message, bytecode load fails, and validating test helpers fail tests.
- [x] `RTVALUE-INV-06` Keep executable-lowering errors local: convert concrete “HIR resolved this but runtime could not lower it” branches to `Err(...)` without adding a global lowering validator phase.

## Failure Policy

- [x] `RTVALUE-FAIL-01` Defaults fail with `DefaultValueError`; they never synthesize fallback compound values after invalid bounds or unsupported types.
- [x] `RTVALUE-FAIL-02` Executable lowering returns a compile/lowering error when type-checked enum or constant lowering cannot produce a runtime value.
- [x] `RTVALUE-FAIL-03` Retain decode rejects structurally invalid payloads and retain apply rejects declared-type mismatches with `RuntimeError::RetainStore`; retained state is never defaulted on corruption.
- [x] `RTVALUE-FAIL-04` Protocol decode rejects structurally invalid array payloads with a structured harness error; messages are not partially applied.
- [x] `RTVALUE-FAIL-05` Bytecode constant decode rejects invalid enum const pools during bytecode load.
- [x] `RTVALUE-FAIL-06` Test helpers use validating constructors unless they intentionally build raw decode payloads for canonicalization tests.

## Contract Tests

- [x] `RTVALUE-TEST-01` Enum initializer/equality regression covers declared variant, qualified/unqualified forms, alias-backed construction, mixed-case type names, and retained legacy enum casing.
- [x] `RTVALUE-TEST-02` Struct constructor tests reject missing/extra/wrongly typed fields and canonicalize alias-backed struct construction to the underlying struct name.
- [x] `RTVALUE-TEST-03` Array constructor tests reject invalid dimensions, wrong element count, and wrongly typed elements; alias-backed arrays compare equal to underlying arrays.
- [x] `RTVALUE-TEST-04` Retain contract tests cover nested enum-in-struct and array-of-struct canonicalization/error paths.

## Gates

- [x] `RTVALUE-GATE-01` `cargo test -p trust-runtime --test bytecode_vm_enum_unqualified`
- [x] `RTVALUE-GATE-02` `cargo test -p trust-runtime --lib enum_value`
- [x] `RTVALUE-GATE-03` focused struct/array value invariant tests
- [x] `RTVALUE-GATE-04` runtime vertical tests: `api_smoke`, `debug_control`, `complete_program`, `runtime_reliability`
- [x] `RTVALUE-GATE-05` `just fmt`
- [x] `RTVALUE-GATE-06` `just clippy`
- [x] `RTVALUE-GATE-07` `just test`
- [x] `RTVALUE-GATE-08` `just test-all`
