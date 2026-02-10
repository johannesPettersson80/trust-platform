# Salsa 0.26 Upgrade Report

Date: 2026-02-08

## Scope

Stage 4.4 spike: upgrade from `salsa 0.18` to `salsa 0.26` and port current `trust-hir` query API usage.

## Code Changes

- Dependency:
  - `Cargo.toml`: `salsa = "0.18"` -> `salsa = "0.26"`
- `trust-hir` Salsa macro/API port:
  - `#[return_ref]` -> `#[returns(ref)]` for `#[salsa::input]` fields.
  - `#[salsa::tracked(return_ref)]` -> `#[salsa::tracked(returns(ref))]`.
  - Removed obsolete `salsa_event` method override from `impl salsa::Database`.

Touched file:
- `crates/trust-hir/src/db/queries/salsa_backend.rs`

## Validation Results

All functional/quality gates executed and passing:

- `cargo check -p trust-hir` PASS
- `cargo check -p trust-lsp` PASS
- `cargo test -p trust-hir` PASS
- `cargo test -p trust-lsp` PASS
- `cargo test -p trust-lsp --no-run` PASS
- `cargo test --workspace` PASS
- `cargo clippy -p trust-hir -- -D warnings` PASS
- `cargo clippy --workspace -- -D warnings` PASS

## Performance Gates

### `./scripts/salsa_spike_gate.sh`

- PASS
- salsa medians:
  - `avg_ms=7.45`
  - `p95_ms=10.38`
  - `cpu_ms_per_iter=7.42`

### `./scripts/salsa_hardening_perf_gate.sh compare`

- PASS (within 5% regression budget)
- baseline medians:
  - `avg_ms=7.96`
  - `p95_ms=11.46`
  - `cpu_ms_per_iter=7.83`
- current medians:
  - `avg_ms=7.58`
  - `p95_ms=10.26`
  - `cpu_ms_per_iter=7.50`

Regression deltas:

- `avg_ms`: -4.77%
- `p95_ms`: -10.47%
- `cpu_ms_per_iter`: -4.21%

Baseline note:

- The hardening perf gate now validates baseline metadata (`benchmark_id`, `rustc_version`,
  `cargo_lock_sha256`, `system`) and fails fast on stale baselines before metric comparison.

## Decision

GO on `salsa 0.26`.

Rationale:

- Functional correctness and lint/test quality are green on `0.26`.
- Hardening perf baseline gate is green on a reproducible metadata-pinned baseline.

## Follow-up

1. Keep running `salsa_hardening_perf_gate.sh compare` after performance-sensitive changes.
2. Keep Miri gate active with parser-path exclusions until Rowan UB signal is resolved upstream.
3. Close CI “3 consecutive green hardening workflow runs” evidence.
