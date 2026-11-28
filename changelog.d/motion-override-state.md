### Fixed

- PLCopen Motion: `MC_SetOverride` with a velocity factor of 0.0 no longer puts the axis in Stopping. It stops the
  virtual axis at once and leaves its state as it was, as PLCopen Part 1 §3.18 requires: a moving axis keeps its
  motion state and its active command, and a Disabled axis or one in ErrorStop stays so. Before, it wrote Stopping
  into any axis, a faulted or unpowered one included. The gallery starter's copy of the library has the fix too.
