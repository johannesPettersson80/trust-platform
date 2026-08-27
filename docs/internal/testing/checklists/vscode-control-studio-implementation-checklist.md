# VS Code Control Studio Implementation Checklist

Status: complete

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

## Completion

- [x] CSTUDIO-DONE-001 Specification, executable tests, implementation, and
  rendered behavior agree.
- [x] CSTUDIO-DONE-002 Monitor, Test, Identify, Tune, Validate, and Automatic
  Tune work through the existing runtime and VS Code systems.
- [x] CSTUDIO-DONE-003 No candidate is silently applied, physical actions are
  explicitly approved, and simulation evidence is never presented as physical.
- [x] CSTUDIO-DONE-004 The five final journeys and final remote gates are green,
  with unavailable physical hardware reported honestly.
