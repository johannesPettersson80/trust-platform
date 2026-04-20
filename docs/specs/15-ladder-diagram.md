# Ladder Diagram (LD) Language Specification

IEC 61131-3 Edition 3.0 (2013-02) - Section 8.2 (Ladder Diagram)

Status: Normative language specification for LD behavior in this repository.

## 1. Purpose and Scope

This document defines the normative Ladder Diagram (LD) language semantics used by truST.
It covers:

- LD program model (networks/rungs and power flow).
- Variable usage rules in LD.
- Contact/coil/branch behavior.
- Function and function block invocation in LD bodies.
- Deterministic scan-cycle evaluation model.
- Required diagnostics for invalid LD programs.

This document is implementation-agnostic. JSON/webview/runtime profile constraints are
defined separately in `docs/specs/16-ladder-profile-trust.md`.

## 2. Conformance Language

The keywords `MUST`, `MUST NOT`, `SHOULD`, `SHOULD NOT`, and `MAY` are normative.

An implementation conforms to this specification only if all `MUST` requirements in this
document are satisfied, except where an explicit deviation is recorded in
`docs/IEC_DEVIATIONS.md`.

## 3. IEC Anchors Used

Primary IEC anchors used by this LD specification:

- IEC 61131-3 Ed.3 Section 8.2: Ladder Diagram (LD)
- IEC 61131-3 Ed.3 Table 40: FUNCTION_BLOCK declarations
- IEC 61131-3 Ed.3 Tables 43-46: standard FB semantics (bistable, edge, counter, timer)
- IEC 61131-3 Ed.3 Section 6.5.2.2 and Figure 7: variable sections and visibility

## 4. LD Program Model

### 4.1 Networks and Execution Order

- An LD body consists of ordered networks (rungs).
- Networks are evaluated from top to bottom in deterministic order.
- Each network is evaluated from logical left rail toward logical right rail.

### 4.2 Power Flow

- Power flow within a network is boolean.
- A path is energized when all series conditions on that path evaluate true.
- Parallel branch paths are independent logical alternatives.

### 4.3 Determinism

The implementation MUST define deterministic tie-break behavior when source layout is
ambiguous (for example, equal horizontal positions or partially disconnected geometry).

## 5. Variable and Data Model

### 5.1 Symbolic-First Policy

LD programs SHOULD be authored with symbolic variables declared in IEC variable sections,
including:

- local declarations (`VAR`, `VAR_TEMP`, `VAR_INPUT`, `VAR_OUTPUT`, `VAR_IN_OUT`)
- global declarations (`VAR_GLOBAL`)
- global access via `VAR_EXTERNAL`

Directly addressed operands (`%I`, `%Q`, `%M`) MAY be used when required for hardware
binding, but they are secondary to symbolic declarations.

### 5.2 Scope and Resolution

Variable resolution in LD MUST respect IEC scope rules:

1. local/POU scope
2. enclosing context scope (where applicable)
3. global scope via `VAR_EXTERNAL`
4. qualified names

### 5.3 Type Requirements

- Contact conditions MUST resolve to BOOL-compatible values.
- Coil targets MUST be assignable BOOL-compatible storage locations.
- Compare and math block operands MUST satisfy their operator type requirements.
- Timer/counter instance typing MUST satisfy the corresponding standard FB contract.

## 6. Core LD Element Semantics

### 6.1 Contacts

- `NO` contact passes power when its operand evaluates true.
- `NC` contact passes power when its operand evaluates false.

### 6.2 Coils

- `NORMAL`: writes current network power to target.
- `SET`: when powered, writes `TRUE`; when unpowered, leaves previous value unchanged.
- `RESET`: when powered, writes `FALSE`; when unpowered, leaves previous value unchanged.
- `NEGATED`: writes logical negation of current network power to target.

### 6.3 Branching

- Branch split duplicates incoming power to outgoing branch legs.
- Branch merge resolves incoming leg power by logical OR.
- Branch evaluation MUST be deterministic and independent of rendering artifacts.

## 7. Functions and Function Blocks in LD

### 7.1 Function Invocation

Functions used in LD boxes MUST follow the same type and parameter rules as ST expression
calls.

### 7.2 Function Block Invocation

Function block instances used in LD MUST follow IEC FB declaration and call contracts
(Table 40 and relevant FB tables).

### 7.3 Standard FB Semantics

When these blocks are supported in LD, semantics MUST align with IEC standard tables:

- Bistable FBs: Table 43
- Edge detection FBs: Table 44
- Counters: Table 45
- Timers: Table 46

If a subset profile is implemented, unsupported FB behavior MUST be diagnosed and listed
in `docs/IEC_DEVIATIONS.md`.

## 8. Scan-Cycle Execution Semantics

An LD scan MUST follow this sequence:

1. Acquire input image.
2. Evaluate networks in deterministic order.
3. Compute writes during evaluation.
4. Commit writes at defined commit boundary.
5. Publish resulting state for next cycle/observation.

To preserve deterministic PLC behavior, write effects from one network SHOULD NOT create
hidden intra-cycle order dependence beyond the defined commit model.

## 9. Diagnostics and Error Conditions

Implementations MUST emit actionable diagnostics for at least:

- unresolved variable references
- invalid or non-assignable coil targets
- type-incompatible element operands
- malformed network topology (for graph-based editors/importers)
- unsupported LD constructs in interchange formats

Diagnostics SHOULD identify the element/network and expected contract.

| Diagnostic Area | Expected message focus |
|-----------------|------------------------|
| unresolved references | identify the missing symbol and the network element that referenced it |
| invalid coil target | state that the target is not assignable and name the offending coil |
| type mismatch | identify the LD element and incompatible operand types |
| malformed topology | identify the broken branch/network structure |
| unsupported interchange construct | identify the PLCopen/import/export construct that was rejected |

## 10. Edition 2 vs Edition 3 Compatibility

For LD core behavior (contacts, coils, branching, and scan-cycle interpretation), this
spec adopts IEC Edition 3 as authoritative and maintains behavior compatible with Edition
2 usage patterns.

Known incompatibilities or profile restrictions MUST be recorded in
`docs/IEC_DEVIATIONS.md`.

## 11. Examples

### 11.1 Symbolic Global + Local Mapping

```iecst
VAR_GLOBAL
  StartPB : BOOL;
  StopPB  : BOOL;
  Motor   : BOOL;
END_VAR

PROGRAM Main
VAR_EXTERNAL
  StartPB : BOOL;
  StopPB  : BOOL;
  Motor   : BOOL;
END_VAR
```

Equivalent LD intent:

- `NO(StartPB)` in series with `NC(StopPB)` drives `NORMAL(Motor)`.

### 11.2 Direct Address Form (Optional)

- `NO(%IX0.0)` in series with `NC(%IX0.1)` driving `NORMAL(%QX0.0)`.

This form is allowed for direct hardware binding but is not the preferred modeling style.

## 12. Profile Boundary

The concrete truST LD profile (JSON schema, VS Code editor behavior, runtime panel wiring,
interop subset, and current limitations) is specified in:

- `docs/specs/16-ladder-profile-trust.md`

Any profile-specific behavior that differs from this normative document MUST be tracked in:

- `docs/IEC_DEVIATIONS.md`
