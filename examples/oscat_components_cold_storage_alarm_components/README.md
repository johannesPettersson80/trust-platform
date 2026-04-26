# Cold Storage Alarm Components

This example separates freezer telemetry conversion from alarm state by using
`UnitConverter` and `HysteresisSwitch`.

The OOP version makes the conversion service stateless and keeps alarm memory in
one scan object. That split is useful when several chambers share one converter
but each chamber owns its own alarm state.

Compare it with `examples/oscat_components_cold_storage_alarm_classic`.

## Pattern Shown

- Service object plus stateful alarm object.
- Interface-backed hysteresis alarm.
- Negative process values without signed typed literals.

## Validate

```bash
trust-runtime test --project examples/oscat_components_cold_storage_alarm_components
```
