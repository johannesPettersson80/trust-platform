# IEC Decisions Log

Authoritative location:
- This tracked file is the repository source of truth for IEC interpretation decisions.
- Do not point tracked docs or code comments at legacy internal IEC decision-log paths.

This file tracks implementation decisions made where IEC 61131-3 leaves room for interpretation.

## 2026-07-30 - POU call evaluation, execution control, and output transfer

- Area: Function, function-block, and method calls
- IEC context:
  - IEC 61131-3 Ed.3 sections 6.6.1.4.1-6.6.1.4.2 require a
    `VAR_IN_OUT` actual to be properly mapped, define complete and incomplete
    formal lists, and make formal binding order insignificant.
  - Section 6.6.1.5 defines `EN`/`ENO`, but makes it
    implementer-specific whether other actuals are evaluated or bound when
    `EN` is false and what happens to outputs when `ENO` is false.
  - Section 7.3.2 requires the left operand of an expression to be evaluated
    first, but does not completely order a list of call actuals or define
    overlapping output targets.
- Decision:
  - Actuals are evaluated exactly once from left to right in source order.
    Formal names affect binding only; reordering formal assignments does not
    reorder their evaluation. A writable target is resolved once at its source
    position.
  - `EN`, when supplied, is evaluated first even if it is not the first written
    formal actual. If it is false, no other actual expression or writable
    target is evaluated or resolved, the POU body is not entered, and `ENO`
    alone is copied as false when connected.
  - When `EN` is true or omitted, `ENO` is initialized to true before body
    execution. The body may set it false. A runtime execution error aborts the
    call, reports the error, and performs no result, output, in-out, or instance
    state commit visible through the call boundary.
  - A normally returning call first captures every connected `VAR_OUTPUT`,
    `VAR_IN_OUT`, and `ENO` value, validates every destination, and then commits
    the complete transfer. If `ENO` is false on normal return, the function
    result is still returned and ordinary connected outputs/in-outs are still
    transferred; `ENO` reports application status rather than rolling back a
    successful call.
  - Mapping the same caller storage, including overlapping aggregate
    projections, to more than one writable formal in one call is rejected as
    ambiguous. This applies to any pair of `VAR_OUTPUT`, `VAR_IN_OUT`, or
    `ENO` connections. A read-only input may read storage that is also mapped
    once as writable.
  - Function and value-returning method results use the declared type default
    when `EN` is false. A skipped function-block or void-method call preserves
    its instance state and connected caller targets except for connected
    `ENO := FALSE`.
- Reason:
  - A source-order, evaluate-once rule makes side effects auditable. Early
    execution-control gating prevents disabled calls from faulting through
    unused arguments. Rejecting multiple writers and committing a validated
    transfer as one unit removes declaration-order-dependent caller state.

## 2026-07-30 - Boolean short-circuit extent

- Area: Structured Text Boolean expression evaluation
- IEC context:
  - IEC 61131-3 Ed.3 section 7.3.2 requires the left operand of a binary
    operator to be evaluated first and makes the extent of Boolean-expression
    evaluation implementer-specific, including possible side effects.
- Decision:
  - For `BOOL` operands, `AND` and `&` do not evaluate the right operand after
    a false left operand. `OR` does not evaluate the right operand after a true
    left operand.
  - `BOOL XOR` evaluates both operands.
  - `AND`, `&`, `OR`, and `XOR` on bit strings evaluate both operands and then
    apply the eager width-preserving bitwise operation.
  - Every evaluated binary operand is evaluated left first. An operand skipped
    by the Boolean rule produces no call, fault, read, write, or other side
    effect.
- Reason:
  - A closed source-level rule makes function-call side effects and fault
    suppression deterministic while retaining eager value semantics for bit
    strings.

## 2026-07-30 - CASE label ordering, ranges, and unmatched values

- Area: Structured Text `CASE` selection
- IEC context:
  - IEC 61131-3 Ed.3 section 7.3.3.3.3 requires a selector of elementary type,
    labels of a comparable type, and at most one selected statement group, but
    it does not prescribe a diagnostic policy for duplicate labels,
    overlapping subranges, or a subrange written with its upper bound first.
  - The same section states that a selector matching no label executes the
    `ELSE` group when present and otherwise executes no statement.
