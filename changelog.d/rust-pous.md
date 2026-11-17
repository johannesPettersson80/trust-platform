### Added

- Rust POUs, the first part of specification 48: a project's crate in `rust/` declares Rust function
  blocks and functions with the `trust-plc` SDK's attributes, `trust-dev rust declarations` writes
  their Structured Text declarations into `src/generated/rust_pous.st`, and `trust-runtime build`
  compiles the crate into the program image, where Structured Text calls them as its own POUs. The
  installed toolchain bundle builds them for a user without Rust on Linux, macOS and Windows: it
  carries cargo, the SDK and the SDK's macros prebuilt, and the build compiles nothing for the
  computer itself (specification 48 §16).
