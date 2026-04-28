# Issue #51 - Struct aggregate initializers and struct field defaults

## Status

This is a pre-implementation report for GitHub issue #51:
https://github.com/johannesPettersson80/trust-platform/issues/51

Work should happen in the clean worktree and branch below, not in the dirty OSCAT/OOP example checkout:

- Worktree: `/home/johannes/projects/trust-platform-issue-51`
- Branch: `fix/issue-51-struct-initializers`
- Base: `origin/main` at `dd8c0f276b8f4ec865633681c725b101fccf4931`
- Main checkout status: intentionally untouched because Claude's OSCAT example/test work is in progress there.

No compiler implementation has been started in this branch. The purpose of this document is to make the diagnosis, risks, and intended fix path reviewable before patching.

## Issue Summary

The report is valid. `STRUCT` aggregate initializers currently fail in at least three different ways:

1. `s : T_Step := (cyl := 2, ext := TRUE);` is rejected by the parser with a cascading recovery error.
2. `s : T_Step := T_Step(cyl := 2, ext := TRUE);` parses as a call, but HIR/type checking treats `T_Step` as a non-callable symbol or produces a generic type mismatch.
3. `TYPE`-level struct field defaults such as `cyl : INT := 2;` parse without diagnostics, but runtime default construction silently uses `0`/`FALSE` instead.

The first two are visible compile failures. The third is more dangerous because it is accepted source code that can produce wrong runtime state without an error.

## Reproduction Matrix

The issue body includes standalone repros. I reproduced the core behavior during triage with `trust-runtime` against local scratch projects:

| Case | Command | Observed result |
| --- | --- | --- |
| Named aggregate in `VAR` | `target/debug/trust-runtime build --project /tmp/trust-issue-51-repro/named` | Parser cascade starting at the initializer `(`. |
| IEC-style `TypeName(field := value)` | `target/debug/trust-runtime build --project /tmp/trust-issue-51-repro/iec-typename` | `PROGRAM init error: type mismatch`. |
| Array of structs with aggregate elements | `target/debug/trust-runtime build --project /tmp/trust-issue-51-repro/array-struct` | Parser cascade across the array elements. |
| Struct field defaults in `TYPE` | `target/debug/trust-runtime test --project /tmp/trust-issue-51-repro/defaults --output human` | Test fails because expected `s.cyl = 2`, actual value is `0`. |

These cover parser, HIR/typecheck, runtime lowering, and default-value construction. The field-default case is the one that can create silent wrong behavior.

## Specification Evidence

The repo specs already describe the expected feature:

- `docs/specs/03-variables.md:29-38` lists structure initialization as `S: MyStruct := (field1 := 1, field2 := 2);` and FB instance initialization as `Timer: TON := (PT := T#1s);`.
- `docs/specs/02-data-types.md:180-197` shows `STRUCT` field defaults and a variable aggregate initializer: `Config: AnalogChannel := (Range := Bipolar, MinScale := 0);`.
- `docs/specs/10-runtime-semantics.md:274-292` says a struct default initializes each field to its type default. That conflicts with `docs/specs/02-data-types.md` when a `STRUCT` declaration gives a field-specific default. The runtime spec needs to clarify that field-specific defaults override type defaults.

The GitHub issue also cites IEC 61131-3:2013 section B.1.4.3 (`structured_var_init ::= derived_type_name ( struct_initializer )`) and section 6.4.2 for initial values of derived data types. Before implementation we should verify the exact IEC wording locally if the internal standard PDF/text is present.

## Architecture Evidence

The relevant pipeline is not isolated to one parser bug:

- `docs/diagrams/syntax/syntax-pipeline.puml:87-94` identifies `declarations.rs` as VAR/type parsing and `expressions.rs` as the Pratt expression parser.
- `docs/diagrams/hir/hir-semantics.puml:73-99` shows symbol collection and type resolution happen before validation/constants.
- `docs/diagrams/hir/hir-semantics.puml:288-314` shows `Type::Struct { name, fields }` as a type-registry value.
- `docs/diagrams/hir/hir-semantics.puml:331-349` shows calls are checked under `CallChecker`.
- `docs/diagrams/architecture/system-architecture.puml:276-305` shows the execution path: source -> HIR -> harness -> runtime.
- `docs/diagrams/architecture/runtime-execution.puml:443-460` says `StructValue::new` is the validation boundary for field names, declaration order, missing fields, extra fields, and wrong types.

Code inspection matches those boundaries:

