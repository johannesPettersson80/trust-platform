### Changed

- Compiled programs use kernel interface 8 and image format 6, which give a debug image the kernel's
  monotonic clock and a table of its Rust POUs, so that it can time each call of its Rust POUs
  against the POU's budget (specification 44 §5.13 and §8.2, specification 48 RU-49). A runtime or
  board firmware of an earlier release refuses a program that this release builds, and this release
  refuses one that an earlier release built, each with its reason: rebuild the project, and install
  this release's firmware on a board with `trust-runtime firmware install`.

### Fixed

- A runtime that refuses a program image of another format version names the version it reads
  (`image format version 5, this runtime reads 6: rebuild the project`); it said "is not 1",
  whatever version it read.
