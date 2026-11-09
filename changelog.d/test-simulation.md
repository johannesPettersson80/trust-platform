### Added
- `{attribute 'test_simulation'}` on a multi-cycle test runs the project's `simulation.toml` in lockstep with the test, as
  `trust-runtime run --simulation` runs it: the plants publish their feedback before each cycle and read the commands and
  advance to the cycle's time after it, in simulated time from zero. A test of a loop checks it against its plant, cycle by
  cycle (specification 22, "A plant in the loop"). With `--device` the test runs on the device and its plants on this
  computer, in the same lockstep: each cycle's step carries the plants' feedback to the device and reads its commands
  back (specification 22, "A plant in the loop on a device").
