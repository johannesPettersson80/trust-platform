# PLCopen

Use this guide when:

- you are importing or exporting PLCopen XML
- you need to understand what survives a round trip
- you want the ST-complete baseline plus Ladder profile notes

The two sections below separate general ST-oriented PLCopen interchange from
the narrower Ladder profile. After reading them, you should know which XML
parts truST preserves, which vendor metadata still needs review, and when to
validate the generated project through build/runtime instead of trusting an XML
round trip alone.

Budget 20-30 minutes if you read both sections. Success means you can name what
survives import/export, what must be checked after a vendor round trip, and
which build/runtime proof is needed before claiming a migration is complete.

## ST-Complete Compatibility

--8<-- "docs/guides/PLCOPEN_INTEROP_COMPATIBILITY.md:3"

## Ladder Interop Profile

--8<-- "docs/guides/PLCOPEN_LD_INTEROP.md:3"

## Related

- [Migrate Into truST](index.md)
- [CODESYS And TwinCAT](codesys-twincat.md)
- [Ladder Editor](../develop/visual-editors/ladder.md)
- [PLCopen XML example](../examples/vendor-profiles.md)
