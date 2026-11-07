# HIR Warning Policy

This specification defines the warnings emitted by `trust-hir`. Warnings do
not reject a program and are not IEC 61131-3 conformance errors. They are a
truST product policy for deterministic engineering feedback.

## 1. Unused declarations

`trust-hir` warns for an unused local or temporary variable, local or global
constant, input parameter, PROGRAM, FUNCTION, or FUNCTION_BLOCK. A global
constant warns wherever it is declared: at the top level of a file, in a
NAMESPACE, in a CONFIGURATION or in a RESOURCE. The owner decided this on
1 November 2026; CODESYS's static analysis (rule SA0033) likewise reports a
project's unused global variables.

A declaration is used when a resolved semantic reference reaches it. The
following references count:

- a value or call reference;
- use of a FUNCTION_BLOCK as a declared type;
- a program instance selected by a configuration;
- a `VAR_CONFIG` access path;
- a resolved type reference;
- a qualified value access chain (`GVals.Member`, dotted declaration sizes):
  the chain resolves to its terminal member and marks that member used —
  the leftmost segment alone never marks anything used. The chain head
  resolves in the referencing expression's scope first, with the project-wide
  lookup as the fallback: a local declaration named like the head shadows the
  global, and the chain can never mark a global member used through that
  local's spelling;
- an `EXTENDS` or `IMPLEMENTS` target of a POU, class, or DUT declaration:
  inheritance is a use of the base;
- a use of a `VAR_EXTERNAL` declaration: a POU uses a global through it
  (IEC 61131-3, 6.5.2.1 and Figure 8), so the use reaches the global that
  the external names. The declaration alone is no use.

Imported declarations, declarations without a source range, members of a
FUNCTION_BLOCK, CLASS, or INTERFACE, interface-method parameters, and non-input
parameters do not receive this warning from the owning file. Interface-method parameters cover both the methods declared inside an
INTERFACE POU and the methods of a FUNCTION_BLOCK or CLASS that IMPLEMENTS an
interface declaring a method of the same name — the implementing POU must
carry the full interface signature even where its body ignores a pin. A
method outside every implemented interface keeps the warning for its
unused input parameters. A used declaration must not retain an unused warning.

Use is project-wide and identity based. A program selected by a configuration,
a function or function block referenced from another file, and a member
selected by a resolved `VAR_CONFIG` access path are used in their declaring
file. Assigning a FUNCTION or METHOD return value through its own declaration
name is part of that POU's result semantics and does not by itself make the POU
externally used.

A library file is library surface and receives none of these warnings, in the
editor and in `trust-lsp check` alike. A file is a library file when the
deepest folder that holds it, among its project's own source folders (the
root, or the `include_paths`) and its library folders (`[[libraries]]`
entries, `[project] library_paths` and resolved `[dependencies]` packages),
is a library folder; at equal depth the project's own folder wins
(specification 14 §7.6). CODESYS and TwinCAT judge a library in its own
project, where its files are that project's own sources and every warning is
on, and truST does the same: "unused" is a claim about all of a declaration's
users, and the application is only one of a library's users. Every other
diagnostic is a property of the code itself — errors, implicit conversion,
complexity, unreachable code — and stays on for library files.

## 2. Implicit conversion

An accepted implicit conversion may emit `ImplicitConversion` when the source
and destination declared types differ. The warning does not replace assignment
compatibility: an incompatible conversion remains an error.

A derived DUT converted to its own ancestor (directly or transitively) does
not emit this warning: the conversion is a well-defined base-part-copy upcast
that assignment compatibility already accepts, and vendor toolchains treat it
as an ordinary inheritance assignment. Conversions between DUTs without an
ancestor relation keep the warning.

A fixed-size array bound to an `ARRAY[*]` parameter, and the reverse arm of
the same in/out binding, do not emit this warning either: the parameter
aliases the argument's storage for the call, so no element conversion takes
place (IEC 61131-3 varying arrays). Compatibility keeps the error channel
unchanged: element mismatches and concrete bound mismatches keep their
rejections.

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

`NondeterministicTimeDate` tracks clock sources, not declarations. A
declaration of time/date storage cannot introduce nondeterminism, so no TIME,
LTIME, DATE, LDATE, TOD, LTOD, DT or LDT declaration ever warns — variable,
parameter, function or method result, property, or an alias resolving to one
of those types, declared locally or imported. What counts as a source follows
truST's own clock contract, specification 07, "Runtime Clock Sources":

