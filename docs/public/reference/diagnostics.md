# Diagnostics

Compiler and semantic diagnostic codes with default severity, likely cause,
user-visible symptom, and first fix.

## How to read this table

- `E...` codes are build-blocking errors.
- `W...` codes are warnings by default.
- `I...` codes are hint-level suggestions.
- “Typical first fix” is the first thing to check, not a guarantee that one
  edit will solve the whole issue.

## Quick code lookup

Common queries land here:

- `E001`: unexpected token
- `W003`: unreachable code
- `W004`: missing `ELSE`
- `W005`: implicit conversion

If you are searching by code only, start with this page before digging into the
language specifications.

## Syntax errors

| Code | Default severity | Cause | User-visible symptom | Typical first fix |
| --- | --- | --- | --- | --- |
| `E001` | error | The parser hit a token that does not fit the current grammar rule. | The editor or CLI stops near the current line with an unexpected-token error. | Check the previous line first for a missing `;`, `END_*`, or misplaced keyword. |
| `E002` | error | A required token or closing delimiter is missing. | The parser reports a missing token and usually points at the next recoverable token. | Add the missing delimiter or closing keyword the parser expects. |
| `E003` | error | A block opener such as `IF`, `CASE`, or `FUNCTION_BLOCK` was not closed correctly. | The file reports an unclosed block near the end of the construct or file. | Match the opening construct with the correct closing token such as `END_IF`, `END_CASE`, or `END_FUNCTION_BLOCK`. |

## Name resolution errors

| Code | Default severity | Cause | User-visible symptom | Typical first fix |
| --- | --- | --- | --- | --- |
| `E101` | error | A referenced variable name does not resolve in the current scope. | Reads, writes, or watches on that name fail with an undefined-variable error. | Check spelling, scope, and whether the declaration is in the project or dependency set. |
| `E102` | error | A declared or referenced type name cannot be resolved. | Type annotations fail and downstream declarations may cascade with extra errors. | Verify the type name and that the defining file or package is included. |
| `E103` | error | A call target cannot be resolved as a function or method. | The call site is flagged and the editor cannot supply the expected signature. | Check the callable name and whether the correct namespace or library is available. |
| `E104` | error | The same symbol is declared more than once in the same effective scope. | Both declarations are flagged as duplicates. | Rename one declaration or remove the duplicate definition. |
| `E105` | error | The compiler cannot disambiguate or resolve a symbol reference. | The reference is flagged even though similarly named symbols may exist. | Check namespace qualifiers, imports, and whether you meant a different symbol kind. |
| `E106` | error | The identifier form is not accepted in the current context. | The declaration or reference is rejected before later semantic checks run. | Rename the symbol to a valid IEC/truST-compatible identifier. |

## Type errors

| Code | Default severity | Cause | User-visible symptom | Typical first fix |
| --- | --- | --- | --- | --- |
| `E201` | error | Two types do not match where exact compatibility is required. | Assignments, calls, or expressions fail with a type-mismatch error. | Check the source and target types, then add an explicit conversion if appropriate. |
| `E202` | error | The operator or operation is not valid for the operand type or types. | The expression is highlighted as an invalid operation. | Verify the operator and the actual operand types. |
| `E203` | error | The right-hand side cannot be assigned safely to the left-hand side. | The assignment is rejected even though both sides may be valid expressions. | Convert the value explicitly or change the destination type. |
| `E204` | error | A call passed too few or too many arguments. | The call site is flagged with an argument-count error. | Match the call site to the declared function or function-block signature. |
| `E205` | error | One or more argument types do not match the declared parameter types. | The call shape looks correct but the arguments are flagged individually. | Check parameter types and add explicit conversions where intended. |
| `E206` | error | A function or method control-flow path does not produce the required result value. | The declaration reports a missing-return error even when some paths assign the result. | Ensure every control-flow path assigns the implicit result or returns correctly. |
| `E207` | error | The returned value does not match the declared result type. | The return expression or implicit result assignment is flagged. | Align the returned expression with the declared return type. |

## Semantic errors

