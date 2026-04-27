# IEC Deviations Log

Authoritative location:
- This tracked file is the repository source of truth for IEC deviations/extensions.
- Do not point tracked docs or code comments at legacy internal IEC deviation-log paths.

This file tracks known, intentional deviations/extensions from strict IEC 61131-3 behavior.

## 2026-04-27 - `UNION` aggregate initialization as truST extension

- Area: Structured Text data types and initializers
- IEC reference: IEC 61131-3 Ed.3 defines structured aggregate initialization
  for structures and FB declarations; `UNION` is not a core Ed.3 data-type
  construct in the same way as `STRUCT`.
- Deviation:
  - truST accepts `UNION` / `END_UNION` as a vendor-style data type and routes
    named union aggregate initializers through the same checked initializer
    machinery as structures.
- Impact:
  - Projects can use named union defaults in truST, but strict IEC exporters may
    need to lower or reject these declarations.
- Mitigation:
  - The behavior is explicit in parser/HIR/runtime tests and uses the same
    unknown/duplicate-field diagnostics as structure initialization.

## 2026-02-25 - CTUD single-input profile in LD v2 node model

- Area: Ladder Diagram counter node representation
- IEC reference: Counter FBs (IEC 61131-3 Ed.3, counter FB tables)
- Deviation:
  - LD schema v2 `counter` node currently exposes one power input.
  - `CTUD` is executed as CU-driven (rising-edge increment) in this profile; separate CD/QD wiring is not represented in node schema yet.
- Impact:
  - Full dual-input CTUD semantics are not available in current LD node contract.
- Mitigation:
  - Behavior is explicit in tests and docs; future schema extension can add dedicated CU/CD/R/LD pins.

## 2026-02-25 - TP/TOF ET exposure uses internal millisecond state

- Area: Ladder Diagram timer diagnostics/state exposure
- IEC reference: Timer FBs (IEC 61131-3 Ed.3, timer timing tables)
- Deviation:
  - Internal ET storage for TP/TOF diagnostics is represented as implementation-facing millisecond state in `%MW_LD_TIMER_<instance>_ET`.
- Impact:
  - Exposed ET key is engine-internal and not a normative IEC variable contract.
- Mitigation:
  - Runtime behavior (`Q` transitions) is tested; ET key is documented as implementation detail.

## 2026-02-25 - PLCopen LD interop subset

- Area: PLCopen LD import/export
- IEC reference: PLCopen XML graphical-body interchange profiles (vendor ecosystem variance)
- Deviation:
  - LD import/export currently targets the supported LD network-body subset used by `editors/vscode/src/ladder/plcopenLdInterop.ts`.
  - Unsupported graphical/vendor constructs are skipped with explicit diagnostics.
- Impact:
  - Not all vendor-specific graphical metadata/layout constructs are round-tripped.
- Mitigation:
  - Unsupported constructs are reported deterministically and covered by interop tests.

## 2026-02-27 - LD node operands use free-form string references

- Area: LD schema v2 operand contract
- IEC reference: Section 8.2 LD operands with declaration-driven typing and scope
- Deviation:
  - Node operands (`contact.variable`, `coil.variable`, compare/math operands) are represented as plain strings in schema v2.
  - Schema v2 does not yet provide explicit `symbolRef` vs `directAddress` discriminators.
- Impact:
  - Symbolic and direct-address references are syntactically mixed at profile level.
  - Additional validation is required to enforce strict declaration-driven addressing policies.
- Mitigation:
  - Normative spec defines symbolic-first policy; profile constraints are documented in `docs/specs/16-ladder-profile-trust.md`.

## 2026-02-27 - Runtime forcing path symbolic support closure

- Area: LD runtime I/O write/force operations
- IEC reference: Implementation-specific external I/O binding around LD execution model
- Previous deviation:
  - Runtime write/force/release operations were direct-address centric.
- Current status:
  - Closed in this stream. Runtime write/force/release now resolve declared symbols
    (including scoped references) in addition to direct `%IX*` addressing.
- Impact:
  - Symbol-first LD projects can be exercised from runtime controls without mandatory
    direct-address operands in node fields.

## 2026-02-27 - LD contact/coil symbol subset (Table 75/76)

- Area: Ladder Diagram symbol set exposed in schema v2/editor tooling
- IEC reference: IEC 61131-3 Ed.3 Table 75 (Contacts), Table 76 (Coils)
- Deviation:
  - Current schema v2/editor profile implements static contacts (`NO`, `NC`) and coil
    variants (`NORMAL`, `NEGATED`, `SET`, `RESET`).
  - Transition-sensing contact/coil variants from Table 75/76 are not yet represented in
    node schema.
