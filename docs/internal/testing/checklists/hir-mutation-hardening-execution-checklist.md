# HIR Mutation Hardening Execution Checklist

Status: Planned
Owner: HIR team
Scope: close audit F2 silent-bug exposure in high-risk HIR semantic code.

Audit evidence showed a focused HIR mutation run with 48 mutants tested, 46 missed, and 2 caught. This checklist is the primary "0 silent bugs" board for the mapped HIR risks.

## Target Files

- [ ] `HIRMUT-TARGET-01` `crates/trust-hir/src/db/symbol_import.rs`.
- [ ] `HIRMUT-TARGET-02` `crates/trust-hir/src/type_check/const_eval.rs`.
- [ ] `HIRMUT-TARGET-03` `crates/trust-hir/src/db/queries/collector/variables.rs`.
- [ ] `HIRMUT-TARGET-04` Related tests under `crates/trust-hir/tests/`.

## Stop Rules

- [ ] `HIRMUT-STOP-01` Do not accept a test that only checks "some diagnostic exists" when the behavior requires a specific diagnostic code/message/location.
- [ ] `HIRMUT-STOP-02` Do not mark a surviving mutant acceptable without written equivalent-mutant rationale.
- [ ] `HIRMUT-STOP-03` Do not collapse distinct const-eval failures into one generic assertion.
- [ ] `HIRMUT-STOP-04` Do not change HIR/runtime boundary direction to make a test easier.

## Phase 1 - Baseline Mutation Reproduction

- [ ] `HIRMUT-P1-001` Record exact `cargo mutants` command for `symbol_import.rs`.
- [ ] `HIRMUT-P1-002` Record exact `cargo mutants` command for `type_check/const_eval.rs`.
- [ ] `HIRMUT-P1-003` Record exact `cargo mutants` command for `collector/variables.rs`.
- [ ] `HIRMUT-P1-004` Store baseline survivor list as an artifact.
- [ ] `HIRMUT-P1-005` Classify survivors by behavior area before writing tests.

## Phase 2 - Cross-Project Import Matrix

- [ ] `HIRMUT-P2-001` Test cross-project import of scalar aliases.
- [ ] `HIRMUT-P2-002` Test cross-project import of array types.
- [ ] `HIRMUT-P2-003` Test cross-project import of struct types.
- [ ] `HIRMUT-P2-004` Test cross-project import of union types.
- [ ] `HIRMUT-P2-005` Test nested alias chains across project boundaries.
- [ ] `HIRMUT-P2-006` Test default initializer ID translation for struct fields.
- [ ] `HIRMUT-P2-007` Test default initializer ID translation for union variants.
- [ ] `HIRMUT-P2-008` Test source/target `TypeId` collision cannot reuse wrong type.
- [ ] `HIRMUT-P2-009` Test cyclic import guard returns safe unknown/error behavior instead of recursing forever.

## Phase 3 - Const Eval Matrix

- [ ] `HIRMUT-P3-001` Test integer literal evaluation.
- [ ] `HIRMUT-P3-002` Test typed enum literal evaluation.
- [ ] `HIRMUT-P3-003` Test name reference to CONST in same scope.
- [ ] `HIRMUT-P3-004` Test name reference to CONST through scope chain.
- [ ] `HIRMUT-P3-005` Test undefined name reports the intended error path.
- [ ] `HIRMUT-P3-006` Test paren expression preserves value.
- [ ] `HIRMUT-P3-007` Test unary plus and unary minus.
- [ ] `HIRMUT-P3-008` Test unary minus overflow.
- [ ] `HIRMUT-P3-009` Test addition/subtraction/multiplication overflow.
- [ ] `HIRMUT-P3-010` Test divide-by-zero.
- [ ] `HIRMUT-P3-011` Test modulo-by-zero.
- [ ] `HIRMUT-P3-012` Test exponent negative exponent.
- [ ] `HIRMUT-P3-013` Test exponent overflow.
- [ ] `HIRMUT-P3-014` Test cyclic CONST dependency emits cyclic dependency diagnostic.
- [ ] `HIRMUT-P3-015` Test error variants are not collapsed into generic `None` in diagnostics that need specificity.

## Phase 4 - Aggregate Initializer Validation Matrix

- [ ] `HIRMUT-P4-001` Test valid struct aggregate by field name.
- [ ] `HIRMUT-P4-002` Test field-order independence.
- [ ] `HIRMUT-P4-003` Test unknown field diagnostic code/location.
- [ ] `HIRMUT-P4-004` Test duplicate field diagnostic code/location.
- [ ] `HIRMUT-P4-005` Test nested aggregate validation.
- [ ] `HIRMUT-P4-006` Test union variant valid path.
- [ ] `HIRMUT-P4-007` Test invalid union variant diagnostic.
- [ ] `HIRMUT-P4-008` Test array aggregate/repetition path if supported.
- [ ] `HIRMUT-P4-009` Test reference default legality.
- [ ] `HIRMUT-P4-010` Test function-block public member override legality.
- [ ] `HIRMUT-P4-011` Test VAR_IN_OUT/private/temp/external member rejection.
- [ ] `HIRMUT-P4-012` Test class aggregate `T(...)` rejection remains E202 with locked wording.
- [ ] `HIRMUT-P4-013` Test unknown target type defers to the existing unknown-type diagnostic without cascaded aggregate errors.

## Phase 5 - Mutation Gate

- [ ] `HIRMUT-P5-001` Rerun focused mutants for all three target files.
- [ ] `HIRMUT-P5-002` Reduce unexplained survivors to zero.
- [ ] `HIRMUT-P5-003` Document any equivalent mutants with source-level rationale.
- [ ] `HIRMUT-P5-004` Add CI/scheduled command or explicit manual gate for the focused mutation shard.

## Exit Criteria

- [ ] `HIRMUT-EXIT-01` Focused tests pass.
- [ ] `HIRMUT-EXIT-02` Focused mutation gate has zero unexplained survivors.
- [ ] `HIRMUT-EXIT-03` Diagnostics assert code, wording where locked, and location where meaningful.
- [ ] `HIRMUT-EXIT-04` No new HIR/runtime dependency violation is introduced.
