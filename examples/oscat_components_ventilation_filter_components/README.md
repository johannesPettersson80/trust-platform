# Ventilation Filter Components

This example uses two `Pt1Filter` component instances for intake and exhaust
airflow.

The component version moves tuning into `Configure()` and keeps the scan path to
`Update(Sample := ...)`. The paired test is intentionally about state isolation:
two filter objects must not share internal OSCAT state.

Compare it with `examples/oscat_components_ventilation_filter_classic`.

## Pattern Shown

- Multiple instances of the same scan object.
- Configuration separated from scan input.
- Read-only `Output` property after update.

## Validate

```bash
trust-runtime test --project examples/oscat_components_ventilation_filter_components
```
