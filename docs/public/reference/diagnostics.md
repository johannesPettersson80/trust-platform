# Diagnostics

This page documents the current code set exposed by the semantic/compiler
diagnostic layer in `crates/trust-hir/src/diagnostics.rs`.
Use it when you are:

- reading diagnostics in the editor or CLI
- configuring severity overrides in `trust-lsp.toml`
- teaching an agent how to interpret a code quickly

If you need exact override syntax, go to
[trust-lsp.toml](config/trust-lsp-toml.md). If you need troubleshooting by
symptom instead of by code, go to [Troubleshooting](../troubleshooting.md).

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

| Code | Default severity | Meaning | Typical first fix |
| --- | --- | --- | --- |
| `E001` | error | Unexpected token. The parser found a token that does not fit the current grammar rule. | Check the previous line first for a missing `;`, `END_*`, or misplaced keyword. |
| `E002` | error | Missing token. A required token was omitted. | Add the missing delimiter or closing keyword the parser expects. |
| `E003` | error | Unclosed block. A block such as `IF`, `CASE`, `FUNCTION_BLOCK`, or similar was not closed correctly. | Match the opening construct with the correct closing token like `END_IF`, `END_CASE`, or `END_FUNCTION_BLOCK`. |

## Name resolution errors

| Code | Default severity | Meaning | Typical first fix |
| --- | --- | --- | --- |
| `E101` | error | Undefined variable. A referenced variable name does not resolve in the current scope. | Check spelling, scope, and whether the declaration is in the project/dependency set. |
| `E102` | error | Undefined type. A declared or referenced type name cannot be resolved. | Verify the type name and that the defining file/package is included. |
| `E103` | error | Undefined function or method. A call target cannot be resolved. | Check the callable name and whether the correct namespace/library is available. |
| `E104` | error | Duplicate declaration. The same symbol is declared more than once in the same effective scope. | Rename one declaration or remove the duplicate definition. |
| `E105` | error | Cannot resolve name. The compiler cannot disambiguate or resolve a symbol reference. | Check namespace qualifiers, imports, and whether you meant a different symbol kind. |
| `E106` | error | Invalid identifier. The identifier form is not accepted for the current context. | Rename the symbol to a valid IEC/truST-compatible identifier. |

## Type errors

| Code | Default severity | Meaning | Typical first fix |
| --- | --- | --- | --- |
| `E201` | error | Type mismatch. Two types do not match where exact compatibility is required. | Check the source and target types, then add an explicit conversion if appropriate. |
| `E202` | error | Invalid operation. The operation is not valid for the operand type(s). | Verify the operator and the actual operand types. |
| `E203` | error | Incompatible assignment. The right-hand side cannot be assigned to the left-hand side safely. | Convert the value explicitly or change the destination type. |
| `E204` | error | Wrong argument count. A call passed too few or too many arguments. | Match the call site to the declared function or FB signature. |
| `E205` | error | Invalid argument type. The call shape is right, but one or more argument types are wrong. | Check parameter types and add explicit conversions where intended. |
| `E206` | error | Missing return. A function or method path does not produce a required result value. | Ensure every control-flow path assigns the implicit result or returns correctly. |
| `E207` | error | Invalid return type. The returned value does not match the declared result type. | Align the returned expression with the declared return type. |

## Semantic errors

| Code | Default severity | Meaning | Typical first fix |
| --- | --- | --- | --- |
| `E301` | error | Invalid assignment target. The left-hand side is not writable in this context. | Assign to a variable, field, or indexable storage location instead of a non-lvalue expression. |
| `E302` | error | Constant modification. A constant or otherwise immutable symbol is being written. | Remove the write or copy the value into a mutable variable first. |
| `E303` | error | Invalid array index. The index expression is not valid for the array shape. | Check index type, bounds, and whether the base expression is actually an array. |
| `E304` | error | Out of range. A value or bound exceeds the accepted range. | Reduce the literal or expression to a representable range for the destination type. |
| `E305` | error | Cyclic dependency. Declarations depend on one another in a cycle the compiler cannot resolve. | Break the cycle by extracting shared types/constants or reordering ownership. |
| `E306` | error | Invalid task configuration. Task metadata is malformed or incomplete. | Check task names, intervals, priorities, and required runtime config fields. |
| `E307` | error | Unknown task. A program or config references a task that does not exist. | Define the task or update the reference to an existing task name. |

## Warnings

| Code | Default severity | Meaning | Typical first fix |
| --- | --- | --- | --- |
| `W001` | warning | Unused variable. A declared variable is never read. | Remove it or use it intentionally. |
| `W002` | warning | Unused parameter. A parameter is declared but never used. | Remove it, use it, or document why it is intentionally reserved. |
| `W003` | warning | Unreachable code. A code path can never execute. | Remove the dead block or simplify the surrounding control flow. |
| `W004` | warning | Missing `ELSE` branch. A conditional branch has no fallback path. | Add an `ELSE` if the branch should be exhaustive, or override severity if the omission is deliberate. |
| `W005` | warning | Implicit conversion. The compiler inserted a conversion you did not write explicitly. | Replace it with an explicit conversion if you want the intent to be obvious and stable. |
| `W006` | warning | Shadowed variable. A declaration hides another symbol with the same name. | Rename the inner or outer symbol to avoid ambiguity. |
| `W007` | warning | Deprecated feature. The construct is supported but discouraged. | Move to the recommended replacement before it becomes a harder migration. |
| `W008` | warning | High complexity. The code exceeds the configured complexity threshold. | Split the logic into smaller POUs or simplify the control flow. |
| `W009` | warning | Unused POU. A program, function, or function block is declared but not referenced. | Remove it, wire it into the project, or keep it intentionally with a clear package boundary. |
| `W010` | warning | Non-deterministic time/date usage. The code uses time/date access patterns that make reproducible execution harder. | Prefer deterministic harness/runtime inputs when testability matters. |
| `W011` | warning | Non-deterministic I/O usage. The code depends on timing-sensitive I/O access patterns. | Move timing-sensitive behavior behind clearer tasking or deterministic test surfaces. |
| `W012` | warning | Shared global task hazard. Multiple tasks write or coordinate through the same global unsafely. | Reduce shared mutable state or make task ownership explicit. |
| `W013` | warning | Floating-point equality comparison. Exact float equality is often fragile. | Replace direct equality with a tolerance/epsilon comparison when appropriate. |
| `W014` | warning | Literal division or modulo by zero. The denominator is provably zero at compile time. | Fix the literal or guard the operation before it is executed. |

## Hints

| Code | Default severity | Meaning | Typical first fix |
| --- | --- | --- | --- |
| `I001` | hint | Simplification suggestion. The compiler found a simpler equivalent form. | Simplify the expression or ignore if the current form is clearer for your team. |
| `I002` | hint | Style suggestion. The code is valid but does not follow preferred style. | Apply the suggestion if you want the project to stay stylistically consistent. |

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
