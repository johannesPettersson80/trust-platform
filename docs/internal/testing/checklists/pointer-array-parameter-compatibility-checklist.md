# Pointer & Array Parameter Compatibility Checklist

Date: 2026-04-16

Purpose: implement the locked pointer and `ARRAY[*]` compatibility work in
strict test-first phases, then remove the current OSCAT workaround shapes.

Locked decisions:
- [x] `D1` Allow writes through `VAR_INPUT` pointer dereferences by fixing the
  HIR lvalue assignability walk only.
- [x] `D2` Implement bound-agnostic compatibility only through `ARRAY[*] OF T`.
  No implicit widening between concrete array bounds.
- [x] `D3` Reject `CONSTANT` semantics on parameter blocks. Warn instead and
  keep runtime/codegen behavior unchanged.
- [x] `D4` Add an explicit `(Pointer, Pointer)` compatibility arm and a
  pointer-target diagnostic.
- [x] `D5` Keep the pointer model typed, non-arithmetic, and lifetime-unchecked.

## Phase 0 Baseline

- [x] `P0.1` `cargo build -p trust-hir -p trust-runtime -p trust-syntax`
- [x] `P0.2` `cargo test -p trust-hir --no-run`
- [ ] `P0.3` `cargo test -p trust-runtime --no-run`

## Phase 1 Tests: `VAR_INPUT` Pointer Write-Through

- [x] `P1.1` Add HIR regression module
  `crates/trust-hir/tests/semantic_type_checking/pointer_param_write_through.rs`
- [x] `P1.2` Pin rejection of assigning to the pointer parameter slot itself
- [x] `P1.3` Pin acceptance of `PT^ := ...`
- [x] `P1.4` Pin acceptance of `PT^[i] := ...`
- [x] `P1.5` Pin acceptance of `PT^.F := ...`
- [x] `P1.6` Pin rejection of `FbIn.Field := ...` on `VAR_INPUT` FB instances
- [x] `P1.7` Pin acceptance of `PT^[i].F := ...`
- [x] `P1.8` Add eval/runtime regression in
  `crates/trust-runtime/src/eval/tests/pou_fb.rs`
- [ ] `P1.9` Commit failing tests:
  `test: pin VAR_INPUT pointer write-through semantics (failing)`

## Phase 2 Tests: `ARRAY[*]` Wildcard

- [ ] `P2.1` Add parser coverage for `ARRAY[*]` in parameter and pointer forms
- [ ] `P2.2` Pin rejection of `ARRAY[*]` outside parameter / `VAR_IN_OUT`
  positions
- [ ] `P2.3` Pin rejection of multi-dimensional wildcard forms in this first cut
- [ ] `P2.4` Add HIR wildcard compatibility regression module
- [ ] `P2.5` Pin wildcard acceptance for `VAR_IN_OUT ARRAY[*]`
- [ ] `P2.6` Pin element-type mismatch rejection
- [ ] `P2.7` Pin concrete-bound mismatch rejection
- [ ] `P2.8` Pin pointer-to-wildcard acceptance for `ADR(array)`
- [ ] `P2.9` Pin pointer-to-concrete mismatch rejection
- [ ] `P2.10` Pin pointer-target-specific mismatch diagnostic
- [ ] `P2.11` Pin rejection of wildcard array return types
- [ ] `P2.12` Add eval/runtime wildcard write-through regressions
- [ ] `P2.13` Commit failing tests:
  `test: pin ARRAY[*] wildcard semantics (failing)`

## Phase 3 Tests: Parameter `CONSTANT` Warning

- [ ] `P3.1` Add HIR regression module
  `crates/trust-hir/tests/semantic_type_checking/parameter_constant_qualifier.rs`
