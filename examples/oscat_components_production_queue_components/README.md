# Production Queue Components

This example uses `DwordFifo16` through the `IDwordQueue` interface to move
order identifiers between production steps.

The component API turns read/write pulse details into `Push()` and `TryPop()`.
That makes queue use easier to review and safer to copy into business logic.

Compare it with `examples/oscat_components_production_queue_classic`.

## Pattern Shown

- Interface facade over a stateful OSCAT memory block.
- Boolean command results that represent queue success or capacity failure.
- `Value`, `Empty`, and `Full` as read-only snapshots after commands.

## Validate

```bash
trust-runtime test --project examples/oscat_components_production_queue_components
```
