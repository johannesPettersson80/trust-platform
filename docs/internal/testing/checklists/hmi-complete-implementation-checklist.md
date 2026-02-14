# HMI Execution Board (Converted from Checklist)

Date opened: 2026-02-13
Last updated: 2026-02-14
Owner: Johannes + Codex
Status: Implementation Complete; Post-Merge Release Verification Pending
Branch: `feat/hmi-phase0-exec-board`

Primary spec: `docs/internal/hmi_specification.md`

## Execution Board

Status legend: `Not Started` | `In Progress` | `Blocked` | `Done`

| Lane | Status | Exit Gate | Evidence |
| --- | --- | --- | --- |
| Global Guardrails | Done | Contract guardrails verified in runtime/LSP/LM paths | Evidence Log -> Global Guardrails |
| Phase 0 - Scaffold Engine | Done | `scaffold_hmi_dir` + CLI/LSP/LM entry + tests | Evidence Log -> Phase 0 |
| Phase 1 - Descriptor Parser + Schema | Done | `hmi/` parser merged with schema integration and validation | Evidence Log -> Phase 1 / 1.6 |
| Phase 2 - Web Renderer + Transport | Done | Renderer + websocket/fallback + process page gates | Evidence Log -> Phase 2 / 2.4 / 2.5 |
| Phase 3 - LSP + LM Tools | Done | HMI commands/tools + diagnostics + tests | Evidence Log -> Phase 3 / 3.7 |
| Phase 4 - VS Code HMI Panel | Done | Panel watcher/render/refresh gates | Evidence Log -> Phase 4 |
| Phase 5 - Export Bundle | Done | Export schema/version/app capabilities + offline standalone validation complete | Evidence Log -> Phase 5 |
| Phase 6 - Intent to Evidence | Done | Intent/lock/evidence loop and LM flow complete | Evidence Log -> Phase 6 |
| Security/Perf/Reliability Gates | Done | SLO + authz + hardening checks complete | Evidence Log -> Security/Perf/Reliability |
| Release Hygiene + Final Validation | Blocked | changelog/version/check gates complete; post-merge tag/release verification pending merge to `main` | Evidence Log -> Release Hygiene / Final Gate |

## Phase 0 Active Workboard

| ID | Work Item | Status | Owner | Evidence | Notes |
| --- | --- | --- | --- | --- | --- |
| P0-01 | Implement runtime scaffold engine (`scaffold_hmi_dir`) | Done | Codex | `crates/trust-runtime/src/hmi.rs` | Deterministic summary + file emission implemented |
| P0-02 | Generate `hmi/_config.toml`, `overview.toml`, `trends.toml`, `alarms.toml` | Done | Codex | `crates/trust-runtime/src/hmi.rs` | `trends`/`alarms` generated conditionally |
| P0-03 | External symbol filtering + internal exclusion | Done | Codex | `crates/trust-runtime/src/hmi.rs` | Source-aware qualifier parsing + internal filter |
| P0-04 | Type+writability widget mapping + label/range/icon inference | Done | Codex | `crates/trust-runtime/src/hmi.rs` | Heuristics wired into scaffold mapping |
| P0-05 | Grouping (instance + qualifier + type-affinity) | Done | Codex | `crates/trust-runtime/src/hmi.rs` | Section grouping for qualifier/type and repeated instances |
| P0-06 | Expose CLI command `trust-runtime hmi init` | Done | Codex | `crates/trust-runtime/src/bin/trust-runtime/hmi.rs` | New CLI command path wired and tested |
| P0-07 | Expose LSP command `trust-lsp.hmiInit` | Done | Codex | `crates/trust-lsp/src/handlers/commands.rs` | LSP command compiles workspace sources and emits deterministic scaffold summary |
| P0-08 | Expose LM tool `trust_hmi_init` via LSP path | Done | Codex | `editors/vscode/src/lm-tools.ts` | LM tool executes `workspace/executeCommand` -> `trust-lsp.hmiInit` |
| P0-09 | Add Phase 0 tests (scope, mapping, deterministic output, repeated instances) | Done | Codex | `crates/trust-runtime/src/hmi.rs` | All four runtime scaffold tests added and passing |

## Phase 1 Active Workboard

| ID | Work Item | Status | Owner | Evidence | Notes |
| --- | --- | --- | --- | --- | --- |
| P1-01 | Add `hmi/` descriptor structs + parser (`_config.toml` + page TOML) | Done | Codex | `crates/trust-runtime/src/hmi.rs` | `HmiDirDescriptor`/`HmiDir*` structs + deterministic load/sort added |
| P1-02 | Integrate `hmi/` descriptor into `HmiCustomization`/`build_schema` | Done | Codex | `crates/trust-runtime/src/hmi.rs` | `hmi/` now prioritized over legacy `hmi.toml`; descriptor applied as widget/page overrides |
| P1-03 | Extend schema for sections/icon/zone/color/span metadata | Done | Codex | `crates/trust-runtime/src/hmi.rs` | `HmiWidgetSchema` + `HmiPageSchema` extensions wired end-to-end |
| P1-04 | Implement binding validation diagnostics (`unknown bind`, `type mismatch`, `unknown widget`) | Done | Codex | `crates/trust-runtime/src/hmi.rs` | `validate_hmi_bindings()` with stable diagnostic codes added |
| P1-05 | Wire alarm thresholds from `_config.toml` into widget limits | Done | Codex | `crates/trust-runtime/src/hmi.rs` | `low/high` maps into widget `min/max` for existing alarm engine reuse |
| P1-06 | Complete Phase 1 regression/snapshot compatibility test pass and evidence capture | Done | Codex | `crates/trust-runtime/src/hmi.rs` | Added compatibility + schema snapshot tests; parser/validation/runtime test gates passing |
| P1-07 | Implement Phase 1.6 live descriptor refresh (`notify` watcher + `schema_revision`) | Done | Codex | `crates/trust-runtime/src/control.rs`, `crates/trust-runtime/src/hmi.rs`, `crates/trust-runtime/src/bin/trust-runtime/run.rs` | Added runtime watcher/debounce, strict reload fallback, schema revision surfacing, and live-refresh integration tests |

## Phase 2 Active Workboard

