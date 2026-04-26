# Irrigation Sun Clock Components

This example uses `CalendarClock` for the irrigation sun-clock use case.

The OOP version keeps location and timezone setup in `Configure()` and exposes
the current sun/calendar state through properties after each `Update()`.

Compare it with `examples/oscat_components_irrigation_sun_clock_classic`.

## Pattern Shown

- Calendar service object.
- Read-only sun and local-time snapshots.
- Classic parity test for a fixed summer timestamp.

## Validate

```bash
trust-runtime test --project examples/oscat_components_irrigation_sun_clock_components
```
