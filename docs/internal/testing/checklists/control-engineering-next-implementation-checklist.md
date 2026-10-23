# Control engineering next implementation checklist

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


Status: implementation complete; external novice observation remains open

This checklist implements the next program recommended by the Control System
Engineering report. Work shall proceed in this dependency order:

1. persist complete Control Studio run results through the existing result
   repository and export path;
2. convert the proven benchmark cases into executable training projects; and
3. qualify and compare multiple tuning methods using the same saved evidence.

The work shall reuse the existing `Control.PID`, simulation controller,
process-image communication, Control Studio workflow, result repository, chart,
and export implementation. It shall not create a parallel PID, simulator,
communication path, artifact database, SHA/content-addressed system, or second
Control Studio.

Product authority:

- `docs/specs/45-control-pid.md`
- `docs/specs/46-simulation-plant-models.md`
- `docs/specs/35-vscode-control-studio.md`
- `docs/specs/36-control-pid-performance-evaluation.md`
- `docs/guides/CONTROL_PID_ENGINEERING_REPORT.md`

## 0. Recovery and scope lock

- [x] `CENEXT-000` Recover and report the canonical checkout, working checkout,
  branch, HEAD, dirty state, active checklist row, and existing uncommitted PID,
  simulator, Control Studio, documentation, and test work.
- [x] `CENEXT-001` Verify the working `AGENTS.md` and complete `.codex/skills`
  tree exactly match the canonical checkout before editing or testing.
- [x] `CENEXT-002` Read Specifications 32, 33, 35, and 36 completely and inspect
  the existing saved-result repository, CSV/JSON export, experiment recording,
  simulation trace, validation result, and chart data paths.
- [x] `CENEXT-003` Produce a short ownership map showing which existing type and
  module owns recording, run result, export, persistence, tuning candidate, and
  rendered history. Resolve duplication before adding fields.
- [x] `CENEXT-004` Confirm that `Control.PID` remains the
  unchanged execution baseline. Any proposed equation change requires its own
  later specification and native expected-red test.
- [x] `CENEXT-005` Record the staged boundary: Phase 1 must be accepted before
  training-project generation begins, and Phase 2 must be accepted before
  tuning-method implementation begins.

Current ownership map:

- synchronized recording: `ControlLoopMonitor` in `loopMonitor.ts`;
- saved-run schema, validation, revision history, JSON, and CSV:
  `workflowResult.ts`;
- workspace persistence and rendered result commands: `panel.ts`;
- identified model and fit: `identification.ts`;
- controller candidate and paper-equivalent prediction adapter:
  `controllerCandidate.ts`;
- tuning and frequency evidence: `tuning.ts` and `engineeringPlots.ts`;
- validation requirements and metrics: `validation.ts`;
- physical execution, authorization, and restoration: the existing runtime
  control client and `experiment.ts`/`controllerApplication.ts` adapters.

Recovery evidence: canonical `/home/johannes/projects/trust-platform`, working
checkout `/home/johannes/projects/trust-platform-pid-core-spec`, branch
`feat/trust-control-pid-core-spec`, HEAD
`e4a4dec94bdb75922eb5b8e22d308a9454247add`; the working checkout was dirty
with the existing PID, plant simulation, Control Studio, test, and documentation
work and was preserved. `AGENTS.md` and `.codex/skills` matched the canonical
checkout before edits. Remote focused validation uses
`trust-builder:/home/johannes/projects/trust-platform-control-next-validation-v2`
at the same branch and HEAD with agent files copied and verified before tests.

Focused evidence recorded on `trust-builder` in that validation checkout:

- result revision history: `./node_modules/.bin/tsc -p ./ --pretty false &&
  ./node_modules/.bin/mocha --ui tdd
  out/test/suite/control-studio-validate-apply.test.js --grep "appends immutable
  bounded saved-result revisions"`; expected red was `saved-result revision
  history is not implemented`, then `1 passing` green;
- evidence consistency: the same command with `--grep "rejects mixed evidence
  sources and non-monotonic saved samples"`; expected red accepted a Physical
  sample in a Simulation result, then `1 passing` green;
- complete record: the same command with `--grep "creates a complete
  reproducible saved-run record"`; expected red was schema `1 !== 2`, then
  `1 passing` green;
