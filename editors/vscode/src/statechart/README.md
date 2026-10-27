# StateChart Editor - Usage Guide

Visual UML StateChart editor for `.statechart.json` files.

Every statechart executes as Structured Text (specification 17 §3): saving the file writes a
sibling companion `<name>.st` holding the machine as `FUNCTION_BLOCK FB_<name>_STATECHART`, and a
runtime entry `<name>.visual.runtime.st` holding a `PROGRAM` and `CONFIGURATION` that run it. The
editor has no runtime controls of its own: run the project from the truST sidebar and watch the
values (`STATE_INDEX`, `STATE_NAME`, the outputs) in Live Values.

The rules about a statechart are the truST language server's (`crates/trust-visual/src/statechart`,
reached through the `trust-lsp.visual*` commands): reading the file into the editor graph, writing
the graph back as the file, Validate, and the companion. The webview draws, places and edits.

## Editing

- Add states (`State`, `Initial`, `Final`), connect transitions by dragging between states.
- Select a state to edit its label, type and entry/exit actions; select a transition to edit its
  event, guard, actions and auto-transition timer (`after`, in milliseconds).
- **Validate** checks that the companion can be generated (the file has an `id` and at least one
  state, and every action mapping it uses has an action type).
- **Generate ST** saves the chart and writes the companion and runtime entry.

Saving writes the machine as `stateMachine` with its initial state, its states (non-empty entry and
exit actions, `final`/`compound` type, transitions by event, `NEXT` for a transition without an
event) and its action mappings. A transition to a state that does not exist is not written.

## The companion

- States are numbered in file order; `_state` holds the active one, starting at `initial`.
- Each event of an untimed transition is a `BOOL` input `EV_<EVENT>`.
- Each transition with `after` has its own `TON` (the delay rounded to whole milliseconds).
- The transitions of the active state are tried in file order. A `guard` joins the trigger with
  `AND`; JavaScript-style operators are rewritten (`==` `=`, `!=` `<>`, `&&` `AND`, `||` `OR`,
  `true`/`false` `TRUE`/`FALSE`).
- A taken transition runs the exit actions of its state, its own actions, then the entry actions
  of its target, and enters the target.

## Action Mappings

Action mappings bind action names to what the companion writes:

```json
{
  "actionMappings": {
    "turnOn_LED": {
      "action": "WRITE_OUTPUT",
      "address": "%QX0.0",
      "value": true
    },
    "resetAll": {
      "action": "SET_MULTIPLE",
      "targets": [
        { "address": "%QX0.0", "value": false },
        { "address": "%QX0.1", "value": false }
      ]
    },
    "logStatus": {
      "action": "LOG",
      "message": "Entering Safe State"
    }
  }
}
```

- `WRITE_OUTPUT`: assigns `value` to a direct address (or a variable).
- `WRITE_VARIABLE`: assigns `value` to a variable (`variable`).
- `SET_MULTIPLE`: assigns each target's `value` to its `address`.
- `LOG`: becomes a comment in the companion.

A value is written as a Structured Text literal (text as an IEC string). Direct addresses become
globals of the runtime entry, typed by their size (specification 03 §5); an address the runtime
does not accept, or an action without a mapping, becomes a comment.

## Example: Traffic Light

`examples/statecharts/traffic-light.statechart.json` cycles `Red -> Green -> Yellow -> Red` on the
`TIMER` event, switching one light on at entry and off at exit. Its companion and runtime entry are
checked in beside it.

## Related Docs

- [examples/statecharts/README.md](../../../examples/statecharts/README.md)

## References

- XState JSON format: <https://xstate.js.org/>
- ReactFlow docs: <https://reactflow.dev/>
