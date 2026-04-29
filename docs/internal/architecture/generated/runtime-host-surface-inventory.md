# Runtime Host Surface Inventory

Generated for `RTHOST-P1-008` on 2026-04-29 from branch
`architecture/runtime-cli-product-workbench-split` at release baseline `v0.24.7`.

Scope: `web`, `hmi`, `ui`, `control`, and `runtime_cloud` ownership before
runtime host-surface extraction. This inventory is classification evidence only;
it is not approval to move files. Phase 4 still requires a reviewed named-file
move table in `docs/internal/testing/checklists/runtime-host-surface-ownership-checklist.md`.

## Ownership Model

| Surface | Current role | Proposed owner | Boundary rule |
| --- | --- | --- | --- |
| `hmi` | HMI schema, descriptors, customization, scaffold generation, runtime HMI view projection. | HMI domain. | Must not import web transport. Runtime snapshot/value access should move behind narrow read/write/status ports before extraction. |
| `control` HMI handlers | HTTP-neutral control requests for HMI schema, values, trends, alarms, descriptor reload, and writes. | Control port layer. | May depend on HMI contracts/domain helpers; must not depend on web implementation types. Existing `control.rs -> web::pairing::PairingStore` is a recorded temporary inversion. |
| `web` HMI files | HTTP routes, static browser assets, and websocket adapter. | Web transport/browser adapter. | Should call control/HMI ports and keep browser transport details; HMI domain logic belongs in `hmi` or control ports. |
| `ui` | Terminal/local presentation client. | UI presentation. | May call control APIs; should not own runtime execution, HMI domain schema, or web/browser transport. |
| `runtime_cloud` | Cloud contracts, routing/preflight contracts, HA policy, keyspace, and UI projection types. | Runtime-cloud domain. | Transport modules may depend on `runtime_cloud`; `runtime_cloud` must not depend on web/transport implementation. |
| `web/runtime_cloud_*` | HTTP route adapters, remote dispatch, browser/cloud UI state persistence, and proxy glue. | Web cloud adapter/state. | Keep HTTP/auth/body parsing in web; move reusable cloud policy/projection/state logic to `runtime_cloud` only after Phase 4 names exact files. |

## Current Gate State

- `FULLMAP-CHECK-07` exists in `architecture-doctor --full-map`.
- Current policy forbids `control -> web` and `hmi -> web` production imports.
- A temporary allowlist exists for `crates/trust-runtime/src/control.rs -> crate::web::pairing::PairingStore`; this is the known host-surface inversion to remove.
- `host_surface.approved_ports_active` is currently `false`, so CHECK-07 remains a partial gate until Phase 2 ports are designed and Phase 3 rules are tightened.

## HMI Files

