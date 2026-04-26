# Maintenance Stack Components

This example uses `DwordStack16` through `IDwordStack` for maintenance work
orders.

The component API expresses the intent as `Push()` and `TryPop()`. The last
popped work order is available through the read-only `Value` property.

Compare it with `examples/oscat_components_maintenance_stack_classic`.

## Pattern Shown

- Stack abstraction over a classic OSCAT memory block.
- Command methods for LIFO behavior.
- Snapshot property for the last popped item.

## Validate

```bash
trust-runtime test --project examples/oscat_components_maintenance_stack_components
```
