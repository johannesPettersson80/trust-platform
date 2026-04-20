# Ladder Diagram Profile for truST

Status: Implementation profile (product-specific behavior and constraints).

This document defines the current truST LD profile that implements
`docs/specs/15-ladder-diagram.md`.

## 1. Scope

This profile covers:

- LD source schema used by VS Code visual editor (`.ladder.json`, schema v2).
- Runtime execution behavior currently implemented for LD in the extension stack.
- Runtime control surface parity contract for visual editors.
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

## 4. Runtime Execution Profile

Current runtime profile behavior:

- Deterministic network order: ascending `network.order`.
- Deterministic node tie-break in engine traversal for ambiguous coordinates.
- Scan-cycle style execution with buffered write commit boundary.
- Topology validation rejects malformed or non-resolvable graph shapes.
- Visual editor runtime controls execute through generated `.st` companion + runtime-entry
  wrapper (`*.visual.runtime.st`) and the shared Structured Text debug command path.

Implementation anchors:

- `editors/vscode/src/ladder/ladderEngine.ts`
- `editors/vscode/src/visual/companionSt.ts`
- `editors/vscode/src/visual/runtime/stRuntimeCommands.ts`
- `editors/vscode/src/debug.ts`
- `editors/vscode/src/test/suite/ladder-engine.test.ts`

## 5. Variable and Address Resolution in Current Profile

Current execution behavior supports both declaration-first symbols and direct addresses:

- node fields such as `contact.variable` / `coil.variable` are string references and may
  contain symbols or `%I/%Q/%M` direct addresses.
- timer/counter output targets (`qOutput`, `etOutput`, `cvOutput`) are explicit references
  (symbolic or `%M*` addresses); hidden internal `%MX_LD_*` / `%MW_LD_*` timer/counter
  output mirrors are not used.
- symbol resolution uses local-first precedence with optional explicit qualification:
  - unqualified: `local` then `global`
  - qualified: `LOCAL::Name` / `GLOBAL::Name` (also `LOCAL.Name` / `GLOBAL.Name`)
- runtime write/force/release pathways resolve symbolic references as well as direct
  `%IX*` addresses.

I/O panel behavior:

- `%IX*` treated as inputs
- `%QX*` treated as outputs
- `%MX*` and `%MW*` treated as marker/memory state
- declared symbols are surfaced with resolved values, including scoped names when local
  and global declarations shadow each other

## 6. Runtime Control UI Parity Contract

Visual editors (Ladder/Statechart/Blockly) share a runtime control contract with:

- mode: `local | external`
- state: `isExecuting`, `status`, optional `lastError`
- actions: `setMode`, `start`, `stop`, `openRuntimePanel`, `openRuntimeSettings`

Current right-pane contract:

- ST-style runtime controls and I/O tree are embedded in visual editor right pane.
- Right pane width persists per editor type and is user-resizable.
- Ladder-specific edit tools are presented in the same right pane as a tools view.
- Start/stop and mode actions route to shared ST command handlers
  (`trust-lsp.debug.start|attach|stop`), with visual sources auto-synced to companion and
  runtime-entry ST files before launch.
- Right-pane I/O write/force/release routes to shared ST I/O command handlers
  (`trust-lsp.debug.io.write|force|release`).

Implementation anchors:

- `editors/vscode/src/visual/runtime/runtimeController.ts`
- `editors/vscode/src/visual/runtime/runtimeMessages.ts`
- `editors/vscode/src/visual/runtime/runtimePanelBridge.ts`
- `editors/vscode/src/visual/runtime/rightPaneResize.ts`
- `editors/vscode/src/ioPanel.ts`

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

At the time of writing, key profile deviations include CTUD pin-model constraints and
schema-level free-form operand strings (`symbol` and direct-address tokens share the same
field type).

## 9. Verification Evidence

Primary test evidence for this profile:

- `editors/vscode/src/test/suite/ladder-schema.test.ts`
- `editors/vscode/src/test/suite/ladder-engine.test.ts`
- `editors/vscode/src/test/suite/ladder-editor-ops.test.ts`
- `editors/vscode/src/test/suite/plcopen-ld-interop.test.ts`
- `editors/vscode/src/test/suite/visual-runtime-controller.test.ts`
- `editors/vscode/src/test/suite/visual-runtime-panel-bridge.test.ts`
- `editors/vscode/src/test/suite/visual-right-pane-resize.test.ts`
