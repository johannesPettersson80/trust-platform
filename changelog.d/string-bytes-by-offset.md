### Changed

- Under the CODESYS profile, `s[i]` of a `STRING` is now the `BYTE` at offset `i`, counted from 0, as in CODESYS,
  instead of the `CHAR` at position `i` counted from 1. A write through it changes the text as a write through a
  pointer does, and `s[n]` of a `STRING[n]` reads 0, the 0 that CODESYS keeps after a full text. The IEC profile
  keeps the `CHAR` from position 1, and a `WSTRING` keeps it under every profile.

### Fixed

- Under the CODESYS profile, `ADR(s[i])` of a string's byte now compiles, as in
  `MemCopySecure(pDest := ADR(sJson[1]), ...)`. The build refused it with "an index of a value that is no array"
  (#453).
