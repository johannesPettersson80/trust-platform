# Benchmarks

## Built-in benchmark surface

```bash
trust-runtime bench <command>
```

Available benchmark commands:

| Command | Purpose | Representative target / evidence |
| --- | --- | --- |
| `project` | measure cycle latency and throughput for a project folder | motion bench gate keeps project-cycle `p95` within the configured cycle budget and requires `0` budget overruns |
| `init` | measure initialization, first-cycle, first-mutation, retain-restart, and steady-cycle latency | reports each initialization phase independently so startup costs are not hidden inside steady-state samples |
| `t0-shm` | measure same-host T0 shared-memory latency and overrun counters | evidence run: `1.167 us` round-trip `p95` with gate threshold `500 us` |
| `mesh-zenoh` | measure synthetic mesh pub/sub and query/reply behavior | evidence run: `678.077 us` pub/sub `p95` with gate threshold `2000 us` |
| `dispatch` | measure runtime-cloud dispatch, preflight, and audit correlation latency | evidence run: `210.316 us` end-to-end `p95` with gate threshold `3000 us` |

## Example commands

```bash
trust-runtime bench project --project examples/plcopen_motion_single_axis_demo --watch g_motion_demo_completed_sequences --output json
trust-runtime bench init --project crates/trust-runtime/tests/fixtures/init_bench --samples 20 --output json
trust-runtime bench t0-shm --samples 2000 --output json
trust-runtime bench mesh-zenoh --samples 1000 --loss-rate 0.01 --reorder-rate 0.02
trust-runtime bench dispatch --fanout 4 --output table
```

## Statistical and Output Contract

Latency summaries sort the measured nanosecond samples and report minimum,
nearest-rank p50/p95/p99, and maximum values in microseconds. Histograms round
each nanosecond sample up to the next whole microsecond, use the documented
finite bucket limits, and retain an overflow bucket for larger observations.

Jitter is the absolute difference between consecutive latency observations.
N latency samples therefore produce `N - 1` jitter samples; zero or one latency
observation produces zero jitter samples and must not invent a zero-valued
measurement. Synthetic mesh loss/reordering uses a deterministic seeded
generator so identical workload parameters exercise the same decision stream.

For T0, mesh, and dispatch benchmarks, `--payload-bytes` is the exact payload
length exercised by each applicable measured path. Mesh pub/sub and
query/reply use the same requested length, dispatch does not apply a hidden
size cap, and reports include the effective payload length.

`--output json` serializes the benchmark name plus its typed report object.
Table output presents the same report fields for humans. Project benchmarks
capture requested globals after the measured cycles, encode missing watched
globals as JSON `null`, and report budget overruns separately from latency.

## Motion benchmark pack

The standard project pack for motion-library benchmarking is:

- `examples/plcopen_motion_single_axis_benchmarks`

It compares:

- runtime floor
- constants-only library overhead
- status and readback overhead
- active move command cost
- full demo cost

## Reference Results

The communication benchmark evidence checked into the repo is:

- samples: `256`
- t0-shm round-trip `p95`: `1.167 us`
- mesh-zenoh pub/sub `p95`: `678.077 us`
- dispatch end-to-end `p95`: `210.316 us`

Source: `docs/internal/testing/evidence/trust-comms-v0.3.1/2026-02-20/artifacts/bench-summary.md`

## Related

- [Libraries and motion examples](../examples/libraries-and-motion.md)
- [PLCopen motion library](../develop/libraries/plcopen-motion.md)
