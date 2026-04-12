# PLCopen Motion Single-Axis Demo

This example is the user-facing reference consumer for
`libraries/plcopen_motion/single_axis_core`.

The demo loads `MC_Constants()` once during initialization and then reuses
those published values during the steady-state scan loop.

It shows one realistic scan-driven sequence:

1. power the axis
2. configure a positive software limit
3. home the axis
4. execute `MC_MoveAbsolute` to `80.0`
5. queue a buffered `MC_MoveRelative` of `+15.0`
6. verify the relative move clamps at the configured `90.0` limit
7. run `MC_MoveVelocity`
8. stop and loop the sequence

## Project Files

- `trust-lsp.toml` — project config and dependency on the motion library
- `runtime.toml` — runtime config with a 10 ms cycle budget
- `io.toml` — selects the simulated I/O backend so the demo runs without hardware; replace `driver = "simulated"` with your target I/O driver for real deployments
- `src/Globals.st` — benchmark/watch globals
- `src/Main.st` — the scan-driven motion sequence

## Build And Validate

From repository root:

```bash
trust-runtime build --project examples/plcopen_motion_single_axis_demo --sources src
trust-runtime validate --project examples/plcopen_motion_single_axis_demo
```

`trust-runtime build` regenerates the ignored `program.stbc` bundle. Rebuild before
`trust-runtime run` if you change the example sources.

## Benchmark The Real Example

Run the example through the focused project benchmark:

```bash
cargo run --release -p trust-runtime --bin trust-runtime -- \
  bench project \
  --project examples/plcopen_motion_single_axis_demo \
  --samples 128 \
  --warmup-cycles 32 \
  --watch g_motion_demo_completed_sequences \
  --watch g_motion_demo_last_error \
  --watch g_motion_demo_limit_clamp_verified \
  --watch g_motion_demo_power_on \
  --watch g_motion_demo_is_homed \
  --output table
```

When `runtime.execution_backend = "vm"`, the bench output also includes a
`vm_profile` section with register-executor execution counts, fallback reasons,
hot blocks, and lowering-cache counters for the measured workload.

Or run the gate script that asserts both semantics and speed budget:

```bash
./scripts/runtime_motion_example_bench_gate.sh
```

## Watched Globals

- `g_motion_demo_completed_sequences` — increments every time the full motion loop completes
- `g_motion_demo_last_error` — stays `0x0000` on the healthy path
- `g_motion_demo_limit_clamp_verified` — latches `TRUE` once the buffered relative move clamps to `90.0`
- `g_motion_demo_power_on` — mirrors `MC_Power.Status`
- `g_motion_demo_is_homed` — mirrors `MC_ReadAxisInfo.IsHomed`

## Expected Healthy Result

A healthy benchmark run should show:

- completed sequences greater than zero
- last error equal to `0`
- clamp verification equal to `true`
- no cycle-budget overruns
- cycle-latency p95 at or below the configured cycle budget
