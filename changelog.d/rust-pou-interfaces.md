### Added

- Rust function blocks implement Structured Text interfaces: `#[trust_fb(name = "COUNTER",
  implements = "I_COUNTER")]` writes `IMPLEMENTS I_COUNTER` into the generated declaration, the build
  checks the block's methods against each prototype as it checks a Structured Text block, and a call
  through the interface reaches the Rust method; a task counts its budget for such a call
  (specification 48 RU-33).
- A Rust POU holds a value of a Structured Text interface as the interface's binding,
  `#[input] pub runner: st::I_RUN`, and calls its methods through it, `self.runner.START(cx)?`: the
  interface's own and those of the interfaces it extends. A call reaches the method of the class of
  the instance the value names, as Structured Text's call through the interface does, and a call
  through `NULL`, the value an interface starts at, ends the scan. `is_null()` tells `NULL`, and two
  values are equal when they name the same instance. Calls and held blocks pass interfaces too, and
  an instance the POU holds is a value of each interface its block implements,
  `st::I_RUN::from(&self.pump)`. An interface that a Rust function block implements has no binding,
  since Structured Text that Rust calls cannot call Rust; the build refuses it and names the chain
  (specification 48 RU-33, RU-39, RU-40).
