# VS Code Control Studio Implementation Checklist

## Required purpose checkpoint

Release-level gates and the final user walkthrough are tracked in the
[Control platform release-readiness checklist](control-platform-release-readiness.md).

Read the [Control Platform Purpose and Acceptance Contract](../../../specs/control-platform-purpose.md)
for every new or resumed task on this checklist. Record a fresh Start/Finish
entry using its Section 6; existing checkmarks retain only their stated evidence.

- [ ] **Start (current/next task):** identify the platform part, owning spec,
  existing worktree and implementation, and actual acceptance target.
- [ ] **Finish (current/next task):** name the controller/project/plant and what
  actually ran or was reviewed; link evidence and state gaps without substituting
  a simpler fixture for the intended goal.


Status: in progress

Product authority:
[`35-vscode-control-studio.md`](../../../specs/35-vscode-control-studio.md)

Related contracts:

- [`32-control-pid.md`](../../../specs/32-control-pid.md)
- [`33-simulation-plant-models.md`](../../../specs/33-simulation-plant-models.md)
- [`34-vscode-plant-simulation-ui.md`](../../../specs/34-vscode-plant-simulation-ui.md)

## Working rule

For each unchecked behavior: confirm the specification, write and register the
smallest executable test, run it to the expected behavior assertion failure,
implement the minimum change, and run the same test green. Existing conforming
behavior needs a green behavior-lock test, not an artificial failure.

Reuse the existing runtime control client, authorization, audit, plant
status/trace, shared theme, chart adapter, and VS Code editor infrastructure.
Do not create parallel communication, styling, persistence, or runtime systems.

Run focused tests while implementing. Run broad repository and rendered UI
gates once after the implementation is frozen.

## 1. Existing foundation

- [x] CSTUDIO-001 Recover the canonical and working checkouts, preserve dirty
  work, and verify `AGENTS.md` and `.codex/skills`.
  Evidence: [Phase 0 recovery](../evidence/vscode-control-studio/phase-0-recovery-and-baseline.md).
- [x] CSTUDIO-002 Establish the focused Control Studio module boundary without
  adding behavior to the network canvas or generic runtime panels.
  Evidence: [Phase 1 architecture preparation](../evidence/vscode-control-studio/phase-1-architecture-preparation.md).
- [x] CSTUDIO-003 Register `Open Control Studio` and reuse the same editor for
  the same plant or loop.
  Evidence: [Phase 4 shell](../evidence/vscode-control-studio/phase-4-shell.md).
- [x] CSTUDIO-004 Consume the typed simulated-plant handoff without copying
  live values or editable plant configuration.
  Evidence: [Phase 3 handoff](../evidence/vscode-control-studio/phase-3-loop-and-handoff.md).
- [x] CSTUDIO-005 Render the compact header, Monitor/Test/Identify/Tune/Validate
  navigation, persistent monitor region, and Automatic Tune action.
  Evidence: [Phase 4 shell](../evidence/vscode-control-studio/phase-4-shell.md).
