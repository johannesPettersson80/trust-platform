### Added

- Rust programs (specification 48 §7): a struct marked `#[trust_program(name = "...")]` with its
  inputs, outputs, internal and `#[retain]` variables and `impl Body` is declared as a `PROGRAM`,
  which a configuration runs like any program: its program connections bind its inputs and outputs,
  its retained variables lie in the retained area, and its budget counts against its task (RU-35,
  RU-47). A Rust program has no in-outs, no `EN`/`ENO` and no methods (RU-35, RU-37); its body in
  Rust goes beyond IEC 61131-3 §4.3 (`docs/IEC_DEVIATIONS.md` DEV-048).
