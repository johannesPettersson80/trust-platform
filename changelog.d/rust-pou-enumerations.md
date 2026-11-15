### Added

- A Rust POU's variables may be of the crate's own enumerations (specification 48 RU-25): a Rust
  `enum` marked `#[trust_enum(name = "MODE")]`, each variant with its value, is declared as a closed
  enumeration of Structured Text with the same names and values, and a variable of it starts at a
  variant such as `#[init(Mode::Auto)]`.
