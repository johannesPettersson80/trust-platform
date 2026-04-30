# Runtime Host Surface Ownership Checklist

Status: Runtime host-surface ownership board complete; post-merge CI watch next
Owner: Runtime/web/HMI/control/cloud
Scope: address audit F11 by defining and enforcing ownership for `web`, `hmi`, `ui`, `control`, and `runtime_cloud`.

## Ownership Target

- [x] `RTHOST-OWN-01` Runtime/core owns execution state and value/snapshot ports.
- [x] `RTHOST-OWN-02` Control owns HTTP-neutral command/query contracts and authorization/write policy.
- [x] `RTHOST-OWN-03` HMI owns schema/contracts/descriptors, not route transport.
- [x] `RTHOST-OWN-04` Web owns HTTP routes, websocket serving, static assets, and browser transport adapters.
- [x] `RTHOST-OWN-05` UI owns terminal/local presentation only where it remains.
- [x] `RTHOST-OWN-06` Runtime-cloud owns cloud projection/contracts and does not own runtime execution.

## Phase 0 - Full-Map Prerequisite

- [x] `RTHOST-P0-001` Hard prerequisite: `architecture-doctor --full-map` MVP implements `FULLMAP-CHECK-07` for HMI/web/control/cloud ownership and forbidden edges before Phase 3 or Phase 4 starts. CHECK-07 now enforces owner categories, forbidden `control -> web` / `hmi -> web` / `runtime_cloud -> web` implementation imports, and direct web runtime-state bypasses with `host_surface.approved_ports_active = true`.
- [ ] `RTHOST-P0-002` If `FULLMAP-CHECK-07` is unavailable, record an owner-approved waiver with the local replacement rule, fixture, owner, and expiration date.
- [x] `RTHOST-P0-GATE-01` Do not claim `ARCHPROG-C-02` or `ARCHPROG-C-04` complete until `FULLMAP-CHECK-07` or its waiver is recorded.

## Stop Rules

- [ ] `RTHOST-STOP-01` Stop if `control` imports web implementation types.
- [ ] `RTHOST-STOP-02` Stop if HMI domain logic is added directly to web routes instead of HMI/control contracts.
- [ ] `RTHOST-STOP-03` Stop if runtime-cloud route code owns runtime execution decisions.
- [ ] `RTHOST-STOP-04` Stop if a port is so broad that it becomes a hidden runtime god object.
- [ ] `RTHOST-STOP-05` Stop if browser-visible behavior changes without Playwright/browser verification in the implementation branch.

## Phase 1 - Inventory

- [x] `RTHOST-P1-001` Map all `src/hmi/` files by responsibility.
- [x] `RTHOST-P1-002` Map all `src/control/hmi_handlers*.rs` files by responsibility.
- [x] `RTHOST-P1-003` Map `src/web/hmi_ws.rs` and HMI route files by responsibility.
- [x] `RTHOST-P1-004` Map `src/runtime_cloud/` files by responsibility.
- [x] `RTHOST-P1-005` Map `src/web/runtime_cloud_*`, `runtime_cloud_routes/*`, and `runtime_cloud_state/*`.
- [x] `RTHOST-P1-006` Record direct imports among `web`, `hmi`, `control`, `ui`, and `runtime_cloud`.
- [x] `RTHOST-P1-007` Identify duplicated DTOs, duplicated auth/write checks, duplicated schema projection, and duplicated runtime snapshot logic.
- [x] `RTHOST-P1-008` Produce `docs/internal/architecture/generated/runtime-host-surface-inventory.md`.
- [x] `RTHOST-P1-009` Inventory output must include per-file owner, current imports, proposed owner, and proposed action: keep, move, split, delete, or adapter-only.
- [x] `RTHOST-P1-010` Do not start Phase 4 until Phase 1 inventory is reviewed and this checklist is tightened with named-file moves.

Phase 1 evidence captured on 2026-04-29:

- Inventory: `docs/internal/architecture/generated/runtime-host-surface-inventory.md`.
- Resolved inversion: `crates/trust-runtime/src/control.rs` no longer imports `crate::web::pairing::PairingStore`; pairing storage moved behind `crate::security::pairing`.
- Important review candidates before Phase 4: `crates/trust-runtime/src/web/runtime_cloud_policy.rs`, `crates/trust-runtime/src/web/runtime_cloud_state/links.rs`, `crates/trust-runtime/src/web/runtime_cloud_state/rollouts.rs`, `crates/trust-runtime/src/web/runtime_cloud_routes/control_proxy.rs`, and HMI write/snapshot coupling in `crates/trust-runtime/src/control/hmi_handlers_write.rs` plus `crates/trust-runtime/src/hmi/runtime_views/values_writes.rs`.

## Phase 2 - Port Design

- [x] `RTHOST-P2-001` Define runtime value read port.
- [x] `RTHOST-P2-002` Define runtime value write port with authorization/write-policy hook.
- [x] `RTHOST-P2-003` Define runtime snapshot/status port.
- [x] `RTHOST-P2-004` Define HMI schema/descriptor port.
- [x] `RTHOST-P2-005` Define HMI event/delta stream port.
- [x] `RTHOST-P2-006` Define runtime-cloud projection port.
- [x] `RTHOST-P2-007` Keep ports narrow and testable; no web request/response types in domain ports.

Phase 2 port definitions captured on 2026-04-29:

