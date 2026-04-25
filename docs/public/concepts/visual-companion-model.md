# Visual Companion Model

truST visual editors are companion authoring surfaces over the same executable
model, not separate runtimes with ad hoc translation.

| Term | Meaning |
| --- | --- |
| Visual editor | Ladder, Statechart, Blockly, or SFC authoring UI. |
| Companion ST | Generated/reviewable Structured Text that bridges the visual model into the normal build/runtime path. |
| Source of truth | Artifact the user should edit directly. |

Use this mental model when you need the relationship between visual editors and
companion ST. For operational authoring workflow, start with
[Companion ST](../develop/visual-editors/companion-st.md).

## What that means

Ladder, Statechart, Blockly, and SFC flows are supposed to land on the same
underlying project/runtime semantics instead of inventing a separate execution
engine for each editor.

## Side-by-side mental model

A visual rung or graph step remains the authoring UI, but the generated
companion ST is still the executable bridge the rest of truST understands.

Conceptually, a visual rung like:

```text
StartCmd ----] [-----------( )---- LampOut
```

lands on companion ST shaped like:

```st
LampOut := StartCmd;
```

The exact generated text depends on the editor and the saved model, but the key
rule is stable: visual authoring changes the input surface, not the runtime
execution model.

## Practical consequence

When you save a visual model, the surrounding tooling should still align with:

- the same project layout
- the same runtime/debug UIs
- the same validation and reload loops
- the same docs/reference model

This is why the companion-ST and visual-runtime unification specs matter:
visual authoring is a different way to author, not a different way to execute.

## What not to do

Do not treat companion ST as a second source of truth while the visual file is
still authoritative. Manual edits to the generated companion file are for
inspection or review only unless you are intentionally breaking out of the
visual workflow.

## Related

- [Companion ST authoring rules](../develop/visual-editors/companion-st.md)
- [Visual Editors overview](../develop/visual-editors/index.md)
- [Visual Editors Runtime Unification spec](../reference/specifications/17-visual-editors-runtime-unification.md)
- [Ladder Diagram spec](../reference/specifications/15-ladder-diagram.md)
