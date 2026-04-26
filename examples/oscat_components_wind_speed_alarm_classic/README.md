# Wind Speed Alarm Classic

This example converts wind speed to the Beaufort scale and raises a classic
`HYST` alarm when measured speed is above the high limit.

The direct style keeps the conversion function and alarm FB at the call site.

Compare it with `examples/oscat_components_wind_speed_alarm_components`.

## Pattern Shown

- Classic pure conversion function.
- Classic hysteresis alarm block.
- Simple real-world weather-station threshold.

## Validate

```bash
trust-runtime test --project examples/oscat_components_wind_speed_alarm_classic
```
