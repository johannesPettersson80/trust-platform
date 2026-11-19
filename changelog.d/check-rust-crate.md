### Fixed

- `trust-runtime check`, and with it VS Code's **Compile**, failed on every project with Rust POUs,
  refusing each one as "without its Rust item": it read the project without its Rust crate. It now
  reads the crate as the build does (specification 11 §6.8.1c).

### Added

- `trust-runtime check` checks a project's Rust crate with the toolchain's cargo, as the build
  compiles it, without building the program: the crate's errors and warnings are issues where they
  are, as the build reports them, and a warning leaves the check passed (specification 48 RU-44).