- Impact:
  - Users cannot model transition-sensing LD symbols directly in the current profile.
- Mitigation:
  - Unsupported symbol forms are not silently coerced; they are rejected with explicit
    diagnostics.

## 2026-04-11 - Numeric hazard diagnostics for ST expressions

- Area: Structured Text diagnostics
- IEC reference: IEC 61131-3 Ed.3 defines expression evaluation and runtime fault behavior, but does not require warnings for floating-point equality or literal zero divisors.
- Deviation:
  - The type checker emits `W013` for `=`/`<>` comparisons when either operand is `REAL`/`LREAL`.
  - The type checker emits `W014` for `DIV`/`MOD` expressions whose right-hand operand is a literal zero.
- Impact:
  - truST reports additional proactive diagnostics beyond strict IEC conformance.
- Mitigation:
  - These are configurable tooling warnings under `[diagnostics].warn_numeric_hazards`, and severities can still be overridden per code.

## 2026-04-11 - File-scope `VAR_GLOBAL` as vendor-style GVL

- Area: Structured Text global-variable declarations
- IEC reference: IEC 61131-3 Ed.3 models globals through `PROGRAM`/`CONFIGURATION`/`RESOURCE`; vendor ecosystems such as CODESYS/TwinCAT also use standalone GVL source files.
- Deviation:
  - truST accepts top-level file-scope `VAR_GLOBAL ... END_VAR` blocks and treats them as global variable libraries (GVLs).
- Impact:
  - CODESYS/TwinCAT-style GVL source files compile directly in truST without wrapping them in a `CONFIGURATION`.
- Mitigation:
  - Duplicate global names in the same effective namespace are rejected.
  - Strict-IEC reshaping remains available as an adapter/export concern rather than a core-language requirement.

## 2026-04-11 - Namespaced vendor-style GVLs

- Area: Structured Text global-variable declarations
- IEC reference: `NAMESPACE`-scoped global-variable libraries are a vendor extension rather than an IEC Ed.3 construct.
- Deviation:
  - truST accepts `NAMESPACE ... VAR_GLOBAL ... END_NAMESPACE`.
  - Qualified access such as `GVL.shared` resolves against the namespaced global directly.
  - CODESYS `{attribute 'qualified_only'}` is not enforced as a semantic restriction in core truST yet.
- Impact:
  - Vendor-style namespaced GVLs compile directly in truST, including qualified reads/writes.
  - Projects imported from vendor tooling may still allow bare access where CODESYS would require qualification.
- Mitigation:
  - Strict import/export paths may keep wrapper or injected-`VAR_EXTERNAL` transforms for external consumers that need them, including PLCopen import calls that opt into `PlcopenImportGlobalVarMode::StrictIecAdapter`.
  - Documentation calls out the current `qualified_only` limitation explicitly.

## 2026-04-11 - Optional `VAR_EXTERNAL` for vendor-parity global access

- Area: Structured Text global-variable access
- IEC reference: IEC 61131-3 Ed.3 §6.5.2.2 / Figure 8 requires explicit `VAR_EXTERNAL` linkage for external global access.
- Deviation:
  - truST accepts direct global access without requiring a matching `VAR_EXTERNAL` declaration.
  - `VAR_EXTERNAL` remains supported and type-checked when authors choose to declare it.
  - This vendor-parity path applies to configuration/resource globals, file-scope GVLs, and qualified namespaced GVL access.
- Impact:
  - CODESYS/TwinCAT-style ST authored without injected `VAR_EXTERNAL` blocks compiles directly in truST.
- Mitigation:
  - Undefined bare names still diagnose as errors.
  - Strict-IEC export/adapter flows may still synthesize `VAR_EXTERNAL` declarations when targeting stricter consumers, including the optional PLCopen strict-adapter import mode.

## 2026-04-11 - `VAR_STAT` runtime semantics

- Area: Structured Text static variables
- IEC reference: `VAR_STAT` is a vendor extension and is not defined by IEC 61131-3 Ed.3.
- Deviation:
  - truST accepts `VAR_STAT` and gives it persistent storage semantics.
  - In `FUNCTION`, `VAR_STAT` persists across calls to that function definition.
  - In `METHOD`, `VAR_STAT` persists per enclosing instance and per method.
  - In `PROGRAM`, `FUNCTION_BLOCK`, and `CLASS`, `VAR_STAT` behaves as ordinary instance storage in the enclosing instance-bearing scope.
