### Added

- In `trust-runtime simulate`, `CURRENT_DT()` reads the simulator's logical time from
  `DT#1970-01-01-00:00:00`, so that every run of a machine test reads the same date and time
  (specification 07, "Runtime Clock Sources"; specification 11, "Simulator for machine tests").
