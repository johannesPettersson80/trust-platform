# IEC Deviations Log

Authoritative location:
- This tracked file is the repository source of truth for IEC deviations/extensions.
- Do not point tracked docs or code comments at legacy internal IEC deviation-log paths.

This file tracks known, intentional deviations/extensions from strict IEC 61131-3 behavior.

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
  - Normative spec defines symbolic-first policy; profile constraints are documented in `docs/specs/12-ladder-profile-trust.md`.

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