| Port | Owner | Inputs | Outputs | Forbidden dependency | First code-backed target |
| --- | --- | --- | --- | --- | --- |
| Runtime value read port | Runtime/control boundary | Resource name, runtime metadata handle, optional immutable runtime snapshot, optional HMI point IDs. | HMI value result and quality/freshness metadata already shaped by HMI contracts. | No `tiny_http`, websocket, or web route types. | Replace direct `ControlState`/snapshot coupling in `control/hmi_handlers_read.rs` and `hmi/runtime_views/values_writes.rs`. |
| Runtime value write port | Control/runtime boundary | HMI target id/path, typed value candidate, caller role, HMI customization write policy, runtime snapshot lookup. | Queued write command or structured rejection reason. | No web auth response types; no browser DTOs. | Split approval from side effect in `control/hmi_handlers_write.rs`. |
| Runtime snapshot/status port | Runtime/control boundary | Resource/runtime id and optional discovery/runtime status context. | Stable runtime status and snapshot summary suitable for HMI and cloud projection. | No route-specific JSON body parsing or remote HTTP client. | Replace direct status/snapshot reads in web runtime-cloud state/proxy helpers before Phase 4 moves. |
| HMI schema/descriptor port | HMI domain with control adapter | Project root/source registry, runtime metadata, optional snapshot, HMI customization/descriptor state. | HMI schema, descriptor revision/error, descriptor reload result. | No web server state and no websocket session state. | Keep `hmi` schema ownership while narrowing `control/hmi_handlers_descriptor.rs` and `control/hmi_handlers_state.rs`. |
| HMI event/delta stream port | HMI/control semantics, web transport adapter | Previous observed HMI state, current schema/value/alarm results, polling/event clock. | Delta events such as `hmi.values.delta`, `hmi.schema.revision`, and `hmi.alarms.event`. | No tungstenite/tiny-http types in HMI/control logic. | Keep `web/hmi_ws.rs` as adapter; move delta payload calculation only if Phase 4 names it. |
| Runtime-cloud projection/preflight port | Runtime-cloud domain with web adapter | Runtime-cloud action request, target status map, caller role, profile/TLS/allowlist policy, optional HA coordinator state. | Preflight report, target decisions, projected UI state, reason codes. | No direct web request/response types and no runtime execution side effects. | Split reusable policy from `web/runtime_cloud_policy.rs` and state logic from `web/runtime_cloud_state/*` only after Phase 4 rows are reviewed. |

Port design constraints:

- Ports are request/response contracts, not broad service objects; each first code-backed target above should stay independently testable.
- Web adapters may perform HTTP auth, body parsing, TLS-origin checks, websocket transport, and response serialization.
- HMI and runtime-cloud domain ports may depend on domain contracts and immutable runtime snapshots, but must not import `web`.
- Control ports may own authorization/write side effects, but must not import web implementation types; the former `PairingStore` inversion was removed and must not be reintroduced.
- Code-backed target landed on 2026-04-29: `crates/trust-runtime/src/control/hmi_runtime_ports.rs` owns the narrow HMI runtime read/write control port. `control/hmi_handlers_read.rs` and `control/hmi_handlers_write.rs` now delegate to it, preserving `hmi.schema.get`, `hmi.values.get`, `hmi.trends.get`, `hmi.alarms.get`, and `hmi.write` response contracts.
- `host_surface.approved_ports_active` is now `true`: the matching direct web runtime-state doctor rule is active and fails web route code that reaches into runtime/control state fields instead of going through approved ports.

## Phase 3 - Doctor Rules

- [x] `RTHOST-P3-001` Forbid `control -> web` implementation imports.
- [x] `RTHOST-P3-002` Forbid `hmi -> web` implementation imports.
- [x] `RTHOST-P3-003` Forbid `runtime_cloud -> web` implementation imports unless explicitly route-adapter scoped.
- [x] `RTHOST-P3-004` Forbid direct runtime state access from web routes when approved ports exist.
- [x] `RTHOST-P3-005` Require new HMI/web/control/cloud files to declare owner category in subsystem map or config.

Phase 3 evidence captured on 2026-04-29:

- `xtask/config/full_map_policy.json` forbids production `control -> web`, `hmi -> web`, and `runtime_cloud -> web` implementation imports.
- `xtask/config/full_map_policy.json` has `host_surface.owned_paths` owner categories for `control`, `hmi`, `web`, `ui`, and `runtime_cloud` roots/subtrees.
- `xtask/src/full_map.rs` fixtures prove an unallowlisted host-surface import fails, test-only imports are ignored, `runtime_cloud -> web` fails, and host-surface files without owner category fail.
- `RTHOST-P3-004` is enforced by `xtask/src/full_map.rs`: web route files that directly access `ControlState` runtime fields such as `debug`, `metadata`, `io_snapshot`, `project_root`, `resource_name`, `hmi_descriptor`, or `historian` fail `FULLMAP-CHECK-07` when approved ports are active. `crates/trust-runtime/src/web/ui_routes.rs` now receives HMI asset roots through the control-owned `hmi_asset_project_root_port`, and runtime-cloud/config UI routes use `runtime_resource_name_port` instead of reading `ControlState` fields directly.

Validation evidence captured on 2026-04-30:

- `RUSTUP_TOOLCHAIN=1.95 cargo run -p xtask -- architecture-doctor --full-map` passes `FULLMAP-CHECK-07` with `approved ports active: true` and `direct web runtime-state bypass findings: 0`.
- `RUSTUP_TOOLCHAIN=1.95 cargo test -p xtask host_surface -- --nocapture` passes the known-bad and no-bypass CHECK-07 fixtures.
- `RUSTUP_TOOLCHAIN=1.95 cargo test -p trust-runtime --lib control::tests::hmi_ -- --nocapture` passes 17 HMI control-port tests.
- `RUSTUP_TOOLCHAIN=1.95 cargo test -p trust-runtime --test web_io_config_integration runtime_cloud -- --nocapture` passes 40 runtime-cloud route/proxy tests.
- `RUSTUP_TOOLCHAIN=1.95 cargo test -p trust-runtime --test hmi_readonly_integration -- --nocapture` passes 19 HMI route/asset integration tests.
- `RUSTUP_TOOLCHAIN=1.95 cargo clippy -p xtask --all-targets -- -D warnings` and `RUSTUP_TOOLCHAIN=1.95 cargo clippy -p trust-runtime --all-targets -- -D warnings` pass for the touched crates.

## Phase 4 - Named-File Extraction

Phase 4 starts only after Phase 1 inventory adds exact named-file moves. The known F11 seed set below is the reviewed starting point from `RTHOST-P1-008`; code movement is still gated by the unchecked implementation rows below.

