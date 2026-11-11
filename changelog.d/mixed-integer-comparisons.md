### Fixed

- Under the CODESYS profile a comparison of two integers of different families, signed, unsigned or bit string,
  `LEN(sTopic) > uiLen` with a `UINT` constant for example, now builds and compares the values, so `-1 < 65535` is
  TRUE. The build refused it with "a comparison of INT with UINT is not compiled yet" (#452).
