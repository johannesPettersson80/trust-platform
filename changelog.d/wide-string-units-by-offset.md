### Changed

- Under the CODESYS profile, `ws[i]` of a `WSTRING` is now the `WORD` at offset `i`, counted from 0, as in CODESYS,
  instead of the `WCHAR` at position `i` counted from 1. `ADR(ws[i])` is a `POINTER TO WORD` that names it, a write
  through it changes the text, which ends at its first 0, and `ws[n]` of a `WSTRING[n]` reads 0. The IEC profile keeps
  the `WCHAR` from position 1.
