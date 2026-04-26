# Weather Station Conversion Components

This example uses `AutomationContext` and `UnitConverter` for weather-station
telemetry.

The OOP version exposes constants/direction lookup as context behavior and wind
speed conversion as a service method. That keeps telemetry code readable while
preserving OSCAT compatibility.

Compare it with `examples/oscat_components_weather_station_conversion_classic`.

## Pattern Shown

- Context object for constants and direction helpers.
- Unit-conversion service object.
- Classic helper parity in the ST test.

## Validate

```bash
trust-runtime test --project examples/oscat_components_weather_station_conversion_components
```
