### Added

- A Rust POU calls the project's Structured Text functions (specification 48 §8): the crate names
  the bindings in its `[dependencies]`, `trust_st = { path = "../build/trust_st" }`, and calls
  `st::SCALE(cx, x, 2.0)`. The build compiles the functions the crate calls, with everything they
  reach, into the crate `trust_st` in the project's `build/trust_st/` before the crate (§9.2); their
  statements count against the scan's budget. A crate that writes `st::` without the line, a name
  of no Structured Text function, `use st::*`, and Structured Text that Rust calls and that calls
  Rust back are refused with the place and the chain (RU-39, RU-40). Functions whose inputs and
  result are elementary types, strings, one-dimensional arrays or the project's structures are
  called, a structure through its binding `st::NAME`.