- [x] CSTUDIO-006 Use the shared chart adapter and packaged ECharts modules.
  Evidence: [Phase 4 chart adapter](../evidence/vscode-control-studio/phase-4-shell.md#shared-chart-adapter-red-green).

## 2. Loop and Monitor

- [x] CSTUDIO-007 Test and implement complete loop binding for controller, SP,
  PV, MV, units, direction, limits, mode, timing, runtime, and optional plant.
- [x] CSTUDIO-008 Reject missing, incompatible, ambiguous, stale, and
  cross-runtime bindings without guessing.
- [x] CSTUDIO-009 Open from a simulated plant, configured loop or PID instance,
  and command palette; focus an existing editor for the same loop.
- [x] CSTUDIO-010 Resolve current state through the existing runtime control
  client; do not introduce another transport or lifecycle owner.
- [x] CSTUDIO-011 Show live synchronized SP, PV, and MV with units, quality,
  freshness, mode, connection, revision, limits, and saturation.
- [x] CSTUDIO-012 Preserve the monitor, selected signals, cursor, time window,
  and recording when changing workflow mode.
- [x] CSTUDIO-013 Show stale data as historical, clear unavailable current
  values, and never reconstruct missing runtime evidence.
- [x] CSTUDIO-014 Start and stop a bounded recording containing synchronized
  samples, timing, quality, units, source, and loop revision.
- [x] CSTUDIO-015 Show concise loop-health summaries only from available
  evidence and avoid unsupported causal diagnosis.

Evidence for CSTUDIO-007..015: [final implementation and verification](../evidence/vscode-control-studio/final-implementation-and-verification.md), especially the registered loop-definition, loop-monitor, command, handoff, stale-data, recording, and live rendered Monitor evidence.

## 3. Test

- [x] CSTUDIO-016 Support observation-only recording and a bounded setpoint
  step through the existing runtime path.
- [x] CSTUDIO-017 Support a bounded output step only when controller mode,
  runtime capability, and authority permit it.
- [x] CSTUDIO-018 Show amplitude, duration, affected signal, predicted bound,
  stop conditions, and restoration value without a large advanced form.
- [x] CSTUDIO-019 Block start when freshness, timing, mode, authorization,
  limits, timeout, force, stop, or restoration requirements are invalid. Block
  physical start when required topology, fault, or interlock evidence is unknown.
- [x] CSTUDIO-020 Require confirmation for simulation and explicit approval
  immediately before a physical test; any material change requires approval
  again.
- [x] CSTUDIO-021 Keep live SP, PV, MV, injected signal, limits, elapsed time,
  and `Abort test` visible while armed or running.
- [x] CSTUDIO-022 Stop on abort, timeout, stale critical data, affected-signal
  force, or runtime termination and attempt restoration through the same path.
- [x] CSTUDIO-023 Recover authoritative test state after editor reload or
  reconnect without duplicating an excitation.
- [x] CSTUDIO-024 Retain recorded data and report completed, aborted, failed,
  or restoration-failed with the exact stop reason.
- [x] CSTUDIO-025 Prove a real simulation test, abort, and restoration through
  the shipped runtime control path.

Evidence for CSTUDIO-016..025: [final implementation and verification](../evidence/vscode-control-studio/final-implementation-and-verification.md), including the focused runtime experiment tests and the live Test/review/run/Abort/restoration journey.

## 4. Identify

- [x] CSTUDIO-026 Review input/output selection, units, sample time, gaps,
  quality, clipping, saturation, drift, excitation, and data suitability.
- [x] CSTUDIO-027 Allow interval exclusion without deleting or rewriting raw
  samples.
- [x] CSTUDIO-028 Fit integrating, first-order, FOPDT, second-order, and SOPDT
  models using independently checked numerical cases and explicit tolerances.
- [x] CSTUDIO-029 Keep an exact configured simulation model distinct from a
  model identified from simulated I/O.
- [x] CSTUDIO-030 Show parameters, units, measured-versus-predicted response,
  validation error, residual behavior, stability, delay, range, and warnings.
- [x] CSTUDIO-031 Recommend the simplest acceptable model and prove that fit
  percentage alone cannot select it.
- [x] CSTUDIO-032 Reject insufficient, invalid, non-finite, unstable, or
  unidentifiable input with deterministic reasons and retained data.

Evidence for CSTUDIO-026..032: [final implementation and verification](../evidence/vscode-control-studio/final-implementation-and-verification.md), with registered qualification, exclusion, fitting, evidence-retention, model-selection, and deterministic-failure tests plus the rendered FOPDT result.

## 5. Tune

- [x] CSTUDIO-033 Prove that opening Tune and editing a candidate never changes
  the running controller.
- [x] CSTUDIO-034 Qualify SIMC and AMIGO against independent published cases,
  including inapplicable and invalid inputs.
- [x] CSTUDIO-035 Support manual and imported `Control.PID` settings without
  silently converting controller form or timing conventions.
- [x] CSTUDIO-036 Translate every candidate exactly to Specification 32 PI, PD,
  or PID semantics and compare prediction with the native `Control.PID`.
- [x] CSTUDIO-037 Show one recommended candidate, current controller,
  `More robust` to `Faster`, predicted response, effort, margins, constraints,
  and recommendation reason.
- [x] CSTUDIO-038 Compare permitted PI, PD, and PID candidates and choose the
  simplest candidate that passes; require material benefit before adding a
  controller term.
- [x] CSTUDIO-039 Show current and candidate time responses, controller output,
  limit use, plant/open-loop Bode, and gain and phase margins.
- [x] CSTUDIO-040 Do not show Bode or margin results without a valid model or
  measured frequency response.
- [x] CSTUDIO-041 Return `No acceptable controller found` instead of weakening
  mandatory requirements or returning an unsafe nominal result.

Evidence for CSTUDIO-033..041: [final implementation and verification](../evidence/vscode-control-studio/final-implementation-and-verification.md), covering published numerical cases, exact `Control.PID` semantics, structure selection, Bode/margin evidence, and unapplied rendered candidates.

## 6. Validate, apply, and rollback

- [x] CSTUDIO-042 Compare current and candidate controllers under identical
  setpoint, disturbance, noise, actuator, timing, and bounded-variation cases.
- [x] CSTUDIO-043 Report response, error, MV, saturation, recovery, noise, and
  stability requirements as Pass, Fail, Not evaluated, or Inconclusive with
  value, limit, unit, source, and reason.
- [x] CSTUDIO-044 Keep model-predicted, simulation-observed, recorded, and
  physical-observed validation evidence distinct.
- [x] CSTUDIO-045 Require explicit candidate review; validation alone must not
  modify the running controller.
- [x] CSTUDIO-046 Apply atomically through existing runtime authorization,
  audit, revision, and control transport; reject stale revisions.
- [x] CSTUDIO-047 Require separate explicit approval for physical apply;
  experiment approval must not authorize apply.
- [x] CSTUDIO-048 Keep SP, PV, MV, limits, active revision, and Rollback visible
  during the bounded post-apply observation period.
- [x] CSTUDIO-049 Restore the exact prior confirmed revision on rollback or
  report the restoration failure.

Evidence for CSTUDIO-042..049: [final implementation and verification](../evidence/vscode-control-studio/final-implementation-and-verification.md), including complete validation rows, evidence-source separation, revision-guarded apply, and exact rendered rollback.

## 7. Automatic Tune

- [x] CSTUDIO-050 Build one concise visible plan from existing Test, Identify,
  Tune, and Validate settings; do not duplicate their algorithms or forms.
- [x] CSTUDIO-051 Reuse suitable data or request the smallest bounded experiment
  needed for identification.
- [x] CSTUDIO-052 Allow one bounded simulation plan confirmation; require fresh
  explicit approval immediately before each physical test.
- [x] CSTUDIO-053 Sequence data qualification, experiment when needed,
  identification, PI/PD/PID comparison, validation, and recommendation.
- [x] CSTUDIO-054 Stop at the failed stage, retain valid work, and present the
  exact reason and nearest recovery action.
- [x] CSTUDIO-055 Complete only with the simplest passing candidate or
  `No acceptable controller found`.
- [x] CSTUDIO-056 Never silently apply the result; applying remains the separate
  reviewed operation from Section 6.
- [x] CSTUDIO-057 Recover after cancel, abort, disconnect, or reload without
  repeating a completed physical action.

Evidence for CSTUDIO-050..057: [final implementation and verification](../evidence/vscode-control-studio/final-implementation-and-verification.md), including successful and failed-stage orchestration tests and the distinct live configured-loop Automatic Tune plan/result captures.

## 8. Saved results and export

- [x] CSTUDIO-058 Save the recording, selected intervals, accepted model,
  candidate, validation requirements/result, source, units, method, loop
  revision, and creation time as one workflow result.
- [x] CSTUDIO-059 Preserve raw recordings when a model, candidate, or result is
  changed; do not build a general artifact database or content-digest graph.
- [x] CSTUDIO-060 Export raw samples as CSV and the complete workflow result as
  JSON without credentials or runtime secrets.
- [x] CSTUDIO-061 Preserve valid inputs and results across safe retries,
  cancellation, editor reload, and recoverable failures.

Evidence for CSTUDIO-058..061: [final implementation and verification](../evidence/vscode-control-studio/final-implementation-and-verification.md), including round-trip, raw-sample preservation, secret-free export, save, and recovery tests.

## 9. UI completion

- [x] CSTUDIO-062 Use only the shared truST theme, typography, icons, controls,
  chart adapter, and engineering-number formatter; add no private palette or
  parallel design system. Prove sensible and consistent precision for ordinary,
  large, small non-zero, negative, unavailable, controller, and model values.
- [x] CSTUDIO-063 Keep the main plot dominant and advanced parameters/help
  collapsed while errors, limits, state, Abort, and Rollback remain visible.
- [x] CSTUDIO-064 Keep the complete workflow usable at wide, split, narrow, and
  200 percent zoom layouts without page-level horizontal scrolling.
- [x] CSTUDIO-065 Support keyboard operation, visible focus, labelled fields,
  accessible plot summaries, selected values, and a tabular plot alternative.
- [x] CSTUDIO-066 Verify the completed shared surface once in dark, light, and
  high-contrast themes; do not duplicate a theme matrix for every behavior.
- [x] CSTUDIO-067 Keep analysis outside the webview UI thread and bounded; add
  cancellation only if an analysis operation becomes long-running. Keep live
  and display data bounded.
- [x] CSTUDIO-068 Provide specific empty and recovery states without fake
  curves or stale values presented as current.

Evidence for CSTUDIO-062..068: [final rendered evidence](../evidence/vscode-control-studio/final-rendered/) and [final implementation and verification](../evidence/vscode-control-studio/final-implementation-and-verification.md), including shared number formatting, real units, transition-only event markers, keyboard checks, distinct themes, 1091/720/505/338 px layouts, and zero page-level overflow.

## 10. Final proof

- [x] CSTUDIO-069 Register every changed VS Code extension test and run
  `python3 scripts/check_vscode_test_registration.py`.
- [x] CSTUDIO-070 Run focused numerical, runtime experiment, identification,
  tuning, validation, apply, rollback, and Automatic Tune tests.
- [x] CSTUDIO-071 Run the required runtime vertical and simulation workflow
  tests on `trust-builder`.
- [x] CSTUDIO-072 Run VS Code lint, compile, and the complete registered
  extension test suite on `trust-builder`.
- [x] CSTUDIO-073 Render and interact with these five shipped-extension
  journeys: simulation Monitor; complete simulation Automatic Tune; approved
  physical-test boundary without physical execution; simulation Abort and
  restoration; failure/recovery; keyboard and responsive workflow in dark,
  light, and high contrast.
- [x] CSTUDIO-074 Record physical hardware as an external boundary when the
  reviewed topology or authorization is unavailable; never substitute
  simulation evidence.
- [x] CSTUDIO-075 Obtain independent UX acceptance for the five rendered
  journeys and correct clipping, confusion, hidden safety state, or visual
  inconsistency before completion.
- [x] CSTUDIO-076 If ownership or data flow changed, update the existing
  Control Studio diagram once and pass diagram drift checking.
- [x] CSTUDIO-077 Freeze the implementation and run final remote `just fmt`,
  `just clippy`, and `just test-all` once.

Evidence for CSTUDIO-069..077: [final implementation and verification](../evidence/vscode-control-studio/final-implementation-and-verification.md). Registration found 608 tests in 60 registered files; VS Code lint/compile and all 608 extension tests passed; the five rendered journeys passed with `CONTROL_STUDIO_SHELL_DONE`; the independent reviewer accepted the fresh screenshots; diagram render/drift passed; and the final remote `just fmt`, `just clippy`, and constrained-disk `just test-all` rerun exited zero. Physical hardware was unavailable and remains an explicitly recorded external boundary.

## 11. Direct professional chart workspace

This section implements Specification 35 Sections 5, 13, 15, and 16. It
extends the completed Control Studio foundation. It MUST reuse the existing
chart adapter, engineering numerics, workflow messages, result storage, shared
theme, and extension editor. It MUST NOT add a plot catalogue, advanced mode,
learner/professional mode, arbitrary dashboard builder, second persistence
system, or second runtime communication path.

### 11.1 Direct navigation and shared chart contract

- [x] CSTUDIO-078 Specify and test one direct plot-tab row for each workflow
  stage, including the additional MIMO tabs. Tabs remain visible when
  unavailable and state the exact prerequisite. Remove the `Plot tools`
  disclosure and any nested plot selection.
  Evidence: `npx tsc -p tsconfig.json && npx mocha --ui tdd
  out/test/suite/control-studio-chart.test.js` (`exposes every stage plot
  directly...`, green; 2026-08-31).
- [x] CSTUDIO-079 Specify and test the single compact toolbar containing the
  direct stage tabs plus Inspect, Zoom out, Zoom in, Fit, Expand, and Export;
  show Undo and Redo only while editing a Tune candidate.
  Evidence: the same focused chart suite, `renders one direct chart
  workspace...` (green; 2026-08-31).
- [x] CSTUDIO-080 Specify and test per-loop/per-stage plot selection, cursor,
  linked inspection window, zoom, fit, expanded mode, and safe fallback when
  evidence or model identity changes.
  Evidence: the same focused chart suite, `clamps zoom and pan...`, `restores
  one evidence-bound direct chart session`, and direct-plot fallback (green;
  2026-08-31).
- [x] CSTUDIO-081 Specify and test one flat Signals view with friendly name,
  role, symbol, unit, value, quality, and direct checkboxes. Keep SP, PV, and MV
  in the live strip and reject incompatible-unit overlays.
  Evidence: focused tests `renders one selectable signal row with identity,
  value, unit, and quality` and `uses aligned lanes instead of silently
  overlaying three unit groups` (green on `trust-builder`; 2026-08-31), plus
  `direct-chart-full-2026-08-31/screenshots-raw/CSTUDIO-J01-signals-dark.png`.
- [x] CSTUDIO-082 Specify and test same-unit shared axes, at most two labelled
  axes, and aligned lanes for three or more unit groups. Preserve explicit MIMO
  input/output names, ordering, units, and channel selection.
  Evidence: focused chart tests `uses aligned lanes instead of silently
  overlaying three unit groups` and `accepts only dimensionally complete named
  MIMO plot evidence` (green on `trust-builder`; 2026-08-31).

### 11.2 Professional engineering plots

- [x] CSTUDIO-083 Specify and test direct Response, Experiment, Limits, Fit,
  Residuals, Validation data, and Poles and zeros views using real retained or
  calculated evidence. Empty views contain one explanation and one next action;
  no view fabricates demonstration data.
  Evidence: focused chart test `exposes every stage plot directly...` and the
  focused identification evidence tests are green (2026-08-31).
- [x] CSTUDIO-084 Specify and test Bode magnitude/phase, crossover and margins;
  root locus; complete Nyquist evidence; Nichols; loop sensitivity; and
  requirements/robustness/constraint views. Invalid or incomplete evidence is
  disabled or reported with the precise reason.
  Evidence: `npx tsc -p tsconfig.json && npx mocha --ui tdd
  out/test/suite/control-studio-identify-tune.test.js --grep "builds bounded
  current/candidate"` (green; 2026-08-31), plus direct chart host contract.
- [x] CSTUDIO-085 Specify and test MIMO channel-response grids, numerical RGA,
  singular values, and interaction views with explicit input/output pairing and
  unit-safe labels.
  Evidence: focused typed-MIMO and direct-workspace tests are green; the real
  Extension Development Host rendered named 2x2 channel responses and singular
  values in `direct-chart-full-2026-08-31/screenshots-raw/CSTUDIO-J09-mimo-*`.
  The rendered host-size defect was rejected at 680x0, fixed red-green, and the
  accepted plots render at 680x332 with explicit input/output names and units.
- [x] CSTUDIO-086 Specify and test stable trace identity, two-pixel primary
  traces, lighter secondary/model traces, gaps for missing samples, explicit
  provenance, event/limit annotations, hover-or-zoom markers, and nominal versus
  uncertainty envelopes.
  Evidence: focused chart tests for stable SP/PV/MV identity, missing-value
  gaps, extrema/quality/events, saturation transitions and output-limit lines,
  plus the engineering-evidence test for uncertainty (green; 2026-08-31).

### 11.3 Direct graphical design

- [x] CSTUDIO-087 Specify and test keyboard- and pointer-editable graphical
  candidate handles on the direct Bode, root-locus, and Nichols tabs. Every edit
  changes only the unapplied candidate and rejects invalid values without
  losing the last valid candidate.
  Evidence: focused direct-workspace tests cover the shared graphical handle and
  invalid-value boundary; the rendered journey focused the Bode plot, changed
  Kp, and retained an unapplied candidate (`CSTUDIO-J08-graphical-edit-dark.png`).
- [x] CSTUDIO-088 Specify and test one candidate-revision transaction that
  immediately and consistently updates frequency response, closed-loop step
  response, margins, sensitivity, constraints, and validation requirements.
  Never combine results from different revisions.
  Evidence: `keeps graphical edits undoable and pins only compatible designs`
  and the engineering-evidence tests are green; the rendered journey observed
  the candidate revision update before pin/undo/redo without changing the
  running controller.
- [x] CSTUDIO-089 Specify and test graphical requirement regions for overshoot,
  settling time, crossover/bandwidth, margins, damping, sensitivity, actuator
  effort/rate, process limits, and robustness. Each region carries a textual
  value, unit, boundary, and Pass, Fail, or Inconclusive result.
  Evidence: the focused engineering and validation suites build the bounded
  requirement evidence and retain status, value, limit, unit, source, and
  reason without coercing unsafe margins (green; 2026-08-31).
- [x] CSTUDIO-090 Specify and test current-versus-active comparison plus up to
  three directly pinned candidates. Reject incompatible model, evidence,
  requirement, or numerical-method comparisons instead of normalizing them
  silently.
  Evidence: focused pin compatibility test is green; rendered screenshots show
  Current, active Candidate, and Pinned design chips bound to the same evidence.
- [x] CSTUDIO-091 Specify and test bounded Undo/Redo for complete graphical
  candidate transactions. Undo/redo never changes the running controller and
  cannot cross loop, model, apply, or session boundaries.
  Evidence: focused chart test `keeps graphical edits undoable and pins only
  compatible designs` (green; 2026-08-31).

### 11.4 Session persistence

- [x] CSTUDIO-092 Specify and test Save session and reopen for active stage,
  selected direct plot per stage, signals/channel order, units and axes,
  cursor/zoom, expanded state, active candidate, pinned candidates, graphical
  requirements, and exact evidence revisions.
  Evidence: focused tests `restores one evidence-bound direct chart session`
  and `saves and restores the complete evidence-bound chart session` (green;
  2026-08-31).
- [x] CSTUDIO-093 Specify and test historical-session labelling, safe fallback
  for incompatible restored state, and proof that reopening never starts a
  runtime, applies a candidate, or changes the running controller.
  Evidence: the same session tests reject loop/model boundary changes; existing
  saved-result reopen tests prove historical labelling and non-applying restore
  behavior (green; 2026-08-31).

### 11.5 Visual, responsive, and accessible completion

- [x] CSTUDIO-094 Specify and test the shared 4/8-pixel spacing rhythm, thin
  separators, restrained VS Code-derived palette, absence of decorative cards,
  and visually dominant plot. Do not add local theme variables when a shared
  truST token exists.
  Evidence: accepted dark/light/high-contrast screenshots use VS Code variables,
  thin separators, restrained controls, and one dominant plot surface; see the
  direct-chart acceptance README and screenshot set.
- [x] CSTUDIO-095 Specify and test direct responsive behavior at wide, split,
  narrow, and 200 percent zoom: plot tabs may scroll, analytical panels stack,
  signals fill the width, the plot remains useful, and no essential safety or
  Abort action disappears.
  Evidence: the green rendered journey measured wide 1092/1092, split 720/720,
  narrow 520/520, and 200 percent 346/346 client/scroll widths. Current Bode
  heights are 359, 357, 412, and 358 px respectively. The stage bar, direct
  plot tabs, Bode selection, graphical gain editor, and chart remain visible at
  200 percent zoom without page overflow.
- [x] CSTUDIO-096 Specify and test smooth linked cursor and candidate updates
  within 120-160 ms where animation is useful, exact displayed values during
  motion, and disabled nonessential transitions under reduced motion.
  Evidence: graphical candidate dispatch uses one 140 ms transaction and a
  zero-delay reduced-motion path; focused chart/webview type checks are green
  (2026-08-31).
- [x] CSTUDIO-097 Specify and test complete keyboard operation, visible focus,
  accessible chart summaries, direct prerequisite explanations, graphical-handle
  alternatives, and accessible tabular data.
  Evidence: focused chart tests cover owned unit-aware summaries and tabular
  samples; the rendered journey exercised plot keyboard focus, graphical-handle
  keyboard input, physical-approval focus, direct prerequisite states, and
  screen-reader semantics in high contrast.

### 11.6 Verification and evidence

- [x] CSTUDIO-098 Register every new extension test and run
  `python3 scripts/check_vscode_test_registration.py`.
  Evidence: `python3 scripts/check_vscode_test_registration.py` -> 849 facts,
  72 files, 72 registrations (2026-08-31).
- [x] CSTUDIO-099 Run the focused red-green chart, workflow, graphical-design,
  MIMO, persistence, responsive, and accessibility tests; record the exact red
  and green commands beside the applicable rows above.
  Evidence: expected assertion reds were captured for direct navigation,
  duplicate chart surfaces, stage restore, identification views, persistent
  signal selection, MIMO evidence, shared graphical handles, stable Monitor
  landing, and hidden Tune controls. Green command: `npx tsc -p
  tsconfig.webview.json && npx tsc -p tsconfig.json && npx mocha --ui tdd
  out/test/suite/control-studio-chart.test.js
  out/test/suite/control-studio-identify-tune.test.js
  out/test/suite/control-studio-validate-apply.test.js` -> 82 passing before
  the final MIMO/visual corrections; the final combined remote rerun -> 87 passing
  (2026-08-31).
- [ ] CSTUDIO-100 Run VS Code lint, compile, and the complete extension suite on
  `trust-builder` after the implementation is frozen.
  Current evidence: remote lint and compile are green. The valid Xvfb complete
  suite run reached 761 passing and 86 failing; all failures are in the separate
  dirty curriculum README/schema work, so this row remains open. Log:
  `trust-builder:/tmp/trust-control-chart-final-npm-test-xvfb.log`.
- [x] CSTUDIO-101 Exercise the shipped extension with real interactions and
  capture current dark, light, and high-contrast screenshots at wide, split,
  narrow, and 200 percent zoom. Include empty, unavailable, MIMO, graphical
  tuning, linked update, pinned comparison, undo/redo, saved-session restore,
  and failure/recovery states.
  Evidence: `direct-chart-full-2026-08-31/README.md`; real Extension
  Development Host workflow and deterministic post-review chart lanes both
  reported `1 passing`, `CONTROL_STUDIO_SHELL_DONE`. The current
  screenshot set covers direct plots, Signals, graphical editing, MIMO, saved
  results, failure/recovery, dark/light/high-contrast, all required widths, and
  200 percent zoom. Invalid physical-desktop captures were discarded.
- [x] CSTUDIO-102 Obtain independent UX review by someone other than the
  implementer. Correct clipping, weak hierarchy, ambiguous controls, excessive
  density, poor contrast, or confusing recovery before marking `ux_accepted`.
  Evidence: Harvey independently rejected the initial captures, then accepted
  the final recaptures after visible no-data guidance, hidden-chart resizing,
  non-overlaying graphical controls, full-height split plots, and a direct Bode
  plot at 200 percent zoom were corrected. Review used
  `direct-chart-full-2026-08-31/screenshots-raw/` and
  `json/CSTUDIO-DIRECT-CHART-REVIEW.json` (2026-08-31).
- [ ] CSTUDIO-103 Run final remote `just fmt`, `just clippy`, and `just test-all`
  once for the frozen candidate and record any unavailable physical-hardware
  boundary honestly.

### 11.7 Direct chart visual polish

- [x] CSTUDIO-104 Specify one coordinated Bode figure, synchronized exact
  graphical editing, live candidate margins, restrained chart typography, and
  VS Code-derived scrollbar and toolbar chrome in Specification 35.
  Evidence: Specification 35 sections 5.5, 5.6, and 15 define observable
  behavior without introducing a catalogue, display mode, or parallel chart
  system (2026-08-31).
- [x] CSTUDIO-105 Add and register focused tests proving that magnitude and
  phase share frequency navigation and inspection, use one legend, and remain
  a visually coordinated Bode figure.
  Evidence: the chart contract asserts the shared ECharts group, linked
  navigation, single legend, and aligned Bode layout; the remote focused suites
  reported 91 passing (2026-08-31).
- [x] CSTUDIO-106 Add and register focused tests proving that the Kp range and
  exact numeric input stay synchronized, reject invalid edits, retain the last
  valid candidate, and remain keyboard accessible.
  Evidence: the rendered audit exercised `1.44 -> 2.5 -> invalid 0 -> 1.44`;
  the invalid edit retained 2.5 and displayed its bounded accessible error.
- [x] CSTUDIO-107 Add and register focused tests proving that visible gain and
  phase margins update from the same engineering-evidence transaction as the
  active candidate.
  Evidence: the rendered candidate transaction displayed `GM 12.4 dB` and
  `PM 58°` beside the exact graphical editor.
- [x] CSTUDIO-108 Implement the minimum shared-chart and webview changes needed
  for CSTUDIO-105 through CSTUDIO-107. Preserve one direct toolbar and one
  candidate editing row.
  Evidence: `panel.ts`, `chart.ts`, and `chartContract.ts`; no second chart
  surface, catalogue, or display mode was introduced.
- [x] CSTUDIO-109 Apply responsive tick-density, overlap suppression, trace and
  grid hierarchy, thin VS Code-derived scrollbars, and compact toolbar
  separators. Verify no essential plot or safety action is hidden.
  Evidence: every plot tab and chart action is directly visible at wide,
  split, narrow, high contrast, and 200 percent zoom; Bode remains 382 pixels
  high at 200 percent zoom.
- [x] CSTUDIO-110 Run the focused tests red before implementation and green
  afterward, then run TypeScript compile, lint, and VS Code test-registration
  validation on `trust-builder`.
  Evidence: rendered reds recorded hidden 200-percent controls and incorrect
  Bode export. Current rendered audit is green; remote lint and compile are
  green; registration validated 853 facts in 72 registered files; focused
  suites reported 91 passing.
- [x] CSTUDIO-111 Exercise the shipped extension and capture current Bode and
  graphical-edit evidence at wide, split, narrow, 200 percent zoom, dark,
  light, and high contrast. Inspect real cursor linkage, exact gain entry,
  margins, keyboard operation, zoom, fit, and recovery from an invalid value.
  Evidence: `all-controls-final-2026-08-31/` pressed all five modes, every
  Tune plot, Inspect, both zoom actions, Fit, Expand/Restore, and Export;
  selected every option in all eight menus; opened and closed all twelve
  disclosures; and restored their original state. Bode exported as
  `control-studio-bode.png`; the same run proves a non-empty 1016 by 387 MIMO
  grid.
- [x] CSTUDIO-112 Obtain independent visual review of the current screenshots.
  Correct weak hierarchy, clipping, overlap, excessive chrome, illegible axes,
  or inconsistent theme behavior before marking this row accepted.
  Evidence: Harvey independently reviewed every current viewport/theme capture
  and the pixel-verified MIMO grid, then explicitly ACCEPTED readability,
  hierarchy, chart quality, discoverability, responsiveness, and contrast
  without a remaining blocker (2026-08-31).

## Completion

- [x] CSTUDIO-DONE-001 Specification, executable tests, implementation, and
  rendered behavior agree.
- [x] CSTUDIO-DONE-002 Monitor, Test, Identify, Tune, Validate, and Automatic
  Tune work through the existing runtime and VS Code systems.
- [x] CSTUDIO-DONE-003 No candidate is silently applied, physical actions are
  explicitly approved, and simulation evidence is never presented as physical.
- [x] CSTUDIO-DONE-004 The five final journeys and final remote gates are green,
  with unavailable physical hardware reported honestly.
- [x] CSTUDIO-DONE-005 The direct professional chart workspace in Section 11 is
  implemented, tested, rendered, independently accepted, and agrees with
  Specification 35 without introducing catalogues, modes, or parallel systems.

### Open-loop recording display repair — 12 September 2026

- [x] Use explicit loop topology for consistent applied-input/measured-output labels in plots, Signals, the value strip and accessible descriptions; omit the nonexistent SP and undeclared controller mode.
- [x] Forward retained native samples through the existing scoped Test/manual-record messages. Recording-only Validate displays that immutable record before Save/Open; Monitor resumes its live sample window. Preserve saved-result identity and qualification limits.
- [x] Prevent duplicate rounded labels at padded measured-axis endpoints while preserving actuator-limit labels and original samples.
- [x] Verify the native short open-loop journey, personally and independently inspect all10 current images, and run the affected637-case extension regression. The existing actual-PID recovery journey also passes23.446s.
- [x] SOLID/KISS/DRY: reuse the shared chart adapter, signal projection and recording owners; update and render the ownership diagram without adding runtime-write paths.

Evidence: [current image review](/home/johannes/projects/.artifacts/control-platform-release-20260911/open-loop-display-final-self-review.md), `open-loop-display-final.log`, `open-loop-final-regression.log` and `open-loop-display-diagrams.log`. The1.1s UI scenario is distinct from the retained full30s Lesson12 numerical experiment. Release-level theme/layout, tutorial and user-acceptance gates remain open.

- [x] Recording-only Validate exposes direct Signals and Recorded Response views over the same immutable native data. Paired process axes name both signals; the legend lists selected traces only, and actual legend clicks ignore absent diagnostic keys. Expected native/SVG assertion reds precede fixes. Lesson13 native Test/Save/Open and all four signal-group plots pass44.866s; the short native navigation/legend interaction check passes15.543s. [Execution and reviewed images](/home/johannes/projects/.artifacts/control-platform-release-20260911/lesson13-self-review.md). SOLID/KISS/DRY: existing chart, selection, recording and result owners; no parallel plant, controller, storage or runtime-write service. Whole-platform and full-editor persistence gates remain open.

- [x] Project-scripted experiment parameters use the existing symbolic review/application owner in Test, with focused settings and review locking. Optional baseline recording uses the native timed trajectory before activation. Per-session lifecycle I/O rejects delayed/foreign observations. Expected focused/rendered/native reds precede fixes; 648 affected checks, native full experiment, short UI and actual Control.PID recovery pass. [Bounded proof and all current image reviews](/home/johannes/projects/.artifacts/control-platform-release-20260911/lesson14-self-review.md). SOLID/KISS/DRY: existing parameter, lifecycle, trajectory, recording and result owners; no second controller or transport. Full persistence and release gates remain open.

- [x] Import an existing native recording through a scoped file boundary, preserve its original versioned acquisition identity and all raw samples, qualify/preview through existing numerical owners, and Save/Open under the active loop without adopting a foreign controller or model. The real303-row Lesson14→15 journey passes12.305s. Recorded Response precedes fitting; Fit selects the preview; one primary action and wide expandable source details pass all8 current personal/independent image reviews. [Evidence and limitations](/home/johannes/projects/.artifacts/control-platform-release-20260911/lesson15-self-review.md).
- [x] Explicit Monitor navigation resumes active observations while restored saved-view navigation remains historical; duplicate historical connection text is removed. Actual Control.PID recovery still passes24.051s. SOLID/KISS/DRY: reuse one record decoder/snapshot, chart workspace, qualification/fitting owner and host composition boundary; no new runtime writer. Full persistence, invalid-import matrix, lesson uncertainty and prior-Test notice clarity remain open.
