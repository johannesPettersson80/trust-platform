# IEC Deviations Log

Authoritative location:
- This tracked file is the repository source of truth for IEC deviations.
- Do not point tracked docs or code comments at legacy internal IEC deviation-log paths.

This file tracks known, intentional departures from normative IEC 61131-3
requirements. Every entry must cite the exact IEC section/table, state the
normative requirement, state truST's behavior, and explain the concrete
conflict, omission, or relaxation.

IEC-silent, out-of-scope, implementation-specific, platform/runtime, and
truST-only API behavior belongs in the relevant product specification, not in
this log. IEC-permitted interpretation choices belong in
`docs/IEC_DECISIONS.md`. Existing entries are not classification precedent;
re-check them against this rule when touched.

## 2026-10-29 - CODESYS language profile: public function block members

- ID: DEV-028
- Area: Access to a function block's members under the CODESYS language
  profile (`vendor_profile` `codesys`, `beckhoff` or `twincat`)
- Normative IEC requirement: IEC 61131-3 Ed.3 section 6.6.3.2 item 11:
  "Variables of the VAR section (static) may be declared PUBLIC or PRIVATE.
  The access specifier PRIVATE is default." Section 6.6.5.9 makes PROTECTED
  the default of a method.
- truST behavior and conflict:
  - The default language rules keep a function block's `VAR` and `VAR_STAT`
    members, methods and properties without an access specifier PROTECTED:
    reachable inside the block and the blocks derived from it, not through an
    instance from outside.
  - Under the CODESYS profile they are PUBLIC: `fbInst.member` reads and
    writes them from any POU, as CODESYS does. An explicit specifier is kept;
    `VAR_IN_OUT`, `VAR_TEMP` and `VAR_EXTERNAL` members stay PROTECTED.
  - Making the default public contradicts the PRIVATE default of item 11.
- Impact:
  - A project with the CODESYS profile may reach a block's internal state from
    outside it; the same source fails on the default rules and on a strictly
    conforming IEC 61131-3 implementation.
- Mitigation:
  - The profile applies only to projects whose `vendor_profile` is `codesys`,
    `beckhoff` or `twincat` (specification 14); the default rules are unchanged.
  - Portable source declares the members it shares `PUBLIC`.

## 2026-10-29 - CODESYS language profile: a function block assigns its own inputs

- ID: DEV-029
- Area: Assignment to `VAR_INPUT` under the CODESYS language profile
  (`vendor_profile` `codesys`, `beckhoff` or `twincat`)
- Normative IEC requirement: IEC 61131-3 Ed.3 section 6.6.3.4.2, Figure 13
  rule 2a: an input assignment inside the function block is "Not allowed"
  (only the caller assigns an input, in the call or separately before it).
- truST behavior and conflict:
  - The default language rules reject an assignment to a function block's or
    program's own input in its body; an assignment from outside the instance
    (`fbInst.input := value`, rule 2b) is accepted under both rules.
  - Under the CODESYS profile the body of a function block or program may
    assign its own inputs between calls, as CODESYS does. A function's or
    method's parameter stays call-local, and a pointer or reference input stays
    read-only (specification 02 section 4.4).
  - Assigning an input inside the block contradicts rule 2a.
- Impact:
  - Such a block overwrites what its caller passed until the next call; the
    same source fails on the default rules.
- Mitigation:
  - The profile applies only to CODESYS, Beckhoff and TwinCAT projects
    (specification 14). Portable source copies the input to a `VAR` member.

## 2026-10-29 - CODESYS language profile: a program reached by its name

- ID: DEV-030
- Area: Access to a program's variables and actions, and program calls, under
  the CODESYS language profile (`vendor_profile` `codesys`, `beckhoff` or `twincat`)
- Normative IEC requirement: IEC 61131-3 Ed.3 section 6.6.4 items 2 and 3:
  "Programs can only be instantiated within resources", and a program's
  variables are reached from outside through access paths (`VAR_ACCESS`), not
  by the program's name.
- truST behavior and conflict:
  - The default language rules reject `ProgramName.member`,
    `ProgramName.action()` and `ProgramName();` in another POU.
  - Under the CODESYS profile a program is one instance named by its type, as
    in CODESYS: `ProgramName.member` reads and writes its variables (also
    through its function block instances), `ProgramName.action()` runs its
    action and `ProgramName();` runs its body. A program name alone in value
    position stays an error.
  - Reaching a program by its name and calling it contradicts items 2 and 3.
- Impact:
  - The same source fails on the default rules and on a strictly conforming
    IEC 61131-3 implementation.
- Mitigation:
  - The profile applies only to CODESYS, Beckhoff and TwinCAT projects
    (specification 14). Portable source shares state through global variables
    or `VAR_ACCESS` paths.

## 2026-10-29 - CODESYS language profile: references dereferenced without `^`

- ID: DEV-031
- Area: Use of a reference's value under the CODESYS language profile
  (`vendor_profile` `codesys`, `beckhoff` or `twincat`)
- Normative IEC requirement: IEC 61131-3 Ed.3 section 6.4.4.10.3:
  "Dereferencing shall be done explicitly. A reference can be dereferenced
  using a succeeding '^' (caret)."
