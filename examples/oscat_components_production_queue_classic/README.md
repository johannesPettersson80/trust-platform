# Production Queue Classic

This example uses OSCAT `FIFO_16` directly to pass order identifiers from one
production step to the next.

The classic call style exposes the read/write/reset pulses at every call site.
That is close to the original OSCAT API and useful when you need exact control
over each queue edge.

Compare it with `examples/oscat_components_production_queue_components`.

## Pattern Shown

- Classic FIFO memory module.
- Explicit read and write pulses.
- Test locks the push/pop order ID behavior.

## Validate

```bash
trust-runtime test --project examples/oscat_components_production_queue_classic
```
