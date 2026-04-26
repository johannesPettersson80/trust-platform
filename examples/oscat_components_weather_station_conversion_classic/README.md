# Weather Station Conversion Classic

This example uses classic OSCAT constants, wind-speed conversion, and direction
helpers for weather-station telemetry.

Compare it with `examples/oscat_components_weather_station_conversion_components`.

## Pattern Shown

- Classic constants loader.
- Classic speed and direction helpers.
- ST test for constants and conversion helper availability.

## Validate

```bash
trust-runtime test --project examples/oscat_components_weather_station_conversion_classic
```