- Decision:
  - truST rejects duplicate scalar labels, a scalar contained in a range,
    overlapping ranges, and a range whose lower bound is greater than its
    upper bound. It never normalizes or reorders a reversed range.
  - Labels are compared in source order after constant evaluation. Accepted
    source therefore has at most one matching branch; runtime still selects the
    first matching branch as a fail-safe rule for an independently constructed
    program model.
  - A selector matching no label and no `ELSE` branch completes as a no-op.
- Reason:
  - Rejecting ambiguous or visually reversed partitions makes the reviewed
    selection domain explicit and prevents source order from hiding
    overlapping safety logic.

## 2026-07-30 - FOR evaluation, step, and post-loop state

- Area: Structured Text `FOR` iteration
- IEC context:
  - IEC 61131-3 Ed.3 section 7.3.3.4.2 requires the control variable, initial
    value, and final value to have the same integer type, defines the
    start-of-iteration test, and makes the control variable's value after
    termination implementer-specific.
  - The section permits a `BY` expression but does not fully prescribe operand
    evaluation timing, a zero increment result, overflow behavior, or the
    post-loop value for normal completion, zero iterations, and `EXIT`.
- Decision:
  - The control variable, initial expression, final expression, and explicit
    `BY` expression have one exact integer type. An omitted `BY` is the value
    one in that type.
  - Initial, final, and step expressions are each evaluated exactly once, in
    that source order, before the first termination test. Their values are
    captured; later body assignments to variables used by those expressions
    do not change the iteration bounds or step.
  - A zero step reports `RuntimeError::ForStepZero` before the control variable
    or loop body is mutated. Advancing beyond the integer type's representable
    range reports `RuntimeError::Overflow` before storing a wrapped value.
  - Positive and negative loops include a final value that is reached exactly.
    A direction that cannot approach the bound executes zero iterations.
  - On normal completion the control variable contains the first value beyond
    the bound. A zero-iteration loop leaves it at the evaluated initial value.
    `EXIT` leaves it at the value for the iteration that executed `EXIT`.
    `CONTINUE` performs the normal increment before the next termination test.
  - IEC's prohibition on modifying the control variable and variables used for
    initial and final values is enforced statically. A variable used only by
    the step expression may be modified in the body because the step has
    already been captured.
- Reason:
  - One-time ordered evaluation and checked mutation provide deterministic
    scan-cycle behavior. The explicit post-loop contract removes a known
    implementer-specific portability hazard without permitting wraparound or
    partial mutation on a rejected loop.

## 2026-07-30 - Variable-section qualifier placement and retention ownership

- Area: Structured Text variable-section qualifiers
- IEC context:
  - IEC 61131-3 Ed.3 Figure 7 summarizes `CONSTANT` as a qualifier that may
    follow the variable-section keywords, while Tables 19, 40, 47, and 48
    explicitly illustrate only ordinary constant declarations and
    `VAR_EXTERNAL CONSTANT`.
  - Sections 6.5.6.1-6.5.6.2 permit `RETAIN` and `NON_RETAIN` for stored
    variables of function blocks and programs and name static `VAR`,
    `VAR_INPUT`, `VAR_OUTPUT`, and `VAR_GLOBAL` as eligible locations; they
    exclude `VAR_IN_OUT`.
- Decision:
  - truST accepts `CONSTANT` on every legal storage-declaration section:
    `VAR`, `VAR_STAT`, `VAR_INPUT`, `VAR_OUTPUT`, `VAR_IN_OUT`, `VAR_TEMP`,
    `VAR_GLOBAL`, and `VAR_EXTERNAL`. The declaration remains read-only even
    when the section normally denotes writable or aliased storage.
    `VAR_ACCESS` and `VAR_CONFIG` do not accept `CONSTANT`.
  - `RETAIN` and `NON_RETAIN` are accepted only where the declaration owns
    state across calls or cycles: ordinary `VAR` in function blocks, programs,
    and classes; truST `VAR_STAT`; function-block/program inputs and outputs;
    and `VAR_GLOBAL`. Function and method ordinary variables, inputs, and
    outputs are call-local and therefore do not accept a retention policy.
  - `VAR_IN_OUT`, `VAR_TEMP`, `VAR_EXTERNAL`, `VAR_ACCESS`, and `VAR_CONFIG`
    never own a retention policy.
  - truST `PERSISTENT` follows exactly the same placement rules as `RETAIN`.
    A variable section accepts at most one occurrence of one of `CONSTANT`,
    `RETAIN`, `NON_RETAIN`, or `PERSISTENT`; duplicates and combinations are
    errors.
