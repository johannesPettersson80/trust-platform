# SFC Profile

This document defines the current truST scope for Sequential Function Chart
(SFC) support.

## Scope

- IEC SFC keywords such as `STEP`, `TRANSITION`, and `ACTION` are reserved by
  the lexer.
- truST ships a visual SFC editor/profile in the public docs and editor
  workflows.
- Textual SFC body syntax is not currently part of the Structured Text parser.

## Current Authoring Model

Use the visual-editor workflow for current SFC authoring guidance:

- `docs/public/develop/visual-editors/sfc.md`

The reserved-keyword and extension boundary is tracked in
`docs/IEC_DEVIATIONS.md` (DEV-020).

## Ownership

- Lexer/token reservation: `01-lexical-elements.md`
- Visual-editor/runtime alignment: `17-visual-editors-runtime-unification.md`
- User-facing authoring workflow: `docs/public/develop/visual-editors/sfc.md`
