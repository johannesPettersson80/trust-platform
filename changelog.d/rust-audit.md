### Added

- `trust-dev rust audit` checks the crates a project's Rust POUs are built from, the scan crate
  graph the build reads, against the RustSec advisory database: each advisory that names a crate at
  the version the project builds is printed with the versions that fix it, a vulnerability fails the
  command and an informational advisory (unmaintained, unsound, a notice) is a warning. The
  database is downloaded each time and a copy kept in truST's data folder for when the computer
  cannot reach it; `--db <folder>` checks against a copy of its repository instead (specification
  48 RU-79).
