# Web IDE Collaboration Model (Phase 1)

Scope date: 2026-02-15

Purpose:

- Define the collaboration/presence contract for browser IDE rollout.

## Phase 1 Decision

Live multi-user cursor/presence is explicitly out of scope for first production release.

Rationale:

- Prioritize single-user reliability for diagnostics/hover/completion and conflict-safe edits.
- Keep authoring safety boundaries clear while API/session hardening matures.

## Current Contract

- Concurrent write conflicts are enforced with optimistic version checks (`expected_version`).
- Session tokens gate IDE operations (`X-Trust-Ide-Session`).
- Presence model endpoint advertises out-of-scope status:
  - `GET /api/ide/presence-model`
  - response mode: `out_of_scope_phase_1`

## Phase 2 Target (Planned)

- Optional live presence channels:
  - active user list
  - per-file active editor list
  - cursor/range snapshots (coarse, throttled)
- Non-blocking hints only (presence does not alter write authority).
- No write bypass of existing optimistic concurrency model.

## Acceptance Boundary

A Phase 1 implementation is complete when:

- single-user authoring flow is stable,
- draft/autosave/reconnect behavior is reliable,
- conflict semantics are deterministic,
- and presence status is documented as deferred.
