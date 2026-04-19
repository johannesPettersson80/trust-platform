# Companion ST

truST visual editors do not bypass Structured Text. Each visual source saves
into a deterministic companion ST path so build, validate, runtime, reload, and
debug all stay on one execution model.

## What this means in practice

- the visual source is the authoring artifact
- the companion ST is the generated executable companion
- the runtime/debug/test surfaces still operate on the same project

## Side-by-side example

Conceptually, a visual rung like:

```text
StartCmd ----] [-----------( )---- LampOut
```

lands on companion ST shaped like:

```st
LampOut := StartCmd;
```

The exact generated output depends on the editor and source model, but the
important rule is the same: the runtime is still executing the unified truST
project model.

## Edit rules

Use the visual editor when:

- you are changing the graph/model itself
- you want the editor to remain authoritative

Use the companion ST for:

- inspection
- review
- understanding what the runtime will execute

Do not edit the companion ST directly if the visual file is still the source of
truth. Your manual changes will be overwritten on the next visual save.

## Related

- [Visual Companion Model](../../concepts/visual-companion-model.md)
- [Compile, Validate, Reload](../../operate/compile-validate-reload.md)
- [Visual editor examples](../../examples/visual-editors.md)