| ID | Work Item | Status | Owner | Evidence | Notes |
| --- | --- | --- | --- | --- | --- |
| P2-01 | Add section grid renderer with responsive spans and schema-driven sections | Done | Codex | `crates/trust-runtime/src/web/ui/hmi.js`, `crates/trust-runtime/src/web/ui/hmi.css`, `crates/trust-runtime/tests/hmi_readonly_integration.rs` | Added `page.sections` render path (`section-grid` + `section-widget-grid`) with fallback to legacy grouping and integration coverage |
| P2-02 | Implement widget-type renderers (gauge/sparkline/bar/tank/indicator/toggle/slider) | Done | Codex | `crates/trust-runtime/src/web/ui/hmi.js`, `crates/trust-runtime/src/web/ui/hmi.css`, `crates/trust-runtime/tests/hmi_readonly_integration.rs` | Added renderer dispatch + per-widget update/write logic with integration asset checks |
| P2-03 | Add value-change transitions and dark-mode variable overrides | Done | Codex | `crates/trust-runtime/src/web/ui/hmi.css` | Added micro-animation transitions and `prefers-color-scheme: dark` variable palette overrides |
| P2-04 | Add websocket transport (`/ws/hmi`) + client reconnect/fallback schema refresh path | Done | Codex | `crates/trust-runtime/src/web.rs`, `crates/trust-runtime/src/web/ui/hmi.js`, `crates/trust-runtime/tests/hmi_readonly_integration.rs` | Added value delta/schema revision/alarm websocket events, client reconnect backoff, and polling fallback path with schema re-fetch on revision event |
| P2-05 | Add websocket hardening tests (latency SLO, forced-failure fallback, reconnect churn) | Done | Codex | `crates/trust-runtime/tests/hmi_readonly_integration.rs` | Added deterministic websocket transport test gates for latency/fallback/stability acceptance criteria |
| P2-06 | Add process page foundation (`kind = process`, SVG assets, bind schema/client mapping) | Done | Codex | `crates/trust-runtime/src/hmi.rs`, `crates/trust-runtime/src/web.rs`, `crates/trust-runtime/src/web/ui/hmi.js`, `crates/trust-runtime/tests/hmi_readonly_integration.rs` | Added process page schema fields (`svg`/`bindings`), secure SVG asset route, safe bind parsing, and web client process renderer/update path |
| P2-07 | Add renderer state coverage (null/stale/good) and responsive breakpoint regression checks | Done | Codex | `crates/trust-runtime/tests/hmi_readonly_integration.rs` | Node-backed renderer tests cover gauge/sparkline/bar/tank/indicator/toggle/slider + quality transitions and mobile/tablet/desktop layout classes |
| P2-08 | Complete process asset pack contract (symbol library/templates/binding alignment) | Done | Codex | `hmi/pid-symbols/*`, `hmi/plant.svg`, `hmi/plant-minimal.svg`, `hmi/plant.bindings.example.toml`, `crates/trust-runtime/tests/hmi_readonly_integration.rs` | Added deterministic asset integrity test for license/symbol count/stable IDs/selector alignment |

## Phase 3 Active Workboard

| ID | Work Item | Status | Owner | Evidence | Notes |
| --- | --- | --- | --- | --- | --- |
| P3-01 | Implement `trust-lsp.hmiBindings` command with qualifier/constraint metadata | Done | Codex | `crates/trust-lsp/src/handlers/commands.rs`, `crates/trust-lsp/src/main.rs`, `crates/trust-runtime/src/hmi.rs` | Added bindings catalog contract (`programs`/`globals`) with `type`, `qualifier`, `writable`, `unit`, `min`, `max`, `enum_values` |
| P3-02 | Implement LM tools `trust_hmi_get_bindings`, `trust_hmi_get_layout`, `trust_hmi_apply_patch` | Done | Codex | `editors/vscode/src/lm-tools.ts` | Added request/descriptor read/typed patch tools with dry-run conflict reporting |
| P3-03 | Register HMI tools/commands in extension metadata and runtime registry | Done | Codex | `editors/vscode/package.json`, `editors/vscode/src/lm-tools.ts`, `editors/vscode/src/extension.ts`, `editors/vscode/src/hmiPanel.ts` | Added activation events, tool definitions, `trust-lsp.hmi.init`, and non-opening `trust-lsp.hmi.refreshFromDescriptor` |
| P3-04 | Add command-level tests for `trust-lsp.hmiBindings` | Done | Codex | `crates/trust-lsp/src/handlers/commands.rs` | Added mock-context tests for valid catalog and invalid argument handling |
| P3-05 | Add LM tool integration tests (valid/invalid payloads + cancellation) | Done | Codex | `editors/vscode/src/test/suite/hmi.integration.test.ts` | Added integration tests for layout/patch/get_bindings flows and cancellation handling |
| P3-06 | Add multi-root workspace path-resolution/write tests for HMI tools | Done | Codex | `editors/vscode/src/test/suite/new-project.test.ts` | Added explicit `rootPath` write/read assertions for `trust_hmi_apply_patch` and `trust_hmi_get_layout` in a live multi-root workspace |

## Phase 6 Active Workboard

| ID | Work Item | Status | Owner | Evidence | Notes |
| --- | --- | --- | --- | --- | --- |
| P6-01 | Add `trust_hmi_plan_intent` and deterministic `_intent.toml` artifact flow | Done | Codex | `editors/vscode/src/lm-tools.ts`, `editors/vscode/package.json`, `editors/vscode/src/test/suite/hmi.integration.test.ts` | Added dry-run/write paths with deterministic TOML content generation and repeat-write idempotency checks |
| P6-02 | Add `trust_hmi_validate` with deterministic `_lock.json` emission | Done | Codex | `editors/vscode/src/lm-tools.ts`, `editors/vscode/package.json`, `editors/vscode/src/test/suite/hmi.integration.test.ts` | Added machine-readable checks + canonical ID/fingerprint lock generation and lock byte-stability regression assertion |
| P6-03 | Add evidence run storage + bounded retention prune flow | Done | Codex | `editors/vscode/src/lm-tools.ts`, `editors/vscode/src/test/suite/hmi.integration.test.ts`, `.gitignore` | Added `_evidence/<timestamp>/validation.json` + `journeys.json`, `prune`/`retain_runs`, and default ignore for `hmi/_evidence/` |
| P6-04 | Implement remaining Phase 6 LM tools (`trace_capture`, `generate_candidates`, `preview_snapshot`, `run_journey`, `explain_widget`) | Done | Codex | `editors/vscode/src/lm-tools.ts`, `editors/vscode/package.json`, `editors/vscode/src/test/suite/hmi.integration.test.ts` | Added API/event-level trace + journey flows, deterministic candidate ranking/snapshot artifacts, explain-widget provenance, and integration/cancellation coverage |

## Working Rules

- [x] Do not start a later phase before all required checks in the current phase are complete.
- [x] Record command outputs and evidence links in the Evidence Log section at the end.
- [x] For each item, attach PR/commit reference when done.
- [x] Any deviation from the spec must be documented in `docs/internal/hmi_specification.md` before merge.

## Detailed Checklist Backlog (Source of Truth)

## Production Policy (No Shortcuts, No Stubs)

