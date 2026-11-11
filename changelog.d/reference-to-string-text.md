### Fixed

- Under the CODESYS profile a `REFERENCE TO STRING` or `REFERENCE TO WSTRING` read where a text is needed, as a
  function block's input, the source of an assignment or an operand of `=` and `<>`, now builds and reads the text of
  the string it names. The build refused it with "a value that is no text where a text is needed" (#454).