- Reason:
  - A qualifier must not be accepted and then discarded by lowering.
    Separating immutable access from restart ownership preserves the broad
    Figure 7 constant interpretation while limiting retention metadata to
    storage that can actually survive an invocation boundary.

## 2026-07-30 - Function-block ordinary-member default access

- Area: Object-oriented function-block member visibility
- IEC context:
  - IEC 61131-3 Ed.3 Table 40 describes the non-object-oriented
    function-block declaration and gives ordinary static `VAR` declarations a
    `PRIVATE` default.
  - Table 53 and sections 6.6.7.6-6.6.7.7 add the object-oriented
    function-block profile, define method and ordinary-variable access by
    reference to the class rules in sections 6.6.5.9-6.6.5.10, and make
    `PROTECTED` the default for those members.
- Decision:
  - truST implements the Edition 3 object-oriented function-block profile
    uniformly. An ordinary `VAR` member, method, or truST `PROPERTY` without
    an explicit access specifier is therefore `PROTECTED`, whether or not that
    particular function block declares `EXTENDS`, `IMPLEMENTS`, or a method.
  - `VAR_INPUT` and `VAR_OUTPUT` keep their separate IEC-mandated implicit
    `PUBLIC` access, `VAR_EXTERNAL` keeps its implicit `PROTECTED` access, and
    `VAR_IN_OUT` remains limited to the function-block body and call
    statement. The ordinary-member default does not override those
    direction-specific rules.
- Reason:
  - Selecting the object-oriented profile is necessary for a language that
    supports function-block inheritance, methods, interfaces, `THIS`, and
    `SUPER`. One uniform default also prevents the visibility of an existing
    member from changing merely because a method or base clause is later added.

## 2026-07-22 - Non-finite REAL result and explicit-conversion policy

- Area: `REAL` arithmetic, numerical functions, and explicit conversions
- IEC context:
  - IEC 61131-3 Ed.3 section 6.4.2.1 and Table 10 footnote e make
    exceptional basic-single floating-point results implementer-specific.
  - Sections 6.6.2.5.2-6.6.2.5.3 make conversion accuracy effects,
    conversion execution errors, and the result of an out-of-range
    `LREAL_TO_REAL` conversion implementer-specific.
  - Section 6.6.2.5.5 and Table 25 define `DWORD_TO_REAL` and
    `LWORD_TO_LREAL` as binary transfers but do not prescribe a stored
    non-finite-value policy.
- Decision:
  - For finite `REAL` operands, binary `+`, `-`, `*`, `/`, and `**` report
    `RuntimeError::Overflow` when the basic-single result is not finite.
  - For finite `REAL` operands, `EXP` and `EXPT` report
    `RuntimeError::Overflow` when the basic-single result is not finite.
  - Every explicit conversion targeting `REAL` or `LREAL` reports
    `RuntimeError::Overflow` when parsing, narrowing, or binary transfer would
    produce NaN, positive infinity, or negative infinity. Finite binary-transfer
    values preserve their IEC-defined bit transfer.
  - Each rejection occurs before assignment storage. The target remains
    unchanged; the runtime does not clamp, normalize, or substitute a value.
- Reason:
  - A visible no-write fault prevents an exceptional IEEE value from silently
    becoming ordinary process state while preserving every finite result and
    binary transfer required by the standard.

## 2026-07-15 - Accuracy-preserving implicit conversion and common-type matrix

- Area: Assignment, initialization, function results, POU parameter transfer,
  expressions, operators, and overloaded standard functions
- IEC context: IEC 61131-3 Ed.3 section 6.6.1.6 permits implicit conversion in
  assignments and input/output parameter transfer only when it keeps the value
  and accuracy of the source type, and forbids implicit conversion for
  `VAR_IN_OUT`. Section 6.6.1.6 rule 7 permits conversion to make operator or
  overloaded-function operands and results the same type. Section 6.6.1.7.2
  leaves mixed same-kind input conversion implementer-specific.
