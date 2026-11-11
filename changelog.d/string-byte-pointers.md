### Fixed

- Under the CODESYS profile `ADR` of a string where a `POINTER TO BYTE` or a `POINTER TO ARRAY[l..u] OF BYTE` is
  wanted, as WAGO's `MemCopy` and JSON parser take it, now builds: the pointer names the string's bytes from its first
  one, and an element read through it past the string's bytes is refused with `IndexOutOfBounds`. A pointer to a
  `CHAR` also works as one to a `BYTE`. The build refused these with "a variable of another type" (#453).
