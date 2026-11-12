### Fixed

- Under the CODESYS profile, bytes written into a string through a pointer, `ADR(s)` taken as a `POINTER TO BYTE` or a
  `POINTER TO ARRAY[..] OF BYTE`, now change its text, which ends at its first 0 byte as CODESYS reads it. A copy
  loop like the vendor `MemCopy` into an empty string gave the empty text, and a 0 written into a text did not end it
  (#453).
