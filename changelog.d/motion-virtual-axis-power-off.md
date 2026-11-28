### Fixed

- PLCopen Motion: `MC_Power` with `Enable` FALSE now switches the virtual axis's power stage off. `Status` goes
  FALSE and the axis goes to Disabled; every on-going move, active or buffered, ends with `CommandAborted`, as
  PLCopen Part 1 §2.1 requires. An axis in ErrorStop keeps that state until `MC_Reset`, and an axis error that
  no block has reported yet takes the axis to ErrorStop, with the moves ending in that error. Before, the axis
  stayed powered and in its state for ever, so a program that removes the power on a fault, as the single-axis
  demo's fault step does, did not fail closed. `MC_Stop` no longer leaves an unpowered axis in Stopping. The
  gallery starter's copy of the library has the fix too.
