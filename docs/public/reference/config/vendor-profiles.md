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

## Behavior differences

| Profile family | Formatting bias | Diagnostic defaults | Notes |
| --- | --- | --- | --- |
| `codesys` / `beckhoff` / `twincat` | CODESYS/TwinCAT-family expectations | standard warning set stays on | best default for CODESYS-style authoring |
| `siemens` | Siemens-oriented spacing and keyword style | disables missing-ELSE and implicit-conversion warnings by default | also accepts Siemens-style `#` local references |
| `mitsubishi` / `gxworks3` | Mitsubishi formatting expectations | warning set remains enabled unless overridden | supports `DIFU` / `DIFD` aliases |

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
