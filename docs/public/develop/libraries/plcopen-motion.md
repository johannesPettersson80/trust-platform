# PLCopen Motion

PLCopen Motion support covers standard motion terminology, the truST-supported
function-block subset, and the proof path from example axis to project
validation.

Use [Libraries](index.md) first for package/project ownership. Use
[Libraries And Motion examples](../../examples/libraries-and-motion.md) for the
smallest runnable motion-shaped project.

Use [PLCopen Motion OOP](plcopen-motion-oop.md) when the project should expose
axis objects, command objects, and PLCopen OOP properties instead of only the
classic function-block call surface.

## Worked Example

The gallery's starter **PLCopen Motion** (`examples/plcopen-motion-single-axis`)
runs one virtual axis with the single-axis core, and the acceptance journeys run
its walkthrough on the packaged extension: [Move One Virtual
Axis](../../examples/plcopen-motion-single-axis.md). Its program keeps the
library's usage rules below: one `AXIS_REF` for the axis; each block instantiated
once and called every scan; the move started by the rising edge of
`MoveAbsolute`'s `Execute`, as `MoveExecute` is TRUE for the one scan of step 10;
and the readback blocks called continuously, so that the outputs show the axis
as it is. This is the program as the example ships it:

```iecst
--8<-- "examples/plcopen-motion-single-axis/src/Main.st"
```

## Library Guide

--8<-- "docs/guides/PLCOPEN_MOTION_LIBRARY_GUIDE.md:3"

## Related

- [Libraries overview](index.md)
- [PLCopen Motion OOP](plcopen-motion-oop.md)
- [Vendor Profiles](../vendor-profiles.md)
- [Libraries and Motion examples](../../examples/libraries-and-motion.md)
