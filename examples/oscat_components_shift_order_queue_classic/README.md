# Shift Order Queue Classic

This example uses classic `FIFO_16` to carry shift orders in arrival order.

The classic API exposes the queue write and read pulses directly.

Compare it with `examples/oscat_components_shift_order_queue_components`.

## Pattern Shown

- Classic FIFO module.
- First-in-first-out order preservation.
- ST test for two queued shift orders.

## Validate

```bash
trust-runtime test --project examples/oscat_components_shift_order_queue_classic
```
