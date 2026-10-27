# Ladder Diagram Profile for truST

Status: Implementation profile (product-specific behavior and constraints).

This document defines the current truST LD profile that implements
`docs/specs/15-ladder-diagram.md`.

## 1. Scope

This profile covers:

- LD source schema used by VS Code visual editor (`.ladder.json`, schema v2).
- Deterministic Ladder authoring and lowering into generated Structured Text.
- Known profile constraints and deviations.

The rules of this profile are implemented once, in the crate `trust-visual`
(`crates/trust-visual/src/ladder/`), and reached by the editor through the
language server's `trust-lsp.visual*` commands (`25-vscode-product-contract.md`
§11.4).

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

The companion is one `FUNCTION_BLOCK`: networks in ascending `order`, each
network's power flow as Boolean expressions (a contact ANDs its operand, NC
its negation; parallel legs OR; a node reached from the left rail gets
`TRUE`), coils assigning (`NORMAL`, `NEGATED`) or latching (`SET`, `RESET`)
the power reaching them, timers and counters called with that power or their
own `input`, compare blocks storing their result in `ld_compare_q_<id>`, math
blocks writing their `output` while power reaches them. Generated division is
Structured Text division and follows the runtime's divide-by-zero fault
behavior; no fallback value of zero is generated. No editor-specific execution
engine exists (`25-vscode-product-contract.md` §11.4.1).

Primary implementation anchors:

- `crates/trust-visual/src/ladder/lower.rs` (companion)
- `crates/trust-visual/src/companion.rs` (companion and runtime entry files)
- `editors/vscode/src/visual/companionSt.ts` (writes the files)

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
  - qualified: `LOCAL::Name` / `GLOBAL::Name` (also `LOCAL.Name` / `GLOBAL.Name`
    and the short `L::Name` / `G::Name`, in any case); the companion names the
    plain variable. `LOCAL::Name` without such a local, `GLOBAL::Name` without
    such a global, and `GLOBAL::Name` hidden by a local of the same name make
    the program untranslatable.
- every symbol an operand names must be declared; `TRUE` and `FALSE` are
  literals, not symbols.
- a direct address becomes the global `ld_io_<address>` declared `AT` it in the
  runtime entry, typed by its size (`03-variables.md` §5), unless a global
  variable is declared at that address; a text written like an address that
  the runtime refuses (`%IX0.9`) makes the program untranslatable.
- a contact or coil operand that names an undeclared identifier declares it as
  a `BOOL` local starting `FALSE` (a global for `GLOBAL::`); such implicit
  locals that no element names any more are removed.

## 6. Runtime Surface Ownership

Runtime placement is owned by
`25-vscode-product-contract.md` section 8.1. Ladder MUST NOT embed a duplicate
runtime, I/O, runtime-settings, or compile-diagnostics pane. Runtime lifecycle
stays in the truST sidebar and runtime values stay in Live Values.

The Ladder right pane remains an authoring surface. Its width MAY persist per
editor type, and its tools/edit/view state MAY use the shared visual-editor
persistence contract, but that persistence does not authorize retired runtime
actions or schemas.

## 7. PLCopen LD Interchange

truST has no PLCopen LD interchange: PLCopen import skips a POU with an LD
body with diagnostic `PLCO216` (`25-vscode-product-contract.md` §11.3). The
earlier editor-side LD interop module was reached only by its own tests and
was removed on 26 October 2026.

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

- `crates/trust-visual/src/ladder/tests.rs` and `tests/` (schema check,
  opening and saving, companion lowering, editor edits)
- `crates/trust-visual/src/ladder/power.rs` (power flow)
- `crates/trust-visual/src/companion.rs` (checked-in examples reproduced
  exactly; every example translates)
- `crates/trust-lsp/src/handlers/commands/visual_tests.rs` (command answers)
- `editors/vscode/src/test/suite/ladder-schema.test.ts`,
  `visual-companion.test.ts`, `visual-new-program-editor.test.ts` (wiring)
- `editors/vscode/src/test/suite/visual-right-pane-resize.test.ts`

Positive runtime and presentation claims require generated-ST execution and
rendered extension evidence.
