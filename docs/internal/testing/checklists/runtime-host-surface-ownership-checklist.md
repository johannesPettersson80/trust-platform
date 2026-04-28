# Runtime Host Surface Ownership Checklist

Status: Planned
Owner: Runtime/web/HMI/control/cloud
Scope: address audit F11 by defining and enforcing ownership for `web`, `hmi`, `ui`, `control`, and `runtime_cloud`.

## Ownership Target

- [ ] `RTHOST-OWN-01` Runtime/core owns execution state and value/snapshot ports.
- [ ] `RTHOST-OWN-02` Control owns HTTP-neutral command/query contracts and authorization/write policy.
- [ ] `RTHOST-OWN-03` HMI owns schema/contracts/descriptors, not route transport.
- [ ] `RTHOST-OWN-04` Web owns HTTP routes, websocket serving, static assets, and browser transport adapters.
- [ ] `RTHOST-OWN-05` UI owns terminal/local presentation only where it remains.
- [ ] `RTHOST-OWN-06` Runtime-cloud owns cloud projection/contracts and does not own runtime execution.

## Phase 0 - Full-Map Prerequisite

- [ ] `RTHOST-P0-001` Hard prerequisite: `architecture-doctor --full-map` MVP implements `FULLMAP-CHECK-07` for HMI/web/control/cloud ownership and forbidden edges before Phase 3 or Phase 4 starts.
- [ ] `RTHOST-P0-002` If `FULLMAP-CHECK-07` is unavailable, record an owner-approved waiver with the local replacement rule, fixture, owner, and expiration date.
- [ ] `RTHOST-P0-GATE-01` Do not claim `ARCHPROG-C-02` or `ARCHPROG-C-04` complete until `FULLMAP-CHECK-07` or its waiver is recorded.

## Stop Rules

- [ ] `RTHOST-STOP-01` Stop if `control` imports web implementation types.
- [ ] `RTHOST-STOP-02` Stop if HMI domain logic is added directly to web routes instead of HMI/control contracts.
- [ ] `RTHOST-STOP-03` Stop if runtime-cloud route code owns runtime execution decisions.
- [ ] `RTHOST-STOP-04` Stop if a port is so broad that it becomes a hidden runtime god object.
- [ ] `RTHOST-STOP-05` Stop if browser-visible behavior changes without Playwright/browser verification in the implementation branch.

## Phase 1 - Inventory

- [ ] `RTHOST-P1-001` Map all `src/hmi/` files by responsibility.
- [ ] `RTHOST-P1-002` Map all `src/control/hmi_handlers*.rs` files by responsibility.
- [ ] `RTHOST-P1-003` Map `src/web/hmi_ws.rs` and HMI route files by responsibility.
- [ ] `RTHOST-P1-004` Map `src/runtime_cloud/` files by responsibility.
- [ ] `RTHOST-P1-005` Map `src/web/runtime_cloud_*`, `runtime_cloud_routes/*`, and `runtime_cloud_state/*`.
- [ ] `RTHOST-P1-006` Record direct imports among `web`, `hmi`, `control`, `ui`, and `runtime_cloud`.
- [ ] `RTHOST-P1-007` Identify duplicated DTOs, duplicated auth/write checks, duplicated schema projection, and duplicated runtime snapshot logic.
- [ ] `RTHOST-P1-008` Produce `docs/internal/architecture/generated/runtime-host-surface-inventory.md`.
- [ ] `RTHOST-P1-009` Inventory output must include per-file owner, current imports, proposed owner, and proposed action: keep, move, split, delete, or adapter-only.
- [ ] `RTHOST-P1-010` Do not start Phase 4 until Phase 1 inventory is reviewed and this checklist is tightened with named-file moves.

## Phase 2 - Port Design

