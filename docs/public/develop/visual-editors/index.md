# Visual Editors

Ladder, SFC, Blockly, Statechart, and ST stay in one project, one build path,
and open artifacts. Pick the visual shape that matches the logic, then validate
the generated or companion behavior through the same truST runtime/debug loop.

## Pick the right editor

| Editor | Best for | What it feels like | Good first file |
| --- | --- | --- | --- |
| Ladder | relay/rung logic, start-stop circuits, classic PLC patterns | left-to-right rung authoring | `examples/ladder/simple-start-stop.ladder.json` |
| Statechart | mode-driven/state-heavy systems | states, transitions, events | `examples/statecharts/traffic-light.statechart.json` |
| Blockly | beginner-friendly block composition and demos | drag-and-drop blocks | `examples/blockly/simple-led-blink.blockly.json` |
| SFC | step/transition sequences and procedural phases | steps, transitions, branches | `examples/sfc/sfc_simple_parallel.sfc` |

## Shared rules across all visual editors

- the visual file lives in the same project as your ST code
- saving the visual source updates the companion ST path
- build, validate, runtime, reload, and debug still go through truST
- the visual editor is an authoring surface, not a second execution engine
- AI can read and edit the underlying files, but dedicated visual-editor AI
  tooling is not a separate surface yet

## Recommended reading order

1. Pick the editor that matches the logic shape you already have in mind.
2. Read that editor page.
3. Read [Companion ST](companion-st.md).
4. Validate the result with [Compile, Validate, Reload](../../operate/compile-validate-reload.md).

## Related

- [Visual Companion Model](../../concepts/visual-companion-model.md)
- [Visual editor examples](../../examples/visual-editors.md)
- [Ladder spec](../../reference/specifications/15-ladder-diagram.md)
