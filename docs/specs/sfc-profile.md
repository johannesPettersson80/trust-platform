# SFC Profile

This document defines the current truST scope for Sequential Function Chart
(SFC) support.

## Scope

- IEC SFC keywords such as `STEP`, `TRANSITION`, and `ACTION` are reserved by
  the lexer.
- truST ships a visual SFC editor/profile in the public docs and editor
  workflows.
- Textual SFC body syntax is not currently part of the Structured Text parser.
- As a bounded source-analysis facility, the parser and HIR accept an
  IEC-shaped textual `ACTION name: ... END_ACTION` declaration directly in a
  `PROGRAM` or `FUNCTION_BLOCK`. This does not include steps, transitions,
  associations, qualifiers, or action-control execution.

## Textual ACTION analysis boundary

The front end retains textual action declarations so imported or partially
authored sources receive syntax, name-resolution, type, and control-flow
diagnostics. This is not a promise of textual-SFC execution:

- the required declaration shape and semantic rules are specified by
  `04-pou-declarations.md`;
- an action name is not directly callable from ordinary ST;
- the runtime compiler must reject every source containing a textual action
  declaration rather than silently discard or execute its body; and
- no step association, qualifier, activation state, or scan-cycle behavior is
  defined for textual actions.

This fail-closed boundary remains in force until a later product specification
defines and verifies textual SFC as an executable language. The optional SFC
feature is currently unsupported, so this boundary is not an IEC deviation.

## Current Authoring Model

Use the visual-editor workflow for current SFC authoring guidance:

- `docs/public/develop/visual-editors/sfc.md`

The reserved-keyword and visual-authoring boundary is a documented truST
product profile. It is outside the ST parser's IEC behavior and is not an IEC
deviation.

## Ownership

- Lexer/token reservation: `01-lexical-elements.md`
- Textual ACTION analysis and compilation boundary:
  `04-pou-declarations.md`
- Visual-editor/runtime alignment: `17-visual-editors-runtime-unification.md`
- User-facing authoring workflow: `docs/public/develop/visual-editors/sfc.md`