Named-file move table from `RTHOST-P1-008`:

| Source file or glob | Current owner | Target owner | Action | Public API / behavior-lock / rollback | Required tests | Doctor rule |
| --- | --- | --- | --- | --- | --- | --- |
| `crates/trust-runtime/src/control.rs` `PairingStore` dependency | control root, previously with temporary web import | `security::pairing` shared auth boundary | moved | Public API: `web::pairing` remains as compatibility re-export; behavior lock: pairing/auth control requests keep current semantics; rollback: restore re-exported store path while keeping CHECK-07 failing on direct control-to-web imports. | `cargo test -p trust-runtime control::tests`, `cargo test -p trust-runtime --test web_ide_integration` focused pairing/auth cases | `FULLMAP-CHECK-07` temporary `control -> web` allowlist removed after split. |
| `crates/trust-runtime/src/control/hmi_handlers*.rs` | control HMI port | control port plus HMI-domain helpers where duplicated | split selectively | Public API: keep control request names stable; behavior lock: HMI schema/value/write JSON remains stable; rollback: keep existing handlers and do not move side effects until tests pass. Current code-backed port: `control/hmi_runtime_ports.rs` handles HMI runtime reads/writes without web types. | `cargo test -p trust-runtime --lib control::tests::hmi_`, `cargo test -p trust-runtime --test hmi_readonly_integration` | `host_surface.owned_paths`, `control -> web` forbidden edge. |
| `crates/trust-runtime/src/hmi/runtime_views/values_writes.rs` | HMI domain coupled to runtime snapshot types | HMI domain using runtime value read/write ports | keep then narrow | Public API: HMI schema/value contracts stay stable; behavior lock: value quality/freshness/write-target resolution stable; rollback: port wrapper can delegate to current functions. | `cargo test -p trust-runtime --lib hmi::tests`, HMI control tests | `hmi -> web` forbidden edge and future approved-port rule. |
| `crates/trust-runtime/src/web/hmi_ws.rs` | web HMI adapter | web adapter only plus HMI-owned event stream semantics | split | Public API: websocket event names stay stable; behavior lock: `hmi.values.delta`, `hmi.schema.revision`, and `hmi.alarms.event` payloads stable; rollback: keep websocket transport in web and move only `hmi/runtime_views/events.rs` semantics back if needed. | `cargo test -p trust-runtime --test hmi_readonly_integration`; Playwright required for browser-visible websocket/UI behavior changes | `host_surface.owned_paths`, future approved HMI event/delta port rule. |
| `crates/trust-runtime/src/web/ui_routes.rs` HMI route sections | web route/static adapter | web adapter only | adapter-only | Public API: `/hmi`, `/hmi/export.json`, `/hmi/app.js`, `/hmi/styles.css`, `/hmi/modules/*`, and `/ws/hmi` stay stable; rollback: keep route dispatch unchanged. | `cargo test -p trust-runtime --test hmi_readonly_integration`; Playwright for browser-visible changes | `host_surface.owned_paths`; no HMI domain logic in web routes. |
| `crates/trust-runtime/src/web/runtime_cloud_policy.rs` | web cloud adapter/policy | runtime-cloud domain policy plus web auth/TLS adapter | split | Public API: preflight denial codes/reasons stay stable; behavior lock: WAN/profile/TLS policy cases stable; rollback: keep function in web until runtime-cloud port proves parity. | `cargo test -p trust-runtime --test web_io_config_integration runtime_cloud_preflight`, `cargo test -p trust-runtime --test runtime_cloud_architecture` | `runtime_cloud -> web` forbidden edge and future approved runtime-cloud projection/preflight port rule. |
| `crates/trust-runtime/src/web/runtime_cloud_state/links.rs` | web cloud state adapter | split runtime-cloud link preference/projection logic from web persistence/HTTP adapter | split | Public API: link transport state endpoints and config keys stay stable; behavior lock: transport preference audit/roundtrip/projection stable; rollback: keep persistence in web and delegate only pure projection. | `cargo test -p trust-runtime --test web_io_config_integration runtime_cloud_link`, `cargo test -p trust-runtime --test web_ide_integration runtime_cloud` | `host_surface.owned_paths`, future runtime-cloud projection port rule. |
| `crates/trust-runtime/src/web/runtime_cloud_state/rollouts.rs` | web cloud state adapter | runtime-cloud rollout state machine plus web persistence adapter | split | Public API: rollout route responses stay stable; behavior lock: queued/staging/applying/verified/failed/aborted transitions stable; rollback: move only pure state transition helpers first. | `cargo test -p trust-runtime --test web_io_config_integration runtime_cloud_rollout` | `host_surface.owned_paths`, future runtime-cloud projection port rule. |
| `crates/trust-runtime/src/web/runtime_cloud_state/config.rs` | web cloud config state adapter | runtime-cloud config-agent state model plus web persistence adapter | split | Public API: desired/reported config JSON, ETag, revision, and status semantics stay stable; rollback: keep file in web and extract pure merge/hash/state helpers first. | `cargo test -p trust-runtime --test web_io_config_integration runtime_cloud_config` | `host_surface.owned_paths`, future runtime-cloud projection port rule. |
| `crates/trust-runtime/src/web/runtime_cloud_routes/control_proxy.rs` | web route adapter with direct control dependency | web adapter using approved control proxy port | adapter-only then narrow | Public API: remote control proxy routes and ACL denials stay stable; behavior lock: viewer/engineer permission handling stable; rollback: keep direct control dependency until port tests pass. | `cargo test -p trust-runtime --test web_io_config_integration runtime_cloud_control_proxy` | future approved control proxy port rule. |
| `crates/trust-runtime/src/web/runtime_cloud_routes/io_proxy.rs` | web route adapter | web adapter using approved IO/control proxy port | adapter-only then narrow | Public API: IO proxy routes and audit semantics stay stable; behavior lock: remote IO config reads/writes stable; rollback: keep direct request construction until port tests pass. | `cargo test -p trust-runtime --test web_io_config_integration runtime_cloud_io_config_proxy` | future approved control/IO proxy port rule. |

