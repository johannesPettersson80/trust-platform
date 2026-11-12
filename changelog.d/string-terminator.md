### Fixed

- Under the CODESYS profile the bytes of a string, which `ADR` names, now end its text with a 0 byte, as CODESYS lays
  out a string. After a shorter text is written over a longer one, code that scans the bytes for the terminator stops
  at the new text's end instead of reading on into the old text (#453).