| File | Lines | Current cross-surface imports | Current owner | Proposed owner / action |
| --- | ---: | --- | --- | --- |
| `crates/trust-runtime/src/hmi/catalog.rs` | 64 | - | hmi-domain | Keep in HMI domain. |
| `crates/trust-runtime/src/hmi/contracts.rs` | 4 | - | hmi-domain | Keep in HMI domain. |
| `crates/trust-runtime/src/hmi/contracts_customization.rs` | 259 | - | hmi-domain | Keep in HMI domain. |
| `crates/trust-runtime/src/hmi/contracts_descriptor.rs` | 99 | - | hmi-domain | Keep in HMI domain. |
| `crates/trust-runtime/src/hmi/contracts_internal.rs` | 330 | - | hmi-domain | Keep in HMI domain. |
| `crates/trust-runtime/src/hmi/contracts_schema.rs` | 232 | - | hmi-domain | Keep in HMI domain. |
| `crates/trust-runtime/src/hmi/customization.rs` | 205 | - | hmi-domain | Keep in HMI domain. |
| `crates/trust-runtime/src/hmi/descriptor.rs` | 6 | - | hmi-domain | Keep in HMI domain. |
| `crates/trust-runtime/src/hmi/descriptor_apply.rs` | 120 | - | hmi-domain | Keep in HMI domain. |
| `crates/trust-runtime/src/hmi/descriptor_io.rs` | 231 | - | hmi-domain | Keep in HMI domain. |
| `crates/trust-runtime/src/hmi/descriptor_load_map.rs` | 302 | - | hmi-domain | Keep in HMI domain. |
| `crates/trust-runtime/src/hmi/descriptor_render.rs` | 237 | - | hmi-domain | Keep in HMI domain. |
| `crates/trust-runtime/src/hmi/layout.rs` | 482 | - | hmi-domain | Keep in HMI domain. |
| `crates/trust-runtime/src/hmi/points.rs` | 280 | - | hmi-domain | Keep in HMI domain. |
| `crates/trust-runtime/src/hmi/runtime_views.rs` | 5 | - | hmi-domain | Keep in HMI domain. |
| `crates/trust-runtime/src/hmi/runtime_views/alarms.rs` | 75 | - | hmi-domain | Keep in HMI domain. |
| `crates/trust-runtime/src/hmi/runtime_views/helpers.rs` | 184 | - | hmi-domain | Keep in HMI domain. |
| `crates/trust-runtime/src/hmi/runtime_views/live_trends.rs` | 95 | - | hmi-domain | Keep in HMI domain. |
| `crates/trust-runtime/src/hmi/runtime_views/schema.rs` | 85 | - | hmi-domain | Keep in HMI domain. |
| `crates/trust-runtime/src/hmi/runtime_views/values_writes.rs` | 89 | - | hmi-domain | Keep in HMI domain; replace direct snapshot coupling only after Phase 2 ports exist. |
| `crates/trust-runtime/src/hmi/scaffold.rs` | 7 | - | hmi-domain | Keep in HMI domain. |
| `crates/trust-runtime/src/hmi/scaffold_annotations.rs` | 256 | - | hmi-domain | Keep in HMI domain. |
| `crates/trust-runtime/src/hmi/scaffold_entry.rs` | 458 | - | hmi-domain | Keep in HMI domain. |
| `crates/trust-runtime/src/hmi/scaffold_infer.rs` | 286 | - | hmi-domain | Keep in HMI domain. |
| `crates/trust-runtime/src/hmi/scaffold_overview.rs` | 3 | - | hmi-domain | Keep in HMI domain. |
| `crates/trust-runtime/src/hmi/scaffold_overview_build.rs` | 118 | - | hmi-domain | Keep in HMI domain. |
| `crates/trust-runtime/src/hmi/scaffold_overview_select.rs` | 323 | - | hmi-domain | Keep in HMI domain; replace direct snapshot coupling only after Phase 2 ports exist. |
| `crates/trust-runtime/src/hmi/scaffold_overview_utils.rs` | 126 | - | hmi-domain | Keep in HMI domain. |
| `crates/trust-runtime/src/hmi/scaffold_render.rs` | 3 | - | hmi-domain | Keep in HMI domain. |
| `crates/trust-runtime/src/hmi/scaffold_render_config.rs` | 87 | - | hmi-domain | Keep in HMI domain. |
| `crates/trust-runtime/src/hmi/scaffold_render_overview.rs` | 150 | - | hmi-domain | Keep in HMI domain. |
| `crates/trust-runtime/src/hmi/scaffold_render_process.rs` | 390 | - | hmi-domain | Keep in HMI domain. |
| `crates/trust-runtime/src/hmi/tests.rs` | 3 | - | hmi-test | Keep as HMI tests. |
| `crates/trust-runtime/src/hmi/tests/live_state.rs` | 168 | - | hmi-test | Keep as HMI tests. |
| `crates/trust-runtime/src/hmi/tests/scaffold.rs` | 3 | - | hmi-test | Keep as HMI tests. |
| `crates/trust-runtime/src/hmi/tests/scaffold_generation.rs` | 6 | - | hmi-test | Keep as HMI tests. |
| `crates/trust-runtime/src/hmi/tests/scaffold_generation/core_tests.rs` | 138 | - | hmi-test | Keep as HMI tests. |
| `crates/trust-runtime/src/hmi/tests/scaffold_generation/helpers.rs` | 89 | - | hmi-test | Keep as HMI tests. |
| `crates/trust-runtime/src/hmi/tests/scaffold_generation/mapping_tests.rs` | 64 | - | hmi-test | Keep as HMI tests. |
| `crates/trust-runtime/src/hmi/tests/scaffold_generation/mode_tests.rs` | 79 | - | hmi-test | Keep as HMI tests. |
| `crates/trust-runtime/src/hmi/tests/scaffold_generation/page_tests.rs` | 106 | - | hmi-test | Keep as HMI tests. |
| `crates/trust-runtime/src/hmi/tests/scaffold_generation/update_tests.rs` | 192 | - | hmi-test | Keep as HMI tests. |
| `crates/trust-runtime/src/hmi/tests/scaffold_legacy.rs` | 49 | - | hmi-test | Keep as HMI tests. |
| `crates/trust-runtime/src/hmi/tests/scaffold_loading.rs` | 231 | - | hmi-test | Keep as HMI tests. |
| `crates/trust-runtime/src/hmi/tests/schema.rs` | 349 | - | hmi-test | Keep as HMI tests. |