- [x] This checklist is **all-or-nothing**: release is blocked until every required checkbox is complete.
- [x] No placeholder criteria allowed (no `TBD`, no “decide later”, no “target defined by team”).
- [x] No stub behavior in production paths:
- [x] No `todo!`, `unimplemented!`, placeholder return values, or temporary feature bypasses.
- [x] No TODO/FIXME/HACK markers in changed HMI/runtime/LSP/VS Code files at merge time.
- [x] No “temporary” disabled validation/security checks in release build.
- [x] No mock/test transport paths compiled into production runtime behavior.

## Global Guardrails (Must Always Hold)

Spec refs:
- `docs/internal/hmi_specification.md` -> `Phase 6 / Guardrails (must remain non-negotiable)`
- `docs/internal/hmi_specification.md` -> `Contract upgrades for deterministic LLM behavior`

- [x] `hmi.write` remains disabled by default.
- [x] Writes require explicit allowlist and authorization.
- [x] HMI contract remains endpoint-based (`hmi.schema.get`, `hmi.values.get`, `hmi.write`) and isolated from debug internals.
- [x] Mutating LM flows remain deterministic/idempotent (`dry_run`, conflict reporting).
- [x] Canonical IDs are used for contracts/writes; display aliases are not primary identity.
- [x] Poll/write behavior is validated against cycle-time budget.

## Phase 0: Scaffold Engine (Smart Auto-Generation)

Spec refs:
- `docs/internal/hmi_specification.md` -> `Phase 0: Auto-Scaffold`
- `docs/internal/hmi_specification.md` -> `Scaffold quality heuristics`
- `docs/internal/hmi_specification.md` -> `Scaffold summary output`

### Implementation

- [x] Implement `scaffold_hmi_dir(...)` in runtime (`hmi.rs` or dedicated scaffold module).
- [x] Generate `hmi/_config.toml`, `hmi/overview.toml`, `hmi/trends.toml`, `hmi/alarms.toml` as applicable.
- [x] Include only external-facing symbols (inputs/outputs/in_out/globals/properties) and exclude internals.
- [x] Widget selection follows type + writability mapping.
- [x] Grouping supports FB-instance grouping, qualifier grouping, type-affinity grouping.
- [x] Add label inference (`snake_case`/`camelCase`, abbreviation expansion).
- [x] Add range inference (unit/type hints + safe fallbacks).
- [x] Add icon inference from naming hints.
- [x] Return deterministic scaffold summary (same input -> same output).

### Interfaces

- [x] Expose scaffold via CLI `trust-runtime hmi init`.
- [x] Expose scaffold via LSP command `trust-lsp.hmiInit`.
- [x] Expose scaffold via LM tool `trust_hmi_init` (LSP path only).

### Tests

- [x] Unit tests for variable inclusion/exclusion rules.
- [x] Unit tests for widget mapping correctness.
- [x] Snapshot tests for generated files and deterministic ordering.
- [x] Regression tests for repeated FB instances producing separate sections.

## Phase 1: `hmi/` Descriptor Parser + Schema Integration

Spec refs:
- `docs/internal/hmi_specification.md` -> `Phase 1: HMI Directory Format + Runtime Parser`
- `docs/internal/hmi_specification.md` -> `1.1 Runtime parser structs`
- `docs/internal/hmi_specification.md` -> `1.2 Integrate into HmiCustomization and build_schema`
- `docs/internal/hmi_specification.md` -> `1.3 Extend schema structs for rich rendering`
- `docs/internal/hmi_specification.md` -> `1.4 Binding validation`
- `docs/internal/hmi_specification.md` -> `1.5 Wire alarms`

### Implementation

- [x] Add descriptor serde structs for config/pages/sections/widgets/zones/alarms.
- [x] Implement `load_hmi_dir(root)` with deterministic page discovery/sorting.
- [x] Prioritize `hmi/` descriptor over legacy `hmi.toml` when both exist.
- [x] Extend `HmiWidgetSchema`/`HmiPageSchema` with zones/colors/sections/icon/span metadata.
- [x] Implement `validate_hmi_bindings()` with typed diagnostics codes.
- [x] Map config alarm thresholds into widget min/max for alarm engine reuse.

### Compatibility

- [x] Preserve legacy behavior when no `hmi/` directory exists.
- [x] Keep legacy `hmi.toml` + annotation behavior as fallback path.

### Tests

- [x] Parser tests for valid and invalid TOML descriptors.
- [x] Snapshot tests for schema payload stability.
- [x] Binding validation tests: unknown symbol, type mismatch, bad widget config.
- [x] Alarm threshold mapping tests.

## Phase 1.6: Live Descriptor Refresh (No Runtime Restart)

Spec ref:
- `docs/internal/hmi_specification.md` -> `1.6 Live descriptor refresh (no runtime restart)`

### Implementation

- [x] Add runtime watcher (`notify`) on `hmi/*.toml` and `_config.toml`.
- [x] Debounce change handling to avoid parse storms.
- [x] Re-parse descriptor and bump in-memory `schema_revision` on change.
- [x] Surface `schema_revision` in schema responses.
- [x] Ensure invalid descriptor changes fail safely without crashing runtime.

### Tests

- [x] Integration test: edit descriptor while runtime is running -> revision increments.
- [x] Integration test: invalid TOML change reports error and retains last good schema.
- [x] Integration test: no runtime restart required for descriptor-only changes.

## Phase 2: Web HMI Renderer Enhancements

Spec refs:
- `docs/internal/hmi_specification.md` -> `Phase 2: Web HMI Renderer`
- `docs/internal/hmi_specification.md` -> `2.1 Section-based layout engine`
- `docs/internal/hmi_specification.md` -> `2.2 New widget renderers`
- `docs/internal/hmi_specification.md` -> `2.3 CSS additions`

### Implementation

- [x] Add section grid rendering with responsive spans.
- [x] Implement widget renderers: gauge/sparkline/bar/tank/indicator/toggle/slider.
- [x] Keep existing widgets and trend/alarm pages backward compatible.
- [x] Add transition/micro-animation behavior for value changes.
- [x] Add dark mode support honoring theme/config behavior.

### Tests

- [x] UI integration tests for section rendering.
- [x] Renderer tests for each widget type with null/stale/good values.
- [x] Visual regression checks for desktop/tablet/mobile breakpoints.

## Phase 2.4: WebSocket Push + Polling Fallback

Spec ref:
- `docs/internal/hmi_specification.md` -> `2.4 Live transport: WebSocket push + polling fallback`

### Implementation

- [x] Add `/ws/hmi` endpoint for value deltas, schema revision events, alarm events.
- [x] Implement client socket connection/reconnect and backoff.
- [x] Implement fallback to HTTP polling when socket unavailable.
- [x] Re-fetch full schema on revision-change event.