- `CURRENT_DT` is the source. It samples the host's wall clock, and
  deterministic replay does not control it. The warning is emitted at a call
  of `CURRENT_DT()`, and at the call site of a function, method, function
  block or program whose body reads it, directly or through the POUs it calls.
  The message names the source: "`CURRENT_DT` reads the host clock, which
  replay does not control".
- `TIME` is not a source. It returns the runtime's logical elapsed time, which
  replay controls (specification 07 recommends it for exactly that reason).
  Neither it nor what is built on it warns: `T_PLC_MS`, `T_PLC_US`, `RTC_MS`,
  `RTC_2` and the timers read runtime clock sources, not the host clock.
- There is no declaration-level fallback: a TIME/DATE-typed declaration never
  warns, and a value whose source cannot be traced does not warn either. A
  call that cannot be resolved does not warn, for the same reason: a lint does
  not name a clock that is not there. One declaration is a source in its own
  right: a POU carrying the `reads_host_clock` pragma attribute (specification
  07, "Declared Clock Sources") declares that it samples the host clock, so a
  body-less library stub that carries it is a source. The attribute is read
  from the pragma trivia before the POU's declaration keyword, never from the
  body. The stub's own file does not warn — the declaration speaks for the
  body it replaces, and the warnings appear at the call sites, as with a
  linked library body. When a call reaches a declared source, the message
  names it: at the call of the declared POU itself, "`FuGetDateAndTime`
  reads the host clock (declared reads_host_clock), which replay does not
  control"; at a call site the source reaches through other POUs, "'mHandle'
  reads the host clock through 'FuGetDateAndTime' (declared
  reads_host_clock), which replay does not control" — the through-form
  belongs to the call the declared POU's key resolves to, so a wrapper of
  the same name in another namespace reads as a reader, never as a
  declarer. A call that reaches more
  than one source names the one whose direct reader comes first in the
  project's file order and document order. This section keeps the
  library-surface decision of #180: a clock read in a library body still
  warns where that body is linked into the project.

Call-site ranges select the called name, and the call graph is computed from
the bodies of the project's POUs, resolving each body call in its scope: an
instance's call maps to the block the instance variable declares, `inst.M()`
maps to that type's method, and a bare call never takes a same-named method's
clock.

`NondeterministicIo` is emitted for a live declaration bound to a direct input
or output address whose normalized address begins with `%I` or `%Q`.
Memory-addressed `%M` storage is not classified as external I/O by this
warning. Address-space comparison is ASCII case-insensitive and ignores
surrounding whitespace. Other, missing, or malformed address prefixes do not
receive a guessed I/O warning.

## 6. Shared global task hazards

`SharedGlobalTaskHazard` (`W012`) is emitted for a resolved global that
programs assigned to more than one task access, when that access can race:

- two or more of those tasks write it, as the CODESYS Static Analysis rule
  SA0006 "Write access from several tasks" reports; or
- one task writes it, another task reads it, and its data is not atomic, as
  SA0103 reports for non-atomic data: `STRING` and `WSTRING`, arrays,
  structures, unions, function block and class instances, and 64-bit values.
  The 64-bit values are `LINT`, `ULINT`, `LWORD`, `LREAL`, `LTIME`, `LDATE`,
  `LTOD` and `LDT`, and also `TIME`, `DATE`, `TOD` and `DT`, which truST
  stores in eight bytes (specification 02). truST also runs on 32-bit chips,
  where a 64-bit access takes two, so no project target makes 64-bit data
  atomic.

A global that one task writes and other tasks only read is silent when its
data is atomic: an elementary value of 32 bits or less, or an enumeration or
subrange of one. A pointer or reference counts as its target, the data the
tasks reach through it, and an interface as the function block instance it
designates. A one-word read cannot tear, and the consistency between several
such globals is the program's protocol, which no lint can judge; CODESYS has
no rule for it either. A global that no task writes is silent, and so is a
single-written global whose type does not resolve (section 9).

`{attribute 'cross_task'}` declares a global shared between tasks by design,
and the warning is silent for it in every pattern, several writers included:
the project owns the protocol, such as a hand-off flag or a ring buffer with
one consumer. Before a `VAR_GLOBAL` list the attribute covers every global of
the list; before a single global inside the list, that global. It carries no
value and names no tasks: which program runs in which task belongs to the
configuration and changes with it, while the attribute states a property of
the data. Undeclared globals keep the rule above, so an unexpected cross-task
access still surfaces.

Multiple programs on the same task do not by themselves trigger the warning.
Cross-file and namespaced globals are compared by resolved identity, not by an
unqualified spelling guess.

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
