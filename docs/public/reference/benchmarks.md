# Benchmarks

Use this page when you need runtime-facing benchmark or performance reference
material.

## Built-in benchmark surface

```bash
trust-runtime bench <command>
```

Available benchmark commands:

| Command | Purpose |
| --- | --- |
| `project` | measure cycle latency and throughput for a real project folder |
| `t0-shm` | measure same-host T0 shared-memory latency and overrun counters |
| `mesh-zenoh` | measure synthetic mesh pub/sub and query/reply behavior |
| `dispatch` | measure runtime-cloud dispatch, preflight, and audit correlation latency |

## Example commands

```bash
trust-runtime bench project --project examples/plcopen_motion_single_axis_demo --watch g_motion_demo_completed_sequences --output json
trust-runtime bench t0-shm --samples 2000 --output json
trust-runtime bench mesh-zenoh --samples 1000 --loss-rate 0.01 --reorder-rate 0.02
trust-runtime bench dispatch --fanout 4 --output table
```

## Motion benchmark pack

The canonical project pack for motion-library benchmarking is:

- `examples/plcopen_motion_single_axis_benchmarks`

Use it when you need to compare:

- runtime floor
- constants-only library overhead
- status and readback overhead
- active move command cost
- full demo cost

## Related

- [Libraries and motion examples](../examples/libraries-and-motion.md)
- [PLCopen motion library](../develop/libraries/plcopen-motion.md)
