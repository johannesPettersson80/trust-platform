# Maintenance Stack Classic

This example uses OSCAT `STACK_16` directly for last-in-first-out maintenance
work orders.

The direct API exposes stack pulse behavior clearly, but the scan code must
remember which call writes and which call reads.

Compare it with `examples/oscat_components_maintenance_stack_components`.

## Pattern Shown

- Classic stack memory module.
- Explicit write/read pulses.
- Test verifies that the newest work order is returned first.

## Validate

```bash
trust-runtime test --project examples/oscat_components_maintenance_stack_classic
```
