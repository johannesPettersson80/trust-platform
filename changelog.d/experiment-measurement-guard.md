### Added

- An experiment can name a measurement guard (`measurement_guard` in `control.experiment.start`): the loop's measured
  output and the band it must stay in. The runtime reads it after every execution of the experiment's task and stops the
  experiment with `measurement_outside_limits` once the measurement leaves the band, restoring its target as for any other
  failed stop; the status reports the guard and the value that stopped it (runtime protocol annex E5.3 to E5.7).
