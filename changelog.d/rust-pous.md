### Added

- Rust POUs, the first part of specification 48: a project's crate in `rust/` declares Rust function
  blocks and functions with the `trust-plc` SDK's attributes, `trust-dev rust declarations` writes
  their Structured Text declarations into `src/generated/rust_pous.st`, and `trust-runtime build`
  compiles the crate into the program image, where Structured Text calls them as its own POUs. The
  build needs a development toolchain with cargo for now; the toolchain bundle gains cargo and the
  SDK in a later release (specification 48 §16).
