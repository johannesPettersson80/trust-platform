# IEC Decisions Log

Authoritative location:
- This tracked file is the repository source of truth for IEC interpretation decisions.
- Do not point tracked docs or code comments at legacy internal IEC decision-log paths.

This file tracks implementation decisions made where IEC 61131-3 leaves room for interpretation.

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
  - `VAR_IN_OUT` requires identical string family and identical declared capacity. Width-changing `VAR_IN_OUT` binding is rejected rather than converted.
  - A rejected `VAR_IN_OUT` binding cannot mutate caller state.
  - The same rules apply to `STRING` and `WSTRING`.
- Reason:
  - Truncation preserves the product's established ordinary-assignment behavior while enforcing every receiving declaration's maximum.
  - Exact-capacity `VAR_IN_OUT` avoids an implicit round trip that can change a caller even when the called POU performs no write.

## 2026-07-05 - Subrange runtime write enforcement

- Area: ST subrange data types and runtime writes
- IEC context: IEC 61131-3 Ed.3 §6.4.4.4.1 defines subrange values by inclusive lower/upper limits and treats values outside that range as errors; Table 11 defines user-defined subrange declarations.
- Decision:
  - Runtime writes into subrange-typed storage are checked against the declared bounds.
  - Out-of-range execution-time assignment, function/FB parameter copy-in, dynamic-reference writes, HMI/control writes, and retain reload surface a deterministic runtime error.
  - The runtime must not silently clamp, wrap, or store an out-of-range value.
  - A rejected write is not committed: the target retains the exact value it held before the attempted write.
  - Declaration-initialization edge cases remain out of this Phase 11 decision unless a separate initializer-specific proof row is opened.
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
