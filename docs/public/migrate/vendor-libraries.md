# Vendor Libraries

Use this guide when:

- you need symbol visibility or migration help
- you do not want to treat the imported pack as normal editable source

The guide below is about engineering contracts, not pretending every vendor
runtime library is implemented. After reading it, you should know when to use
local stubs, what a stub can safely prove, and where real vendor behavior still
needs site-specific validation.

Budget 15-20 minutes. Success means you can decide whether a vendor dependency
should become a local stub, a supported truST library dependency, or an
explicit commissioning risk that cannot be proven by symbol visibility alone.

## Compatibility Guide

--8<-- "docs/guides/VENDOR_LIBRARY_COMPATIBILITY.md:3"

## Related

- [Libraries overview](../develop/libraries/index.md)
- [Migrate Into truST](index.md)
- [Vendor profile examples](../examples/vendor-profiles.md)
