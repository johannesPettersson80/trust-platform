# Salsa Overnight Hardening Report (2026-02-09)

## Scope

This report documents a full local overnight hardening run for Salsa integration gates.

- Run log root: `logs/salsa-overnight-20260209T090426Z`
- Summary log: `logs/salsa-overnight-20260209T090426Z/summary.log`
- Mode: `nightly`
- Continue on fail: `true`
- Baselines:
  - `docs/reports/salsa-hardening-perf-baseline.env`
  - `docs/reports/salsa-memory-baseline.env`

## Executed Gate Matrix

| Step | Command | Result | Duration |
|---|---|---|---|
| 1 | `cargo fmt --all --check` | PASS | 2s |
| 2 | `cargo clippy -p trust-hir -p trust-lsp -- -D warnings` | PASS | 12s |
| 3 | `cargo test -p trust-hir` | PASS | 14s |
| 4 | `cargo test -p trust-lsp --tests` | PASS | 13s |
| 5 | `./scripts/salsa_hardening_perf_gate.sh compare` | PASS | 8s |
| 6 | `./scripts/salsa_memory_gate.sh compare` | PASS | 20s |
| 7 | `./scripts/salsa_miri_gate.sh` | PASS | 20s |
| 8 | `./scripts/salsa_fuzz_gate.sh smoke` | PASS | 195s |
| 9 | `SALSA_FUZZ_EXTENDED_SECONDS=28800 ./scripts/salsa_fuzz_gate.sh extended` | PASS | 57606s |

Overall: `PASS`

## Key Measured Results

### Perf Gate (`05-Perf_gate_compare_.log`)

- Median edit-loop latency: `avg=6.43ms`, `p95=6.89ms`
- Median CPU/op: `6.42ms`
- Baseline deltas:
  - `avg_ms`: `-19.22%`
  - `p95_ms`: `-39.88%`
  - `cpu_ms_per_iter`: `-18.01%`
- Verdict: `PASS` (within configured regression budget)

### Memory Gate (`06-Memory_gate_compare_.log`)

- Medians:
  - `avg=6.82ms`, `p95=7.09ms`, `cpu=6.83ms/op`
  - `alloc_calls=7185.46/op`
  - `alloc_bytes=1720933.45B/op`
  - `retained=5017.80B/op`
  - `rss_end=13744KB`, `rss_delta=2768KB`
- Baseline deltas:
  - `avg_ms`: `-12.34%`
  - `p95_ms`: `-31.03%`
  - `cpu_ms_per_iter`: `-11.87%`
  - `alloc_calls_per_iter`: `0.00%`
  - `alloc_bytes_per_iter`: `0.00%`
  - `retained_bytes_per_iter`: `0.00%`
  - `rss_delta_kb`: `-2.26%`
- Cap checks:
  - `rss_end_kb=13744` vs cap `524288`
  - `rss_delta_kb=2768` vs cap `131072`
  - `retained_bytes_per_iter=5017.80` vs cap `50000`
- Verdict: `PASS`

### Miri Gate (`07-Miri_gate_.log`)

Executed allowlist:

1. `db::queries::database::tests::set_source_text_same_content_keeps_source_revision`
2. `db::queries::database::tests::remove_missing_source_keeps_source_revision`
3. `db::queries::database::tests::expr_id_at_offset_returns_none_for_missing_file`

All three passed under Miri.

### Fuzz Gates

Smoke (`08-Fuzz_smoke_gate_.log`):

- Target `syntax_parse` (smoke mode) passed
- Target `hir_semantic` (smoke mode) passed
- Verdict: `PASS`

Extended (`09-Fuzz_extended_gate_28800s_.log`):

- Target `syntax_parse` (extended mode) passed
- Target `hir_semantic` (extended mode) passed
- Verdict: `PASS`

## Interpretation

The overnight hardening run passed all quality gates without regressions:

- No formatting or lint breaks in required packages.
- No `trust-hir` / `trust-lsp` test failures.
- Perf and memory baselines remain within policy limits, with measured improvement vs baseline.
- Miri allowlist passed.
- Both fuzz targets completed in smoke and extended modes with no crash reported.

## Important Timing Note

`Fuzz extended gate (28800s)` completed in `57606s` because `extended` mode runs two targets sequentially:

1. `syntax_parse` for up to `28800s`
2. `hir_semantic` for up to `28800s`

So the practical wall-clock budget is approximately `2 * SALSA_FUZZ_EXTENDED_SECONDS` (plus setup/teardown).

## Recommendation

From this run alone, Salsa hardening status is release-ready for the validated gate set.

Before final release tagging, keep normal release checks in place:

1. Fresh CI run on latest `main`
2. Release workflow (`v*` tag) green
