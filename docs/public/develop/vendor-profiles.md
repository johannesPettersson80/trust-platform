# Vendor Profiles

Vendor profiles tune authoring, parsing, formatting, and migration
expectations for real PLC ecosystems.

## Pick the profile by starting point

| If your codebase looks like... | Use profile |
| --- | --- |
| CODESYS / Beckhoff / TwinCAT family ST | `codesys`, `beckhoff`, or `twincat` |
| Siemens SCL/TIA style | `siemens` |
| Mitsubishi GX Works3 | `mitsubishi` or `gxworks3` |

## What lives in a vendor path

- `vendor_profile` value in `trust-lsp.toml`
- formatting and naming expectations
- common diagnostics and quick fixes
- known migration limits
- example project links

## Safe migration checklist

1. Set the profile in `trust-lsp.toml`.
2. Reformat one representative file.
3. Review diagnostics and warnings.
4. Build and validate the project.
5. Commit the formatting/profile change separately from logic edits.

## Start here

- [CODESYS and TwinCAT](../migrate/codesys-twincat.md)
- [Siemens](../migrate/siemens.md)
- [Mitsubishi](../migrate/mitsubishi.md)

## Exact config

- [trust-lsp.toml reference](../reference/config/trust-lsp-toml.md)
- [Vendor profile reference](../reference/config/vendor-profiles.md)
