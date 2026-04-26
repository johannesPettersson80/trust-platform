# Compressor Pressure Filter Components

This example composes `Pt1Filter` and `HysteresisSwitch` for compressor pressure
monitoring.

Compare it with `examples/oscat_components_compressor_pressure_filter_classic`.

## Pattern Shown

- Filter object feeding an alarm object.
- Separate tuning and alarm thresholds.
- Classic parity test for both stages.

## Validate

```bash
trust-runtime test --project examples/oscat_components_compressor_pressure_filter_components
```