- truST behavior and conflict:
  - The default language rules dereference only with `^`: `r.x`, `r[i]`,
    `v := r`, `r := 3`, `r + 1` and passing `r` where its value is expected are
    errors for a reference `r`.
  - Under the CODESYS profile a reference (`REF_TO T` or its spelling
    `REFERENCE TO T`) in value position denotes the value it references, as
    CODESYS's `REFERENCE TO` does: member access, indexing, arithmetic,
    assignment and arguments use the referenced value. Assigning a reference to
    a reference still rebinds it, `REF=` rebinds explicitly, and a `POINTER TO`
    still needs `^`.
  - Implicit dereferencing contradicts the explicit dereferencing required.
- Impact:
  - The same source fails on the default rules and on a strictly conforming
    IEC 61131-3 implementation; `r := 3` writes the referenced variable instead
    of failing.
- Mitigation:
  - The profile applies only to CODESYS, Beckhoff and TwinCAT projects
    (specification 14). Portable source writes `r^`.

## 2026-10-29 - CODESYS language profile: a named actual without a value

- ID: DEV-032
- Area: Formal call arguments under the CODESYS language profile (`vendor_profile` `codesys`, `beckhoff` or `twincat`)
- Normative IEC requirement: IEC 61131-3 Ed.3 section 6.6.1.4.2: a formal
  call assigns "values to input and in-out variables using the ':=' operator"
  and "the values of output variables to variables using the '=>' operator".
- truST behavior and conflict:
  - The default language rules report a named actual without a value
    (`Value := )`, `Done =>,`) as "expected expression" at the delimiter after
    the operator.
  - Under the CODESYS profile such an input is left unbound and takes its
    initial value, and such an output is discarded, as CODESYS reads it. `?=`
    still needs its target.
  - An assignment without a value or variable contradicts the formal call's
    assignments.
- Impact:
  - The same source fails on the default rules and on a strictly conforming
    IEC 61131-3 implementation.
- Mitigation:
  - The profile applies only to CODESYS, Beckhoff and TwinCAT projects
    (specification 14). Portable source omits the actual.

## 2026-10-29 - CODESYS language profile: ON, NULL and SET as names

- ID: DEV-034
- Area: Identifiers under the CODESYS language profile (`vendor_profile` `codesys`, `beckhoff` or `twincat`)
- Normative IEC requirement: IEC 61131-3 Ed.3 section 6.1.3 and Table 1 list
  `NULL`, `ON` and `SET` as keywords, which "shall not be used for any other
  purpose, for example, variable names".
- truST behavior and conflict:
  - The default language rules reject `ON`, `NULL` and `SET` as names.
  - Under the CODESYS profile they are accepted as names of variables,
    structure and union members, parameters and enumeration values, as
    CODESYS and real libraries use them (OSCAT `TIMER_EXT.ON`,
    `FF_DRE.SET`, WAGO `eChar.Null`); `ON` in value position is the variable.
    The keyword positions keep their meaning: `NULL` in value position is the
    null reference, and `ON` separates `RESOURCE ... ON <name>`.
  - Using the keywords as names contradicts section 6.1.3.
- Impact:
  - The same source fails on the default rules and on a strictly conforming
    IEC 61131-3 implementation.
- Mitigation:
  - The profile applies only to CODESYS, Beckhoff and TwinCAT projects
    (specification 14). POU and method names and the other keywords stay
    rejected.

## 2026-10-23 - Warm restart of a RETAIN global's instances: instance initializers of reset variables

- ID: DEV-027
- Area: Warm restart of the running runtime (`RETAIN` globals that hold
  function-block or class instances)
- Normative IEC requirement: IEC 61131-3 Ed.3 §6.5.6.1 requires a variable in a
  `NON_RETAIN` section to be initialized on a warm restart, and §6.5.6.2
  initializes a variable with its variable-specific user-defined value first
  (for an instance member, the value an instance initializer gives it), then the
  type-specific value, then the type's default.
- truST behavior and conflict:
  - A warm restart of the running runtime carries the instances of a `RETAIN`
    global over whole, so references and aliases to them stay valid
    (specification 10 §4.8).
  - Each variable in them that is not kept (its type declares it
    `NON_RETAIN`, or it belongs to a nested instance declared `NON_RETAIN`) runs
    its declaration's initializer in its type again. An instance initializer
    that set such a variable (`G : FB := (Speed := 50.0)`) is not applied
    again, so the variable takes its type's initial value.
  - A restart from the retain store (power loss, reload) constructs every
    instance anew and applies all initializers, so it follows §6.5.6.2 exactly.
- Impact:
  - Only a variable that is not kept and that an instance initializer sets
    differs, and only on a warm restart of the running runtime.
- Mitigation:
  - Give such a setting its value in the function block type, or declare it
    `RETAIN` so it keeps its value.

## 2026-10-21 - CODESYS language profile: declaration sizes from initialized variables

- ID: DEV-033
- Area: `STRING`/`WSTRING` declared lengths and array dimension bounds under
  the CODESYS language profile (`vendor_profile` `codesys`, `beckhoff` or `twincat`)
- Normative IEC requirement: IEC 61131-3 Ed.3 section 6.4.4.5.1: "The limits
  of the index subrange(s) shall be literals or constant expressions"; section
  6.4.2.2 and the Annex A grammar give a string's maximum length as an unsigned
  integer. A plain (non-`CONSTANT`) variable is neither.
- truST behavior and conflict:
  - The default language rules accept only literals and constant expressions
    as declaration sizes; a size that names a plain variable is an error.
  - Under the CODESYS profile a declaration size may name a plain variable
    of any file of the project whose own initializer is a literal or a
    constant expression, also qualified (`STRING(GVLMqtt.uiLenMsgState)`)
    and in bound expressions (`ARRAY[0..(GVLMqtt.udiLenMsgBuffer - 1)]`): the
    declared initial value fixes the size when the program is built, as
    CODESYS does.
  - Taking a size from a variable contradicts the constant sizes required.
