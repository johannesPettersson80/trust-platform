### Added

- Rust function blocks implement Structured Text interfaces: `#[trust_fb(name = "COUNTER",
  implements = "I_COUNTER")]` writes `IMPLEMENTS I_COUNTER` into the generated declaration, the build
  checks the block's methods against each prototype as it checks a Structured Text block, and a call
  through the interface reaches the Rust method; a task counts its budget for such a call
  (specification 48 RU-33).
