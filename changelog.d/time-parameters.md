### Added

- The runtime's parameter transactions (`control.parameters.status`, `apply` and `rollback`) take a `TIME` parameter, a
  time that a law keeps as `TIME`, such as the reset time `tTn` of TwinCAT's `FB_CTRL_PI`. Its unit is `s`: its values
  and bounds are seconds, whole nanoseconds as `TIME` holds them, and it reads back in seconds. A controller transfer
  (`control.transfer.*`) takes such a time as a guard, which it reads in seconds and never writes.
