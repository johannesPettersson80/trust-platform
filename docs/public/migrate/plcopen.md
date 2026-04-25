# PLCopen

PLCopen is the open interchange path for XML import/export. The ST-complete
baseline and Ladder profile are separate because they preserve different parts
of the source model.

| Question | Check |
| --- | --- |
| what survives import/export? | ST-complete compatibility |
| what applies to Ladder? | Ladder interop profile |
| is the migration complete? | build, validate, run, and inspect the generated project |

## ST-Complete Compatibility

--8<-- "docs/guides/PLCOPEN_INTEROP_COMPATIBILITY.md:3"

## Ladder Interop Profile

--8<-- "docs/guides/PLCOPEN_LD_INTEROP.md:3"

## Related

- [Migration While Programming](index.md)
- [CODESYS And TwinCAT](codesys-twincat.md)
- [Ladder Editor](../develop/visual-editors/ladder.md)
- [PLCopen XML example](../examples/vendor-profiles.md)
