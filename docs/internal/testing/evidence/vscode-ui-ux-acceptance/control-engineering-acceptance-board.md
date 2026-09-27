# Control engineering UI acceptance board

Status: Current plant editor, examples/curriculum and Control Studio journeys under remediation

## Current remediation acceptance — 2026-09-05

The historical acceptance records below describe their original fixtures and
revisions. They do **not** accept the current 84-lesson rewrite, nine-model
selector, changed examples routing/copy behavior, or revised Control Studio
opening path. The user rejected the long plant settings form and examples
destination behavior. These changed journeys require fresh rendered evidence
and independent review; no historical `ux_accepted` status transfers to them.

Current source: `feat/trust-control-tutorial-completion`, base `f5ba9d02`, plus
preserved curriculum WIP and remediation changes. Implementers: root and
delegated agents. Independent reviewer for the current revision: **unassigned**.

The user approved continuation with the first-layout organization on 2026-09-05.
This approves the direction, not the unfinished model forms or full journey.
The optional FOPDT temperature example now has focused rendered proof: explicit
replacement review, physical parameter mapping, percent/Celsius assumptions,
unchanged generic imports and exact stored-value inspection. CPR-J02 also gains
per-field actuator blank/default meanings, with zero distinct from omission.
Root reviewed current temperature dark/light/high-contrast/narrow screenshots;
the 380px split has no horizontal overflow. Multiport model journeys remain
under implementation, and no independent novice acceptance is claimed.

