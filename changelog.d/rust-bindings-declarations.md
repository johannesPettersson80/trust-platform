### Added

- `trust-dev rust declarations` also writes the bindings of the Structured Text that the project's
  crate calls, `build/trust_st/`, as the build writes them (specification 48 RU-39), so that
  rust-analyzer and the crate's own cargo find `st` before the project's first build.
