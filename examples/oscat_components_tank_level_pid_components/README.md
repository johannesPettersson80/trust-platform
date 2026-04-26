# Tank Level PID Components

This is the OOP/component version of the tank-level controller example.

`PidController` owns the OSCAT `CTRL_PID` instance. Configuration is done once
through methods, and each scan only supplies the current level and target. The
program stores the controller behind `IPidController` to show interface-based
substitution without changing the scan logic.

Compare it with `examples/oscat_components_tank_level_pid_classic`.

## Pattern Shown

- Stateful wrapper around a classic OSCAT control block.
- Read-only status properties such as `Output`, `Difference`, and `Limited`.
- Interface variable `IPidController` for code that should not depend on the
  concrete controller type.

## Validate

```bash
trust-runtime test --project examples/oscat_components_tank_level_pid_components
```