| ID | User journey and surface | Observable acceptance | Current evidence and remaining work | Status |
| --- | --- | --- | --- | --- |
| CPR-J01 | Devices & Connections: Add simulated plant, select FOPDT from Plant model | Exactly nine actual models in one list; plain-language model/connection/starting-condition sections; no equipment cards or expert gate; optional physical example with explicit unit assumptions | First-layout and `control-remediation/2026-09-05/CPUI-005-temperature-example-*.png` theme/split captures reviewed; registered rendered physical/canonical, invalid-input, replacement and equation assertions pass. Keyboard focus checked. All-model and save/reopen journeys remain open | Direction approved; focused temperature slice implemented; independent acceptance pending |
| CPR-J02 | Edit model values, open/close Model details, correct an invalid value | Invalid numeric/vector text remains visible and blocks Save; correction restores valid save; disclosure never discards invalid input or saves old values | Registered real CDP assertion red/green: invalid gain and vector retained; vector remains invalid across disclosure; correcting gain alone still blocks Save; correcting vector restores Save. `CPFIX-005-invalid-rendered.png`, `CPFIX-005-recovered-rendered.png`; focused test passes, not full extension suite | Implemented focused slice; independent acceptance pending |
| CPR-J03 | Edit a saved plant name, save and reopen | Exact unrelated validity, transforms, effects and channels retained; cleared managed fields stay cleared; saved state distinct from runtime activation | Native candidate roundtrip tests pass. Actual robot and 2×2 state-space Save/reopen now passes with all channels, conversions, dimensions and selected matrix value retained; normal/300px split captures reviewed. Customized label-only validity/effects journey remains open | Open; multiport persistence implemented |
| CPR-J04 | Home > Start from example > topic > lesson > Create project | Topic browses examples; Read guide alone opens docs; existing destination can be renamed without overwriting; selected project opens its own instructions; copied next-lesson guidance works | Focused route/copy tests pass. Real VS Code Lesson 1 selection, project creation, normal folder reload and its own README preview verified; before/after PNGs reviewed, all nine assertions true and copied metadata/Main hashes match source (`CPEX-009-example-folder-transition.json`). Other topics, failures, themes and next-lesson journeys pending | Selected-lesson happy path verified; full acceptance open |
| CPR-J05 | Open Control Studio from Home or command palette | Same configured loop, labelled project chooser, harmless cancel, actionable invalid/missing configuration; no guessed bindings | 13 registered command/sidebar tests pass with real files and selected UI responses stubbed; real chooser/error interaction pending | Open |
| CPR-J06 | Record a short-scan lesson and save/reopen evidence | Complete runtime scan acquisition or explicit gaps/overflow/reset boundaries; no every-scan claim from slower UI snapshots | Runtime-owned recording and current rendered lesson evidence remain unimplemented | Open |
| CPR-J07 | Select robot joint, set starting angle and speed, save and reopen | Two labelled rad/rad/s fields in Starting conditions; no raw array required; independent edits, invalid-text guard and exact persistence | Expected rendered red then green; exact -1.5 rad / 1.25 rad/s Save/reopen; dark/light/high-contrast and 300px split captures reviewed under control-remediation/2026-09-05. First-order, integrator, gravity-tank and second-order labelled starting states also pass registered rendered tests | Focused implementation verified; novice acceptance open |
| CPR-J08 | Open selected Lesson 1 details in the shipped gallery | Full objective, description, prerequisites and expected observation; lesson-specific preview, not generic headings; no page overflow | Registered rendered assertion red/green; dark/light/high-contrast and 900px-window captures reviewed under `control-remediation/2026-09-05/gallery-detail-*` | Focused rendering verified; novice acceptance open |
| CPR-J09 | Browse all with a missing-metadata lesson in an isolated bundle | Valid starter remains usable; damaged lesson shows its reason and offers no Create action; original manifest restored after test | Registered rendered assertion red/green (`unavailable-rendered-baseline-red.log`, `unavailable-rendered-green.log`) | Focused rendering verified; selected-context recovery still open |
| CPR-J10 | Browse all bundled examples | Compact labelled hardware/category selects combine with search; count/reset stay accurate; six starters separately identify simulator execution and optional physical integration | Registered filter and badge assertion reds/greens; dark/light/high-contrast/narrow captures, including visible TwinCAT optional-hardware badge (`cpex005-execution-badges.md`) | Focused filters/badges verified; keyboard-only journey and independent acceptance remain open |
| CPR-J11 | Open plant optional settings; edit and close each destination | Actuator, conversions, realism and scenarios are separate; one editor at a time; configured/disabled settings remain summarized; invalid draft still blocks Save | Two rendered red/green slices and seven-title regression; current dark/light/high-contrast/narrow split captures reviewed (`cpui017-019-optional-destinations.md`) | Focused destination/summary behavior verified; final effects and scenario forms remain open |
| CPR-J12 | Schedule a scenario in seconds and select a robot-joint quantity | Exact 1.001 s/1001 ms Save/reopen; invalid raw time blocks Save across disclosure; named state/sensor targets retain numeric channel 1 | Three native tests and isolated rendered timing/target assertions after expected reds, with real TOML inspection (`cpui019-scenario-time-and-targets.md`) | Focused time/target behavior verified; all actions, parameter wording and novice acceptance remain open |
| CPR-J13 | Add noise and bias, then navigate to an error with Realism closed | One active effect editor, order preserved, correct fields; error opens the right effect and focuses its invalid value | Registered rendered expected red/green and reviewed `effects-focus-dark.png`; `cpui017-focused-effect-cards.md` | Focused uncertainty cards verified; individual actuator editors and full UX acceptance remain open |
| CPR-J14 | Rename a saved model with omitted starting values | Show original dynamics and resolved zero state; preserve omission on name-only Save; persist explicit nonzero state | Native all-nine projection and runtime behavior lock plus real first-order Save/reopen; reviewed `omitted-state-defaults-dark.png`; `cpfix006-omitted-starting-state.md` | Focused saved-model/default behavior verified; cross-model acceptance remains open |
| CPR-J15 | Rename a fully configured saved plant without losing other settings | Dynamics, histories, conversions, actuator, validity, uncertainties and events survive Save/reopen; unrelated model and simulation settings stay unchanged | Full-envelope rendered red/green and reviewed `cpfix003-full-preservation-dark.png`; native unknown-key rejection keeps bytes unchanged (`cpfix-003-full-plant-save.md`) | Focused preservation verified; final theme and novice acceptance remain open |
| CPR-J16 | Retry opening a copied lesson after a folder-open failure | Real Notifications Center shows the retained path and Retry; unobscured pointer click repeats the same open without overwriting a learner edit | Registered focused rendered test and root-reviewed `cpex009-retry-unobscured.png`; earlier covered/transitional captures rejected | Dark-theme recovery verified; second open intercepted here, real folder transition proved separately; full novice acceptance open |

Evidence paths in this section are relative to `docs/internal/testing/evidence/`.
Core changed surfaces require dark, light, high contrast, narrow and keyboard
verification. User design review and novice acceptance cannot be replaced by
agent approval or passing source-text assertions.

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
| CSTUDIO-J11 | Actual simulated heater begins at 35 °C / 75% with explicit unlimited-request and retained-integral bindings | Open Control Studio; run the reviewed project-owned 35→45→35 °C Test; inspect the retained saturation card | Requested output remains separate from 0–100% applied clamp; integral change and release delay use complete runtime evidence; the card recommends anti-windup without a reset/write or capacity claim | Registered rendered red/green and current `saturation-windup-actual/{light,dark,high-contrast}.png` captures | Focused rendered journey verified; independent UX review and package acceptance pending |

