### Fixed

- PLCopen Motion: a motion block whose command was aborted while the kernel's three records of aborted commands
  were full no longer stays Busy for ever. It finds its command neither active nor queued and ends it as aborted,
  with `CommandAborted` (a queued parameter write with `Error`). The records fill up when blocks whose commands
  were aborted are not called again, for example in an unused branch of a `CASE`.
