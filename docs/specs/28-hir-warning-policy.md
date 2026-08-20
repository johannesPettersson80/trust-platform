# HIR Warning Policy

This specification defines the warnings emitted by `trust-hir`. Warnings do
not reject a program and are not IEC 61131-3 conformance errors. They are a
truST product policy for deterministic engineering feedback.

## 1. Unused declarations

`trust-hir` warns for an unused local or temporary variable, local constant,
input parameter, PROGRAM, FUNCTION, or FUNCTION_BLOCK.

A declaration is used when a resolved semantic reference reaches it. The
following references count:

- a value or call reference;
- use of a FUNCTION_BLOCK as a declared type;
- a program instance selected by a configuration;
- a `VAR_CONFIG` access path;
- a resolved type reference.

Imported declarations, declarations without a source range, members of a
FUNCTION_BLOCK, CLASS, or INTERFACE, interface-method parameters, non-input
parameters, and global constants do not receive this warning from the owning
file. A used declaration must not retain an unused warning.

Use is project-wide and identity based. A program selected by a configuration,
a function or function block referenced from another file, and a member
selected by a resolved `VAR_CONFIG` access path are used in their declaring
file. Assigning a FUNCTION or METHOD return value through its own declaration
name is part of that POU's result semantics and does not by itself make the POU
externally used.

## 2. Implicit conversion

An accepted implicit conversion may emit `ImplicitConversion` when the source
and destination declared types differ. The warning does not replace assignment
compatibility: an incompatible conversion remains an error.

## 3. Cyclomatic complexity

Cyclomatic complexity is one plus the number of IF, ELSIF, FOR, WHILE, REPEAT,
and CASE-branch decision points owned directly by one PROGRAM, FUNCTION,
FUNCTION_BLOCK, METHOD, or PROPERTY. Nested POUs are counted separately.

`HighComplexity` is emitted only when the value is greater than 15. The
diagnostic may retain at most three decision points as related locations.

## 4. Unreachable statements

`UnreachableCode` is emitted for a statement after an unconditional terminator
and for a branch proven unreachable from a constant-false IF or ELSIF
condition. This warning is conservative; absence of the warning is not proof
that arbitrary control flow is reachable.

Within one statement list, `RETURN`, `EXIT`, `CONTINUE`, and `JMP` are
unconditional terminators; every following sibling statement is warned.
Termination inside a nested statement list does not terminate its containing
list.

The branch proof folds only side-effect-free Boolean constants made from
`TRUE`, `FALSE`, parentheses, unary `NOT`, and binary `AND`, `OR`, or `XOR`.
It does not guess values for names, calls, comparisons, or other expressions.
A false `IF`/`ELSIF` condition makes that branch unreachable. Once a true
condition is encountered in an ordered `IF`/`ELSIF` chain, every later
`ELSIF` and `ELSE` branch is unreachable, regardless of whether earlier
conditions were constant or unknown. Each unreachable statement receives the
warning at its own source range.

## 5. Nondeterministic values and I/O

`NondeterministicTimeDate` is emitted for live variable-like declarations of
TIME, LTIME, DATE, LDATE, TOD, LTOD, DT, or LDT, including aliases.
Variable-like declarations are variables, parameters, function or method
results, and properties. Type declarations and constants do not themselves
represent a changing clock value and do not receive this warning. Imported and
synthetic declarations are diagnosed only at their real source declaration,
never again in each consuming file.

`NondeterministicIo` is emitted for a live declaration bound to a direct input
or output address whose normalized address begins with `%I` or `%Q`.
Memory-addressed `%M` storage is not classified as external I/O by this
warning. Address-space comparison is ASCII case-insensitive and ignores
surrounding whitespace. Other, missing, or malformed address prefixes do not
receive a guessed I/O warning.

## 6. Shared global task hazards

`SharedGlobalTaskHazard` is emitted when one resolved global is accessed by
programs assigned to more than one task and at least one participating task
writes it. Multiple programs on the same task do not by themselves trigger
the warning. Cross-file and namespaced globals are compared by resolved
identity, not by an unqualified spelling guess.

Task identity includes its enclosing configuration and resource and compares
the task name case-insensitively. Consequently, two spellings of one task in
one resource denote one scheduling context, while equally spelled tasks in
different resources or configurations remain distinct. A program instance
whose task or program type cannot be resolved does not contribute guessed
accesses.

A resolved global reference is a write when it is the assignment target,
including through a qualified field, array index, or dereference target.
References on the assignment right-hand side and in other expression contexts
are reads. A local declaration that shadows a global remains a distinct symbol.
Read-only access from any number of tasks is not a write hazard.

The warning is reported on the owning global declaration. Its message lists
task labels in lexical order, includes at most three labels per access/write
list followed by the remaining count, and retains at most three writing tasks
as related locations. These bounds keep diagnostics deterministic and compact;
they do not weaken the underlying all-task hazard decision.

## 7. Floating-point equality

`FloatingPointEquality` is emitted for equality or inequality when either
resolved operand belongs to the REAL or LREAL family. Integer-only equality
does not emit this warning.

## 8. Literal zero divisor

`LiteralDivisionByZero` is emitted when the right operand of DIV or MOD is a
numeric literal whose value is zero. A nonliteral divisor is not reported by
this syntactic warning, even when runtime data could make it zero.

## 9. Stability and suppression

Warnings use their registered diagnostic codes and remain warnings rather than
errors. A warning is suppressed when its prerequisite cannot be resolved; a
wrong-reason warning must not be manufactured from an unknown type, symbol,
task, address, or expression.
