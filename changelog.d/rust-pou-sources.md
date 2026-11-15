### Added

- In VS Code, Go to Definition of a Rust POU opens its Rust item in the project's crate, from its
  `#[trust_fb]` or `#[trust_function]` attribute to its name, and of a method of a Rust function
  block its `fn` from its `#[trust_method]`; Go to Declaration opens the generated Structured Text
  declaration; VS Code keeps the generated `src/generated/rust_pous.st` read-only,
  whose header names the Rust source to change (specification 48 RU-31, RU-50).
