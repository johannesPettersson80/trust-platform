# Release Gate Checklist

Legend: `[ ]` pending, `[x]` complete.

## CI Gates
- [x] Rust format gate is enforced.
- [x] Rust clippy gate is enforced.
- [x] Rust test gate is enforced.
- [x] The firmware of every chip in `chips/` is built and its image checked on every change (`gate-firmware-<chip>`).
- [x] VS Code extension lint and compile gates are enforced.
- [x] VS Code extension integration tests run with `ST_LSP_TEST_SERVER`.
- [x] Neovim and Zed editor integration smoke gate is enforced (`scripts/check_editor_integration_smoke.sh`).
- [x] Workspace and VS Code extension release versions are enforced to stay aligned (`Cargo.toml`, `package.json`, `package-lock.json` root fields).
- [x] A pull request that changes the version merges only after the Release workflow rehearsed it on its exact head, and the release follows `main`'s CI: the Release workflow tags the commit whose CI passed, publishes the GitHub release and the Marketplace packages, and verifies the tag, the release and every Marketplace target (specification 24).
- [x] `version-release-guard` gate evidence is published as `gate-version-release-guard` and required by the aggregated release-gate report.

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