- Impact:
  - The size is fixed from the declared initial value; writes to the variable
    while the program runs never resize anything (CODESYS sizes the same way).
- Mitigation:
  - The profile applies only to CODESYS, Beckhoff and TwinCAT projects
    (specification 14), and only to the two declaration-size contexts. `TEMP`
    and interface-direction sections are excluded, chains through other
    mutable variables keep the standing diagnostics, and every other
    constant-expression context keeps the strict rule.

## 2026-10-15 - MOD by zero faults

- ID: DEV-023
- Area: Integer arithmetic (`MOD` function and operator)
- Normative IEC requirement: IEC 61131-3 Ed.3 Table 29 defines `MOD` as the
  result of the ST statements `IF (IN2 = 0) THEN OUT := 0; ELSE OUT := IN1 -
  (IN1/IN2)*IN2; END_IF`, so `MOD` by zero yields `0`.
- truST behavior and conflict:
  - `MOD` by zero raises `RuntimeError::ModuloByZero` (stable code
    `runtime_modulo_by_zero`) and aborts the containing assignment before
    storage, as integer division by zero raises `DivisionByZero`
    (specification 10 §6.4, specification 07 §3.1).
  - For a nonzero divisor truST computes exactly the Table 29 formula.
  - Faulting replaces the normative zero result for a zero divisor.
- Impact:
  - A program that relies on `MOD(x, 0) = 0` faults on truST instead of
    continuing with `0`.
- Mitigation:
  - A silent `0` hides a logic error in a control program, so truST reports it
    as it reports integer division by zero. Portable source that needs the
    Table 29 result guards the divisor: `IF d = 0 THEN r := 0; ELSE r := x MOD
    d; END_IF`.

## 2026-10-07 - CODESYS conversion profile: rounding of ties

- ID: DEV-024
- Area: Type conversion functions under the CODESYS conversion profile
  (`vendor_profile` `codesys`, `beckhoff` or `twincat`)
- Normative IEC requirement: IEC 61131-3 Ed.3 section 6.6.2.5.2, Table 22
  footnote: "Conversion from type REAL or LREAL to SINT, INT, DINT or LINT
  shall round according to the convention of IEC 60559, according to which,
  if the two nearest integers are equally near, the result shall be the
  nearest even integer" (`REAL_TO_INT(2.5)` is 2, `REAL_TO_INT(-1.5)` is -2).
  Section 6.6.2.5.3 and Table 23 apply the same rounding to every conversion
  from a floating point number to an integer type.
- truST behavior and conflict:
  - The default conversion rules round ties to even, as required.
  - Under the CODESYS conversion profile, selected by `vendor_profile`
    `codesys`, `beckhoff` or `twincat` in the project manifest or by a
    `trust-harness` request, the rounding conversions from `REAL`/`LREAL` to
    integer and bit-string types round ties away from zero:
    `REAL_TO_INT(2.5)` is 3 and `REAL_TO_INT(-2.5)` is -3.
  - Rounding ties away from zero contradicts the required IEC 60559
    ties-to-even rounding.
- Impact:
  - A project using the CODESYS profile (a CODESYS, Beckhoff or TwinCAT
    project) computes CODESYS results for ties; the same source gives
    different results under the default rules and on a strictly conforming
    IEC 61131-3 implementation.
- Mitigation:
  - The profile applies only to projects whose `vendor_profile` is
    `codesys`, `beckhoff` or `twincat`, and to conversion functions only; the
    default rules are unchanged.
  - `TRUNC_*` truncation is the same under both rules; portable source can
    state the rounding it needs explicitly.
  - Specified in `docs/specs/07-standard-functions.md` section 2.10.

## 2026-10-07 - CODESYS conversion profile: numeric REAL and DWORD/LWORD conversions

- ID: DEV-025
- Area: Type conversion functions under the CODESYS conversion profile
  (`vendor_profile` `codesys`, `beckhoff` or `twincat`)
