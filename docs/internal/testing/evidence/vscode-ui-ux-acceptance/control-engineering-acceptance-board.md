# Control engineering UI acceptance board

Status: Plant Simulation UI acceptance retained; Control Studio acceptance under remediation

This board is the rendered-journey authority for the Plant Simulation UI and
Control Studio checklists. A checked implementation row may link here only
after its automated interaction assertions, current screenshots, and the named
independent review are complete. Simulation evidence is never physical-plant
evidence.

## Shared capture rule

Verify the completed shared surface once in dark, light, high contrast, wide,
narrow, and 200 percent zoom. Individual journeys add screenshots only when a
different state is material. Keyboard proof includes focus and plot-table
access; plot proof identifies signals without relying on colour.

Implementer: Codex. Independent reviewer: Harvey (`plant_ui_reviewer`). An
implementer cannot set `ux_accepted`; the reviewer must inspect the current
rendered result and record their identity and decision.

## Plant Simulation UI journeys

Detailed, documentation-ready steps and explanations live in
[Plant Simulation VS Code acceptance journeys](../vscode-plant-simulation-ui/plant-simulation-acceptance-journeys.md).

| ID | Preconditions and deterministic fixture | Exact user actions | Visible and safety assertions | Required evidence | Status |
| --- | --- | --- | --- | --- | --- |
| PSUI-J01 | Offline project with numeric `%Q` command and `%I` feedback | Add Simulated plant; choose Tank; map signals; review values; Save and reopen | One simulated child brick; concise essential fields; exact values retained; no live chart in Plant Properties | Add, guided-editor, save/reopen tests and current screenshot/JSON | `ux_accepted` — Harvey |
| PSUI-J02 | Eligible runtime and open Add picker | Activate by click, Enter, Space, valid drag/drop; cancel invalid drag with Escape | All valid routes open the same draft; invalid/cancelled routes write nothing; focus restores correctly | Phase 4 rendered pointer, keyboard, drag, rejection, and cancel evidence | `ux_accepted` — Harvey |
| PSUI-J03 | Wrong-direction, absent, duplicate, or conflicting signal candidate | Select invalid signal; attempt Save; read field error; correct; Save | Owning field names exact conflict and repair; focus moves there; draft is retained | Registered conflict suite and rendered conflict screenshot/JSON | `ux_accepted` — Harvey |
| PSUI-J04 | Runtime on revision A and saved revision B | Edit; Save; inspect saved versus active identity; use existing restart workflow | Save never implies activation; Restart required is truthful; no duplicate lifecycle controls | Save-result/topology tests and Phase 8 evidence | `ux_accepted` — Harvey |
| PSUI-J05 | Saved plant, resolved PID loop, reachable simulation runtime | Open in Control Studio; inspect live state; Configure plant to return | Exact project/runtime/plant/loop and Simulation source; current values; identity-only handoff; same plant restored on return | Typed handoff tests and live Extension Host journey/screenshots | `ux_accepted` — Harvey |
| PSUI-J06 | Saved scalar plant | Configure actuator; add/reorder imperfection and equal-time events; correct invalid row; Save | Progressive fields and concise help; omitted versus zero preserved; displayed execution order; exact row errors | Phase 7 native, extension, and rendered evidence | `ux_accepted` — Harvey |
| PSUI-J07 | Advanced custom-model draft | Edit piecewise points or state-space dimensions/matrices; Save; reopen | Indexed keyboard order; destructive resize confirmation; local grid scrolling; exact precision round trip | Phase 6 editor tests and rendered screenshots/JSON | `ux_accepted` — Harvey |
| PSUI-J08 | Saved plant and unrelated TOML content | Edit and save; Remove; cancel; repeat and confirm | Normal brick interaction; cancellation writes nothing; only plant and owned events removed; focus returns to runtime | Full save/remove interaction records and screenshots | `ux_accepted` — Harvey |
| PSUI-J09 | Enabled physical driver overlaps simulated address | Attempt Save; inspect owner/range; choose free address; Save | Fail closed before mutation; exact owner and repair route; no physical-I/O override | Runtime ownership and Extension Host conflict assertions | `ux_accepted` — Harvey |
| PSUI-J10 | Dirty local draft plus stale saved/runtime state | Attempt Save; review current values; explicitly reload or keep draft; reconnect | Local work retained; reload is explicit; stale/unavailable/current states remain distinct | Stale-revision interaction record, screenshot, and failure-state tests | `ux_accepted` — Harvey |

