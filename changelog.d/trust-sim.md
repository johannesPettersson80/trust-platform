### Added

- `trust-sim`, a crate for machine tests of a truST project in `cargo test`. `Project::open` finds
  the installed trust-runtime and builds the project, and `simulator()` runs its program and its
  simulated plant in a simulator of the test's own. The test writes, forces and reads variables by
  their paths, runs cycles or logical time, and sees a fault as an error, with the same result on
  every run (specification 48 RU-58).
