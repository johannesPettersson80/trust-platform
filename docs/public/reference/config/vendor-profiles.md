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
| `beckhoff` / `twincat` | CODESYS/TwinCAT-family expectations | standard warning set stays on | CODESYS | TwinCAT is built on CODESYS |
| `siemens` | Siemens-oriented spacing and keyword style | disables missing-ELSE and implicit-conversion warnings by default | IEC | also accepts Siemens-style `#` local references |
| `mitsubishi` / `gxworks3` | Mitsubishi formatting expectations | warning set remains enabled unless overridden | IEC | supports `DIFU` / `DIFD` aliases |

## Conversion rules

`vendor_profile` `codesys`, `beckhoff` or `twincat` also makes the type conversion
functions follow CODESYS when the project runs: `REAL_TO_INT(2.5)` is 3,
`DINT_TO_SINT(128)` is -128 instead of an overflow fault, `REAL_TO_DWORD` and
`DWORD_TO_REAL` convert the number, and `BYTE_TO_BOOL` is `TRUE` for any non-zero value.
`trust-runtime build`/`check`/`run`, `trust-dev test`, debug sessions and
`trust-harness --project` apply it; `siemens`, `mitsubishi`, `gxworks3`, `iec` and a
missing `vendor_profile` keep the IEC rules. Some of these CODESYS
rules conflict with IEC 61131-3 (`docs/IEC_DEVIATIONS.md`, DEV-024 to DEV-026); see the
[standard functions specification](../specifications/07-standard-functions.md), section 2.10.

A `vendor_profile` that is not listed above stops these tools from running the project,
and the language server reports `C006` in `trust-lsp.toml`.

## Pointer bindings

Under `codesys`, `beckhoff` and `twincat`, a pointer whose target differs from the formal's or the
variable's only as CODESYS allows binds: `POINTER TO CHAR` and `POINTER TO BYTE` either way, and a
string's address against a `POINTER TO BYTE` or a `POINTER TO ARRAY[i..j] OF BYTE`. When the source
is an explicit `ADR(...)`, such as `MemCopy(pSource := ADR(sPayload))`, the binding is silent, as
CODESYS compiles it. Any other source, such as a pointer variable, gives the warning `E205` for an
argument or `E203` for an assignment. The other profiles report these bindings as errors
(`docs/IEC_DEVIATIONS.md`, DEV-039).

## String bytes

Under `codesys`, `beckhoff` and `twincat`, a string's bytes behave as in CODESYS:

- `s[i]` of a `STRING` is the `BYTE` at offset `i`, counted from 0, and `ADR(s[i])` is a
  `POINTER TO BYTE` that names it, as in `MemCopySecure(pDest := ADR(sJson[1]), ...)`.
- A string's text ends at its first 0 byte. A byte written through `s[i]` or through a pointer to
  the string's bytes changes the text: a 0 ends it, and another byte written over its end extends it
  to the next 0.
- `s[n]` of a `STRING[n]` reads 0, the 0 that CODESYS keeps after a full text.
- `ws[i]` of a `WSTRING` is likewise the `WORD` at offset `i`, its code unit there, and
  `ADR(ws[i])` is a `POINTER TO WORD` that names it. Its text ends at its first 0 unit, and
  `ws[n]` of a `WSTRING[n]` reads 0.

The other profiles keep the IEC rule: `s[i]` is the `CHAR` at position `i`, counted from 1, and
`ws[i]` the `WCHAR` there (`docs/IEC_DEVIATIONS.md`, DEV-046).

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