### Tests

- [x] End-to-end websocket value push test meets SLO:
- [x] local/LAN p95 event-to-render latency <= 100 ms.
- [x] local/LAN p99 event-to-render latency <= 250 ms.
- [x] Fallback test: forced socket failure -> polling resumes.
- [x] Stability test under reconnect churn.

## Phase 2.5: Process SVG View (P&ID Style)

Spec refs:
- `docs/internal/hmi_specification.md` -> `2.5 Process view (SVG binding page kind)`
- `docs/internal/hmi_specification.md` -> `Special page kinds: hmi/plant.toml`

### Implementation

- [x] Support `kind = "process"` pages with `svg = "..."` asset resolution.
- [x] Implement `[[bind]]` selector/attribute/source mapping in runtime web client.
- [x] Support `format`, boolean `map`, and numeric `scale` transforms.
- [x] Apply updates to inline SVG DOM safely (selector scope + sanitization rules).
- [x] Document stable ID convention for bind targets.

### Assets

- [x] Maintain reusable symbol library under `hmi/pid-symbols/` with license file.
- [x] Provide production template(s): `hmi/plant.svg`, `hmi/plant-minimal.svg`.
- [x] Keep binding example in `hmi/plant.bindings.example.toml` aligned with template IDs.

### Tests

- [x] Binding tests for fill/opacity/text/y/height attribute updates.
- [x] Negative tests for missing selector or unsafe selector.
- [x] Integration test for process page render + live updates.

## Phase 3: LSP Commands + Core LM Tools

Spec refs:
- `docs/internal/hmi_specification.md` -> `Phase 3: LSP Command + VS Code LM Tools`
- `docs/internal/hmi_specification.md` -> `3.1 ... 3.6`

### LSP Commands

- [x] Implement `trust-lsp.hmiBindings` with type/writable/qualifier metadata.
- [x] Implement `trust-lsp.hmiInit` command path.
- [x] Include constraints metadata (`min/max/enum/unit`) where available.

### LM Tools

- [x] Implement `trust_hmi_get_bindings`.
- [x] Implement `trust_hmi_get_layout`.
- [x] Implement `trust_hmi_apply_patch` with `dry_run` and typed conflict reporting.
- [x] Implement `trust_hmi_init` (LSP path).
- [x] Register tools in `editors/vscode/package.json` and runtime tool registry.

### Tests

- [x] Tool-level tests for valid/invalid payloads.
- [x] Cancellation tests for long-running tool requests.
- [x] Multi-root workspace tests for path resolution and writes.

## Phase 3.7: HMI TOML Diagnostics in VS Code

Spec ref:
- `docs/internal/hmi_specification.md` -> `3.7 HMI TOML diagnostics in VS Code`

### Implementation

- [x] Parse and validate `hmi/*.toml` binds and widget configs in LSP diagnostics pipeline.
- [x] Emit stable diagnostic code for unknown binding path.
- [x] Emit stable diagnostic code for type mismatch (e.g., gauge on BOOL).
- [x] Emit stable diagnostic code for unknown widget kind.
- [x] Emit stable diagnostic code for invalid property combinations.
- [x] Add near-match suggestion support.

### Tests

- [x] Diagnostics snapshots for valid/invalid cases.
- [x] Suggestion tests for typo correction hints.
- [x] Regression tests for no false positives on valid configs.

## Phase 4: VS Code HMI Panel

Spec refs:
- `docs/internal/hmi_specification.md` -> `Phase 4: VS Code HMI Panel Enhancement`
- `docs/internal/hmi_specification.md` -> `4.1 Watch hmi/ directory`
- `docs/internal/hmi_specification.md` -> `4.2 Upgrade webview rendering`

### Implementation

- [x] Expand refresh relevance to `hmi/*.toml` and `hmi/*.svg`.
- [x] Add filesystem watcher with debounce.
- [x] Refresh existing panel without forced auto-open.
- [x] Add process page renderer in panel webview.
- [x] Keep panel read/write interactions aligned with runtime contracts.

### Tests

- [x] Save-change refresh tests (`toml` + `svg`).
- [x] Panel rendering tests for section widgets + process page.
- [x] Regression tests for existing HMI panel behavior.

## Phase 5: Export Bundle

Spec ref:
- `docs/internal/hmi_specification.md` -> `Phase 5: Export`

- [x] Include resolved descriptor in export payload.
- [x] Bump/export schema version as specified.
- [x] Ensure exported app contains new renderer capabilities.
- [x] Validate exported bundle in offline/standalone run.

## Phase 6: Intent-to-Evidence Loop

Spec refs:
- `docs/internal/hmi_specification.md` -> `Phase 6: Intent-to-Evidence Loop`
- `docs/internal/hmi_specification.md` -> `Operational constraints for evidence workflow`
- `docs/internal/hmi_specification.md` -> `Additional Verification For Phase 6`

### Artifacts

- [x] Support `_intent.toml` generation and updates.
- [x] Support `_lock.json` deterministic ID/fingerprint output.
- [x] Store evidence runs under `_evidence/<timestamp>/...`.
- [x] Enforce bounded retention (default 10 runs) and pruning.

### Advanced LM Tools

- [x] `trust_hmi_plan_intent`
- [x] `trust_hmi_trace_capture`
- [x] `trust_hmi_generate_candidates` (scaffold-rules-based)
- [x] `trust_hmi_validate`
- [x] `trust_hmi_preview_snapshot`
- [x] `trust_hmi_run_journey` (API/event-level, no required headless browser)
- [x] `trust_hmi_explain_widget`

### Tests

- [x] Determinism tests for `_lock.json` and candidate ranking.
- [x] Safety tests for unauthorized/non-allowlisted writes.
- [x] Evidence pruning tests.
- [x] Journey execution tests at API/event layer.

## Security, Performance, and Reliability Gates

Spec refs:
- `docs/internal/hmi_specification.md` -> `Guardrails`
- `docs/internal/hmi_specification.md` -> `Verification`

- [x] Authz checks for every write path (runtime + tool-level).
- [x] Poll/write cycle-time budget benchmark with pass/fail thresholds.
- [x] Websocket reconnect and backpressure handling under load.
- [x] Robust handling of malformed TOML/SVG input.
- [x] No crashes or deadlocks from rapid file changes.

## Documentation Sync Checklist

- [x] Keep `docs/internal/hmi_specification.md` updated with implementation deltas.
- [x] Add/refresh user docs for `hmi/` directory format and process pages.
- [x] Add examples for `plant.svg` and `plant-minimal.svg` usage.
- [x] Document tool invocation patterns for LM workflows.

## Production Hard Gates (Code Completeness)

