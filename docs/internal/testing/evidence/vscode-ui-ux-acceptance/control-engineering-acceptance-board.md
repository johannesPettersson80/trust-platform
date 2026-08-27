# Control engineering UI acceptance board

Status: Plant Simulation UI and Control Studio accepted

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
| CSTUDIO-J01 | Running `live-simulation-project` FOPDT fixture opened with source `Simulation` | Open Control Studio; select Monitor; inspect state and live values; focus, navigate, reset, and open the plot table | Exact loop/runtime, source, mode, revision, freshness, quality, SP/PV/MV units, limits, and labelled plot are visible; stale values are unavailable | Extension Host runner keys `liveLoop`, `chart`, `chartKeyboard`; J01 screenshot | `ux_accepted` — Harvey |
| CSTUDIO-J02 | Healthy simulated loop with bounded setpoint plan and restoration contract | Select Test; enter amplitude/duration; Review; verify the exact plan; Approve; watch live values; Abort | Review does not execute; SP/PV/MV stay visible; Abort is enabled only while running; terminal state reports `user_abort` and restored value | Runner keys `testReview`, `testRunning`, `testTerminal`; running-test screenshot | `ux_accepted` — Harvey |
| CSTUDIO-J03 | Accepted recording and normal runtime apply/rollback path | Identify; review model; Tune; compare time/Bode plots; Validate requirements; review application; Apply; Rollback; Save | Model and candidate remain distinct; candidate is unapplied; values, limits, units, sources, and reasons remain inspectable; application and rollback are explicit | Runner keys `engineering`, `applicationReview`, `applied`, `rolledBack`, `saved`; validation screenshot | `ux_accepted` — Harvey |
| CSTUDIO-J04 | Healthy loop with exact simulated model and no active experiment | Open Automatic Tune; select objective; review and approve exact plan; follow Test, Identify, Tune, Validate; review result | Stage progress and evidence source are visible; selected structure and metrics are explained; result remains unapplied until a separate confirmation | Runner keys `automaticPlan`, `automaticResult`, `configuredAutomatic`; plan/result screenshots | `ux_accepted` — Harvey |
| CSTUDIO-J05 | Deterministic invalid, stale, unsuitable, failed, disconnected, and recovered fixtures | Exercise invalid test configuration, failed identification, failed tuning, restoration failure, reconnect, and recovered validation | No invented evidence; actionable reasons are visible; valid recording/model/candidate is retained; stale values remain unavailable | Runner keys `loopFailure`, `failureJourney`; six failure/recovery screenshots | `ux_accepted` — Harvey |
| CSTUDIO-J06 | Strict physical-source boundary fixture; no physical hardware attached | Select Test; acknowledge independently reviewed topology; inspect exact plan and immediate approval; do not execute | Physical source, limits, timeout, stops, and restoration are visible; preview is not connected; approval is exact and single-use | Runner key `physicalBoundary`; approval-boundary screenshot; automated boundary tests only | `ux_accepted` for UI boundary — physical execution unavailable |
| CSTUDIO-J07 | Accepted fixture in real Extension Host with VS Code theme and layout controls | Navigate modes and plot by keyboard; inspect wide, split, narrow, 200 percent zoom, dark, light, and high contrast | Focus and semantics remain visible; chart table exists; navigation/workspace remain usable; no page-level horizontal overflow or colour-only meaning | Runner keys `afterKeyboard`, `chartKeyboard`, `responsive`; seven-image rendered matrix | `ux_accepted` — Harvey |

## Review record

Harvey (`plant_ui_reviewer`) independently accepted the corrected Plant
Simulation representative matrix and the current rendered Control Studio
journeys. The accepted evidence includes current screenshots, interaction
records, responsive and theme variants, keyboard operation, numerical
precision, units, stopped-runtime truth, and 200 percent layouts.

Physical hardware remains explicitly outside this evidence and cannot be
replaced by simulation or fake-boundary proof.