- ordered history: the same command with `--grep "decodes only one ordered
  saved-result history"`; expected red was `saved-result history decoding is
  not implemented`, then `1 passing` green; and
- complete CSV: the same command with `--grep "exports the complete
  reproducible signal table as CSV"`; expected red exposed the old four-column
  header, then `1 passing` green.

## 1. Complete run-result persistence

### 1.1 Specification

- [x] `CENEXT-010` Extend Specification 35 Section 13 instead of defining a new
  persistence subsystem. Define one versioned saved-run result that the current
  Control Studio repository can save, list, open, export, and compare.
- [x] `CENEXT-011` Define the minimum reproducible identity: result revision,
  project, loop, controller revision, creation time, evidence source, software
  version, and numerical-adapter version where applicable.
- [x] `CENEXT-012` Define the complete timestamped recording: SP, PV, MV, actual
  cycle interval, engineering units, quality/freshness, limits, operating mode,
  safety state, and event markers.
- [x] `CENEXT-013` Define the experiment context: requested and applied test
  plan, bounds, approval source, start and stop reason, abort, restoration, and
  restoration confirmation or failure.
- [x] `CENEXT-014` Define the plant context: simulation or physical source,
  plant configuration or physical topology reference, effective simulation
  seed, uncertainty, scheduled events, actuator behavior, and model-validity
  range.
- [x] `CENEXT-015` Define the engineering result: selected recording interval,
  identified model and qualification, current and candidate controller,
  tuning method and objective, validation requirements, measured metrics,
  model-predicted margins, uncertainty result, and conclusion.
- [x] `CENEXT-016` Preserve evidence classes. A saved simulation result shall
  not be relabelled as physical, and a calculated prediction shall not become
  an observed measurement after save, reopen, copy, or export.
- [x] `CENEXT-017` Specify raw-recording immutability. Saving a new selection,
  model, tuning, or validation creates a new result revision without rewriting
  the accepted source samples.
- [x] `CENEXT-018` Specify bounded storage, explicit replacement or deletion,
  atomic save, interrupted-save recovery, unknown-version rejection, and
  deterministic failure messages using the existing repository conventions.
- [x] `CENEXT-019` Keep JSON as the complete typed export and CSV as the selected
  signal-table export. PNG remains a presentation export and shall not be used
  as numerical evidence.
- [x] `CENEXT-020` Explicitly exclude a general artifact database,
  content-addressed dependency graph, hash/SHA scheme, pagination framework,
  background synchronization service, and migration framework from this slice.

### 1.2 Tests before implementation

- [x] `CENEXT-021` Add the smallest native serialization round-trip test for a
  complete simulated run result and obtain an expected assertion failure before
  implementing missing fields or persistence behavior.
- [x] `CENEXT-022` Add native rejection tests for missing version, unknown
  version, nonfinite samples, inconsistent vector lengths, non-monotonic time,
  invalid units, and incomplete source identity. Rejection shall not partially
  save a result.
- [x] `CENEXT-023` Add a native test proving a new analysis revision leaves raw
  samples byte-for-byte equivalent at the typed-value level without requiring
  file hashes.
- [x] `CENEXT-024` Add a native test proving simulation evidence cannot be
  loaded, copied, or exported as physical evidence.
- [x] `CENEXT-025` Add native JSON and CSV export tests covering actual cycle
  time, units, quality, events, limits, source, and enough numeric precision to
  reproduce every calculated metric.
- [x] `CENEXT-026` Add interrupted-save and invalid-existing-file tests proving
  the last valid result remains readable and the failure is explicit.
- [x] `CENEXT-027` Add VS Code extension tests for save, list, reopen, compare,
  JSON export, CSV export, empty state, invalid record, and recovery. Register
  every new test in the real suite index.
- [x] `CENEXT-028` Add rendered interaction tests proving the reopened run shows
  the correct source, revision, units, SP/PV/MV, limits, quality, metrics, model,
  candidate, and validation conclusion.

### 1.3 Minimal implementation

- [x] `CENEXT-029` Extend the existing saved-result type and repository only
  after the focused expected-red tests are valid.
- [x] `CENEXT-030` Reuse the existing runtime and Control Studio recording path;
  do not add a second trace collector or controller equation.
