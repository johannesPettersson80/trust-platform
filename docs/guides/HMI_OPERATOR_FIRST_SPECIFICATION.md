# Operator-First HMI Specification (V4)

Status: Draft

Primary target:
- Under 5 minutes from first `trust-runtime run` to a curated, operator-ready HMI.

This specification defines both:
1. Operator UX contract
2. Deterministic generation contract (code -> scaffold -> curation)

Related:
- `docs/internal/hmi_specification.md`
- `docs/guides/HMI_DIRECTORY_WORKFLOW.md`
- `docs/guides/HMI_OPERATOR_FIRST_IMPLEMENTATION_CHECKLIST.md`

## 1. Product Promise

The first-run experience must be zero-config and impressive:
1. Developer has only PLC code.
2. Runtime scaffolds required HMI pages automatically (or prompts a single command if auto-scaffold is disabled).
3. Terminal prints a clear ready line with URL and page count.
4. Browser opens a live HMI with meaningful defaults.
5. Engineer can curate visually in-browser without editing TOML manually.

Required startup output example:
- `HMI ready: http://127.0.0.1:18082/hmi (5 pages scaffolded, edit mode available)`

## 2. Page Set and Purpose

Required top-level pages:
1. `Overview` - single-glance health and core KPIs
2. `Process` - topology/P&ID context with live state
3. `Control` - commands and setpoint writes
4. `Trends` - temporal behavior and drift/transient analysis
5. `Alarms` - triage, priority, acknowledgement

Optional pages:
- `Settings` (role-gated)
- domain pages (`quality`, `energy`, `maintenance`)

Bypass default behavior:
- Represented as mode/state in `Process` + `Control`.
- Separate `Bypass` page only when explicitly configured.

## 3. Lifecycle Modes (`init`/`update`/`reset`)

The scaffold contract must support re-runs safely.

### `init`
- Creates `hmi/` from scratch.
- Fails if `hmi/` already exists unless `--force`.

### `update`
- Merges new signals/pages into existing descriptor files.
- Preserves user edits and layout intent.
- Fills missing required pages when project is incomplete.

### `reset`
- Regenerates all scaffold-owned files.
- Keeps backup snapshots before overwrite.

If `hmi/` exists but is incomplete, runtime must run `update` semantics, not silently degrade.

## 4. Input Scope for Generation

Default eligible points:
- `VAR_INPUT`, `VAR_OUTPUT`, `VAR_IN_OUT`, `VAR_GLOBAL`, `VAR_EXTERNAL`
- properties with get/set semantics

Default excluded points:
- internal `VAR`, `VAR_TEMP`, `VAR_STAT`, constants

Fallback for projects with only locals (no explicit external interface):
- Allow scaffold with inferred visibility from discovered runtime points.
- Mark these points as `inferred_interface = true` in scaffold metadata for user review.

## 5. Auto-Generation Story for All Required Pages

## 5.1 Overview (always generated)
- Budget: 8-12 widgets (default 10)
- Uses deterministic scoring and slot caps (section 6)
- Must not render full inventory

## 5.2 Process (always generated)

Two modes:

1. `custom-svg`
- Used when descriptor references user-provided SVG.

2. `auto-schematic` (default fallback)
- Used when no project SVG exists.
- Rendered as generated SVG (stable IDs) so bindings and drill-down still work.
- Layout algorithm: deterministic left-to-right flow lanes.
- Node groups inferred from naming/function hints (`source/tank`, `pump/actuator`, `valve/control`, `measurement`, `sink/product`).
- Connection inference:
  - Prefer explicit producer/consumer relations when available.
  - Otherwise infer from naming pairs and process domains.
  - Unknown links rendered as dashed advisory edges.
- Symbol policy:
  - Prefer bundled industrial symbols (`hmi/pid-symbols/`) when matching shape exists.
  - Fall back to simplified canonical glyphs when symbol mapping is unknown.

Minimum output quality:
- A non-empty topology with at least one main flow path and anchored live values.
- Never a raw unordered card grid on Process page.

Promotion path:
- Dropping user SVG + bindings upgrades page from `auto-schematic` to `custom-svg` with preserved signal mappings where possible.

### 5.2.1 Grid and Anchor Contract (mandatory for auto-schematic)

Auto-schematic Process pages must be generated from a deterministic grid contract, not free-pixel placement.