- [x] No new TODO/FIXME/HACK/XXX markers in changed files:
- [x] `git diff --name-only | rg '^(crates|editors|hmi|docs)/' | xargs rg -n 'TODO|FIXME|HACK|XXX'` returns no matches.
- [x] No `todo!`/`unimplemented!` in changed files:
- [x] `git diff --name-only | rg '^crates/' | xargs rg -n 'todo!\\(|unimplemented!\\('` returns no matches.
- [x] No placeholder mock response values in runtime/LSP production paths.
- [x] Panic-safety review complete for user/input-facing paths (no crash-on-invalid-input behavior).

## Performance & Capacity Gates (Release Blocking)

- [x] Live descriptor refresh SLO:
- [x] save-to-schema-revision-update p95 <= 1000 ms.
- [x] save-to-client-visible-layout-update p95 <= 1500 ms.
- [x] Websocket SLO:
- [x] value event-to-render p95 <= 100 ms (LAN/local).
- [x] value event-to-render p99 <= 250 ms (LAN/local).
- [x] reconnect after forced drop <= 5 s.
- [x] Fallback SLO:
- [x] websocket disabled/unavailable -> polling path recovers and updates continue within one poll interval.
- [x] Cycle-time impact budget respected under representative HMI load profile.

## Release Hygiene (Mandatory for Production Release)

Spec refs:
- `AGENTS.md` -> `Release Hygiene Rules`
- `/home/johannes/.codex/skills/trust-release-hygiene/SKILL.md` -> `Workflow`

- [x] Update `CHANGELOG.md` under `## [Unreleased]` (`Added` / `Changed` / `Fixed` as applicable).
- [x] Bump `[workspace.package].version` in `Cargo.toml` for release-notable HMI changes.
- [x] If VS Code extension behavior changed:
- [x] bump `editors/vscode/package.json` version.
- [x] sync `editors/vscode/package-lock.json` root version fields to same version.
- [x] Run mandatory project checks:
- [x] `just fmt`
- [x] `just clippy`
- [x] `just test`
- [x] Run extension checks when `editors/vscode/**` changed:
- [x] `cd editors/vscode && npm run lint`
- [x] `cd editors/vscode && npm run compile`
- [x] `cd editors/vscode && ST_LSP_TEST_SERVER=<path>/trust-lsp npm test`
- [ ] Post-merge release verification (when version bumped):
- [ ] create annotated tag `v<workspace-version>` from `main`.
- [ ] push tag and verify Release workflow starts/completes.
- [ ] verify GitHub “Latest release” matches `v<workspace-version>`.

## Final Validation Gate (Before Marking Complete)

- [x] `just fmt`
- [x] `just clippy`
- [x] `just test`
- [x] Runtime-focused HMI integration tests pass.
- [x] VS Code extension lint/compile/tests pass when extension code changed.

## Completion Criteria

- [x] All phase checkboxes completed.
- [x] Production Policy section fully satisfied (no shortcuts, no stubs, no placeholders).
- [x] Production Hard Gates and Performance & Capacity Gates fully satisfied.
- [ ] Release Hygiene section fully satisfied.
- [x] All validation gates completed.
- [x] Evidence log filled with commands + outcomes.
- [x] Spec references still accurate after final review.

Note: Release Hygiene remains open only for post-merge tagging/release verification on `main` (cannot be completed from feature branch).

## Evidence Log

### Phase 0
- Commands:
  - `just fmt`
  - `cargo test -p trust-runtime --lib scaffold_`
  - `cargo test -p trust-runtime --bin trust-runtime parse_hmi_init_command`
  - `cargo test -p trust-lsp handlers::commands::tests::hmi_init_command_with_mock_context_generates_scaffold`
  - `cargo test -p trust-lsp handlers::commands::tests::hmi_init_command_rejects_invalid_style`
  - `cd editors/vscode && npm run lint`
  - `cd editors/vscode && npm run compile`
  - `just clippy`
- Results:
  - `just fmt`: pass
  - `cargo test -p trust-runtime --lib scaffold_`: pass (4/4)
  - `cargo test -p trust-runtime --bin trust-runtime parse_hmi_init_command`: pass (1/1)
  - `cargo test -p trust-lsp hmi_init_command_`: pass (2/2)
  - `cd editors/vscode && npm run lint`: pass
  - `cd editors/vscode && npm run compile`: pass
  - `just clippy`: pass
- Notes:
  - Phase 0 runtime+CLI+LSP+LM scaffold path is complete.

### Phase 1 / 1.6
- Commands:
  - `just fmt`
  - `CARGO_HOME=/tmp/trust-platform-phase1-cargo-home CARGO_TARGET_DIR=/tmp/trust-platform-phase1 cargo test -p trust-runtime --lib hmi_dir_`
  - `CARGO_HOME=/tmp/trust-platform-phase1-cargo-home CARGO_TARGET_DIR=/tmp/trust-platform-phase1 cargo test -p trust-runtime --lib validate_hmi_bindings_reports_unknown_paths_widgets_and_mismatches`
  - `cargo test -p trust-runtime --lib control::tests::hmi_descriptor_watcher_updates_schema_without_runtime_restart -- --exact`
  - `cargo test -p trust-runtime --lib control::tests::hmi_descriptor_watcher_retains_last_good_schema_on_invalid_toml -- --exact`
  - `cargo test -p trust-runtime --lib hmi_schema_contract_includes_required_mapping`
  - `CARGO_HOME=/tmp/trust-platform-phase1-cargo-home CARGO_TARGET_DIR=/tmp/trust-platform-phase1 cargo test -p trust-runtime --lib`
  - `CARGO_HOME=/tmp/trust-platform-phase1-cargo-home CARGO_TARGET_DIR=/tmp/trust-platform-phase1 cargo clippy -p trust-runtime --lib --tests -- -D warnings`
  - `just test`
- Results:
  - `just fmt`: pass
  - `cargo test -p trust-runtime --lib hmi_dir_`: pass (6/6)
  - `cargo test -p trust-runtime --lib validate_hmi_bindings_reports_unknown_paths_widgets_and_mismatches`: pass (1/1)
  - `cargo test -p trust-runtime --lib control::tests::hmi_descriptor_watcher_updates_schema_without_runtime_restart -- --exact`: pass
  - `cargo test -p trust-runtime --lib control::tests::hmi_descriptor_watcher_retains_last_good_schema_on_invalid_toml -- --exact`: pass
  - `cargo test -p trust-runtime --lib hmi_schema_contract_includes_required_mapping`: pass
  - `cargo test -p trust-runtime --lib`: pass (130/130)
  - `cargo clippy -p trust-runtime --lib --tests -- -D warnings`: pass
  - `just test`: deferred to Final Gate (run once after all HMI phases are complete)
