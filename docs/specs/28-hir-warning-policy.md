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
and for a branch proven unreachable from a literal-false IF or ELSIF
condition. This warning is conservative; absence of the warning is not proof
that arbitrary control flow is reachable.

## 5. Nondeterministic values and I/O

`NondeterministicTimeDate` is emitted for live variable-like declarations of
TIME, LTIME, DATE, LDATE, TOD, LTOD, DT, or LDT, including aliases.

`NondeterministicIo` is emitted for a live declaration bound to a direct input
or output address whose normalized address begins with `%I` or `%Q`.
Memory-addressed `%M` storage is not classified as external I/O by this
warning.

## 6. Shared global task hazards

`SharedGlobalTaskHazard` is emitted when one resolved global is accessed by
programs assigned to more than one task and at least one participating task
writes it. Multiple programs on the same task do not by themselves trigger
the warning. Cross-file and namespaced globals are compared by resolved
identity, not by an unqualified spelling guess.

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
