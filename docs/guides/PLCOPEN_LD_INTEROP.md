# PLCopen LD Interop Guide (Schema v2)

truST does not import or export Ladder Diagram bodies as PLCopen XML. The
TypeScript LD interchange (`editors/vscode/src/ladder/plcopenLdInterop.ts`),
which no command of the extension called and only its own tests reached, was
removed on 26 October 2026 with those tests
(`docs/specs/25-vscode-product-contract.md` §11.4.1).

A Ladder program is its `.ladder.json` file (schema v2), drawn by the Ladder
editor. It runs as Structured Text: saving it writes the companion
`<name>.st` and the runtime entry `<name>.visual.runtime.st` beside it
(`docs/specs/17-visual-editors-runtime-unification.md` §3).

PLCopen XML import and export cover Structured Text; see
`docs/guides/PLCOPEN_INTEROP_COMPATIBILITY.md`. The withdrawn LD deviation is
recorded in `docs/PLCOPEN_DEVIATIONS.md` (entry of 2026-02-25).
