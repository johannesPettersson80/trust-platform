# Ladder

Use Ladder when your logic is naturally rung-oriented and operators or PLC
engineers already think in contacts, coils, timers, counters, and branches.

The screenshot was removed until the capture pipeline reliably opens the ladder
custom editor instead of raw JSON. The workflow below is still the product
workflow; the image will come back with a truthful capture.

## What it gives you

- rung-based visual authoring
- deterministic companion ST output
- the same runtime/debug path as the rest of truST

## Five-step quickstart

1. Open `examples/ladder/simple-start-stop.ladder.json` in VS Code.
2. Let truST auto-open the custom editor or use `Reopen Editor With...`.
3. Add or inspect contacts/coils in the rung.
4. Save the file and inspect the generated companion ST.
5. Run the same project through build/validate/runtime as you would for ST.

## Best for

- start/stop and seal-in circuits
- timer/counter-heavy control
- teams that already maintain ladder in another PLC environment

## When not to use Ladder

- when the logic is mostly state-machine behavior
- when the control flow is primarily step/transition sequencing
- when a straight ST file is simpler than a visual graph

## Common mistakes

- treating the diagram as if it had a separate runtime
- editing the generated companion ST directly and expecting the visual model to stay authoritative
- using Ladder for a state machine that would be clearer as Statechart or SFC

## Example folder

- `examples/ladder`

## Related

- [Companion ST](companion-st.md)
- [PLCopen](../interoperability/plcopen.md)
- [Ladder specification](../../reference/specifications/11-ladder-diagram.md)
- [truST ladder profile](../../reference/specifications/12-ladder-profile-trust.md)