## Control HMI Handler Files

| File | Lines | Current cross-surface imports | Current owner | Proposed owner / action |
| --- | ---: | --- | --- | --- |
| `crates/trust-runtime/src/control/hmi_handlers.rs` | 20 | - | control-hmi-port | Keep as control request dispatcher. |
| `crates/trust-runtime/src/control/hmi_handlers_descriptor.rs` | 192 | hmi | control-hmi-port | Keep as control port; Phase 4 should move descriptor-domain logic only if it duplicates HMI ownership. |
| `crates/trust-runtime/src/control/hmi_handlers_parse.rs` | 118 | - | control-hmi-port | Keep as control request parsing. |
| `crates/trust-runtime/src/control/hmi_handlers_read.rs` | 159 | hmi | control-hmi-port | Keep as control read port; Phase 2 should define the runtime value/snapshot inputs used by HMI projection. |
| `crates/trust-runtime/src/control/hmi_handlers_state.rs` | 247 | hmi | control-hmi-port | Keep as control state bridge; review live-state mutation for Phase 4 HMI-domain extraction. |
| `crates/trust-runtime/src/control/hmi_handlers_write.rs` | 127 | hmi | control-hmi-port | Keep as control write port; HMI write allowlist and value-template logic should become an explicit Phase 2 write-policy hook. |

## Web HMI Files

| File | Lines | Current cross-surface imports | Current owner | Proposed owner / action |
| --- | ---: | --- | --- | --- |
| `crates/trust-runtime/src/web.rs` | 219 | control, runtime_cloud | web-root | Keep as web module root and shared route constants; do not add domain logic here. |
| `crates/trust-runtime/src/web/hmi_ws.rs` | 220 | - | web-hmi-adapter | Keep as websocket adapter only; it should continue to call `hmi.*` control requests rather than own schema/value logic. |
| `crates/trust-runtime/src/web/ui_routes.rs` | 601 | - | web-hmi-adapter | Keep browser/static route adapter; review route dispatch size separately under large-file/KISS work. |
| `crates/trust-runtime/src/web/ui/hmi.html` | 61 | - | web-asset | Keep as browser asset. |
| `crates/trust-runtime/src/web/ui/hmi.css` | 5 | - | web-asset | Keep as browser asset. |
| `crates/trust-runtime/src/web/ui/hmi.js` | 3 | - | web-asset | Keep as browser asset wrapper; bundled chunks remain under `src/web/ui/chunks`. |

## Runtime-Cloud Domain Files

| File | Lines | Current cross-surface imports | Current owner | Proposed owner / action |
| --- | ---: | --- | --- | --- |
| `crates/trust-runtime/src/runtime_cloud/contracts.rs` | 489 | - | runtime-cloud-domain | Keep in runtime-cloud domain. |
| `crates/trust-runtime/src/runtime_cloud/ha.rs` | 365 | - | runtime-cloud-domain | Keep in runtime-cloud domain. |
| `crates/trust-runtime/src/runtime_cloud/ha/policy.rs` | 119 | - | runtime-cloud-domain | Keep in runtime-cloud domain. |
| `crates/trust-runtime/src/runtime_cloud/ha/tests.rs` | 210 | - | runtime-cloud-test | Keep as runtime-cloud tests. |
| `crates/trust-runtime/src/runtime_cloud/keyspace.rs` | 290 | - | runtime-cloud-domain | Keep in runtime-cloud domain. |
| `crates/trust-runtime/src/runtime_cloud/mod.rs` | 19 | - | runtime-cloud-domain | Keep in runtime-cloud domain; existing module-level boundary comment is canonical. |
| `crates/trust-runtime/src/runtime_cloud/projection.rs` | 513 | - | runtime-cloud-domain | Keep in runtime-cloud domain. |
| `crates/trust-runtime/src/runtime_cloud/routing.rs` | 583 | - | runtime-cloud-domain | Keep in runtime-cloud domain. |

## Web Runtime-Cloud Files

