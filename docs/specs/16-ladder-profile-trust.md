# Ladder Diagram Profile for truST

Status: Implementation profile (product-specific behavior and constraints).

This document defines the current truST LD profile that implements
`docs/specs/15-ladder-diagram.md`.

## 1. Scope

This profile covers:

- LD source schema used by VS Code visual editor (`.ladder.json`, schema v2).
- Deterministic Ladder authoring and lowering into generated Structured Text.
- PLCopen LD interop subset.
- Known profile constraints and deviations.

## 2. Canonical Data Contract (Schema v2)

Every LD source file MUST include:

```json
{
  "schemaVersion": 2
}
```

Top-level model (`LadderProgram`):

- `schemaVersion: 2`
- `metadata: { name, description, created?, modified? }`
- `variables: Variable[]`
- `networks: Network[]`

`Variable` profile shape:

- `name: string`
- `type: BOOL | INT | REAL | TIME | DINT | LREAL`
- `scope?: local | global` (defaults to `global` when omitted)
- `address?: string`
- `initialValue?: unknown`

`Network` profile shape:

- `id: string`
- `order: number`
- `nodes: LadderNode[]`
- `edges: Edge[]`
- `layout: { y: number }`

Schema enforcement behavior:

- Files missing `schemaVersion: 2` are rejected with actionable diagnostics.
- Legacy schema is not auto-migrated.
- Enum-like node attributes are strict (`contactType`, `coilType`, `timerType`,
  `counterType`, `op`) and invalid values are rejected with diagnostics.
- Invalid payloads are never silently coerced to fallback defaults.

## 3. Supported LD Node Subset

Supported node kinds and profile fields:

- `contact`: `contactType (NO|NC)`, `variable`
- `coil`: `coilType (NORMAL|SET|RESET|NEGATED)`, `variable`
- `timer`: `timerType (TON|TOF|TP)`, `instance`, `presetMs`, optional `input`,
  required `qOutput`, required `etOutput`
- `counter`: `counterType (CTU|CTD|CTUD)`, `instance`, `preset`, optional `input`,
  required `qOutput`, required `cvOutput`
- `compare`: `op (GT|LT|EQ)`, `left`, `right`
- `math`: `op (ADD|SUB|MUL|DIV)`, `left`, `right`, `output`
- topology nodes: `branchSplit`, `branchMerge`, `junction`

## 4. Authoring-to-Runtime Lowering

The shipped runtime path is the generated Structured Text companion and runtime
wrapper defined by `17-visual-editors-runtime-unification.md`. The authoring
model preserves:

- Deterministic network order: ascending `network.order`.
- Topology validation rejects malformed or non-resolvable graph shapes.
- Visual editor runtime controls execute through generated `.st` companion + runtime-entry
  wrapper (`*.visual.runtime.st`) and the shared Structured Text debug command path.

`ladderEngine.ts` is a retained editor/component model and is not an
independent product-runtime oracle. Its tests MUST NOT override Structured Text
runtime semantics. In particular, generated division follows the Structured
Text runtime's divide-by-zero fault behavior; a component fallback value of
zero is not shipped runtime authority.

Primary implementation anchors:

- `editors/vscode/src/visual/companionSt.ts`
- `editors/vscode/src/visual/runtime/stRuntimeCommands.ts`
- `editors/vscode/src/debug.ts`

## 5. Variable and Address Resolution

The authoring-to-ST lowering supports declaration-first symbols and direct
addresses:

- node fields such as `contact.variable` / `coil.variable` are string references and may
  contain symbols or `%I/%Q/%M` direct addresses.
- timer/counter output targets (`qOutput`, `etOutput`, `cvOutput`) are explicit references
  (symbolic or `%M*` addresses); hidden internal `%MX_LD_*` / `%MW_LD_*` timer/counter
  output mirrors are not used.
- symbol resolution uses local-first precedence with optional explicit qualification:
  - unqualified: `local` then `global`
  - qualified: `LOCAL::Name` / `GLOBAL::Name` (also `LOCAL.Name` / `GLOBAL.Name`)

## 6. Runtime Surface Ownership

Runtime placement is owned by
`25-vscode-product-contract.md` section 8.1. Ladder MUST NOT embed a duplicate
runtime, I/O, runtime-settings, or compile-diagnostics pane. Runtime lifecycle
stays in the truST sidebar and runtime values stay in Live Values.

The Ladder right pane remains an authoring surface. Its width MAY persist per
editor type, and its tools/edit/view state MAY use the shared visual-editor
persistence contract, but that persistence does not authorize retired runtime
actions or schemas.

## 7. PLCopen LD Interoperability Profile

Supported:

- Import PLCopen LD network bodies to schema v2 subset.
- Export schema v2 subset back to PLCopen LD network bodies.
- Deterministic diagnostics for unsupported/malformed constructs.
- Invalid node enum attributes are diagnosed and skipped (not auto-normalized).

Unsupported vendor-specific constructs are skipped with diagnostics and are not silently
accepted.

Implementation anchors:

- `editors/vscode/src/ladder/plcopenLdInterop.ts`
- `editors/vscode/src/test/suite/plcopen-ld-interop.test.ts`
- `docs/guides/PLCOPEN_LD_INTEROP.md`

## 8. Known Deviations and Decisions

Normative ambiguities and profile differences are tracked here:

- `docs/IEC_DECISIONS.md`
- `docs/IEC_DEVIATIONS.md`

At the time of writing, the CTUD pin-model constraint is an IEC deviation. The
schema-level free-form operand string (`symbol` and direct-address tokens share
the same field type) is a truST editor/schema constraint documented by this
product profile, not an IEC deviation.

## 9. Verification Evidence

Primary test evidence for this profile:

- `editors/vscode/src/test/suite/ladder-schema.test.ts`
- `editors/vscode/src/test/suite/ladder-editor-ops.test.ts`
- `editors/vscode/src/test/suite/plcopen-ld-interop.test.ts`
- `editors/vscode/src/test/suite/visual-companion.test.ts`
- `editors/vscode/src/test/suite/visual-right-pane-resize.test.ts`

The retained Ladder engine, embedded runtime-panel bridge, and embedded I/O
panel tests are component or historical behavior locks only. They are not
evidence for the shipped runtime route. Positive runtime and presentation
claims require generated-ST execution and rendered extension evidence.
