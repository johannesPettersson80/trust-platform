# IEC Deviations Log

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