- [x] `CENEXT-031` Reuse the existing chart-series representation or introduce
  one shared typed adapter if the runtime recording and chart currently
  duplicate sample conversion.
- [x] `CENEXT-032` Save atomically through the existing project-result location
  and expose compact result history through the existing Control Studio shell.
- [x] `CENEXT-033` Keep the main control plot dominant. Result history shall be
  a compact selector; metadata and advanced fields remain progressively
  disclosed.
- [x] `CENEXT-034` Show a concise unavailable reason when a result cannot be
  opened. Never invent samples, units, quality, margins, or conclusions.
- [x] `CENEXT-035` Update architecture documentation and diagrams only if the
  audit proves ownership or data flow changed; do not create diagram churn for
  field additions within the existing owner.

### 1.4 Phase 1 acceptance

- [x] `CENEXT-036` Run focused native red-green tests continuously and record
  the exact red and green command for every changed behavior.
- [x] `CENEXT-037` Run VS Code registration validation, lint, compile, complete
  extension tests, and the applicable runtime vertical tests on
  `trust-builder`.
- [x] `CENEXT-038` In a real Extension Development Host, record a simulated
  benchmark, save it, close Control Studio, reopen the exact result, compare it
  with another revision, and export JSON and CSV.
- [x] `CENEXT-039` Capture current dark, light, and high-contrast rendered
  evidence at wide and narrow layouts. Verify keyboard operation, focus,
  screen-reader labels, long units, missing data, invalid record, and recovery.
- [x] `CENEXT-040` Obtain independent UX acceptance that result persistence is
  understandable and does not turn Control Studio into an artifact-management
  dashboard.
- [x] `CENEXT-041` Do not begin Phase 2 until one complete saved result can
  reproduce its displayed metrics and plots after reopening.

Phase 1 acceptance evidence (2026-08-26):

- focused bundle: `ssh trust-builder 'cd
  "$HOME/projects/trust-platform-control-next-validation-v2/editors/vscode" &&
  ./node_modules/.bin/tsc -p ./ && ./node_modules/.bin/mocha --ui tdd
  out/test/suite/control-studio-validate-apply.test.js
  out/test/suite/control-studio-chart.test.js
  out/test/suite/control-studio-loop-monitor.test.js'` — `42 passing`;
- registration: `python3 scripts/check_vscode_test_registration.py` — `622`
  discovered facts, `60` files, `60` registrations;
- extension milestone: `npm run lint` and
  `xvfb-run -a -s "-screen 0 1920x1080x24" npm test` on `trust-builder` —
  lint green and `622 passing`; the first `npm test` attempt without Xvfb
  failed before tests because Electron had no display and is not counted as a
  product red;
- rendered expected red: the existing Control Studio shell journey reopened
  the correct saved result but initially exposed an incomplete comparison and
  absolute export path; after the compact comparison/export fix, the same
  command ended `1 passing`, exit `0`, `CONTROL_STUDIO_SHELL_DONE`;
- current evidence:
  `docs/internal/testing/evidence/vscode-control-studio/persistent-results-2026-08-26/`,
  including `CSTUDIO-J06-saved-results-dark.png` and
  `CSTUDIO-PERSISTENCE-diagnostic.json`; and
- independent reviewer acceptance: both revisions identify source, method,
  controller revision, and creation time; the shared conclusion is shown once;
  export status is compact; the historical plot remains dominant; and the
  persistence controls remain an integrated disclosure rather than an
  artifact-management dashboard.

## 2. Executable control-engineering training projects

### 2.1 Shared lesson contract

- [x] `CENEXT-050` Specify one reusable lesson layout containing a runnable
  truST project, controller ST, `simulation.toml`, README, objective, expected
  metrics, native test, learner questions, and advanced exercise.
- [x] `CENEXT-051` Reuse the exact benchmark plant configurations and acceptance
  definitions from Specification 36. Do not maintain separate undocumented
  teaching equations or expected values.
- [x] `CENEXT-052` Make every lesson follow Predict, Configure, Monitor, Test,
  Identify, Tune, Validate, Explain, and Export where those stages apply.
- [x] `CENEXT-053` Start each project with safe bounded simulation defaults and
  an explicit statement that its results are simulation evidence.
