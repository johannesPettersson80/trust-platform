# Greenhouse Temperature Classic

This example uses classic OSCAT calls to normalize greenhouse temperature and
raise a ventilation request when the room exceeds the hysteresis high limit.

The direct style is compact, but conversion and switching policy are mixed in
the scan body.

Compare it with `examples/oscat_components_greenhouse_temperature_components`.

## Pattern Shown

- Classic `TEMPERATURE` conversion block.
- Classic `HYST` switch used directly.
- Deterministic ST test for conversion and switching behavior.

## Validate

```bash
trust-runtime test --project examples/oscat_components_greenhouse_temperature_classic
```
