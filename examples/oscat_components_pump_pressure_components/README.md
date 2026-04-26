# Pump Pressure Components

This example composes `PidController` and `HysteresisSwitch` for the same pump
station behavior as the classic example.

The control and alarm objects have their own interfaces and state. That makes it
straightforward to pass them into a larger pump-station FB without exposing the
classic OSCAT pulse and parameter surface everywhere.

Compare it with `examples/oscat_components_pump_pressure_classic`.

## Pattern Shown

- Composition of two OOP component objects.
- Separate configuration for control limits and alarm limits.
- Classic parity test for both controller output and alarm state.

## Validate

```bash
trust-runtime test --project examples/oscat_components_pump_pressure_components
```