- Decision:
  - Signed integers widen only within `SINT -> INT -> DINT -> LINT`.
  - Unsigned integers widen only within `USINT -> UINT -> UDINT -> ULINT`.
  - Bit strings widen only within `BYTE -> WORD -> DWORD -> LWORD`.
  - Integer-to-real implicit conversion is limited to ranges that are exactly
    representable for every source value: `SINT` and `INT` may widen to
    `REAL`; `SINT`, `INT`, and `DINT` may widen to `LREAL`.
  - `REAL` may widen to `LREAL`. Typed `DINT -> REAL` and `LINT -> LREAL`
    require an explicit conversion because some source values require
    rounding. Signed/unsigned cross-family, numeric/`BOOL`, and other
    incompatible conversions also require an explicit conversion where one
    exists.
  - Contextual untyped numeric literals may initialize or assign directly to a
    target type when the literal is representable by that target.
  - Typed operands of an operator or overloaded standard function have a
    common type only when they are already identical or one operand can use the
    closed accuracy-preserving widening matrix above to reach the other
    operand's type. If neither direction is permitted, semantic analysis and
    runtime evaluation reject the operation instead of selecting a type by
    total numeric rank.
  - A representable untyped numeric literal is contextualized to the other
    typed operand or to the common typed argument of an overloaded standard
    function. Explicitly typed literals remain governed by the strict matrix.
  - A value that reaches a declared VM primitive with an incompatible runtime
    tag is rejected with `TypeMismatch` before storage. Stable public error-code
    mapping remains governed by `SPEC_GAP_VM_ERROR_MODEL_001`.
- Reason:
  - Type width alone is not sufficient for integer-to-floating conversion:
    binary32 cannot represent every `DINT`, and binary64 cannot represent every
    `LINT`. Requiring explicit conversion makes possible rounding visible in
    source while retaining the exact IEC-permitted widenings.

## 2026-07-14 - Malformed Structured Text parser recovery policy

- Area: Structured Text control-flow and expression parsing
- IEC context: IEC 61131-3 Ed.3 section 7.3.3.3 and Table 72 define the
  required `THEN`, `OF`, label colon, `:=`, `TO`, `DO`, and terminator tokens
  for selection and iteration statements; sections 7.3.3.4.2 through
  7.3.3.4.4 define the `FOR`, `WHILE`, and `REPEAT` forms. The standard defines
  valid syntax but does not prescribe an editor-oriented partial-tree recovery
  algorithm for malformed source.
- Decision:
  - Missing required control-flow tokens always produce a parse diagnostic and
    make the parse result unsuccessful. A retained partial syntax tree is for
    tooling recovery only and cannot make the malformed construct valid.
  - Recovery must advance over an offending token or stop at a known statement,
    block, POU, or end-of-file synchronization boundary; it must not retry the
    same token indefinitely.
  - A missing inner terminator stops before an outer terminator, reports the
    inner error, and leaves the outer boundary available to its owning
    construct. Statements following that outer construct remain parseable.
  - Missing POU and expression delimiters diagnose at the nearest bounded
    boundary. Expression nesting and recovery scans remain explicitly bounded.
  - A bounded expression-recovery scan treats commas and closing delimiters as
    top-level synchronization tokens only outside nested parentheses and
    brackets. Balanced nested delimiters do not hide the owning closer, while
    an unclosed nested bracket prevents a parenthesis from closing the outer
    construct and makes recovery stop before the next configured statement
    boundary.
- Reason:
  - Silent acceptance can turn malformed control flow into a different valid
    program, which is unsafe for both compilation and editor diagnostics.
  - Preserving a partial tree is useful to IDE features only when the parse
    remains visibly failed and recovery does not consume unrelated constructs.

## 2026-07-12 - Standard timer scan-step and lifecycle policy

- Area: TP, TON, TOF, and their LTIME variants
- IEC context: IEC 61131-3 Ed.3 section 6.6.3.5.5 requires Table 46 and Figure 15 timer behavior, supports TIME and LTIME variants, and states that the effect of changing `PT` during timing is implementer-specific. Restart retention is governed separately by section 6.5.6.2.
- Decision:
  - Timer inputs and outputs are observed at executed function-block call boundaries. The first call initializes the elapsed-time baseline and contributes zero elapsed time. No background or continuous-time transition is implied.
  - `PT` is sampled on every executed call while timing is active. Changing it immediately changes the active threshold; elapsed time is compared with the new non-negative value. After TOF expires, its `ET` holds the `PT` value used at expiry until the instance is rearmed.
  - `PT <= T#0s` is treated as zero for TP, TON, and TOF.
  - A skipped or conditional call performs no state transition. On the next executed call, elapsed time is measured from the preceding executed call, so the skipped interval is included.
  - If the runtime clock does not advance or moves backward, that call contributes zero elapsed time and establishes the new clock baseline for the next call.
  - Warm and cold restart reinitialize non-retained timer instances, including `Q`, `ET`, edge state, and the elapsed-time baseline. An in-process restart preserves the runtime's current monotonic time and establishes each new timer and task baseline at that value; only construction of a new runtime starts a new zero-based time epoch. The first executed timer call after restart therefore contributes zero elapsed time without rewinding the runtime clock. Retained function-block instance storage is a separate runtime-retention concern and is neither asserted nor changed by this timer vertical.
  - A TP pulse is not cancelled by a sampled falling input. A new sampled FALSE-to-TRUE edge starts a new PT interval; the first timer proof keeps IN high through expiry and makes no retrigger or short-input assertion.
  - TIME and LTIME variants use the same state transitions and clock source; `PT` and `ET` retain the variant's duration type.
