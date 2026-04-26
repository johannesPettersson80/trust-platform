# Irrigation Sun Clock Classic

This example uses classic OSCAT calendar calculation to decide whether night
watering is allowed.

The classic version passes a mutable `CALENDAR` record and holiday table through
`CALENDAR_CALC` each scan.

Compare it with `examples/oscat_components_irrigation_sun_clock_components`.

## Pattern Shown

- Classic calendar/sun calculation.
- Outdoor irrigation time window.
- ST test for a fixed summer timestamp.

## Validate

```bash
trust-runtime test --project examples/oscat_components_irrigation_sun_clock_classic
```