- [x] `CENEXT-054` Keep core controls immediately understandable. Explanations
  and advanced parameters shall use the existing help/progressive-disclosure
  pattern rather than permanent instructional panels.
- [x] `CENEXT-055` Require each lesson's documentation tables and plots to come
  from a saved run result or executable expected values, not manually copied
  numbers.

### 2.2 Foundation lessons

- [x] `CENEXT-056` Create the nominal FOPDT lesson: identify SP/PV/MV,
  observe offset removal, and explain Lambda PI robustness versus speed.
- [x] `CENEXT-057` Create the sampling lesson comparing `50`, `100`, and
  `200 ms`, including actual cycle-time effects.
- [x] `CENEXT-058` Create the disturbance-rejection lesson using the `0.2` plant
  bias event after settling.
- [x] `CENEXT-059` Create the saturation and anti-windup lesson comparing
  BackCalculation and Combined behavior.
- [x] `CENEXT-060` Create the measurement-noise lesson using the identical seed
  with raw and second-order filtered PV.
- [x] `CENEXT-061` Create the rate-limit lesson showing why actuator protection
  can determine achievable response speed.

### 2.3 Practitioner lessons

- [x] `CENEXT-062` Create the integrating-plant lesson comparing the rejected PI
  proposal with the accepted P controller and explaining why plant integration
  changes structure selection.
- [x] `CENEXT-063` Create the underdamped second-order lesson comparing PI and
  PID damping and measurement-noise sensitivity.
- [x] `CENEXT-064` Create the delay-dominant lesson showing the limit of
  increasingly aggressive PI tuning and the future predictor boundary.
- [x] `CENEXT-065` Create the nonlinear-actuator lesson enabling saturation,
  slew, lag, and deadband individually before combining them.
- [x] `CENEXT-066` Create the bounded plant-variation lesson using the four
  declared gain, time-constant, and delay cases.

### 2.4 Advanced lessons

- [x] `CENEXT-067` Create the open-loop unstable lesson comparing the three
  attempted PI tunings and distinguishing convergence from acceptable
  transient performance.
- [x] `CENEXT-068` Create the scheduled-gain lesson changing gain from `0.6` to
  `1.4` and explaining the future gain-scheduling boundary.
- [x] `CENEXT-069` Create the severe-jitter lesson using the repeated
  `20/180/50/150/100 ms` sequence and actual `CycleTime`.
- [x] `CENEXT-070` Create the coupled two-loop lesson with individual and
  simultaneous setpoint changes and an introduction to decoupling.
- [x] `CENEXT-071` Create the evidence-integrity lesson demonstrating why
  `%IL0` and `%IL1` overlap for `LREAL`, then correcting the second address to
  `%IL8`.

### 2.5 Training acceptance

- [x] `CENEXT-072` Register every lesson in the existing example catalog and
  public learning paths without introducing a separate tutorial browser.
- [x] `CENEXT-073` Give every lesson one native executable expected-result test
  and run it through the normal example/test runner.

Phase 2 implementation evidence (2026-08-26):

- expected red: the registered `control-engineering-fixtures.test.ts` lesson
  contract failed at `control-nominal-fopdt` because no lesson catalog entry
  existed; the same focused command is now `1 passing` for all sixteen;
- every catalog project passed `trust-dev test --project <lesson> --output
  json`, proving its ordinary Structured Text project compiles through the
  native project runner;
- every project passed `trust-runtime check --project <lesson> --ci`, then was
  built and kept alive in `trust-runtime run --project <lesson> --simulation`
  until the intentional two-second timeout, proving its actual simulation
  configuration activates rather than merely parsing as text;
- `cargo test -p trust-runtime --test control_pid_performance` — `12 passed`;
  `cargo test -p trust-runtime --test control_pid_mimo_performance` — `1
  passed`; and `cargo test -p trust-runtime --lib
  simulation_process_image_ranges_detect_lreal_overlap` — `1 passed`;
- the first incorrectly scoped overlap command built all runtime test targets,
  exhausted the isolated target disk, and never reached an assertion. The
  generated target was removed, disk space restored, and the `--lib` command
  above is the valid green evidence; and
