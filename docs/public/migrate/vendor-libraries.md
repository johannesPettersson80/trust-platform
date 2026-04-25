# Vendor Libraries

Vendor library migration is about engineering contracts, not pretending every
vendor runtime library is implemented.

| Dependency state | Action |
| --- | --- |
| symbols needed for editing/navigation | add local stubs |
| behavior needed at runtime | use a supported truST library or implement it |
| behavior depends on a vendor runtime or device | keep it as a commissioning risk until proven on target |

## Compatibility Guide

--8<-- "docs/guides/VENDOR_LIBRARY_COMPATIBILITY.md:3"

## Related

- [Libraries overview](../develop/libraries/index.md)
- [Migration While Programming](index.md)
- [Vendor profile examples](../examples/vendor-profiles.md)
