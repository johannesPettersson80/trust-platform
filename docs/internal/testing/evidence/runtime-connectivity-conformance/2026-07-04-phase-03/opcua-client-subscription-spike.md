# Phase 3 OPC UA Client Subscription Spike

Date: 2026-07-04

## Scope

This spike answers the Phase 3 dependency question before changing runtime
behavior: can the currently pinned `opcua` crate support a persistent OPC UA
client worker with Subscriptions, MonitoredItems, keep-alives, session renewal,
subscription transfer/recreate after reconnect, and an explicit republish path if
truST needs one?

This is not the persistent-worker implementation. The existing runtime still
uses the per-poll client path described below until `RTCONN-P3-001` and the
worker tests land.

## Current Runtime Behavior

The current scan-cycle subsystem calls the host OPC UA client helpers from the
cycle path:

- `crates/trust-runtime/src/runtime/opcua_client_subsystem.rs:84` through
  `:165` call `read_opcua_client_point_values` when `next_poll_ms` elapses.
- `crates/trust-runtime/src/runtime/opcua_client_subsystem.rs:168` through
  `:241` call `write_opcua_client_point_values` when writable bound values
  change.

The host helpers currently create and close a session for each operation:

- `crates/trust-runtime/src/host/opcua/client.rs:286` through `:318` creates a
  session, performs one read batch, and then calls `session.read().disconnect()`.
- `crates/trust-runtime/src/host/opcua/client.rs:355` through `:408` creates a
  session, performs one write batch, and then calls `session.read().disconnect()`.

That confirms the Phase 3 target: replace this connect-read/write-disconnect
path for runtime client operation with a persistent worker after the dependency
spike passes.

## Crate API Findings

Dependency under test:

- `opcua = "0.12"` from `crates/trust-runtime/Cargo.toml`
- local source inspected at
  `/home/johannes/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/opcua-0.12.0`

Findings:

- `Session` implements `SubscriptionService` with `create_subscription`,
  `modify_subscription`, `set_publishing_mode`, and `transfer_subscriptions`.
  Evidence: `opcua-0.12.0/src/client/session/services.rs:788` through `:876`
  and `opcua-0.12.0/src/client/session/session.rs:1549` through `:1689`.
- `Session` implements `MonitoredItemService` with `create_monitored_items`.
  Evidence: `opcua-0.12.0/src/client/session/services.rs:630` through `:635`
  and `opcua-0.12.0/src/client/session/session.rs:1828` through `:1916`.
- `Session::reconnect_and_activate` attempts to reconnect, reactivate or create
  a new session, then transfer or recreate existing subscriptions and monitored
  items. Evidence: `opcua-0.12.0/src/client/session/session.rs:291` through
  `:328` and `:332` through `:420`.
- The crate starts session and subscription activity tasks after session
  creation. Evidence: `opcua-0.12.0/src/client/session/session.rs:1427`
  through `:1435`.
- The session keep-alive task sends an empty `ReadRequest` at three quarters of
  the revised session timeout. Evidence: `opcua-0.12.0/src/client/session/session.rs:756`
  through `:819`.
- The session keep-alive implementation documents a caveat: it assumes the
  session timeout does not change after reconnect. Evidence:
  `opcua-0.12.0/src/client/session/session.rs:756` through `:763`.
- The crate exposes a public generic `Service::send_request<T>` path for service
  requests, and `RepublishRequest` implements the required message conversion
  through `SupportedMessage`. Evidence:
  `opcua-0.12.0/src/client/session/services.rs:80` through `:96` and
  `opcua-0.12.0/src/types/service_types/republish_request.rs:17` through
  `:23`.

## Spike Code

Added a compile/API proof test:

- `crates/trust-runtime/tests/opcua_client_runtime.rs`
- test name:
  `opcua_client_subscription_api_surface_is_available_for_phase3_worker`

The test proves the current dependency exposes the types and traits the worker
design needs:

- `Session: SubscriptionService`
- `Session: MonitoredItemService`
- `Session: Service`
- `MonitoredItemCreateRequest` construction from a `NodeId`
- `DataChangeCallback` access to changed monitored item values
- `Session::run` for session processing
- `Session::reconnect_and_activate` for reconnect lifecycle
- `RepublishRequest` through `<Session as Service>::send_request`

## Server Fixture

No new live server fixture was required for the dependency spike because this
slice is an API-surface and compile-proof gate. Existing live OPC UA fixtures
remain the integration fixture for runtime/server behavior:

- `crates/trust-runtime/tests/opcua_integration.rs`
- fixture roots:
  `crates/trust-runtime/tests/fixtures/opcua/interop`,
  `crates/trust-runtime/tests/fixtures/opcua/security`, and
  `crates/trust-runtime/tests/fixtures/opcua/perf`

The persistent-worker implementation must add behavior tests against the worker
seam and keep the `opcua_integration` gate in the Phase 3 completion gate.

## Validation

Local formatting and whitespace checks:

```sh
rustfmt --check crates/trust-runtime/tests/opcua_client_runtime.rs
git diff --check -- crates/trust-runtime/tests/opcua_client_runtime.rs
```

Result:

```text
passed
```

Remote disk preflight before the focused spike proof:

```sh
ssh trust-builder 'df -hT /home/johannes /tmp'
```

Result:

```text
trust-builder:/home/johannes 80G free
trust-builder:/tmp           6.8G free
```

Remote focused spike proof:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" cargo test -p trust-runtime --test opcua_client_runtime'
```

Result:

```text
Finished `test` profile [unoptimized + debuginfo] target(s) in 3m 49s
running 2 tests
test opcua_client_subscription_api_surface_is_available_for_phase3_worker ... ok
test opcua_client_accepts_vs_code_global_var_names ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; finished in 0.03s
```

## Decision

The current `opcua` 0.12 dependency is sufficient to proceed with the Phase 3
persistent worker design. No dependency upgrade or replacement slice is required
before implementing `RTCONN-P3-001`.

Worker constraints carried forward:

- The runtime scan cycle must not block on the OPC UA session task. Use a bounded
  latest-value handoff between the persistent worker and the scan-cycle
  subsystem.
- The worker must treat connection/session status callbacks, publish activity,
  and stale deadlines as authoritative for truST state transitions instead of
  relying only on the crate's keep-alive task.
- The crate's keep-alive task uses the revised session timeout at session
  creation but documents that it assumes the same timeout after reconnect. The
  truST worker must record negotiated timeout behavior and handle reconnect
  status explicitly.
- The crate already transfers subscriptions with `send_initial_values = true`
  during `reconnect_and_activate`; explicit `RepublishRequest` remains available
  through `Service::send_request` if the worker needs a stricter recovery path.
- `runtime.toml` and `io.toml` stay unchanged in Phase 3; worker settings must
  project from the existing OPC UA client connection config.