- [x] `RTHOST-P4-001` Replace the template row above with reviewed named-file rows from `RTHOST-P1-008`.
- [x] `RTHOST-P4-002` For every `move` or `split` row, record destination module, owner, public API change, behavior-lock tests, and rollback plan.
- [x] `RTHOST-P4-003` For every `adapter-only` web route row, replace direct domain/runtime access with approved control/HMI/cloud ports. Evidence: web route/control helpers now use `dispatch_web_control_request_port`, `runtime_resource_name_port`, and `hmi_asset_project_root_port`; `FULLMAP-CHECK-07` fails direct web `handle_request_value` dispatch and direct runtime-state field access when approved ports are active.
- [x] `RTHOST-P4-004` For every runtime-cloud route/state row, replace direct runtime execution ownership with runtime-cloud projection contracts. Evidence: profile, link, rollout, config, control-proxy, and IO-proxy runtime-cloud rows now delegate pure policy/state/action planning to `runtime_cloud::*_policy` modules while `web/runtime_cloud_*` files remain HTTP/filesystem/discovery adapters.
- [x] `RTHOST-P4-005` For every duplicated DTO/schema/auth/write-check row, identify the canonical owner before deleting duplicates.
- [x] `RTHOST-P4-006` Keep browser assets and websocket details in web; any browser-visible row requires Playwright evidence in the implementation branch.
- [x] `RTHOST-P4-007` Decide exact action for `crates/trust-runtime/src/web/hmi_ws.rs`: keep websocket handshake/session/tungstenite/poll timers in web; split HMI event/delta semantics into the HMI domain.
- [x] `RTHOST-P4-008` Decide exact action for `crates/trust-runtime/src/control/hmi_handlers*.rs`: keep HTTP-neutral control handlers, with runtime value read/write access delegated to `control/hmi_runtime_ports.rs`; HMI schema/contracts stay in `hmi`.
- [x] `RTHOST-P4-009` Decide exact action for `crates/trust-runtime/src/runtime_cloud/` files and `crates/trust-runtime/src/web/runtime_cloud_*` route/state files.
- [x] `RTHOST-P4-010` Add the reviewed named-file move table to this checklist before code movement.

Phase 4 adapter-port evidence captured on 2026-04-30:

- `crates/trust-runtime/src/control.rs::dispatch_web_control_request_port` is the approved control-owned bridge for web routes that need authenticated local control dispatch.
- `crates/trust-runtime/src/web/auth_helpers.rs::dispatch_control_request` now delegates to the control-owned port instead of calling `handle_request_value` directly.
- `crates/trust-runtime/src/web/ui_routes.rs` `/hmi/export.json` and `crates/trust-runtime/src/web/runtime_cloud_state/config.rs` config-agent apply flow now use the approved web dispatch helper.
- `xtask/src/full_map.rs` records `direct web control-dispatch bypass findings: 0` and has a known-bad CHECK-07 fixture for direct web `handle_request_value` dispatch.
- Validation: `RUSTUP_TOOLCHAIN=1.95 cargo run -p xtask -- architecture-doctor --full-map`, `RUSTUP_TOOLCHAIN=1.95 cargo test -p xtask direct_control_dispatch -- --nocapture`, `RUSTUP_TOOLCHAIN=1.95 cargo test -p trust-runtime --test web_io_config_integration runtime_cloud_config_agent -- --nocapture`, and `RUSTUP_TOOLCHAIN=1.95 cargo test -p trust-runtime --test hmi_readonly_integration hmi_standalone_export -- --nocapture` pass.

Phase 4 runtime-cloud extraction evidence captured on 2026-04-30:

- `crates/trust-runtime/src/web/runtime_cloud_policy.rs` moved to `crates/trust-runtime/src/runtime_cloud/profile_policy.rs`; web now imports the profile/TLS/WAN allowlist policy as a runtime-cloud domain contract instead of owning it.
- `crates/trust-runtime/src/runtime_cloud/link_policy.rs` now owns link transport preference state, TOML preference seeding, topology channel projection, feature flags, and host-group grouping policy. `crates/trust-runtime/src/web/runtime_cloud_state/links.rs` remains the web adapter for filesystem persistence, mutex locking, and discovery-backed same-host checks.
- `crates/trust-runtime/src/runtime_cloud/rollout_policy.rs` now owns rollout manager state, rollout record/target contracts, create/action/reconcile policy, terminal-state checks, and queued/staging/applying/verified/failed/aborted transitions. `crates/trust-runtime/src/web/runtime_cloud_state/rollouts.rs` remains the web adapter for filesystem persistence, mutex locking, HTTP request DTO translation, and config-state view projection.
- `crates/trust-runtime/src/runtime_cloud/config_policy.rs` now owns config-agent state/snapshot contracts, desired-write policy, deterministic JSON merge/hash behavior, config apply/reconcile state transitions, and config error classification. `crates/trust-runtime/src/web/runtime_cloud_state/config.rs` remains the web adapter for filesystem persistence, mutex locking, HTTP request DTO translation, and control dispatch side effects.
- `crates/trust-runtime/src/runtime_cloud/control_proxy_policy.rs` now owns control-proxy request validation, runtime-cloud action planning, local control request payload shaping, request-id defaulting, and role-denial reason-code selection. `crates/trust-runtime/src/web/runtime_cloud_routes/control_proxy.rs` remains the web adapter for auth, POST policy, body decoding, local control dispatch through `control_request_required_role_port`/`dispatch_control_request`, and remote HTTP forwarding.
- `crates/trust-runtime/src/runtime_cloud/io_proxy_policy.rs` now owns IO-proxy target/actor validation, read/write action planning, request-id defaulting, and preflight payload classification. `crates/trust-runtime/src/web/runtime_cloud_routes/io_proxy.rs` remains the web adapter for auth, POST policy, body decoding, local `io.toml` load/save, remote HTTP forwarding, and transport error shaping.
- `crates/trust-runtime/tests/runtime_cloud_architecture.rs` includes `runtime_cloud/config_policy.rs` in the runtime-cloud no-transport-import assertion.
- `crates/trust-runtime/tests/runtime_cloud_architecture.rs` includes `runtime_cloud/control_proxy_policy.rs` in the runtime-cloud no-transport-import assertion.
- `crates/trust-runtime/tests/runtime_cloud_architecture.rs` includes `runtime_cloud/io_proxy_policy.rs` in the runtime-cloud no-transport-import assertion.
- `crates/trust-runtime/tests/runtime_cloud_architecture.rs` includes `runtime_cloud/profile_policy.rs` in the runtime-cloud no-transport-import assertion.
- `crates/trust-runtime/tests/runtime_cloud_architecture.rs` includes `runtime_cloud/link_policy.rs` in the runtime-cloud no-transport-import assertion.
- `crates/trust-runtime/tests/runtime_cloud_architecture.rs` includes `runtime_cloud/rollout_policy.rs` in the runtime-cloud no-transport-import assertion.
- Validation: `RUSTUP_TOOLCHAIN=1.95 cargo test -p trust-runtime --lib web::runtime_cloud_state::links -- --nocapture`, `RUSTUP_TOOLCHAIN=1.95 cargo test -p trust-runtime --test web_io_config_integration runtime_cloud_link -- --nocapture`, `RUSTUP_TOOLCHAIN=1.95 cargo test -p trust-runtime --test web_ide_integration runtime_cloud -- --nocapture`, `RUSTUP_TOOLCHAIN=1.95 cargo test -p trust-runtime --test runtime_cloud_architecture -- --nocapture`, `RUSTUP_TOOLCHAIN=1.95 cargo run -p xtask -- architecture-doctor --full-map`, and `RUSTUP_TOOLCHAIN=1.95 cargo clippy -p trust-runtime --all-targets -- -D warnings` pass.
- Rollout validation: `RUSTUP_TOOLCHAIN=1.95 cargo test -p trust-runtime --lib runtime_cloud::rollout_policy -- --nocapture`, `RUSTUP_TOOLCHAIN=1.95 cargo test -p trust-runtime --test web_io_config_integration runtime_cloud_rollout -- --nocapture`, `RUSTUP_TOOLCHAIN=1.95 cargo test -p trust-runtime --test runtime_cloud_architecture -- --nocapture`, `RUSTUP_TOOLCHAIN=1.95 cargo run -p xtask -- architecture-doctor --full-map`, and `RUSTUP_TOOLCHAIN=1.95 cargo clippy -p trust-runtime --all-targets -- -D warnings` pass.
- Config validation: `RUSTUP_TOOLCHAIN=1.95 cargo test -p trust-runtime --lib runtime_cloud::config_policy -- --nocapture`, `RUSTUP_TOOLCHAIN=1.95 cargo test -p trust-runtime --test web_io_config_integration runtime_cloud_config -- --nocapture`, `RUSTUP_TOOLCHAIN=1.95 cargo test -p trust-runtime --test runtime_cloud_architecture -- --nocapture`, `RUSTUP_TOOLCHAIN=1.95 cargo run -p xtask -- architecture-doctor --full-map`, and `RUSTUP_TOOLCHAIN=1.95 cargo clippy -p trust-runtime --all-targets -- -D warnings` pass.
- Control-proxy validation: `RUSTUP_TOOLCHAIN=1.95 cargo test -p trust-runtime --lib runtime_cloud::control_proxy_policy -- --nocapture`, `RUSTUP_TOOLCHAIN=1.95 cargo test -p trust-runtime --test web_io_config_integration runtime_cloud_control_proxy -- --nocapture`, `RUSTUP_TOOLCHAIN=1.95 cargo test -p trust-runtime --test runtime_cloud_architecture -- --nocapture`, `RUSTUP_TOOLCHAIN=1.95 cargo run -p xtask -- architecture-doctor --full-map`, and `RUSTUP_TOOLCHAIN=1.95 cargo clippy -p trust-runtime --all-targets -- -D warnings` pass.
- IO-proxy validation: `RUSTUP_TOOLCHAIN=1.95 cargo test -p trust-runtime --lib runtime_cloud::io_proxy_policy -- --nocapture`, `RUSTUP_TOOLCHAIN=1.95 cargo test -p trust-runtime --test web_io_config_integration runtime_cloud_io_config_proxy -- --nocapture`, `RUSTUP_TOOLCHAIN=1.95 cargo test -p trust-runtime --test runtime_cloud_architecture -- --nocapture`, `RUSTUP_TOOLCHAIN=1.95 cargo run -p xtask -- architecture-doctor --full-map`, and `RUSTUP_TOOLCHAIN=1.95 cargo clippy -p trust-runtime --all-targets -- -D warnings` pass.

Phase 4 duplicate-owner decisions captured on 2026-04-30:

| Duplicate surface | Canonical owner | Adapter/secondary owner | Deletion guard |
| --- | --- | --- | --- |
| HMI schema/result DTOs and widget/page schema contracts | `crates/trust-runtime/src/hmi/contracts*.rs` plus `hmi/runtime_views/schema.rs` | `control/hmi_handlers*.rs` may expose HTTP-neutral control responses; `web/ui_routes.rs` only serializes route responses. | Do not delete or duplicate schema fields outside HMI unless the HMI contract tests and `hmi.schema.get` route tests are updated together. |
| HMI values, trends, alarms, and freshness projection | `hmi/runtime_views/*` | `control/hmi_runtime_ports.rs` owns runtime metadata/snapshot access for control requests; web only asks control for JSON. | No web route may read runtime state directly; `FULLMAP-CHECK-07` is the drift gate. |
| HMI write target/value policy | HMI customization owns write enable/allowlist data; `control/hmi_runtime_ports.rs` owns side-effect approval and queued runtime write dispatch. | Web owns caller authentication and request-body parsing only. | Preserve control-port tests for allowlist, read-only, type mismatch, and queued write behavior before deleting duplicated checks. |
| HMI websocket delta payloads | `hmi/runtime_views/events.rs` owns changed-value delta calculation, schema revision events, widget subscription ids, and alarm payload de-duplication. | `web/hmi_ws.rs` owns handshake validation, tungstenite session framing, control request polling, request auth token propagation, and send errors. | Browser-visible websocket changes require Playwright evidence under `RTHOST-P4-006`; keep `hmi_readonly_integration` websocket tests green. |
| Runtime-cloud public API DTOs, reason codes, and preflight target contracts | `runtime_cloud/contracts.rs`, `runtime_cloud/routing.rs`, and `runtime_cloud/projection.rs` | `web/models.rs` may define route-body request DTOs and aggregate dispatch responses. | Web DTOs must stay route-private unless promoted into runtime-cloud contracts with route and contract tests. |
| Runtime-cloud profile, TLS/auth posture, WAN write allowlist, and preflight reason mapping | `runtime_cloud/profile_policy.rs` | Web passes auth mode, TLS state, caller role, and configured WAN rules as context. | Do not reintroduce profile/TLS/WAN policy in routes; keep runtime-cloud architecture tests covering no transport imports. |
| Runtime-cloud link transport preference and topology projection policy | `runtime_cloud/link_policy.rs` | `web/runtime_cloud_state/links.rs` owns filesystem persistence, mutex access, discovery lookup, and HTTP serialization. | Delete only duplicated projection/state logic after link route and web IDE runtime-cloud tests prove unchanged responses. |
| Runtime-cloud rollout state machine, record/target DTOs, and reconcile/action policy | `runtime_cloud/rollout_policy.rs` | `web/runtime_cloud_state/rollouts.rs` owns persistence, locking, and route payload translation. | Keep rollout unit tests plus route integration tests before removing any remaining web-local state-machine helper. |
| Runtime-cloud config-agent state, desired/reported merge/hash behavior, and reconcile error classification | `runtime_cloud/config_policy.rs` | `web/runtime_cloud_state/config.rs` owns persistence, locking, route DTO translation, and control-dispatch side effects. | Keep config-policy tests and config-agent route tests before deleting adapter duplicates. |
| Runtime-cloud control-proxy request validation, control payload shaping, action type selection, request id, and role-denial reason codes | `runtime_cloud/control_proxy_policy.rs` plus `control::control_request_required_role_port` for control-role classification. | `web/runtime_cloud_routes/control_proxy.rs` owns HTTP auth, POST/TLS policy, body decoding, local control dispatch, and remote HTTP forwarding. | Do not duplicate role/action classification in the route; keep control-proxy route tests and policy tests before deleting route-local copies. |
| Runtime-cloud IO-proxy target/actor validation and read/write action planning | `runtime_cloud/io_proxy_policy.rs` | `web/runtime_cloud_routes/io_proxy.rs` owns HTTP auth, POST/TLS policy, body decoding, local `io.toml` load/save, and remote HTTP forwarding. | Do not move filesystem or transport errors into runtime-cloud policy; keep IO-proxy policy and route tests before deletion. |
| Web authentication and caller-role extraction | `web/auth_helpers.rs` for HTTP request authentication; `security::AccessRole` for role semantics. | Runtime-cloud policies can consume an already-resolved role and produce domain denial codes; control ports can require roles for control commands. | No runtime-cloud or HMI module may parse web auth headers; `FULLMAP-CHECK-07` guards owner imports and route bypasses. |
| Browser assets, UI DTO shaping, and static/export routes | `web/ui/**` and `web/ui_routes.rs` | HMI owns schema contracts; runtime-cloud owns domain projections. | Keep static asset and browser rendering changes behind `RTHOST-P4-006` with Playwright evidence. |

Deletion rule: any future duplicate deletion must name the table row above, keep the canonical owner in place, leave adapters transport-only, and run the listed behavior-lock tests before the checklist can mark that deletion complete.

Phase 4 HMI websocket/event-stream evidence captured on 2026-04-30:

- `crates/trust-runtime/src/hmi/runtime_views/events.rs` now owns the HMI event/delta stream semantics: changed-value deltas, schema revision events, widget subscription IDs derived from schema, and alarm event de-duplication.
- `crates/trust-runtime/src/web/hmi_ws.rs` remains the web adapter for websocket upgrade validation, tungstenite session lifecycle, polling intervals, dispatching HMI control requests, token propagation, and JSON frame sending.
- Browser assets remain under `crates/trust-runtime/src/web/ui/**`; no browser asset file moved into HMI/control/runtime-cloud.
- Validation: `RUSTUP_TOOLCHAIN=1.95 cargo test -p trust-runtime --lib hmi::tests::hmi_event_stream -- --nocapture` passed 3 event-stream unit tests.
- Validation: `RUSTUP_TOOLCHAIN=1.95 cargo test -p trust-runtime --test hmi_readonly_integration hmi_websocket -- --nocapture` passed the 5 websocket integration tests covering value/schema/alarm events, local latency, forced failure recovery, reconnect churn, and slow consumers.
- Browser evidence: live `/hmi` Playwright pass against `http://127.0.0.1:18082/hmi` observed `hmi.values.delta` and `hmi.alarms.event` websocket frames, verified connected/fresh UI state with no placeholder values, and wrote screenshot evidence to `target/playwright/hmi-event-stream.png`.

Phase 4 runtime-cloud exact action decisions captured on 2026-04-30:

| Surface | Exact action | Canonical owner | Guard before future deletion/move |
| --- | --- | --- | --- |
| `crates/trust-runtime/src/runtime_cloud/mod.rs`, `contracts.rs`, `routing.rs`, `projection.rs`, `ha.rs`, `keyspace.rs` | Keep in runtime-cloud domain. These files define public contracts, reason codes, action routing, HA policy contracts, keyspace, and UI projection rules. | Runtime-cloud domain. | `runtime_cloud_core_modules_do_not_import_transport_layers` must continue to reject web/discovery/mesh/runtime transport imports. |
| `runtime_cloud/profile_policy.rs`, `link_policy.rs`, `rollout_policy.rs`, `config_policy.rs`, `control_proxy_policy.rs`, `io_proxy_policy.rs` | Keep as runtime-cloud policy/state/action-planning modules. They are the extracted pure owners from Phase 4. | Runtime-cloud domain with `pub(crate)` policy surfaces unless an external API requires promotion. | Keep each focused unit test plus the matching route integration test before deleting any route-local fallback. |
| `web/runtime_cloud_routes/mod.rs` | Keep as HTTP route dispatcher and route context owner. It owns tiny-http request matching, web auth context, TLS state, control/discovery/state handles, and adapter wiring. | Web adapter. | Do not move `tiny_http`, auth token, pairing, `Arc<Mutex<_>>`, or route matching into runtime-cloud. |
| `web/runtime_cloud_routes/actions.rs` | Keep as HTTP preflight/dispatch adapter. It reads route bodies, resolves web roles, calls runtime-cloud preflight/profile/HA policy, maps to local control dispatch or remote HTTP forwarding, and serializes aggregate dispatch responses. | Web adapter, with action contracts in `runtime_cloud/routing.rs` and profile policy in `runtime_cloud/profile_policy.rs`. | `runtime_cloud_dispatch_route_uses_contract_preflight_before_dispatch_mapping` must keep preflight before control mapping; future moves need a discovery-neutral dispatch port first. |
| `web/runtime_cloud_dispatch.rs` | Keep as web/discovery adapter for target status maps, preferred URL selection, live socket probes, and HA dispatch gating context. It intentionally touches `DiscoveryState`, mesh reachability, sockets, and HA coordinator locks. | Web adapter feeding runtime-cloud routing/profile/HA policy. | Split only after a narrow discovery/status provider exists; never import discovery/socket probing into `runtime_cloud`. |
| `web/runtime_cloud_helpers.rs` | Keep as route response mapping helper for control errors, remote HTTP status, HA result records, and audit IDs. | Web adapter. | Promote to runtime-cloud only if a non-web caller needs the same mapping and route status-code concerns are removed. |
| `web/runtime_cloud_routes/config.rs` and `web/runtime_cloud_state/config.rs` | Keep route/persistence/locking/control-dispatch side effects in web; `runtime_cloud/config_policy.rs` owns config-agent state shape, desired/reported merge/hash behavior, reconcile action, and error classification. | Runtime-cloud config policy plus web adapter. | Keep config-policy tests and `runtime_cloud_config` / `runtime_cloud_config_agent` route tests before deleting any adapter code. |
| `web/runtime_cloud_routes/links.rs` and `web/runtime_cloud_state/links.rs` | Keep route body parsing, filesystem persistence, mutex locking, discovery same-host checks, and HTTP serialization in web; `runtime_cloud/link_policy.rs` owns preference/projection/grouping policy. | Runtime-cloud link policy plus web adapter. | Keep link route tests, web IDE runtime-cloud tests, and same-host transport denial behavior before future deletion. |
| `web/runtime_cloud_routes/rollouts.rs` and `web/runtime_cloud_state/rollouts.rs` | Keep route payload translation, persistence, locking, and config-state view assembly in web; `runtime_cloud/rollout_policy.rs` owns rollout records, state machine, action handling, and reconciliation policy. | Runtime-cloud rollout policy plus web adapter. | Keep rollout policy tests and rollout route tests before moving or deleting web helpers. |
| `web/runtime_cloud_routes/control_proxy.rs` | Keep HTTP auth, POST/TLS policy, body decoding, local control dispatch, remote HTTP forwarding, and response serialization in web; `runtime_cloud/control_proxy_policy.rs` owns validation/action/control-payload planning. | Runtime-cloud control-proxy policy plus web/control adapters. | Keep control-proxy policy tests and `runtime_cloud_control_proxy` route tests before future deletion. |
| `web/runtime_cloud_routes/io_proxy.rs` | Keep HTTP auth, POST/TLS policy, query/body parsing, local `io.toml` load/save, remote HTTP forwarding, filesystem and transport errors in web; `runtime_cloud/io_proxy_policy.rs` owns target/actor validation and read/write action planning. | Runtime-cloud IO-proxy policy plus web adapter. | Keep IO-proxy policy tests and `runtime_cloud_io_config_proxy` route tests before future deletion. |
| `web/models.rs` runtime-cloud request/response structs | Keep route-private request DTOs in web for now. Public runtime-cloud API contracts remain in `runtime_cloud/contracts.rs`, `routing.rs`, and `projection.rs`. | Web adapter for route bodies; runtime-cloud for public contracts. | Promote a DTO into runtime-cloud only when it is reused outside web or becomes part of a documented external contract. |
| `web/config_ui_routes.rs` runtime-cloud config-mode endpoints | Keep as config-UI adapter. It owns config-mode workspace/file/runtime lifecycle HTTP behavior and uses runtime-cloud projection/policy helpers where needed. | Web/config UI adapter. | Treat large-file splitting separately under runtime-large-file work; do not use `RTHOST-P4-009` as permission to move config-UI transport into runtime-cloud. |

Exit decision: Phase 4 runtime-cloud extraction is complete when the table above remains true and `FULLMAP-CHECK-07` plus `runtime_cloud_architecture` tests pass. Any later movement must be opened as a new named row with behavior-lock tests.

## Phase 5 - Tests

- [x] `RTHOST-P5-001` Contract tests for HMI schema/descriptor projection.
- [x] `RTHOST-P5-002` Contract tests for HMI write authorization policy.
- [x] `RTHOST-P5-003` Contract tests for runtime snapshot/status projection.
- [x] `RTHOST-P5-004` Contract tests for runtime-cloud projection.
- [x] `RTHOST-P5-005` Route tests prove web remains a thin adapter.
- [x] `RTHOST-P5-006` Browser-visible changes use Playwright verification in implementation branches.

Phase 5 HMI control evidence captured on 2026-04-29:

