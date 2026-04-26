# Cold Storage Alarm Classic

This example uses classic OSCAT conversion and hysteresis logic to detect a
freezer chamber that is too warm.

The classic program keeps the conversion FB and alarm FB in the scan body.

Compare it with `examples/oscat_components_cold_storage_alarm_components`.

## Pattern Shown

- Classic `TEMPERATURE` conversion.
- Classic `HYST` alarm with negative setpoints expressed without signed typed
  literals.
- Real cold-storage threshold test.

## Validate

```bash
trust-runtime test --project examples/oscat_components_cold_storage_alarm_classic
```