| Code | Default severity | Cause | User-visible symptom | Typical first fix |
| --- | --- | --- | --- | --- |
| `E301` | error | The left-hand side is not writable in this context. | The assignment target is rejected as non-writable. | Assign to a variable, field, or indexable storage location instead of a non-lvalue expression. |
| `E302` | error | A constant or otherwise immutable symbol is being written. | The write is flagged as a constant modification. | Remove the write or copy the value into a mutable variable first. |
| `E303` | error | The index expression is not valid for the array shape. | The indexed access is flagged and later element typing may fail. | Check index type, bounds, and whether the base expression is actually an array. |
| `E304` | error | A value or bound exceeds the representable range for its destination type. | The literal, conversion, or bound expression is flagged as out of range. | Reduce the literal or expression to a representable range for the destination type. |
| `E305` | error | Declarations depend on each other in a cycle the compiler cannot resolve. | Multiple declarations are reported together as a cyclic dependency. | Break the cycle by extracting shared types/constants or reordering ownership. |
| `E306` | error | Task metadata is malformed or incomplete. | Runtime/task declarations fail validation before build or execution. | Check task names, intervals, priorities, and required runtime config fields. |
| `E307` | error | A program or config references a task that does not exist. | The task reference is flagged as unknown. | Define the task or update the reference to an existing task name. |

## Warnings

| Code | Default severity | Cause | User-visible symptom | Typical first fix |
| --- | --- | --- | --- | --- |
| `W001` | warning | A declared variable is never read. | The declaration is highlighted as unused. | Remove it or use it intentionally. |
| `W002` | warning | A declared parameter is never read. | The parameter list shows an unused-parameter warning. | Remove it, use it, or document why it is intentionally reserved. |
| `W003` | warning | The compiler proved that a code path can never execute. | The dead block is marked unreachable. | Remove the dead block or simplify the surrounding control flow. |
| `W004` | warning | A conditional branch has no fallback path. | The conditional is flagged as missing `ELSE`. | Add an `ELSE` if the branch should be exhaustive, or override severity if the omission is deliberate. |
| `W005` | warning | The compiler inserted a conversion you did not write explicitly. | The expression is valid but marked with an implicit-conversion warning. | Replace it with an explicit conversion if you want the intent to be obvious and stable. |
| `W006` | warning | An inner declaration hides another symbol with the same name. | The declaration is flagged as shadowing an existing symbol. | Rename the inner or outer symbol to avoid ambiguity. |
| `W007` | warning | The construct is still supported but scheduled for migration away from it. | The feature works, but the editor warns that it is deprecated. | Move to the recommended replacement before it becomes a harder migration. |
| `W008` | warning | The code exceeds the configured complexity threshold. | The POU or routine is flagged as hard to maintain. | Split the logic into smaller POUs or simplify the control flow. |
| `W009` | warning | A program, function, or function block is declared but not referenced. | The POU is flagged as unused. | Remove it, wire it into the project, or keep it intentionally with a clear package boundary. |
| `W010` | warning | The code uses time/date access patterns that reduce reproducibility. | Deterministic tests or replay flows may become harder to trust. | Prefer deterministic harness or runtime inputs when testability matters. |
| `W011` | warning | The code depends on timing-sensitive I/O access patterns. | Execution stays valid, but deterministic diagnosis becomes harder. | Move timing-sensitive behavior behind clearer tasking or deterministic test APIs. |
| `W012` | warning | Multiple tasks coordinate through shared globals unsafely. | The shared global access is flagged as a task hazard. | Reduce shared mutable state or make task ownership explicit. |
| `W013` | warning | The code compares floating-point values for exact equality. | The comparison is highlighted as numerically fragile. | Replace direct equality with a tolerance or epsilon comparison when appropriate. |
| `W014` | warning | The denominator is provably zero at compile time. | The division or modulo expression is flagged before runtime. | Fix the literal or guard the operation before it is executed. |

## Hints

| Code | Default severity | Cause | User-visible symptom | Typical first fix |
| --- | --- | --- | --- | --- |
| `I001` | hint | The compiler found a simpler equivalent form. | The code remains valid but gets a simplification hint. | Simplify the expression or ignore it if the current form is clearer for your team. |
| `I002` | hint | The code is valid but does not match the preferred style. | The editor shows a style hint without blocking build or test. | Apply the suggestion if you want the project to stay stylistically consistent. |

## Severity overrides

You can override severities in `trust-lsp.toml`, for example:

```toml
[diagnostics]
severity_overrides = { W003 = "error", W004 = "off" }
```

Use that for project-specific policy, but keep the default meaning of the code
in mind when deciding whether to silence or promote it.

## Related

- [trust-lsp.toml](config/trust-lsp-toml.md)
- [Troubleshooting](../troubleshooting.md)
- [Specifications overview](specifications/index.md)
