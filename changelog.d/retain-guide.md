### Added

- A guide, "Retain Across Restarts", with the example `retain_counter`: what `RETAIN` keeps over a warm restart, a
  cold restart, a stop and a power loss, how the runtime saves retained values to its retain file and reads them back
  at a warm start, and why a supervised runtime starts warm. CI runs the guide's steps against the real runtime.