Required placement rules:
1. A single global process grid is declared in generated SVG metadata (`origin`, `cell_w`, `cell_h`).
2. Equipment/symbol anchor points must land on integer grid cells only.
3. Main process pipes must route through connector anchors only; no floating gaps or partial overlaps.
4. Instrument templates must be type-stable:
   - `FIT` and `PT` share the same footprint and stem geometry by default.
   - Tag and value offsets are identical between `FIT` and `PT` unless explicitly overridden.
5. Instrument text offset rule:
   - Value text baseline is placed exactly one grid row above the instrument centerline (label may be above value).
6. Stem rule:
   - Instrument stem endpoint lands exactly on the routed process line y-coordinate.
7. Collision rule:
   - Auto-resolution may only shift by whole grid cells and must preserve relative anchor offsets.
8. Tank fill rule:
   - Percent-to-visual fill mapping must update both `y` and `height` so displayed percent and fill height remain consistent.

Validation requirements:
- Generated SVG includes hidden guide layer for debug/inspection.
- Scaffold validation fails if anchor/grid rules are violated.
- Snapshot tests assert deterministic coordinates for core instrument templates (`FIT`, `PT`, `pump`, `valve`, `tank`).

## 5.3 Control (always generated)
- Generated from writable points.
- Sections:
  1. Commands (writable BOOL)
  2. Setpoints (writable numeric)
  3. Modes (writable enum/selectors)
  4. Writable text fields (if allowed)
- Safety UX:
  - Confirmation required for command-class writes (`start`, `stop`, `reset`, `enable`, `bypass`) unless explicitly disabled by policy.
  - Bounds and validation always shown for setpoints.
  - If matching PV exists, show PV beside each setpoint.

## 5.4 Trends (generated when numeric points exist)
- Curated default: 6-8 signals.
- Includes high-interest KPI/deviation signals.
- Default time range: 10 minutes.
- Standard controls: 1m / 10m / 1h presets.
- Auto-scale enabled by default with optional fixed range when limits are known.

## 5.5 Alarms (always generated)
- Use explicit alarm rules when present.
- Otherwise infer starter rules from ranges:
  - `min+max`: `low = min + 10% span`, `high = max - 10% span`
  - only `max`: `high = 90% max`
  - only `min`: infer conservative low bound only when nominal context exists
- Default inferred deadband/hysteresis: `2% of span`.
- Mark inferred alarms as `inferred = true` for explicit review.

## 6. Deterministic Scoring and Slot Allocation

Scoring weights (Overview):
- safety/alarm state: `100`
- command/mode state: `80`
- KPI PV/SP: `60`
- deviation: `45`
- inventory headline: `35`
- counters/diagnostics: `20`

Slot caps (default budget 10):
- safety/alarm: max 2
- command/mode: max 2
- KPI+SP/deviation groups: max 4
- inventory: max 2
- diagnostics/fillers: max 2

Tie-breakers (in order):
1. explicit priority metadata
2. declaration order
3. stable lexical path order

Overflow signals go to non-Overview pages.

Trends selection weights:
- start from Overview KPI set
- add temporal-interest bonus from observed variance/change rate
- cap static/rarely-changing config values

## 7. Visual Hierarchy Contract

Auto-layout must use `widget_span` and structural rails, not flat equal cards.

Overview structure:
1. top status rail (connection/mode/alarm summary)
2. prominent safety card/banner
3. primary KPI cards
4. PV/SP/deviation grouped cards
5. compact state indicators

Default 12-column spans:
- banner: 12
- primary KPI: 3-4
- PV/SP/deviation cluster: 4-6
- compact indicators: 2

Reference wireframe:

```text
[ status rail ---------------------------------------------------------- ]
[ safety summary banner ----------------------------------------------- ]
[ KPI A (4) ][ KPI B (4) ][ KPI C (4) ]
[ PV/SP/DEV group (6)     ][ PV/SP/DEV group (6)     ]
[ state (2) ][ state (2) ][ state (2) ][ inventory (3) ][ inventory (3) ]
```

## 8. In-Browser Curation Mode (Critical)

The browser HMI must support engineering-role layout curation without manual TOML editing.

### 8.1 Widget-level actions
- Remove from current page (hide placement, not delete signal)
- Move to another page
- Change widget type (compatible types only)
- Edit label inline
- Resize span (`small/medium/large` mapped to numeric span)
- Pin/unpin on Overview

### 8.2 Page-level actions
- Reorder sections
- Rename section titles
- Add widgets from available/unplaced pool

### 8.3 System-level actions
- Available-signals panel
- Reset to scaffold defaults

