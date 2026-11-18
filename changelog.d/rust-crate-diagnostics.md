### Added

- `trust-runtime build` reports each error and warning of a project's Rust crate where it is. The
  report names the crate's files as the project does (`rust/src/lib.rs`, not the compiler's
  `project/src/lib.rs`). With `--json` or `--ci`, a failed build gives each error as an issue with
  its file, line and column, the compiler's code and the source `truST (rustc)`. A successful build
  writes the crate's warnings and lists them as issues of its JSON; a warning still does not fail the
  build (specification 48 RU-44, specification 11 §6.8.1d).
