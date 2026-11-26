### Fixed

- Tutorial 19 (safety commissioning) walks through its steps with `trust-runtime ctl` commands and the output of a real
  run, from the input that the output follows to the division by zero that drives the safe state and the cold restart
  that brings the program back. It says that its loopback driver touches no real I/O, so the run proves the runtime's
  own reaction, and that the steps are repeated with the target's I/O driver before they count as commissioning
  evidence. CI runs its steps on tutorial 13's project.
