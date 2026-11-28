### Fixed

- PLCopen Motion: `MC_Power` with `Enable` FALSE now switches the virtual axis's power stage off. `Status`
  goes FALSE, the axis goes to Disabled (an axis in ErrorStop keeps that state until `MC_Reset`), and
  every on-going command, active or buffered, ends with `CommandAborted`, as PLCopen Part 1 §2.1 requires.
  Before, the axis stayed powered and in its state for ever, so a program that removes the power on a
  fault, as the single-axis demo's fault step does, did not fail closed. The gallery starter's copy of
  the library has the fix too.