- Reason:
  - Scan-boundary observation makes the IEC timing diagrams deterministic in a cyclic runtime while exposing implementation-owned boundaries explicitly.
  - The first proof vertical asserted Figure 15's basic TP/TON behavior and TOF post-expiry plateau. Later traces may assert the reviewed restart, clock-step, PT-change, and short-input decisions independently.

## 2026-07-13 - STRING and WSTRING binding-capacity policy

- Area: STRING/WSTRING assignment and POU parameter binding
- IEC context: IEC 61131-3 Ed.3 Table 10 defines declared string maxima, and section 6.6.1.2.2 makes the result of assigning a longer source string implementation-specific.
- Decision:
  - Ordinary assignment, `VAR_INPUT` copy-in, function result assignment, and `VAR_OUTPUT` copy-back truncate by Unicode scalar value to the receiving declaration's capacity.
  - `VAR_IN_OUT` requires identical string family and identical effective capacity after alias and constant-expression resolution. Width-changing and bounded-to-unbounded `VAR_IN_OUT` bindings are rejected with diagnostic category `E205` rather than converted.
  - A rejected `VAR_IN_OUT` binding cannot mutate caller state.
  - Literal bounds and truncation count Unicode scalar values. The same rules apply to bounded `STRING[n]` and `WSTRING[n]`.
  - `STRING` and `WSTRING` are separate families. Cross-family assignment or
    parameter transfer requires an explicit standard conversion function.
- Reason:
  - Truncation preserves the product's established ordinary-assignment behavior while enforcing every receiving declaration's maximum.
  - Exact-capacity `VAR_IN_OUT` avoids an implicit round trip that can change a caller even when the called POU performs no write.

## 2026-07-05 - Subrange runtime write enforcement

- Area: ST subrange data types and runtime writes
- IEC context: IEC 61131-3 Ed.3 §6.4.4.4.1 defines subrange values by inclusive lower/upper limits and treats values outside that range as errors; Table 11 defines user-defined subrange declarations.
- Decision:
  - Constant initializers outside the inclusive declared bounds are rejected at
    compile time.
  - Runtime writes into subrange-typed storage are checked against the declared bounds.
  - Out-of-range execution-time assignment, function/FB parameter copy-in, dynamic-reference writes, HMI/control writes, and retain reload surface a deterministic runtime error.
  - The runtime must not silently clamp, wrap, or store an out-of-range value.
  - A rejected write is not committed: the target retains the exact value it held before the attempted write.
  - A source with the wrong base type is rejected at compile time with `E203`.
    Crafted bytecode presenting the wrong runtime value tag is rejected with
    `TypeMismatch` before storage; stable public error-code mapping remains a
    separate contract.
- Reason:
  - Existing specs already describe subrange range violations as errors; Phase 11 proof showed the runtime currently stores out-of-range values silently.
  - A visible runtime error is safer and more auditable than silently changing the value.

## 2026-02-25 - LD deterministic network traversal

- Area: Ladder Diagram (LD) execution ordering
- IEC context: LD network scan/evaluation ordering requirements and deterministic scan-cycle behavior (IEC 61131-3 Ed.3, LD semantics and standard FB timing tables)
- Decision:
  - Networks are evaluated strictly by ascending `network.order`.
  - Within a network, node processing order is deterministic (`x`, then `y`, then `id`) after topology expansion.
  - Parallel branch merge power is resolved as logical OR over incoming branch legs.
- Reason:
  - Guarantees reproducible behavior across platforms and editor layout variance.

## 2026-02-25 - Invalid topology handling

- Area: LD graph integrity
- IEC context: Implementations must provide deterministic behavior and reject malformed programs.
- Decision:
  - Ladder engine validates graph integrity before execution.
  - Unknown edge endpoints, disconnected nodes, and cycles are rejected with actionable diagnostics.
- Reason:
  - Prevents non-deterministic execution and hidden runtime faults.