## Control Studio journeys

Detailed, documentation-ready steps and explanations live in
[Control Studio VS Code acceptance journeys](../vscode-control-studio/control-studio-acceptance-journeys.md).

| ID | Preconditions and deterministic fixture | Exact user actions | Visible and safety assertions | Required evidence | Status |
| --- | --- | --- | --- | --- | --- |
| CSTUDIO-J01 | Shipped `control-water-heater-p` lesson running in its simulator | Open Control Studio; select Monitor; inspect state and live values; use Inspect, Zoom, Fit, Expand, and Export | Exact copied project, lesson, loop/runtime, plant, signal addresses, source, mode, revision, quality, SP/PV/MV units, limits, and a non-empty labelled plot are visible | `water-heater-real-ui-audit-final4-2026-08-31/json/CSTUDIO-REAL-UI-AUDIT.json`; `CSTUDIO-REAL-monitor-response-dark.png` | Current real dark/wide journey passed; independent re-review and display variants pending |
| CSTUDIO-J02 | Healthy simulated loop with bounded setpoint plan and restoration contract | Select Test; enter amplitude/duration; Review; verify the exact plan; Approve; watch live values; Abort | Review does not execute; SP/PV/MV stay visible; Abort is enabled only while running; terminal state reports `user_abort` and restored value | Current shipped-lesson runner keys `testReview`, `testRunning`, `testTerminal`; running-test screenshot | Real water-heater interaction passed; independent re-review pending |
| CSTUDIO-J03 | Accepted recording and normal runtime apply/rollback path | Identify; review model; Tune; compare time/Bode plots; Validate requirements; review application; Apply; Rollback; Save | Model and candidate remain distinct; candidate is unapplied; values, limits, units, sources, and reasons remain inspectable; application and rollback are explicit | A complete repeatable shipped-lesson journey is required | Open — the water-heater identification path rejected physically plausible runs and is not repeatable |
| CSTUDIO-J04 | Healthy loop with exact simulated model and no active experiment | Open Automatic Tune; select objective; review and approve exact plan; follow Test, Identify, Tune, Validate; review result | Stage progress and evidence source are visible; selected structure and metrics are explained; result remains unapplied until a separate confirmation | Current shipped-lesson runner keys `automaticPlan`, `automaticResult`, `configuredAutomatic`; plan/result screenshots | Automatic review opens on the real lesson; complete current journey and independent re-review pending |
| CSTUDIO-J05 | Deterministic invalid, stale, unsuitable, failed, disconnected, and recovered fixtures | Exercise invalid test configuration, failed identification, failed tuning, restoration failure, reconnect, and recovered validation | No invented evidence; actionable reasons are visible; valid recording/model/candidate is retained; stale values remain unavailable | Runner keys `loopFailure`, `failureJourney`; six failure/recovery screenshots | Component-fixture proof only; real shipped-lesson recovery acceptance pending |
| CSTUDIO-J06 | Strict physical-source boundary fixture; no physical hardware attached | Select Test; acknowledge independently reviewed topology; inspect exact plan and immediate approval; do not execute | Physical source, limits, timeout, stops, and restoration are visible; preview is not connected; approval is exact and single-use | Runner key `physicalBoundary`; approval-boundary screenshot; automated boundary tests only | Component boundary proof only; physical execution unavailable and UI re-review pending |
| CSTUDIO-J07 | Shipped lesson in real Extension Host with VS Code theme and layout controls | Navigate modes and plot by keyboard; inspect wide, split, narrow, 200 percent zoom, dark, light, and high contrast | Focus and semantics remain visible; chart table exists; navigation/workspace remain usable; no page-level horizontal overflow or colour-only meaning | Current screenshots from the same shipped lesson and revision | Open — prior fixture matrix does not replace current shipped-lesson variants |
| CSTUDIO-J08 | Two bounded simulated setpoint experiments from the same settled operating point | Qualify the estimation recording; Fit preview; verify acceptance is unavailable; return to baseline; record a different amplitude; Qualify; Fit validation; inspect equation, units, uncertainty and measured/predicted/residual chart; Accept | Same-run preview cannot reach Tune; independent-run metrics and evidence remain compact and explicit; out-of-order scans cannot enter the recording; acceptance is a separate action | `curriculum-identify-2026-08-28/json/CSTUDIO-P4-shell-dark.json`; `CSTUDIO-J04-identification-review-dark.png`; focused seven-test Identify slice and out-of-order scan red/green | Implemented and rendered; independent UX review pending |
| CSTUDIO-J09 | Extension Development Host with no project, then any truST project | Inspect the disabled truST destinations; open the water-heater P lesson; start its simulator; activate Control Studio directly below Devices & Connections | Control Studio is always discoverable; the no-project action is disabled with an explicit reason; the lesson opens its explicitly configured loop in the existing editor with current SP/PV/MV; contextual plant handoff remains available | Focused sidebar/loop-binding red-green; `vscode-control-studio/sidebar-2026-08-31/no-project.png`; `vscode-control-studio/sidebar-2026-08-31/water-heater-live.png` | Implemented and rendered; independent review pending |
| CSTUDIO-J10 | Shipped Control Studio with typed SISO and MIMO engineering evidence | Open direct stage plots; inspect Bode, Nichols, channel responses, and singular values; edit Kp; resize to split, narrow, high contrast, and 200 percent | No plot catalogue or mode switch; plots remain dominant and readable; names, units, axes, trace identity, no-data recovery, and unapplied graphical edits remain explicit without page overflow | A shipped MIMO lesson with `control-loop.json`, actual runtime channels, model evidence, and current screenshots is required | Acceptance withdrawn — previous MIMO rendering used an injected synthetic payload; no shipped MIMO lesson is currently bound to Control Studio |

