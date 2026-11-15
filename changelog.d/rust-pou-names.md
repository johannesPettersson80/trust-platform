### Added

- The attributes of a Rust POU refuse a name that is a reserved word of Structured Text (`at`,
  `step`, `time` and the rest), where it stands in the Rust source, instead of writing a declaration
  that does not parse; `NULL`, `ON` and `SET` still name variables, fields, parameters and
  enumeration values, as specification 01 §3 allows (specification 48 RU-22).
