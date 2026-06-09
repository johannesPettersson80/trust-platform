# IEC Validation Functions and Partial Access Checklist

Scope: land IEC 61131-3 Ed.3 Table 39 validation functions in HIR/IDE/docs and
finish Table 17 partial-access support for structured `VAR_IN_OUT` references.

## Source References

- IEC 61131-3 Ed.3 6.6.2.5.15 / Table 39: `IS_VALID`, `IS_VALID_BCD`.
- IEC 61131-3 Ed.3 6.6.1.3 / Table 17: partial access to `ANY_BIT` variables.
- IEC 61131-3 Ed.3 6.4.4.6.1: structured elements use dotted access.
- IEC 61131-3 Ed.3 6.6.1.4.1: properly mapped `VAR_IN_OUT` variables are read/write.

## Implementation

- [x] `VALPART-001` Create this implementation checklist before code changes.
- [x] `VALPART-002` Add HIR type checking for `IS_VALID(IN)` with `REAL`/`LREAL`
  input and `BOOL` result.
- [x] `VALPART-003` Add HIR type checking for `IS_VALID_BCD(IN)` with strict
  `BYTE`/`WORD`/`DWORD`/`LWORD` input and `BOOL` result; reject `BOOL`.
- [x] `VALPART-004` Add IDE standard-function signature/help/completion docs for
  `IS_VALID` and `IS_VALID_BCD`.
- [x] `VALPART-005` Extend runtime bytecode codegen for partial read/write on
  structured `VAR_IN_OUT` references.
- [x] `VALPART-006` Update standard-function specs, coverage docs, and partial
  access specs to match the shipped behavior and IEC table numbers.
- [x] `VALPART-007` Update changelog and synchronize release version metadata.

## Tests

- [x] `VALPART-T001` HIR tests cover valid `IS_VALID` and invalid non-real
  arguments.
- [x] `VALPART-T002` HIR tests cover valid `IS_VALID_BCD` bit-string arguments
  and reject `BOOL`/non-bit arguments.
- [x] `VALPART-T003` Runtime stdlib tests cover finite/NaN/Inf real validation
  and valid/invalid BCD values.
- [x] `VALPART-T004` Runtime tests cover partial read/write through a structured
  `VAR_IN_OUT` reference.
- [x] `VALPART-T005` IDE tests cover validation-function standard docs/signature
  surface.

## Acceptance

- [x] `VALPART-A001` SOLID: changes stay in the existing HIR standard-function,
  IDE standard-library, and runtime bytecode ownership boundaries.
- [x] `VALPART-A002` KISS: no new abstraction unless it removes real duplication
  in the existing standard-function checker pattern.
- [x] `VALPART-A003` DRY: validation-function type rules are centralized in one
  HIR module and docs/IDE tables do not duplicate inconsistent IEC table numbers.
- [x] `VALPART-A004` Remote focused checks pass on `trust-builder`.
- [x] `VALPART-A005` Final `trust-builder` gates pass: `just fmt`,
  `just clippy`, and `just test-all`.
