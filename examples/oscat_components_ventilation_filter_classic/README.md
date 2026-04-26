# Ventilation Filter Classic

This example uses two classic `FT_PT1` instances to filter intake and exhaust
airflow.

The classic version makes each FB instance explicit. The caller is responsible
for keeping each instance separate and for passing tuning values on every scan.

Compare it with `examples/oscat_components_ventilation_filter_components`.

## Pattern Shown

- Two independent classic filter FB instances.
- Per-call tuning values.
- Test protects against accidental state sharing.

## Validate

```bash
trust-runtime test --project examples/oscat_components_ventilation_filter_classic
```
