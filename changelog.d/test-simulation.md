### Added
- `{attribute 'test_simulation'}` on a multi-cycle test runs the project's `simulation.toml` in lockstep with the test, as
  `trust-runtime run --simulation` runs it: the plants publish their feedback before each cycle and read the commands and
  advance to the cycle's time after it, in simulated time from zero. A test of a loop checks it against its plant, cycle by
  cycle. Such a test runs on this computer; on a device it ends with an error that says so (specification 22, "A plant
  in the loop").