## 2026-02-25 - Buffered write commit boundary

- Area: scan-cycle write semantics
- IEC context: PLC scan-cycle model (read -> evaluate -> update outputs)
- Decision:
  - Writes are buffered during network evaluation and committed only at end-of-scan.
- Reason:
  - Preserves scan determinism and avoids same-scan cascade side effects.

## 2026-02-27 - LD variable modeling policy (symbolic-first)

- Area: LD operand naming and address binding
- IEC context: IEC variable section/scope rules (Section 6.5.2.2, Figure 7) and LD operand usage in Section 8.2
- Decision:
  - The normative LD spec is symbolic-first: users SHOULD model LD with declared variables.
  - Direct addresses (`%I/%Q/%M`) remain allowed as an implementation form for hardware binding.
  - Local vs global separation follows IEC declarations (`VAR*`, `VAR_GLOBAL`, `VAR_EXTERNAL`) instead of editor-only heuristics.
- Reason:
  - Aligns LD with IEC declaration semantics and improves portability across targets.

## 2026-02-27 - Edition baseline and compatibility statement for LD

- Area: LD standard baseline
- IEC context: IEC 61131-3 Edition 3.0 (2013-02) superseding Edition 2 (2003)
- Decision:
  - Edition 3.0 is authoritative for LD conformance in this repository.
  - Core LD semantics are specified to remain compatible with common Edition 2 usage patterns.
- Reason:
  - Keeps a single normative baseline while avoiding unnecessary migration breakage for existing LD logic.

## 2026-04-11 - Mixed-width infix bitwise result typing

- Area: ST expression typing
- IEC context: IEC 61131-3 Ed.3 expression semantics allow `AND`/`OR`/`XOR`/`NOT` on bit strings, but mixed-width infix result typing is not stated concretely enough for interoperable implementation behavior.
- Decision:
  - Infix `AND`, `&`, `OR`, and `XOR` on `ANY_BIT` operands widen to the wider operand type.
  - Infix `NOT` preserves the operand type.
- Reason:
  - This keeps infix operators aligned with the existing standard-function `ANY_BIT` behavior and avoids divergent typing between `a AND b` and `AND(a, b)`.

## 2026-04-11 - `VAR_GLOBAL` inside `PROGRAM`

- Area: ST variable declarations
- IEC context: IEC 61131-3 Ed.3 Table 47 feature 8a and §6.5.2.2 permit `VAR_GLOBAL ... END_VAR` within a `PROGRAM`, and allow `VAR_EXTERNAL` to match the associated `program`, `configuration`, or `resource`.
- Decision:
  - truST accepts `VAR_GLOBAL ... END_VAR` inside `PROGRAM`.
  - `VAR_EXTERNAL` may link to a `PROGRAM`-scoped `VAR_GLOBAL`.
- Reason:
  - This aligns the implementation with the checked-in IEC interpretation used by the project and removes an internal contradiction between HIR/runtime behavior and the older variables spec table.

## 2026-04-11 - Duplicate global-name policy across file/program/configuration scope

- Area: ST global-variable naming
- IEC context: IEC 61131-3 Ed.3 does not give a repository-level collision policy for vendor-style file-scope GVLs mixed with `PROGRAM`/`CONFIGURATION` globals.
- Decision:
  - truST rejects duplicate global names within the same effective namespace, even when they are declared in different global-host scopes such as file-scope GVL, `PROGRAM`, `CONFIGURATION`, or `RESOURCE`.
- Reason:
  - Bare global access and `VAR_EXTERNAL` linkage become ambiguous if multiple globals with the same effective name coexist.
  - Rejecting duplicates matches the chosen vendor-parity direction more closely than silently preferring one declaration.

## 2026-07-05 - Unqualified variable warm-restart policy

- Area: runtime restart and retain behavior
- IEC context: IEC 61131-3 Ed.3 §6.5.6 / Figure 9 specifies warm-restart behavior for `RETAIN` and `NON_RETAIN`, while unqualified warm-restart initialization is implementation-specific.
- Decision:
  - truST preserves only `RETAIN` and `PERSISTENT` values across warm restart.
  - `NON_RETAIN` and unqualified variables are initialized from their declared/default initial values on warm restart.
  - Cold restart initializes `RETAIN`, `PERSISTENT`, `NON_RETAIN`, and unqualified variables.
- Reason:
  - This keeps warm-restart retention explicit in source and prevents unqualified variables from accidentally behaving as retained state.
