### Added

- `trust-runtime play --paused` and `run --paused`, and `[runtime.control] start_paused = true` in
  `runtime.toml`, start the program paused: no scan runs until `resume`, so a client can attach,
  set breakpoints or force values before the first scan. The setting needs the debug surface
  (`debug_enabled`), since `resume`, a debug operation, ends the pause. A device started from a deployment record
  with the setting holds before its first scan the same way.
