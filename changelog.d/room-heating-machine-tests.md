### Added

- The example `room_heating` has machine tests: `machine-tests/` runs its program and its simulated
  plant under `cargo test` with trust-sim, presses Start and Stop as the Heating panel does, and
  checks the pump before the heater, the pump's 30 s run-on after a stop and the low-water
  protection. CI runs them, and the guide "Machine Tests" builds them step by step.
