# PLCopen Motion Single-Axis Benchmark Breakdown Pack

Docs category: `docs/public/examples/libraries-and-motion.md`

This profiling pack isolates the main per-scan cost suspects in the shipped
single-axis PLCopen motion library.

Projects:

- `runtime_floor` — runtime floor with no motion-library dependency, just one global counter increment.
- `constants_only` — calls `MC_Constants()` every scan and nothing else.
- `status_only` — powers one axis and runs the status/readback FBs every scan.
- `command_idle` — instantiates the same command/readback FB set as the demo, but keeps every execute-style command inactive.
- `move_absolute_only` — runs one active `MC_MoveAbsolute` loop with minimal supporting FBs.
- `full_demo_constants_once` — same semantics as the real demo, but `MC_Constants()` is invoked once during initialization instead of every scan.
- `../plcopen_motion_single_axis_demo` — the canonical user-facing full demo; benchmarked by the runner script for comparison.

Run the full breakdown on the current hardware:

```bash
./scripts/runtime_motion_benchmark_breakdown.sh
```

That script writes raw JSON outputs plus a markdown summary under:

- `target/gate-artifacts/runtime-motion-benchmark-breakdown/`

Each JSON report includes a `vm_profile` section for VM-backed projects, and
the markdown summary highlights the motion-heavy workloads that dominate the
shipped example path.

If you want a quick manual spot check for one variant:

```bash
target/release/trust-runtime bench project \
  --project examples/plcopen_motion_single_axis_benchmarks/status_only \
  --samples 128 \
  --warmup-cycles 32 \
  --watch g_motion_bench_cycles \
  --watch g_motion_bench_completed_sequences \
  --watch g_motion_bench_last_error \
  --watch g_motion_bench_power_on \
  --watch g_motion_bench_is_homed \
  --output table
```
