# Issue #51 Initialization Benchmark Evidence

Date: 2026-04-28

Command:

```sh
cargo run -p trust-runtime --bin trust-runtime -- bench init --project crates/trust-runtime/tests/fixtures/init_bench --samples 1000 --output json
```

Machine:

- Host: `raspberrypi`
- Kernel: `Linux 6.12.62+rpt-rpi-2712 aarch64`
- Run mode: best-effort local run, not CPU-pinned

Fixture:

- Path: `crates/trust-runtime/tests/fixtures/init_bench`
- Resource: `InitBench`
- Backend: `vm`
- Warmup cycles: `0`

Current supported-feature baseline:

| Metric | p50 | p95 | p99 |
| --- | ---: | ---: | ---: |
| init-only | `19974.007 us` | `23108.791 us` | `25647.193 us` |
| init-plus-first-cycle | `19995.964 us` | `23142.997 us` | `25681.064 us` |
| first-cycle / first-mutation | `13.889 us` | `45.352 us` | `59.185 us` |
| retain restart | `375.094 us` | `661.410 us` | `846.651 us` |
| steady cycle | `4.630 us` | `18.556 us` | `33.482 us` |
| `StructValue::new`, 1000 constructions/sample | `313689.932 us` | `317186.461 us` | `324257.912 us` |
| untyped constructor proxy, 1000 constructions/sample | `68846.835 us` | `69809.181 us` | `70953.442 us` |

Pre-implementation same-fixture baseline:

- Status: unsupported/no measurement.
- `origin/main` has no `bench init` subcommand and no checked-in
  `crates/trust-runtime/tests/fixtures/init_bench` fixture.
- `origin/main` still parses declaration `:=` initializers through
  `parse_expression()`, so the representative aggregate initializer fixture is
  not a valid pre-change workload.

Interpretation:

- A same-fixture 10 percent regression comparison is not meaningful for Issue
  #51 because the pre-change runtime did not support the benchmarked feature
  set.
- These numbers are the first reproducible supported-feature baseline for
  future changes to initializer startup, retain restart, first-mutation, and
  steady-cycle behavior.
