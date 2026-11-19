### Added

- Control Studio's isolated simulation runs and searches run a project with Rust POUs: the run's
  snapshot carries the files of the project's Rust crate, at most 256 files and 4 MiB, and the run
  builds the crate with the program. Its registry dependencies are read where the project's cargo
  configuration puts them, vendored sources included. A larger crate, or one with a dependency
  outside the crate's folder, is reported as not ready, with the reason (Control Studio protocol
  annex E8.2).
