### Changed

- Control Studio's Identify, Validate's model check and Apply refuse at once while the runtime is
  paused or the debugger holds its scan, and say what holds it and how to go on (continue in the
  debugger, or resume the runtime), instead of waiting for a scan that does not come.