- the existing example gallery contains all sixteen projects and the public
  learning path names all sixteen without introducing another tutorial
  browser.
- [x] `CENEXT-074` Exercise the complete foundation, practitioner, and advanced
  paths in Control Studio and verify that saved results reopen and export.
- [ ] `CENEXT-075` Have one learner unfamiliar with control theory and one
  experienced controls reviewer independently complete representative lessons.
- [x] `CENEXT-076` Record confusing terminology, unnecessary parameters, hidden
  safety information, and dead ends; correct the lesson and UI before
  acceptance.
- [x] `CENEXT-077` Do not begin Phase 3 until lesson results are generated from
  the same run-result and metric implementation used by ordinary Control
  Studio workflows.

Phase 2 rendered evidence (2026-08-26):

- expected red: the first real Extension Development Host lesson launch failed
  because the catalog launch file named missing `src/config.st`; the registered
  lesson contract was strengthened and failed at `control-nominal-fopdt`, then
  all sixteen launch files were corrected to their shipped `src/Main.st` and
  the same focused command passed;
- the existing `control-studio-shell-runner.js` was parameterized, not
  duplicated, and ran `control-nominal-fopdt`, `control-bounded-variation`, and
  `control-scheduled-gain` as ordinary Foundation, Practitioner, and Advanced
  projects in the shipped extension;
- all three journeys completed Monitor, bounded Test, Identify, Tune, Validate,
  two saves, close/reopen, revision comparison, JSON export, and CSV export;
  the practitioner and advanced cases retained explicit validation failure and
  refused application rather than weakening requirements;
- each command ended `1 passing`, exit `0`, `CONTROL_STUDIO_SHELL_DONE`; current
  screenshots and typed diagnostics are under
  `docs/internal/testing/evidence/vscode-control-studio/lessons-2026-08-26/`;
  every exported result is schema version 2 and contains the ordinary Control
  Studio samples, model, candidate, validation requirements, plant context,
  evidence class, and conclusion; and
- visual inspection confirmed the main plot remains dominant, the safety
  envelope and Abort remain visible during tests, result history remains a
  compact disclosure, and wide, split, narrow, 200 percent zoom, dark, light,
  and high-contrast assertions have no page-level overflow.
- exact reruns replaced the early timing shortcuts with the declared `20 s`,
  `30 s`, and `45 s` lesson durations under `lessons-2026-08-26/*-v2/`;
  four bounded-variation cases also passed as real extension journeys; and
- independent re-review accepted the corrected terminology, progressive
  disclosure, visible safety state, responsive layouts, and absence of dead
  ends. `CENEXT-075` remains open because no automated or agent review can
  substitute for an observed novice session.

## 3. Qualified tuning-method comparison

### 3.1 Method authority and numerical boundary

- [x] `CENEXT-080` Audit existing identification, tuning, validation, Bode, and
  numerical-library code before selecting dependencies or adding algorithms.
- [x] `CENEXT-081` Specify each supported method's plant domain, controller
  structures, formula or algorithm version, parameter convention, objective,
  constraints, invalid inputs, and deterministic failure behavior.
- [x] `CENEXT-082` Qualify Lambda/IMC or SIMC-style tuning first because it
  provides an understandable robustness-speed parameter and matches the paper
  benchmark's teaching model.
- [x] `CENEXT-083` Qualify an AMIGO-style industrial comparison second, limited
  to the model families and controller structures supported by its references.
- [x] `CENEXT-084` Add constrained optimization only after the formula-based
  methods are qualified. Its objective shall include performance, robustness,
  MV movement, limits, noise, and uncertainty rather than IAE alone.
- [x] `CENEXT-085` Keep third-party numerical types behind one truST-owned
  adapter. Pin dependencies and qualify adopted algorithms against independent
  references and declared tolerances.
- [x] `CENEXT-086` Preserve dimensioned `Control.PID` gains and actual sample
  interval semantics. Reject silent conversion between parallel, ideal,
  series, positional, incremental, or vendor-specific parameter conventions.

### 3.2 Tests before implementation

- [x] `CENEXT-087` Write independent published-reference oracle cases for every
  method before implementing its production adapter.
- [x] `CENEXT-088` Add rejection cases for invalid, singular, unstable,
  insufficiently excited, delay-dominated outside-domain, noisy, and
  nonfinite inputs.
