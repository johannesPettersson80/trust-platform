# OpenPLC

If your target is Siemens, Mitsubishi, or CODESYS/TwinCAT, use the matching
vendor page instead.

Use this page for OpenPLC-oriented projects and examples. The guide below
explains the ST-focused interoperability path, what to inspect after import,
and why target/runtime assumptions still need validation before deployment.

Budget 10-20 minutes for the guide and more time for target validation. Success
means you can separate portable ST from OpenPLC-specific runtime assumptions,
then choose whether PLCopen interchange, direct ST review, or a new truST
project layout is the next step.

Use [PLCopen](plcopen.md) next if the source project can be exported through an
open interchange artifact.

## Compatibility Guide

--8<-- "docs/guides/OPENPLC_INTEROP_V1.md:3"

## Related

- [Migrate Into truST](index.md)
- [PLCopen](plcopen.md)
- [Vendor profile examples](../examples/vendor-profiles.md)
