### Added

- Control Studio serves a law in standard form whose reset time or derivative time is a `TIME` variable, such as `tTn : TIME` of
  TwinCAT's `FB_CTRL_PI`. Setup offers the `TIME` variables for the times, Validate and Apply use the same whole milliseconds, and
  Keep in project writes the time as a TIME literal, `T#<n>ms`.
