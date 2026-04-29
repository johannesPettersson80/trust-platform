# Parser Recovery Hardening Execution Checklist

Status: Done - bounded parser recovery helpers, declaration-boundary tests, full-map doctor guardrail, and focused mutation evidence are in place.
Owner: Syntax/parser team
Scope: address audit F3 by replacing fragile ad hoc recovery scanning with bounded helpers and fuzz/property coverage.

## Target Area

- [x] `PARSERREC-TARGET-01` `crates/trust-syntax/src/parser/grammar/declarations.rs::parse_var_initializer`.
- [x] `PARSERREC-TARGET-02` `at_positional_initializer_start`.
- [x] `PARSERREC-TARGET-03` `has_top_level_comma_before_rparen`.
- [x] `PARSERREC-TARGET-04` `parse_positional_initializer_list`.
- [x] `PARSERREC-TARGET-05` any similar declaration-boundary recovery loops.

## Stop Rules

- [x] `PARSERREC-STOP-01` Do not add another unbounded scan loop.
- [x] `PARSERREC-STOP-02` Do not fix one token class while leaving equivalent BOOL/string/name-ref cases untested.
- [x] `PARSERREC-STOP-03` Do not accept parser tests that assert only error count when wording/location is locked.
- [x] `PARSERREC-STOP-04` Do not let malformed initializer recovery consume the next declaration.

## Phase 1 - Baseline Coverage

- [x] `PARSERREC-P1-001` Record current parser tests covering aggregate initializers.
- [x] `PARSERREC-P1-002` Add baseline tests for malformed positional shapes: `(1, 2)`, `(TRUE, FALSE)`, `(MyConst, 5)`, `('a', 'b')`.
- [x] `PARSERREC-P1-003` Add nested malformed aggregate tests.
- [x] `PARSERREC-P1-004` Add declaration-boundary tests with the next declaration after malformed input.
- [x] `PARSERREC-P1-005` Assert diagnostic wording for positional initializers.
- [x] `PARSERREC-P1-006` Assert bounded cascade counts.

## Phase 2 - Bounded Scanner API

- [x] `PARSERREC-P2-001` Introduce a small parser helper for bounded top-level scanning.
- [x] `PARSERREC-P2-002` Parameterize stop tokens: semicolon, END_VAR, END_TYPE, END_STRUCT, END_UNION, END_PROGRAM, END_FUNCTION, END_FUNCTION_BLOCK, END_CLASS, END_CONFIGURATION, EOF.
- [x] `PARSERREC-P2-003` Track nested `()`, `[]` depth.
- [x] `PARSERREC-P2-004` Define max lookahead or explicit declaration-boundary cutoff.
- [x] `PARSERREC-P2-005` Reuse the helper for positional initializer detection.
- [x] `PARSERREC-P2-006` Reuse the helper for positional initializer skipping.
- [x] `PARSERREC-P2-007` Remove duplicated local depth-scanning loops where safe.

## Phase 3 - Fuzz / Property Coverage

- [x] `PARSERREC-P3-001` Add a fuzz target or property-style generator for declaration initializers.
- [x] `PARSERREC-P3-002` Generate nested parens/brackets.
- [x] `PARSERREC-P3-003` Generate comments/trivia inside aggregates.
- [x] `PARSERREC-P3-004` Generate missing commas, missing `:=`, missing closing delimiters, and declaration-boundary truncation.
- [x] `PARSERREC-P3-005` Assert parser termination.
- [x] `PARSERREC-P3-006` Assert recovery does not consume unrelated following declarations.
- [x] `PARSERREC-P3-007` Store minimal reproducer cases from fuzz failures as unit tests.

## Phase 4 - Doctor Rule

- [x] `PARSERREC-P4-001` Add a doctor/source-count rule for positional initializer diagnostic wording.
- [x] `PARSERREC-P4-002` Add a source-count rule to prevent new ad hoc declaration scanner loops without explicit allowlist.
- [x] `PARSERREC-P4-003` Add a parser recovery test command to architecture evidence.

## Exit Criteria

- [x] `PARSERREC-EXIT-01` Focused parser tests pass.
- [x] `PARSERREC-EXIT-02` Fuzz/property smoke passes.
- [x] `PARSERREC-EXIT-03` Syntax mutation slice remains killed or has no unexplained survivors.
- [x] `PARSERREC-EXIT-04` Doctor rule prevents drift back to ad hoc unbounded scanning.

## Evidence

- `cargo test -p trust-syntax positional_initializer_recovery -- --nocapture`
- `cargo test -p trust-syntax initializer_recovery_property -- --nocapture`
- `cargo test -p trust-syntax test_positional_and_empty_aggregate_recovery_is_bounded -- --nocapture`
- `cargo test -p trust-syntax bounded_ -- --nocapture`
- `cargo test -p trust-syntax`
- `cargo test -p xtask`
- `cargo run -p xtask -- architecture-doctor --full-map`: `FULLMAP-PARSERREC` passed with `scan_top_level_ahead`, `recover_top_level_until`, one positional diagnostic definition, and the two parser recovery tests.
- Safe copied-tree mutation slices:
  - `cargo mutants -p trust-syntax --file crates/trust-syntax/src/parser/grammar/declarations.rs --re 'parse_var_initializer|at_positional_initializer_start|parse_positional_initializer_list|parse_initializer_list|parse_initializer_element' --output target/gate-artifacts/parser-recovery-mutants-safe/declarations-rerun --timeout 80 --minimum-test-timeout 20 --jobs 2 --caught`: 14 tested, 14 caught.
  - `cargo mutants -p trust-syntax --file crates/trust-syntax/src/parser/parser.rs --re 'scan_top_level_ahead|recover_top_level_until' --output target/gate-artifacts/parser-recovery-mutants-safe/parser-helpers-final --timeout 80 --minimum-test-timeout 20 --jobs 2 --caught`: 26 tested, 24 caught, 2 unviable, 0 missed.
