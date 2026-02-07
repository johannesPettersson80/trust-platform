# Release Gate Checklist

Legend: `[ ]` pending, `[x]` complete.

## CI Gates
- [x] Rust format gate is enforced.
- [x] Rust clippy gate is enforced.
- [x] Rust test gate is enforced.
- [x] VS Code extension lint and compile gates are enforced.
- [x] VS Code extension integration tests run with `ST_LSP_TEST_SERVER`.

## Reliability Gates
- [x] Nightly workflow runs runtime load and soak scripts.
- [x] Nightly workflow uploads load/soak logs and summary artifacts.
- [x] Reliability summary includes CPU/RSS trend data plus fault/restart signals.
- [x] Nightly workflow captures ST test flake sample artifacts.
- [x] Nightly workflow generates a rolling 14-day ST test flake aggregate report.

## Aggregation Gates
- [x] CI uploads per-job gate marker artifacts.
- [x] Release-gate report is generated and uploaded as a CI artifact.
- [x] Release-gate report fails when required gate artifacts are missing.