## Example and curriculum discovery journeys

| ID | Preconditions and deterministic fixture | Exact user actions | Visible and safety assertions | Required evidence | Status |
| --- | --- | --- | --- | --- | --- |
| CURRICULUM-J01 | Shipped extension with the generated 76-project catalog | Open Start from example; inspect topic routes and starters; open Browse all; return; open Control engineering; select Feedback foundations | One entry exposes 12 documentation routes, six starters with honest hardware badges, all 76 copyable projects, five ordered control courses, and direct lesson actions | `example-gallery-all-routes-*`, `example-gallery-browse-all-*`, `control-curriculum-overview-*`, and `control-course-foundation-*` screenshots in all three themes; runner assertions and JSON | `ux_accepted` — Harvey |
| CURRICULUM-J02 | Feedback foundations course with Lesson 2 available | Open Water heater P; use the example; verify project files; render its README | Course position, prerequisite, next lesson, equipment, units, theory, experiment, validation, and advanced exercise remain visible; copied project is editable and simulation evidence is not labelled physical evidence | `control-water-heater-p-gallery-*`, `control-water-heater-p-readme-*`, copied-file assertions, rendered-heading assertions, and JSON | `ux_accepted` — Harvey |

## Review record

Harvey (`plant_ui_reviewer`) previously accepted the Plant Simulation matrix
and fixture-backed Control Studio captures. Plant Simulation acceptance remains
recorded. Control Studio acceptance is reopened because the shipped
water-heater lesson exposed a blank chart, the complete Identify path is not
repeatable, and the earlier MIMO proof was an injected component payload rather
than a shipped runtime journey. Current Control Studio rows must be independently
re-reviewed only after the open real-journey evidence is complete.

Harvey also independently accepted the current Example Gallery and control
curriculum discovery journey after verifying the course view and unified
browse-all route in dark, light, and high contrast. The review found no
remaining discoverability, hierarchy, contrast, or professional-quality
blocker.

Physical hardware remains explicitly outside this evidence and cannot be
replaced by simulation or fake-boundary proof.
