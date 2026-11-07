### Fixed

- A typed conversion function takes an untyped literal as its input when the value fits, as every
  other call does (specification 05 §5.2.1): `REAL_TO_INT(2.5)` compiles, and gives 3 under the
  CODESYS profiles and 2 under the IEC rules, where it failed with E205.