## Example and curriculum discovery journeys

| ID | Preconditions and deterministic fixture | Exact user actions | Visible and safety assertions | Required evidence | Status |
| --- | --- | --- | --- | --- | --- |
| CURRICULUM-J01 | Shipped extension with the generated 76-project catalog | Open Start from example; inspect topic routes and starters; open Browse all; return; open Control engineering; select Feedback foundations | One entry exposes 12 documentation routes, six starters with honest hardware badges, all 76 copyable projects, five ordered control courses, and direct lesson actions | `example-gallery-all-routes-*`, `example-gallery-browse-all-*`, `control-curriculum-overview-*`, and `control-course-foundation-*` screenshots in all three themes; runner assertions and JSON | `ux_accepted` — Harvey |
| CURRICULUM-J02 | Feedback foundations course with Lesson 2 available | Open Water heater P; use the example; verify project files; render its README | Course position, prerequisite, next lesson, equipment, units, theory, experiment, validation, and advanced exercise remain visible; copied project is editable and simulation evidence is not labelled physical evidence | `control-water-heater-p-gallery-*`, `control-water-heater-p-readme-*`, copied-file assertions, rendered-heading assertions, and JSON | `ux_accepted` — Harvey |

## Review record

Harvey (`plant_ui_reviewer`) previously accepted the Plant Simulation matrix
and fixture-backed Control Studio captures. That historical Plant Simulation acceptance remains
recorded; current remediation acceptance is governed by the open CPR rows above. Control Studio acceptance was reopened because the shipped
water-heater lesson exposed a blank chart, the complete Identify path is not
repeatable, and the earlier MIMO proof was an injected component payload rather
than a shipped runtime journey. Current Control Studio rows must be independently
re-reviewed only after the open real-journey evidence is complete.

Harvey also independently accepted the then-current Example Gallery and control
curriculum discovery journey after verifying the course view and unified
browse-all route in dark, light, and high contrast. The review found no
remaining discoverability, hierarchy, contrast, or professional-quality
blocker.

Physical hardware remains explicitly outside this evidence and cannot be
replaced by simulation or fake-boundary proof.

## Lesson 14 experiment and runtime observation regression — 12 September 2026

| ID | Preconditions | User actions | Required result and evidence | Status |
|---|---|---|---|---|
| CPREL-UI-L014 | Actual Lesson14 open-loop Main/native heater, 35 °C/75%, current shared extension | Test settings → inspect → invalid value → review/apply 12/25 → qualification → review/change/review/start → Monitor → Validate → Save/Open | Full native tJ9Xl2 preserves 303 rows including five-second baseline and 25-second 87% step. Final short kDJgkZ covers locked/focused settings and historical display; all 13 originals personally and independently inspected for polish, ease, discoverability and shared consistency. [Image-by-image evidence](/home/johannes/projects/.artifacts/control-platform-release-20260911/lesson14-self-review.md) | Bounded desktop dark-theme review accepted; whole lesson/all themes/user acceptance open |
| CPREL-UI-PID-OBS | Actual Control.PID Lesson11 and native heater after per-session I/O ordering repair | Start → Abort → Save/Open → restart → force bad PV → blocked Test → release → Save/Open | Native zJ9xuZ passes 23.834 s; all 8 originals personally and independently reviewed. Fresh runtime starts at 35 °C/75%, faults leave MV zero and the latch set, and both original records remain historical. [Independent review](/home/johannes/projects/.artifacts/control-platform-release-20260911/lesson14-independent-image-review.md) | Bounded lifecycle/UI regression accepted; all-family recovery gate open |

## Recording import and historical status — 12 September 2026

| ID | Preconditions | User actions | Required result and evidence | Status |
|---|---|---|---|---|
| CPREL-UI-L015-IMPORT | Actual Lesson15 open-loop Main/native heater and preserved303-row Lesson14 result | Identify → Import recording → source details → Check → Fit preview → Record validation navigation → Validate → Save/Open → Monitor | Native2vQxwm passes12.305s; original samples/identities retained, current loop unchanged, one primary action, unclipped source details, recording before fit, preview-only model and explicit live return. All8 originals personally/independently reviewed. [Current review](/home/johannes/projects/.artifacts/control-platform-release-20260911/lesson15-self-review.md) | Accepted for bounded desktop dark import journey; full lesson, file-picker/cancel/invalid variants, themes and full editor persistence open |
| CPREL-UI-PID-HISTORY | Actual Control.PID Lesson11 after shared historical-footer cleanup | Start/Abort/Save/Open/restart/forced-PV stop/release/Save/Open | Native5MsAOr passes24.051s; all8 current originals reviewed. Historical footer repetition fixed; source/values preserved. [Independent review](/home/johannes/projects/.artifacts/control-platform-release-20260911/lesson15-pid-status-independent-review.md) | Functional regression passes; fresh Monitor's prior-Test notice needs an explicit historical distinction (CPREL-B08) |
| CPREL-UI-CATALOG-STAGES | Rebuilt actual gallery containing all82 accepted lesson identities | Learning path → module → Manual PI / Routh–Hurwitz lesson detail | Native selected-detail cases pass1.063s and1.084s; current objective/observation/title appear, content fits horizontally. Both originals personally and independently inspected. [Review](/home/johannes/projects/.artifacts/control-platform-release-20260911/catalog-stage-independent-review.md) | Catalog rendering verified; repeated objective and oversized metadata/observation hierarchy remain open (CPREL-B09) |