- [x] `CENEXT-089` Add comparison tests proving all methods consume the same
  accepted model, units, constraints, sample interval, and objective.
- [x] `CENEXT-090` Add candidate-execution tests using the actual Structured
  Text `Control.PID` and simulator, not a duplicate tuning-side PID equation.
- [x] `CENEXT-091` Add deterministic ranking tests: infeasible candidates are
  rejected first; feasible candidates are ranked by the declared objective;
  ties have a stable documented rule.
- [x] `CENEXT-092` Add structure-selection tests for P, PI, PD, and PID,
  including the integrating example where P shall remain eligible and
  unnecessary I action shall be penalized or rejected.

### 3.3 Product workflow

- [x] `CENEXT-093` Present a compact method choice and More robust, Balanced,
  or Faster objective. Keep exact method parameters under advanced controls.
- [x] `CENEXT-094` Compare current and candidate responses, IAE/ISE/ITAE where
  applicable, MV peak/rate/movement, saturation, noise, margins, and bounded
  uncertainty using the existing shared plots and metric formatters.
- [x] `CENEXT-095` Explain the recommended structure, winning method, rejected
  alternatives, limiting constraint, uncertainty, and known model boundary in
  plain engineering language.
- [x] `CENEXT-096` Save every comparison in the Phase 1 run-result format so the
  exact model, method, candidate, traces, metrics, and conclusion reopen later.
- [x] `CENEXT-097` Keep generated candidates unapplied. Automatic Tune may
  recommend but shall never silently apply gains.
- [x] `CENEXT-098` Require the existing separate review and apply action,
  revision check, bounded observation, and rollback. Physical execution still
  requires reviewed topology and immediate approval.

### 3.4 Tuning acceptance

- [x] `CENEXT-099` Run every qualified method across the complete paper,
  representative, and advanced benchmark catalog and retain all failures.
- [x] `CENEXT-100` Prove at least one useful, explainable tradeoff relative to
  the current tuning without weakening stability, uncertainty, or actuator
  limits. Do not claim a universal winner.
- [x] `CENEXT-101` Exercise learner, professional, failure, responsive, and
  accessibility journeys against the shipped VS Code extension.
- [x] `CENEXT-102` Obtain independent numerical review and independent UI/UX
  acceptance before marking tuning comparison complete.

Phase 3 implementation evidence (2026-08-26):

- formula authority and domains are specified in Specification 35 Sections
  9.1-9.2 and Specification 36. No numerical dependency was added: the small
  formulas and bounded candidate grid remain behind the existing truST-owned
  `tuning.ts`/`controllerCandidate.ts` boundary;
- independent-oracle expected red: `control-studio-identify-tune.test.ts`
  initially reported the old SIMC result outside the published constants;
  after implementation, the consolidated focused command for identification,
  validation, and the sixteen-case campaign reports `57 passing`;
- the same focused suite records expected assertion reds and paired greens for
  AMIGO objective invariance, invalid method domains, proportional structure
  eligibility, complete metric retention, deterministic feasible-first
  ranking, rejected-candidate retention, automatic comparison attempts,
  saved P/constrained candidates, compact disclosure, saved comparison status,
  shared running-controller limits, and validated SIMC lambda persistence;
- `cargo test -p trust-runtime --test control_pid_performance
  qualified_formula_candidates_execute_through_public_control_pid --
  --nocapture` runs the independent SIMC and AMIGO constants through the actual
  Structured Text `Control.PID` and simulated process image: SIMC settled in
  about `3.9 s` with IAE about `1.40`; AMIGO settled in about `6.8 s` with IAE
  about `2.16`; both were stable with zero measured overshoot;
- `control-engineering-fixtures.test.ts` evaluates both qualified formula
  methods across all sixteen catalog projects and retains every unavailable
  domain reason (`32` total method attempts); and
- the shipped Extension Development Host journey ended `1 passing`, exit `0`,
  `CONTROL_STUDIO_SHELL_DONE`. It exercised the compact comparison, published
  lambda, deterministic rejection reasons, save/reopen/export, failure and
  recovery, keyboard use, dark/light/high-contrast themes, wide/split/narrow
  layouts, and 200 percent zoom. Current evidence is under
  `docs/internal/testing/evidence/vscode-control-studio/tuning-comparison-2026-08-26/nominal/`.
