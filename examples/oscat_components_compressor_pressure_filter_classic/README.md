# Compressor Pressure Filter Classic

This example filters compressor pressure with classic `FT_PT1` before applying
a classic `HYST` alarm.

Compare it with `examples/oscat_components_compressor_pressure_filter_components`.

## Pattern Shown

- Classic signal filter feeding an alarm block.
- Pressure-alarm process pattern.
- Test proves the filtered value drives the alarm.

## Validate

```bash
trust-runtime test --project examples/oscat_components_compressor_pressure_filter_classic
```