- Impact:
  - Vendor-authored code using `VAR_STAT` compiles and preserves static state without rewriting to IEC-only forms.
- Mitigation:
  - `VAR_STAT` remains an explicit vendor extension in docs/specs.
  - Strict-IEC export/adapter paths may rewrite or reject `VAR_STAT` for consumers that do not support it.

## 2026-04-17 - ADR / SIZEOF built-ins

- ID: DEV-016
- Area: ADR / SIZEOF built-ins
- IEC reference: Not specified in IEC 61131-3 Ed3; vendor extension.
- Deviation:
  - truST parses and executes `ADR(...)` and `SIZEOF(...)` as built-in expressions.
  - `SIZEOF(...)` returns a `DINT` byte count for the static storage representation of either an explicit `type_ref` or a storage operand (`var`, field/index access, dereference, `THIS.field`) without evaluating the operand.
  - Bare identifiers resolve variables before type names, matching common CODESYS shadowing behavior.
  - `SIZEOF(...)` is const-foldable when the static type is known.
  - `STRING[n]` reports `n`, `WSTRING[n]` reports `2n`.
  - `POINTER TO` / `REF_TO` operands report the platform pointer word size (`sizeof(usize)`), not truST's internal runtime handle layout.
  - Open arrays, unsized strings/WSTRINGs, and whole FB/class/interface instances are rejected.
- Impact:
  - Common vendor ST patterns like `ARRAY[0..SIZEOF(packet)-1] OF BYTE` compile and fold deterministically.
  - Pointer `SIZEOF` matches platform pointer width rather than leaking the size of truST's private runtime reference handle.
  - `ADR(str)` yields a typed pointer/reference-compatible handle for string dereference/indexing, not a raw byte-array reinterpretation of string storage.
- Mitigation:
  - This behavior is documented as a vendor extension rather than an IEC core feature.

## 2026-04-17 - Runtime STRING / WSTRING element semantics

- ID: DEV-017
- Area: Runtime string indexing, character access, and stdlib string element operations
- IEC reference:
  - IEC 61131-3 Ed.3 Table 10 defines `STRING` as single-byte and `WSTRING` as double-byte character strings.
  - IEC examples and common vendor practice use 1-based element access for `str[idx]`.
- Deviation:
  - truST stores `STRING` as UTF-8 text and `WSTRING` as Rust `String` text rather than raw single-byte / UCS-2 buffers.
  - Public runtime element access is 1-based for both `STRING` and `WSTRING`.
  - `STRING[idx]`, `LEN`, `LEFT`, `RIGHT`, `MID`, `INSERT`, `DELETE`, `REPLACE`, and `FIND` operate on Unicode scalar elements, not raw UTF-8 bytes.
  - `WSTRING[idx]` and the same stdlib helpers operate on the same Unicode scalar element model rather than raw 16-bit code units.
  - Materializing a `STRING` element as `CHAR` still requires the selected scalar value to fit in `u8`; otherwise the runtime reports overflow.
  - Materializing a `WSTRING` element as `WCHAR` requires the scalar value to fit in `u16`.
  - `SIZEOF(STRING[n])` and `SIZEOF(WSTRING[n])` remain storage-oriented (`n` and `2n` respectively), while runtime value sizing uses the same scalar-element counts described above.
- Impact:
  - Non-ASCII text behaves consistently across VM ref indexing and the shipped string stdlib, but the behavior is not raw IEC byte/code-unit indexing.
  - Existing projects that feed UTF-8 strings through file, MQTT, or fieldbus paths get stable element access semantics instead of mixed byte/scalar behavior.
- Mitigation:
- The runtime and docs now use one explicit element model end-to-end.
- A future raw-byte/raw-code-unit storage rewrite would be a separate compatibility project because it would require changing the underlying runtime value representation.

## 2026-04-20 - POINTER TO support beyond IEC REF_TO

- ID: DEV-018
- Area: Structured Text pointer types and runtime pointer operations
- IEC reference: `REF_TO` is standardized in IEC 61131-3 Ed.3 §6.4.4.10.2;
  `POINTER TO`, `ADR`, and dereference-style pointer workflows are vendor
  extensions.
- Deviation:
  - truST supports typed `POINTER TO` declarations in the language/type system.
  - `ADR(...)`, dereference (`^`), `NULL`, and `?=` work for pointers in both
    `trust-hir` and `trust-runtime`.
  - `POINTER TO` shares the runtime reference storage model used by `REF_TO`.
  - Pointer arithmetic is not supported.
