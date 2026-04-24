# Visual Editors

Visual examples show the open-artifact model: Ladder, SFC, Blockly, and
Statechart sources live beside ST and still validate through the same runtime
path.

| Example folder | Best for | Related docs |
| --- | --- | --- |
| `examples/ladder` | rung-based logic and companion ST flow | [Ladder](../develop/visual-editors/ladder.md) |
| `examples/statecharts` | event/state-centric logic | [Statechart](../develop/visual-editors/statechart.md) |
| `examples/blockly` | block-based authoring and generated ST | [Blockly](../develop/visual-editors/blockly.md) |
| `examples/sfc` | sequential function chart style authoring | [SFC](../develop/visual-editors/sfc.md) |

## Shared rule

All of these still execute through the same ST/runtime/debug path. The visual
asset is an authoring surface, not a second execution engine.

Read [Companion ST](../develop/visual-editors/companion-st.md) if you want the
mental model behind that rule.