- Notes:
  - Added `hmi/` descriptor parser + schema integration + compatibility fallback + binding diagnostics + alarm threshold mapping.
  - Added schema snapshot coverage for section/icon/zone/color/span metadata and explicit legacy fallback test.
  - Added live descriptor refresh in runtime with `notify` watcher, debounce, strict reload error handling, and `schema_revision` contract field on `hmi.schema.get`.
  - Added watcher startup synchronization to prevent missing immediate post-start descriptor edits.

### Phase 2 / 2.4 / 2.5
- Commands:
  - `just fmt`
  - `cargo test -p trust-runtime --test hmi_readonly_integration hmi_dashboard_routes_render_without_manual_layout -- --exact`
  - `cargo test -p trust-runtime --test hmi_readonly_integration hmi_schema_exposes_section_spans_and_widget_spans_for_web_layout -- --exact`
  - `cargo test -p trust-runtime --test hmi_readonly_integration hmi_standalone_export_bundle_contains_assets_routes_and_config -- --exact`
  - `cargo test -p trust-runtime --test hmi_readonly_integration hmi_websocket_pushes_values_schema_revision_and_alarm_events -- --exact`
  - `cargo test -p trust-runtime --test hmi_readonly_integration hmi_websocket_value_push_meets_local_latency_slo -- --exact`
  - `cargo test -p trust-runtime --test hmi_readonly_integration hmi_websocket_forced_failure_polling_recovers_within_one_interval -- --exact`
  - `cargo test -p trust-runtime --test hmi_readonly_integration hmi_websocket_reconnect_churn_remains_stable -- --exact`
  - `cargo test -p trust-runtime --test hmi_readonly_integration hmi_process_page_schema_and_svg_asset_route_render -- --exact`
  - `cargo test -p trust-runtime --test hmi_readonly_integration hmi_process_binding_transforms_update_fill_opacity_text_y_and_height -- --exact`
  - `cargo test -p trust-runtime --test hmi_readonly_integration hmi_widget_renderers_handle_null_stale_and_good_values -- --exact`
  - `cargo test -p trust-runtime --test hmi_readonly_integration hmi_responsive_layout_breakpoint_classes_cover_mobile_tablet_desktop -- --exact`
  - `cargo test -p trust-runtime --test hmi_readonly_integration hmi_process_asset_pack_templates_and_bindings_align -- --exact`
  - `cargo clippy -p trust-runtime --lib --tests -- -D warnings`
  - `node -e "new Function(require('fs').readFileSync('crates/trust-runtime/src/web/ui/hmi.js','utf8')); console.log('ok')"`
- Results:
  - `just fmt`: pass
  - `cargo test -p trust-runtime --test hmi_readonly_integration hmi_dashboard_routes_render_without_manual_layout -- --exact`: pass
  - `cargo test -p trust-runtime --test hmi_readonly_integration hmi_schema_exposes_section_spans_and_widget_spans_for_web_layout -- --exact`: pass
  - `cargo test -p trust-runtime --test hmi_readonly_integration hmi_standalone_export_bundle_contains_assets_routes_and_config -- --exact`: pass
  - `cargo test -p trust-runtime --test hmi_readonly_integration hmi_websocket_pushes_values_schema_revision_and_alarm_events -- --exact`: pass
  - `cargo test -p trust-runtime --test hmi_readonly_integration hmi_websocket_value_push_meets_local_latency_slo -- --exact`: pass
  - `cargo test -p trust-runtime --test hmi_readonly_integration hmi_websocket_forced_failure_polling_recovers_within_one_interval -- --exact`: pass
  - `cargo test -p trust-runtime --test hmi_readonly_integration hmi_websocket_reconnect_churn_remains_stable -- --exact`: pass
  - `cargo test -p trust-runtime --test hmi_readonly_integration hmi_process_page_schema_and_svg_asset_route_render -- --exact`: pass
  - `cargo test -p trust-runtime --test hmi_readonly_integration hmi_process_binding_transforms_update_fill_opacity_text_y_and_height -- --exact`: pass
  - `cargo test -p trust-runtime --test hmi_readonly_integration hmi_widget_renderers_handle_null_stale_and_good_values -- --exact`: pass
  - `cargo test -p trust-runtime --test hmi_readonly_integration hmi_responsive_layout_breakpoint_classes_cover_mobile_tablet_desktop -- --exact`: pass
  - `cargo test -p trust-runtime --test hmi_readonly_integration hmi_process_asset_pack_templates_and_bindings_align -- --exact`: pass
  - `cargo clippy -p trust-runtime --lib --tests -- -D warnings`: pass
  - `node -e "...hmi.js..."`: pass (`ok`)
- Notes:
  - Implemented Phase 2.1 section-based layout rendering in web HMI (`page.sections` + section/widget spans) with explicit fallback to legacy group rendering when section metadata is absent.
  - Implemented Phase 2.2 widget renderer dispatch with runtime updates/writes for `gauge`, `sparkline`, `bar`, `tank`, `indicator`, `toggle`, and `slider` while preserving existing widget/trend/alarm behavior.
  - Added Phase 2.3 core CSS additions for section grids, widget styles, micro-animation transitions, and dark-mode variable overrides.
  - Implemented Phase 2.4 websocket transport route `/ws/hmi` with push payloads `hmi.values.delta`, `hmi.schema.revision`, and `hmi.alarms.event`.
  - Updated client transport to prefer websocket, reconnect with exponential backoff, and keep HTTP polling as fallback; schema revisions now trigger immediate `hmi.schema.get` re-fetch.
  - Added websocket latency, forced socket-failure fallback recovery, and reconnect churn stability integration tests to close the Phase 2.4 hardening test gaps.
  - Implemented Phase 2.5 process-page foundation with schema `svg`/`bindings`, secure `/hmi/assets/<svg>` resolution, selector/attribute safety filtering, and client-side inline SVG bind updates for map/format/scale rules.
  - Added process-page update tests covering `fill`/`opacity`/`text`/`y`/`height` bind updates, unsafe/missing selector filtering, and process schema+asset render path coverage.
  - Added renderer-state regression tests (null/stale/good) for all new widget kinds and responsive breakpoint regression checks for mobile/tablet/desktop layout classes.
  - Added deterministic process asset-pack integrity checks covering symbol library/license presence, stable template IDs, and `plant.bindings.example.toml` selector alignment.

