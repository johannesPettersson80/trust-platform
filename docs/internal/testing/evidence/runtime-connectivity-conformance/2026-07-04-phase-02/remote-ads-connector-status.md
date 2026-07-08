# Phase 2 ADS Connector Status Projection

Date: 2026-07-04

## Scope

This slice projects existing ADS status reports into the additive
`connectors.status` surface:

- ADS client `AdsStatusReport` entries become `ConnectorStatusReport` entries
  with supervisory-client kind, ADS protocol, target endpoint, AMS Net ID, state,
  health, reconnect policy, and aggregate point counts.
- ADS server status becomes a supervisory-server connector with the configured
  server AMS Net ID, ADS port, chosen bind IP, state, health, and aggregate point
  counts.
- ADS reconnecting, stale, faulted, route-failure, and auth-failure status
  reports preserve degraded/faulted state and human-readable failure detail in
  the connector projection.
- Live ADS point statuses from `ActiveAdsDeviceSnapshot` project read,
  notification, stale/cold-start, write-pending, read-error, and write-failure
  quality into connector `PointQuality` without adding point rows to legacy
  `ads.status` JSON.
- Existing `ads.status`, `ads.server.status`, and Phase 0 golden surfaces remain
  byte-identical.
- RBAC coverage proves `connectors.status` stays Viewer-readable while ADS
  route writes, live imports, and write-enabled doctor flows retain stronger
  Engineer/Admin role requirements.
- Control-surface consistency coverage compares legacy `ads.status` and the
  additive `connectors.status` ADS projection in the same runtime state.
  CLI/HMI rendering was not touched in this Phase 2 slice.

No ADS transport, worker, notification, route-add, or config-file execution
behavior changed in this slice.

## Files

- `crates/trust-runtime/src/connectors/adapters/ads.rs`
- `crates/trust-runtime/src/connectors/report.rs`
- `crates/trust-runtime/src/control/connectors_handlers.rs`
- `crates/trust-runtime/src/control/ads_handlers/status.rs`
- `crates/trust-runtime/src/control/ads_handlers/server.rs`
- `crates/trust-runtime/src/control/policy.rs`
- `crates/trust-runtime/src/control/tests/connectors.rs`
- `crates/trust-runtime/tests/connectors_status.rs`

## Local Validation

```sh
rustfmt --check crates/trust-runtime/src/connectors/report.rs crates/trust-runtime/src/connectors/adapters/ads.rs crates/trust-runtime/src/control/ads_handlers/status.rs crates/trust-runtime/src/control/ads_handlers/server.rs crates/trust-runtime/src/control/ads_handlers/mod.rs crates/trust-runtime/src/control/connectors_handlers.rs crates/trust-runtime/src/control/tests/connectors.rs crates/trust-runtime/tests/connectors_status.rs
git diff --check -- crates/trust-runtime/src/connectors/report.rs crates/trust-runtime/src/connectors/adapters/ads.rs crates/trust-runtime/src/control/ads_handlers/status.rs crates/trust-runtime/src/control/ads_handlers/server.rs crates/trust-runtime/src/control/ads_handlers/mod.rs crates/trust-runtime/src/control/connectors_handlers.rs crates/trust-runtime/src/control/tests/connectors.rs crates/trust-runtime/tests/connectors_status.rs
```

Result:

```text
Both checks passed for the touched Phase 2 files.
```

## Remote Validation

Remote path: `$HOME/projects/trust-platform-rtconn-validation`

Disk preflight:

```sh
ssh trust-builder 'df -hT /home/johannes /tmp && du -xhd1 "$HOME/projects" 2>/dev/null | sort -h | tail -20 && du -xhd1 "$HOME/.cache" 2>/dev/null | sort -h | tail -20'
```

Result:

```text
/home/johannes: 73G free
/tmp: 6.8G free
$HOME/.cache/codex-targets: 7.3G after the warmed focused test target
```

Focused connector/status compatibility commands:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" cargo test -p trust-runtime --test connectors_status && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" cargo test -p trust-runtime connectors_status --lib && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" cargo test -p trust-runtime phase0 --lib'
```

Initial result:

```text
connectors_status.rs: 8 passed
connectors_status --lib: 4 passed
phase0 --lib: 5 passed
```

After adding ADS reconnect/stale/fault/route/auth projection cases and ADS
point-quality/RBAC projection cases:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" cargo test -p trust-runtime --test connectors_status'
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" cargo test -p trust-runtime connectors_status --lib'
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" cargo test -p trust-runtime ads_connector_status --lib'
```

Result:

```text
connectors_status.rs: 11 passed
connectors_status --lib: 5 passed
ads_connector_status --lib: 1 passed
```

ADS-specific gate commands:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" cargo test -p trust-runtime --test ads_cli_command && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" cargo test -p trust-runtime --test ads_web_api'
```

Result:

```text
ads_cli_command: 9 passed
ads_web_api: 6 passed
```

Common per-phase gate commands after rustfmt-only cleanup:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && cargo fmt --check --all'
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" just fmt'
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" just clippy'
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" just test'
```

Result:

```text
cargo fmt --check --all: passed
just fmt: passed
just clippy: passed, finished in 3m25s
just test: passed; cargo-nextest missing fallback ran cargo test -p trust-runtime --lib with 819 passed, 16 ignored
```

Full boundary gate preflight after the common gate warmed the target:

```text
trust-builder:/home/johannes 79G free
trust-builder:/tmp           6.8G free
$HOME/.cache/codex-targets/trust-platform-rtconn-validation: 6.6G
```

Full boundary gate command:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" just test-all'
```

Result:

```text
passed
scripts/cargo_test_fast_link.sh test --all finished successfully
notable covered suites included trust-runtime lib, connectors_status, ads_cli_command, ads_web_api, modbus_driver, ethercat_driver, io_multidriver_live, opcua_client_runtime, and opcua_integration
```

## Open Phase 2 Gaps

- None. Phase 2 targeted, common, and full boundary gates are green on the
  isolated validation copy.
