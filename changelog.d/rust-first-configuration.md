### Added

- Rust-first projects (specification 48 §14): `trust.toml`'s `[rust] crate = "."` makes the
  project's own folder its Rust crate, and `trust-dev rust declarations` writes the configuration
  that its `[resource]`, `[tasks]`, `[programs]`, `[globals]` and `[bindings]` declare into
  `src/generated/trust/configuration.st`, beside the generated declarations; the build runs it and
  refuses a configuration that is not what `trust.toml` declares (RU-65 to RU-67).
