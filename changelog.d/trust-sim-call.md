### Added

- `trust-sim` calls a method between two cycles, as a screen's command does: a machine test presses a
  sequence's Start with `plant.call("Main.Heating", "Plant.ISequence", "Start",
  Inputs::new().with("Setpoint", 21.5))`, and the next cycle sees what the method changed. A method
  without a result returns `()` (specification 48 RU-58).