- Normative IEC requirement: IEC 61131-3 Ed.3 section 6.6.2.5.5, rule 1 ("Data
  type conversion is done as binary transfer") and Table 25 rows 1, 2, 43 and
  44: `LWORD_TO_LREAL`, `DWORD_TO_REAL`, `LREAL_TO_LWORD` and `REAL_TO_DWORD`
  are binary transfers of the bit pattern.
- truST behavior and conflict:
  - The default conversion rules transfer the bit pattern, as required.
  - Under the CODESYS conversion profile these four functions convert the
    numeric value instead: `REAL_TO_DWORD(3.7)` is `DWORD#4` and
    `DWORD_TO_REAL(DWORD#5)` is `5.0`, where the binary transfers give
    `DWORD#16#406CCCCD` and about `7.0E-45`.
  - Converting the value instead of transferring the bit pattern contradicts
    the binary transfer that rule 1 and Table 25 require.
- Impact:
  - Code that moves floating point bit patterns through these functions, for
    example to pack a `REAL` into a fieldbus word, gets other values under the
    CODESYS profile.
- Mitigation:
  - The profile applies only to projects whose `vendor_profile` is
    `codesys`, `beckhoff` or `twincat`; the default rules keep the binary
    transfers.
  - Specified in `docs/specs/07-standard-functions.md` section 2.10.

## 2026-10-07 - CODESYS conversion profile: bit string to BOOL

- ID: DEV-026
- Area: Type conversion functions under the CODESYS conversion profile
  (`vendor_profile` `codesys`, `beckhoff` or `twincat`)
- Normative IEC requirement: IEC 61131-3 Ed.3 section 6.6.2.5.4 and Table 24
  rows 4, 8, 12 and 16: `LWORD_TO_BOOL`, `DWORD_TO_BOOL`, `WORD_TO_BOOL` and
  `BYTE_TO_BOOL` are a "binary transfer of the rightmost bit into the target".
- truST behavior and conflict:
  - The default conversion rules transfer the rightmost bit, as required:
    `BYTE_TO_BOOL(16#80)` is `FALSE`.
  - Under the CODESYS conversion profile these functions, and the overloaded
    `TO_BOOL` of a bit string, return `TRUE` when the value is not zero:
    `BYTE_TO_BOOL(16#80)` is `TRUE`.
  - Testing the whole value instead of transferring the rightmost bit
    contradicts Table 24.
- Impact:
  - A bit string whose rightmost bit is 0 but which is not zero converts to
    `TRUE` under the CODESYS profile and to `FALSE` under the default rules and
    on a strictly conforming IEC 61131-3 implementation.
- Mitigation:
  - The profile applies only to projects whose `vendor_profile` is
    `codesys`, `beckhoff` or `twincat`; the default rules keep the IEC
    conversion.
  - Portable source can compare with zero (`b <> 0`) or select a bit
    explicitly (`b.0`).
  - Specified in `docs/specs/07-standard-functions.md` section 2.10.

## 2026-07-30 - Value-bearing RETURN statement

- ID: DEV-022
- Area: Structured Text subprogram control statements
- Normative IEC requirement: IEC 61131-3 Ed.3 section 7.3.3.2.4 and the
  `Subprog_Ctrl_Stmt` production in Annex A define the textual `RETURN`
  statement as the keyword `RETURN` without an expression. Function results
  are assigned through the function-name result variable.
- truST behavior and conflict:
  - truST additionally accepts `RETURN <expression>` in a function or
    value-returning method.
  - The expression is checked against the declared result type, supplies that
    result, and exits the POU immediately.
  - Accepting an expression after `RETURN` extends both the normative grammar
    and the normative function-result assignment model.
- Impact:
  - Source using value-bearing `RETURN` is accepted by truST but is not
    portable to a strictly conforming IEC 61131-3 implementation.
- Mitigation:
  - Portable source should assign the function-name result variable and then
    use bare `RETURN`.
  - truST rejects a value-bearing `RETURN` in a POU without a declared return
    type and type-checks the expression before execution.

## 2026-07-27 - Integer-base exponentiation

- ID: DEV-021
- Area: Structured Text arithmetic expressions
- Normative IEC requirement: IEC 61131-3 Ed.3 section 6.6.2.5.8 and Table 29
  define `EXPT`/`**` with `IN1` of generic type `ANY_REAL`, `IN2` of generic
  type `ANY_NUM`, and an output of the same type as `IN1`.
- truST behavior and conflict:
  - truST additionally accepts the reviewed integer expression
    `INT#2 ** INT#3` and evaluates it to `INT#8`.
  - Accepting an integer `IN1` extends the normative input domain beyond
    `ANY_REAL`.
- Impact:
  - Source that uses this reviewed integer-base expression is accepted by
    truST but is not portable to a strictly conforming IEC 61131-3
    implementation.
- Mitigation:
  - Portable source should convert the base to `REAL` or `LREAL` before
    exponentiation.
  - The expression and runtime specifications label that reviewed integer
    form as a truST extension. Other integer operand combinations and
    boundaries require their own specification behavior and focused proof.

## 2026-07-26 - `REF(...)` rejects CONSTANT-qualified variables

- ID: DEV-020
- Area: Structured Text reference operations
- Normative IEC requirement: IEC 61131-3 Ed.3 section 6.4.4.10.3 defines
  `REF(...)` for a variable or instance and forbids temporary variables,
  including `VAR_TEMP` and variables inside functions. Section 6.5.4 defines
  CONSTANT-qualified declarations as variables and does not exclude them from
  `REF(...)`.
- truST behavior and conflict:
  - `trust-hir` rejects `REF(c)` whenever `c` is CONSTANT-qualified.
  - This adds a restriction not present in the normative reference-operation
    rule.
- Impact:
  - IEC source may read a constant through a reference, but the equivalent
    source is rejected by truST.
- Mitigation:
  - Pass the constant value directly or copy it into non-CONSTANT storage
    before taking a reference.
  - This deviation closes when the type system can represent a read-only
    reference target without permitting mutation through the reference.

## 2026-07-26 - Non-NULL reference defaults on type and aggregate members

- ID: DEV-019
- Area: Structured Text reference initialization
- Normative IEC requirement: IEC 61131-3 Ed.3 section 6.4.4.10.2 permits a
  reference to be initialized with `NULL` or with `REF(...)` naming an already
  declared variable, function-block instance, or class instance.
- truST behavior and conflict:
  - truST accepts `NULL` as a reference type/member default.
  - During user-defined type and aggregate-member default collection,
    `trust-hir` rejects every non-`NULL` reference initializer, including
    `REF(target)`, with `InvalidOperation`.
  - Rejecting the standard-permitted `REF(target)` form omits part of the
    normative reference-initialization model.
- Impact:
  - IEC source that embeds an address-bound reference default in a type or
    aggregate member is rejected by truST.
- Mitigation:
  - Use a `NULL` default and assign the reference after an instance has been
    created.
  - This deviation closes only when HIR retains and validates safe
    address-bound reference defaults without weakening lifetime checks.

## 2026-07-26 - Mixed positional-prefix and formal-suffix calls

- ID: DEV-018
- Area: Structured Text textual call parameter binding
- Normative IEC requirement: IEC 61131-3 Ed.3 section 6.6.1.4.2 defines a
  textual parameter list as either:
  - a formal parameter list consisting of `:=` input/in-out assignments and
    `=>` output assignments, whose ordering is insignificant; or
  - a non-formal parameter list containing exactly the parameters of the
    definition in declaration order, excluding `EN` and `ENO`.
- truST behavior and conflict:
  - truST additionally accepts a hybrid call when all positional arguments
    precede all formal assignments, for example `Add(1, b := 2)`.
  - Such a list is neither the IEC formal form nor the IEC non-formal form, so
    accepting it extends the normative textual-call grammar and binding rules.
  - A formal assignment followed by a positional argument remains rejected.
- Impact:
  - Source using this convenience syntax is accepted by truST but is not
    portable to strictly conforming IEC 61131-3 implementations.
- Mitigation:
  - Portable source should use either a wholly formal or wholly non-formal
    parameter list.
  - The semantic specification labels the hybrid form as a truST extension,
    and the compiler still enforces the positional-prefix ordering.

## 2026-04-20 - Runtime STRING / WSTRING element semantics

- ID: DEV-017
- Area: Runtime string indexing, character access, and standard string operations
- Normative IEC requirement: IEC 61131-3 Ed.3 section 6.3.3 and Table 10 define
  `STRING` as a single-byte character string and `WSTRING` as a double-byte
  character string using the ISO/IEC 10646 character model.
- truST behavior and conflict:
  - truST stores both `STRING` and `WSTRING` as Rust `String` values.
  - Capacity counts storage units: `STRING[n]` holds at most n bytes of UTF-8
    and `WSTRING[n]` at most n UTF-16 code units, and `SIZEOF(STRING[n])` and
    `SIZEOF(WSTRING[n])` are `n` and `2n` (`docs/IEC_DECISIONS.md` 2026-10-17).
  - Element operations count characters: indexed access and `LEN`, `LEFT`,
    `RIGHT`, `MID`, `INSERT`, `DELETE`, `REPLACE`, and `FIND` operate on
    Unicode scalar elements rather than raw single bytes or UTF-16 code units.
  - Materializing a `STRING` element as `CHAR` still requires the selected
    scalar value to fit in `u8`; materializing a `WSTRING` element as `WCHAR`
    requires it to fit in `u16`.
  - This conflicts with the standard's single-byte and double-byte element
    model for non-ASCII text: a `STRING` character outside ASCII is one element
    but two to four bytes of capacity, and a `WSTRING` character outside the
    Basic Multilingual Plane is one element but two code units of capacity.
- Impact:
  - Non-ASCII text behaves consistently across VM reference indexing and the
    shipped string library, but it is not raw IEC byte/code-unit indexing.
  - Such a text holds fewer characters than its declared length.
- Mitigation:
  - The runtime and specifications use one explicit element model and one
    explicit capacity unit end to end.
  - A raw-byte/raw-code-unit rewrite is a separate compatibility change because
    it requires changing the underlying runtime value representation.

## 2026-04-20 - Simplified `VAR_ACCESS` / `VAR_CONFIG` path validation

- ID: DEV-003
- Area: Structured Text access-path and configuration-variable validation
- Normative IEC requirement: IEC 61131-3 Ed.3 section 6.5.2.2 and Tables 13-16
  define access paths and configuration-variable targets across configurations,
  resources, program instances, globals, function blocks, and directly
  represented variables, including matching target types.
- truST behavior and conflict:
  - truST validates the accepted access-path shape and target-type compatibility.
  - `trust-hir` does not completely model or statically validate every
    cross-resource and cross-program-instance mapping required by those forms.
  - The missing topology validation is an omission from the normative access
    path model, not an IEC-silent product choice.
- Impact:
  - Full IEC communication-service topology is not statically proven in the
    language layer.
- Mitigation:
  - The supported subset is documented in the variables specification and
    enforced consistently for accepted forms.

## 2026-04-20 - Assignment-attempt compatibility is runtime-oriented

- ID: DEV-006
- Area: `?=` assignment-attempt semantics
- Normative IEC requirement: IEC 61131-3 Ed.3 section 6.6.6.7 and Table 52
  define `?=` as a compatibility check for interface implementation and safe
  class/function-block reference downcasts, returning a valid reference only
  for a compatible instance and `NULL` otherwise.
- truST behavior and conflict:
  - truST accepts `?=` for typed reference-style assignment attempts.
  - `trust-hir` does not fully enforce inheritance/interface compatibility for
    every source/target pair during static analysis; some incompatibility is
    deferred to runtime `NULL` behavior.
  - Deferring compatibility checks that can be derived statically is an
    incomplete implementation of the normative assignment-attempt type model.
- Impact:
  - Some compatibility failures are observed at runtime instead of being
    diagnosed statically.
- Mitigation:
  - The operator remains typed, null-producing behavior is documented, and
    callers must test the result before dereference.

## 2026-04-20 - ASCII-only identifier validation

- ID: DEV-013
- Area: Lexical identifier support
- Normative IEC requirement: IEC 61131-3 Ed.3 sections 6.1.1-6.1.2 and Tables
  1-2 define textual characters using ISO/IEC 10646 and identifiers using
  letters, digits, and permitted underscores.
- truST behavior and conflict:
  - truST accepts ASCII letters, digits, and `_` in identifiers and uses ASCII
    case folding.
  - Non-ASCII ISO/IEC 10646 letters are rejected even when they otherwise form
    a valid IEC identifier.
  - Rejecting those standard-defined characters is a supported-character-set
    omission.
- Impact:
  - IEC source using non-ASCII identifiers does not compile in truST.
- Mitigation:
  - The lexer specification and diagnostics state the current ASCII subset
    explicitly.

## 2026-04-11 - Optional `VAR_EXTERNAL` for vendor-parity global access

- Area: Structured Text global-variable access
- Normative IEC requirement: IEC 61131-3 Ed.3 sections 6.5.2.1-6.5.2.2 and
  Figure 8 require a global variable to be redeclared in the consuming POU with
  a matching `VAR_EXTERNAL` declaration before it is accessible there.
- truST behavior and conflict:
  - truST accepts direct global access without a matching `VAR_EXTERNAL`
    declaration.
  - `VAR_EXTERNAL` remains supported and type-checked when authors declare it.
  - This relaxed path applies to configuration/resource globals, file-scope
    vendor GVLs, and qualified namespaced GVL access.
  - Permitting direct access relaxes the normative explicit-linkage requirement.
- Impact:
  - CODESYS/TwinCAT-style source without injected `VAR_EXTERNAL` blocks compiles
    directly in truST.
- Mitigation:
  - Undefined names still diagnose as errors, and strict IEC export/adapter
    flows may synthesize `VAR_EXTERNAL` declarations.

## 2026-04-11 - Missing WHILE/REPEAT termination-guarantee analysis

- Area: Structured Text iteration safety
- Normative IEC requirement: IEC 61131-3 Ed.3 section 7.3.3.4.1 requires an
  error when a `WHILE` or `REPEAT` algorithm cannot guarantee satisfaction of
  its termination condition or execution of an `EXIT` statement.
- truST behavior and conflict:
  - truST parses, type-checks, and executes `WHILE` and `REPEAT`, but
    `trust-hir` does not prove that each loop can terminate.
  - A loop whose termination cannot be guaranteed is therefore not rejected
    solely for that normative error condition.
- Impact:
  - Source can compile even when the standard requires rejection because the
    loop can act as an unbounded wait or execution loop.
- Mitigation:
  - Authors must keep termination conditions locally evident and must not use
    iteration for inter-process synchronization; a future control-flow analysis
    must diagnose the missing guarantee before this deviation can close.

## 2026-02-27 - LD contact/coil symbol subset

- Area: Ladder Diagram symbol set exposed in schema v2/editor tooling
- Normative IEC requirement: IEC 61131-3 Ed.3 Tables 75-76 define the standard
  contact and coil symbol families, including transition-sensing forms.
- truST behavior and conflict:
  - The current schema v2/editor profile implements static contacts (`NO`,
    `NC`) and coil variants (`NORMAL`, `NEGATED`, `SET`, `RESET`).
  - Transition-sensing contact/coil variants from Tables 75-76 are not
    represented in the node schema.
  - The missing standard symbols are an omission from the IEC LD surface.
- Impact:
  - Users cannot model transition-sensing LD symbols directly in the current
    profile.
- Mitigation:
  - Unsupported symbol forms are rejected explicitly rather than silently
    coerced.

## 2026-02-25 - CTUD single-input profile in LD v2 node model

- Area: Ladder Diagram counter node representation
- Normative IEC requirement: IEC 61131-3 Ed.3 section 6.6.3.5.4 and Table 45
  define `CTUD` with separate `CU`, `CD`, `R`, `LD`, and `PV` inputs and `QU`,
  `QD`, and `CV` outputs.
- truST behavior and conflict:
  - The LD schema v2 `counter` node exposes one power input.
  - `CTUD` is executed as CU-driven rising-edge increment; separate `CD`, `QD`,
    `R`, and `LD` wiring is not represented by this node profile.
  - This omits required `CTUD` inputs, outputs, and state transitions from the
    IEC-defined function-block behavior available through this LD surface.
- Impact:
  - Full dual-input `CTUD` semantics are unavailable in the current LD node
    contract.
- Mitigation:
  - The subset is explicit in tests and the LD profile; a future schema version
    can add dedicated `CU`, `CD`, `R`, and `LD` pins plus `QU`/`QD` outputs.

## 2026-10-26 - Enum values as their base integer, and fully qualified member access

- Area: enumerated values in assignments, comparisons, call arguments, and
  CASE labels; dotted `Namespace.EnumType.Member` references of any depth.
- Normative IEC requirement: IEC 61131-3 Ed.3 treats an enumeration as a
  distinct type whose members are not integer literals (§6.4.4.2); assigning
  an enumerated value to an integer variable of the underlying base type is
  not an implicit conversion the standard defines. Qualified member access
  is spelled `EnumType#Value` (§6.4.4.2); the standard text was unavailable
  on the authoring machine and the clause numbers mirror the citations
  already used in `docs/specs/02-data-types.md`.
- truST behavior and conflict:
  - CODESYS treats an enum as its base type freely: an enumerated value
    converts to its declared integer base in assignments, comparisons, and
    call arguments, and members are reachable through fully qualified dotted
    paths (`WagoTypesLog.eSysLogPriority.LOG_ERR`). truST now follows: any
    enum source converts to its base (base-less enums are `INT`-based), and
    dotted member access accepts any namespace depth. Integer assignment
    INTO a base-less ("closed") enum and cross-enum assignment stay
    rejected, preserving the existing user-type contract.
- Impact: real CODESYS projects (the downstream tree carries ~154 such
  sites: `eSysLogPriority`, `eLedID`, `eArea` classes) stop reporting E202
  "cannot be used as a value" and IncompatibleAssignment for enum-to-base
  flows; the strict reading rejects the same code.
- Mitigation: the leniency is one-directional (enum → base only); integer
  into closed enums, cross-enum assignment, and unknown member references
  keep their diagnostics; generic `ANY` matching still does not treat an
  enum as an integer actual beyond this base conversion.

## 2026-10-28 - In-out string binding across declared capacities

- ID: DEV-035
- Area: `VAR_IN_OUT` parameter binding for `STRING[n]`/`WSTRING[n]` actuals.
- Normative IEC requirement: IEC 61131-3 Ed.3 requires the actual parameter
  of a `VAR_IN_OUT` formal to be a variable of the same data type
  (§6.6.2.2). String capacity is part of a string type, so binding a
  `STRING[255]` actual to a `STRING[1]` formal is not a same-type binding
  and the strict reading rejects it. The standard text was unavailable on
  the authoring machine; the clause number mirrors the citation style
  already used in `docs/specs/02-data-types.md`.
- truST behavior and conflict: truST requires only the same string family
  (`STRING` vs `WSTRING`) and accepts in-out binding across declared
  capacities, matching CODESYS. CODESYS accepts the vendor buffer idiom — a
  `STRING(1)` in-out formal (used by every `WagoAppString` function) whose
  real capacity arrives in a separate size argument
  (`StrAppend(sBuffer, SIZEOF(sBuffer), sAppendee)`) — so the
  capacity-strict reading rejects library calls that compile clean in the
  vendor toolchain.
- Impact: vendor-library call sites with in-out string formals (the
  downstream tree carries several hundred `WagoAppString` in-out and size
  call arguments) now pass the compile gate; the previous
  capacity-equality rule rejected the same code.
- Mitigation: nothing is lost at the boundary — the callee still sees the
  actual variable through the reference and owns the capacity contract
  through its formal capacity or size argument; the caller's variable keeps
  its own storage. Cross-family binding and non-string in-out type
  mismatches keep their diagnostics.

## 2026-10-28 - Vendor conversion extensions: enums, BOOL, durations, strings

- ID: DEV-036
- Area: typed `*_TO_*`/`TO_*` standard conversion functions.
- Normative IEC requirement: IEC 61131-3 Ed.3 Tables 22-27 enumerate the
  typed conversions; enumerated source types appear in no conversion table
  (`§6.4.4.2` treats an enumeration as a distinct type), `BOOL`/`TIME`
  string forms are not listed, duration-to-narrow-integer forms exceed the
  listed `TIME`/`DWORD` pair, and `TOD`/`LTOD` to `DWORD` is not listed.
  The standard text was unavailable on the authoring machine; the clause
  numbers mirror the citations already used in
  `docs/specs/07-standard-functions.md`.
- truST behavior and conflict: under a CODESYS `vendor_profile`
  (specification 14), truST accepts — matching the CODESYS / TwinCAT vendor
  toolchain — (a) `TO_STRING`/`*_TO_STRING` over an enumerated value
  (yields the variant name), over `BOOL` (`'TRUE'`/`'FALSE'`) and over
  `TIME`/`LTIME` (`T#`/`LT#` duration text); (b) `TIME_TO_<int>` /
  `LTIME_TO_<int>` (ms / ns magnitude truncated into the destination,
  negative durations overflow); (c) `TOD_TO_DWORD` / `LTOD_TO_DWORD`
  (ms-of-day magnitude). The strict table reading rejects vendor-project
  call sites that compile clean in the vendor toolchain (the downstream
  tree carries ~130 such sites: `TO_STRING(eArea)` families, boot-report
  `TIME_TO_DINT` chains, `TOD_TO_DWORD` day-phase math).
- Impact: those call sites pass the compile gate; the runtime conversion
  dispatch (`trust-runtime-core`) implements the same entries so executed
  conversions produce the documented values.
- Mitigation: the IEC dialect keeps the tables' reading and rejects all
  of (a) to (c); `TO_STRING` over a `STRING[n]` value is an identity copy
  in every dialect, since a `STRING[n]` is a `STRING`. The extension set is
  deliberately bounded — time-of-day and date string forms, and
  REAL-to-bitstring conversions (upstream #127 territory), stay rejected
  under every profile; integer destinations wrap rather than fault on
  magnitude overflow, matching the vendor toolchain's silent narrowing
  documented in DEV-023's neighborhood.

## 2026-10-28 - Integer/bit-string narrowing bindings as warnings

- ID: DEV-037
- Area: implicit integer↔bit-string conversions at assignments and call
  arguments (`E203` "cannot assign", `E205` "expected '<int>' for parameter").
- Normative IEC requirement: IEC 61131-3 Ed.3 defines implicit assignments
  only where value and accuracy are preserved (`§6.6.1.6`); a signed to
  narrower-unsigned store (`SINT` into `BYTE`, `INT` into `BYTE`,
  `UDINT` into `DWORD`) is not an implicit conversion the standard defines.
  The standard text was unavailable on the authoring machine; the clause
  mirrors the citation style already used in `docs/specs/02-data-types.md`.
- truST behavior and conflict: a binding whose target and source are both
  integer/bit-string types is diagnosed as a **warning** with the same
  message and code, not an error — matching CODESYS, which compiles the
  store silently with a compiler warning. Diagnostics outside that type
  class (REAL targets, enums, strings, aggregates) stay errors. `SIZEOF`
  additionally yields its natural unsigned magnitude (`UDINT`) like the
  vendor toolchain, so vendor `udiSize` parameters bind directly.
- Impact: vendor-idiom call sites and configuration wiring tables (the
  downstream tree carries 57 narrowing assignments, 7 port arguments, and
  ~163 `SIZEOF`-into-`UDINT` sites) pass the compile gate while every
  narrowed store remains individually visible.
- Mitigation: the demotion is type-class-scoped, not blanket — mixed
  (non-integer) mismatches keep their errors, and each narrowed binding
  still produces one warning so recurrence is loud.

## 2026-10-28 - Mixed-family comparisons and bit-string arithmetic

- ID: DEV-038
- Area: comparison operators (`= <> < <= > >=`) over mixed integer
  families, and `+ - * /` over bit-string operands.
- Normative IEC requirement: IEC 61131-3 Ed.3 `§6.6.1.6` admits implicit
  operations only where value and accuracy are preserved; signed/unsigned
  pairs have no accuracy-preserving common type in the reviewed matrix,
  and bit-strings (`ANY_BIT` minus `BOOL`) are not `ANY_NUM` operands at
  all, so `DWORD - DWORD` and `INT < UINT` are not defined operations.
  The standard text was unavailable on the authoring machine; the clause
  mirrors the citation style already used in `docs/specs/05-expressions.md`.
- truST behavior and conflict: comparisons over any mix of
  signed/unsigned/bit-string integer operands are admitted and evaluate
  through one mathematical magnitude (i128 promotion at runtime); two
  bit-string operands participate in `+ - * /` as unsigned magnitudes
  whose result carries the wider operand's width and wraps like the
  vendor toolchain (`Div`/`Mod` keep the unsigned zero-divisor faults;
  `Pow` stays rejected). This matches CODESYS, which compiles the
  downstream tree's `UINT` vs division-result comparisons and
  `DWORD - DWORD` energy-meter arithmetic silently. The strict reading
  rejects both families (62 downstream sites).
- Impact: mixed-sign comparisons, integer/bit-string comparisons, and
  bit-string arithmetic pass the compile gate and execute correctly in
  the runtime (the VM promotes instead of faulting); MOD keeps its
  integer-only gate, mixed integer/bit-string *arithmetic* (`INT +
  UINT`, `WORD + INT`), and integer/REAL accuracy-loss combinations keep
  their diagnostics. The compile-time constant evaluator promotes true
  mixed signed×unsigned pairs the same way (exact i128 math, materialized
  LINT/ULINT) so array bounds and initializers like
  `ARRAY[0..SIZEOF(x)-1]` keep working with an unsigned SIZEOF; the VM
  execution path keeps its stricter contract.
- Mitigation: admission is silent (matches the vendor toolchain and the
  existing logical-operator precedent) but type-class-scoped — BOOL and
  non-scalar pairs, REAL accuracy-loss pairs, and mixed-family
  arithmetic keep their errors; the runtime promotion lives in the
  compare/arithmetic paths, leaving the value-helper rejection pins
  intact.

## 2026-10-29 - Pointer-target lenient bindings

- ID: DEV-039
- Area: pointer bindings at call arguments and plain assignments
  (`E205` "pointer target mismatch for parameter ...", `E203` "cannot
  assign") where the two pointer targets differ by a 1-byte scalar class
  (`CHAR` ↔ `BYTE`) or by the vendor byte-buffer idiom (`ADR(STRING[n])`
  against a `POINTER TO ARRAY[i..j] OF BYTE` formal).
- Normative IEC requirement: IEC 61131-3 Ed.3 defines no implicit
  conversion between pointer targets; `§6.6.1.6` admits implicit
  assignments only where value and accuracy are preserved, and strict
  reading requires identical pointee types. The standard text was
  unavailable on the authoring machine; the clause mirrors the citation
  style already used in `docs/specs/09-semantic-rules.md`.
- truST behavior and conflict: those two binding forms are diagnosed as
  a **warning** with the same code and message, not an error — matching
  CODESYS, which compiles them as implicit pointer conversions with a
  compiler warning. `ADR` keeps its pointee typing (`ADR(str[1])` stays
  `POINTER TO CHAR`), so the vendor `MemCopy`/`MemCopySecure`
  `POINTER TO BYTE` formals and the vendor `Fb_JSON_ParseAndModify`
  `pData : POINTER TO ARRAY[0..65535] OF BYTE` formal bind directly.
  Every other pointee mismatch stays an error (wider scalar classes,
  array bound widening, records, other families).
- Impact: the downstream tree's 3 remaining pointer sites (2×
  `MemCopy`/`MemCopySecure` string-offset byte-pointer bindings, 1×
  JSON-parser data binding) pass the compile gate while each binding
  stays individually visible. No runtime mirror is needed: no
  conversion dispatch is involved and the pointer value is copied
  verbatim, so only the compile contract changes.
- Mitigation: the demotion is form-scoped, not blanket — element/record
  boundary mismatches keep their errors, and each lenient binding still
  produces one warning so recurrence is loud. Function/method
  `VAR_OUT`/`VAR_IN_OUT` pointer arms and variable initializers are not
  wired (mirroring the narrowing record's scoping (DEV-037)) and keep the error until a
  downstream need appears.
