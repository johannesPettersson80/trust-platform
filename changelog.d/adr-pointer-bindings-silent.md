### Changed

- Under the CODESYS profiles (`codesys`, `beckhoff`, `twincat`), a pointer binding that CODESYS
  accepts across targets, `CHAR` and `BYTE` or a string and a byte buffer, is silent when its
  source is an explicit `ADR(...)`, as the CODESYS compiler binds it (#295). A pointer variable as
  the source keeps the warning, and the other profiles keep the error.
