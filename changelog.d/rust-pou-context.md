### Added

- A Rust POU reads the task's logical time and the wall clock from its own context, `cx.now()` and
  `cx.wall_clock()`, which keeps the statement budget, the call depth, the breakpoints and the
  kernel's pointers out of its reach; the build refuses a crate that depends on the SDK's internal
  crates (`trust-plc-abi`, `trust-plc-lib`, `trust-plc-mem`, `trust-plc-decl`) instead of
  `trust-plc` (specification 48 RU-11, RU-16, RU-20).
