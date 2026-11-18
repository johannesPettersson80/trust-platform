### Added

- `trust-runtime new --rust` creates a Structured Text project with a Rust crate in `rust/`, whose
  function block `FILTER_EMA` `src/Main.st` calls, and `trust-runtime new --rust-first` a Rust-first
  project, whose crate is the project and whose `trust.toml` is its configuration (specification 48
  RU-51, §14). Each writes its generated Structured Text, so the project builds as it is created,
  and a `rust-toolchain.toml` with the toolchain bundle's Rust version, so that your own rustup
  gives rust-analyzer and `cargo test` the same compiler.
