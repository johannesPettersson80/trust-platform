### Fixed

- Saving a simulated plant whose actuator is on-off (`kind = "on-off"`) no longer deletes the actuator
  from `simulation.toml`. Before, reopening and saving such a plant, even unchanged, removed the whole
  actuator table. The plant editor now keeps the actuator's kind and its minimum on and off times,
  which it does not edit, and takes its own actuator fields from what you entered.
