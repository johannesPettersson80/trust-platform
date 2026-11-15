### Added

- The attributes of a Rust POU refuse a name that is a reserved word of Structured Text (`at`,
  `step`, `time` and the rest), where it stands in the Rust source, instead of writing a declaration
  that does not parse; `NULL`, `ON` and `SET` too, which only the CODESYS profile takes as names, so
  that a Rust POU's declarations hold under every profile (specification 48 RU-22).
