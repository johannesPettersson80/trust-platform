### Fixed

- Under the CODESYS profile the string functions (`LEN`, `LEFT`, `RIGHT`, `MID`, `CONCAT`, `INSERT`, `DELETE`,
  `REPLACE`, `FIND`) take a `REFERENCE TO STRING` or `REFERENCE TO WSTRING` as their text and read the string it
  names. They refused it with E205 "expected STRING or WSTRING input" (#454).
