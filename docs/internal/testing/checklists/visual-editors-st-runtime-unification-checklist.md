# Visual Editors ST Runtime Unification Checklist

Superseded 26 October 2026: the visual editors have no runtime mode of their own any more. A Ladder,
Blockly or Statechart program runs as its Structured Text companion, like any program
(`docs/specs/25-vscode-product-contract.md` §11.4.1). Sections 3 and 5 describe the removed editor
wiring, and the evidence lines below record the code of February 2026: `stRuntimeCommands.ts`,
`handleRuntimeMessage`, `generateVisualRuntimeEntrySource` and `resolveStartProgramUri` no longer exist.

Repository anchor:
- `/home/johannes/projects/trust-platform-merge-run`

Truth policy:
- Keep `[x]` only when implementation + verification evidence exist.
- Keep `[ ]` for anything not yet verified end-to-end.

## 1. Runtime Entry Generation

- [x] Visual companion sync writes sibling `*.st` companion for ladder/blockly/statechart.
  Evidence: `editors/vscode/src/visual/companionSt.ts`
- [x] Visual companion sync writes sibling `*.visual.runtime.st` launch wrapper with `PROGRAM` + `CONFIGURATION`.
  Evidence: `editors/vscode/src/visual/companionSt.ts`, `editors/vscode/src/test/suite/visual-companion.test.ts`
- [x] Wrapper naming and FB binding are deterministic from visual source name.
  Evidence: `editors/vscode/src/visual/companionSt.ts` (`generateVisualRuntimeEntrySource`)

## 2. Shared Runtime Command Path

- [x] `trust-lsp.debug.start` accepts visual source URIs and resolves them to generated runtime wrapper entries.
  Evidence: `editors/vscode/src/debug.ts` (`resolveStartProgramUri`)
- [x] Shared stop command exists for all editors (`trust-lsp.debug.stop`).
  Evidence: `editors/vscode/src/debug.ts`
- [x] Shared I/O commands exist for all editors (`trust-lsp.debug.io.write|force|release`).
  Evidence: `editors/vscode/src/debug.ts`
- [x] I/O panel now calls shared debug I/O commands (not duplicate custom-request logic).
  Evidence: `editors/vscode/src/ioPanel.ts` (`writeInput`, `forceInput`, `releaseInput`)

## 3. Visual Editor Wiring

- [x] Ladder runtime start/stop routes through shared ST command adapter.
  Evidence: `editors/vscode/src/ladder/ladderEditor.ts` (`handleRuntimeMessage`)
- [x] Blockly runtime start/stop routes through shared ST command adapter.
  Evidence: `editors/vscode/src/blockly/blocklyEditor.ts` (`handleRuntimeMessage`)
- [x] Statechart runtime start/stop routes through shared ST command adapter.
  Evidence: `editors/vscode/src/statechart/stateChartEditor.ts` (`handleRuntimeMessage`)
- [x] Ladder/Blockly/Statechart runtime I/O write/force/release routes through shared ST command adapter.
  Evidence: `editors/vscode/src/ladder/ladderEditor.ts`, `editors/vscode/src/blockly/blocklyEditor.ts`, `editors/vscode/src/statechart/stateChartEditor.ts`
- [x] Shared adapter utility exists under visual runtime module.
  Evidence: `editors/vscode/src/visual/runtime/stRuntimeCommands.ts`

## 4. Automated Validation

- [x] `cd editors/vscode && npm run lint`
  Evidence: run on 2026-02-27 (pass)
- [x] `cd editors/vscode && npm run compile`
  Evidence: run on 2026-02-27 (pass)
- [ ] `cd editors/vscode && npm test`
  Evidence: run on 2026-02-27; suite shows unrelated pre-existing failure in `snippets.test.ts` (`ton-usage` completion assertion), with all visual-runtime-related tests passing.

## 5. Manual Runtime Parity Verification

- [ ] Ladder visual editor: local/external start-stop and I/O force/write via right pane verified manually in Extension Development Host.
- [ ] Blockly visual editor: local/external start-stop and I/O force/write via right pane verified manually in Extension Development Host.
- [ ] Statechart visual editor: local/external start-stop and I/O force/write via right pane verified manually in Extension Development Host.
- [ ] UI parity screenshots/evidence captured for all three editors vs ST right-pane behavior.
