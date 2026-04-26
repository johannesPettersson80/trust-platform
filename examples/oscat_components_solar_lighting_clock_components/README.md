# Solar Lighting Clock Components

This example uses `CalendarClock` through `ICalendarClock` to produce the same
local calendar and sun-state data as the classic OSCAT example.

The location and timezone policy are configured once. The scan then calls
`Update(Utc := ...)` and reads named properties such as `Year`, `Night`,
`SunRise`, and `SunSet`.

Compare it with `examples/oscat_components_solar_lighting_clock_classic`.

## Pattern Shown

- Stateful service object around OSCAT calendar records.
- Configuration separated from timestamp update.
- Read-only properties expose calendar and sun-state snapshots.

## Validate

```bash
trust-runtime test --project examples/oscat_components_solar_lighting_clock_components
```
