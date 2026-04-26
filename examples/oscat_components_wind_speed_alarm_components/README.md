# Wind Speed Alarm Components

This example adapts `UnitConverter` and `HysteresisSwitch` into a small
weather-station alarm.

The conversion is a service call and the alarm is a stateful scan object. That
split is useful when a larger station controller needs to swap conversion or
alarm policy independently.

Compare it with `examples/oscat_components_wind_speed_alarm_classic`.

## Pattern Shown

- Adapter-style composition of a service and scan object.
- Interface-backed alarm object.
- Parity with classic conversion and hysteresis behavior.

## Validate

```bash
trust-runtime test --project examples/oscat_components_wind_speed_alarm_components
```