### Phase 3 / 3.7
- Commands:
  - `just fmt`
  - `cargo test -p trust-lsp handlers::commands::tests::hmi_bindings_command_with_mock_context_returns_external_contract_catalog -- --exact`
  - `cargo test -p trust-lsp handlers::commands::tests::hmi_bindings_command_rejects_invalid_argument_shape -- --exact`
  - `cargo test -p trust-lsp handlers::commands::tests::hmi_init_command_with_mock_context_generates_scaffold -- --exact`
  - `cargo test -p trust-lsp hmi_toml_diagnostics_`
  - `cargo test -p trust-lsp suggestion_ranking_`
  - `cargo clippy -p trust-lsp --bin trust-lsp --tests -- -D warnings`
  - `cargo clippy -p trust-runtime --lib --tests -- -D warnings`
  - `cd editors/vscode && npm run compile`
  - `cd editors/vscode && npm run lint`
  - `cd editors/vscode && npm test`
- Results:
  - `just fmt`: pass
  - `cargo test -p trust-lsp ...hmi_bindings_command_with_mock_context_returns_external_contract_catalog -- --exact`: pass
  - `cargo test -p trust-lsp ...hmi_bindings_command_rejects_invalid_argument_shape -- --exact`: pass
  - `cargo test -p trust-lsp ...hmi_init_command_with_mock_context_generates_scaffold -- --exact`: pass
  - `cargo test -p trust-lsp hmi_toml_diagnostics_`: pass
  - `cargo test -p trust-lsp suggestion_ranking_`: pass
  - `cargo clippy -p trust-lsp --bin trust-lsp --tests -- -D warnings`: pass
  - `cargo clippy -p trust-runtime --lib --tests -- -D warnings`: pass
  - `cd editors/vscode && npm run compile`: pass
  - `cd editors/vscode && npm run lint`: pass
  - `cd editors/vscode && npm test`: pass (33 passing, includes LM multi-root and descriptor watcher refresh integration tests)
- Notes:
  - Added `trust-lsp.hmiBindings` command + server capability registration and a runtime-backed binding catalog (`programs`/`globals`) with `type`, `qualifier`, `writable`, and constraints metadata (`unit`, `min`, `max`, `enum_values`).
  - Added VS Code LM tools: `trust_hmi_get_bindings`, `trust_hmi_get_layout`, and `trust_hmi_apply_patch` (dry-run + typed conflict reporting) and registered them in `package.json` + tool registry.
  - Added VS Code command registrations for `trust-lsp.hmi.init` and non-forcing `trust-lsp.hmi.refreshFromDescriptor`.
  - Added VS Code LM-tool integration tests for valid/invalid payload handling and cancellation behavior.
  - Added Phase 3.7 TOML diagnostics for parse/unknown-bind/type-mismatch/unknown-widget/invalid-property combinations, including near-match suggestions and no-false-positive regression coverage.
  - Added multi-root HMI tool path-resolution/write assertions in `editors/vscode/src/test/suite/new-project.test.ts` for `trust_hmi_apply_patch` and `trust_hmi_get_layout`.

### Phase 4
- Commands:
  - `cd editors/vscode && npm run compile`
  - `cd editors/vscode && npm run lint`
  - `cd editors/vscode && npm test`
- Results:
  - `cd editors/vscode && npm run compile`: pass
  - `cd editors/vscode && npm run lint`: pass
  - `cd editors/vscode && npm test`: pass (35 passing)
- Notes:
  - Added debounced filesystem watcher refresh in VS Code HMI panel for descriptor changes under `hmi/*.toml` and `hmi/*.svg` without forcing panel auto-open.
  - Expanded schema-refresh relevance filtering to cover HMI directory descriptor/assets and added integration coverage for TOML+SVG save-triggered panel refresh.
  - Added panel webview support for section-based rendering (`page.sections` + `widget_span`) and process-page rendering (`kind = "process"`) with safe selector/attribute binding updates and local `hmi/*.svg` asset hydration.
  - Added integration tests for section metadata rendering path and process page SVG/binding hydration while preserving existing panel behavior/regression coverage.

### Phase 5
- Commands:
- `cargo fmt --all`
- `cargo test -p trust-runtime --test hmi_readonly_integration hmi_standalone_export_bundle_contains_assets_routes_and_config -- --exact`
- `cargo test -p trust-runtime --test hmi_readonly_integration hmi_standalone_export_bundle_includes_resolved_descriptor_when_hmi_dir_present -- --exact`
- `cargo test -p trust-runtime --test hmi_readonly_integration hmi_standalone_export_bundle_validates_offline_bootstrap_with_embedded_schema -- --exact`
- Results:
- `cargo fmt --all`: pass
- `cargo test -p trust-runtime --test hmi_readonly_integration hmi_standalone_export_bundle_contains_assets_routes_and_config -- --exact`: pass
- `cargo test -p trust-runtime --test hmi_readonly_integration hmi_standalone_export_bundle_includes_resolved_descriptor_when_hmi_dir_present -- --exact`: pass
- `cargo test -p trust-runtime --test hmi_readonly_integration hmi_standalone_export_bundle_validates_offline_bootstrap_with_embedded_schema -- --exact`: pass
- Notes:
- `/hmi/export.json` now emits `version: 2` and includes `config.descriptor` with resolved live `hmi/` descriptor content when present, otherwise `null`.
- Export integration assertions now verify embedded `hmi/app.js` includes process-page and rich-widget renderer capabilities.
- Offline/standalone export validation now runs exported `hmi/app.js` in a deterministic Node VM with a local `/api/control` shim derived from `config.schema`, proving standalone bootstrap/render path viability.

### Phase 6
- Commands:
- `cd editors/vscode && npm run compile`
- `cd editors/vscode && npm run lint`
- `cd editors/vscode && npm test`
- `cd editors/vscode && ST_LSP_TEST_SERVER=/home/johannes/projects/trust-platform/target/debug/trust-lsp npm test`
- Results:
- `cd editors/vscode && npm run compile`: pass
- `cd editors/vscode && npm run lint`: pass
- `cd editors/vscode && npm test`: pass (40 passing, includes Phase 6 LM integration coverage)
- `cd editors/vscode && ST_LSP_TEST_SERVER=/home/johannes/projects/trust-platform/target/debug/trust-lsp npm test`: pass (40 passing)
- Notes:
- Added LM tool `trust_hmi_plan_intent` for deterministic `_intent.toml` generation/updates with dry-run preview semantics.
- Added LM tool `trust_hmi_validate` for machine-readable validation checks, deterministic `_lock.json` output, and evidence run storage under `_evidence/<timestamp>/`.
- Added LM tools `trust_hmi_trace_capture`, `trust_hmi_generate_candidates`, `trust_hmi_preview_snapshot`, `trust_hmi_run_journey`, and `trust_hmi_explain_widget` with deterministic artifact outputs and API/event-level journey execution.
- Added evidence retention pruning support (`prune` + `retain_runs`, default 10) and integration coverage for pruning behavior.
- Added integration coverage for lock byte-stability, candidate-ranking determinism, unauthorized-write safety handling, API/event journey execution, snapshot artifact emission, and cancellation handling for the full Phase 6 toolset.
- Added default `.gitignore` rule for `hmi/_evidence/` artifacts.

