# HMI Operator-First V4 Implementation Checklist

Primary specification:
- `docs/guides/HMI_OPERATOR_FIRST_SPECIFICATION.md`

Execution intent:
- Turn the V4 specification into a testable, traceable delivery plan.
- Every checklist item must reference a concrete spec section.

Status legend:
- `Not Started` | `In Progress` | `Blocked` | `Done`

Current status: `Done` (implementation complete; validation gates recorded in repo checklist evidence).

## 0. Traceability Rules

- [x] Every implementation PR item includes at least one spec reference (for example `Spec §5.2`).
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §1`
- [x] Every acceptance claim is backed by a test, screenshot, or command output artifact.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §19`
- [x] No feature is marked complete unless both behavior and UX criteria pass.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §19`

## 1. First-Run Promise and Lifecycle Modes

### 1.1 First-run readiness
- [x] Runtime auto-scaffolds HMI when `hmi/` is missing (or emits one-command guidance when disabled by policy).
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §1`, `§3`
- [x] Startup logs print explicit ready line with URL and scaffolded page count.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §1`, `§19`
- [x] First browser load shows live values (not static labels-only placeholders).
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §19`

### 1.2 Lifecycle mode semantics
- [x] `init` mode implemented and validated (`fail if exists` unless force).
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §3`
- [x] `update` mode merges missing pages/signals while preserving user edits.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §3`
- [x] `reset` mode implemented with backup snapshot behavior.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §3`
- [x] Incomplete `hmi/` directory triggers update-fill behavior, not silent degradation.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §3`

## 2. Input Scope and Fallback Rules

- [x] External-interface include/exclude rules are enforced in scaffold path.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §4`
- [x] Local-only projects (no explicit external vars) scaffold via inferred interface path.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §4`
- [x] Inferred interface points are marked in metadata for user review.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §4`

## 3. Auto-Generation of All 5 Required Pages

### 3.1 Overview generation
- [x] Overview auto-generates with budget 8-12 (default 10).
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §5.1`
- [x] Overview never renders full variable inventory by default.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §5.1`

### 3.2 Process generation
- [x] `custom-svg` mode works when descriptor references SVG.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §5.2`
- [x] `auto-schematic` fallback generates deterministic SVG when no project SVG exists.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §5.2`
- [x] Auto-schematic uses left-to-right lane layout and stable IDs.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §5.2`
- [x] Connection inference produces at least one main flow path; unknown edges are visually distinguished.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §5.2`
- [x] Symbol mapping prefers bundled industrial symbols and falls back gracefully.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §5.2`
- [x] Auto-schematic grid/anchor contract enforced (shared FIT/PT template footprint, value offset one grid row above sensor centerline, stem endpoint on process line, pipe endpoints on connectors).
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §5.2.1`
- [x] Auto-schematic tank percent fill updates both `y` and `height` attributes for consistent visual scaling.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §5.2.1`
- [x] Dropping custom SVG promotes page from auto-schematic to custom mode without losing useful mappings.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §5.2`

### 3.3 Control generation
- [x] Control page auto-generated from writable points only.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §5.3`
- [x] Commands/setpoints/modes/text are grouped into dedicated sections.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §5.3`
- [x] Command-class writes have confirmation UX by default.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §5.3`
- [x] Matching PV values are shown next to setpoint controls where available.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §5.3`

### 3.4 Trends generation
- [x] Trends page defaults to curated 6-8 signals.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §5.4`
- [x] Default duration is 10 minutes with 1m/10m/1h presets.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §5.4`
- [x] Signal selection avoids static config-like values by default.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §5.4`, `§6`

### 3.5 Alarm generation
- [x] Explicit alarm rules override inferred rules.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §5.5`
- [x] Range-based starter thresholds are inferred when no explicit rules exist.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §5.5`
- [x] Default inferred deadband/hysteresis is applied (`2% span`).
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §5.5`
- [x] Inferred alarms are clearly marked for user review.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §5.5`

## 4. Deterministic Scoring and Hierarchy

- [x] Overview scoring weights implemented exactly (100/80/60/45/35/20).
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §6`
- [x] Slot caps implemented and tested.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §6`
- [x] Tie-breaker order implemented and deterministic.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §6`
- [x] Overflow routing to non-Overview pages enforced.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §6`
- [x] Auto-layout uses `widget_span` hierarchy (non-flat layout).
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §7`
- [x] Reference wireframe semantics are reflected in rendered Overview structure.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §7`

