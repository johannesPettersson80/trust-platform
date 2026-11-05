### Fixed

- Under the CODESYS profile, `ADR(STRING[n])` at the string's base now binds a plain `POINTER TO BYTE` formal (or a byte-pointer variable target) with the DEV-039 pointer-binding warning instead of an error, matching the vendor `MemCopy` copy-loop idiom. Every other pointee mismatch keeps its error.