- [ ] `RTHOST-P2-001` Define runtime value read port.
- [ ] `RTHOST-P2-002` Define runtime value write port with authorization/write-policy hook.
- [ ] `RTHOST-P2-003` Define runtime snapshot/status port.
- [ ] `RTHOST-P2-004` Define HMI schema/descriptor port.
- [ ] `RTHOST-P2-005` Define HMI event/delta stream port.
- [ ] `RTHOST-P2-006` Define runtime-cloud projection port.
- [ ] `RTHOST-P2-007` Keep ports narrow and testable; no web request/response types in domain ports.

## Phase 3 - Doctor Rules

- [ ] `RTHOST-P3-001` Forbid `control -> web` implementation imports.
- [ ] `RTHOST-P3-002` Forbid `hmi -> web` implementation imports.
- [ ] `RTHOST-P3-003` Forbid `runtime_cloud -> web` implementation imports unless explicitly route-adapter scoped.
- [ ] `RTHOST-P3-004` Forbid direct runtime state access from web routes when approved ports exist.
- [ ] `RTHOST-P3-005` Require new HMI/web/control/cloud files to declare owner category in subsystem map or config.

## Phase 4 - Named-File Extraction

Phase 4 starts only after Phase 1 inventory adds exact named-file moves. The known F11 seed set below is the minimum starting point, not a complete move list.

Named-file move table template to fill from `RTHOST-P1-008` before code movement:

| Source file or glob | Current owner | Target owner | Action | Required tests | Doctor rule |
| --- | --- | --- | --- | --- | --- |
| `TBD after RTHOST-P1-008` | `TBD` | `TBD` | `keep/move/split/delete/adapter-only` | `TBD` | `TBD` |

- [ ] `RTHOST-P4-001` Replace the template row above with reviewed named-file rows from `RTHOST-P1-008`.
- [ ] `RTHOST-P4-002` For every `move` or `split` row, record destination module, owner, public API change, behavior-lock tests, and rollback plan.
- [ ] `RTHOST-P4-003` For every `adapter-only` web route row, replace direct domain/runtime access with approved control/HMI/cloud ports.
- [ ] `RTHOST-P4-004` For every runtime-cloud route/state row, replace direct runtime execution ownership with runtime-cloud projection contracts.
- [ ] `RTHOST-P4-005` For every duplicated DTO/schema/auth/write-check row, identify the canonical owner before deleting duplicates.
- [ ] `RTHOST-P4-006` Keep browser assets and websocket details in web; any browser-visible row requires Playwright evidence in the implementation branch.
- [ ] `RTHOST-P4-007` Decide exact action for `crates/trust-runtime/src/web/hmi_ws.rs`: keep as websocket adapter only or split domain logic out.
- [ ] `RTHOST-P4-008` Decide exact action for `crates/trust-runtime/src/control/hmi_handlers*.rs`: keep HTTP-neutral control handlers or move HMI-domain pieces to `hmi`.
- [ ] `RTHOST-P4-009` Decide exact action for `crates/trust-runtime/src/runtime_cloud/` files and `crates/trust-runtime/src/web/runtime_cloud_*` route/state files.
- [ ] `RTHOST-P4-010` Add the reviewed named-file move table to this checklist before code movement.

## Phase 5 - Tests

- [ ] `RTHOST-P5-001` Contract tests for HMI schema/descriptor projection.
- [ ] `RTHOST-P5-002` Contract tests for HMI write authorization policy.
- [ ] `RTHOST-P5-003` Contract tests for runtime snapshot/status projection.
- [ ] `RTHOST-P5-004` Contract tests for runtime-cloud projection.
- [ ] `RTHOST-P5-005` Route tests prove web remains a thin adapter.
- [ ] `RTHOST-P5-006` Browser-visible changes use Playwright verification in implementation branches.

## Exit Criteria

- [ ] `RTHOST-EXIT-01` HMI logic is not split three ways without ownership rules.
- [ ] `RTHOST-EXIT-02` `control -> web` inversion is removed or explicitly justified with a removal ticket.
- [ ] `RTHOST-EXIT-03` Runtime-cloud does not own runtime execution.
- [ ] `RTHOST-EXIT-04` Web route code is transport adapter code, not domain owner.
- [ ] `RTHOST-EXIT-05` Doctor rules prevent drift back.
