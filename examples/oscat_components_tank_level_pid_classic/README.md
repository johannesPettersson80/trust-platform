# Tank Level PID Classic

This is the direct OSCAT version of the tank-level controller example.

The program calls `CTRL_PID` directly every scan and reads the `Y` and `LIM`
outputs from the classic function block. This is the shortest form when an
application only needs one controller and the scan code is already local to the
device.

Compare it with `examples/oscat_components_tank_level_pid_components`, where
the same behavior is wrapped behind `PidController` and `IPidController`.

## Pattern Shown

- Classic function-block call.
- All tuning and input values are passed at the call site.
- The test fixes the manual-output case so the classic and component examples
  can be compared without timing-sensitive integral behavior.

## Validate

```bash
trust-runtime test --project examples/oscat_components_tank_level_pid_classic
```
