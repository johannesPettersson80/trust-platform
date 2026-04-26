# Pump Pressure Classic

This example combines classic `CTRL_PID` and `HYST` calls for a pump station:
the controller produces a pump command while the hysteresis block raises a
pressure alarm.

The direct version is compact, but controller and alarm policy are coupled in
the scan program.

Compare it with `examples/oscat_components_pump_pressure_components`.

## Pattern Shown

- Classic composition of two OSCAT FBs.
- Manual PID output for deterministic test behavior.
- Separate alarm FB using the same measured pressure.

## Validate

```bash
trust-runtime test --project examples/oscat_components_pump_pressure_classic
```
