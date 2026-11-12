### Fixed

- Under the CODESYS profile, an initial value that reads a string's byte, such as `first : BYTE := cText[0]`,
  now starts at the byte at that offset, as the program reads it. The build refused it, with "array index 0 out
  of bounds" or "type mismatch".
