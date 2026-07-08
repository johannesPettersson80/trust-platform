# Phase 1 Remote Focused Connector Test

Date: 2026-07-04

## Builder Cleanup

Remote builder cleanup was requested before validation.

Before cleanup:

- `trust-builder:/home/johannes`: 13G free
- `trust-builder:/tmp`: 3.2G free
- `trust-builder:$HOME/.cache/codex-targets`: 57G
- `trust-builder:$HOME/.cache/sccache`: 1.4G

Cleanup performed:

```sh
ssh trust-builder 'rm -rf "$HOME/.cache/codex-targets"/* "$HOME/.cache/sccache"; mkdir -p "$HOME/.cache/codex-targets"'
```

After cleanup:

- `trust-builder:/home/johannes`: 71G free
- `trust-builder:/tmp`: 3.2G free

Only generated/cache artifacts were removed. Source worktrees were not deleted.

## Remote Checkout Handling

The main remote checkout at `$HOME/projects/trust-platform` was dirty on branch
`trust-twin/p0-io-boundary-spike`, so it was not overwritten.

Validation used an isolated copy:

- Remote path: `$HOME/projects/trust-platform-rtconn-validation`
- Sync excluded `.git`, `target`, `fuzz/target`, `node_modules`, generated diagrams,
  large VS Code media, visual evidence, and large trust-twin component assets.
- Target dir: `$HOME/.cache/codex-targets/trust-platform-rtconn-validation`

## Command

```sh
ssh trust-builder 'mkdir -p "$HOME/.cache/codex-targets/trust-platform-rtconn-validation" && cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" cargo test -p trust-runtime --test connectors_status'
```

## Result

Passed.

```text
running 7 tests
test ads_state_mapping_covers_worker_and_report_states ... ok
test discovery_confidence_serializes_honest_tcp_only_label ... ok
test io_driver_health_mapping_honors_error_policy ... ok
test connector_status_report_serializes_stable_schema ... ok
test opcua_mapping_covers_client_and_server_states ... ok
test process_image_protocol_mappings_cover_mqtt_modbus_and_ethercat ... ok
test stale_connector_state_and_stale_point_quality_are_distinct_fields ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Notes

At the initial focused-test checkpoint, `just fmt`, `just clippy`, and
`just test` had not run for this phase yet. Local `cargo fmt --check` reported
unrelated pre-existing formatting drift in dirty files outside this connector
slice, including
`crates/trust-debug/src/adapter/tests_part_04.rs` and
`crates/trust-runtime/tests/io_cycle.rs`.

## Common Gate Re-run

Date: 2026-07-04

The same isolated builder copy was re-synced after rustfmt-only fixes to the
two formatting drift files above.

Remote preflight after builder cleanup and before the broad gates:

```text
trust-builder:/home/johannes 80G free
trust-builder:/tmp           6.8G free
```

Remote path: `$HOME/projects/trust-platform-rtconn-validation`

Commands:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && cargo fmt --check --all'
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" just fmt'
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" just clippy'
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" just test'
```

Results:

```text
cargo fmt --check --all: passed
just fmt: passed
just clippy: passed, finished in 3m25s
just test: passed; cargo-nextest missing fallback ran cargo test -p trust-runtime --lib with 819 passed, 16 ignored
```

## Additive Control Surface And RBAC Follow-Up

Date: 2026-07-04

Files covered:

- `crates/trust-runtime/src/control/connectors_handlers.rs`
- `crates/trust-runtime/src/control/handlers/connectors.rs`
- `crates/trust-runtime/src/control/policy.rs`
- `crates/trust-runtime/src/control/tests/connectors.rs`
- `crates/trust-runtime/src/connectors/adapters/io_driver.rs`

Behavior proved:

- `connectors.status` is a new additive control route with top-level
  `schema_version`.
- Process-image `IoDriverStatus` entries are projected into
  `ConnectorStatusReport` without adding connector fields to legacy
  `status.io_drivers`.
- The route requires Viewer access and introduces no write request.
- Local Unix control without a configured token can still read the route.
- Token-protected control rejects missing and invalid tokens.
- A Viewer pairing token can read the route.

Local commands:

```sh
cargo test -p trust-runtime --test connectors_status
cargo test -p trust-runtime connectors_status --lib
git diff --check -- CHANGELOG.md crates/trust-runtime/src/connectors/contract.rs crates/trust-runtime/src/connectors/adapters/io_driver.rs crates/trust-runtime/src/control/connectors_handlers.rs crates/trust-runtime/src/control/handlers/connectors.rs crates/trust-runtime/src/control/handlers/mod.rs crates/trust-runtime/src/control.rs crates/trust-runtime/src/control/policy.rs crates/trust-runtime/src/control/tests.rs crates/trust-runtime/src/control/tests/connectors.rs crates/trust-runtime/tests/connectors_status.rs
```

Local results:

```text
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 824 filtered out
```

Remote preflight before focused route/RBAC proof:

```text
trust-builder:/home/johannes 65G free
trust-builder:/tmp           3.2G free
```

Remote sync handling:

- Validation again used `$HOME/projects/trust-platform-rtconn-validation`.
- The first broad rsync was interrupted because it was transferring unrelated
  bulky evidence/assets.
- The final rsync used `--delete-excluded` against the isolated validation copy
  only, excluded build outputs, unrelated large evidence, and generated VS Code
  media, and retained
  `docs/internal/testing/evidence/runtime-connectivity-conformance/**`.

Remote commands:

```sh
ssh trust-builder 'df -hT /home/johannes /tmp && mkdir -p "$HOME/.cache/codex-targets/trust-platform-rtconn-validation" && cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" cargo test -p trust-runtime --test connectors_status'
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" cargo test -p trust-runtime connectors_status --lib'
```

Remote results:

```text
running 7 tests
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 3 tests
test control::policy::tests::connectors_status_requires_viewer_role ... ok
test control::tests::connectors_status_reports_process_image_drivers_without_mutating_legacy_status ... ok
test control::tests::connectors_status_authz_requires_viewer_and_preserves_local_unix_read ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 824 filtered out
```

Remote focused timings:

- `cargo test -p trust-runtime --test connectors_status`: 3m33s after target
  cache cleanup.
- `cargo test -p trust-runtime connectors_status --lib`: 35.83s using the same
  target directory.

## Diagram Render And Drift Proof

Date: 2026-07-04

Source updated:

- `docs/diagrams/architecture/runtime-execution.puml`

Generated artifacts refreshed from the isolated builder copy:

- `docs/diagrams/generated/runtime-execution.svg`
- `docs/diagrams/manifest.json`

Remote command:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && scripts/render_diagrams.sh && python scripts/check_diagram_drift.py'
```

Result: passed with no drift output after render. The manifest changed only for
`docs/diagrams/architecture/runtime-execution.puml`, whose SHA256 became
`c961a7d475002d27dc05ccb1b292adc8001e23a9ee9d90713982f04592c56fc1`.