| File | Lines | Current cross-surface imports | Current owner | Proposed owner / action |
| --- | ---: | --- | --- | --- |
| `crates/trust-runtime/src/web/runtime_cloud_dispatch.rs` | 376 | - | web-cloud-route-adapter | Keep as web dispatch adapter; move reusable target selection only if Phase 4 names it. |
| `crates/trust-runtime/src/web/runtime_cloud_helpers.rs` | 65 | runtime_cloud | web-cloud-route-adapter | Keep as web error/status mapping helper unless duplicated in domain. |
| `crates/trust-runtime/src/web/runtime_cloud_policy.rs` | 388 | runtime_cloud | web-cloud-route-adapter | Review for Phase 4 split: profile/allowlist policy may belong in runtime-cloud domain once ports are designed. |
| `crates/trust-runtime/src/web/runtime_cloud_routes/actions.rs` | 385 | runtime_cloud | web-cloud-route-adapter | Keep as HTTP action route adapter. |
| `crates/trust-runtime/src/web/runtime_cloud_routes/config.rs` | 134 | - | web-cloud-route-adapter | Keep as HTTP config route adapter. |
| `crates/trust-runtime/src/web/runtime_cloud_routes/control_proxy.rs` | 375 | control | web-cloud-route-adapter | Keep as web/control proxy adapter; Phase 2 must define the approved control proxy port. |
| `crates/trust-runtime/src/web/runtime_cloud_routes/io_proxy.rs` | 378 | - | web-cloud-route-adapter | Keep as HTTP IO proxy adapter; review direct control request construction in Phase 4. |
| `crates/trust-runtime/src/web/runtime_cloud_routes/links.rs` | 169 | - | web-cloud-route-adapter | Keep as HTTP link route adapter. |
| `crates/trust-runtime/src/web/runtime_cloud_routes/mod.rs` | 94 | - | web-cloud-route-adapter | Keep as route dispatcher. |
| `crates/trust-runtime/src/web/runtime_cloud_routes/rollouts.rs` | 170 | - | web-cloud-route-adapter | Keep as HTTP rollout route adapter; rollout state machine may be Phase 4 extraction candidate. |
| `crates/trust-runtime/src/web/runtime_cloud_routes/state.rs` | 127 | - | web-cloud-route-adapter | Keep as HTTP state route adapter. |
| `crates/trust-runtime/src/web/runtime_cloud_state/config.rs` | 334 | web | web-cloud-state-adapter | Review for Phase 4 extraction; currently persists desired/reported config state under web. |
| `crates/trust-runtime/src/web/runtime_cloud_state/links.rs` | 858 | runtime_cloud, web | web-cloud-state-adapter | Review for Phase 4 extraction; contains transport preference state and UI projection mutation. |
| `crates/trust-runtime/src/web/runtime_cloud_state/mod.rs` | 13 | - | web-cloud-state-adapter | Keep as web state module index until Phase 4. |
| `crates/trust-runtime/src/web/runtime_cloud_state/rollouts.rs` | 460 | web | web-cloud-state-adapter | Review for Phase 4 extraction; contains rollout state machine and persistence. |

## Direct Cross-Surface Imports

Production imports found by scanning `crate::{web,hmi,control,ui,runtime_cloud}` references:

| From file | Imports | Classification |
| --- | --- | --- |
| `crates/trust-runtime/src/web.rs` | control, runtime_cloud | Allowed web adapter dependency on control and runtime-cloud contracts; keep as root/router glue only. |
| `crates/trust-runtime/src/control.rs` | hmi, web | HMI dependency is expected for current control HMI port; web dependency is the temporary `PairingStore` inversion tracked by CHECK-07 allowlist. |
| `crates/trust-runtime/src/ui.rs` | control | Allowed UI presentation dependency on control client API. |
| `crates/trust-runtime/src/control/hmi_handlers_descriptor.rs` | hmi | Expected control-to-HMI domain call. |
| `crates/trust-runtime/src/control/hmi_handlers_read.rs` | hmi | Expected control-to-HMI domain call. |
| `crates/trust-runtime/src/control/hmi_handlers_state.rs` | hmi | Expected control-to-HMI domain call. |
| `crates/trust-runtime/src/control/hmi_handlers_write.rs` | hmi | Expected control-to-HMI domain call. |
| `crates/trust-runtime/src/control/types.rs` | hmi | Expected control response/type exposure for HMI results. |
| `crates/trust-runtime/src/web/auth_helpers.rs` | control | Allowed web adapter dependency for control auth role handling. |
| `crates/trust-runtime/src/web/config_ui_routes.rs` | runtime_cloud | Allowed web/config UI projection dependency. |
| `crates/trust-runtime/src/web/models.rs` | runtime_cloud | Allowed web DTO dependency on runtime-cloud contracts. |
| `crates/trust-runtime/src/web/runtime_cloud_helpers.rs` | runtime_cloud | Allowed web adapter dependency on cloud reason codes. |
| `crates/trust-runtime/src/web/runtime_cloud_policy.rs` | runtime_cloud | Review in Phase 4: policy helpers may move to runtime-cloud domain if reused outside web. |
| `crates/trust-runtime/src/web/runtime_cloud_routes/actions.rs` | runtime_cloud | Allowed HTTP route dependency on cloud request/response contracts. |
| `crates/trust-runtime/src/web/runtime_cloud_routes/control_proxy.rs` | control | Review in Phase 2/4 as approved control proxy port. |
| `crates/trust-runtime/src/web/runtime_cloud_state/config.rs` | web | Internal web module dependency. |
| `crates/trust-runtime/src/web/runtime_cloud_state/links.rs` | runtime_cloud, web | Review in Phase 4: UI projection mutation and state persistence may need domain split. |
| `crates/trust-runtime/src/web/runtime_cloud_state/rollouts.rs` | web | Internal web module dependency. |