- `crates/trust-syntax/src/parser/grammar/declarations.rs:366-370` parses a variable initializer as a plain expression.
- `crates/trust-syntax/src/parser/grammar/expressions.rs:230-239` parses a leading `(` as `ParenExpr`, so `(field := value)` has no struct-initializer route.
- `crates/trust-hir/src/type_check/calls.rs:52-101` resolves `NameRef(...)` through callable targets. Struct type names do not become struct constructors there.
- `crates/trust-hir/src/types/defs.rs:210-219` defines `StructField` with `name`, `type_id`, and `address`, but no field default.
- `crates/trust-runtime/src/harness/lower/expr/lowering.rs:147-150` lowers calls and array initializers, but rejects `InitializerList` with `initializer lists are not supported yet`.
- `crates/trust-runtime/src/harness/compiler/types.rs:201-220` parses struct field declarations and explicitly discards `_initializer`.
- `crates/trust-runtime/src/value/defaults.rs:89-98` constructs struct defaults from each field's type default only.

## Root Cause

This is a cross-layer feature gap:

1. The parser does not support a struct initializer expression after `:=`; it treats the opening `(` as a parenthesized expression.
2. The HIR call checker does not reinterpret `TypeName(...)` as a struct-constructor expression when `TypeName` resolves to a `STRUCT` type.
3. Runtime lowering has a syntax kind for initializer lists but currently rejects it.
4. Struct field default initializers are parsed, then discarded while lowering type definitions.
5. The HIR/runtime type model does not retain a default initializer on `StructField`.
6. There is no diagnostic that says "struct field defaults are unsupported", so accepted syntax silently falls back to type defaults.

The third and fourth points explain why this is not just a syntax bug.

## Risk Assessment

This is not a general architecture collapse, but it is an architectural gap across parser, HIR, runtime lowering, and value defaults.

The highest risk is silent behavior from accepted `STRUCT` field defaults. Any code using defaults inside a derived `STRUCT` type can run with zero/false/empty values instead of declared values. That can affect globals, locals, arrays of structs, nested structs, FB members, and generated configuration tables.

The visible aggregate-initializer failures are easier to detect because they fail at parse/typecheck time. The silent default issue was not detected because the parser accepts the syntax, lower_struct_def discards the initializer, and default_value_for_type_id has no access to field-specific defaults.

The blast radius appears bounded to derived struct initialization/defaulting and any related FB-instance initializer grammar that shares the same `(field := value)` surface. I do not see evidence that arbitrary scalar, array, enum, or call semantics are silently corrupted by this issue.

## Why Existing Tests Missed It

Likely gaps:

- Parser tests cover scalar and plain array initializers but not struct aggregate initializers in variable declarations.
- Runtime tests cover struct field assignment after declaration, but not declaration-time struct aggregate construction.
- Tests do not assert `TYPE`-level `STRUCT` field defaults at runtime.
- There is no negative test requiring a diagnostic when accepted syntax cannot be preserved into runtime semantics.
- Existing array aggregate work fixed scalar arrays but did not add arrays whose element type is a struct.

## Proposed Fix Plan

### Phase 0 - Clarify Scope

Before coding:

- Verify IEC syntax for named struct initializers, positional struct initializers, and FB instance initialization against the local IEC source if available.
- Treat named field initialization as required because repo specs already document it.
- Treat `TypeName(field := value)` as likely required because the issue cites IEC B.1.4.3 and the syntax is already partly accepted.
- Decide whether positional `(2, TRUE)` is IEC-required, vendor extension, or deferred with a clear diagnostic.
- Decide whether FB instance initialization `Timer: TON := (PT := T#1s);` shares the same implementation route or needs its own path.

### Phase 1 - Add Red Tests First

Add tests before implementation:

- Parser: named struct init in `VAR`, nested struct init, array-of-struct init, `VAR_GLOBAL`, and `TypeName(field := value)`.
- HIR/typecheck: struct constructor resolves to the declared struct type; unknown field, duplicate field, missing required field, and wrong field type produce diagnostics.
- Runtime: named aggregate materializes expected field values.
- Runtime: explicit aggregate value overrides field defaults.
- Runtime: `TYPE`-level struct field defaults apply to locals and globals.
- Runtime: arrays of structs preserve element values and defaults.
- Runtime: nested struct defaults and explicit nested initializers work.
- Negative: if positional struct initialization is deferred, it must produce a direct diagnostic and must not misparse.
- Regression: no accepted field default can be silently discarded.