### 8.4 Save and source-of-truth
- Changes are persisted to `hmi/*.toml` (descriptor remains canonical).
- Save path uses typed descriptor update API (e.g. `hmi.descriptor.update`).
- Live refresh applies immediately after save.
- File watchers in editor/runtime observe updates.

## 9. Cross-Page Navigation and Drill-Down

Required drill-downs:
- Alarm row -> Process page focus/highlight
- Overview KPI -> Trends page focused signal
- Setpoint value click -> Control page target card

Priority tiers:
- P0: Alarm -> Process, KPI -> Trends
- P1: Setpoint -> Control, context-preserving back navigation

Deep-link examples:
- `/hmi?page=process&focus=<id>`
- `/hmi?page=trends&signal=<id>`
- `/hmi?page=control&target=<id>`

## 10. Operator vs Engineering Mode UX

Operator mode default:
- hide internal paths and raw types

Engineering mode:
- show path/type/quality/binding health
- optional diagnostics overlay

Mode switching:
- topbar toggle button
- keyboard shortcut (`g`)
- optional URL param (`?mode=engineering`)

Persistence:
- per browser session/profile (localStorage default)

## 11. Settings Scope and Phase Split

Phase 0 (within 5-minute target):
- overview size preset
- pin/unpin/move/hide
- density
- operator/engineering mode

Phase 2 (extended admin):
- shared-default publishing
- role policy editing
- advanced theme profile management

## 12. Motion, Staleness, and Feedback Standards

Default timing budgets:
- value transition: 300ms ease-out
- page transition: 200ms
- value-update highlight: 1s fade
- stale threshold: 5s no update
- disconnected threshold: 10s no heartbeat

Per-widget quality state is mandatory (`good|stale|bad`), not global-only.

## 13. Transport and Live Data Contract

Live transport:
- WebSocket primary
- Polling fallback when WebSocket unavailable
- Automatic reconnect with exponential backoff

Performance targets:
- value update latency p95: < 100ms (local)
- schema/live refresh after descriptor edit: < 500ms target

## 14. Developer Feedback Loop

Must provide:
1. runtime/schema hot refresh for descriptor edits
2. clear malformed descriptor errors in browser and editor
3. VS Code inline preview as first-class workflow
4. binding diagnostics visible in both LSP diagnostics and engineering overlay

Error UX requirement:
- malformed TOML or invalid binds must surface actionable messages (file + field + reason), not silent failure.

## 15. Responsive and Kiosk Modes

Minimum support:
- tablet-usable layout at ~768px width
- kiosk mode for wall displays (reduced chrome, persistent critical context)

## 16. Standalone Export

HMI must support standalone/exported bundle generation and validation:
- export includes schema, assets, and bootstrap route metadata
- useful for demos, review, and offline inspection

## 17. Theme and Visual Identity

Theme semantics are fixed:
- green = normal
- amber = warning/degraded/stale
- red = critical alarm/fault
- neutral grays = structure/inactive

Design direction requirement:
- modern, clean, minimal, professional visual identity
- polished light default plus polished dark mode
- dark mode must be designed, not naive inversion

## 18. Versioning and Migration

Descriptor contract must be versioned.

Requirements:
- include descriptor version marker in config
- forward-compatible read behavior where possible
- migration tooling for scaffold format updates

Re-scaffold behavior:
- user edits preserved by default (`update` mode)
- explicit `reset` required for destructive regeneration

## 19. Acceptance Criteria (5-Minute Success)

A project with only PLC code passes when:
1. Scaffold generation: < 2s target on local project
2. Runtime startup to "HMI ready" output: < 5s target
3. Browser first load on localhost: < 2s target
4. First paint shows live values (not only static labels)
5. All 5 required pages exist and are usable
6. Overview has 8-12 critical items with hierarchy
7. Process page exists via custom SVG or auto-schematic fallback
8. Auto-schematic Process page passes grid/anchor validation (instrument/value offsets and pipe anchors deterministic)
9. Control page safely exposes writable points with validation/confirm behavior
10. Trends default to curated 6-8 useful signals
11. Alarms include explicit or inferred thresholds with deadband
12. P0 drill-downs work (Alarm->Process, KPI->Trends)
13. Engineer can curate in browser and save back to descriptor files
14. Operator mode hides internal names/types by default
15. Per-widget staleness is visible

## 20. Non-Goals

- Auto-generating final production P&ID artistry without human refinement
- Inferring perfect domain intent from variable names alone

The generator must deliver a strong draft fast; human curation is first-class.