- independent review accepted the numerical and UI result after requiring the
  published-oracle tolerance and explicit `Ki = Kp / Ti` checks, bounded-model
  worst-case IAE in robustness scoring, the ten-percent material-benefit rule
  for added controller complexity, preservation of the running controller's
  limits and units, and truthful exact-simulation-model evidence. The corrected
  rendered result exposes the complete candidate table without stale results,
  labels controller settings in `%`, `%/s`, and `% RMS`, and passed the same
  narrow, zoom, theme, keyboard, save, reopen, and export journey.

## 4. Final integration and handoff

- [x] `CENEXT-110` Run all focused red-green tests recorded above and preserve
  exact commands and outcomes in the checklist evidence.
- [x] `CENEXT-111` Run VS Code test registration validation and all applicable
  runtime vertical, simulation, extension, rendered interaction, documentation,
  diagram, and drift checks.
- [x] `CENEXT-112` Freeze the final candidate, perform the required remote disk
  preflight, then run `just fmt`, `just clippy`, and `just test-all` once on
  `trust-builder`.
- [x] `CENEXT-113` Capture current screenshots for the complete saved-result,
  lesson, method-comparison, automatic-tune, failure, narrow, zoom, theme, and
  keyboard journeys.
- [x] `CENEXT-114` Update specifications, architecture documentation only where
  ownership changed, public guides, lesson catalog, changelog, and checklist
  evidence so they agree with shipped behavior.
- [x] `CENEXT-115` Document remaining external boundaries: unavailable physical
  hardware, unqualified plant classes, unsupported tuning domains, stronger
  MIMO coupling, and proposed advanced-control components.
- [x] `CENEXT-116` Do not push, merge, tag, release, operate a physical plant,
  or perform another external or safety-relevant action without its separately
  required authorization.

## Completion condition

This checklist is complete only when:

- a complete run can be saved, reopened, compared, and exported through the
  existing Control Studio result path;
- all sixteen lessons are runnable projects with native assertions and
  evidence-backed explanations;
- each shipped tuning method has independent numerical qualification and runs
  the actual `Control.PID` through the benchmark catalog;
- automatic recommendations remain unapplied until explicit review and apply;
- specifications, tests, implementation, documentation, screenshots, and
  checklist evidence agree; and
- every applicable final gate is green or an exact external blocker remains
  openly unchecked.

Final integration evidence (2026-08-26):

- `python3 scripts/check_vscode_test_registration.py` passed with `633`
  discovered test facts across `60` registered suite files;
- `npm run lint` and `npm run compile` passed, the focused fixture secret scan
  passed after the shipped lesson tokens were made empty, and the complete real
  Extension Development Host suite passed with `633 passing`;
- the runtime vertical commands passed: `api_smoke` (`3`), `debug_control`
  (`20`), `complete_program` (`1`), `runtime_reliability` (`4`),
  `simulation_workflow` (`10`), and the focused scaled-clock test (`1`);
- `python3 scripts/check_diagram_drift.py` passed. This slice did not change
  architecture ownership or execution-flow topology, so no new diagram was
  introduced;
- the final remote gates passed in the isolated validation checkout:
  `just fmt`, `just clippy`, and
  `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 just test-all`. The first broad-test
  attempt was an invalid infrastructure result because generated link artifacts
  exhausted the builder disk; after stopping residual compilers, cleaning only
  generated targets/caches, and restoring `71 GiB` free, the single rerun
  completed with exit `0`;
- saved-result, lesson, comparison, automatic-tune, failure/recovery, narrow,
  200-percent zoom, theme, and keyboard screenshots are retained under
  `docs/internal/testing/evidence/vscode-control-studio/`; and
- Specification 35, Specification 36, the public Control Studio guide, the
  engineering report, changelog, lesson catalog, implementation, tests, and
  evidence agree. The report explicitly leaves physical commissioning,
  unqualified plant/method domains, stronger MIMO control, advanced-control
  components, and the independent novice observation as external or future
  boundaries. No physical plant, push, merge, tag, or release action occurred.