If IDE diagnostics surface parser/HIR errors, add or update focused LSP/IDE tests after the core compiler tests are stable.

### Phase 2 - Implement by Layer

Implement in the same order as the execution pipeline:

1. Parser/syntax:
   - Add or fully wire a struct initializer expression/list representation.
   - Support `(field := value, ...)` after `:=`.
   - Ensure the parser does not recover into bogus VAR declarations on struct initializer failure.
   - Preserve `TypeName(field := value)` as a call-like syntax until HIR can classify it.

2. HIR/type checking:
   - Add an expected-type path for initializer lists so `(field := value)` can be typed when the declared variable type is known.
   - Recognize `TypeName(...)` where the callee resolves to `Type::Struct`.
   - Validate field names case-insensitively using declaration order.
   - Produce diagnostics for unknown, duplicate, missing required, and type-incompatible fields.
   - Preserve field default initializer expressions in the type model or an adjacent table.

3. Runtime lowering:
   - Lower struct initializer syntax to a struct value using the declared/expected struct type.
   - Reuse `StructValue::new` or the same validation contract described in the runtime diagram.
   - Fill omitted fields from struct field defaults first, then type defaults.
   - Make array initializer element lowering use the element type context so arrays of structs work.

4. Struct field defaults:
   - Stop discarding `_initializer` in `lower_struct_def`.
   - Extend the model that crosses HIR/runtime so each struct field can carry an optional default initializer.
   - Update `default_value_for_type_id` or an equivalent runtime default constructor to apply field defaults recursively.
   - If some initializer expressions cannot be evaluated safely at default-construction time, emit a diagnostic instead of silently dropping them.

### Phase 3 - Docs and Diagrams

Update docs and diagrams if implementation changes ownership, data flow, or type contracts:

- Resolve the struct default wording conflict in `docs/specs/10-runtime-semantics.md`.
- Update `docs/IEC_DECISIONS.md` if positional initializers or `TypeName(...)` need a repo-specific decision.
- Update `docs/IEC_DEVIATIONS.md` if any accepted vendor extension or deferred IEC behavior remains.
- Update `docs/diagrams/hir/hir-semantics.puml` if `StructField`, initializer nodes, or constructor typing are changed.
- Update `docs/diagrams/architecture/runtime-execution.puml` if struct default construction or `StructValue` contracts change.
- Run `scripts/render_diagrams.sh`.
- Run `python scripts/check_diagram_drift.py`.
- Update `docs/internal/testing/checklists/architecture-improvements.md` if architecture checklist coverage is expected for this change.

### Phase 4 - Release Hygiene and Validation

This is a user-visible compiler/runtime behavior fix, so release hygiene applies:

- Update `CHANGELOG.md` under `## [Unreleased]`.
- Bump `[workspace.package].version` in `Cargo.toml` unless explicitly told not to.
- Sync VS Code package versions if the workspace version changes.
- Run focused parser/HIR/runtime tests first.
- Run runtime vertical checks if runtime behavior changes:
  - `cargo test -p trust-runtime --test api_smoke`
  - `cargo test -p trust-runtime --test debug_control`
  - `cargo test -p trust-runtime --test complete_program`
  - `cargo test -p trust-runtime --test runtime_reliability`
- Before declaring completion, run:
  - `just fmt`
  - `just clippy`
  - `just test-all`
- If version is bumped and landed on `main`, complete the tag/release flow and close GitHub issue #51 with a release-linked comment.

## Recommended Branch Strategy

Use the existing clean branch/worktree:

- `/home/johannes/projects/trust-platform-issue-51`
- `fix/issue-51-struct-initializers`

Do not implement in `/home/johannes/projects/trust-platform` while Claude's OSCAT example work is dirty there. This avoids mixing the GitHub issue fix with generated example/test edits and makes review/revert/release accounting much safer.

## Open Questions for Review

1. Is positional struct initialization `(2, TRUE)` required by IEC for this project, or should it be rejected with a targeted diagnostic?
2. Does FB instance initialization `Timer: TON := (PT := T#1s);` need to share the same initializer-list implementation?
3. Should `TypeName(field := value)` become the canonical internal representation, with `(field := value)` only allowed when an expected struct type exists?
4. What expression subset is valid for `STRUCT` field defaults during default construction?
5. Where should field defaults live: directly on HIR `StructField`, in runtime type metadata, or in a separate initializer/default table?
6. Are anonymous inline `STRUCT` aggregate values in scope, or only named derived `STRUCT` types?

