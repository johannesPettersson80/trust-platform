### Added

- `trust-runtime simulate --project <folder>` runs a project's program and its simulation for a
  machine test, on a clock that only the test moves: `simulator.advance` runs a number of cycles,
  or the cycles due within a duration, and stops at a fault, which the answer names. It answers the
  control protocol on its standard input and output, opens no endpoint and starts no web server or
  I/O driver, so every run of a test gives the same result (specification 11, "Simulator for
  machine tests").
