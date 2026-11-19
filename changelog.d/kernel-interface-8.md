### Changed

- Compiled programs use kernel interface 8, which gives a debug image the kernel's monotonic clock,
  so that it can time each call of its Rust POUs against the POU's budget (specification 44 §8.2,
  specification 48 RU-49). A runtime or board firmware of an earlier release refuses a program that
  this release builds, and this release refuses one that an earlier release built, each with its
  reason: rebuild the project, and install this release's firmware on a board with
  `trust-runtime firmware install`.
