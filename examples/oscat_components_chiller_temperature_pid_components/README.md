# Chiller Temperature PID Components

This example combines `UnitConverter` and `PidController` for a chiller
return-temperature loop.

Compare it with `examples/oscat_components_chiller_temperature_pid_classic`.

## Pattern Shown

- Service object plus stateful controller.
- Interface-backed PID control.
- Classic parity test for conversion and controller output.

## Validate

```bash
trust-runtime test --project examples/oscat_components_chiller_temperature_pid_components
```
