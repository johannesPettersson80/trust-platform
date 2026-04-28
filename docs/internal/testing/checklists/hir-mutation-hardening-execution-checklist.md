# HIR Mutation Hardening Execution Checklist

Status: Done
Owner: HIR team
Scope: close audit F2 silent-bug exposure in high-risk HIR semantic code.

Audit evidence showed a focused HIR mutation run with 48 mutants tested, 46 missed, and 2 caught. This checklist is the primary "0 silent bugs" board for the mapped HIR risks.

## Target Files

- [x] `HIRMUT-TARGET-01` `crates/trust-hir/src/db/symbol_import.rs`.
- [x] `HIRMUT-TARGET-02` `crates/trust-hir/src/type_check/const_eval.rs`.
- [x] `HIRMUT-TARGET-03` `crates/trust-hir/src/db/queries/collector/variables.rs`.
- [x] `HIRMUT-TARGET-04` Related tests under `crates/trust-hir/tests/`.

## Stop Rules

- [x] `HIRMUT-STOP-01` Do not accept a test that only checks "some diagnostic exists" when the behavior requires a specific diagnostic code/message/location.
- [x] `HIRMUT-STOP-02` Do not mark a surviving mutant acceptable without written equivalent-mutant rationale.
- [x] `HIRMUT-STOP-03` Do not collapse distinct const-eval failures into one generic assertion.
- [x] `HIRMUT-STOP-04` Do not change HIR/runtime boundary direction to make a test easier.

## Phase 1 - Baseline Mutation Reproduction

- [x] `HIRMUT-P1-001` Record exact `cargo mutants` command for `symbol_import.rs`.
- [x] `HIRMUT-P1-002` Record exact `cargo mutants` command for `type_check/const_eval.rs`.
- [x] `HIRMUT-P1-003` Record exact `cargo mutants` command for `collector/variables.rs`.
- [x] `HIRMUT-P1-004` Store baseline survivor list as an artifact.
- [x] `HIRMUT-P1-005` Classify survivors by behavior area before writing tests.

## Phase 2 - Cross-Project Import Matrix

- [x] `HIRMUT-P2-001` Test cross-project import of scalar aliases.
- [x] `HIRMUT-P2-002` Test cross-project import of array types.
- [x] `HIRMUT-P2-003` Test cross-project import of struct types.
- [x] `HIRMUT-P2-004` Test cross-project import of union types.
- [x] `HIRMUT-P2-005` Test nested alias chains across project boundaries.
- [x] `HIRMUT-P2-006` Test default initializer ID translation for struct fields.
- [x] `HIRMUT-P2-007` Test default initializer ID translation for union variants.
- [x] `HIRMUT-P2-008` Test source/target `TypeId` collision cannot reuse wrong type.
- [x] `HIRMUT-P2-009` Test cyclic import guard returns safe unknown/error behavior instead of recursing forever.

## Phase 3 - Const Eval Matrix

- [x] `HIRMUT-P3-001` Test integer literal evaluation.
- [x] `HIRMUT-P3-002` Test typed enum literal evaluation.
- [x] `HIRMUT-P3-003` Test name reference to CONST in same scope.
- [x] `HIRMUT-P3-004` Test name reference to CONST through scope chain.
- [x] `HIRMUT-P3-005` Test undefined name reports the intended error path.
- [x] `HIRMUT-P3-006` Test paren expression preserves value.
- [x] `HIRMUT-P3-007` Test unary plus and unary minus.
- [x] `HIRMUT-P3-008` Test unary minus overflow.
- [x] `HIRMUT-P3-009` Test addition/subtraction/multiplication overflow.
- [x] `HIRMUT-P3-010` Test divide-by-zero.
- [x] `HIRMUT-P3-011` Test modulo-by-zero.
- [x] `HIRMUT-P3-012` Test exponent negative exponent.
- [x] `HIRMUT-P3-013` Test exponent overflow.
- [x] `HIRMUT-P3-014` Test cyclic CONST dependency emits cyclic dependency diagnostic.
- [x] `HIRMUT-P3-015` Test error variants are not collapsed into generic `None` in diagnostics that need specificity.

## Phase 4 - Aggregate Initializer Validation Matrix

- [x] `HIRMUT-P4-001` Test valid struct aggregate by field name.
- [x] `HIRMUT-P4-002` Test field-order independence.
- [x] `HIRMUT-P4-003` Test unknown field diagnostic code/location.
- [x] `HIRMUT-P4-004` Test duplicate field diagnostic code/location.
- [x] `HIRMUT-P4-005` Test nested aggregate validation.
- [x] `HIRMUT-P4-006` Test union variant valid path.
- [x] `HIRMUT-P4-007` Test invalid union variant diagnostic.
- [x] `HIRMUT-P4-008` Test array aggregate/repetition path if supported.
- [x] `HIRMUT-P4-009` Test reference default legality.
- [x] `HIRMUT-P4-010` Test function-block public member override legality.
- [x] `HIRMUT-P4-011` Test VAR_IN_OUT/private/temp/external member rejection.
- [x] `HIRMUT-P4-012` Test class aggregate `T(...)` rejection remains E202 with locked wording.
- [x] `HIRMUT-P4-013` Test unknown target type defers to the existing unknown-type diagnostic without cascaded aggregate errors.

## Phase 5 - Mutation Gate

- [x] `HIRMUT-P5-001` Rerun focused mutants for all three target files.
- [x] `HIRMUT-P5-002` Reduce unexplained survivors to zero.
- [x] `HIRMUT-P5-003` Document any equivalent mutants with source-level rationale.
- [x] `HIRMUT-P5-004` Add CI/scheduled command or explicit manual gate for the focused mutation shard.

## Exit Criteria

- [x] `HIRMUT-EXIT-01` Focused tests pass.
- [x] `HIRMUT-EXIT-02` Focused mutation gate has zero unexplained survivors.
- [x] `HIRMUT-EXIT-03` Diagnostics assert code, wording where locked, and location where meaningful.
- [x] `HIRMUT-EXIT-04` No new HIR/runtime dependency violation is introduced.
