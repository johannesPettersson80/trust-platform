# Phase 3 OPC UA Persistent Worker Proof

Date: 2026-07-05

## Scope

This evidence covers the Phase 3 implementation slice after the subscription
spike passed: the runtime OPC UA client now uses a persistent worker, bounded
cache handoff, subscriptions/monitored items for reads, batched writes through
the persistent session, and OPC UA point-quality projection into the additive
`connectors.status` surface.

This evidence also closes the Phase 3 common per-phase gate on the isolated
`trust-builder` validation copy after the generated target cache was cleaned.

## Implementation Evidence

Source changes:

- `crates/trust-runtime/src/host/opcua/client_cache.rs` owns the bounded
  latest-value cache and bounded `sync_channel` event sink.
- `crates/trust-runtime/src/host/opcua/client_bridge.rs` is the scan-cycle
  bridge. It applies cached input values during input scan and queues changed
  writable outputs after scan without transport I/O on the scan thread.
- `crates/trust-runtime/src/host/opcua/client_worker.rs` owns persistent
  transport/session lifecycle, session callbacks, subscriptions, monitored
  items, stale transitions, reconnect backoff, and batched writes.
- `crates/trust-runtime/src/runtime/opcua_client_subsystem.rs` no longer calls
  `read_opcua_client_point_values` or `write_opcua_client_point_values` from the
  scan cycle. Runtime operation flows through `OpcUaClientBridge`.
- `crates/trust-runtime/src/runtime/core/accessors.rs` adds
  `start_opcua_client_connection` and `reset_opcua_client_connections` so the
  bundle launcher can start one persistent worker per configured connection.
- `crates/trust-runtime/src/bin/trust-runtime/run/runtime/bundle_apply.rs`
  starts OPC UA client workers from the existing `runtime.opcua_client` and
  `opcua_client.toml` settings; no `runtime.toml` or `io.toml` schema change is
  introduced.
- `crates/trust-runtime/src/connectors/adapters/opcua.rs` projects OPC UA client
  connection and point status into the shared connector contract, including
  `PointQuality`, point source, IEC data type, and direction.
- `crates/trust-runtime/src/control/connectors_handlers.rs` includes live OPC UA
  client status in the additive `connectors.status` response through the existing
  `ResourceCommand::OpcUaClientStatus` boundary.

Behavior-lock tests:

- `persistent_worker_applies_subscription_updates_without_reconnecting_per_scan`
  proves subscription updates are delivered through the cache and scan-cycle
  reads do not create another session.
- `persistent_worker_batches_writes_without_reconnecting_per_write` proves
  changed writable points are batched through the existing persistent session.
- `persistent_worker_marks_stale_then_recovers_on_subscription_update` proves
  stale transition and recovery from a later subscription update.
- `persistent_worker_reconnects_after_session_loss_without_scan_thread_io`
  proves connection loss enters reconnecting, respects backoff, and reconnects
  without scan-thread transport I/O.
- `persistent_worker_uses_recovery_hook_to_reestablish_subscriptions` proves the
  worker attempts transport-owned reconnect recovery before falling back to a
  fresh session.
- `persistent_worker_recreates_subscription_after_server_restart` proves a
  server-restart-style session closure recreates read subscriptions after
  reconnect and delivers post-restart subscription values.
- `connected_detail_reports_timeout_negotiation_or_documented_gap` proves the
  worker status detail reports either an observed revised timeout or the
  documented `opcua` 0.12 limitation when revised timeout is unavailable.
- `opcua_client_status_projects_point_quality_and_metadata` proves OPC UA point
  freshness/server status maps to connector `PointQuality` and point metadata.
- `connectors_status_reports_opcua_client_points_with_quality` proves the same
  OPC UA point-quality projection is visible through the `connectors.status`
  control request.

## Validation

Disk preflight:

```sh
ssh trust-builder 'df -hT /home/johannes /tmp && du -xhd1 "$HOME/projects" 2>/dev/null | sort -h | tail -20 && du -xhd1 "$HOME/.cache" 2>/dev/null | sort -h | tail -20'
```

Result:

```text
/home/johannes: 104G free after the isolated validation sync
/tmp:           6.8G free
```

Focused connector contract test:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" cargo test -p trust-runtime --test connectors_status'
```

Result:

```text
running 12 tests
test opcua_client_status_projects_point_quality_and_metadata ... ok
test result: ok. 12 passed; 0 failed; 0 ignored; finished in 0.00s
```

Control-surface connector test:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" cargo test -p trust-runtime connectors_status --lib'
```

Result:

```text
running 6 tests
test control::tests::connectors_status_reports_opcua_client_points_with_quality ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 834 filtered out; finished in 0.02s
```

Focused OPC UA client status tests:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" cargo test -p trust-runtime opcua_client --lib'
```

Result:

```text
running 12 tests
test result: ok. 12 passed; 0 failed; 0 ignored; 828 filtered out; finished in 0.02s
```

Persistent worker tests:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" cargo test -p trust-runtime persistent_worker --lib'
```

