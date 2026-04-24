# Siemens

Use it when:

- the source code comes from Siemens/TIA
- you need Siemens-style local references and formatting expectations
- you are preparing `.scl` import/export workflows

The baseline explains the syntax/style boundary; the import tutorial gives the
first practical path. After reading both, you should know whether to set a
Siemens-oriented vendor profile, what to inspect after import, and where manual
library or hardware modeling is still required.

Budget 20-30 minutes for the baseline plus tutorial. Success means you can
decide whether Siemens SCL import is a syntax/style task, a vendor-library
stub task, or a hardware/config modeling task before you start editing code.

## Compatibility Baseline

--8<-- "docs/guides/SIEMENS_SCL_COMPATIBILITY.md:3"

## TIA Import Tutorial

--8<-- "docs/guides/SIEMENS_TIA_SCL_IMPORT_TUTORIAL.md:3"

## Related

- [Migrate Into truST](index.md)
- [Vendor Profiles](../develop/vendor-profiles.md)
- [Vendor profile examples](../examples/vendor-profiles.md)