- Impact:
  - Vendor-authored pointer-oriented ST can compile and execute in truST
    without rewriting to IEC-only `REF_TO` forms.
- Mitigation:
  - The extension is documented explicitly in specs and remains typed and
    non-arithmetic.

## 2026-04-20 - Assertion helper functions for test POUs

- ID: DEV-019
- Area: Standard-library assertion helpers
- IEC reference: IEC 61131-3 Ed.3 Tables 22-36 do not define assertion
  functions such as `ASSERT_TRUE` / `ASSERT_FALSE`.
- Deviation:
  - truST ships `ASSERT_TRUE`, `ASSERT_FALSE`, `ASSERT_EQUAL`,
    `ASSERT_NOT_EQUAL`, `ASSERT_GREATER`, `ASSERT_LESS`,
    `ASSERT_GREATER_OR_EQUAL`, `ASSERT_LESS_OR_EQUAL`, and `ASSERT_NEAR`.
  - These functions are executed by the runtime test framework and report
    assertion context in failure output.
- Impact:
  - Test POUs can use built-in assertion helpers instead of encoding
    expectations manually in ST control flow.
- Mitigation:
  - The helpers are documented as non-IEC test extensions and scoped to the
    testing/runtime workflow.

## 2026-04-20 - Reserved SFC keywords without textual SFC body syntax

- ID: DEV-020
- Area: Lexing and SFC authoring profile
- IEC reference: IEC 61131-3 Ed.3 reserves SFC keywords and defines SFC
  constructs, but truST currently ships visual SFC authoring rather than a
  textual SFC body syntax inside the ST parser.
- Deviation:
  - truST reserves SFC keywords such as `STEP`, `TRANSITION`, and `ACTION`.
  - truST ships a visual SFC editor/profile in the public docs.
  - Textual SFC body syntax is not currently accepted as part of the ST parser.
- Impact:
  - SFC-related words remain unavailable as identifiers even though textual SFC
    bodies are not yet part of the shipped ST grammar.
- Mitigation:
  - The reserved-keyword scope and authoring workflow are documented explicitly
    in the lexer spec and visual-editor docs.

## 2026-04-20 - Simplified `VAR_ACCESS` / `VAR_CONFIG` path validation

- ID: DEV-003
- Area: Structured Text access-path and configuration-variable validation
- IEC reference: IEC 61131-3 Ed.3 §6.5.2.2, Tables 13-16
- Deviation:
  - truST validates declared access-path shape and target-type compatibility.
  - Cross-resource / cross-program-instance mapping is not modeled completely in
    `trust-hir`.
- Impact:
  - Full IEC communication-service topology is not statically proven in the
    language layer.
- Mitigation:
  - The supported subset is documented in the variables spec and enforced
    consistently for the accepted forms.

## 2026-04-20 - Assignment-attempt compatibility is runtime-oriented

- ID: DEV-006
- Area: `?=` assignment-attempt semantics
- IEC reference: IEC 61131-3 Ed.3 §6.6.6.7.2, Table 71
- Deviation:
  - truST accepts `?=` for typed reference-style assignment attempts.
  - `trust-hir` does not fully enforce inheritance/interface compatibility for
    the source/target pair during static analysis.
- Impact:
  - Some compatibility failures are deferred to runtime null-result behavior
    instead of being diagnosed statically.
- Mitigation:
  - The operator remains typed, null-producing behavior is documented, and
    callers must check before dereference.

## 2026-04-20 - `PERSISTENT` behaves like `RETAIN`

- ID: DEV-007
- Area: Variable persistence qualifiers
- IEC reference: `PERSISTENT` is a vendor extension, not an IEC Ed.3 storage
  qualifier.
- Deviation:
  - truST accepts `PERSISTENT` and validates it with the same storage semantics
    as `RETAIN`.
- Impact:
  - Vendor-authored code using `PERSISTENT` compiles without rewriting to IEC
    core qualifiers.
- Mitigation:
  - The qualifier is documented explicitly as a vendor extension.

## 2026-04-20 - Standard FB semantic ownership split

- ID: DEV-010
- Area: Standard function-block modeling
- IEC reference: IEC 61131-3 Ed.3 §6.6.3.5, Tables 43-46, Figure 15
- Deviation:
  - `trust-hir` validates standard FB signatures and static types only.
  - `trust-runtime` executes the stateful timer/counter/trigger behavior.
