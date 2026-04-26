# Shift Order Queue Components

This example uses `DwordFifo16` as an explicit shift-order queue.

The OOP wrapper hides the read/write pulse mechanics behind `Push()` and
`TryPop()`, while preserving the classic FIFO behavior.

Compare it with `examples/oscat_components_shift_order_queue_classic`.

## Pattern Shown

- Queue interface for process order flow.
- Command methods instead of scan pulse wiring.
- ST test proving first-in-first-out behavior.

## Validate

```bash
trust-runtime test --project examples/oscat_components_shift_order_queue_components
```