## 5. In-Browser Curation Mode (Critical)

### 5.1 Widget-level edits
- [x] Remove from page (without deleting signal).
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §8.1`
- [x] Move between pages.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §8.1`
- [x] Change widget type with compatibility checks.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §8.1`
- [x] Inline label edit.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §8.1`
- [x] Resize span presets.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §8.1`
- [x] Pin/unpin on Overview.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §8.1`

### 5.2 Page/system edits
- [x] Reorder sections.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §8.2`
- [x] Rename section titles.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §8.2`
- [x] Add from available/unplaced signal pool.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §8.2`, `§8.3`
- [x] Reset to scaffold defaults action available.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §8.3`

### 5.3 Persistence and refresh
- [x] Curation changes save back to `hmi/*.toml` (descriptor remains source of truth).
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §8.4`
- [x] Typed descriptor update API available (`hmi.descriptor.update` or equivalent).
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §8.4`
- [x] Save triggers immediate live-refresh.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §8.4`

## 6. Cross-Page Drill-Down

### P0 (must ship first)
- [x] Alarm row click -> Process page focus/highlight.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §9`
- [x] Overview KPI click -> Trends page focused signal.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §9`

### P1
- [x] Setpoint click -> Control page target card.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §9`
- [x] Context-preserving back navigation.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §9`

## 7. Mode UX and Settings Scope

- [x] Operator mode hides internal names/types by default.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §10`
- [x] Engineering mode toggle present in topbar.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §10`
- [x] Keyboard shortcut for mode toggle works.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §10`
- [x] URL-param mode override works.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §10`
- [x] Mode preference persists in browser storage.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §10`
- [x] Phase-0 settings only (no overscoped admin workflows in initial 5-minute path).
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §11`

## 8. Motion, Quality, and Data Freshness

- [x] Default animation timings match spec values.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §12`
- [x] Per-widget quality state (`good|stale|bad`) implemented and visible.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §12`
- [x] Stale/disconnected thresholds implemented and tested.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §12`

## 9. Transport and Reliability

- [x] WebSocket primary transport implemented for live values.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §13`
- [x] Polling fallback implemented and verified.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §13`
- [x] Reconnect backoff behavior implemented and tested.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §13`

## 10. Developer Feedback Loop

- [x] Descriptor edits reflect in UI within target latency.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §13`, `§14`
- [x] Malformed TOML/bind errors are visible in browser and editor diagnostics.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §14`
- [x] VS Code preview workflow is first-class and documented.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §14`
- [x] Engineering diagnostics overlay for binding health available.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §10`, `§14`

## 11. Responsive, Export, Theme, and Migration

- [x] Tablet and kiosk render contracts verified.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §15`
- [x] Standalone/export bundle includes required assets/schema/bootstrap and validates offline.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §16`
- [x] Theme semantic mapping is consistent across light/dark profiles.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §17`
- [x] Dark mode is polished (not simple inversion).
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §17`
- [x] Descriptor version marker + migration path implemented.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §18`

## 12. Performance and 5-Minute Success Gates

- [x] Scaffold generation target: < 2s.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §19`
- [x] Runtime startup to HMI-ready target: < 5s.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §19`
- [x] Browser first load target: < 2s localhost.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §19`
- [x] Value update latency p95 target: < 100ms local.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §13`, `§19`
- [x] 5-minute acceptance criteria all pass.
  - Ref: `HMI_OPERATOR_FIRST_SPECIFICATION.md §19`

## 13. Evidence Commands (Minimum)

- [x] `cargo test -p trust-runtime --test hmi_readonly_integration`
- [x] `cargo test -p trust-runtime --test web_ide_integration`
- [x] `cargo test -p trust-runtime --test web_io_config_integration`
- [x] `cargo test -p trust-runtime --test web_tls_integration`
- [x] `cd editors/vscode && npm run lint`
- [x] `cd editors/vscode && npm run compile`
- [x] `cd editors/vscode && npm test`

## 14. Release Readiness Note

If this checklist drives user-visible behavior changes, release hygiene artifacts must be updated before merge (changelog/version/synced extension versions per project rules).

- [x] `CHANGELOG.md` updated for operator-first generation and curation behavior
- [x] workspace/extension version sync handled if release-notable changes are shipped
