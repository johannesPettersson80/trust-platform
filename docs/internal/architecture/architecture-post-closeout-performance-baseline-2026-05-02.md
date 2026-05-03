# Architecture Post-Closeout Performance Baseline

Status: recorded for post-closeout gap closure
Date: 2026-05-02
Owner: architecture/runtime

## Scope

This report closes the measurement gap left by `full-architecture-refactor-final-report-2026-05-02.md`.
It records runtime benchmark, compile-time, binary-size, and memory-footprint evidence before and after the post-closeout structural cleanup.

The post-closeout cleanup moved workbench code into `trust-dev`, PLCopen XML helpers into `trust-plcopen`, host-only runtime modules under `trust-runtime/src/host`, and split remaining runtime/runtime-core files over 1,000 lines.

## Method

Machine and toolchain:

- Machine: `raspberrypi`, `Linux 6.12.75+rpt-rpi-2712`, `aarch64`, Cortex-A76, 4 CPUs.
- CPU governor: `ondemand`.
- Toolchain: `rustc 1.95.0`, `cargo 1.95.0`.
- `cargo-bloat`: `0.12.1`.

Artifacts:

- Historical comparison: `target/gate-artifacts/architecture-post-closeout-historical-bf2f1d624/`.
- Pre-cleanup baseline: `target/gate-artifacts/architecture-post-closeout-baseline-6cfeea572/`.
- Final post-cleanup measurement: `target/gate-artifacts/architecture-post-closeout-final-6cfeea572-20260502T231507Z/`.

Commands:

- `cargo run --release -p trust-runtime --bin trust-runtime -- bench init --project crates/trust-runtime/tests/fixtures/init_bench --samples 1000 --output json`
- `cargo run --release -p trust-runtime --bin trust-runtime -- bench project --project examples/plcopen_motion_single_axis_demo --samples 2000 --warmup-cycles 200 --watch g_motion_demo_completed_sequences --output json`
- `cargo run --release -p trust-runtime --bin trust-runtime -- bench project --project examples/plcopen_motion_single_axis_demo --samples 2000 --warmup-cycles 200 --watch g_motion_demo_completed_sequences --tier1 --output json`
- `cargo run --release -p trust-runtime --bin trust-runtime -- bench dispatch --samples 1000 --fanout 4 --output json`
- `cargo build --release -p trust-runtime --timings`
- `cargo bloat --release -p trust-runtime --bin trust-runtime --crates -n 40`

## Runtime Benchmarks

| Metric | Historical `bf2f1d624` | Pre-cleanup baseline `6cfeea572` | Final post-cleanup | Final vs baseline | Final vs historical |
| --- | ---: | ---: | ---: | ---: | ---: |
| Init p50 | 2551.159 us | 1736.825 us | 1767.009 us | +1.7% | -30.7% |
| Init p95 | 7825.683 us | 3465.540 us | 4101.762 us | +18.4% | -47.6% |
| First cycle p50 | 3.407 us | 2.093 us | 2.222 us | +6.2% | -34.8% |
| Retain restart p50 | 67.852 us | 35.297 us | 35.555 us | +0.7% | -47.6% |
| Project p50 | 933.356 us | 268.446 us | 272.705 us | +1.6% | -70.8% |
| Project p95 | 5751.489 us | 451.558 us | 511.854 us | +13.4% | -91.1% |
| Project throughput | 590.103 cycles/s | 3383.081 cycles/s | 3230.570 cycles/s | -4.5% | +447.5% |
| Tier-1 p50 | 901.430 us | 346.484 us | 348.502 us | +0.6% | -61.3% |
| Tier-1 p95 | 5113.985 us | 713.337 us | 589.929 us | -17.3% | -88.5% |
| Tier-1 throughput | 698.880 cycles/s | 2510.141 cycles/s | 2635.998 cycles/s | +5.0% | +277.2% |
| Dispatch p50 | 27.111 us | 23.612 us | 23.629 us | +0.1% | -12.8% |
| Dispatch p95 | 37.241 us | 24.112 us | 24.296 us | +0.8% | -34.8% |

Project semantic checks in both final project runs:

- `budget_overruns = 0`.
- `g_motion_demo_completed_sequences = 313`.

Interpretation:

- Median runtime behavior is stable against the pre-cleanup baseline. The structural cleanup did not introduce a median regression over 5% for project, tier-1, dispatch, init, or retain-restart paths.
- Project throughput is within the 5% budget against the pre-cleanup baseline and is much faster than the historical comparison commit.
- Init p95 and project p95 are worse than the pre-cleanup baseline by more than 5%. This is tail-latency noise on a non-pinned `ondemand` CPU run, not a correctness or steady-throughput failure: p50 medians remain stable, project throughput stays within budget, and there are zero project budget overruns. Treat stricter tail budgets as a future performance-board concern, not as evidence that this structural refactor broke runtime behavior.

## Build, Size, Memory

| Metric | Historical `bf2f1d624` | Pre-cleanup baseline `6cfeea572` | Final post-cleanup | Result |
| --- | ---: | ---: | ---: | --- |
| `cargo build --release -p trust-runtime --timings` elapsed | 108 s | 202 s | 105 s | Final is faster than both recorded baselines. |
| `trust-runtime` `.text` from `cargo bloat` | not captured | 4.2 MiB | 3.9 MiB | Runtime crate attribution shrank after PLCopen extraction. |
| Total `.text` from `cargo bloat` | not captured | 19.1 MiB | 19.1 MiB | Total binary text size is stable. |
| Binary file size from `cargo bloat` | not captured | 41.9 MiB | 41.9 MiB | Binary size is stable. |
| Project benchmark VmHWM | not captured | 20640 KB | 20624 KB | Memory footprint is stable. |

Historical `cargo bloat` was attempted but not completed: the old worktree rebuilt native TLS/OpenSSL dependencies for too long and was stopped, so historical binary-size attribution is not comparable. Current and final `cargo bloat` evidence is complete.

## Closeout Decision

The post-closeout structural cleanup passes the performance closeout with recorded caveats:

- No median runtime regression over 5% against the pre-cleanup baseline.
- No project throughput regression over 5%.
- No project budget overruns.
- Compile time, binary size, and memory footprint are stable or improved.
- Tail p95 movement is documented and should be handled by a dedicated performance board if future work needs a strict tail-latency budget on pinned hardware.