Test-only imports:

| From file | Imports | Classification |
| --- | --- | --- |
| `crates/trust-runtime/src/control/tests/helpers.rs` | hmi, web | Test-only helper dependency; ignored by CHECK-07 production forbidden-edge rules. |

## Duplication And Extraction Candidates

| Area | Current location | Risk | Proposed next action |
| --- | --- | --- | --- |
| HMI schema/value projection | HMI projection lives in `hmi/runtime_views/*`; control handlers call it directly with runtime metadata/snapshot. | Runtime snapshot access is still coupled to control state and debug snapshot types. | Phase 2 should define narrow runtime value read, value write, and snapshot/status ports before moving code. |
| HMI write authorization | HMI customization owns write enable/allowlist; `control/hmi_handlers_write.rs` enforces it during control request handling. | Write policy is split between HMI customization data and control write side effects. | Phase 2 should define the write-policy hook and keep side effects in control/runtime execution. |
| HMI websocket deltas | `web/hmi_ws.rs` polls control HMI requests and computes websocket deltas. | Delta transport is web-owned, but polling cadence and payload shape can become hidden domain logic. | Keep as adapter for now; Phase 4 should split only if HMI event/delta stream port is added. |
| Runtime-cloud profile/allowlist policy | `web/runtime_cloud_policy.rs` uses `runtime_cloud` reason codes and routing types. | WAN/profile policy may be reusable domain policy but is currently tied to web auth/TLS context. | Phase 2 should define the runtime-cloud projection/preflight port before moving. |
| Runtime-cloud rollout/config/link state | `web/runtime_cloud_state/*` persists desired/reported state, link transport preference state, and rollout manager state. | Persistent state machine code is web-owned today and may outgrow route-adapter responsibility. | Phase 4 should name exact state files to keep under web or extract to runtime-cloud domain/state service. |
| Runtime-cloud control proxy | `web/runtime_cloud_routes/control_proxy.rs` maps HTTP requests into control requests and preflight/dispatch actions. | Direct control dependency is legitimate adapter work but needs an approved port boundary. | Phase 2 should define the approved control proxy port and Phase 3 should enforce it. |

## Proposed Phase 2 Port List

- Runtime value read port: HMI schema/value projection reads runtime metadata plus optional snapshot without taking `ControlState`.
- Runtime value write port: HMI writes enqueue through a control/runtime side-effect boundary after HMI write policy approves target/value.
- Runtime snapshot/status port: HMI and runtime-cloud projections read status without direct web route access to execution internals.
- HMI schema/descriptor port: web and control expose schema/descriptor data without duplicating descriptor parsing.
- HMI event/delta stream port: websocket code remains web transport, with HMI/control owning payload semantics if the current polling helper is split.
- Runtime-cloud projection/preflight port: web routes call runtime-cloud preflight/projection contracts without owning reusable policy/state logic.

## Checklist Impact

- `RTHOST-P1-001` through `RTHOST-P1-009` are satisfied by this inventory.
- `RTHOST-P1-010` remains open: Phase 4 must not start until this inventory is reviewed and the checklist gains a named-file move table.
- `ARCHPROG-C-02` should remain open until `FULLMAP-CHECK-07` is recorded as the active gate or a waiver is accepted, because `host_surface.approved_ports_active` is still `false`.
