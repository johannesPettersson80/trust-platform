# Solar Lighting Clock Classic

This example uses OSCAT `CALENDAR_CALC` directly to calculate local calendar and
sun-state data for exterior lighting.

The classic API is close to the OSCAT data model: the caller owns the
`CALENDAR` record and the holiday array and passes them into the calculator.

Compare it with `examples/oscat_components_solar_lighting_clock_components`.

## Pattern Shown

- Classic calendar record plus calculator FB.
- Explicit location, UTC, offset, and DST inputs.
- Test locks the local date fields for the sample timestamp.

## Validate

```bash
trust-runtime test --project examples/oscat_components_solar_lighting_clock_classic
```
