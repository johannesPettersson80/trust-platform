# IEC Decisions Log

Authoritative location:
- This tracked file is the repository source of truth for IEC interpretation decisions.
- Do not point tracked docs or code comments at legacy internal IEC decision-log paths.

This file tracks implementation decisions made where IEC 61131-3 leaves room for interpretation.

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
