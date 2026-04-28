# Parser Recovery Hardening Execution Checklist

Status: Planned
Owner: Syntax/parser team
Scope: address audit F3 by replacing fragile ad hoc recovery scanning with bounded helpers and fuzz/property coverage.

## Target Area

- [ ] `PARSERREC-TARGET-01` `crates/trust-syntax/src/parser/grammar/declarations.rs::parse_var_initializer`.
- [ ] `PARSERREC-TARGET-02` `at_positional_initializer_start`.
- [ ] `PARSERREC-TARGET-03` `has_top_level_comma_before_rparen`.
- [ ] `PARSERREC-TARGET-04` `parse_positional_initializer_list`.
- [ ] `PARSERREC-TARGET-05` any similar declaration-boundary recovery loops.

## Stop Rules

- [ ] `PARSERREC-STOP-01` Do not add another unbounded scan loop.
- [ ] `PARSERREC-STOP-02` Do not fix one token class while leaving equivalent BOOL/string/name-ref cases untested.
- [ ] `PARSERREC-STOP-03` Do not accept parser tests that assert only error count when wording/location is locked.
- [ ] `PARSERREC-STOP-04` Do not let malformed initializer recovery consume the next declaration.

## Phase 1 - Baseline Coverage

- [ ] `PARSERREC-P1-001` Record current parser tests covering aggregate initializers.
- [ ] `PARSERREC-P1-002` Add baseline tests for malformed positional shapes: `(1, 2)`, `(TRUE, FALSE)`, `(MyConst, 5)`, `('a', 'b')`.
- [ ] `PARSERREC-P1-003` Add nested malformed aggregate tests.
- [ ] `PARSERREC-P1-004` Add declaration-boundary tests with the next declaration after malformed input.
- [ ] `PARSERREC-P1-005` Assert diagnostic wording for positional initializers.
- [ ] `PARSERREC-P1-006` Assert bounded cascade counts.

## Phase 2 - Bounded Scanner API

- [ ] `PARSERREC-P2-001` Introduce a small parser helper for bounded top-level scanning.
- [ ] `PARSERREC-P2-002` Parameterize stop tokens: semicolon, END_VAR, END_TYPE, END_STRUCT, END_UNION, END_PROGRAM, END_FUNCTION, END_FUNCTION_BLOCK, END_CLASS, END_CONFIGURATION, EOF.
- [ ] `PARSERREC-P2-003` Track nested `()`, `[]` depth.
- [ ] `PARSERREC-P2-004` Define max lookahead or explicit declaration-boundary cutoff.
- [ ] `PARSERREC-P2-005` Reuse the helper for positional initializer detection.
- [ ] `PARSERREC-P2-006` Reuse the helper for positional initializer skipping.
- [ ] `PARSERREC-P2-007` Remove duplicated local depth-scanning loops where safe.

## Phase 3 - Fuzz / Property Coverage

- [ ] `PARSERREC-P3-001` Add a fuzz target or property-style generator for declaration initializers.
- [ ] `PARSERREC-P3-002` Generate nested parens/brackets.
- [ ] `PARSERREC-P3-003` Generate comments/trivia inside aggregates.
- [ ] `PARSERREC-P3-004` Generate missing commas, missing `:=`, missing closing delimiters, and declaration-boundary truncation.
- [ ] `PARSERREC-P3-005` Assert parser termination.
- [ ] `PARSERREC-P3-006` Assert recovery does not consume unrelated following declarations.
- [ ] `PARSERREC-P3-007` Store minimal reproducer cases from fuzz failures as unit tests.

## Phase 4 - Doctor Rule

- [ ] `PARSERREC-P4-001` Add a doctor/source-count rule for positional initializer diagnostic wording.
- [ ] `PARSERREC-P4-002` Add a source-count rule to prevent new ad hoc declaration scanner loops without explicit allowlist.
- [ ] `PARSERREC-P4-003` Add a parser recovery test command to architecture evidence.

## Exit Criteria

- [ ] `PARSERREC-EXIT-01` Focused parser tests pass.
- [ ] `PARSERREC-EXIT-02` Fuzz/property smoke passes.
- [ ] `PARSERREC-EXIT-03` Syntax mutation slice remains killed or has no unexplained survivors.
- [ ] `PARSERREC-EXIT-04` Doctor rule prevents drift back to ad hoc unbounded scanning.
