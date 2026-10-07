# Vendor Profiles Reference

Set the exact profile in `trust-lsp.toml`:

```toml
[project]
vendor_profile = "siemens"
```

## Accepted profile values

| Value | Primary use |
| --- | --- |
| `codesys` | CODESYS-style authoring and formatting |
| `beckhoff` | Beckhoff-oriented authoring defaults |
| `twincat` | TwinCAT-oriented authoring defaults |
| `siemens` | Siemens SCL compatibility path |
| `mitsubishi` | Mitsubishi GX Works3 compatibility path |
| `gxworks3` | alias for Mitsubishi profile |
| `iec` | no vendor defaults; IEC conversion rules |

## Behavior differences

| Profile family | Formatting bias | Diagnostic defaults | Conversion rules | Notes |
| --- | --- | --- | --- | --- |
| `codesys` | CODESYS/TwinCAT-family expectations | standard warning set stays on | CODESYS | best default for CODESYS-style authoring |
| `beckhoff` / `twincat` | CODESYS/TwinCAT-family expectations | standard warning set stays on | IEC | |
| `siemens` | Siemens-oriented spacing and keyword style | disables missing-ELSE and implicit-conversion warnings by default | IEC | also accepts Siemens-style `#` local references |
| `mitsubishi` / `gxworks3` | Mitsubishi formatting expectations | warning set remains enabled unless overridden | IEC | supports `DIFU` / `DIFD` aliases |

## Conversion rules

`vendor_profile = "codesys"` also makes the type conversion functions follow CODESYS
when the project runs: `REAL_TO_INT(2.5)` is 3, `DINT_TO_SINT(128)` is -128 instead of an
overflow fault, `REAL_TO_DWORD` and `DWORD_TO_REAL` convert the number, and
`BYTE_TO_BOOL` is `TRUE` for any non-zero value. `trust-runtime build`/`check`/`run`,
`trust-dev test`, debug sessions and `trust-harness --project` apply it; every other
profile, `iec` and a missing `vendor_profile` keep the IEC rules. Some of these CODESYS
rules conflict with IEC 61131-3 (`docs/IEC_DEVIATIONS.md`, DEV-023 to DEV-025); see the
[standard functions specification](../specifications/07-standard-functions.md), section 2.10.

A `vendor_profile` that is not listed above stops these tools from running the project,
and the language server reports `C006` in `trust-lsp.toml`.

## Switching profiles safely

1. change `vendor_profile`
2. reformat representative files
3. review diagnostics
4. run build and validate
5. only then treat the profile change as complete

## Related

- [Vendor Profiles](../../develop/vendor-profiles.md)
- [Siemens](../../migrate/siemens.md)
- [Mitsubishi](../../migrate/mitsubishi.md)