- `RUSTUP_TOOLCHAIN=1.95 cargo test -p trust-runtime --lib control::tests::hmi_` passed 17/17.
- Direct port behavior-lock tests: `hmi_runtime_read_port_is_code_backed_without_json_transport` and `hmi_runtime_write_port_queues_allowlisted_write_without_json_transport`.
- Existing HMI contract tests in the same run cover schema mapping, values quality/timestamps, descriptor update/reload, write allowlist, write type mismatch, and read-only rejection.

Phase 5 runtime snapshot/status evidence captured on 2026-04-30:

- `crates/trust-runtime/src/control/tests/core.rs::runtime_status_projection_contract_reports_resource_metrics_realtime_and_io_health` locks the control-owned runtime status payload for resource identity, PLC alias, control mode, simulation fields, cycle/fault/overrun/profiling metrics, realtime requested/observed posture, and IO driver health projection.
- `crates/trust-runtime/src/control/tests/core.rs::runtime_health_projection_contract_marks_faulted_driver_unhealthy` locks the health payload so faulted IO drivers make `ok=false` while preserving state, fault, driver status, and driver error fields.
- Validation: `RUSTUP_TOOLCHAIN=1.95 cargo test -p trust-runtime --lib control::tests::runtime_ -- --nocapture` passed both runtime snapshot/status contract tests.

Phase 5 runtime-cloud projection evidence captured on 2026-04-30:

- `crates/trust-runtime/src/runtime_cloud/projection.rs::runtime_cloud_projection_contract_reports_topology_edges_and_warnings` locks the runtime-cloud UI projection contract for local and peer node ordering, active/member roles, lifecycle/health/config state transitions, edge channel/state/metric fields, stale/offline flags, and communication warning timeline entries.
- `crates/trust-runtime/src/runtime_cloud/projection.rs::presence_projection_contract_does_not_stale_future_heartbeat` locks the presence projection edge case where a future heartbeat timestamp must not underflow into a stale/partitioned result.
- Validation: `RUSTUP_TOOLCHAIN=1.95 cargo test -p trust-runtime --lib runtime_cloud::projection::tests -- --nocapture` passed 10 runtime-cloud projection tests.

Phase 5 thin web adapter route evidence captured on 2026-04-30:

- `crates/trust-runtime/tests/runtime_cloud_architecture.rs::runtime_cloud_proxy_routes_are_policy_first_adapters` locks route order so runtime-cloud action dispatch, control proxy, and IO proxy routes run policy/preflight planning before control dispatch, local IO config load, or local IO config save side effects.
- `crates/trust-runtime/tests/runtime_cloud_architecture.rs::runtime_cloud_state_adapters_delegate_domain_state_to_policy_modules` locks the web state adapters as persistence/locking shells over `runtime_cloud::config_policy`, `runtime_cloud::link_policy`, and `runtime_cloud::rollout_policy`.
- Validation: `RUSTUP_TOOLCHAIN=1.95 cargo test -p trust-runtime --test runtime_cloud_architecture -- --nocapture` passed 5 runtime-cloud architecture tests.

Phase 5 browser-visible verification evidence captured on 2026-04-30:

- Browser-visible HMI websocket/event-stream work in this host-surface slice used a live `/hmi` Playwright pass against `http://127.0.0.1:18082/hmi`; the pass observed `hmi.values.delta` and `hmi.alarms.event` websocket frames, verified connected/fresh UI state without placeholder values, and wrote screenshot evidence to `target/playwright/hmi-event-stream.png`.
- `RTHOST-P5-003`, `RTHOST-P5-004`, and `RTHOST-P5-005` branches did not change browser assets, browser-side JavaScript/CSS, `/hmi` static route behavior, or VS Code webviews; they changed Rust contract/architecture tests and checklist evidence only.
- Browser-visible host-surface changes remain gated by `RTHOST-STOP-05`, the repo `AGENTS.md` browser verification rule, and the existing Playwright browser capture entrypoint `scripts/captures/run-playwright-captures.sh browser`; CI browser analysis gates are useful signal but do not replace a live Playwright pass when `/hmi`, web UI, webview, or browser-side assets change.

## Exit Criteria

- [x] `RTHOST-EXIT-01` HMI logic is not split three ways without ownership rules.
- [x] `RTHOST-EXIT-02` `control -> web` inversion is removed or explicitly justified with a removal ticket.
- [x] `RTHOST-EXIT-03` Runtime-cloud does not own runtime execution.
- [x] `RTHOST-EXIT-04` Web route code is transport adapter code, not domain owner.
- [x] `RTHOST-EXIT-05` Doctor rules prevent drift back.

Exit evidence captured on 2026-04-30:

- `RTHOST-EXIT-01`: HMI ownership is now split by contract, not by duplicated logic. `control/hmi_runtime_ports.rs` owns the HTTP-neutral runtime read/write port, `hmi/runtime_views/events.rs` owns HMI websocket event/delta semantics, HMI schema/descriptor contracts remain in `hmi`, and `web/hmi_ws.rs` remains websocket transport only.
- `RTHOST-EXIT-03`: runtime-cloud owns profile/link/rollout/config/control-proxy/IO-proxy policy and projection contracts in `runtime_cloud::*_policy` plus `runtime_cloud::projection`; route/state side effects remain in `web/runtime_cloud_*`. `runtime_cloud_core_modules_do_not_import_transport_layers` prevents runtime-cloud domain modules from importing web/discovery/mesh transport layers.
- `RTHOST-EXIT-04`: `runtime_cloud_proxy_routes_are_policy_first_adapters` and `runtime_cloud_state_adapters_delegate_domain_state_to_policy_modules` lock web route/state files as policy-first adapters before dispatch, local IO load/save, or persistence side effects.
- `RTHOST-EXIT-05`: `FULLMAP-CHECK-07` is active with `approved ports active: true`, host-surface owner path rules, forbidden `control -> web` / `hmi -> web` / `runtime_cloud -> web` implementation imports, and direct web runtime-state/control-dispatch bypass findings at zero.
- Final local validation: `RUSTUP_TOOLCHAIN=1.95 cargo run -p xtask -- architecture-doctor --full-map` passed on merge base `55c510b85` with `FULLMAP-CHECK-07` green.
