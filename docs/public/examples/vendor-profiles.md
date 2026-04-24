# Vendor Profiles

| Example folder | Use it for | Related docs |
| --- | --- | --- |
| `examples/siemens_scl_v1` | Siemens `#` symbol style, formatting, export path | [Siemens](../migrate/siemens.md) |
| `examples/mitsubishi_gxworks3_v1` | Mitsubishi `DIFU` / `DIFD` style and profile behavior | [Mitsubishi](../migrate/mitsubishi.md) |
| `examples/plcopen_xml_st_complete` | PLCopen XML import/export and round-trip workflows | [PLCopen](../migrate/plcopen.md) |
| `examples/vendor_library_stubs` | local stub strategy for missing vendor libraries | [Vendor Libraries](../migrate/vendor-libraries.md) |

## Recommended order

1. Start with `plcopen_xml_st_complete` if you are importing an existing
   codebase.
2. Use the vendor-specific tutorial (`siemens_scl_v1` or
   `mitsubishi_gxworks3_v1`) for day-to-day authoring behavior.
3. Add `vendor_library_stubs` when missing vendor symbols block editor
   productivity during migration.