- [ ] `P3.2` Pin warning on `VAR_INPUT CONSTANT`
- [ ] `P3.3` Pin warning on `VAR_OUTPUT CONSTANT`
- [ ] `P3.4` Pin warning on `VAR_IN_OUT CONSTANT`
- [ ] `P3.5` Pin no-warning regression for `VAR_GLOBAL CONSTANT`
- [ ] `P3.6` Pin current local `VAR CONSTANT` behavior unchanged
- [ ] `P3.7` Commit failing tests:
  `test: pin parameter CONSTANT qualifier warning (failing)`

## Phase 4 Implementation: Pointer Deref Assignability

- [ ] `P4.1` Change `assignment_target_symbol` to stop at `DerefExpr`
- [ ] `P4.2` Re-read the `None` short-circuit path before editing
- [ ] `P4.3` Keep `IndexExpr` and `FieldExpr` walking unchanged
- [ ] `P4.4` Verify all Phase 1 HIR tests pass
- [ ] `P4.5` Verify the Phase 1 eval/runtime test passes
- [ ] `P4.6` Commit fix:
  `fix(hir): allow writes through VAR_INPUT pointer deref`

## Phase 5 Implementation: `ARRAY[*]`

- [ ] `P5.1` Implement parser support for `ARRAY[*]`
- [ ] `P5.2` Enforce wildcard position/scope restrictions
- [ ] `P5.3` Enforce single-dimension wildcard restriction
- [ ] `P5.4` Add a shared wildcard helper to the HIR type layer
- [ ] `P5.5` Route array compatibility through the wildcard helper
- [ ] `P5.6` Add explicit `(Pointer, Pointer)` compatibility arm
- [ ] `P5.7` Add pointer-target-specific mismatch diagnostic
- [ ] `P5.8` Add the pointer-model comment near `infer_addr_expr`
- [ ] `P5.9` Verify all Phase 2 tests pass
- [ ] `P5.10` Run broader HIR/runtime suites for array/pointer regressions
- [ ] `P5.11` Commit feature:
  `feat(hir): support ARRAY[*] wildcard and explicit pointer-target compat`

## Phase 6 Implementation: Parameter `CONSTANT` Warning

- [ ] `P6.1` Add warning emission during parameter collection/validation
- [ ] `P6.2` Keep symbol/runtime semantics unchanged
- [ ] `P6.3` Add or reuse a warning diagnostic code
- [ ] `P6.4` Verify all Phase 3 tests pass
- [ ] `P6.5` Commit feature:
  `feat(hir): warn on ineffective CONSTANT qualifier in parameter blocks`

## Phase 7 Cleanup: OSCAT Consumers

- [ ] `P7.1` Rewrite `CRC_GEN` to `POINTER TO ARRAY[*] OF BYTE`
- [ ] `P7.2` Rewrite `_BUFFER_*` / `BUFFER_*` to `VAR_IN_OUT ARRAY[*] OF BYTE`
- [ ] `P7.3` Remove giant ceremony arrays from OSCAT core fixtures
- [ ] `P7.4` Re-run OSCAT integration fixtures and keep behavior unchanged
- [ ] `P7.5` Commit refactor:
  `refactor(oscat_basic): use ARRAY[*] wildcards in buffer and CRC helpers`

## Phase 8 Final Validation

- [ ] `P8.1` `cargo build -p trust-hir -p trust-runtime -p trust-syntax`
- [ ] `P8.2` `cargo test -p trust-hir`
- [ ] `P8.3` `cargo test -p trust-runtime`
- [ ] `P8.4` `cargo test -p trust-syntax`
- [ ] `P8.5` `just fmt`
- [ ] `P8.6` `just clippy`
- [ ] `P8.7` `just test-all`

## Documentation Follow-Up

- [ ] `DOC.1` Update `CHANGELOG.md` under `## [Unreleased]`
- [ ] `DOC.2` Update standards/runtime docs for the shipped pointer and
  `ARRAY[*]` behavior
- [ ] `DOC.3` Keep the checklist in sync with each completed phase
