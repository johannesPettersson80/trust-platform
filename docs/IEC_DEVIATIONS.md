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
  - Indexed access and `LEN`, `LEFT`, `RIGHT`, `MID`, `INSERT`, `DELETE`,
    `REPLACE`, and `FIND` operate on Unicode scalar elements rather than raw
    single-byte or UTF-16 code units.
  - Materializing a `STRING` element as `CHAR` still requires the selected
    scalar value to fit in `u8`; materializing a `WSTRING` element as `WCHAR`
    requires it to fit in `u16`.
  - `SIZEOF(STRING[n])` and `SIZEOF(WSTRING[n])` remain storage-oriented (`n`
    and `2n` respectively), while runtime value operations use scalar-element
    counts.
  - This scalar-element model conflicts with the standard's byte/code-unit
    representation for non-ASCII text.
- Impact:
  - Non-ASCII text behaves consistently across VM reference indexing and the
    shipped string library, but it is not raw IEC byte/code-unit indexing.
- Mitigation:
  - The runtime and specifications use one explicit element model end to end.
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
