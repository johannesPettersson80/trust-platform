### Added

- A Rust POU's variables may be of the crate's own structures (specification 48 RU-25): a Rust
  `struct` with `pub` fields marked `#[trust_struct(name = "POINT")]` is declared as a structure of
  Structured Text with the same fields and initial values, and the generated code carries it field by
  field, inside other structures and arrays too.