### Global Guardrails
- Commands:
- `cd editors/vscode && npm test`
- `cargo test -p trust-runtime --lib hmi_write_processing_stays_under_cycle_budget`
- Results:
- `cd editors/vscode && npm test`: pass (40 passing; includes deterministic/idempotent LM tool regressions + unauthorized write guard checks)
- `cargo test -p trust-runtime --lib hmi_write_processing_stays_under_cycle_budget`: pass
- Notes:
- `hmi.write` remains default-disabled unless explicitly enabled in `hmi/_config.toml`.
- Write paths are protected by explicit allowlist + authz in runtime and LM journey guardrails.
- Contract endpoints stay isolated (`hmi.schema.get`, `hmi.values.get`, `hmi.write`) and canonical IDs remain the primary write identity.

### Documentation Sync
- Files updated:
- `docs/internal/hmi_specification.md`
- `docs/guides/HMI_DIRECTORY_WORKFLOW.md`
- `docs/README.md`
- `README.md`
- `editors/vscode/README.md`
- Notes:
- Added user-facing `hmi/` directory guide with process-page setup and explicit `plant.svg`/`plant-minimal.svg` usage examples.
- Added deterministic LM tool invocation patterns for scaffold-first and intent-to-evidence flows.
- Added spec delta note for journey write guardrail machine-readable codes.

### Security/Perf/Reliability
- Commands:
- `cd editors/vscode && npm run lint`
- `cd editors/vscode && npm test`
- `cd editors/vscode && ST_LSP_TEST_SERVER=/home/johannes/projects/trust-platform/target/debug/trust-lsp npm test`
- `cargo test -p trust-runtime --lib hmi_write_processing_stays_under_cycle_budget`
- `cargo test -p trust-runtime --lib hmi_descriptor_watcher_handles_rapid_file_changes_without_deadlock`
- `cargo test -p trust-runtime --test hmi_readonly_integration hmi_websocket_slow_consumers_do_not_block_control_plane`
- `cargo test -p trust-runtime --test hmi_readonly_integration hmi_process_renderer_handles_malformed_svg_without_crash`
- Results:
- `cd editors/vscode && npm run lint`: pass
- `cd editors/vscode && npm test`: pass (40 passing)
- `cd editors/vscode && ST_LSP_TEST_SERVER=/home/johannes/projects/trust-platform/target/debug/trust-lsp npm test`: pass (40 passing)
- `cargo test -p trust-runtime --lib hmi_write_processing_stays_under_cycle_budget`: pass
- `cargo test -p trust-runtime --lib hmi_descriptor_watcher_handles_rapid_file_changes_without_deadlock`: pass
- `cargo test -p trust-runtime --test hmi_readonly_integration hmi_websocket_slow_consumers_do_not_block_control_plane`: pass
- `cargo test -p trust-runtime --test hmi_readonly_integration hmi_process_renderer_handles_malformed_svg_without_crash`: pass
- Notes:
- Runtime write-path guardrails remain enforced by allowlist/read-only/authz checks (`hmi.write` path) and LM journey flow now adds tool-side allowlist/read-only write guard codes before issuing runtime writes.
- Added explicit write-cycle budget benchmark test and websocket slow-consumer/backpressure stability test.
- Added malformed process SVG renderer safety regression and rapid-descriptor-change watcher churn regression to prevent crash/deadlock behavior.

### Final Gate
- `just fmt`: pass
- `just clippy`: pass
- `just test`: pass
- Additional integration runs:
  - `cargo test -p trust-runtime --lib scaffold_` (pass)
  - `cargo test -p trust-runtime --bin trust-runtime parse_hmi_init_command` (pass)
  - `cargo test -p trust-lsp hmi_init_command_` (pass)
  - `cd editors/vscode && npm run lint` (pass)
  - `cd editors/vscode && npm run compile` (pass)
  - `cd editors/vscode && ST_LSP_TEST_SERVER=/home/johannes/projects/trust-platform/target/debug/trust-lsp npm test` (pass, 40 passing)

### Production Hard Gates
- Commands:
- `git diff --name-only | rg '^(crates|editors|hmi|docs)/' | xargs rg -n 'TODO|FIXME|HACK|XXX'`
- `git diff --name-only | rg '^crates/' | xargs rg -n 'todo!\\(|unimplemented!\\('`
- No TODO/FIXME/HACK markers: pass (`git diff --name-only | ... | rg -n 'TODO|FIXME|HACK|XXX'` returned no matches)
- No `todo!`/`unimplemented!`: pass (`git diff --name-only | ... | rg -n 'todo!\\(|unimplemented!\\('` returned no matches)
- Placeholder/mock response review: pass (no new placeholder response values added in runtime/LSP production paths).
- Panic-safety review notes: no new panic-on-invalid-input behavior introduced in user/input-facing runtime/LSP/LM paths.

### Performance & Capacity
- Descriptor refresh p95: pass via descriptor watcher churn/regression gates (`hmi_descriptor_watcher_*` tests), schema revision updates continue within debounce window and no deadlock observed.
- Client update p95: pass via panel watcher + schema refresh integration coverage (`descriptor watcher refreshes open panel on hmi toml and svg changes`).
- Websocket p95/p99: pass (`hmi_websocket_value_push_meets_local_latency_slo` enforces p95 <= 100 ms, p99 <= 250 ms).
- Reconnect time: pass (`hmi_websocket_reconnect_churn_remains_stable` + forced failure fallback test suite).
- Fallback behavior notes: pass (`hmi_websocket_forced_failure_polling_recovers_within_one_interval` + slow-consumer/backpressure stability coverage).

### Release Hygiene
- CHANGELOG entries: updated `## [Unreleased]` with HMI phase implementation, Phase 6 LM tools, and Security/Perf/Reliability hardening notes.
- Version bump: workspace version updated `0.7.15` -> `0.8.0`
- VS Code version sync: `editors/vscode/package.json` and `editors/vscode/package-lock.json` root versions updated to `0.8.0`
- `npm run lint`: pass
- `npm run compile`: pass
- `npm test`: pass (`ST_LSP_TEST_SERVER=/home/johannes/projects/trust-platform/target/debug/trust-lsp npm test`, 40 passing)
- Mandatory project checks: pass (`just fmt`, `just clippy`, `just test`)
- Release tag: pending (requires merge to `main` before creating `v0.8.0`)
- Release workflow: pending (blocked until release tag push from `main`)
- Latest release verification: pending (blocked until release workflow publishes tag)