Initial result:

```text
test opcua::tests::persistent_worker_reconnects_after_session_loss_without_scan_thread_io ... FAILED
left: Stale
right: Connected
```

Fix:

- `OpcUaSharedClientCache::mark_connected` now refreshes connector-level
  `last_seen_ms` on successful reconnect instead of retaining the pre-loss
  timestamp. Per-point freshness remains tracked on each point status.

Final result:

```text
running 5 tests
test opcua::tests::persistent_worker_applies_subscription_updates_without_reconnecting_per_scan ... ok
test opcua::tests::persistent_worker_reconnects_after_session_loss_without_scan_thread_io ... ok
test opcua::tests::persistent_worker_recreates_subscription_after_server_restart ... ok
test opcua::tests::persistent_worker_marks_stale_then_recovers_on_subscription_update ... ok
test opcua::tests::persistent_worker_batches_writes_without_reconnecting_per_write ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 836 filtered out; finished in 0.00s
```

After adding the explicit recovery hook, the worker filter was rerun:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" cargo test -p trust-runtime recovery_hook --lib && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" cargo test -p trust-runtime persistent_worker --lib'
```

Result:

```text
test opcua::tests::persistent_worker_uses_recovery_hook_to_reestablish_subscriptions ... ok

running 6 tests
test opcua::tests::persistent_worker_marks_stale_then_recovers_on_subscription_update ... ok
test opcua::tests::persistent_worker_reconnects_after_session_loss_without_scan_thread_io ... ok
test opcua::tests::persistent_worker_applies_subscription_updates_without_reconnecting_per_scan ... ok
test opcua::tests::persistent_worker_uses_recovery_hook_to_reestablish_subscriptions ... ok
test opcua::tests::persistent_worker_batches_writes_without_reconnecting_per_write ... ok
test opcua::tests::persistent_worker_recreates_subscription_after_server_restart ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 837 filtered out; finished in 0.00s
```

## Reconnect/Republish Decision

`OpcUaClientTransport::recover_after_disconnect` is the explicit reconnect
recovery seam. The real `OpcUaWireClientTransport` implementation calls
`Session::reconnect_and_activate`.

For `opcua` 0.12, `reconnect_and_activate` attempts to reconnect/reactivate the
session, calls `TransferSubscriptions` with `send_initial_values=true`, and
recreates subscriptions and monitored items that cannot be transferred. That is
the implemented subscription re-establish path for this phase.

Direct `RepublishRequest` issuance remains unavailable through the public crate
API because it requires the server subscription id plus a retransmit sequence
number, while `opcua` 0.12 keeps publish sequence state private inside
`session_state.rs` and exposes subscription callbacks only as changed
`MonitoredItem` values. The worker records this limitation in recovery detail
instead of fabricating a republish claim.

Timeout negotiation/detail test:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" cargo test -p trust-runtime connected_detail --lib'
```

Result:

```text
running 1 test
test opcua::tests::connected_detail_reports_timeout_negotiation_or_documented_gap ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 841 filtered out; finished in 0.00s
```

OPC UA runtime spike/integration targets:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" cargo test -p trust-runtime --test opcua_client_runtime'
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" cargo test -p trust-runtime --test opcua_integration'
```

Result:

```text
opcua_client_runtime: 2 passed; 0 failed
opcua_integration:   4 passed; 0 failed
```

Runtime networking gates:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" RUSTFLAGS=-Dwarnings cargo check -p trust-runtime --all-targets'
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" ./scripts/runtime_mesh_tls_stability_gate.sh --iterations 8'
```

Result:

```text
RUSTFLAGS=-Dwarnings cargo check -p trust-runtime --all-targets: passed in 2m47s
runtime_mesh_tls_stability_gate.sh --iterations 8: PASS
```

Common per-phase gate:

```sh
rsync -az --delete --delete-excluded \
  --exclude '/.git/' \
  --exclude '/target/' \
  --exclude '/fuzz/target/' \
  --exclude '**/node_modules/' \
  --exclude '/.venv-docs/' \
  --exclude '/docs/internal/testing/evidence/vscode-ui-ux-acceptance/' \
  --exclude '/scripts/captures/.cache/' \
  --exclude '**/.vscode-test/' \
  --exclude '**/.pytest_cache/' \
  --exclude '**/.ruff_cache/' \
  --exclude '**/.mypy_cache/' \
  /home/johannes/projects/trust-platform/ \
  trust-builder:~/projects/trust-platform-rtconn-validation/
ssh trust-builder 'set -euo pipefail
cd "$HOME/projects/trust-platform-rtconn-validation"
export CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation"
df -hT /home/johannes /tmp
just fmt
just clippy
just test'
```

Result:

```text
Disk preflight: /home/johannes 110G free, /tmp 6.8G free
just fmt:    passed
just clippy: passed; trust-runtime checked cleanly in 3m07s
just test:   passed; cargo-nextest missing fallback ran cargo test -p trust-runtime --lib
             827 passed; 0 failed; 16 ignored; finished in 4.16s
```
