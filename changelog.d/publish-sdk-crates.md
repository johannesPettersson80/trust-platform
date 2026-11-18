### Added

- Each release publishes the SDK's crates (`trust-plc-abi`, `trust-plc-lib`, `trust-plc-decl`,
  `trust-plc-macros` and `trust-plc`) to crates.io at its version, after the release is verified, so
  that your own cargo and rust-analyzer resolve the version a project's `Cargo.toml` names. The
  publication starts once the repository holds the crates.io token; until then a release says that
  it published no crate (specification 24, "Publishing the SDK's crates"; specification 48 RU-12).
