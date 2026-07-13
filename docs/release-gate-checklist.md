# Release Gate Checklist

Legend: `[ ]` pending, `[x]` complete.

## CI Gates
- [x] Rust format gate is enforced.
- [x] Rust clippy gate is enforced.
- [x] Rust test gate is enforced.
- [x] VS Code extension lint and compile gates are enforced.
- [x] VS Code extension integration tests run with `ST_LSP_TEST_SERVER`.
- [x] Windows CI packages the release-profile `win32-x64` VSIX, extracts its
  exact `trust-runtime.exe`, runs the focused migration/discovery tests in a
  real Windows Extension Host, and reserves loopback TCP `48898` while the
  packaged runtime browses a same-computer ADS target. The gate requires a
  structured native-ADS result and proves that no raw TCP connection was made;
  same-computer Windows ADS must use `TcAdsDll` and must never require or emulate
  a self-route.
- [x] The pre-push workflow runs the packaged ADS proof's portable unit
  contracts; execution of the extracted Windows PE remains a required Windows
  CI gate.
- [x] The tag-triggered Release workflow reruns the packaged Windows ADS proof
  before uploading or publishing the VSIX.
- [x] Neovim and Zed editor integration smoke gate is enforced (`scripts/check_editor_integration_smoke.sh`).
- [x] Workspace and VS Code extension release versions are enforced to stay aligned (`Cargo.toml`, `package.json`, `package-lock.json` root fields).
- [x] Main/master version bumps are blocked unless matching tag + successful release workflow + published GitHub release evidence exist.
- [x] `version-release-guard` gate evidence is published as `gate-version-release-guard` and required by the aggregated release-gate report.

## Reliability Gates
- [x] Nightly workflow runs runtime load and soak scripts.
- [x] Nightly workflow uploads load/soak logs and summary artifacts.
- [x] Reliability summary includes CPU/RSS trend data plus fault/restart signals.
- [x] Nightly workflow captures ST test flake sample artifacts.
- [x] Nightly workflow generates a rolling 14-day ST test flake aggregate report.
- [x] Self-hosted long-read-only HMI soak workflow exists for 24h evidence capture.

## Aggregation Gates
- [x] CI uploads per-job gate marker artifacts.
- [x] Release-gate report is generated and uploaded as a CI artifact.
- [x] Release-gate report fails when required gate artifacts are missing.