- Impact:
  - Static docs/specs must distinguish language-level signature knowledge from
    runtime behavior/state.
- Mitigation:
  - The standard-FB spec now documents the split explicitly and cross-links the
    runtime-owned deviations.

## 2026-04-20 - ASCII-only identifier validation

- ID: DEV-013
- Area: Lexical identifier support
- IEC reference: IEC 61131-3 Ed.3 §6.1.1-6.1.2 permits broader character sets
  than truST currently accepts.
- Deviation:
  - truST validates identifiers using ASCII letters, digits, and `_` only.
  - Unicode identifiers are not currently accepted.
- Impact:
  - Some IEC-valid identifiers from broader character sets are rejected.
- Mitigation:
  - The lexer spec calls out the current limitation explicitly.

## 2026-04-20 - Extra non-IEC conversion helpers

- ID: DEV-021
- Area: Standard-library conversion functions
- IEC reference: IEC 61131-3 Ed.3 Tables 22-27 do not define `TIME_TO_DWORD`,
  `DWORD_TO_TIME`, or direct character-to-bitstring conversions such as
  `CHAR_TO_BYTE`.
- Deviation:
  - truST ships `TIME_TO_DWORD` / `DWORD_TO_TIME` using millisecond units.
  - truST accepts direct character-to-bitstring conversions such as
    `CHAR_TO_BYTE` and `WCHAR_TO_WORD`.
- Impact:
  - Vendor-oriented conversion workflows compile without custom wrappers.
- Mitigation:
  - These helpers are documented as non-IEC conversion extensions in the
    standard-functions spec.

## 2026-04-20 - Debugger visibility is broader than access-specifier rules

- ID: DEV-023
- Area: Debug-adapter variable visibility
- IEC reference: IEC 61131-3 Ed.3 §6.5.2.3 access-specifier rules
- Deviation:
  - The debugger may expose `PRIVATE`, `PROTECTED`, and `INTERNAL` members for
    inspection even when normal source-level access rules would hide them.
- Impact:
  - Debug inspection is more permissive than source-level member access.
- Mitigation:
  - The behavior is documented explicitly as a debug-surface deviation.

## 2026-04-20 - Per-resource hot reload with retained globals

- ID: DEV-024
- Area: Runtime online change / hot reload
- IEC reference: IEC 61131-3 does not standardize truST's runtime reload API.
- Deviation:
  - truST hot reload operates per resource rather than as a single-file-only
    mechanism.
  - Retained globals are preserved across the warm-restart style reload path.
- Impact:
  - The reload scope is broader and more runtime-oriented than earlier
    single-file wording suggested.
- Mitigation:
  - The engine/LSP/debug docs now describe the real per-resource behavior.

## 2026-04-20 - Debug forcing includes outputs as well as inputs

- ID: DEV-025
- Area: Runtime debug I/O forcing
- IEC reference: IEC 61131-3 does not define truST's DAP/control forcing
  surface.
- Deviation:
  - truST allows debugger/control forcing for output areas in addition to input
    areas.
- Impact:
  - The debug/runtime control path can simulate or override both sides of the
    process image.
- Mitigation:
  - The behavior is documented explicitly in runtime-engine/debug docs.

## 2026-04-20 - Test-oriented POU declarations

- ID: DEV-033
- Area: POU declarations
- IEC reference: IEC 61131-3 Ed.3 does not define `TEST_PROGRAM` or
  `TEST_FUNCTION_BLOCK`.
- Deviation:
  - truST accepts `TEST_PROGRAM` and `TEST_FUNCTION_BLOCK` as test-oriented
    declaration forms for the built-in test workflow.
- Impact:
  - Test POUs can be authored directly without wrapping test logic into regular
    IEC POUs plus external harness glue.
- Mitigation:
  - The declarations are documented as explicit non-IEC extensions and paired
    with the assertion-helper runtime workflow.

## 2026-04-20 - Siemens SCL `#local` reference prefix

- ID: DEV-034
- Area: Structured Text name references
- IEC reference: IEC 61131-3 Ed.3 does not define `#identifier` as a local-name
  reference operator in ST expressions/statements.
- Deviation:
  - truST accepts `#identifier` and lowers it as the same local name reference
    as `identifier`.
  - Malformed uses still diagnose with `expected identifier after '#'`.
- Impact:
  - Siemens-authored SCL that prefixes local names with `#` compiles without
    source rewriting.
- Mitigation:
  - The behavior is documented as a vendor-compatibility extension rather than
    IEC core syntax.