## Identify method matrix — 14 September 2026

| ID | Surface and actions | Evidence | Status |
| --- | --- | --- | --- |
| CSTUDIO-IDENTIFY-METHODS-20260914 | Identify: all seven scalar acquisition methods, all three applicable multichannel acquisitions, recording/manual fit, supplied model, project experiment, prerequisites, result/reopen/reset | [Method checklist](../../checklists/control-studio-identification-method-acceptance.md); [23 individual screenshot reviews](/home/johannes/projects/.artifacts/control-pid-preview-20260914/all-identification-methods/image-review.md). Actual Control.PID/native plant for generic acquisitions; imported-data scope explicitly separate. MIMO reviewed in dark/light/high contrast. | Focused execution and image self-review pass. Independent reviewer/user acceptance and full release gates remain separate; `ux_accepted` is not asserted. |

## Connected live Identify correction — 14 September 2026

The earlier method matrix proved isolated batch acquisition and must not be cited as live-plant proof. Current evidence executes the running Control.PID/native-plant project, writes through the bounded native experiment service and records native scans. Reset and reopening resume current selected signals; plots grow during acquisition.

| Scope | Current evidence | Status |
| --- | --- | --- |
| Seven scalar methods; two-input PRBS/Multisine; native measured inputs/outputs; model fit, reopen and Reset | [Commands and per-run results](/home/johannes/projects/.artifacts/control-pid-preview-20260914/live-identification/commands.md) | Passed for the declared native-simulator fixtures |
| Continuous live progress, independent scan-buffer observation, Abort restoring MV/mode, close/reopen | [Final five named checks](/home/johannes/projects/.artifacts/control-pid-preview-20260914/live-identification/final/tests.json) | Passed |
| Individual method plots, actual in-progress plot and user desktop preview | [Screenshot self-review](/home/johannes/projects/.artifacts/control-pid-preview-20260914/live-identification/image-review.md) | Personally inspected; bounded Identify scope, not whole-platform UX/release acceptance |

## One Identify experiment; explicit Validate — 14 September 2026

This supersedes the two-record acquisition order in the preceding live-correction
evidence. Identify now fits one provisional record; only explicit Validate
collects the independent model-check record and executes the controller campaign.

| Scope | Current evidence | Status |
| --- | --- | --- |
| One live acquisition, provisional model and explicit stage boundaries | [Task and expected red/green](../../checklists/control-studio-single-identification.md) | Implemented and focused verification passed |
| Actual Control.PID/native temperature plant: model and controller requirements, transfer check, Apply eligibility and edited-requirement invalidation | [Final five named checks](/home/johannes/projects/.artifacts/control-pid-preview-20260914/single-identification/final/tests.json) | Passed; no Apply transaction sent |
| Seven scalar methods, two-input PRBS/Multisine, live progress, Reset, Abort, reopen and existing desktop preview | [Commands and proof scope](/home/johannes/projects/.artifacts/control-pid-preview-20260914/single-identification/commands.md) | Passed within declared simulator fixtures |
| Provisional model, unchanged compact actions, Tune stopping and Validate live/results | [Individual image review](/home/johannes/projects/.artifacts/control-pid-preview-20260914/single-identification/image-review.md) | Self-review passed for this correction; whole-platform/user acceptance remains separate |

## Automatic Tune setup — 2026-09-14

| Scope | Evidence | Status |
| --- | --- | --- |
| Identified native plant → Tune without empty requirements modal; optional prefilled editor, persisted criteria and explicit Validate | [Active task](../../checklists/control-studio-automatic-tuning-setup.md) | Focused native journey and dark/light/high-contrast screenshot self-review passed; whole-platform/user acceptance remains separate |
