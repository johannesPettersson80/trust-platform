# Energy Normalization Classic

This example normalizes telemetry with classic OSCAT conversion FBs.

The direct style is useful for batch conversion code where the output fields of
`ENERGY` and `SPEED` are already familiar to the team.

Compare it with `examples/oscat_components_energy_normalization_components`.

## Pattern Shown

- Classic conversion FB usage.
- Output fields read directly from OSCAT structures.
- Test fixes simple engineering-unit conversions.

## Validate

```bash
trust-runtime test --project examples/oscat_components_energy_normalization_classic
```
