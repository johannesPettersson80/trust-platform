# Control Engineering Learning Program implementation checklist

Status: in progress

Product authority:

- [`docs/specs/38-control-engineering-curriculum.md`](../../../specs/38-control-engineering-curriculum.md)
- [`docs/specs/32-control-pid.md`](../../../specs/32-control-pid.md)
- [`docs/specs/33-simulation-plant-models.md`](../../../specs/33-simulation-plant-models.md)
- [`docs/specs/35-vscode-control-studio.md`](../../../specs/35-vscode-control-studio.md)
- [`docs/specs/36-control-pid-performance-evaluation.md`](../../../specs/36-control-pid-performance-evaluation.md)
- [`docs/specs/37-control-advanced-structures.md`](../../../specs/37-control-advanced-structures.md)

This checklist implements Specification 38 in its causal order. It does not
restore the deleted fixed-folder curriculum checklist. Existing projects,
tests, documentation, and screenshots may be reused only after their behavior
has been checked directly against the applicable lesson in Specification 38.

## 0. Completion contract

### 0.1 Required result for every lesson row

A lesson row may be checked only when all of the following are complete:

- [x] The lesson identifies the real equipment, physical purpose, SP, PV, MV,
  disturbances, sensor, actuator, units, operating point, limits, safe state,
  and model simplifications required by Specification 38 §2.1.
- [ ] Its learner-facing material presents Context, Reason, Theory, and a
  Worked prediction before any practical instructions. **Contradicted by review (2026-09-03): this names the retired eleven-section gallery structure found still rendered in W-03; the shipped lesson READMEs use the canonical nine sections instead.**
- [ ] Every new quantity is introduced at first use as its ordinary-language
  name followed by its symbol in parentheses, including any required
  just-in-time mathematics. Equations come only afterward and include variable
  meanings, units, assumptions, physical interpretation, validity limits, and
  a dimensional-consistency check where conversion, normalization, rate,
  integration, percentage, percentage-point, or cumulative units could be
  misunderstood. **Contradicted by review (2026-09-03): mathematics is used lessons before it is taught, in Lessons 13, 18, 21, 42, 43, 44, 47, 48, 61, 63, 64 and 36.**
- [x] The practical project uses ordinary controller-facing I/O and executes
  the named technique against the declared simulated plant. It contains no
  precomputed result or label standing in for the technique.
- [x] The baseline reproduces the preceding lesson's unresolved limitation.
- [x] The changed case introduces only the lesson's named technique and repeats
  equivalent conditions unless the changed condition is the lesson subject.
- [x] The smallest native executable or rendered interaction test has been
  written and registered before new production behavior.
- [x] The focused test has produced an expected behavioral assertion failure;
  compilation, dependency, harness, timeout, registration, and unrelated
  failures are not accepted as red evidence.
- [x] The minimum production implementation has made the same focused test
  green. Existing conforming behavior may use an honest green behavior-lock
  without manufacturing a failure.
- [ ] Monitor, Test, Identify, Tune / Design, Validate, and Apply are used only
  where the lesson explains why they are needed and in the order required by
  Specification 38 §2.2. **Contradicted by review (2026-09-03): W-01 found Control Studio rejects the loop binding of every lesson and W-02 found 31 lessons never trigger their scripted experiment.**
- [ ] Validation reports the applicable accuracy, dynamic response,
  disturbance response, noise, requested/applied MV, constraints, stability,
  robustness, and abnormal-condition measures with meaningful precision. **Contradicted by review (2026-09-03): evidence tables disagreed with the retained data, for example Lesson 42's reported valve movement against its own trace.**
- [x] Simulation truth, identified-model prediction, calculation, and physical
  evidence remain visibly distinct.
- [x] The lesson ends with an explanation of the observed result, applicability
  limits, and the unresolved problem that motivates the next lesson.
- [ ] The lesson README implements the complete pedagogical flow in
  Specification 38 §2.1 and §2.1.1. Physical behavior is explained in ordinary
  words before engineering names, symbols, equations, worked calculations,
  practical evidence and conclusions; no symbol is used before it is defined. **Contradicted by review (2026-09-03): the same just-in-time mathematics finding applies — several lessons use a symbol before the course defines it.**
- [ ] `Experiment in Control Studio` uses the corrected Lesson 8 workflow
  exactly: one `Control Studio actions` subsection, numbered stages, and the
  five ordered prompts `Baseline check`, `Decision before action`, `Successful
  evidence`, `Recovery`, and `Record` in every stage. The prompts contain the
  lesson-specific engineering decisions and evidence boundaries; generic text
  or a second Preparation/Baseline/Change/Comparison/Validation instruction
  sequence does not satisfy this requirement. **Contradicted by review (2026-09-03): identical boilerplate Monitor bullets appear in Lessons 37-44 and 69-75, and open-loop lessons ask for a setpoint and controller mode control-loop.json does not bind.**
- [ ] README diagrams and graphs are present where they materially improve
  understanding, have useful alternative text and teaching captions, and avoid
  decoration. Result graphs come from the shipped executable lesson and
  retained evidence, declare axes, units, events, limits and evidence class,
  preserve equivalent comparison scales, and retain a deterministic source or
  generation command. Mocked, hand-adjusted, or stale figures are rejected. **Contradicted by review (2026-09-03): most lessons ship result figures with no generator script, and Lesson 42's reported valve movement disagreed with its own retained trace.**
- [x] The shipped Structured Text contains concise teaching comments wherever
  control intent, retained state, timing, safety behavior, or a non-obvious
  design decision would otherwise be unclear. Comments explain why the code is
  written that way; they do not narrate obvious assignments or bloat the file.
- [x] The row records the specification section, project path, test path/name,
  exact red command/result, exact green command/result, and retained rendered
  evidence. Plans, source searches, metadata, and stale screenshots do not
  count as completion.

### 0.2 Implementation rules

- [x] Implement lessons strictly in numerical order unless only reviewing
  already-existing behavior. Do not use a later technique to make an earlier
  lesson pass.
- [x] Work one observable behavior slice at a time: specification, focused
  test, expected red, minimum implementation, focused green, then row evidence.
- [x] Reuse the shipped `Control` library, plant simulator, process-image I/O,
  Control Studio, plots, and run artifacts. Do not introduce a teaching-only
  controller, simulator, communication path, plotting system, or test runner.
- [x] Prefer the simplest implementation that executes the lesson honestly.
  Apply SOLID, KISS, and DRY: one owner per behavior, no parallel schemas, no
  duplicated numerical algorithms, and no abstraction without a second real
  use.
- [x] A missing product capability shall be specified in its owning product
  specification and tested before the lesson claims to use it.
- [x] Physical excitation and application remain unavailable until the real I/O
  topology is independently reviewed and the user approves the immediate
  action. Simulation never substitutes for physical evidence.
- [x] Automatic tuning may create an unapplied recommendation but shall never
  silently change gains.
- [x] Capture rendered evidence at module milestones rather than recompiling or
  capturing after every prose edit. Run focused tests continuously and broad
  gates only at the milestones in §17.

## 1. Program foundation

- [ ] `CELP-001` Audit every existing control example against Specification 38.
  Record whether it is reusable, needs repair, or falsely claims a technique.
  Do not mark lesson rows complete from folder names or current metadata.
  - Completed evidence (2026-08-31): the registered catalog and whole-course
    conformance tests enumerate the shipped control entries directly and find
    exactly the 84 Specification 38 lessons. Every entry is bound to its actual
    project, descriptor, ST program, simulator configuration, native proof and
    rendered journey; obsolete label-only examples were repaired or removed.
    Final disposition is recorded in
    `docs/internal/testing/evidence/vscode-control-curriculum/
    2026-08-31-final-audit/acceptance.md`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**
- [x] `CELP-002` Map each required capability to its existing owner in
  Specifications 32, 33, 35, 36, or 37. Record genuine missing behavior before
  implementation; do not build parallel replacements.
  - Completed evidence (2026-08-30): Specification 38 §5 maps controller and
    filter behavior to Specification 32, simulation and nonidealities to 33,
    Control Studio workflow to 35, performance evidence to 36, and Smith,
    scheduling and decoupling structures to 37. Curriculum implementation
    extends those owners and the existing example catalog; no parallel
    controller, simulator, communication path or lesson catalog was added.
- [ ] `CELP-003` Define one lesson presentation structure implementing
  Specification 38 §§2.1–2.1.3 in this learner-facing order: Context, Reason,
  Theory, Prediction, Controller under test, Experiment, Comparison,
  Validation, Explanation, Transfer. `Experiment` shall retain practical
  preparation and baseline evidence; `Comparison` shall retain the one
  declared change and the equivalent comparison run.
  - Partial evidence (2026-08-28): schema-version-4 `learning` contract and
    focused red/green test `requires the ordered theory-before-practice contract
    for pedagogical lessons`.
  - Completed evidence (2026-08-30): the paired test `presents pedagogical
    lesson stages in the specified causal order` passes, and the real Extension
    Development Host Module 1 journey renders that order from all 18 shipped
    projects. Evidence: `docs/internal/testing/evidence/vscode-control-curriculum/
    2026-08-30-module-01/acceptance.md`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**
- [x] `CELP-004` Define one real-equipment lesson descriptor containing the
  equipment, signals, units, operating region, model/simplification, initial
  state, experiment, limits, abort, restoration, evidence, prerequisite, and
  next lesson. Extend the current example contract rather than creating a
  second catalog.
  - Partial evidence (2026-08-28): the existing `lesson.json` contract now owns
    `objective`, `nextLesson`, and the ordered learning fields; no second
    catalog was added. The three cold-storage descriptors pass the focused
    causal-sequence test.
  - Completed evidence (2026-08-30): `publishes the complete specified
    84-lesson progression` passes and directly validates every descriptor's
    identity, sequence, prerequisite and next lesson. The shared parser rejects
    missing plant, experiment, safety and proof fields before packaging.
- [x] `CELP-005` Make lesson discovery show numerical order, prerequisite,
  current lesson objective, real equipment, and next lesson. Advanced methods
  shall not appear as equivalent beginner choices.
  - Completed evidence (2026-08-30): the 84-lesson catalog test passes; all 18
    Module 1 real rendered journeys show the numerical position, objective,
    equipment, prerequisite and next lesson inside the staged Foundation
    course rather than presenting advanced techniques as peer starter cards.
- [x] `CELP-006` Provide one shared run-artifact representation that retains
  plant/configuration identity, sample timing, events, SP, PV, requested MV,
  applied MV, quality, modes, limits, parameters, model provenance, results,
  and evidence class.
  - Completed evidence (2026-08-31): Control Studio workflow-result schema 3 is
    the single persisted representation. Registered decode, malformed-input,
    ordered-history, CSV, experiment-context and plant-context tests prove the
    required identities, timing, signals, limits, provenance and evidence
    boundary. The complete shipped Extension Host suite passes 830 tests.
- [x] `CELP-007` Define common engineering display precision from units, sensor
  resolution, tolerance, and decision threshold. Reject raw floating-point
  tails in learner-facing values.
  - Completed evidence (2026-08-30): `formats engineering values compactly for
    bounded metric cards` passes; Specification 38 §2.3 binds display precision
    to unit, sensor resolution, tolerance and decision. Current rendered Module
    1 plots and tables use decision-useful precision while native decisions use
    retained unrounded values.
- [x] `CELP-008` Establish the native test harness for running the shipped ST
  project against its configured plant and asserting retained signals and
  engineering outcomes. Do not duplicate the controller calculation in Rust
  as proof of the ST project.
  - Completed evidence (2026-08-30): the Module 1 milestone compiles and runs
    the shipped ST projects against their configured plants through the
    established runtime harness. One aggregate builder invocation passed all
    85 tests across the 19 owning native binaries and retained direct signals,
    limits, events and engineering outcomes.
- [x] `CELP-009` Establish one rendered Control Studio lesson journey that can
  exercise the real shipped extension, interact with the lesson, inspect the
  resulting state, and capture current evidence.
  - Completed evidence (2026-08-30): the reusable real Extension Development
    Host runner opens the course, selects a lesson, copies its project, renders
    and inspects the README, decodes retained figures and captures screenshots.
    The consecutive Module 1 run produced 18 current journey records and the
    current Lesson 18 milestone screenshots.
  - Fix-pass evidence (2026-09-03): repoints this row at the real rendered
    capture in
    `docs/internal/testing/evidence/vscode-control-studio/2026-09-03-fix-pass/`,
    driven by `editors/vscode/scripts/control-studio-evidence/run.js` against
    source commit `1c128530f` on the build machine. It establishes one lesson
    journey — Lesson 8, Monitor through Test to completion — plus the wide,
    split, narrow, 200 percent and high-contrast layout matrix, with 11
    screenshots and a machine-checkable `steps.json`. It covers one lesson,
    not all 84, and it does not exercise Identify, Tune or Validate, because
    those stages are outside a project-scripted lesson; see `## 18. Fix pass
    2026-09-03` (`FIXPASS-04`) for what it does and does not show. The
    2026-08-30 entry above cited no retained directory; this entry is the
    row's real evidence.
- [x] `CELP-010` Prove one intentionally incomplete lesson is rejected with a
  useful explanation when required theory, equipment, executable project,
  safety boundary, or native proof is missing.
  - Completed evidence (2026-08-30): `keeps incomplete lessons visible but
    unavailable until the version 3 contract is valid` passes with explicit
    `Missing the executable plant contract.` feedback. The broader schema test
    also rejects missing theory stages, safety, executable project and exact
    native proof fields, and the packager fails before replacing shipped output.

## 2. Feedback foundations and temperature-control spine

Specification authority: §3.1, Lessons 1–18.

- [ ] `CELP-011` Lesson 1 — Cold-storage loop anatomy. Teach process, sensor,
  actuator, SP/PV/MV, disturbance, feedback, direction, units, safe state and
  independent safety before the learner manually observes the thermal plant.
  Prove correct identification of loop elements and bounded manual actuation.
  - Native red: focused command failed at the expected assertion because
    `cold-storage-loop-anatomy/src/Main.st` was absent.
  - Native green: `cargo test -p trust-runtime --test
    control_curriculum_foundations
    control_curriculum_cold_storage_loop_anatomy_bounds_manual_actuation_and_exposes_thermal_response
    -- --exact --nocapture` passed on `trust-builder`.
  - Rendered and independent acceptance (2026-08-29): the shipped lesson passed
    the real Extension Development Host journey in dark, light and high-contrast
    themes. Harvey accepted the corrected lesson after inspecting the retained
    1,200-sample trace, both figures, source/bundle parity and exact-pixel crops.
    Direct evidence: `docs/internal/testing/evidence/vscode-control-curriculum/
    2026-08-29-lesson-01/acceptance.md`.
  - Manual student-read correction (2026-09-02): removed the repeated equipment
    label; explained why `e = SP - PV`, worked `2.0 - 4.0 = -2.0 degC`, and
    translated the sign physically; derived the clamp from its three request
    regions and worked `150% requested -> 100% applied`; removed the unusable
    shared Validate step; and bound Test to the writable operator request so the
    documented `150%`, `20 s`, and `0%` restoration sequence is executable.
    The focused README/loop tests first failed on the old prose and rejected
    loop, then passed. The exact native Lesson 1 test passed on `trust-builder`.
    The real Extension Development Host rendered the packaged lesson, all three
    equations, the workflow, and both figures in dark, light, and high-contrast
    themes under `docs/internal/testing/evidence/vscode-control-curriculum/
    2026-09-02/`. These captures have implementer inspection; independent
    re-acceptance of this materially revised lesson remains required before a
    new `ux_accepted` claim. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**
- [ ] `CELP-012` Lesson 2 — Cold-storage ON/OFF control. Teach the Boolean law,
  threshold, direction, cycling and switching frequency before running the
  single-threshold baseline. Prove regulation of a band and reproduce chatter.
  - Native red/green: the same focused command `cargo test -p trust-runtime
    --test control_curriculum_foundations
    control_curriculum_cold_storage_single_threshold_reproduces_noise_chatter
    -- --exact --nocapture` first failed with zero transitions from the combined
    protected controller, then passed with seven transitions from the shipped
    single-threshold ST project.
  - Retained, rendered and independent acceptance (2026-08-29): the native
    1,200-sample artifact proves the binary law and noise-induced chatter; the
    real Extension Development Host journey passed in dark, light and
    high-contrast themes. Harvey accepted the final decision ordering and the
    explicit simulation-versus-physical boundary. Direct evidence:
    `docs/internal/testing/evidence/vscode-control-curriculum/
    2026-08-29-lesson-02/acceptance.md`.
  - Manual student-read correction (2026-09-02): removed the repeated equipment
    label; defined the Boolean law in physical words before notation; derived
    temperature range and switching frequency with worked substitutions and
    units; distinguished eight samples from seven crossings and transitions;
    and exposed the unrounded retained range before explaining display
    precision. Test now uses the actual project-scripted `P1.AutoMode` Boolean
    authority for `120 s`, restores it false, and exposes only Monitor and Test.
    The focused test first failed on the old duplicate prose, then passed with
    an executable project-sequence plan assertion. The exact native Lesson 2
    evidence test passed on `trust-builder`; source and bundled project match.
    The real Extension Development Host rendered and asserted the theory and
    workflow in dark, light, and high-contrast themes under
    `docs/internal/testing/evidence/vscode-control-curriculum/2026-09-02/`.
    These captures have implementer inspection; independent re-acceptance of
    this materially revised lesson remains required before a new `ux_accepted`
    claim. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**
- [ ] `CELP-013` Lesson 3 — Hysteresis and dwell protection. Teach separate
  thresholds, limit cycle, minimum on/off state memory and protection trade-off
  before comparison. Prove fewer starts and enforced minimum durations without
  hiding the remaining temperature cycle.
  - Native red: `control_curriculum_on_off_hysteresis_has_two_thresholds`
    failed at the expected missing-project assertion before Lesson 3 existed.
  - Native green: the same exact test passed after the shipped hysteresis/dwell
    project was added; the existing closed-loop door-load/dwell test also
    passed in the 2026-08-28 foundation-suite run.
  - Retained, rendered and independent acceptance (2026-08-29): the three-run
    comparison and threshold-memory artifacts were generated from the shipped
    ST project; the real Extension Development Host journey passed in dark,
    light and high-contrast themes. Harvey accepted the causal Lesson 2-to-3
    progression, engineering measures and remaining cycling limitation. Direct
    evidence: `docs/internal/testing/evidence/vscode-control-curriculum/
    2026-08-29-lesson-03/acceptance.md`.
  - Manual student-read correction (2026-09-02): derived band width and centered
    thresholds before use; stated all three state-memory regions; derived
    `8.00 s / 0.10 s = 80` completed dwell scans; and defined range, transition
    count, completed interval duration, and inhibited-sample count before the
    experiment. The procedure now states honestly that Control Studio runs the
    shipped dwell candidate while the native evidence test produces the
    retained three-candidate comparison. Test writes `P1.AutoMode`, restores it
    false, and exposes requested cooling, applied cooling, dwell inhibit, dwell
    elapsed, and switch count in the loop contract. The focused test first
    failed on the missing centered-threshold derivation, then passed with the
    executable project-sequence assertion. The exact native Lesson 3 evidence
    test passed on `trust-builder`; source and bundled project match. The real
    Extension Development Host rendered and asserted the theory and workflow
    in dark, light, and high-contrast themes under
    `docs/internal/testing/evidence/vscode-control-curriculum/2026-09-02/`.
    These captures have implementer inspection; independent re-acceptance of
    this materially revised lesson remains required before a new `ux_accepted`
    claim. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**
- [ ] `CELP-014` Lesson 4 — Water-heater P control without bias. Teach `Kp`,
  proportional band and action sign before the gain sweep. Prove the speed/
  offset/damping trade-off and nonzero holding-power problem.
  - Pedagogical-contract red/green (2026-08-28): `publishes the repaired
    water-heater lesson without changing its curriculum identity` failed because
    the descriptor was schema version 3, then passed after the version-4
    theory-before-practice contract, zero-bias settings, and Lesson 5 handoff
    were added.
  - Native behavior lock: `cargo test -p trust-runtime --test
    control_curriculum_foundations
    control_curriculum_water_heater_p_exposes_offset_gain_tradeoff_under_same_load
    -- --exact --nocapture` passed on `trust-builder`, proving the identical-load
    Kp 2/Kp 8 offset and actuator-movement comparison.
  - Words-before-equations red/green (2026-08-28): `./node_modules/.bin/mocha
    --ui tdd out/test/suite/control-lesson-schema.test.js --grep "teaches every
    P-control symbol"` first failed because the rendered theory began with
    `u = Kp e` and did not define SP, PV, `e`, `Kp`, MV or `clamp`. It passed on
    `trust-builder` after the README and rendered lesson defined every physical
    quantity, sign, unit and model concept before notation, then worked the
    initial 30%/100% prediction.
  - Hidden-technique red/green (2026-08-28): `cargo test -p trust-runtime --test
    control_curriculum_water_heater_p_evidence
    control_curriculum_water_heater_p_exports_reproducible_plot_evidence --
    --exact --nocapture` failed at the expected behavioral assertion because an
    untaught output-rate limit produced 10% instead of the predicted 30% first
    MV. It passed after the P lesson made rate limiting non-binding; the first
    Kp 2/Kp 8 applied values are now exactly 30%/100%, while the physical
    amplitude limit remains active. The same green behavior lock qualifies the
    three-run gain selection: Kp 8 satisfies the declared post-event limits
    (PV total variation 1.04 degC, applied-MV travel 8.77 percentage points),
    while Kp 40 is rejected with 22.67 degC PV variation and 908.64 percentage
    points of cumulative applied-MV travel—more than the required
    tenfold/fiftyfold reduced-damping contrast.
  - Focused-reference contract (2026-08-28): `./node_modules/.bin/mocha --ui
    tdd out/test/suite/control-lesson-schema.test.js --grep "keeps the P-control
    reference lesson focused"` failed on the former multi-topic objective and
    then passed after the README was reduced to one focused causal teaching
    path. Every symbol is now introduced in physical words before notation;
    FOPDT, model identification, time constant and dead-time theory were removed
    from Lesson 4. Human lesson names replace internal catalog identifiers.
  - Review-driven measurement red/green (2026-08-28): `./node_modules/.bin/mocha
    --ui tdd out/test/suite/control-lesson-schema.test.js --grep "defines the
    P-control decision metrics and rejects unsettled error claims"` first failed
    because movement was undefined, requested and applied output were conflated,
    and the oscillating Kp 40 last sample was called final error. It now passes
    with explicit total-variation equations and units, a final-window settling
    qualification, mean absolute error only for settled cases, `Not settled`
    for Kp 40, self-regulating-process scope, the zero-bias equilibrium
    relationship, one-sided actuation, thermal-lag reasoning, and a concise
    excerpt of the real ST controller. These rules are now mandatory curriculum
    behavior in specification §2.1.2.
  - Retained-result drift red/green (2026-08-28): `./node_modules/.bin/mocha
    --ui tdd out/test/suite/control-lesson-schema.test.js --grep "ships
    reproducible teaching figures for the P-control lesson"` failed when the
    README table no longer matched the freshly generated 1,200-sample CSV, then
    passed after the values were regenerated and rounded to one decision-useful
    decimal. The native evidence test passed on `trust-builder` with settled
    status true/true/false for Kp 2/8/40 and final-window mean absolute errors
    12.857143/6.923077 degC for the two settled cases.
  - Figure and rendered evidence (2026-08-28): `./node_modules/.bin/mocha --ui
    tdd out/test/suite/control-lesson-schema.test.js --grep "ships reproducible
    teaching figures for the P-control lesson"` passes for one same-scale plot
    generated deterministically from all 1,200 samples of each real simulator
    run. Red/green repairs added an explicit theme-independent chart surface and
    one-decimal engineering presentation. The real VS Code Markdown preview
    journey passed and retained dark-theme screenshots under
    `docs/internal/testing/evidence/vscode-control-curriculum/2026-08-28-p-reference/`.
  - Final rendered and independent acceptance (2026-08-29): the repaired
    reference lesson passed the real Extension Development Host journey in
    dark, light and high-contrast themes and retained current same-scale gain
    comparison evidence. Harvey accepted the words-before-symbols teaching,
    decision metrics, requested/applied distinction and source/bundle parity.
    Direct evidence: `docs/internal/testing/evidence/vscode-control-curriculum/
    2026-08-29-reference/acceptance.md`.
  - Manual student-read correction (2026-09-02): derived proportional band
    from the output span with unit cancellation; derived the settled offset
    equation algebraically; and moved cumulative travel, settling ranges,
    final-window mean absolute error, validity, and selection rules before
    practice and retained results. The shipped project now starts with the
    selected `SP = 35 degC`, `Kp = 8 %/degC` candidate ready but Auto disabled.
    Control Studio Test writes `P1.AutoMode`, runs that candidate and the owned
    load event for `120 s`, then restores false and zero heat; the README states
    that the native test, not the UI, produces the separate three-gain
    comparison. A new exact native test first failed because the old defaults
    produced zero heat, then passed after the safe selected defaults were
    implemented. The original exact gain-tradeoff native test also passed on
    `trust-builder`; focused README/schema checks and source/bundle parity are
    green. The real Extension Development Host rendered and asserted theory
    and workflow in dark, light, and high-contrast themes under
    `docs/internal/testing/evidence/vscode-control-curriculum/2026-09-02/`.
    These captures have implementer inspection; independent re-acceptance of
    this materially revised lesson remains required before a new `ux_accepted`
    claim. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**
- [ ] `CELP-015` Lesson 5 — Water-heater operating-point bias. Teach equilibrium,
  heat-loss balance, `u0` and deviation variables before measuring and applying
  bias. Prove nominal correction and load-dependent residual offset.
  - Native red/green (2026-08-28): `cargo test -p trust-runtime --test
    control_curriculum_foundations
    control_curriculum_water_heater_bias_removes_nominal_offset_but_not_load_offset
    -- --exact --nocapture` first failed at the expected missing-project
    assertion, then passed after the real shipped ST project applied the
    calculated 75% bias. The test proves nominal error at most 0.1 degC and a
    remaining 1.5…3.0 degC changed-load error under the identical plant event.
  - Focused pedagogical red/green (2026-08-28): `./node_modules/.bin/mocha --ui
    tdd out/test/suite/control-lesson-schema.test.js --grep "teaches
    operating-point bias with the focused reference lesson structure"` first
    failed on the old objective, then passed after the lesson adopted the exact
    11-section reference structure, human navigation, and words-before-symbols
    definitions for SP, PV, error, MV, operating point, bias, deviation and
    proportional gain. The worked prediction now explains the 75% nominal
    holding command without prematurely using the later identified-model
    calculation.
  - Retained, rendered and independent acceptance (2026-08-29): the native
    artifacts retain the manual holding trials and equal-condition biased-P
    comparison; the real Extension Development Host journey passed in dark,
    light and high-contrast themes. Harvey accepted the equilibrium teaching,
    qualification rules, changed-load boundary and source/bundle parity. Direct
    evidence: `docs/internal/testing/evidence/vscode-control-curriculum/
    2026-08-29-lesson-05/acceptance.md`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**
- [ ] `CELP-016` Lesson 6 — Integral-only water-heater control. Teach accumulated
  error, `Ki`, integral time, constant-disturbance rejection and added lag before
  execution. Prove zero offset together with the expected slow/oscillatory
  weakness.
  - Native red/green (2026-08-28): `cargo test -p trust-runtime --test
    control_curriculum_foundations
    control_curriculum_integral_only_removes_offset_but_responds_slower_than_biased_p
    -- --exact --nocapture` first failed at the expected missing-project
    assertion, then passed with the shipped integral-only ST controller. It
    proves monotonic initial `Ki e dt` accumulation, a slower reach than biased
    P, final error at most 0.2 degC, and bounded finite MV.
  - Unit-contract red/green: `accepts integral gain in output percent per
    degree-second` first rejected `%/(degC*s)`, then passed after that engineering
    unit was added to the existing lesson schema. Row remains open for rendered
    comparison evidence and learner review.
  - Focused pedagogical red/green (2026-08-28): `./node_modules/.bin/mocha --ui
    tdd out/test/suite/control-lesson-schema.test.js --grep "teaches integral-only
    and PI control as one causal words-before-symbols progression"` first failed
    on the former abstract integral-ramp objective, then passed after Lesson 6
    defined the heater signals, error, integral gain, cycle time and stored
    integral contribution in physical words before its per-scan equation. The
    learner calculates the first five updates before running and compares the
    slow transient with the biased proportional baseline.
  - Retained, rendered and independent acceptance (2026-08-29): the native
    suite retains the equal-condition biased-P and integral-only traces,
    qualification summary and deterministic plot; the real Extension
    Development Host journey passed in dark, light and high-contrast themes.
    Harvey accepted the causal progression, dimensional prediction, actual ST,
    bumpless preload, restoration and simulation-only boundary. Direct evidence:
    `docs/internal/testing/evidence/vscode-control-curriculum/
    2026-08-29-lesson-06/acceptance.md`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**
- [ ] `CELP-017` Lesson 7 — Water-heater PI. Teach the parallel PI terms and
  damping/speed trade-off before combining P and I. Prove improved transient
  response and zero constant-load offset, then expose saturation.
  - Native red/green (2026-08-28): `cargo test -p trust-runtime --test
    control_curriculum_temperature_progression
    control_curriculum_water_heater_pi_combines_prompt_response_and_zero_offset_then_exposes_saturation
    -- --exact --nocapture` reached the expected missing-project assertion,
    then passed with the shipped PI project. The final criterion uses settling
    and overshoot rather than misleading first-crossing speed: PI settles sooner
    and with less overshoot than integral-only control, removes the changed-load
    error to 0.2 degC, and a separate unreachable 45 degC request visibly
    saturates at 100%. Row remains open for rendered comparison and learner
    review evidence.
  - Focused pedagogical green (2026-08-28): the same paired Lesson 6–7 contract
    now proves that Lesson 7 introduces proportional gain, integral gain,
    stored integral contribution, requested/applied heater power and integral
    time before its equations. The normal P/I/PI comparison is kept separate
    from the unreachable-request exposure, which creates the causal handoff to
    Lesson 8. Source and bundled descriptors were regenerated together. Row
    was superseded by the complete 2026-08-30 evidence below.
  - Complete focused evidence (2026-08-30): the dedicated native suite
    `control_curriculum_water_heater_pi_evidence` passes all six tests. Its
    expected behavior reds prove Manual holding-output tracking, the exposed
    parallel increments, next-scan adoption of a live `Ki` change by both the
    diagnostic and actual PID, and the inherited selected default. The focused
    README test first failed on the missing `Controller under test` stage and
    later on the missing live-mapping and independent-case restoration
    instructions; the same test passes after each specification-led correction.
    The paired lesson-schema test passes, and VS Code registration reports 807
    discovered facts in 70 registered files.
  - Executable result (2026-08-30): equivalent 0.1-second simulations compare
    the inherited biased-P and integral-only references with `Ki` 0.5, 2, 4,
    and 8 at fixed `Kp` 8. Only settled and physically eligible cases are
    ranked. `Ki` 4 is selected with 6.6-second recovery, 0.171653 degC
    overshoot, 0.000003 degC final-window error, 26.109284 percentage points of
    applied-output movement, and no normal-case saturation. The separate
    unreachable exposure records a 159.00% preliminary request, 100.00%
    applied heater power, 60.1 seconds of saturation, and safe Manual zero
    restoration; it is not ranked as a tuning candidate.
  - Packaging and rendered acceptance (2026-08-30): source and bundled README,
    metadata, ST, CSVs, and generated SVG are byte-identical and contain no
    stale root `Main.st`. Real Extension Development Host journeys pass in
    dark, light, and high-contrast themes and verify the generated plot,
    decision table, live-gain boundary, selected and rejected cases, saturation
    exposure, and simulation-only evidence boundary. Harvey
    (`plant_ui_reviewer`) independently rereviewed and accepted the corrected
    lesson with no remaining technical, pedagogical, reproducibility, safety,
    or rendered-UX blocker. Direct evidence:
    `docs/internal/testing/evidence/vscode-control-curriculum/2026-08-30-lesson-07/acceptance.md`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**
- [ ] `CELP-018` Lesson 8 — Heater amplitude saturation. Teach requested versus
  applied MV, physical range, saturation error and unreachable SP before the
  constrained test. Prove limit compliance and hidden integral accumulation.
  - Native red/green (2026-08-28): `cargo test -p trust-runtime --test
    control_curriculum_temperature_progression
    control_curriculum_heater_saturation_separates_requested_and_applied_mv_and_exposes_windup
    -- --exact --nocapture` first reached the expected missing-project
    assertion for `water-heater-saturation`, then passed with the shipped ST
    project. The genuine PID request peaked at 306.601%, the physical 0…100%
    clamp produced 206.601% peak saturation error, the upper limit was active
    for 51.7 s, and accumulated demand held applied MV at 100% for 60.0 s after
    restoration. Every sample remained finite, applied MV remained bounded,
    and saturation error equalled requested minus applied MV. Remote evidence:
    `scena-rust-builder`, validation checkout
    `/home/johannes/codex-workspaces/control-curriculum.Ca6G5h`. Packaging
    command `node editors/vscode/scripts/build-examples.js` accepted and bundled
    82 total examples, including 76 control lessons. Row remains open for the
    current rendered requested/applied overlay and unfamiliar-learner review.
  - Focused pedagogical red/green (2026-08-28): `./node_modules/.bin/mocha --ui
    tdd out/test/suite/control-lesson-schema.test.js --grep "teaches actuator
    constraints and anti-windup without borrowing later model theory"` first
    failed because equations and abbreviations preceded their physical names
    and the lesson imported FOPDT theory from Lesson 15. It passed after Lesson
    8 defined requested/applied heater power, limits, saturation error and
    unreachable demand before notation and replaced the premature model
    calculation with a controller-only 102% versus 100% worked prediction.
  - Superseding specification and executable proof (2026-08-30): Specification
    38 Lesson 8 now defines the complete isolated saturation experiment,
    reachability calculation, 159% first-scan prediction, requested/applied
    signal contract, retained evidence, safe restoration, and bounded transfer.
    The dedicated native suite
    `control_curriculum_water_heater_saturation_evidence` passes all five
    tests. Its valid behavior reds established missing Manual holding output,
    inherited `Kp = 8` and `Ki = 4`, removal of the confounding cold-water
    event, and the missing persistent run-disposition contract. The final
    retained simulation records a 426.053491% peak request, 100.000000% peak
    applied power, 326.053491 percentage-point peak mismatch, 10.0 seconds at
    the upper limit, 12.7 seconds of post-restoration saturation, 25.1 seconds
    to recovery, and Manual zero safe restoration. The strengthened artifact
    test verifies all 951 trace rows carry the deterministic run identity,
    model and abort disposition and verifies the summary validity, restoration,
    and bounded conclusion. Exact final command:
    `ssh trust-builder 'cd "$HOME/projects/trust-platform-control-curriculum-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/control-curriculum-dedicated" cargo test -p trust-runtime --test control_curriculum_water_heater_saturation_evidence control_curriculum_water_heater_saturation_artifacts_identify_and_dispose_the_run -- --exact --nocapture'`
    (`1 passed`; complete suite previously `5 passed`).
  - Packaging and rendered acceptance (2026-08-30): source and bundled lesson
    assets are byte-identical. Real Extension Development Host journeys pass in
    dark, light, and high-contrast themes and render the generated two-panel
    requested/applied plot and qualified decision table. Harvey
    (`plant_ui_reviewer`) independently rejected the first evidence contract,
    then accepted the corrected lesson with no remaining technical,
    pedagogical, reproducibility, safety-boundary, precision, or rendered-UX
    blocker. Direct evidence:
    `docs/internal/testing/evidence/vscode-control-curriculum/2026-08-30-lesson-08/acceptance.md`.
  - Course-wide workflow reference review (2026-09-01): the corrected Lesson 8
    README retains the complete reachability, requested/applied output,
    saturation-exposure, recovery, and safe-restoration teaching content inside
    the exact five-prompt Control Studio workflow. The real Extension
    Development Host journey passed sequentially in dark, light, and
    high-contrast themes, including the generated signal-path diagram, retained
    result plot, qualified decision table, and syntax-colored Structured Text.
    This reviewed workflow is now the normative course-wide pattern in
    Specification 38 §2.1.1. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**
- [ ] `CELP-019` Lesson 9 — Heating-valve slew limitation. Teach amplitude versus
  rate limits, asymmetric rates, admissible per-sample output and configured `dt`
  before comparison. Prove both amplitude and rate compliance and the changed
  achievable response.
  - Native red/green (2026-08-28): `cargo test -p trust-runtime --test
    control_curriculum_temperature_progression
    control_curriculum_heating_valve_rate_limit_uses_actual_dt_and_asymmetric_travel
    -- --exact --nocapture` first reached the expected behavior assertion with
    the old symmetric project (1.0%/s opening, 1.0%/s closing, and 0% retained
    request gap), then passed after the shipped ST calculated the asymmetric
    admissible interval from configured `dt`. The measured limits were 1.5%/s
    opening and 0.5%/s closing, requested/applied position differed by 15.998%,
    and rate-constrained flow was 5.340 L/min at 5 s while applied position
    remained finite and within 0…100%.
  - Unit-contract red/green (2026-08-28): focused
    `control-lesson-schema.test.ts` execution first rejected the physically
    correct `L/min/%` valve-flow gain unit, then passed after that unit was
    added to the shared lesson schema. Row remains open for packaged sequence
    validation after Lessons 10–11, rendered comparison evidence, and learner
    review.
  - Focused pedagogical green (2026-08-28): the shared Lessons 8–10 contract
    now proves explicit words-and-units definitions for configured cycle time,
    previous valve position, asymmetric opening/closing rates and the next
    admissible interval before rate-limit equations. Premature FOPDT model
    claims and the duplicate Control Studio appendix were removed.
  - Corrective reviewer red/green (2026-08-30): the strengthened native artifact
    test first failed on trace row 3 because the retained previous position had
    already been overwritten (`reconstructed=0`, `retained=1.5`). The ST now
    snapshots `PreviousPositionUsedForUpdate` before the limiter and state
    update. The same test passes, and all 901 retained rows reproduce measured
    travel rate within `1e-9`; the independent reconstruction found zero
    failures and maximum absolute error `1e-11`. ST `TASK T#100ms`, runtime
    `cycle_interval_ms = 100`, and `ConfiguredCycleTime = 0.1 s` are now
    explicitly synchronized configured values, not a measured scheduler
    interval; Lesson 14 owns measured elapsed time and jitter.
  - Final focused and packaged evidence (2026-08-30): the dedicated native suite
    passes `3 passed`; the renamed legacy asymmetric behavior lock passes
    `1 passed`; focused learner and shared Lessons 8–10 contracts each pass
    `1 passing`; VS Code registration reports `809 discovered facts, 70 files,
    70 registrations`; `npm run build:examples` bundles 90 projects; and source
    and bundled Lesson 9 directories are byte-identical. Real Extension
    Development Host journeys pass `1 passing` in dark, light, and high
    contrast. Harvey (`plant_ui_reviewer`) independently accepted the corrected
    lesson with no remaining technical, pedagogical, reproducibility,
    precision, safety-boundary, or rendered-UX blocker. Direct evidence:
    `docs/internal/testing/evidence/vscode-control-curriculum/2026-08-30-lesson-09/acceptance.md`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**
- [ ] `CELP-020` Lesson 10 — Batch-vessel anti-windup. Teach clamping,
  conditional integration, back calculation, `Tt`, integration direction and
  the combined method before testing. Prove and explain each strategy's
  saturation, recovery, overshoot and MV trade-off.
  - Native red/green (2026-08-28): `cargo test -p trust-runtime --test
    control_curriculum_temperature_progression
    control_curriculum_batch_vessel_compares_four_anti_windup_strategies --
    --exact --nocapture` first reached the expected behavior assertion because
    the old project mapped all enabled cases to one Combined configuration,
    then passed with four configurations of the unchanged shipped PID: external
    clamp without anti-windup, BackCalculation with `Tt=dt` signal clamping,
    BackCalculation with `Tt=3.25 s`, and Combined with `Tt=3.25 s`. Recovery
    times were respectively 19.0, 12.0, 6.7, and 4.3 s; post-restoration upper
    saturation was 4.5 s for the baseline and 0 s for all protected cases; total
    valve travel was 7.000%, 16.108%, 14.000%, and 14.000%. All traces remained
    finite and applied trim stayed within -3.5…3.5%. Row remains open for the
    four-run rendered comparison, explanation of overshoot, and learner review.
  - Focused pedagogical green (2026-08-28): the shared Lessons 8–10 contract
    now proves that integral windup, anti-windup, cycle time, tracking time and
    back-calculation correction are introduced in physical words before the
    correction equation. The comparison retains all four genuine strategies
    and removes untaught FOPDT terminology and the duplicate workflow appendix.
  - Superseding executable and rendered acceptance (2026-08-30): Specification
    38 and the shipped lesson now define one initial Manual sample, 500 Auto
    samples through the 20.0–50.0 s recovery observation, and Manual-zero
    restoration at 50.1 s for each of four fresh runs. The dedicated native
    suite first failed at `501 != 502`, then passes `4 passed`; its 2,008 rows
    reconstruct complete-run applied-valve travel of `7.169438`, `14.699942`,
    `14.462547`, and `14.507017 pp`, including the final transition. A second
    expected red exposed floating-point handling at an exact `0.1 s` boundary;
    the green test now proves the exact recovery boundary, exact `0.1 pp`
    travel boundary, and state-equation-simplicity tie. Eligibility and
    selection are calculated from the predeclared rules, selecting ordinary
    BackCalculation for its `2.7 s` shortest eligible recovery, not an
    undeclared `fast` label. Focused learner tests for Lessons 8–10 pass `3
    passing`; anti-windup schema checks pass `2 passing`; registration reports
    `811 discovered facts, 70 files, 70 registrations`; `npm run
    build:examples` bundles 90 projects; and source/bundled Lesson 10 assets are
    byte-identical. Real Extension Development Host journeys pass `1 passing`
    in dark, light, and high contrast with the verified native aarch64 server.
    Harvey (`plant_ui_reviewer`) independently accepted the final executable,
    pedagogical, reproducibility, precision, source/package, and clean rendered
    evidence after rejecting three earlier candidates. Direct evidence:
    `docs/internal/testing/evidence/vscode-control-curriculum/2026-08-30-lesson-10/acceptance.md`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**
- [ ] `CELP-021` Lesson 11 — Reactor servo versus regulatory objectives. Teach
  disturbance paths and response/performance measures before separate SP and
  load tests. Prove that one tuning can satisfy one objective and fail another.
  - Native red/green (2026-08-28): `cargo test -p trust-runtime --test
    control_curriculum_temperature_progression
    control_curriculum_reactor_separates_servo_and_regulatory_objectives_and_metrics
    -- --exact --nocapture` first reached the expected servo-overshoot assertion:
    the former tuning had 0 degC overshoot and 58.1 s regulatory recovery, so it
    neither reproduced the Lesson 12 motivation nor gave strong load rejection.
    A corrected experiment establishes 80 degC for 5 s before stepping SP; the
    earlier initialization-at-85 result was discarded. It then passed with the
    same shipped PI configured for the declared regulatory objective. The
    unchanged `b=1` controller produced 1.049 degC servo overshoot (failing the
    1.0 degC requirement) while rejecting the
    cold-feed-equivalent load in 11.6 s with 0.00077 degC final error. Separate
    IAE, ISE, ITAE, settling/recovery, peak-deviation, and total-valve-travel
    measures were finite and positive; both runs stayed inside PV/MV limits.
  - Unit-contract red/green (2026-08-28): focused
    `control-lesson-schema.test.ts` execution first rejected `degC^2*s` and
    `degC*s^2`, then passed after the shared schema admitted the correct ISE and
    ITAE units. At that stage the row remained open for two-run rendered
    evidence and learner review.
  - Focused pedagogical red/green (2026-08-28): `./node_modules/.bin/mocha --ui
    tdd out/test/suite/control-lesson-schema.test.js --grep "teaches response
    metrics weighting filtering and timing before using their notation"` first
    failed because Lesson 11 used SP, PV, sampled error and metric abbreviations
    before definitions. It passed after the lesson introduced servo/regulatory
    tests, sample indexing, IAE, ISE, ITAE, overshoot, settling/recovery and
    total valve travel in physical words and units before equations. The
    premature plant-model calculation and duplicate workflow appendix were
    removed. At that stage the row remained open for rendered comparison and
    learner review.
  - Final executable and learner acceptance (2026-08-30): Specification 38 and
    the README now define all metric calculations, units, windows, validity
    prerequisites, upper/lower requirements, and numerical tolerances before
    results. A focused README red failed because the settling-band tolerance
    was absent; the paired green passed after the lesson declared the
    0.5-degree-Celsius band with 0.01-degree-Celsius comparison tolerance and
    the 0.1-second servo-settling reproduction tolerance. A focused native red
    failed at missing retained field `settling_band_degC`; the paired dedicated
    suite passed `3 passed; 0 failed` after the executable metric used the
    effective 0.51-degree-Celsius boundary and the 57-field summary retained
    the band, tolerances, and individual dispositions. Current results are
    1.049273-degree-Celsius servo overshoot, 19.7-second servo settling,
    1.276193-degree-Celsius regulatory peak, 11.6-second recovery, and
    0.003732-degree-Celsius final-window mean absolute error. The aligned
    legacy native test passed `1 passed; 0 failed`; the complete focused README
    file passed `10 passing`; the schema contract passed `1 passing`;
    registration reported `812 discovered facts, 70 files, 70 registrations`;
    example packaging bundled 90 projects; and source/bundled Lesson 11 assets
    are byte-identical. Real Extension Development Host journeys passed in
    dark, light, and high contrast using the verified native aarch64 server.
    `plant_ui_reviewer` independently accepted the final specification,
    executable evidence, tolerance semantics, safety boundary, pedagogy,
    source/package parity, and current unclipped rendered evidence after two
    rejection-and-correction cycles. Direct evidence:
    `docs/internal/testing/evidence/vscode-control-curriculum/2026-08-30-lesson-11/acceptance.md`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**
- [ ] `CELP-022` Lesson 12 — Reactor proportional setpoint weighting. Teach `b`
  and true integral error before comparing weights. Prove changed servo
  aggressiveness with unchanged nominal disturbance response.
  - Native red/green (2026-08-28): `cargo test -p trust-runtime --test
    control_curriculum_temperature_progression
    control_curriculum_reactor_setpoint_weight_reduces_servo_overshoot_without_changing_regulatory_path
    -- --exact --nocapture` first reached the expected missing-project
    assertion. An intermediate run exposed an invalid experiment that
    initialized directly at 85 degC; that evidence was discarded and the test
    now establishes 80 degC for 5 s before stepping. The final shipped project
    passed with unchanged `Kp=8`, `Ki=2`, plant, limits, timing, and true-error
    integral action: servo overshoot was 1.049, 0.882, and 0.807 degC for
    `b=1`, `0.5`, and `0`, so both weighted cases pass the 1.0 degC requirement.
    The complete fixed-SP regulatory PV/MV traces were identical within
    `1e-10`. The historical `0.00077 degC` terminal-sample values are superseded:
    a final sample is not equilibrium evidence. The aligned test now reports the
    qualified mean absolute error over `110.1..120.0 s`,
    `0.003731619943637696 degC`, after recovery. All six runs remained finite
    and bounded.
  - Packaging evidence (2026-08-28): `node
    editors/vscode/scripts/build-examples.js` on `scena-rust-builder` accepted
    83 total examples and 77 gap-free control lessons; source and shipped
    Lesson 12 descriptors agreed at that historical checkpoint.
  - Focused pedagogical green (2026-08-28): the shared Lessons 11–14 contract
    now proves definitions for the true error, dimensionless proportional
    setpoint weight, setpoint/process changes, and proportional/integral output
    changes before the weighted equations. The worked prediction explains all
    three weights in words and preserves the unchanged regulatory comparison.
  - Final executable and learner acceptance (2026-08-30): Specification 38 and
    the README now implement the complete reusable practical-lesson pattern.
    The focused README test first failed at the missing objective and later
    passed; the focused native restoration test first failed because the old ST
    reused current MV as its Manual request and passed after a distinct
    `ManualOutput` produced the specified rate-constrained restoration. The
    dedicated six-run suite then failed only at the missing retained artifacts
    and passed `3 passed; 0 failed` after the 5,340-row, 38-field native trace,
    six-row, 61-field qualification summary, and generated 900-by-760 SVG were
    retained. Current servo overshoot is `1.049273342257`, `0.881877753624`, and
    `0.806937072829 degC`; the largest eligible weight is therefore `b = 0.5`.
    All regulatory cases reproduce `1.276193205671 degC` peak deviation,
    `11.6 s` recovery, `0.003731619944 degC` final-window mean absolute error,
    `33.485594923714 pp` complete valve travel, and exactly zero aligned PV/MV
    disagreement. The aligned legacy test passed `1 passed; 0 failed`; the
    complete focused README file passed `11 passing`; applicable schema tests
    passed `2 passing`; packaging bundled 90 examples; and source/bundled
    Lesson 12 assets are byte-identical. Real Extension Development Host
    journeys passed in dark, light, and high contrast. `plant_ui_reviewer`
    independently recalculated the measures from all retained samples,
    regenerated the figure byte-for-byte, and accepted the specification,
    actual ST, evidence, safety boundary, pedagogy, source/package parity, and
    current unclipped rendering. Direct evidence:
    `docs/internal/testing/evidence/vscode-control-curriculum/2026-08-30-lesson-12/acceptance.md`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**
- [ ] `CELP-023` Lesson 13 — Pump-pressure measurement filtering. Teach spectrum,
  quantization, first-/second-order filtering, cutoff, lag and roll-off before
  applying deterministic noise. Prove the PV-error/MV-activity/lag trade-off.
  - Native red/green (2026-08-28): `cargo test -p trust-runtime --test
    control_curriculum_temperature_progression
    control_curriculum_pump_pressure_compares_unfiltered_first_and_second_order_filters
    -- --exact --nocapture` first reached the expected strategy assertion
    because the former project always executed the same second-order filter.
    It then passed with bypass, first-order, and cascaded second-order runs on
    one deterministic noise sequence. Controller-PV increment RMS was 0.02549,
    0.01463, and 0.00891 bar; MV increment RMS was 0.01787, 0.01028, and
    0.00628%; positive lag area was 0.16782, 0.21105, and 0.26265 bar*s; mean
    final error remained about 0.0034 bar. A noisy 90% crossing was explicitly
    rejected as a lag metric in favour of the integrated within-run
    raw-minus-controller gap. All runs were finite and bounded.
  - Unit-contract red/green (2026-08-28): focused schema tests first rejected,
    then accepted, the physically required `bar/%` process gain and `bar*s` lag
    units. Packaging on `scena-rust-builder` again accepted 83 examples and 77
    gap-free control lessons. Row remains open for equal-scale rendered spectra,
    traces, metric table, and learner review.
  - Focused pedagogical green (2026-08-28): the shared Lessons 11–14 contract
    now proves words-and-units definitions for raw and filtered pressure,
    filter time constant, cutoff frequency, filter coefficient and RMS before
    their equations. Untaught FOPDT parameters and the duplicate workflow
    appendix were removed. Row remains open for real equal-scale plots and
    rendered learner review.
  - Current acceptance (2026-08-30): Specification 38 now defines the actual
    discrete coefficient, continuous-reference versus executed cutoff, aligned
    windows, units, thresholds, tolerances, weakest-qualified selection,
    simulator-only truth boundary, and deterministic invalid-configuration
    behavior. The dedicated native suite passed `3 passed; 0 failed`; the
    aligned legacy comparison passed `1 passed; 0 failed`; the complete focused
    README file passed `12 passing`; the applicable schema test passed
    `1 passing`; registration validation reported `814 discovered facts, 70
    files, 70 registrations`; packaging bundled 90 examples; and source and
    bundled assets are byte-identical. The historical positive-lag numbers
    above were produced by a one-scan-misaligned metric and are superseded by
    the synchronized signed lag areas `0`, `0.048598893775`, and
    `0.098300637440 bar*s`. Independent review first rejected a nonzero Manual
    request escaping the invalid path and mispositioned pressure-axis ticks.
    Both received valid focused assertion reds, paired greens, regenerated
    evidence, and fresh real Extension Development Host captures in dark,
    light, and high contrast. The same reviewer then returned explicit
    `ACCEPTED`. Direct evidence:
    `docs/internal/testing/evidence/vscode-control-curriculum/2026-08-30-lesson-13/acceptance.md`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**
- [ ] `CELP-024` Lesson 14 — Dryer sampling and jitter. Teach sampling, hold,
  introductory aliasing, `dt`, jitter, missed scans and timing-dependent rate
  limits before experiments. Prove when measured elapsed time preserves
  behavior and when timing becomes a fault.
  - Native red/green (2026-08-28):
    `cargo test -p trust-runtime --test control_curriculum_temperature_progression control_curriculum_oven_contrasts_nominal_and_measured_elapsed_time -- --exact --nocapture`
    first failed its behavioral assertion because the former program used the
    same 0.1 s nominal interval during a real 0.2 s scan, producing 0.0667%
    instead of the calculated 0.1334% integral increment. The unchanged command
    passed after the lesson exposed a measured-elapsed-time mode while retaining
    the deliberately incorrect nominal-time baseline.
  - Native safety red/green (2026-08-28):
    `cargo test -p trust-runtime --test control_curriculum_temperature_progression control_curriculum_oven_rejects_nonpositive_and_missed_scan_intervals_safely -- --exact --nocapture`
    first reached the expected assertion red because the former program accepted
    a 0.3 s missed scan as valid. It passed after the executable lesson admitted
    only `0 < dt <= 0.25 s`, skipped `PID.Update` outside that envelope, exposed
    timing validity, and commanded the declared 0% safe MV. Earlier input-map
    and division-by-zero harness failures were discarded and do not count as red.
  - Packaged behavior lock (2026-08-28):
    `cargo test -p trust-runtime --test control_curriculum_foundations control_curriculum_sampling_project_uses_each_actual_cycle_interval -- --exact --nocapture`
    passed against the newly packaged project, proving exact 0.03335, 0.0667,
    and 0.1334% first integral increments plus bounded fixed-interval and jitter
    runs. `node editors/vscode/scripts/build-examples.js` accepted 83 examples
    and 77 gap-free control lessons. Row remains open for the rendered equal-axis
    timing journey and unfamiliar-learner review.
  - Focused pedagogical green (2026-08-28): the shared Lessons 11–14 contract
    now proves words-and-units definitions for cycle time, sampling frequency,
    Nyquist frequency, aliasing, timing jitter and missed scans before timing
    equations and experiments. Untaught plant-model terminology was removed;
    the 0.25 s safe timing envelope remains explicit. Row remains open for the
    rendered timing journey and learner review.
  - Current acceptance (2026-08-30): Specification 38 now defines the
    conveyor-oven physical boundary, sample/hold concept, actual/controller/
    nominal interval meanings, exact predictions, equivalent timing runs,
    fixed `0.25 s` ceiling, explicit recovery, measures, tolerances, evidence,
    and transfer. The dedicated native suite passed `4 passed; 0 failed`; the
    inherited timing behaviors passed `3 passed; 0 failed`; the complete
    focused README file passed `13 passing`; the applicable schema contract
    passed `1 passing`; registration reported `815 discovered facts, 70 files,
    70 registrations`; packaging bundled 90 examples; and source and bundled
    assets are byte-identical. Independent review rejected the first candidate
    for a widenable safety ceiling, missing zero-interval proof, incomplete
    retained decisions, mismatched fault timestamps, and a missing introductory
    diagram. After test-first correction it rejected one remaining conflation
    of fault qualification with recovery success. The final artifacts classify
    both invalid journeys as unqualified `Timing fault` while separately
    retaining successful reset/re-enable, and the same reviewer returned
    explicit `ACCEPTED`. Direct evidence:
    `docs/internal/testing/evidence/vscode-control-curriculum/2026-08-30-lesson-14/acceptance.md`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**
- [ ] `CELP-025` Lesson 15 — First-principles water-heater model. Teach energy
  conservation, thermal capacitance, losses, equilibrium, gain, time constant,
  delay, units and validity before opening Identify. Prove the derived model's
  prediction against configured simulation truth.
  - Native red/green (2026-08-28):
    `cargo test -p trust-runtime --test control_curriculum_modelling_progression control_curriculum_heater_energy_balance_predicts_configured_plant_truth -- --exact --nocapture`
    first reached the expected assertion red because the required shipped
    `water-heater-energy-balance` project did not exist. The unchanged command
    passed after the executable ST lesson derived `K=0.2 degC/%`, `tau=5 s`,
    and `Teq=24 degC` from `C=50 kJ/degC`, `H=10 kW/degC`, and `P100=200 kW`;
    the configured plant was within 0.08 degC of the 63.2% prediction at 6 s
    and within 0.02 degC of equilibrium at 30 s with exactly 20% bounded MV.
  - Unit-contract red/green (2026-08-28): focused schema execution first
    rejected `kJ/degC`, then the unchanged test accepted both `kJ/degC` and
    `kW/degC` after those explicit units were added. Packaging accepted 84
    examples and 78 gap-free control lessons. The ST source contains teaching
    comments for the energy-balance derivation, command clamp, and invalid-model
    safe state. Row remains open for the rendered prediction overlay and
    unfamiliar-learner review.
  - Focused pedagogical red/green (2026-08-28): `./node_modules/.bin/mocha --ui
    tdd out/test/suite/control-lesson-schema.test.js --grep "teaches the heater
    modelling identification validation and tuning chain words first"` first
    failed because the energy-balance equation preceded all physical quantity
    definitions. It passed after Lesson 15 introduced temperature, elapsed time,
    thermal capacitance, heat input, heat loss, ambient temperature, rated power,
    equilibrium, gain, time constant and delay with units before equations.
  - Final reference-lesson acceptance (2026-08-30): Specification 38 §2.1.3
    now makes the corrected lesson pattern mandatory course-wide. Focused
    assertion reds established requested-versus-applied prediction, missing
    retained artifacts, exact Manual restoration, Test authority, complete
    provenance, data-derived `30.10 s` restoration labelling, non-overlapping
    figure layout, prediction-before-controller ordering, and the exact retained
    six-second error field. The existing finite diagnostic-envelope behavior
    received honest green coverage for parameter, ambient, PV and requested-
    power lower/upper boundaries, out-of-envelope safe zero, retained rejected
    requests, and typed-runtime nonfinite rejection. Final focused commands
    passed: `cargo test -p trust-runtime --test
    control_curriculum_modelling_progression
    control_curriculum_heater_energy_balance_predicts_configured_plant_truth --
    --exact --nocapture` (`1 passed`), `cargo test -p trust-runtime --test
    control_curriculum_water_heater_model_contract
    lesson_15_retains_reconstructible_prediction_and_equilibrium_evidence --
    --exact --nocapture` (`1 passed`), and the focused Lesson 15 README contract
    (`1 passing`). Registration reports `816 discovered facts, 70 files, 70
    registrations`; packaging bundles 90 examples; source/bundle are byte-
    identical. Fresh real Extension Development Host journeys pass `1 passing`
    in dark, light, and high contrast. Harvey (`plant_ui_reviewer`) rejected two
    intermediate candidates, then returned explicit `ACCEPTED` with no remaining
    technical, pedagogical, safety-boundary, evidence-integrity, precision, or
    rendered-UX blocker. Direct evidence:
    `docs/internal/testing/evidence/vscode-control-curriculum/2026-08-30-lesson-15/acceptance.md`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**
- [ ] `CELP-026` Lesson 16 — Heater step identification. Teach experiment
  baseline, bounded step, settling, FOPDT parameters, noise and model provenance
  before the test. Prove an estimated model with uncertainty without calling it
  configured truth.
  - Native behavior lock (2026-08-28):
    `cargo test -p trust-runtime --test control_curriculum_modelling_progression control_curriculum_heater_step_recovers_fopdt_from_recorded_reaction_curve -- --exact --nocapture`
    passed before and after the pedagogical/ST-comment refactor. A fresh 10 s
    20% baseline and 20→30% manual step recovered `K` within 0.005 degC/%, `L`
    within 0.2 s, and `T` within 0.3 s without giving the fitter access to
    configured truth.
  - Contract red/green (2026-08-28): focused schema execution first failed
    because the shipped v2 metadata did not implement Lesson 16's causal v4
    contract. The unchanged test passed after the project became sequence 16,
    prerequisite Lesson 15, and explicitly retained qualification metrics, raw
    samples, competing fits, parameter intervals, residuals, warnings,
    restoration, provenance, and `validation required`. Row remains open for
    the rendered Identify journey and unfamiliar-learner review.
  - Focused pedagogical green (2026-08-28): the same modelling-chain contract
    proves that baseline input/output, step amplitude, output change, process
    gain and the complete first-order-plus-dead-time structure are explained
    before the model equation. Configured truth remains unavailable to the fit.
  - Completed acceptance (2026-08-30): two focused native tests, the registered
    pedagogical/specification contract, complete five-candidate identification
    evidence, deterministic selection contract, packaging parity, and the
    current real dark-theme Extension Development Host journey are green.
    Harvey (`plant_ui_reviewer`) rejected two intermediate candidates and then
    returned explicit `ACCEPTED` after the specification made the resolved-
    dead-time override unambiguous. Theme variants remain assigned to the
    Module 1 gate. Direct evidence:
    `docs/internal/testing/evidence/vscode-control-curriculum/2026-08-30-lesson-16/acceptance.md`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**
- [ ] `CELP-027` Lesson 17 — Independent heater-model validation. Teach holdout
  data, residual, error, bias, introductory whiteness, uncertainty and rejection
  before validation. Prove acceptance, restriction, or rejection on new data.
  - Native red/green (2026-08-28):
    `cargo test -p trust-runtime --test control_curriculum_modelling_progression control_curriculum_heater_validation_accepts_new_step_but_restricts_unmodelled_load -- --exact --nocapture`
    first reached the expected behavioral assertion red because the former
    project was a 50 degC heat exchanger rather than the same heater. The
    unchanged command passed after a distinct 20→25% heater run achieved
    fixed-flow prediction RMSE at most 0.03 degC, while an omitted cold-flow
    path produced -0.9 to -1.1 degC residual bias. The result therefore accepts
    local fixed-flow command prediction and explicitly restricts load prediction.
  - Contract red/green (2026-08-28): the source-contract test first rejected
    the old v2 heat-exchanger identity and passed after Lesson 17 became a v4
    holdout workflow with frozen parameters, distinct run IDs, prediction
    envelope, residual diagnostics, accepted/restricted/rejected semantics, and
    no-refit rule. Row remains open for rendered residual evidence and review.
  - Focused pedagogical green (2026-08-28): the modelling-chain contract now
    proves that independent estimation/validation data, measured and predicted
    temperature, residual, residual bias, whiteness, uncertainty and prediction
    envelope precede the residual equation and verdict workflow.
  - Completed acceptance (2026-08-30): the focused native test uses the exact
    frozen Lesson 16 selection and independent samples. Fixed-flow RMSE is
    `0.004760 degC`; the omitted cold-flow path produces `-0.991545 degC`
    residual bias and is restricted rather than falsely accepted. The complete
    14-test modelling progression, focused registered README contract, source/
    bundle parity, and current real dark-theme Extension Development Host
    response/residual journey are green. Harvey (`plant_ui_reviewer`) returned
    explicit `ACCEPTED`; no physical-plant evidence is claimed. Direct evidence:
    `docs/internal/testing/evidence/vscode-control-curriculum/2026-08-30-lessons-17-18/acceptance.md`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**
- [ ] `CELP-028` Lesson 18 — Model-based heater PI tuning. Teach Lambda/IMC,
  response target, model parameters and robustness-speed trade-off before
  calculating gains. Compare against the earlier empirical tuning across SP,
  disturbance, noise, limits and model perturbation.
  - Native red/green (2026-08-28):
    `cargo test -p trust-runtime --test control_curriculum_modelling_progression control_curriculum_lambda_pi_improves_declared_response_and_survives_model_error -- --exact --nocapture`
    first reached the expected behavioral assertion red because empirical and
    model-based selections executed identical hard-coded `Kp=4`, `Ki=0.8`
    gains. The unchanged command passed after the shipped ST calculated
    `Kp=5`, `Ti=5 s`, `Ki=1` from `K=0.2`, `T=5 s`, `L=1 s`, and `lambda=4 s`.
    The final model-validity correction retains an in-range 30.2→30.6 degC
    experiment with a 0.4 degC cold-flow load. Tracking IAE improves
    `6.075021→5.492143 degC*s` and load-period IAE improves
    `2.499546→1.999319 degC*s`. With `K=0.19`, `T=6 s`, `L=1.3 s`, and seeded
    0.03 degC noise, final error is `0.000990 degC`; every approved-run PV and
    applied MV remains inside 30–31 degC and 20–25%, and complete perturbed MV
    travel is `212.785368` percentage points.
  - Contract red/green and current module gate (2026-08-28): schema execution
    first rejected the old v3/sequence-17 contract and then accepted causal v4
    Lesson 18 with the validated-model prerequisite, equations, uncertainty
    cases, invalid/infeasible rejection, and unapplied recommendation. The full
    four-test modelling module and all 32 compiled control-lesson schema tests
    passed; the real extension compile packaged 84 examples and 78 gap-free
    control lessons. Row remains open for rendered tuning comparison and review.
  - Focused pedagogical green (2026-08-28): the modelling-chain contract now
    proves physical meanings and units for desired response time, frozen model
    gain/time/delay, proportional gain, integral time and integral gain before
    the Lambda/SIMC equations. Source and bundled examples are synchronized.
  - Validity correction red/green (2026-08-30): the exact lifecycle command
    first reached the expected assertion red `out-of-bounds model gain must not
    create a tuning candidate`; the unchanged command passed after declared
    engineering bounds were enforced. The exact performance command then
    reached the expected assertion red `empirical run must stay inside the
    accepted 30-31 degC model range`; it passed after the common experiment was
    moved wholly inside Lesson 17's accepted PV/MV region. The retained-artifact
    drift red was regenerated explicitly, and the unchanged 14-test modelling
    module now passes on `trust-builder`.
  - Rendered-contract red/green (2026-08-30): the registered README test first
    failed because the descriptor still declared the former 20 degC/0% initial
    state. It passed after source, metadata, retained results, plots and bundle
    agreed on the corrected experiment. Real dark-theme Extension Development
    Host execution passes `1 passing`; the common-axis comparison, uncertainty
    plot, lifecycle, metrics, safe state and simulation-only boundary are all
    asserted from the rendered document.
  - Completed acceptance (2026-08-30): reviewer-found assertion reds corrected
    qualification-limit saturation, five-percentage-point local actuator
    headroom, and missing immutable model provenance/parameter intervals.
    Retained saturation is `4.0/3.8/3.6 s` for empirical/Lambda/perturbed runs;
    a 5.5-point request is executably rejected; exact Lesson 17 source hashes
    and intervals are retained. Source/bundle diff is clean, the fresh real
    Extension Development Host journey passes, and Harvey
    (`plant_ui_reviewer`) returned `ACCEPTED`, calling the current dark evidence
    readable, unclipped, and professional. Direct evidence:
    `docs/internal/testing/evidence/vscode-control-curriculum/2026-08-30-lessons-17-18/acceptance.md`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**
- [ ] `CELP-M01` Module 1 gate — Run Lessons 1–18 consecutively as one learner
  journey. Prove the causal ON/OFF → hysteresis → P → bias → I → PI → limits →
  anti-windup → filtering/timing → modelling → identification → validation →
  model-based tuning progression and capture the rendered milestone.
  - Partial pedagogical gate (2026-08-28): `./node_modules/.bin/mocha --ui tdd
    out/test/suite/control-lesson-schema.test.js --grep "keeps every
    temperature-spine README on the focused teaching path"` first failed at
    Lesson 1's duplicate workflow appendix and advanced lesson by lesson until
    all 18 READMEs passed the single ordered 11-section path, learner-facing
    navigation and 1,500-word focus limit.
  - Completed module evidence (2026-08-30): one builder invocation passed all
    85 tests across the 19 native Lesson 1–18 binaries. The real Extension
    Development Host then opened, copied and rendered Lessons 1 through 18
    consecutively, producing 18 ordered current journey records and the Lesson
    18 milestone screenshots. All source/bundle directories are byte-equivalent;
    the focused causal-path, complete 84-lesson catalog and registration checks
    pass. Harvey independently accepted the complete milestone and the two
    minimal legacy-test repairs. Direct evidence:
    `docs/internal/testing/evidence/vscode-control-curriculum/
    2026-08-30-module-01/acceptance.md`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**

## 3. Motion-control spine

Specification authority: §3.2, Lessons 19–25.

- [ ] `CELP-029` Lesson 19 — Position-table P control. Teach error, torque,
  inertia, friction, natural response, damping and proportional stiffness before
  the gain sweep. Prove accuracy improvement followed by reduced damping.
  - Source/native evidence (2026-08-28): schema-v4 project
    `examples/control/02-derivative-action-and-dynamic-response/19-servo-position-p` and focused README contract
    are implemented. Native test
    `control_curriculum_position_p_exposes_stiffness_damping_tradeoff` proves
    the gain sweep; row remains open for retained plot and rendered review.
  - Completed evidence (2026-08-30): the same native suite retains the two gain
    traces and decision summary, the focused README/schema checks are green,
    and the real VS Code journey rendered the single comparison plot. Harvey
    independently accepted the technical lesson, pedagogy and rendered result.
    Module evidence: `docs/internal/testing/evidence/vscode-control-curriculum/
    2026-08-30-module-02/acceptance.md`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**
- [ ] `CELP-030` Lesson 20 — Position-table PD. Teach derivative action,
  velocity feedback, phase lead, damping ratio, `Kd` and bias before tuning.
  Prove improved damping and remaining constant-load offset.
  - Focused evidence (2026-08-28): the README test first failed on the missing
    numerical damping-ratio prediction, then passed after the worked `Kd=0`
    and `Kd=3` calculation. Remote native test
    `control_curriculum_position_pd_adds_damping_without_changing_stiffness`
    passed with P/PD overshoot 0.595806/0.040406 rad and late span
    0.119873/0.000043 rad. Retained plot and rendered review remain open.
  - Completed evidence (2026-08-30): the native result is retained as aligned
    position and torque evidence, the focused README/schema checks are green,
    and the real VS Code journey rendered the comparison. Harvey independently
    accepted the technical lesson, causal progression and rendered result.
    Module evidence: `docs/internal/testing/evidence/vscode-control-curriculum/
    2026-08-30-module-02/acceptance.md`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**
- [ ] `CELP-031` Lesson 21 — Loaded-table PID. Teach integral action in the
  mechanical loop, PID term roles and torque-limit windup before adding I.
  Prove zero load error while preserving acceptable damping and constraints.
  - Focused evidence (2026-08-28): the pedagogical contract failed first on
    missing windup and settling definitions, then passed. Remote native test
    `control_curriculum_loaded_position_pid_removes_pd_load_error_with_bounded_torque`
    passed with PD/PID final error 0.164582/0.002689 rad, settled final PV and
    torque ranges, bounded torque, and PID overshoot -0.002227 rad. Plot and
    rendered review remain open.
  - Completed evidence (2026-08-30): the retained comparison proves the
    PD/PID final-error reduction and a separate torque-saturation/recovery run
    proves bounded recovery rather than inferring it from the nominal response.
    The focused native and README checks pass, the real VS Code journey renders
    both figures, and Harvey independently accepted the corrected lesson.
    Module evidence: `docs/internal/testing/evidence/vscode-control-curriculum/
    2026-08-30-module-02/acceptance.md`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**
- [ ] `CELP-032` Lesson 22 — Derivative kick and setpoint weighting. Teach
  derivative of error, derivative on PV and `c` before the SP experiment. Prove
  reduced peak requested/applied torque without hiding position response.
  - Focused evidence (2026-08-28): the pedagogical contract failed first on the
    missing final-sample/equilibrium distinction, then passed. Remote native
    test `control_curriculum_derivative_setpoint_weight_exposes_and_removes_torque_kick`
    passed with first requested torque 79.02/41.52/4.02 N m for
    `c=1/0.5/0`, while applied torque remained 10/10/4.02 N m. Plot and rendered
    review remain open.
  - Completed evidence (2026-08-30): retained position, requested-torque and
    applied-torque traces expose the full, intermediate and measurement-only
    derivative cases. Focused tests and the real VS Code journey are green;
    Harvey independently accepted the technical lesson and rendered result.
    Module evidence: `docs/internal/testing/evidence/vscode-control-curriculum/
    2026-08-30-module-02/acceptance.md`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**
- [x] `CELP-033` Lesson 23 — Filtered derivative. Teach finite-difference noise
  gain, filter order, cutoff, lag and resonance before adding encoder noise.
  Prove the position/torque/noise/damping compromise.
  - Focused evidence (2026-08-28): the pedagogical contract failed first on
    missing metric equations, then passed. Remote native test
    `control_curriculum_filtered_derivative_trades_encoder_noise_for_measured_lag`
    passed with torque-increment RMS 0.874919/0.192292/0.045913 N m and positive
    lag area 0.009053/0.017473/0.028451 rad s. Plot and rendered review remain
    open.
  - Completed evidence (2026-08-30): the native decision applies the declared
    0.250 N m activity and 0.020 rad s lag limits and proves only the first-order
    0.05 s candidate qualifies. Retained evidence, shipped bundle and current
    VS Code rendering agree; Harvey independently accepted the lesson.
- [x] `CELP-034` Lesson 24 — Mechanical nonidealities. Teach physical signatures
  of torque/velocity/acceleration limits, friction, deadband, backlash and
  stiction before injection. Prove diagnosis from PV, requested MV, applied MV
  and actuator state; do not mislabel maintenance defects as tuning defects.
  - Backlash runtime red/green (2026-08-29): exact remote test
    `process_output_backlash_holds_then_follows_the_engaged_flank` first failed
    because `width` was unknown, then passed after the Specification 33 play
    operator was implemented. Negative boundary/width validation is green.
  - Lesson red/green (2026-08-29): exact remote native test
    `control_curriculum_mechanical_nonidealities_are_diagnosed_before_retuning`
    first failed because the executable project was absent, then passed with
    ideal/nonideal reversal delay 0.52/1.34 s, requested/applied maxima
    3.364/2.461 N m, bounded 0.5 rad/s and 1.0 rad/s2 profile, and observed
    saturation, slew, deadband, stiction, and backlash. Hardware Canvas editor
    test first returned no backlash fields, then passed with a positive-width
    process-output/replace contract. Retained plot and rendered interaction
    review remain open.
  - Completed evidence (2026-08-30): the unprofiled reference, six isolated
    effects and combined case retain per-case requested/applied torque and
    actuator state. Per-case limits and decisions are reconstructible; the
    shipped rendered lesson is current and Harvey independently accepted it.
- [x] `CELP-035` Lesson 25 — Digital motion realization. Teach hold, Euler and
  Tustin forms, quantization, aliasing, precision and computational delay before
  discretizing. Prove method/sample-time effects on stability, noise and timing.
  - Lesson red/green (2026-08-29): exact remote native test
    `control_curriculum_digital_realization_exposes_method_sampling_alias_and_delay`
    first failed because the executable project was absent, then passed with a
    0.209612 N m forward/backward-Euler torque difference, 0.002042 rad 2 Hz
    alias from 8 Hz interference sampled at 10 Hz, exact 10/20/100 ms update
    spacing, zero-order-held torque, 0.001 rad quantization, and first torque at
    0.010/0.020 s without/with one-update delay. The seven-lesson source and
    pedagogical tests are green; retained plots, bundled build, and rendered
    review remain open.
  - Completed evidence (2026-08-30): retained results expose actual
    0.01/0.02/0.10 s holds, permanent-band plus final-window settling, and an
    independently estimated 2.00 Hz alias. Four current figures are readable
    in VS Code with meaningful milliradian precision; Harvey accepted the
    technical, pedagogical and visual result.
- [ ] `CELP-M02` Module 2 gate — Run Lessons 19–25 as one P → PD → PID →
  setpoint handling → derivative filtering → real actuator → discrete-control
  journey and capture the rendered milestone.
  - Completed evidence (2026-08-30): the final remote native suite passed 8/8,
    the registered focused README test passed, registration validated 823 facts
    across 70 files and 70 registrations, source/bundle parity is exact, all
    corrected real VS Code journeys passed, and independent review accepted
    Lessons 19–25. Evidence: `docs/internal/testing/evidence/
    vscode-control-curriculum/2026-08-30-module-02/acceptance.md`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**

## 4. Production-quality digital PID

Specification authority: §3.3, Lessons 26–30.

- [ ] `CELP-036` Lesson 26 — Positional, incremental and combined forms. Teach
  both laws, their state, constant-error behavior and the integral switch before
  comparison. Reproduce incremental P failure and prove correct P/PI behavior
  from the combined implementation.
  - Native red/green (2026-08-29): exact remote test
    `control_curriculum_combined_pid_selects_the_valid_form_from_integral_action`
    first failed because `examples/control/03-production-quality-digital-pid/26-pid-forms/src/Main.st`
    was absent, then passed with positional P torque changing 2.000 to
    4.000 N m at unchanged 0.5 rad error, the rejected incremental-P diagnostic
    remaining at its arbitrary 1.000 N m state, and PI gain-change
    discontinuity after the expected integral increment equal to zero within
    `1e-12` percent.
  - Learner-facing red/green (2026-08-29): registered VS Code test `teaches why
    the production PID selects positional or incremental realization` first
    failed on the missing Lesson 26 descriptor, then passed against the
    schema-v4 descriptor, focused README, actual ST, and two executable plant
    declarations. The catalog test first failed because Lesson 26 was absent,
    then passed after it replaced the superseded duplicate sequence-19 catalog
    entry while retaining 84 curriculum identities.
  - Completed evidence (2026-08-30): focused runtime assertions pass for both
    realization behavior and the 500-row two-plant run; source and shipped
    bundle are exact; the real VS Code journey and independent review accepted
    the current plot and lesson. Evidence: `docs/internal/testing/evidence/
    vscode-control-curriculum/2026-08-30-module-03/acceptance.md`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**
- [x] `CELP-037` Lesson 27 — Booster-pump initialization and modes. Teach state,
  prior signals, filter settling, Disabled/Manual/Auto/Track and safe startup
  before execution. Prove correct startup from multiple real actuator states.
  - Native red/green (2026-08-29): exact remote test
    `control_curriculum_booster_pump_initializes_from_actual_state_before_auto`
    first failed because the booster-pump project was absent, then passed with
    measured-state first requests equal to actual 20, 40, and 70 percent valve
    positions, a deliberate zero-state request/filter result of 10.000 percent
    and 0.198 bar at the 40 percent/6 bar operating point, the station-level
    Disabled output at 30 percent, and the real simulated equilibrium retained
    at 6.000 bar and 40.000 percent.
  - Learner-facing red/green (2026-08-29): registered VS Code test `teaches
    booster-pump state initialization before automatic authority` first failed
    on the missing descriptor and then passed against the schema-v4 contract,
    focused README, one actual `Control.PID`, actual `Control.LowPass`, and
    executable first-order pump. The catalog test first failed with Lesson 27
    absent, then passed after the future plant-classification project stopped
    occupying that slot.
  - Completed evidence (2026-08-30): the retained run includes zero-state,
    measured 20/40/70 percent starts, Disabled/Manual/Track/Auto behavior and
    the complete ten-second plant history. Native, focused VS Code, shipped
    bundle, rendered journey and independent review passed; see the Module 3
    acceptance evidence.
- [x] `CELP-038` Lesson 28 — Bumpless transfer and external tracking. Teach state
  alignment, feasible/infeasible bumpless transitions and external station
  responsibility before switching modes. Prove bounded transitions and expose
  failure without tracking.
  - Native red/green (2026-08-29): exact remote test
    `control_curriculum_booster_pump_tracking_makes_authority_transfer_bumpless`
    first failed because the handover project was absent, then passed with the
    tracked external/Auto output remaining 60.000/60.000 percent (0.000 pp
    step), the untracked controller returning to stale 40.000 percent (20.000
    pp step), and unaligned/aligned Auto-to-Manual steps of 30.000/0.000 pp.
  - Learner-facing red/green (2026-08-29): registered VS Code test `teaches
    bumpless authority transfer as a shared tracking contract` first failed on
    the missing descriptor, then passed against the schema-v4 contract,
    focused README, one actual `Control.PID`, Track invocation, and the explicit
    simulation-only inactive-without-tracking failure path. The catalog test
    also failed first, then passed after Lesson 28 replaced the future tank
    conservation project in that slot.
  - Completed evidence (2026-08-30): five transition cases include the stale
    tracking infeasibility boundary and a separate ten-second recovery. Native,
    focused VS Code, shipped bundle, rendered journey and independent review
    passed; see the Module 3 acceptance evidence.
- [x] `CELP-039` Lesson 29 — Atomic updates and signal ordering. Teach coherent
  configuration, feedforward contribution, nominal/applied state and scan order
  before reproducing partial-update and external-summation failures. Prove one
  atomic, constraint-aware path.
  - Native red/green (2026-08-29): exact remote test
    `control_curriculum_booster_pump_applies_atomic_parameters_and_orders_feedforward`
    compiled and first failed only because the Lesson 29 project was absent,
    then passed against one `Control.PID`. The coherent and deliberately mixed
    first outputs were 55.300 and 55.100 percent; internal feedforward returned
    the rate-constrained 42.000 percent while the disconnected post-limit sum
    exposed 60.000 percent.
  - Learner-facing red/green (2026-08-29): registered VS Code test `teaches
    atomic configuration and one constraint-aware feedforward path` first
    failed because the descriptor was absent, then exposed and closed the
    missing `%/bar` and `%/(bar*s)` schema units before passing against the
    schema-v4 contract, canonical lesson flow, actual Structured Text, metric
    definitions, and simulation-only failure diagnostics. The catalog test
    `publishes Lesson 29 without assigning its slot to a model-selection
    lesson` failed first, then passed after the future dryer model-selection
    project stopped occupying this slot.
  - Completed evidence (2026-08-30): two expected assertion reds exposed false
    current/previous input aliasing (`40` instead of `60`, then `40.3` instead
    of `55.3`). The paired exact native test passes with declaration-specific
    bindings and runtime-read evidence. Six scans retain a 60 percent independent
    target, prior-applied rate bounds, 42–52 percent applied output and active
    rate limiting. Source/bundle parity, real VS Code rendering and independent
    review passed; see the Module 3 acceptance evidence.
- [x] `CELP-040` Lesson 30 — Invalid data and safe behavior. Teach numeric,
  configuration, freshness, timing, watchdog, latch and recovery policies before
  fault injection. Prove deterministic diagnostics, no partial update, safe MV
  and controlled restoration.
  - Native red/green (2026-08-29): exact remote test
    `control_curriculum_booster_pump_rejects_invalid_scans_and_requires_qualified_recovery`
    first failed because the Lesson 30 project was absent, then passed with the
    runtime typed-I/O boundary rejecting nonfinite pressure before execution,
    station codes 1/2/3/4/5 identifying physical-range, stale, invalid-cycle,
    deadline, and configuration faults, zero PID/configuration calls on invalid
    scans, and 30.000 percent safe and recovered output. The recovery proof
    separately exercised acknowledgement, three valid scans, reset-state
    initialization, and renewed Auto authority.
  - Learner-facing red/green (2026-08-29): registered VS Code test `teaches
    deterministic invalid-data handling and qualified recovery` first failed
    because the descriptor was absent, then passed against the schema-v4
    contract, canonical lesson flow, stable fault map, layered runtime/station
    boundary, exact recovery sequence, actual Structured Text, and defined
    decision measures. The catalog test `publishes Lesson 30 without assigning
    its slot to an excitation lesson` failed first, then passed after the future
    excitation project stopped occupying this slot.
  - Completed evidence (2026-08-30): the retained lifecycle covers isolated
    codes 1–5, configuration fault, unacknowledged and acknowledged/count-zero
    scans, valid scans 1–3, Configure/Initialize-only reset and later PID/Auto.
    Native, focused VS Code, shipped bundle, rendered journey and independent
    review passed; see the Module 3 acceptance evidence.
- [ ] `CELP-M03` Module 3 gate — Qualify the reusable P/PI/PD/PID implementation
  against all practical behaviors introduced in Lessons 1–30. Confirm no second
  controller path or duplicated mode/constraint logic was introduced.
  - Completed evidence (2026-08-30): `control_curriculum_pid_production` passes
    5/5; the two Lesson 26 exact tests pass 1/1 each; the two focused registered
    VS Code tests pass 1/1 and 4/4; test registration reports 823 facts across
    70 registered files; all five real Extension Development Host journeys pass.
    Harvey independently accepted pedagogy, artifacts, source/bundle parity and
    rendered semantics. Full commands, reds, screenshots and boundaries are in
    `docs/internal/testing/evidence/vscode-control-curriculum/
    2026-08-30-module-03/acceptance.md`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**

## 5. Controller selection and plant modelling

Specification authority: §3.4, Lessons 31–36.

- [ ] `CELP-041` Lesson 31 — Controller selection workshop. Teach accuracy,
  integration, damping, noise, actuator type and servo/regulatory priority
  before selecting structures for the cold room, heater, tank and table. Prove
  the minimum sufficient choice, including cases where PID is not selected.
  - Native red/green (2026-08-29): exact remote test
    `control_curriculum_selects_the_minimum_sufficient_structure_for_four_real_loops`
    first required a test-harness integer-type correction and then produced the
    valid missing-project assertion red. It passed after the executable
    four-plant workshop selected ON/OFF, PI, P, and PID respectively, retained
    cold-room temperature from 1.492 to 2.502 degrees Celsius, and measured
    final-window heater/tank/table errors of 0.0159 degrees Celsius, 0.0000 cm,
    and 0.0000 rad.
  - Learner-facing red/green (2026-08-29): new registered VS Code test `teaches
    minimum-sufficient controller selection from requirements` first failed
    because the descriptor was absent, then exposed and closed the missing `cm`
    engineering unit before passing against the schema-v4 contract, canonical
    teaching flow, four conditional selection arguments, actual ON/OFF and
    `Control.PID` code, requirement-coverage decision rule, and executable
    metrics. The catalog test `publishes Lesson 31 before the plant-modelling
    progression` failed first, then passed after the future local-linear-model
    project stopped occupying the slot.
  - Completed evidence (2026-08-30): the retained twenty-candidate matrix and
    four executed selections, byte-identical shipped bundle, clean rendered
    journey, screenshots, and independent acceptance are recorded in
    `docs/internal/testing/evidence/vscode-control-curriculum/2026-08-30-module-04/acceptance.md`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**
- [x] `CELP-042` Lesson 32 — Modular-skid plant behavior. Teach operating point,
  gain, direction, stored quantities, time constants, integration, delay,
  damping and resonance before equal step tests. Prove correct response-class
  distinctions without fitting models prematurely.
  - Native behavior-lock/red/green (2026-08-29): exact remote test
    `control_curriculum_classifies_four_physical_plant_responses_before_modelling`
    first required legacy address mapping so the old example could execute,
    then validly failed when its normalized vessel ended at 0.998 rather than
    the specified physical temperature. It passed after replacement with the
    open-loop physical skid: vessel 53.927 degrees Celsius, tank 62.000 cm,
    oven 84.908 degrees Celsius after 4.0 seconds delay, and flexible-drive
    peak 1.527 rad with four steady-value crossings.
  - Learner-facing red/green (2026-08-29): registered VS Code test `teaches
    physical plant-response classification before model fitting` first failed
    against the legacy schema-v2 descriptor, then exposed and closed legitimate
    open-loop contract gaps for optional SP and `cm/s` before passing against
    schema v4, physical commands and PV units, words-before-symbols theory,
    predictions, exact metrics, no hidden PID, and the canonical lesson flow.
    The catalog test `publishes the physical plant-response workshop as Lesson
    32` failed first, then passed after the future frequency-response project
    stopped occupying the slot.
  - Completed evidence (2026-08-30): physical-unit traces, the first-second
    ambiguity worksheet with no answer key, closed/revealed VS Code captures,
    clean logs, shipped equality, and independent acceptance are recorded in
    the Module 4 acceptance evidence.
- [x] `CELP-043` Lesson 33 — Tank and heater conservation models. Teach mass/
  energy balance, states, differential equations, equilibrium and units before
  derivation. Prove equilibrium and transient predictions against both plants.
  - Native red/green (2026-08-29): exact remote test
    `control_curriculum_conservation_balances_predict_tank_and_heater` first
    failed because the executable `tank-heater-conservation` project was
    absent, then passed with tank rate `0.4000 cm/s`, tank level `58.000 cm` at
    20 seconds, equal-flow drift `0.000000 cm`, heater temperature `22.528
    degC` at delay plus one time constant, and `23.988 degC` after 30 seconds.
    The test also proves the one-second delay, five-second time constant,
    24-degree equilibrium prediction, matching physical units, and absence of
    a hidden PID.
  - Learner-facing red/green (2026-08-29): registered test `teaches tank mass
    and heater energy conservation before equations` first failed because the
    Lesson 33 descriptor was absent, then exposed the missing `m2` engineering
    unit and a litre-to-cubic-metre explanation that occurred too late. It
    passed after the explicit unit contract and words-before-equations theory
    were corrected. Catalog test `publishes the conservation-law workshop as
    Lesson 33` failed with an absent index, then passed after the lesson
    replaced the premature sensitivity entry while retaining 84 control
    identities. The combined modelling/catalog suites pass 11 tests and VS
    Code registration passes with 737 facts across 66 registered files.
  - Completed evidence (2026-08-30): both physical traces, the independent
    equal-flow run, units/quality/model provenance, teaching plot, shipped
    equality, clean rendered journey, and independent acceptance are recorded
    in the Module 4 acceptance evidence.
- [x] `CELP-044` Lesson 34 — Gravity-tank linearization and transfer functions.
  Teach Taylor approximation, deviation variables, Laplace representation,
  gain, poles, zeros, order and delay before local-model comparison. Prove each
  model only inside its declared operating region.
  - Native red/green (2026-08-29): exact remote test
    `control_curriculum_gravity_tank_linearization_is_local_and_unit_consistent`
    first failed because the canonical project was absent, then passed against
    three nonlinear gravity tanks and three independently derived tangent
    models. Small-step errors were `0.24994`, `0.24888`, and `0.24508 cm` at
    0.25, 0.50, and 0.81 m; the low-level large-step extrapolation error was
    `6.241 cm`. The test independently checks equilibrium flows, gains, time
    constants, poles, unit conversion, initial equilibria, no hidden PID, and
    no artificial dead time.
  - Learner-facing red/green (2026-08-29): registered test `teaches
    gravity-tank linearization as a validated local model` first failed because
    the Lesson 34 descriptor was absent, then exposed the missing physical
    units `m2*sqrt(m)/s` and `m/(m3/s)` before passing against the schema-v4
    lesson, words-before-equations derivation, actual ST, three nonlinear
    simulator plants, declared validity bounds, and the explicit distinction
    between local plant models and gain scheduling. Catalog test `publishes
    gravity-tank linearization as Lesson 34` failed with an absent index and
    passed after replacement of the premature crane-stability entry while the
    catalog retained 84 control identities.
  - Completed evidence (2026-08-30): synchronized nonlinear/local runs retain
    operating point, coefficients, poles, errors and validity decisions; the
    plot, shipped equality, clean rendered journey, and independent acceptance
    are recorded in the Module 4 acceptance evidence.
- [x] `CELP-045` Lesson 35 — Poles, zeros and plant classes. Teach stable/
  unstable and real/complex poles, damping, dominant dynamics, nonminimum-phase
  zeros, inverse response and cancellation risk before matching real plants to
  maps and responses.
  - Native red/green (2026-08-29): exact remote test
    `control_curriculum_pole_zero_maps_predict_six_real_plant_responses` first
    failed because the canonical `pole-zero-plant-classes` project was absent,
    then passed against six executable physical examples. Measured signatures
    were heater five-second fraction `0.632`, integrating-tank slope `0.200
    cm/s`, flexible-drive peak/crossings `1.527/9`, crane dominant-tail ratio
    `0.607`, magnetic-bearing 10 mm abort at `9.3 s`, and boiler inverse-
    response minimum/final `-0.505/0.554`. The stable five-plant comparison and
    unstable-bearing experiment are separate so the declared bearing abort is
    operational rather than decorative.
  - Learner-facing red/green (2026-08-29): registered test `teaches pole-zero
    interpretation from six executable plant responses` first failed because
    the Lesson 35 descriptor was absent, then exposed the missing `1/s` unit and
    two unclear workflow sentences before passing against the schema-v4 lesson,
    words-before-symbols theory, all six numerical predictions, exact metric
    definitions, actual ST, six simulator declarations, and separate bearing
    abort. Catalog test `publishes the six-plant pole-zero workshop as Lesson
    35` failed with an absent index and passed after replacement of the
    premature root-locus entry while retaining 84 control identities.
  - Completed evidence (2026-08-30): six executed physical traces, a common-
    scale complex-plane worksheet, hidden matching/reveal record, separate abort
    evidence, shipped equality, clean rendered journey, and independent
    acceptance are recorded in the Module 4 acceptance evidence.
- [x] `CELP-046` Lesson 36 — Two-tank state-space introduction. Teach state,
  vectors, matrices, state/output equations, equilibrium, eigenvalues and the
  transfer-function relationship before simulation. Prove internal-state
  behavior that the external transfer view hides.
  - Native red/green (2026-08-29): exact remote test
    `control_curriculum_two_tank_state_space_exposes_hidden_stored_water` first
    failed because the canonical `two-tank-state-space` project was absent,
    then passed with the inlet-step equilibrium at `20.000/50.000 cm` and two
    zero-input cases that began with identical downstream readings. The empty
    case remained zero while the upstream-charged case produced a `10.857 cm`
    downstream peak at `3.10 s`, proving that the external reading did not
    reveal the complete stored-water state.
  - Learner-facing red/green (2026-08-29): registered test `teaches state space
    by revealing hidden water in interconnected tanks` first failed because the
    Lesson 36 descriptor was absent, then exposed anonymous initial-state arrays,
    the missing `m2/s` unit, and an unclear equilibrium definition before
    passing against named state components, words-before-equations theory,
    physical matrix rows and units, actual ST, three identical executable
    matrix models, and the prediction/commit/reveal workflow. Catalog test
    `publishes the interconnected-tank state-space workshop as Lesson 36`
    failed with an absent index and passed after replacement of the premature
    Nyquist entry while retaining 84 control identities.
  - Completed evidence (2026-08-30): main and hidden-state runs retain state
    order and matrices; a blank current-output prediction is captured before
    the future is revealed; shipped equality, clean rendered interactions, and
    independent acceptance are recorded in the Module 4 acceptance evidence.
- [ ] `CELP-M04` Module 4 gate — Have an unfamiliar learner derive one physical
  model, classify all declared plant types and explain why modelling is now
  necessary. Capture current model/time-response/pole-zero evidence.
  - Accepted (2026-08-30): all six native lesson contracts, focused VS Code
    contracts, byte-identical bundles, clean real Extension Host journeys,
    precommit/reveal screenshots, and independent review passed. Evidence:
    `docs/internal/testing/evidence/vscode-control-curriculum/2026-08-30-module-04/acceptance.md`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**

## 6. Experiment design and model identification

Specification authority: §3.5, Lessons 37–41.

- [ ] `CELP-047` Lesson 37 — Safe heat-exchanger experiment design. Teach the
  engineering question, baseline, operating point, amplitude, duration,
  settling, confounding loads, limits, abort, restoration and approval before
  any excitation. Prove a reviewable simulation plan and honest physical gate.
  - Native red/green (2026-08-29): exact remote test
    `control_curriculum_safe_heat_exchanger_plan_rejects_executes_and_restores`
    first failed because the canonical `safe-heat-exchanger-experiment`
    project was absent. The same test then exposed an overlapping process-image
    address and the specification's inconsistent treatment of 3 seconds dead
    time inside a nominal 60-second observation. After correcting the binding
    and specifying a 63-second hold, it passed with Plan B rejected before the
    valve moved, Plan A late-window temperature `54.676 degC`, qualified
    restoration at `50.099 degC`, and bad-quality abort reason `1` with both
    requested and applied valve positions restored to `60.000%`. Command:
    `ssh trust-builder 'cd /home/johannes/codex-workspaces/control-curriculum.Ca6G5h && CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum cargo test -p trust-runtime --test control_curriculum_modelling_progression control_curriculum_safe_heat_exchanger_plan_rejects_executes_and_restores -- --exact --nocapture'`.
  - Learner-facing and catalog red/green (2026-08-29): registered test `teaches
    safe heat-exchanger experiment design before data collection` first failed
    because the Lesson 37 descriptor was absent, passed against the completed
    schema-v4 project, then intentionally failed because the project was not
    reachable from `examples/catalog.json`, and passed again after catalog
    replacement. It proves the complete lesson-stage order, words before
    symbols, exact model and plan prediction, requested/applied distinction,
    metric definitions and validity, actual ST, physical-authority denial,
    measured result table, and the causal handoff to Lesson 38. Exact direct
    command: `ssh trust-builder 'cd /home/johannes/codex-workspaces/control-curriculum.Ca6G5h/editors/vscode && npx tsc -p ./ && npx mocha --ui tdd --grep "teaches safe heat-exchanger experiment design before data collection" out/test/suite/control-lesson-selection-modelling.test.js'`.
    The nearby modelling/schema/catalog run passed `71` tests, and
    `python3 scripts/check_vscode_test_registration.py` reported `744`
    discovered facts across `66` registered files.
  - Completed evidence (2026-08-30): accepted, rejected, aborted, and restored
    records, the reviewed plan, generated comparison plot, byte-identical
    shipped bundle, and clean rendered journey passed independent review.
    Evidence: `docs/internal/testing/evidence/vscode-control-curriculum/2026-08-30-module-05/acceptance.md`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**
- [ ] `CELP-048` Lesson 38 — Heater and servo excitation methods. Teach step,
  pulse, doublet, relay, PRBS, chirp, frequency content, persistent excitation,
  SNR and safety before comparison. Prove plant-appropriate method selection.
  - Native red/green (2026-08-29): exact remote test
    `control_curriculum_excitation_methods_match_heater_and_flexible_drive_dynamics`
    first failed because the canonical `excitation-methods` project was absent,
    then passed against the shipped dual-plant simulator and actual ST waveform
    generator. The deterministic run measured heater step/chirp output spans
    `1.065/0.274 degC`, heater/servo PRBS internal transitions `5/45` at
    `2.5/0.3 s` dwell, servo doublet signed area `-0.0000 N m s`, servo chirp
    peak `1.091 Hz` at `0.180 rad`, and fail-closed relay reason `4` with zero
    plant departure. Command: `ssh trust-builder 'cd /home/johannes/codex-workspaces/control-curriculum.Ca6G5h && CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum cargo test -p trust-runtime --test control_curriculum_modelling_progression control_curriculum_excitation_methods_match_heater_and_flexible_drive_dynamics -- --exact --nocapture'`.
  - Learner-facing red/green (2026-08-29): registered test `teaches plant-
    matched excitation with heater and flexible-drive evidence` first failed
    because the Lesson 38 descriptor was absent and then produced the expected
    schema assertion for unsupported signed-area unit `N m s`. It passed after
    that unit was deliberately added to the explicit engineering-unit
    allowlist and after the complete README, descriptor, executable project,
    measured result table, and catalog entry agreed. It proves words-before-
    symbols definitions for all six methods, persistent excitation and SNR,
    actual timing and ST, plant-specific dwell and chirp band, metric validity,
    relay rejection, and the causal handoff to model selection. The adjacent
    modelling/schema/catalog run passed `72` tests; registration reported `745`
    discovered facts across `66` registered files.
  - Completed evidence (2026-08-30): all seven runs and qualification decisions,
    heater and servo plots, byte-identical shipped bundle, and rendered method-
    selection and relay-rejection journey passed independent review. Evidence:
    `docs/internal/testing/evidence/vscode-control-curriculum/2026-08-30-module-05/acceptance.md`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**
- [ ] `CELP-049` Lesson 39 — Dryer model fitting and selection. Teach estimation,
  residuals, complexity, bias/variance and candidate structures before fitting.
  Prove selection by independent prediction rather than training fit.
  - Native red/green (2026-08-29): exact remote test
    `control_curriculum_dryer_selects_simplest_model_from_independent_prediction`
    first ran against the pre-existing draft and failed its physical baseline:
    the project used an oscillatory second-order plant and a PID wrapper instead
    of the specified open-loop transport-delayed dryer. The replacement test
    then exposed and corrected the simulator's absolute-input `output_bias`
    convention before passing. The independent evidence recovered FOPDT
    `K/T/L = 0.40/12.0/4.0`, validation RMSE `0.0508 degC`, and bias
    `-0.0004 degC`; first order retained RMSE `0.1635 degC` and delay-window
    bias `-0.2233 degC`; no-delay second order retained RMSE `0.0729 degC`
    without the required 10% improvement; and the integrating candidate ended
    with `1.200 degC` restoration error. Exact command: `ssh trust-builder 'cd /home/johannes/codex-workspaces/control-curriculum.Ca6G5h && CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum cargo test -p trust-runtime --test control_curriculum_modelling_progression control_curriculum_dryer_selects_simplest_model_from_independent_prediction -- --exact --nocapture'`.
  - Learner-facing red/green (2026-08-29): registered test `teaches conveyor-
    dryer model selection from independent prediction` failed against the old
    schema-v2 summary and passed after the schema-v4 lesson, open-loop ST,
    FOPDT simulator, four-candidate theory, words-before-equations residual and
    RMSE definitions, independent pulse workflow, predeclared complexity rule,
    measured result table, and catalog entry agreed. The nearby modelling/
    schema/catalog run passed `73` tests after one stale assertion that called
    dryer selection a future lesson was updated to prove its later position.
    Registration reported `746` discovered facts across `66` registered files.
  - Completed evidence (2026-08-30): independent estimation and validation
    records, candidate predictions, residuals, uncertainty, scores, rejection
    decisions, generated plots, byte-identical bundle, and rendered selection
    journey passed independent review. Evidence:
    `docs/internal/testing/evidence/vscode-control-curriculum/2026-08-30-module-05/acceptance.md`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**
- [ ] `CELP-050` Lesson 40 — Heat-exchanger poor excitation and closed-loop ID.
  Teach identifiability, collinearity, closed-loop bias and input-noise
  correlation before analysis. Prove deterministic “data inadequate” results.
  - Native red/green (2026-08-29): exact remote test
    `control_curriculum_poor_excitation_rejects_unidentifiable_records` first
    compiled and ran the legacy single-plant PID project, then failed the
    expected behavior assertion because its applied-input span was `7.006`
    percentage points instead of the specified poor `0.2`-point experiment.
    After the three independent heat-exchanger records were implemented, the
    first candidate run exposed an important specification correction: a
    first-order response at dead time plus three time constants is about 95%
    complete, so the contract now distinguishes the `5.0 degC` predicted
    equilibrium movement from the measured `4.795 degC` late-window movement.
    A subsequent design guard produced an expected assertion red because the
    draft ST embedded qualification reason codes; those codes were removed and
    are now derived from recorded evidence. The final green measured poor
    span/response/SNR `0.2/0.088/0.67`, qualified
    `10.0/4.795/33.33`, feedback-created correlation `-1.0000`, and derived
    decisions `[1, 0, 2]`. Exact command: `ssh trust-builder 'cd /home/johannes/codex-workspaces/control-curriculum.Ca6G5h && CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum cargo test -p trust-runtime --test control_curriculum_modelling_progression control_curriculum_poor_excitation_rejects_unidentifiable_records -- --exact --nocapture'`.
  - Learner-facing red/green (2026-08-29): registered test `teaches why heat-
    exchanger records can be unidentifiable` failed against the old schema-v2
    summary, then passed after the schema-v4 descriptor, three-copy simulator,
    direct ST experiment, catalog entry, and 1,600-word README agreed. It proves
    words-before-symbols definitions for identifiability, persistent
    excitation, SNR, collinearity, exogenous/endogenous input and correlation;
    the actual feedback equation; requested/applied separation; predeclared
    reasons; no PID wrapper or embedded fit/answer; and the causal handoff to
    independent validation. The adjacent modelling suite passed `10` tests,
    the native modelling-progression suite passed `11` tests, and registration
    reported `747` discovered facts across `66` registered files.
  - Completed evidence (2026-08-30): all three raw records, derived decisions,
    signal/SNR/correlation plots, byte-identical shipped bundle, and rendered
    inadequate-data/recovery journey passed independent review. Evidence:
    `docs/internal/testing/evidence/vscode-control-curriculum/2026-08-30-module-05/acceptance.md`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**
- [ ] `CELP-051` Lesson 41 — Heat-exchanger independent validation. Teach
  holdout evidence, confidence, uncertainty envelope, extrapolation and model
  region before testing. Prove accepted, restricted and rejected outcomes.
  - Native red/green (2026-08-29): exact remote test
    `control_curriculum_heat_exchanger_validation_classifies_scope_and_uncertainty`
    first ran the legacy single-water-heater project and failed the expected
    assertion because it could not produce three equal 5-point validation
    records. The replacement uses a separate Lesson 41 project so the existing
    Lesson 17 water-heater validation remains intact; its original focused test
    was restored and passed. The Lesson 41 green retained local
    RMSE/bias/lag-one-correlation/envelope-coverage
    `0.1488 degC/-0.0018 degC/-0.0133/0.998`, cold-flow pre-load RMSE and
    post-load bias `0.1491/-1.8680 degC`, and high-flow late bias/coverage
    `-1.2482 degC/0.000`, producing evidence-derived
    accepted/restricted/rejected classifications. Exact new command: `ssh trust-builder 'cd /home/johannes/codex-workspaces/control-curriculum.Ca6G5h && CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum cargo test -p trust-runtime --test control_curriculum_modelling_progression control_curriculum_heat_exchanger_validation_classifies_scope_and_uncertainty -- --exact --nocapture'`. Preservation command:
    the same invocation with exact test
    `control_curriculum_heater_validation_accepts_new_step_but_restricts_unmodelled_load`
    also passed.
  - Learner-facing red/green (2026-08-29): registered test `teaches independent
    heat-exchanger validation and bounded model scope` first failed because the
    old project identity did not match the new Lesson 41 contract. It passed
    after the distinct schema-v4 project, catalog entry, three-condition
    simulator, open-loop ST, and README agreed. It proves words-before-symbols
    definitions for holdout data, frozen models, residuals, RMSE, bias,
    autocorrelation, parameter intervals, confidence level, prediction
    envelopes, coverage, interpolation/extrapolation, validity region, and all
    three verdicts; it also prevents a PID, fitted model, residual, envelope, or
    verdict from being embedded in ST. The nearby modelling learner suite
    passed `11` tests and registration reported `748` facts across `66` files.
  - Packaging audit: `npm run build:examples` progressed through Lesson 41 and
    now stops at the next honest legacy collision,
    `control-heat-exchanger-tuning-methods` sequence `36`, which is the source
    position to be replaced by Lesson 42. This is a catalog migration boundary,
    not test evidence for Lesson 41.
  - Completed evidence (2026-08-30): the three validation artifacts retain a
    resolvable Lesson 40 estimation source, frozen initial-state rule, interval
    corners, predictions, envelopes, residuals, metrics, and all verdicts.
    Generated plots, byte-identical bundle, and rendered journey passed
    independent review. Evidence:
    `docs/internal/testing/evidence/vscode-control-curriculum/2026-08-30-module-05/acceptance.md`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**
- [x] `CELP-M05` Module 5 gate — Run a complete safe Test → Identify → Validate
  journey including insufficient data, wrong model order, model rejection and
  recovery. Confirm no model is produced merely to keep the workflow moving.
  - Accepted (2026-08-30): the real Extension Development Host journey passed
    with integrating-model rejection on independent validation, acceptance
    hidden, explicit FOPDT recovery, model acceptance, tuning, and validation.

## 7. PID tuning methods

Specification authority: §3.6, Lessons 42–44.

- [x] `CELP-052` Lesson 42 — Heat-exchanger manual and historical tuning. Teach
  P/I/D effects, ultimate gain/period, Ziegler-Nichols assumptions and aggressive
  decay behavior before tuning. Prove why the historical result is not a
  universal default.
  - Native expected red: on `trust-builder`, `CARGO_TARGET_DIR=$HOME/.cache/
    codex-targets/control-curriculum cargo test -p trust-runtime --test
    control_curriculum_tuning_progression
    control_curriculum_manual_and_historical_pi_expose_their_tradeoffs --
    --exact --nocapture` reached the result assertion against the legacy
    one-case draft and failed because its four required controller records had
    zero valve span. This was a behavior failure, not a compile or harness
    failure.
  - Native green (2026-08-29): the same exact command passed against
    `examples/control/06-pid-tuning-methods/42-heat-exchanger-tuning-methods`. It proved
    manual-P late offset 1.248 degC; rejection of the independently executed
    `Ki = 0.05` trial at 0.336 degC final load error; 14 critical crossings and
    measured `Pu = 11.550 s`; manual/historical final errors 0.078/0.006 degC;
    rise times 33.40/3.00 s; manual overshoot 4.13 percent; and cumulative
    applied-valve movement 209.26/1064.01 percentage points.
  - Learner-contract expected red/green (2026-08-29): after compiling the
    extension TypeScript, `./node_modules/.bin/mocha --ui tdd
    out/test/suite/control-lesson-pid-production.test.js --grep "teaches manual
    and historical heat-exchanger PI tuning"` first failed on the legacy
    schema-version-2 lesson. The completed schema-version-4 lesson then exposed
    one further focused red because the shared unit vocabulary rejected the
    physically correct `%/degC` proportional-gain unit. Adding only that unit
    made the same test pass. The README now follows Specification 38
    §§2.1–2.1.3, shows the real two-instance `Control.PID` implementation,
    defines every comparison measure and validity rule before reporting the
    executable results, and remains 1,449 words.
  - Completed evidence (2026-08-30): the native 3/3 batch and focused extension
    3/3 batch passed; the 9,250-row trace and 5-row summary retain exact
    assumptions, requested/applied MV, validity metrics, restoration, and
    `applied=false`. Source/bundle parity, rendered README/plot review, and
    independent acceptance are recorded in the Module 6 acceptance report.
- [x] `CELP-053` Lesson 43 — Relay autotuning. Teach relay limit cycle, critical
  estimates, hysteresis, noise rejection, applicability and aborts before the
  bounded test. Produce an unapplied candidate or an honest rejection.
  - Native lesson expected red/green (2026-08-29):
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum cargo test
    -p trust-runtime --test control_curriculum_tuning_progression
    control_curriculum_feedback_relay_qualifies_cycles_before_producing_candidate
    -- --exact --nocapture` first reached the expected missing-project
    assertion for `examples/control/06-pid-tuning-methods/43-relay-autotuning/src/Main.st`.
    The same command then passed on `trust-builder` with 16 qualified cycles,
    process amplitude `1.293 degC`, period `11.938 s`, half-cycle variation
    `0.045/0.053`, signal-to-noise ratio `25.85`, and estimated critical gain
    `17.73`; the deliberately weak record was rejected at `0.085 degC` and
    signal-to-noise ratio `1.69`.
  - Learner-contract expected red/green (2026-08-29): after compiling the
    extension, `./node_modules/.bin/mocha --ui tdd
    out/test/suite/control-lesson-pid-production.test.js --grep "teaches
    qualified feedback-relay autotuning without automatic application"` first
    failed because the executable relay project was absent and then passed with
    the schema-version-4 lesson, real stateful relay ST, bounded timeout and
    restoration, qualification/rejection criteria, and an explicitly unapplied
    PI candidate.
  - Shared Control Studio expected red/green (2026-08-29):
    `./node_modules/.bin/tsc -p ./ && ./node_modules/.bin/mocha --ui tdd
    out/test/suite/control-studio-experiment.test.js --grep "builds relay as
    measured feedback instead of a timed square wave"` first failed because
    the planner returned a 500-millisecond alternating trajectory and then
    passed on `trust-builder` with an empty trajectory and a typed PV center,
    upper/lower threshold, hysteresis, and high/low MV contract. The runtime
    command `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum
    cargo test -p trust-runtime
    control_studio_relay_switches_from_process_feedback_and_never_from_elapsed_dwell
    -- --nocapture` first failed at the expected assertion because
    `relay_feedback` was unknown, then passed: elapsed time caused no switch,
    threshold crossings switched high/low, the hysteresis band retained state,
    and abort restored the original typed value.
  - Completed evidence (2026-08-30): the 3,840-row trace and 2-row decision
    summary retain qualified/rejected records, 130 threshold crossings, exact
    assumptions, failure criteria, derived unapplied PI candidate, and
    restoration. A real Extension Development Host journey passed in 59 s with
    both feedback states, live values, available Abort, `user_abort`, and
    `restoration=restored`; independent review accepted the current screenshots.
- [x] `CELP-054` Lesson 44 — Lambda/IMC and SIMC. Teach response target, FOPDT/
  integrating eligibility, model dependence and uncertainty before calculation.
  Prove candidate behavior on heater, tank and dryer without selecting a winner
  before stability and robustness analysis.
  - Specification authority completed (2026-08-29):
    `docs/specs/38-control-engineering-curriculum.md` Lesson 44 defines the
    self-regulating and integrating model domains, words-first quantities and
    units, Fast/Balanced/Robust targets, worked heater/tank/dryer calculations,
    controlled variations, exact metric validity, simulation boundary, and
    causal handoff to Lesson 45. `docs/specs/35-vscode-control-studio.md` §9.1
    defines integrating SIMC eligibility, equations, presets, rejection, and
    frequency-response treatment.
  - Native expected red/green (2026-08-29, `trust-builder`):
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum cargo test
    -p trust-runtime --test control_curriculum_tuning_progression
    control_curriculum_simc_targets_are_model_qualified_and_remain_unapplied
    -- --exact --nocapture` first reached the expected assertion that
    `examples/control/06-pid-tuning-methods/44-model-based-tuning/lesson.json` was missing.
    The same command then passed after the executable three-plant project was
    added. Measured Fast/Balanced/Robust 10-to-90-percent rise times were
    heater `3.1/6.9/14.0 s`, integrating tank `1.3/1.8/3.0 s`, and dryer
    `15.2/31.5/65.5 s`; nominal, 20-percent-gain, and 50-percent-delay cases
    remained finite and actuator-bounded, and metadata kept application false.
  - Learner expected red/green (2026-08-29, `trust-builder`):
    `./node_modules/.bin/tsc -p ./ && ./node_modules/.bin/mocha --ui tdd
    out/test/suite/control-lesson-pid-production.test.js --grep "teaches
    model-qualified SIMC targets across self-regulating and integrating
    plants"` first failed at its expected missing-project assertion and then
    passed with the schema-version-4 catalog entry, all ten pedagogical stages,
    actual three-instance `Control.PID` program, three simulator plants,
    requested/applied distinction, worked formulas, defined measures,
    uncertainty cases, and explicitly unapplied candidates.
  - Affected-file gates (2026-08-29): the complete native tuning-progression
    test file passed `3/3`; the complete production-PID curriculum extension
    test file passed `7/7`; `python3 scripts/check_vscode_test_registration.py`
    passed with `753` discovered facts and `66/66` registered files.
  - Completed evidence (2026-08-30): the 19,800-row trace and 27-row summary are
    rectangular across all candidates and uncertainty variations, preserve
    physical and normalized signals, exact assumptions and qualification, and
    keep every candidate unapplied. Source/bundle parity, rendered plot quality,
    and independent acceptance passed.
- [x] `CELP-M06` Module 6 gate — Retain the manual/historical, relay-derived,
  and model-based candidates with their exact assumptions for Lessons 42–44.
  Compare method applicability and evidence quality, and confirm every tuning
  output remains unapplied.
  - Accepted (2026-08-30): the independent reviewer accepted all three lesson
    artifacts and the focused real Control Studio relay journey. The unchanged
    Identify/Tune/Validate and method-comparison stages reuse the already
    accepted Module 5 journey; no tuning is silently applied, and all evidence
    is explicitly simulation-only.

## 8. Classical stability and frequency-domain design

Specification authority: §3.7, Lessons 45–50.

- [x] `CELP-055` Lesson 45 — Crane poles and Routh-Hurwitz. Teach characteristic
  equation, asymptotic stability, Routh array and special cases before analysis.
  Prove the calculated stable gain range with stable, marginal and unstable runs.
  - Specification refined (2026-08-29): Lesson 45 now defines the physical
    crane loop, words-first pole and half-plane theory, both Routh special
    cases, the complete array for `s^3 + 2*s^2 + 3*s + K`, the predicted
    interval `0 < K < 6`, equivalent `K = 4/6/8` executable cases, exact
    envelope and actuator measures, numerical-root oracle, abort boundary, and
    causal handoff to root locus. The former draft project and broad
    source-text assertion were not accepted as evidence for that contract.
  - Native expected red/green (2026-08-29, `trust-builder`):
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum cargo test
    -p trust-runtime --test control_curriculum_classical
    control_curriculum_routh_range_predicts_crane_stable_marginal_and_unstable_runs
    -- --exact --nocapture` first reached the expected assertion because the
    draft contained one plant instead of equivalent `K=4/6/8` crane copies.
    The same command passed after implementation. The independent numerical
    roots found no right-half-plane pole for `K=4`, two imaginary-axis poles
    for `K=6`, and two right-half-plane poles for `K=8`; measured late/early
    position-envelope ratios were `0.0078`, `1.1232`, and `4.6053`.
  - Learner expected red/green (2026-08-29, `trust-builder`):
    `./node_modules/.bin/tsc -p ./ && ./node_modules/.bin/mocha --ui tdd
    out/test/suite/control-lesson-selection-modelling.test.js --grep "teaches
    the crane Routh stability interval before time-domain confirmation"`
    first rejected the schema-version-2 draft and then passed with the
    schema-version-4 descriptor, catalog entry, ten-stage README, words-first
    pole and half-plane theory, both Routh special cases, explicit
    proportional paths, three equivalent plants, defined measures, and the
    physical-crane prohibition.
  - Affected-file gates (2026-08-29): the complete native classical test file
    passed `2/2`; the combined modelling-lesson and curriculum-catalog files
    passed `23/23`; VS Code registration remained green with `66/66` files.
    The catalog retains exactly `84` control lesson identities. The crane uses
    a declared 10-millisecond motion-control interval because a
    100-millisecond sampled feedback delay materially shifts the continuous
    marginal boundary; discrete-time consequences remain assigned to Lesson
    50 rather than being hidden here.
  - Completed evidence (2026-08-30): retained pole, position, requested-force
    and applied-force data cover `K=4/6/8`; the source and shipped bundle match;
    the real VS Code lesson and shared Control Studio journeys passed; current
    screenshots are readable and unclipped; independent review accepted the row.
- [x] `CELP-056` Lesson 46 — Servo root locus. Teach branches, asymptotes,
  breakaway, damping lines, dominant poles and compensator zeros before design.
  Prove when gain is sufficient and when lead/lag structure is required.
  - Specification authority completed (2026-08-29): Lesson 46 now defines the
    physical positioning servo, simultaneous overshoot/settling/accuracy/current
    requirements, every root-locus construction term in words before symbols,
    the gain-only locus and failure, explicit angle/magnitude design, complete
    transfer-function qualification, exact metrics, and the causal handoff to
    Bode analysis.
  - Native expected red/green (2026-08-29, `trust-builder`):
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum cargo test
    -p trust-runtime --test control_curriculum_classical
    control_curriculum_root_locus_lead_design_meets_servo_damping_and_time_requirements
    -- --exact --nocapture` first reached the expected assertion because the
    draft had one generic servo instead of equivalent proportional and lead
    cases. The initial `80*(s+4)/(s+20)` implementation was then correctly
    rejected at the response assertion with `13.31%` overshoot: dominant-pole
    damping alone had hidden the closed-loop zero. The redesigned
    `50*(s+2.5)/(s+15)` case passed the same unchanged requirements.
  - Measured native evidence (2026-08-29): proportional gain four produced
    `16.70%` overshoot and `4.06 s` settling; lead compensation produced
    `3.35%` overshoot, `1.38 s` settling, less than `0.0001 mm` final-window
    error, and no saturation. Independent roots were
    `-3.235 +/- j1.186` and `-10.530`.
  - Learner expected red/green (2026-08-29, `trust-builder`):
    `./node_modules/.bin/tsc -p ./ && ./node_modules/.bin/mocha --ui tdd
    out/test/suite/control-lesson-selection-modelling.test.js --grep "teaches
    why servo gain alone fails and designs lead compensation from root locus"`
    first rejected the schema-version-2 draft, then passed with the catalogued
    schema-version-4 lesson, ten-stage README, actual two-path ST, two
    equivalent plants, definitions and arithmetic, complete-transfer-function
    correction, exact measures, and physical-plant boundary.
  - Affected-file gates (2026-08-29): complete native classical tests passed
    `3/3`; combined modelling lesson and catalog tests passed `24/24`; VS Code
    registration passed with `755` facts and `66/66` registered files; the
    catalog retains exactly `84` control identities.
  - Completed evidence (2026-08-30): the final evidence contract first failed
    because requested-current samples were incorrectly zero. After binding the
    executable requested variables, the same contract passed; P and lead peaks
    are `4%` and `50%`. Real root-locus/response rendering, source/bundle parity,
    shared Control Studio coverage and independent review all passed.
- [x] `CELP-057` Lesson 47 — Flexible-drive Bode analysis. Teach sinusoidal
  response, magnitude, phase, crossover, bandwidth, resonance and gain/phase/
  delay margins before plotting. Prove why similar step responses can have
  materially different robustness and noise behavior.
  - Specification authority completed (2026-08-29): Lesson 47 now defines the
    flexible-shaft motor and load, physical units and limits, words-before-
    symbols frequency-response theory, the exact continuous model, three
    worked magnitude/phase predictions, all-crossing margin rules, requested
    versus applied torque, the simulator timestamp reference, an explicitly
    unapplied resonance-shaped candidate, perturbation checks, and the causal
    handoff to Nyquist analysis. The corrected gain-five baseline has one gain
    crossover near `2.99 rad/s`, not the two incorrectly stated by the draft.
  - Native expected red/green (2026-08-29, `trust-builder`):
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum cargo test
    -p trust-runtime --test control_curriculum_classical
    control_curriculum_bode_measurement_exposes_flexible_drive_resonance_and_margins
    -- --exact --nocapture` first compiled and reached the expected assertion
    failure because the schema-version-2 draft exposed one plant rather than
    equivalent low-, resonant-, and high-frequency copies. The same focused
    test then passed against the three executable 10-millisecond plants.
  - Measured native evidence (2026-08-29): the fitted magnitude/phase pairs
    were `0.26657 rpm/%`, `-1.5275 degrees` at `0.5 rad/s`;
    `2.49898 rpm/%`, `-89.9946 degrees` at `2 rad/s`; and
    `0.016657 rpm/%`, `-178.4723 degrees` at `8 rad/s`. The independent margin
    oracle found the unshaped crossover/margin at `2.994 rad/s`,
    `6.878 degrees`, and shaped crossover/margin at `1.443 rad/s`,
    `112.554 degrees`. The raw high-frequency result exposed the simulator's
    known half-sample timestamp alignment; the specification, README, and
    native comparison now declare and retain that correction instead of
    silently widening tolerance.
  - Learner expected red/green (2026-08-29, `trust-builder`): after a clean
    TypeScript compile,
    `mocha --ui tdd out/test/suite/control-lesson-selection-modelling.test.js
    --grep "measured Bode resonance"` first rejected the old schema-version-2
    Nyquist draft, then passed with the catalogued schema-version-4 Bode lesson,
    ten-stage pedagogical README, actual three-probe Structured Text, exact
    measures, Control Studio sequence, and Simulation/physical boundary.
  - Affected-file gates (2026-08-29): complete native classical tests passed
    `4/4`; combined modelling-lesson and catalog tests passed `25/25`; VS Code
    registration passed with `756` discovered facts and `66/66` registered
    files; the catalog retains exactly `84` control identities by replacing
    the superseded sequence-47 draft slot.
  - Completed evidence (2026-08-30): retained low/resonant/high probe runs,
    measured and model frequency response, unshaped/shaped controller Bode
    figures, decision records and perturbations are complete. Source/bundle
    parity, real VS Code and shared Control Studio journeys, responsive evidence
    and independent review passed.
- [x] `CELP-058` Lesson 48 — Delayed-process Nyquist stability. Teach complex
  response, `-1`, encirclement, open-loop pole count and delay phase before
  plotting. Prove stable and unstable predictions including a case where a
  simple Bode-margin reading is inadequate.
  - Specification authority completed (2026-08-29): Lesson 48 now uses a
    continuous conveyor curing oven with a downstream infrared sensor and
    three equivalent first-order thermal plants whose only changed quantity is
    transport delay (`5 s`, `25 s`, and `60 s`). It defines complex response,
    the complete positive/negative-frequency contour, traversal and winding
    convention, the minus-one critical point, open- and closed-loop
    right-half-plane pole counts, `Z = P - N_ccw`, pure-delay phase, numerical
    rejection conditions, the proportional-loop equilibrium, requested versus
    applied heater command, abort behavior, exact comparison windows, and the
    Monitor–Test–Identify–Tune–Validate workflow before practical action.
  - Numerical expected red/green (2026-08-29, `trust-builder`): after the
    existing numerical adapter and dependencies were synchronized into the
    isolated validation checkout, `npx mocha --ui tdd
    out/test/suite/control-studio-numerics.test.js --grep "counts delayed-oven
    Nyquist encirclements"` reached the expected assertion and rejected the
    valid five-second-delay model as `nonfinite-input` because the adapter did
    not support transport delay. The same test passed after the adapter added
    delayed transfer evaluation, contour-pole, critical-point-distance and
    endpoint-closure qualification, normalized winding counts, and the
    declared `Z = P - N_ccw` convention. Invalid poles on the contour remain
    fail-closed instead of receiving a stability verdict.
  - Native expected red/green (2026-08-29, `trust-builder`):
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum cargo test
    -p trust-runtime --test control_curriculum_classical
    control_curriculum_nyquist_count_predicts_delayed_oven_stability -- --exact
    --nocapture` first reached the expected assertion failure because the old
    draft exposed one unrelated plant rather than three equivalent delayed
    ovens. The paired green proved crossover phases of `-84.810`, `-184.049`,
    and `-357.718 degrees`, plus early/late equilibrium-deviation envelopes of
    `5.192e-8/9.948e-14`, `0.852/1.100`, and `2.020/10.252 degrees Celsius`.
    This confirms decay for the five-second loop and growth beyond the declared
    `1.20` ratio for both unstable loops without confusing the stable P loop's
    `0.333 degrees Celsius` offset with instability.
  - Learner expected red/green (2026-08-29, `trust-builder`): after a clean
    TypeScript compile, `mocha --ui tdd
    out/test/suite/control-lesson-selection-modelling.test.js --grep "complete
    Nyquist stability"` first rejected the old schema-version-2 boiler-drum
    draft, then passed against the schema-version-4 delayed-oven project,
    actual three-path Structured Text, complete words-before-symbols theory,
    worked predictions, validity rules, physical/simulation boundary, and the
    causal handoff to sensitivity and complementary sensitivity. Prose-order
    assertions were split into semantic facts when they falsely rejected the
    correct table and explanation ordering.
  - Affected-file gates (2026-08-29): the complete native classical suite
    passed `5/5`; the combined numerical-adapter, modelling-lesson and catalog
    suites passed `38/38`; VS Code test registration passed with `758`
    discovered facts and `66/66` registered files; the catalog retains exactly
    `84` control identities by replacing the superseded sequence-48 draft.
  - Completed evidence (2026-08-30): the retained Nyquist contours include the
    marked minus-one point and traversal direction, time responses and separate
    requested/applied heater records. Source/bundle parity, real VS Code and
    shared Control Studio journeys, responsive evidence and independent review
    passed.
- [x] `CELP-059` Lesson 49 — Motor sensitivity and loop shaping. Teach `S`, `T`,
  disturbance/noise/control sensitivity, bandwidth, roll-off and waterbed
  trade-off before design. Prove time- and frequency-domain requirements
  together.
  - Specification authority completed (2026-08-29): Lesson 49 now uses two
    equivalent precision motors with compliant loads, separate controller and
    opposing-load torque inputs, noisy measured-speed and independent true-
    speed channels, a one-rpm request, identical `10 rad/s` encoder
    interference, and a common `0.1 percent` load step. It introduces the
    physical paths before `L`, `S`, `T`, `GS`, and `CS`; defines bandwidth,
    `M_s`, `M_t`, roll-off and the qualified waterbed effect; derives the
    proportional and proper lead-lag-roll-off controllers; and predeclares all
    frequency, time, validity, limit, and Simulation/physical boundaries.
  - Numerical expected red/green (2026-08-29, `trust-builder`): after a clean
    TypeScript compile, `mocha --ui tdd
    out/test/suite/control-studio-numerics.test.js --grep "precision-drive
    sensitivity"` reached the expected assertion failure because
    `analyzeLoopShape` was absent. The paired green proves both controller paths
    on one qualified grid, including baseline/shaped `M_s` of `3.197/1.724`,
    `M_t` of `3.009/1.334`, crossovers of `2.187/1.706 rad/s`, phase margins of
    `19.13/44.11 degrees`, relative-minus-three-decibel bandwidths of
    `3.43/2.83 rad/s`, and the declared low- and high-frequency `S`, `T`, `GS`,
    and `CS` values. Plant and controller remain separate so load and encoder
    paths cannot be conflated.
  - Native expected red/green (2026-08-29, `trust-builder`):
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum cargo test
    -p trust-runtime --test control_curriculum_classical
    control_curriculum_loop_shaping_balances_motor_sensitivity_and_effort --
    --exact --nocapture` first reached the expected assertion because the old
    placeholder exposed one motor instead of equivalent baseline and shaped
    copies. The paired green executes the actual Structured Text filters and
    measured plant paths: baseline/shaped overshoot was `66.41/19.16 percent`,
    pre-load settling `13.01/15.13 s`, worst post-load equilibrium departure
    `0.200/0.323 rpm`, and final quiet-window torque travel `1.058/0.631`
    percentage points. The evidence intentionally retains the shaped loop's
    slower load recovery instead of presenting robustness as a free benefit.
  - Learner expected red/green (2026-08-29, `trust-builder`): after a clean
    TypeScript compile, `mocha --ui tdd
    out/test/suite/control-lesson-selection-modelling.test.js --grep
    "sensitivity trade-offs"` first rejected the schema-version-2 placeholder
    and then passed against the schema-version-4 project, 1,524-word
    eleven-stage README, actual two-path Structured Text, equivalent two-input/
    two-output state-space simulations, exact measures, Control Studio flow,
    and causal handoff to z-plane analysis.
  - Affected-file gates (2026-08-29): the complete native classical suite
    passed `6/6`; combined numerical-adapter, modelling-lesson and catalog
    suites passed `40/40`; VS Code registration passed with `760` discovered
    facts and `66/66` registered files; the catalog retains exactly `84`
    control identities by replacing the superseded sequence-49 draft.
  - Completed evidence (2026-08-30): retained `L/S/T/GS/CS`, speed response,
    requested/applied torque and decision data show the frequency/time/effort
    trade-off together. Source/bundle parity, real VS Code and shared Control
    Studio journeys, responsive evidence and independent review passed.
- [x] `CELP-060` Lesson 50 — Position-system z-plane stability. Teach z-transform,
  unit circle, discrete poles/zeros, mapping, sampling zeros, aliasing and delay
  before analysis. Prove a sample time that invalidates the continuous design
  and a corrected discrete implementation.
  - Specification authority completed (2026-08-29): Lesson 50 now uses three
    equivalent industrial positioning tables with common continuous mechanics
    and `10 ms` plant integration. It compares unchanged gains at `10 ms`, the
    same gains at `100 ms`, and corrected gains at `100 ms`; every digital path
    explicitly applies its preceding request and holds force between updates.
    The lesson defines samples, sequences, the z-transform, `z^-1`, discrete
    poles, the unit circle, exact `z=exp(sT_s)` mapping, zero-order hold,
    sampling zeros, Nyquist angular frequency, aliasing and discrete root locus
    before equations or motion. Exact matrices, limits, requested/pending/
    applied force, bounded controlled stop, and Simulation/physical boundaries
    are part of the authority.
  - Native expected red/green (2026-08-29, `trust-builder`):
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum cargo test
    -p trust-runtime --test control_curriculum_classical
    control_curriculum_z_plane_predicts_sampled_axis_failure_and_recovery --
    --exact --nocapture` first reached the expected assertion because the old
    placeholder exposed one anonymous state-space plant instead of fine,
    coarse-baseline, and corrected-coarse copies. The paired green executes all
    three actual Structured Text paths and proves largest augmented-pole
    magnitudes `0.971845`, `1.041260`, and `0.743613`, sampling zeros
    `-0.993356` and `-0.935525`, the `22.831853 rad/s` coarse alias, bounded
    abort/controlled stop only for the unstable coarse baseline, no abort for
    fine or corrected cases, force within limits, and final qualified position
    error below `0.02 mm`.
  - Numerical behavior-lock proof (2026-08-29, `trust-builder`): after the
    written lesson contract and before any numerical-adapter change, `mocha
    --ui tdd out/test/suite/control-studio-numerics.test.js --grep "executed
    positioning-axis"` passed against the existing qualified discrete-state-
    space analyzer. It independently checks the exact fine, unchanged-coarse,
    and corrected-coarse augmented matrices and pole sets plus analytical
    sampling-zero and alias oracles; no additional numerical production path
    was needed.
  - Learner expected red/green (2026-08-29, `trust-builder`): after a clean
    TypeScript compile, `mocha --ui tdd
    out/test/suite/control-lesson-selection-modelling.test.js --grep
    "sampled-axis stability"` first rejected the old schema-version-2 abstract
    pole example, then passed against the schema-version-4 project, 1,355-word
    eleven-stage README, actual delayed controller implementation, three
    equivalent continuous plants, exact predictions, Control Studio sequence,
    controlled-stop behavior, and causal handoff to tuning-method comparison.
  - Affected-file gates (2026-08-29): the complete native classical suite
    passed `7/7`; combined numerical-adapter, modelling-lesson and catalog
    suites passed `42/42`; VS Code registration passed with `762` discovered
    facts and `66/66` registered files; the catalog retains exactly `84`
    control identities by replacing the superseded sequence-50 draft.
  - Completed evidence (2026-08-30): exact matrices, sampled poles, unit circle,
    corrected root locus, position, and requested/pending/applied force are
    retained. The final contract first exposed the missing corrected pending
    series and then passed after the native evidence was regenerated with all
    `1,000` samples. Source/bundle parity, real rendering and independent review
    passed.
- [ ] `CELP-M07` Module 7 gate — Independently recompute representative Routh,
  root-locus, Bode, Nyquist, sensitivity and z-plane results against qualified
  numerical oracles and declared tolerances. Capture every rendered plot type.
  - Accepted (2026-08-30): the complete native classical suite passed `7/7`;
    the strengthened shipped-evidence contract passed `1/1`; all six real VS
    Code lesson journeys passed; and the shared Control Studio journey passed
    `1/1` in `99.927 s`, covering Monitor/Test/Identify/Tune/Validate, automatic
    tuning, failures/recovery, explicit physical approval, persistence, dark,
    light, high contrast, wide, split, narrow, 200-percent zoom and keyboard
    operation. Current proof and exact commands are recorded in
    `docs/internal/testing/evidence/vscode-control-curriculum/2026-08-30-module-07/acceptance.md`.
    Independent review accepted all rows and confirmed that separate per-lesson
    Control Studio implementations would be unnecessary duplication. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**

## 9. Tuning qualification and automatic tuning

Specification authority: §3.8, Lessons 51–52.

- [x] `CELP-061` Lesson 51 — Heater/tank/dryer tuning comparison. Teach method
  eligibility, time metrics, margins, sensitivity peaks, actuator variation,
  Pareto trade-offs and inconclusive outcomes before ranking. Prove all
  candidates against one common retained validation plan without hidden scores.
  - Specification and source implementation (2026-08-29): §3.8 now defines the
    complete eleven-stage lesson, exact SIMC/AMIGO domains and formulas,
    plant-specific requirements, IAE/ISE/ITAE, requested/applied output, total
    variation, margins, delay margin, sensitivity peaks, visible scoring and
    Pareto rules, and `Unavailable`, `Inconclusive`, and `No acceptable
    candidate` outcomes. The new canonical project is
    `examples/control/08-tuning-qualification-and-automation/51-method-comparison`: five real `Control.PID` paths
    drive equivalent heater, integrating-tank, and dryer copies. AMIGO is not
    instantiated for the integrating tank, and all candidates remain
    unapplied Simulation evidence.
  - Native expected red/green (2026-08-29, `trust-builder`):
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum cargo test
    -p trust-runtime --test control_curriculum_tuning_methods
    control_curriculum_tuning_methods_separate_eligibility_from_tradeoffs --
    --exact --nocapture` first reached the expected assertion because the
    Lesson-51 project was absent. The paired green executed 2,200 samples for
    each of five real candidates. Heater SIMC/AMIGO IAE was `27.44/23.87
    degC*s` with `93.34/102.48` percentage points of applied movement; dryer
    SIMC/AMIGO IAE was `167.11/240.63 dryness-%*s` with `33.94/23.04`
    percentage points of movement. Tank SIMC finished at `0.0135` level-percent
    mean absolute error while its AMIGO status remained `Unavailable`. All
    outputs were finite and bounded; the test also exposed and fixed an
    overlapping `%MX96.0`/`%ML96` address before accepting the result.
  - Learner expected red/green and numerical proof (2026-08-29,
    `trust-builder`): after direct TypeScript compilation, `mocha --ui tdd
    out/test/suite/control-lesson-tuning-methods.test.js --grep "compares only
    model-eligible"` first rejected the absent project, then passed against the
    schema-version-4 project and 1,990-word pedagogical README. The complete
    three-test suite also reproduced every formula within `1e-7`, retained the
    tank unavailable reason, and independently qualified gain, phase and delay
    margins, bandwidth, and maximum sensitivity on `0.0001..100 rad/s` at 100
    points per decade. Heater AMIGO has lower error but smaller margins and
    larger sensitivity; dryer AMIGO has lower bandwidth and actuator movement
    but larger margins and higher time error. A second focused assertion red
    then exposed cumulative movement recorded as instantaneous `%`; the paired
    green added the `pp` lesson-contract unit and changed both movement
    acceptance rows to percentage points.
  - Focused affected-file gates (2026-08-29): the new lesson suite passed
    `3/3`; adjacent SIMC/AMIGO identification-and-tuning checks passed `5/5`;
    combined lesson, numerical-adapter, and currently implemented catalog tests
    passed `28/28`; VS Code registration passed with `765` discovered facts and
    `67/67` registered files.
  - The row remains open for retained machine-readable run traces and generated
    figures, source-to-bundle synchronization, the real rendered Control Studio
    journey, responsive/accessibility evidence, and independent learner/UX
    review. The pre-existing source catalog sequence-39 collision must also be
    resolved before the full example-bundle build can be accepted.
  - Accepted (2026-08-30): the retained 2,200-sample common-plan record now
    includes every method-domain decision, physical plant-specific requirement,
    IAE/ISE/ITAE, settling/dwell, requested/applied peaks, movement, maximum
    rate, saturation duration, noise RMS, margins, sensitivity, bounded
    variation and separate scenario disposition. Source/bundle parity is exact,
    focused lesson tests pass `5/5`, native tests pass `2/2`, and independent
    learner/engineering review accepted the lesson.
- [x] `CELP-062` Lesson 52 — Complete water-heater Automatic Tune. Teach every
  qualification, approval, identification, selection, robustness, application
  and rollback decision before starting. Prove success, invalid data, unsafe
  request, abort, identification failure, tuning failure, restoration and an
  explicitly unapplied result.
  - Specification and source implementation (2026-08-29): §3.8 now defines the
    exact-Simulation reuse route separately from observed and physical routes,
    including immutable plan content, evidence qualification, stage ownership,
    model verification versus identification, candidate retention, common
    validation, failure prefixes, abort/restoration, Apply authority, read-back
    and rollback boundaries. The canonical
    `examples/control/08-tuning-qualification-and-automation/52-automatic-water-heater` project runs a confirmed
    Robust PI and an unapplied Balanced PI on independent equivalent heater
    copies; no orchestration verdict or controller write is embedded in ST.
  - Native expected red/green (2026-08-29, `trust-builder`):
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum cargo test
    -p trust-runtime --test control_curriculum_tuning_methods
    control_curriculum_automatic_tune_validates_without_replacing_the_running_heater
    -- --exact --nocapture` first reached the expected missing-project
    assertion. The paired green executed 2,200 samples on both heater copies.
    Current Robust versus candidate Balanced IAE was `47.00/27.44 degC*s`,
    applied movement was `52.64/93.34 pp`, and final mean absolute error was
    `0.0126/0.0126 degC`; every heater output remained finite and bounded.
  - Learner expected red/green (2026-08-29, `trust-builder`): after direct
    TypeScript compilation, `mocha --ui tdd
    out/test/suite/control-lesson-tuning-methods.test.js --grep "complete
    automatic heater sequence"` first rejected the absent project, then passed
    against the schema-version-4 lesson, 1,345-word README, two real PID paths,
    two plant copies and canonical catalog entry. It also executes the existing
    Automatic Tune service: exact model reuse produces the ordered four-stage
    result, Balanced `Kp=5.7692308`, `Ti=5 s`, about `13.5 s` model settling,
    all requirements passing, all method attempts retained, and `applied=false`.
    Missing-plant and negative-gain fixtures stop at Test and Identify with the
    correct completed-stage prefixes and recovery text.
  - Existing-service failure and restoration behavior lock (2026-08-29,
    `trust-builder`): the registered lesson test `stops automatic tuning at
    unsafe data tuning validation and restoration boundaries` passed after the
    broken test insertion was corrected so the assertion actually compiled and
    ran. It directly exercises the existing experiment-plan, Automatic Tune,
    candidate-validation and restoration-state services: stale and bad-quality
    data, an out-of-range request and an unreviewed physical topology are
    rejected; a finite but numerically unusable model stops after
    `Test, Identify` at `Tune`; a no-acceptable-candidate case stops after
    `Test, Identify, Tune` at `Validate`; the running-controller object remains
    byte-for-byte unchanged; and user-abort, automatic-limit-abort, restored and
    failed-restoration states remain distinct. Exact command: `ssh
    trust-builder 'cd /home/johannes/projects/trust-platform-control-curriculum-validation/editors/vscode
    && ./node_modules/.bin/tsc -p . && node
    ./node_modules/mocha/bin/mocha --ui tdd
    out/test/suite/control-lesson-tuning-methods.test.js --grep "stops automatic
    tuning"'` (`1 passing`). This is an honest green behavior lock over existing
    services, not a manufactured red or a new parallel orchestration path.
  - Runtime-native abort/restoration evidence (2026-08-29, `trust-builder`):
    focused `cargo test -p trust-runtime --lib <test-name> -- --nocapture` runs
    each passed one executed assertion for
    `control_studio_experiment_starts_once_and_abort_restores_exact_value`,
    `control_studio_abort_preempts_future_excitation_and_confirms_restoration_from_io`,
    `control_studio_runtime_data_trip_stops_and_failed_restoration_is_explicit`,
    and `control_studio_running_experiment_trips_on_force_and_staleness`. These
    prove queued exact-value restoration, no late excitation after abort,
    observed successful restoration, automatic force/staleness stops, and
    terminal `failed_to_restore` after an out-of-limit trip. The first broad
    integration-test-shaped attempt was discarded as evidence because an
    unrelated `control_curriculum_motion_progression.rs` compile error prevented
    execution; an intermediate `--exact` attempt was also discarded because it
    ran zero tests. Five unrelated pre-existing `trust-hir` unused-import
    warnings remain visible.
  - The focused source behavior is now covered. The row remains open for
    retained run artifacts, generated figures, source-to-bundle
    synchronization, real rendered Control Studio success/failure journeys,
    responsive/accessibility evidence and independent review.
  - Accepted (2026-08-30): the orchestration record retains the complete
    Kp/Ti/Ki recommendation with units, exact successful stage/count/revision
    evidence, unapplied state, every returned failure prefix/reason/recovery,
    and an explicit pure-service execution boundary. Classifier examples are
    labelled rather than presented as observations; four linked runtime-native
    tests own abort, output-stop, restoration read-back, force/staleness and
    failed-restoration proof and pass in the current `11/11` runtime batch.
    Independent review confirmed that no synthesized execution evidence remains.
- [ ] `CELP-M08` Module 8 gate — Run the full rendered Automatic Tune journey on
  simulation. Verify that physical execution remains separately gated and that
  no gains change without immediate explicit approval.
  - Accepted (2026-08-30): the exact real VS Code journey shows the saved heater
    model, ordered Test/Identify/Tune/Validate stages, SIMC PI gains, visible
    7.4-percent-versus-10-percent selection explanation, passing validation and
    unapplied result. Shared rendered evidence covers failure/recovery, physical
    approval, no silent apply, themes, responsive layouts, zoom and keyboard
    operation. Commands, screenshots and independent acceptance are recorded in
    `docs/internal/testing/evidence/vscode-control-curriculum/2026-08-30-module-08/acceptance.md`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**

## 10. Industrial control structures

Specification authority: §3.9, Lessons 53–60.

- [x] `CELP-063` Lesson 53 — Conveyor-oven feedforward. Teach disturbance and
  process paths, static/dynamic compensation, feedback trim, model mismatch and
  internal constraint ordering before design. Prove feedback-only versus
  feedforward-plus-feedback behavior under equal disturbances.
  - Specification and source implementation (2026-08-29): §3.9 now applies the
    complete pedagogical pattern to a continuous conveyor oven with separate
    heater and measured-speed paths. It defines SP, PV, error, requested and
    applied MV, disturbance, both path gains and time constants, static and
    proper dynamic feedforward, PI trim, constraint ordering, quality fallback,
    mismatch, exact metric windows and settling eligibility before practice.
    The schema-version-4 canonical project runs three real `Control.PID`
    controllers on three equivalent two-input state-space ovens; a shipped
    `Control.LowPass` implements the dynamic path model. PLC code contains only
    the controller calculation and concise design-intent comments, not metrics
    or a preferred-case verdict.
  - Native expected red/green (2026-08-29, `trust-builder`): exact command
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum cargo test
    -p trust-runtime --test control_curriculum_industrial_structures
    control_curriculum_feedforward_compensates_measured_oven_speed_before_feedback_error
    -- --exact --nocapture` first reached the expected assertion because the
    old lesson still used schema version 2. After implementation it reached the
    numerical comparison, exposed and corrected one-scan dynamic-filter order,
    and passed. Feedback-only, static and dynamic event IAE was respectively
    `122.2082`, `48.1326` and `0.6035 degC*s`; peak temperature deviation was
    `9.0724`, `4.1426` and `0.0520 degC`; applied-heater travel was `12.2926`,
    `18.5362` and `39.9940 pp`. The dynamic feedforward begins at `25 pp` and
    converges to `10 pp` without saturation. A 0.8-scale mismatch produced
    `24.3210 degC*s` IAE and `0.0024 degC` final-window error, still better than
    feedback-only because PI trim remains active. Invalid speed quality makes
    both feedforward paths exactly match feedback-only. The static case's
    `0.1159 degC` final-window span fails the declared `0.05 degC` settling rule
    and is honestly classified `Not settled`; its last sample is not used as
    steady-state evidence.
  - Learner expected red/green (2026-08-29, `trust-builder`): registered test
    `teaches conveyor-oven feedforward from measured paths before
    implementation` first rejected the old schema-version-2 summary, then
    passed against the full README, actual ST, three plant declarations,
    words-before-symbols equations, predictions, exact measures, mismatch,
    invalid-data fallback and causal transfer to cascade. VS Code registration
    passes with `768` discovered facts and `68/68` registered files.
  - Current retained/rendered evidence (2026-08-30): the owning test generates
    and then exactly revalidates its SVG, trace CSV and summary CSV. The
    canonical source and 90-example bundle are synchronized; the real gallery/
    copy/render journey passes and is independently accepted in the Module 9
    evidence record.
- [x] `CELP-064` Lesson 54 — Reactor cascade control. Teach primary/secondary
  variables, time-scale separation, authority, saturation propagation, tracking
  and inner-first tuning before implementation. Prove why the secondary loop
  must be faster and see the disturbance sooner.
  - Specification: `docs/specs/38-control-engineering-curriculum.md`, Lesson 54,
    now defines the jacketed-reactor physics, loop ownership, inner-first
    commissioning, tracking, time-scale criterion, controlled comparisons,
    exact decision measures, validity rules and causal transfer to ratio control.
  - Native expected red: on `trust-builder`, the focused command
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum cargo test -p trust-runtime --test control_curriculum_industrial_structures control_curriculum_cascade_commissions_inner_loop_before_rejecting_reactor_utility_disturbance -- --exact --nocapture`
    reached the intended assertion and failed because the shipped descriptor
    was schema version `2` rather than the specified version `4`.
  - Native paired green: the same focused command passed. The fast inner loop
    settled in `3.100 s`; the deliberately slow loop did not settle in the
    `30 s` evaluation interval; jacket detection led product detection by
    `0.800 s`; direct, commissioned-cascade and slow-cascade reactor event IAE
    were `35.468`, `5.192` and `110.144 degC*s`; peak deviations were `2.441`,
    `0.382` and `4.225 degC`; and every valve request remained finite and within
    `0..100 percent`.
  - Learner-contract expected red: after compiling the registered TypeScript
    test directly to avoid an unrelated duplicate-sequence catalog failure,
    `TRUST_REPOSITORY_ROOT=$HOME/projects/trust-platform-control-curriculum-validation node ./node_modules/mocha/bin/mocha --ui tdd out/test/suite/control-lesson-industrial-structures.test.js --grep "teaches inner-first reactor cascade"`
    reached the intended contract assertion and rejected the old schema-2
    summary.
  - Learner-contract paired green: the same Mocha command passed `1` test after
    the schema-4 descriptor, causal README, five-controller Structured Text and
    three-plant simulation were implemented. The test proves words before
    symbols, actual equations and ST, inner-first Control Studio steps, defined
    measures, settling eligibility, requested/applied distinction and the
    Lesson-55 handoff.
  - Current retained/rendered evidence (2026-08-30): commissioning and utility-
    event evidence is retained in the SVG, full trace CSV and summary CSV and
    passes no-update drift comparison. The synchronized bundled lesson passes
    its real gallery/copy/render journey and independent review.
- [x] `CELP-065` Lesson 55 — Blending ratio control. Teach wild/controlled
  streams, ratio station, scaling, low-flow singularity and feedback trim before
  operation. Prove ratio maintenance, meter-bias behavior and safe low-flow
  fallback.
  - Specification: `docs/specs/38-control-engineering-curriculum.md`, Lesson 55,
    now defines the two material streams, ratio versus final-mixture fraction,
    analyser trim, local material-balance model, scaling qualification,
    low-flow authority, exact experiments, measures and causal Lesson-56 handoff.
  - Native expected red: on `trust-builder`,
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum cargo test -p trust-runtime --test control_curriculum_industrial_structures control_curriculum_ratio_station_moves_with_wild_flow_and_fails_safe_at_low_flow -- --exact --nocapture`
    reached the intended behavior assertion and rejected the old schema-2
    descriptor before production implementation.
  - Native paired green: the same command passed after the two-controller,
    two-input/two-plant lesson was implemented. The `100` to `120 L/min` event
    produced feedback-only and ratio-trim IAE of `0.526905` and `0.000000 s`,
    with peak ratio deviations `0.033861` and `0.000000`. A separate `+5 L/min`
    meter-bias run ended with mean absolute ratio error `0.000218` and
    `-1.481363 L/min` trim. Invalid quality and `5 L/min` low-flow cases both
    held ratio demand and additive request exactly at zero after loss of
    authority. The focused red also exposed same-scan PID input action during
    Track; tracking the current measured ratio as well as the safe output
    removed the unwanted `0.446268 L/min` fallback request.
  - Learner-contract expected red: the registered test command
    `TRUST_REPOSITORY_ROOT=$HOME/projects/trust-platform-control-curriculum-validation node ./node_modules/mocha/bin/mocha --ui tdd out/test/suite/control-lesson-industrial-structures.test.js --grep "teaches blending ratio"`
    reached the intended contract assertion and rejected the old schema-2
    summary.
  - Learner-contract paired green: the same command passed `1` test and proves
    the full pedagogical flow, words-before-symbols definitions, actual ratio
    and local-model equations, real Structured Text, Control Studio steps,
    exact measures, validity classification and next-lesson reason.
  - Current retained/rendered evidence (2026-08-30): nominal, meter-bias,
    invalid-quality and low-flow evidence is retained in the SVG, full trace
    CSV and summary CSV and passes no-update drift comparison. Independent
    review rejected the first plot and accepted the recapture after the green-
    dashed ratio-demand reference was identified explicitly.
- [x] `CELP-066` Lesson 56 — Reactor split-range heating/cooling. Teach split
  point, dead zone, overlap, installed gain, sequencing and energy conflict
  before mapping. Prove bumpless crossover without simultaneous wasteful action.
  - Specification: `docs/specs/38-control-engineering-curriculum.md`, Lesson 56,
    now defines signed demand ownership, the ideal mapper, four-point dead-zone
    and overlap candidates, installed-gain characterization, tracking, local
    reactor physics, exact comparison measures and causal transfer to override
    control.
  - Native expected red: on `trust-builder`,
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum cargo test -p trust-runtime --test control_curriculum_industrial_structures control_curriculum_split_range_exposes_gap_overlap_and_characterized_crossover -- --exact --nocapture`
    reached the intended assertion and rejected the old schema-2 descriptor.
  - Native paired green: the same command passed with three equivalent plants
    and identical signed PIs. The dead-zone, overlap and characterized mappings
    produced `167`, `0` and `0` qualified gap samples; simultaneous-utility
    samples were `0`, `1661` and `0`; total utility throughputs were `644.003`,
    `1699.440` and `741.403 percent*s`. The test also proves exact two-thirds
    cooling characterization and zero demand/valves under unavailable authority.
    The first implementation run corrected the gap metric from a `0.1` to
    `0.25 percent` demand threshold because the lower threshold falsely labeled
    a correctly characterized sub-`0.1 percent` cooling request as dead zone.
  - Learner-contract expected red: the registered command
    `TRUST_REPOSITORY_ROOT=$HOME/projects/trust-platform-control-curriculum-validation node ./node_modules/mocha/bin/mocha --ui tdd out/test/suite/control-lesson-industrial-structures.test.js --grep "teaches reactor split-range"`
    reached the intended contract assertion and rejected the old schema-2
    summary.
  - Learner-contract paired green: the same command passed `1` test and proves
    the full causal pedagogy, words-before-symbols equations, actual three-PID
    implementation, Control Studio sequence, installed-gain reasoning, exact
    energy/gap/crossover measures, validity classes and Lesson-57 handoff.
  - Current retained/rendered evidence (2026-08-30): temperature, demand, valve,
    crossover and authority evidence is retained in the SVG, full trace CSV and
    summary CSV and passes no-update drift comparison. The synchronized real
    gallery/copy/render journey is independently accepted.
- [x] `CELP-067` Lesson 57 — Compressor selector and override. Teach constraint
  controllers, MIN/MAX, authority, external tracking, windup and release
  hysteresis before takeover. Prove protection and bumpless return.
  - Specification: `docs/specs/38-control-engineering-curriculum.md`, Lesson 57,
    now defines physical selector direction, common-unit candidates, priority,
    inactive windup, external tracking, 14/16-percent hysteresis, local
    compressor dynamics, exact transitions/measures, quality fallback and the
    causal reason for gain scheduling next.
  - Native expected red: on `trust-builder`,
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum cargo test -p trust-runtime --test control_curriculum_override control_curriculum_override_tracks_inactive_compressor_controllers_and_releases_once -- --exact --nocapture`
    reached the intended assertion and rejected the old schema-2 descriptor.
    Repeated contention on that shared target was removed for subsequent runs
    by warming the dedicated
    `$HOME/.cache/codex-targets/control-curriculum-dedicated` target.
  - Native paired green: with the dedicated target, the same focused Cargo test
    passed. Normal-only, untracked override and tracked override minimum margins
    were `1.279`, `8.202` and `11.874 percent`. Tracked protection took over at
    sample `218`, released at sample `805`, and changed the selected valve by
    only `0.108` and `0.064 percentage points`. Untracked protection did not
    release until sample `1381`, directly exposing stale inactive integral
    state. Invalid margin quality removed protection authority and held the
    reviewed `30 percent` simulated fallback exactly.
  - Learner-contract expected red: the registered command
    `TRUST_REPOSITORY_ROOT=$HOME/projects/trust-platform-control-curriculum-validation node ./node_modules/mocha/bin/mocha --ui tdd out/test/suite/control-lesson-industrial-structures.test.js --grep "teaches tracked compressor override"`
    reached the intended contract assertion and rejected the old schema-2
    summary.
  - Learner-contract paired green: the same command passed `1` test and proves
    words-before-symbols signal definitions, MAX-versus-MIN reasoning, actual
    five-PID tracking code, local-model prediction, Control Studio takeover and
    release steps, exact metrics, validity boundaries and the explicit
    anti-surge/machinery-protection disclaimer.
  - Current retained/rendered evidence (2026-08-30): candidate, selected/applied
    valve, pressure, margin, authority and quality evidence is retained in the
    SVG, full trace CSV and summary CSV and passes no-update drift comparison.
    The synchronized real gallery/copy/render journey is independently accepted.
- [x] `CELP-068` Lesson 58 — Gravity-tank gain scheduling. Teach local models,
  scheduling variable, regions, interpolation/hysteresis, atomic update and
  parallel tracking before implementation. Prove fixed-gain failure and both
  scheduled forms across every boundary and direction.
  - Written behavior authority: Specification 38 Lesson 58 now defines the
    nonlinear tank balance, three local models, qualified PI points, filtered
    scheduling variable, atomic interpolation, hysteretic parallel selection,
    compensated external tracking, common limits, controlled 420-second
    comparison, exact metrics, validity states and physical-plant boundary.
  - Native expected red: on `trust-builder`, the focused command
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum TMPDIR=$HOME/.cache/codex-targets/control-curriculum/tmp cargo test -p trust-runtime --test control_curriculum_scheduling control_curriculum_gain_scheduling_crosses_gravity_tank_regions_without_chatter_or_bumps -- --exact --nocapture`
    reached the intended assertion and rejected the old schema-2 lesson.
  - Native paired green: the same focused test, using isolated warmed target
    `$HOME/.cache/codex-targets/control-curriculum-dedicated`, passed after
    executing three real nonlinear gravity tanks for fixed, interpolated and
    parallel schedules plus a separate unavailable-authority run. Regional IAE
    was `[3.2839, 3.2207, 2.6221, 3.2669]`, `[2.6837, 3.0500, 2.3617,
    2.3014]`, and `[2.4123, 2.8488, 2.4749, 2.4807] m*s`; worst values were
    `3.2839`, `3.0500`, and `2.8488 m*s`. The parallel architecture made four
    transitions with pump steps `0.000193`, `0.000172`, `0.000245`, and
    `0.000284 m3/s`, and unavailable authority forced all requests to zero.
  - Learner-contract expected red: the registered command
    `TRUST_REPOSITORY_ROOT=$HOME/projects/trust-platform-control-curriculum-validation node ./node_modules/mocha/bin/mocha --ui tdd out/test/suite/control-lesson-industrial-structures.test.js --grep "teaches gravity-tank scheduling"`
    reached the intended contract assertion and rejected the old schema-2
    summary.
  - Learner-contract paired green: the same command passed `1` test and proves
    the complete words-before-symbols nonlinear-model derivation, local tuning,
    interpolation and parallel-transition theory, real five-PID/scheduler ST,
    Control Studio experiment, defined measures, evidence boundary and causal
    handoff to transport-delay compensation.
  - Current retained/rendered evidence (2026-08-30): the full scheduling run and
    model/transition evidence is retained in the SVG, full trace CSV and summary
    CSV and passes no-update drift comparison. The synchronized real gallery/
    copy/render journey is independently accepted.
- [x] `CELP-069` Lesson 59 — Conveyor-oven Smith predictor. Teach delay-free and
  delay models, internal prediction and mismatch sensitivity before design.
  Prove the strongest acceptable PI baseline, nominal improvement and degraded
  behavior under perturbed gain/time/delay.
  - Written behavior authority: Specification 38 Lesson 59 now defines a
    delay-dominant `K = 0.8 degC/%`, `T = 12 s`, `L = 16 s` conveyor oven,
    ordinary-PI candidate qualification, delay-free/delayed Smith signals,
    correction and fallback, restored nominal/moderate/severe runs, exact
    measures, validity classification and physical-oven boundary.
  - Native expected red: on `trust-builder`,
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum-dedicated TMPDIR=$HOME/.cache/codex-targets/control-curriculum-dedicated/tmp cargo test -p trust-runtime --test control_curriculum_smith control_curriculum_smith_predictor_improves_nominal_delay_response_and_exposes_mismatch -- --exact --nocapture`
    reached the intended schema assertion and rejected the old schema-2 lesson.
    The preceding cold compile and quoting error were harness failures and are
    not counted as red evidence.
  - Native paired green: the same command passed `1` test across accepted and
    rejected ordinary PI candidates, nominal Smith, moderate mismatch and
    severe mismatch. Accepted ordinary PI produced `580.2065 degC*s` total IAE,
    `51.4 s` 90-percent rise time, `1.0989 degC` overshoot and `0.0661 degC`
    final-window error. The next ordinary candidate was rejected at `2.9317
    degC` overshoot. Nominal Smith produced `499.1734 degC*s` total IAE,
    `41.3 s` rise time, `3.1547 degC` peak correction and `0.0623 degC` final
    error. Moderate mismatch increased IAE to `531.1725 degC*s` and correction
    to `4.2351 degC`; severe mismatch made `1,102` controller samples use the
    exact measured-feedback fallback while all signals remained bounded.
  - Learner-contract expected red: the registered command
    `TRUST_REPOSITORY_ROOT=$HOME/projects/trust-platform-control-curriculum-validation node ./node_modules/mocha/bin/mocha --ui tdd out/test/suite/control-lesson-industrial-structures.test.js --grep "teaches Smith prediction"`
    reached the intended contract assertion and rejected the old schema-2
    summary.
  - Learner-contract paired green: the same command passed `1` test and proves
    words-before-symbols delayed-plant and Smith equations, ordinary candidate
    rejection, the actual one-PID/one-predictor ST path, Control Studio
    Identify/Tune/Test workflow, separately defined servo/regulatory metrics,
    mismatch/fallback interpretation and Simulation/physical boundaries.
  - Current retained/rendered evidence (2026-08-30): ordinary, nominal,
    moderate and severe-mismatch evidence is retained in the SVG, full trace
    CSV and summary CSV and passes no-update drift comparison. The synchronized
    real gallery/copy/render journey is independently accepted.
- [x] `CELP-070` Lesson 60 — Valve characterization and stiction. Teach installed
  characteristics, equal-percentage action, deadband, backlash, stiction and
  positioner evidence before diagnosis. Prove correct distinction and select
  maintenance where controller changes are inappropriate.
  - Written behavior authority: Specification 38 Lesson 60 defines three
    equivalent cooling-water valve benches, the inherent equal-percentage
    reference and installed square-law flow model, independent requested and
    measured positions, deadband/stiction mechanisms, repeated restored
    reversal probes, automatic process consequence, exact measures, validity
    rules and the simulation/physical-valve evidence boundary.
  - Native expected red: on `trust-builder`,
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum-dedicated TMPDIR=$HOME/.cache/codex-targets/control-curriculum-dedicated/tmp cargo test -p trust-runtime --test control_curriculum_valve_stiction control_curriculum_valve_characterization_distinguishes_deadband_from_stiction -- --exact --nocapture`
    reached the intended schema assertion and rejected the old schema-2 lesson.
    A later executable iteration disproved the draft assumption that the
    memoryless model would necessarily create repeated closed-loop breakaways;
    the specification was corrected before acceptance to require repeated
    restored position probes and to classify the automatic response honestly.
  - Native paired green: the same command passed `1` test. The installed curve
    measured `0.0291/3.9996/15.9989/35.9982/63.9975/99.9968 m3/h` at
    `0/20/40/60/80/100 percent`; the deadband stem held at `70.0000 percent`
    through a `72 percent` request and moved to `70.5530 percent` at `72.5
    percent`; the stiction stem held through `75 percent`, broke away to
    `72.0000 percent` at `75.5 percent`, and repeated this result in all three
    restored probes. The automatic run produced one stiction breakaway, a
    `4.95`-percentage-point peak request-position gap and a `1.6331 m3/h` peak
    healthy/stiction flow difference without fabricating a sustained cycle.
  - Learner-contract expected red: after direct TypeScript compilation, the
    registered command `TRUST_REPOSITORY_ROOT=$HOME/projects/trust-platform-control-curriculum-validation node ./node_modules/mocha/bin/mocha --ui tdd out/test/suite/control-lesson-industrial-structures.test.js --grep "teaches installed-valve characterization"`
    reached the intended descriptor assertion and rejected the legacy lesson.
  - Learner-contract paired green: the same command passed `1` test and proves
    the full pedagogical sequence, words-before-symbols theory, physical valve
    and installed-curve distinction, real three-PID Structured Text, defined
    measures, repeated diagnostic evidence, Control Studio steps and explicit
    evidence boundary. `python3 scripts/check_vscode_test_registration.py`
    reports the test file registered in the real extension suite.
  - Current retained/rendered evidence (2026-08-30): the installed curve,
    restored probes and automatic run are retained in the SVG, full trace CSV
    and summary CSV and pass no-update drift comparison. The synchronized real
    gallery/copy/render journey is independently accepted.
- [ ] `CELP-M09` Module 9 gate — Run one integrated reactor/process-area journey
  combining only justified industrial structures. Prove tracking, constraint
  ownership, mode transitions, failure recovery and maintainable signal flow.
  - Accepted (2026-08-30): the eight owning native tests pass both artifact
    update and no-update execution; the registered learner suite passes all
    `13` industrial-curriculum tests. All eight current real Extension
    Development Host journeys are independently accepted in
    `docs/internal/testing/evidence/vscode-control-curriculum/2026-08-30-module-09/acceptance.md`.
    The tracked reactor capstone provides the integrated process-area journey:
    one authority path coordinates inner-first cascade, measured-load
    feedforward, characterized split range, protection override, tracking,
    manual return and controlled shutdown without presenting these structures
    as unrelated fragments. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**

## 11. Multivariable control

Specification authority: §3.10, Lessons 61–63.

- [ ] `CELP-071` Lesson 61 — Distillation interaction and RGA. Teach response/
  gain matrices, coupling, pairing, RGA, signs and operating-point dependence
  before testing. Prove the matrix from executed steps and compare good/bad
  pairings.
  - Written behavior authority: Specification 38 Lesson 61 now defines one
    local continuous binary-column operating point, both manipulated inputs and
    composition outputs, four signed steady gains, two separately restored
    `+5`-percentage-point experiments, settling/excitation rules, determinant,
    Relative Gain Array interpretation, pairing validity and the physical-
    column boundary.
  - Native expected red: on `trust-builder`,
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum-dedicated TMPDIR=$HOME/.cache/codex-targets/control-curriculum-dedicated/tmp cargo test -p trust-runtime --test control_curriculum_distillation_rga control_curriculum_distillation_steps_identify_gain_matrix_and_rga_pairing -- --exact --nocapture`
    reached the intended schema assertion and rejected the legacy schema-2
    centralized-MIMO summary. A later zero-applied-step failure exposed the
    native harness's need to bind the real program variables directly, as in
    the established curriculum tests; it was not counted as red or weakened by
    substituting configured coefficients.
  - Native paired green: the same exact command passed `1` test after compiling
    and running the shipped manual Structured Text and state-space plant. The
    two independently restored records measured `G = [[0.799872, -0.397300],
    [-0.296218, 0.697944]]`, the four `63.2 percent` response times as
    `20.0/34.6/39.2/29.9 s`, determinant `0.440579` and `RGA = [[1.267120,
    -0.267120], [-0.267120, 1.267120]]`; both non-excited inputs remained fixed,
    every path settled and met its finite-record gain/time tolerance, all row/
    column sums met `0.001`, the diagonal pairing qualified and the cross
    pairing was rejected. The unequal measured dynamics are retained as the
    causal input to Lesson 62's static-versus-dynamic decoupling decision.
  - Dynamic-continuity expected red/green: after the first steady-matrix green,
    Specification 38 added the four distinct path dynamics needed to make the
    next decoupling lesson causal. The same native command then reached the new
    response-time assertion and failed because the old row-shared model
    measured the reboiler-to-overhead path at `20.0 s` instead of `35.0 s`.
    After replacing it with four executable path states, the same command
    passed with `20.0/34.6/39.2/29.9 s`; the declared `1.0 s` tolerance records
    finite-180-second endpoint estimation and does not rename the measured
    values as their exact asymptotes.
  - Learner-contract expected red: after repairing a test-local root-variable
    compile mistake that does not count as red, the registered command
    `TRUST_REPOSITORY_ROOT=$HOME/projects/trust-platform-control-curriculum-validation node ./node_modules/mocha/bin/mocha --ui tdd out/test/suite/control-lesson-industrial-structures.test.js --grep "teaches measured distillation interaction"`
    reached the intended descriptor assertion and rejected the old schema-2
    lesson.
  - Learner-contract paired green: the same command passed `1` test and proves
    the full words-before-symbols teaching sequence, all four physical paths,
    measured-matrix and RGA derivation, real manual Structured Text without a
    hard-coded analytical verdict, Control Studio experiment, exact validity
    rules, sensible displayed precision, pairing decision and Simulation/
    physical boundary.
  - Completion evidence (2026-08-30): the native test retains the complete
    trace, decision summary and rendered response figure; the canonical bundle
    contains the same files. The real VS Code gallery/copy/Markdown journey
    passed and independent reviewer Harvey accepted the current rendered
    lesson. Evidence:
    `docs/internal/testing/evidence/vscode-control-curriculum/2026-08-30-modules-10-11/`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**
- [x] `CELP-072` Lesson 62 — Distillation decentralized PID and decoupling. Teach
  static/dynamic decoupling, realizability, noise and mismatch before design.
  Prove independent PID, static and eligible dynamic cases under equal events.
  - Written behavior authority: Specification 38 Lesson 62 now derives two
    fixed diagonal PI tunings, normalized static inversion, proper causal
    dynamic path ratios, realizability/rejection rules, nominal overhead,
    bottoms and feed-disturbance records, one declared cross-path perturbation,
    separate cross/error/effort requirements and the physical-column boundary.
  - Native expected red: on `trust-builder`,
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum-dedicated TMPDIR=$HOME/.cache/codex-targets/control-curriculum-dedicated/tmp cargo test -p trust-runtime --test control_curriculum_distillation_decoupling control_curriculum_distillation_decoupling_compares_direct_static_and_dynamic_evidence -- --exact --nocapture`
    reached the intended missing-project assertion before any Lesson 62
    production file existed. A later missing `PIDInitialState` field was a
    compile failure and is not counted as red.
  - Native paired green: the same exact command passed `1` test after compiling
    the shipped two-PID, one-static-decoupler, two-filter Structured Text and
    executing twelve separately initialized nominal/perturbed records. Nominal
    overhead cross peaks were `0.185093/0.042939/0.000051 pp`, bottoms cross
    peaks `0.244192/0.122258/0.000160 pp`, and feed-disturbance combined IAE
    `22.587910/31.300300/38.500753 pp*s` for direct/static/dynamic. All declared
    nominal error, travel, final-error and boundedness requirements passed.
    Perturbed direct/static/dynamic cross peaks were
    `0.156818/0.067782/0.059432 pp`; dynamic increased IAE from static's
    `54.774217` to `63.996471 pp*s`, so it failed the predeclared `20 percent`
    cross/`5 percent` IAE materiality rule and the simpler static fallback was
    selected.
  - Learner-contract expected red: after repairing a test-only filesystem API
    compile error that does not count as red, the registered command
    `TRUST_REPOSITORY_ROOT=$HOME/projects/trust-platform-control-curriculum-validation node ./node_modules/mocha/bin/mocha --ui tdd out/test/suite/control-lesson-industrial-structures.test.js --grep "teaches evidence-qualified static and dynamic"`
    reached the intended assertion that the Lesson 62 executable project was
    absent.
  - Learner-contract paired green: the same command passed `1` test and proves
    the complete causal teaching sequence, ordinary words before every symbol,
    actual static/dynamic equations and ST instances, Control Studio nominal/
    perturbation workflow, declared measures and materiality thresholds,
    evidence-based static fallback, sensible precision and Simulation/physical
    boundary.
  - Completion evidence (2026-08-30): retained trace, decision summary and the
    two-panel response figure are native-test-owned and bundle-synchronized.
    The real VS Code journey passed. After two explicit visual rejection/fix
    cycles for clipping, tick collision and panel spacing, independent reviewer
    Harvey accepted the current figure at the evidence root above.
- [x] `CELP-073` Lesson 63 — Multivariable robustness and architecture. Teach
  singular values, directional gain, interaction uncertainty and centralized/
  decentralized selection before analysis. Prove the simplest robust choice.
  - Written behavior authority: Specification 38 Lesson 63 now defines the
    same binary-column context, nominal and stronger-interaction uncertainty
    cases, words-first vector/singular-value/condition/sensitivity theory,
    direct/static/centralized executable candidates, common normal records,
    explicit requirements, joint constraint infeasibility, Manual restoration,
    architecture selection, and Simulation/physical boundary.
  - Learner-contract expected red: on `trust-builder`, direct TypeScript
    compilation followed by
    `TRUST_REPOSITORY_ROOT=$HOME/projects/trust-platform-control-curriculum-validation node ./node_modules/mocha/bin/mocha --ui tdd out/test/suite/control-lesson-industrial-structures.test.js --grep "teaches singular-direction robustness"`
    reached the intended assertion that the Lesson 63 executable project did
    not exist. The earlier curriculum bundle sequence-gap failure was a staged
    harness boundary and is not counted as red.
  - Learner-contract paired green: the same focused Mocha command passed `1`
    test after the project and README were implemented. It proves the complete
    eleven-stage causal flow, ordinary words before symbols, both declared
    matrices and singular results, real ST excerpt, Control Studio workflow,
    metric definitions, infeasibility decision, sensible precision and
    Simulation/physical distinction.
  - Native expected red: on `trust-builder`,
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum-dedicated TMPDIR=$HOME/.cache/codex-targets/control-curriculum-dedicated/tmp cargo test -p trust-runtime --test control_curriculum_distillation_robustness control_curriculum_distillation_robustness_selects_from_coupled_evidence -- --exact --nocapture`
    compiled and reached the intended missing `lesson.json` assertion before
    any Lesson 63 production file existed.
  - Native paired green: the same exact command passed `1` test in `160.13 s`
    after compiling the shipped two-PID/static-decoupler/controller program and
    executing 27 independently initialized records. Independent oracles found
    normalized singular values `1.464923/0.536352` and
    `1.751249/0.251249`, condition numbers `2.73/6.97`, uncertainty norm
    `0.30`, and direct/static sensitivity peaks of
    `1.005030/1.009405` nominally and `1.006677/1.007105` under stronger
    interaction. Direct/static/centralized stronger-interaction simultaneous
    final errors were `0.1728/0.0767/0.0767 pp`; direct therefore failed while
    static and centralized passed. With no centralized material benefit in any
    feasible record, static was selected. The `+2/+2` final errors were
    `1.0275/0.4174/0.2747 pp`; centralized scaling reached `0.8436`, preserved
    command direction and remained honestly `Infeasible`. All explicit Manual
    restoration records passed the `0.02` input and product criteria.
  - Completion evidence (2026-08-30): retained coupled-response trace, decision
    summary and figure are generated by the native oracle, bundled, rendered in
    the real extension and independently accepted at the evidence root above.
- [x] `CELP-M10` Module 10 gate — Qualify matrix, RGA, decoupling and singular-
  value calculations against independent oracles. Capture coupled time plots
  and the professional interaction-analysis view.

## 12. State-space, estimation, optimal and predictive control

Specification authority: §3.11, Lessons 64–68.

- [x] `CELP-074` Lesson 64 — Two-tank controllability and observability. Teach
  matrices, rank, modes and hardware interpretation before analysis. Prove
  controllable/observable and deliberately deficient sensor/actuator layouts.
  - Specification-first implementation (2026-08-29): Specification 38 §3.11
    now defines the two interconnected water-treatment tanks, exact local
    `A/B/C` matrices, ordinary-language state and matrix concepts, four
    actuator/sensor layouts, predictions, bounded Control Studio records,
    measures, requirements, simulation-truth boundary, aborts, restoration,
    and the causal handoff to Lesson 65.
  - Focused learner red/green (2026-08-29): after direct TypeScript compilation,
    `TRUST_REPOSITORY_ROOT=$HOME/projects/trust-platform-control-curriculum-validation node ./node_modules/mocha/bin/mocha --ui tdd --timeout 20000 out/test/suite/control-lesson-selection-modelling.test.js --grep "teaches two-tank actuator and sensor feasibility before design"`
    first reached the behavior assertion and failed because the existing lesson
    used schema version 2 rather than the reviewed schema version 4 contract.
    The same command now passes `1 passing`. A discovered equation assertion
    normalized away its own multiplication marks; that harness defect was
    corrected to inspect the original README and was not counted as behavioral
    red evidence.
  - Focused native red/green (2026-08-29):
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum-dedicated TMPDIR=$HOME/.cache/codex-targets/control-curriculum-dedicated/tmp cargo test -p trust-runtime --test control_curriculum_two_tank_feasibility control_curriculum_two_tank_feasibility_proves_reachable_and_hidden_states -- --exact --nocapture`
    on `trust-builder` first failed the expected missing-project assertion for
    Lesson 64's `lesson.json`, then passed after the shipped manual-routing ST
    and three equivalent state-space configurations were implemented. The
    executable evidence gives controllability ranks `2/1`, observability ranks
    `2/1`, normal condition numbers `33.417/33.387`, upstream first-second
    least-squares slope `0.1951 cm/s`, Tank-1/Tank-2 first responses at
    `0.1/0.6 s`, upstream-step peaks `3.973/2.805 cm`, bypass peaks
    `0.000/4.908 cm`, downstream response to an upstream initial state
    `1.229 cm`, hidden downstream initial state `5.000 cm`, maximum selected-
    sensor residual `0.000 cm`, and step-record restoration errors
    `0.0123/0.0045 cm`.
  - Completion evidence (2026-08-30): the native test owns retained trace,
    decision summary and figure; bundle synchronization and real VS Code
    rendering passed, and Harvey independently accepted the current figure at
    the modules 10–11 evidence root.
- [x] `CELP-075` Lesson 65 — Magnetic-bearing state feedback and motor observer.
  Teach pole placement, reference gain, estimation dynamics, separation and
  mismatch before design. Prove stabilization and state-estimation limits.
  - Specification-first implementation (2026-08-29): Specification 38 §3.11
    now defines one causal two-bench lesson with exact bearing and motor models,
    words-first state-feedback/reference/observer theory, derived gains and
    continuous/discrete poles, requested/applied limits, controlled records,
    metric eligibility, Simulation-truth boundaries, aborts, disabled outputs,
    model-mismatch validation, and the handoff to Lesson 66.
  - Focused learner red/green (2026-08-29): after direct TypeScript compilation,
    `TRUST_REPOSITORY_ROOT=$HOME/projects/trust-platform-control-curriculum-validation node ./node_modules/mocha/bin/mocha --ui tdd --timeout 20000 out/test/suite/control-lesson-selection-modelling.test.js --grep "teaches state feedback before model-qualified state observation"`
    first reached the expected behavior assertion and failed `2 !== 4` because
    the old magnetic-bearing summary did not implement the reviewed lesson
    contract. The same command now passes `1 passing` with the combined shipped
    bearing/motor project and complete pedagogical README.
  - Focused native red/green (2026-08-29):
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum-dedicated TMPDIR=$HOME/.cache/codex-targets/control-curriculum-dedicated/tmp cargo test -p trust-runtime --test control_curriculum_state_feedback_observer control_curriculum_state_feedback_and_observer_match_executed_modes -- --exact --nocapture`
    on `trust-builder` first failed the expected schema-version assertion
    (`2` rather than `4`). The implemented runtime test then exposed a real
    overlapping process-image defect between `%MX32.*` mode bits and `%ML32`
    requested torque; the shipped mode bindings were moved to `%MX120.*`
    without widening any plant limit. The final command passes and independently
    reproduces bearing poles `-3/-4`, observer poles `-4/-5`, bearing regulation
    and reference settling `1.70/1.70 s`, final bearing errors
    `0.00000/0.00000 mm`, peak current `1.6000 A`, and current movement
    `1.7298 A`. Motor baseline/nominal/mismatch final speed errors are
    `0.1667/0.0000/0.0000 rad/s`; nominal/mismatch load errors are
    `0.0000/0.2000 N m`; observer settling is `2.00 s`; innovation peak is
    `0.2379 rad/s`; and applied-torque peak/movement is `3.0000/2.2239 N m`.
  - Completion evidence (2026-08-30): retained trace, summary and figure are
    native-test-owned; the canonical catalog excludes superseded standalone
    variants. Bundle, real VS Code rendering and independent acceptance passed
    at the evidence root above.
- [x] `CELP-076` Lesson 66 — Weigh-belt Kalman filter. Teach probability, mean,
  covariance, process/measurement noise, prediction, correction, gain and
  innovation before estimation. Prove covariance-assumption effects and
  innovation diagnostics.
  - Specification-first implementation (2026-08-29): Specification 38 Lesson
    66 now defines one two-copy weigh-feeder experiment with words-first random-
    variable, mean, variance, covariance, prediction, correction, gain and
    innovation theory; exact discrete coefficients and a worked first update;
    fixed, nominal, sensor-overtrust, sensor-distrust, paired bias and invalid-
    covariance cases; requested/applied command and Simulation-truth boundaries;
    reproducible measures, aborts, fallback, and the handoff to Lesson 67.
  - Focused learner red/green (2026-08-29): after direct TypeScript compilation,
    `TRUST_REPOSITORY_ROOT=$HOME/projects/trust-platform-control-curriculum-validation node ./node_modules/mocha/bin/mocha --ui tdd --timeout 20000 out/test/suite/control-lesson-state-space.test.js --grep "teaches Kalman uncertainty weighting and innovation diagnosis"`
    first reached the expected behavior assertion and failed `2 !== 4` because
    the prior summary did not implement the reviewed lesson contract. The same
    command now passes `1 passing` against the shipped project. When executable
    noise proved that a per-sample settling band was statistically invalid, the
    specification and test were corrected first; the revised assertion then
    failed because the README lacked the two-second mean 90-percent response
    definition and passed after that explanation was implemented.
  - Focused native red/green (2026-08-29):
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum-dedicated TMPDIR=$HOME/.cache/codex-targets/control-curriculum-dedicated/tmp cargo test -p trust-runtime --test control_curriculum_weigh_feeder_kalman control_curriculum_weigh_feeder_kalman_qualifies_covariance_and_innovation -- --exact --nocapture`
    first reached the expected product assertion and failed because the old
    descriptor was schema 2 rather than 4. It now passes `1 passed` while
    compiling and executing the shipped Structured Text with both feeder
    configurations. An intermediate bias failure also found that renamed plant
    and uncertainty identities changed the deterministic random stream; the
    shipped bias configuration now changes only noise mean, making the
    comparison genuinely paired.
  - Current native Simulation evidence (2026-08-29): fixed/nominal/overtrust/
    distrust truth-error RMS is `0.5960/0.3303/0.8884/0.1327 kg/s`; two-second
    mean 90-percent response is `3.1/1.4/2.8/1.5 s`; average Kalman gain is
    `0.1095/0.4290/0.0378` for nominal/overtrust/distrust; estimate movement is
    `78.129/44.340/182.623/17.533 kg/s`. Nominal innovation mean/RMS/NIS/lag-one
    correlation is `0.0072/1.8200/0.9832/-0.1490`. The paired `+1 kg/s` bias
    produces innovation mean `0.4970 kg/s`, final truth error `0.6483 kg/s`, and
    paired estimate shift `0.5638 kg/s`; invalid Q selects the finite fixed
    fallback. These are explicitly Simulation results.
  - Completion evidence (2026-08-30): retained trace, summary and figure are
    generated from the executable oracle. Bundle synchronization, real VS Code
    rendering and independent acceptance passed at the evidence root above.
- [x] `CELP-077` Lesson 67 — Suspension LQR/LQG. Teach quadratic cost, state/input
  weights, Riccati solution, scaling, estimator combination and robustness
  limits before design. Prove comfort/travel/energy trade-offs.
  - Specification-first implementation (2026-08-29): Specification 38 Lesson
    67 now defines one reduced active-suspension project that progresses from a
    pole-placement baseline through four unit-scaled LQR costs to balanced
    position-only LQG. It defines the physical model and local range, words-
    first cost/Riccati/normalization/estimator theory, independently qualified
    continuous and discrete designs, one common road pulse, requested/applied
    force, common physical measures, abnormal noise/model cases, aborts,
    Simulation boundaries, and the handoff to Lesson 68.
  - Focused learner red/green (2026-08-29): after direct TypeScript compilation,
    `TRUST_REPOSITORY_ROOT=$HOME/projects/trust-platform-control-curriculum-validation node ./node_modules/mocha/bin/mocha --ui tdd --timeout 20000 out/test/suite/control-lesson-state-space.test.js --grep "teaches LQR tradeoffs before position-only LQG"`
    first reached the expected behavior assertion and failed `2 !== 4` because
    the old suspension summary did not implement the reviewed teaching
    contract. It now passes `1 passing` against the shipped combined project.
  - Focused native red/green (2026-08-29):
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum-dedicated TMPDIR=$HOME/.cache/codex-targets/control-curriculum-dedicated/tmp cargo test -p trust-runtime --test control_curriculum_suspension_lqr_lqg control_curriculum_suspension_lqr_lqg_qualifies_cost_and_estimation -- --exact --nocapture`
    first reached the expected product assertion and failed because the old
    descriptor was schema 2 rather than 4. It now passes `1 passed` while
    independently solving the four continuous Riccati designs and steady-state
    discrete estimator gain, compiling and executing the shipped Structured
    Text, and running six nominal plus doubled-noise and model-mismatch records.
    Intermediate executions rejected `1.5` and `1.0 m/s2` pulses for exceeding
    the declared `0.08 m` local range; the specification was corrected to a
    `0.5 m/s2` pulse instead of widening the model envelope to admit the test.
  - Current native Simulation evidence (2026-08-29): pole/body/travel/balanced/
    effort/LQG deflection RMS is `0.0077/0.0145/0.0120/0.0156/0.0207/0.0156 m`;
    force energy is `0.2638/0.0759/0.1218/0.0503/0.0054/0.0520 kN^2*s`; return
    time is `1.6/2.8/2.3/3.5/5.5/3.5 s`. Nominal LQG position/velocity estimate
    RMS is `0.00127 m/0.00511 m/s`, with innovation mean `0.000058 m`. Doubled
    noise raises those errors to `0.00254/0.01021`; model mismatch produces
    `0.00132/0.00590` and innovation mean `0.000191 m`. LQG reproduces balanced
    motion and energy but increases force movement from `0.911` to `1.952 kN`,
    which remains visible as an actuator-wear tradeoff.
  - Completion evidence (2026-08-30): retained trace, summary and figure are
    native-test-owned; superseded standalone variants are excluded from the
    canonical catalog. Bundle, real VS Code rendering and independent
    acceptance passed at the evidence root above.
- [x] `CELP-078` Lesson 68 — Constrained HVAC MPC. Teach optimization variables,
  objective, model, horizons, constraints, preview, receding horizon,
  feasibility and fallback before execution. Prove a genuine online solve,
  compare against strong PI/cascade, and exercise infeasibility.
  - Specification-first implementation (2026-08-29): Specification 38 Lesson
    68 now defines one two-zone office-floor project with a continuous coupled
    thermal model and exact 60-second sampled matrices; a three-step, six-move,
    729-candidate finite-grid optimization; words-first objective, constraints,
    feasibility and receding-horizon theory; a separately labelled incremental
    PI fallback; requested/applied commands; a common weather, occupancy and
    tariff schedule; reproducible measures; completed-infeasible, invalid,
    recovery and disable cases; Simulation boundaries; and the handoff to
    Lesson 69.
  - Focused learner red/green (2026-08-29): after direct TypeScript compilation,
    `TRUST_REPOSITORY_ROOT=$HOME/projects/trust-platform-control-curriculum-validation node ./node_modules/mocha/bin/mocha --ui tdd --timeout 20000 out/test/suite/control-lesson-state-space.test.js --grep "teaches genuine constrained HVAC MPC with explicit fallback"`
    first reached the expected behavior assertion and failed `2 !== 4` because
    the old project did not implement the reviewed lesson contract. It now
    passes `1 passing` against the shipped project. An intermediate assertion
    was corrected when README normalization removed Markdown multiplication
    markers from the displayed PI unit; this did not weaken the unit or lesson.
  - Focused native red/green (2026-08-29):
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum-dedicated cargo test -p trust-runtime --test control_curriculum_building_hvac_mpc -- --nocapture`
    first reached the expected product assertion and failed because the old
    descriptor was schema 2 rather than 4. It now passes `1 passed` while
    compiling and executing the shipped Structured Text, independently
    enumerating the finite grid, checking all normal first moves and objectives,
    and exercising PI, MPC, completed infeasibility, invalid configuration,
    fallback, recovery and disable behavior. An intermediate executable run
    proved that the originally declared 5-to-0-degree cold front was physically
    infeasible under the heater and comfort limits; the specification was
    corrected first to a 5-to-4-degree normal cold front, while retaining the
    separate 22-degree-minimum case as the intentional infeasibility lesson.
  - Current native Simulation evidence (2026-08-29): qualified PI/MPC zone
    absolute-error integral is `158.67/146.72 degC*s`; both have `0.00 degC*s`
    and `0 s` comfort violation; tariff-weighted heating is `46,624/46,200`
    relative cost-percent seconds; applied-heater movement is `18.7/160.0`
    percentage points; peak shared heating is `166.3/160.0 percent`; and both
    have `0 s` normal fallback. MPC reduces error integral by `7.5 percent` and
    tariff-weighted heating by `0.9 percent`, but uses about `8.6` times the
    actuator movement, which remains visible as an engineering tradeoff.
  - Completion evidence (2026-08-30): retained trace, summary and figure are
    generated by the executable constrained-search oracle. Bundle, real VS Code
    rendering and independent acceptance passed at the evidence root above.
- [x] `CELP-M11` Module 11 gate — Qualify all matrix, estimator, Riccati and MPC
  numerical results against independent references. Confirm MPC is a real
  constrained optimizer and not a precomputed affine law.

## 13. Nonlinear, adaptive and robust control

Specification authority: §3.12, Lessons 69–72.

- [ ] `CELP-079` Lesson 69 — Pendulum nonlinear stability. Teach equilibria,
  phase plane, local/global stability, attraction region and Lyapunov conditions
  before simulation. Prove certified versus merely simulated regions.
  - Specification-first implementation (2026-08-29): Specification 38 Lesson
    69 now defines one motor-driven inverted-pendulum rig using the existing
    nonlinear sine-gravity simulator around the upright `pi-rad` equilibrium.
    It defines words-first state, equilibrium, phase-plane, local/global
    stability, attraction-region and Lyapunov theory; the exact nonlinear
    remainder and derivative; an analytically bounded `V<=0.025` sublevel set;
    requested/applied torque and saturation; eight certified releases, two
    outside observational releases and one saturated rejection; reproducible
    measures, aborts, Simulation boundaries, and the handoff to Lesson 70.
  - Focused learner red/green (2026-08-29): after direct TypeScript compilation,
    `TRUST_REPOSITORY_ROOT=$HOME/projects/trust-platform-control-curriculum-validation node ./node_modules/mocha/bin/mocha --ui tdd --timeout 20000 out/test/suite/control-lesson-state-space.test.js --grep "separates nonlinear Lyapunov certification from simulated recovery"`
    first reached the expected behavior assertion and failed `2 !== 4` because
    the old linear summary did not implement the reviewed nonlinear lesson
    contract. It now passes `1 passing` against the shipped project.
  - Focused native red/green (2026-08-29):
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum-dedicated cargo test -p trust-runtime --test control_curriculum_pendulum_lyapunov control_curriculum_pendulum_lyapunov_certifies_only_declared_sublevel_set -- --exact --nocapture`
    first reached the expected product assertion and failed because the old
    descriptor was schema 2 rather than 4. It now passes `1 passed`, compiling
    and executing the shipped Structured Text against the nonlinear plant,
    matching a separate ten-substep RK4 oracle, qualifying the analytic angle,
    rate, torque and derivative bounds, executing all declared trajectories,
    rejecting saturation and proving zero disabled torque.
  - Current native Simulation evidence (2026-08-29): the symmetric certified
    pairs starting at `(alpha,omega)=(+/-0.05,0)`, `(+/-0.10,0)`,
    `(+0.15,-0.15)` with sign reverse, and `(0,+/-0.35)` have initial V
    `0.001667/0.006667/0.015000/0.020417`, peak applied torque
    `0.200/0.400/0.300/0.700 N m`, and settling time
    `1.5/2.5/2.7/3.2 s`. All eight have zero sublevel exits, certificate
    violations, saturation and positive sampled Lyapunov change. Both
    `+/-0.25 rad` releases begin outside at `V=0.041667`, later enter the set
    and recover but remain labelled observational. The `0.8-rad` case begins at
    `V=0.426667`, requests `3.200 N m`, is limited to `2.000 N m`, and records
    `0.4 s` saturation, so the unsaturated certificate fails closed.
  - Source project, learner contract, analytic bounds, native nonlinear oracle,
    executable trajectory evidence, and measured README results are
    implemented. Current SVG, trace and summary artifacts are bundled and the
    real VS Code rendered journey is independently accepted in
    `docs/internal/testing/evidence/vscode-control-curriculum/2026-08-30-modules-12-13/`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**
- [x] `CELP-080` Lesson 70 — Robot gain scheduling versus feedback linearization.
  Teach local scheduling, nonlinear cancellation, residual dynamics,
  singularity and model-error risks before comparison. Prove architecture
  trade-offs across motion and load envelopes.
  - Specification-first implementation (2026-08-29): Specification 38 Lesson
    70 now defines one gravity-loaded industrial robot joint and compares fixed
    PID, gain-scheduled PID and eligible feedback linearisation over the same
    four-step wide-angle record. It derives local gains from gravity stiffness,
    defines interpolation and its unproven boundaries, derives nonlinear model
    compensation, reserves `1.2 N m` of nominal motor authority, predicts the
    heavy-payload residual, separates requested/applied torque, and requires
    deterministic scheduled fallback for invalid input effectiveness. The
    ten-second nonzero plateaus are explicitly part of steady-state validity.
  - Focused learner red/green (2026-08-29, `trust-builder`): after direct
    TypeScript compilation,
    `TRUST_REPOSITORY_ROOT=$HOME/projects/trust-platform-control-curriculum-validation node ./node_modules/mocha/bin/mocha --ui tdd --timeout 20000 out/test/suite/control-lesson-state-space.test.js --grep "compares robot gain scheduling with eligible feedback linearisation"`
    first reached the expected behavior assertion and failed `2 !== 4` because
    the former lesson was only a schema-version-2 linear summary. It now passes
    `1 passing` against the schema-version-4 executable project.
  - Focused native red/green (2026-08-29, `trust-builder`): exact command
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum-dedicated cargo test -p trust-runtime --test control_curriculum_robot_joint_nonlinear control_curriculum_robot_joint_compares_scheduling_and_linearisation -- --exact --nocapture`
    first reached the expected schema assertion and failed `2 !== 4`. During
    numerical qualification it then exposed one `20 ms` nominal saturation
    sample caused by designing exactly on the motor boundary, and unsettled
    three-second PID plateaus. The specification and implementation now reserve
    command margin, use the declared common PID integral design, and provide
    ten-second nonzero plateaus. The same command passes `1 passed`, compiling
    and executing the shipped Structured Text and shared Control library over
    six nonlinear records plus invalid-model fallback and disabled output.
  - Current native Simulation evidence (2026-08-29): nominal fixed, scheduled
    and feedback-linearised IAE is `3.028/3.028/1.631 rad*s`; applied-torque
    travel is `173.2/208.4/264.2 N m`; peak torque is
    `18.52/26.02/28.80 N m`; all saturation durations are zero. Nonzero nominal
    settling times are `3.14,3.22 s`, `4.66,4.76 s`, and `1.02,1.02 s`;
    all-four-segment spread is `0.92/0.70/0.02 s`. On the heavy plant,
    scheduled final-window errors are `0.00189/0.00207 rad`, while unchanged
    feedback linearisation retains `0.07380 rad` in both directions against the
    `0.076 rad` prediction. Invalid inertia selects scheduled fallback and
    disabled output is exactly zero. Heavy fixed/scheduled zero-return segments
    remain truthfully `Not settled` after five seconds.
  - Source project, learner contract, nonlinear runtime evidence and measured
    README results are implemented. Current SVG, trace and summary artifacts
    are bundled and the real VS Code rendered journey is independently
    accepted in the Modules 12–13 evidence directory.
- [x] `CELP-081` Lesson 71 — Extruder adaptive control. Teach online estimation,
  forgetting, excitation, projection, confidence, bounds and frozen fallback
  before adaptation. Prove drift response, loss of excitation, corrupted data
  and bounded recovery.
  - Specification-first implementation (2026-08-29): Specification 38 Lesson
    71 now defines one real single-screw polymer-extruder pressure loop and
    compares fixed, recipe-scheduled and supervised adaptive PI over a common
    grade transition and continuous unmeasured viscosity drift. It derives the
    two-parameter gain-and-bias regression, recursive least squares, forgetting,
    information determinant, bounded probe, projection, confidence,
    qualification, fallback and indirect PI design in words before equations.
    Persistent excitation is explicitly a data condition rather than
    experiment authorization; removing the authorized probe freezes learning
    immediately even while historical information remains eligible.
  - Focused learner red/green (2026-08-29, `trust-builder`): after direct
    TypeScript compilation,
    `TRUST_REPOSITORY_ROOT=$HOME/projects/trust-platform-control-curriculum-validation node ./node_modules/mocha/bin/mocha --ui tdd --timeout 20000 out/test/suite/control-lesson-state-space.test.js --grep "freezes bounded extruder adaptation when information or quality is lost"`
    first reached the expected behavior assertion and failed `2 !== 4` against
    the former schema-version-2 one-line adaptive placeholder. It now passes
    `1 passing` against the schema-version-4 executable lesson, including the
    separate data-eligibility/authorization explanation, actual Structured
    Text, exact measures, safe-state rules and Simulation/physical boundary.
  - Focused native red/green (2026-08-29, `trust-builder`): exact command
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum-dedicated cargo test -p trust-runtime --test control_curriculum_extruder_adaptive control_curriculum_extruder_adaptation_requires_information_and_bounds -- --exact --nocapture`
    was run with the corrected input-mapping harness against an isolated
    pre-authorization-gate source and reached the expected assertion failure at
    `60.0 s`: probe authorization and generated probe were both false/zero, but
    adaptation remained active and the estimated gain changed. The same
    command passes `1 passed` with the shipped gate, compiling and executing
    the actual lesson Structured Text and shared `Control.PID` while matching
    every estimator scan to an independent RLS oracle.
  - Current native Simulation evidence (2026-08-29): fixed, scheduled and
    adaptive grade-B pressure IAE is `158.457/134.651/109.792 bar*s`; total
    applied-speed movement is `1430.5/1457.9/1468.4 percentage points`;
    saturation is `2.8/2.9/3.0 s`; and all final 60-bar responses are truthfully
    `Not settled`. Qualified adaptive gain RMSE is `0.07350 bar/%` against the
    Simulation-only oracle. The grade transition produces 63 visible projection
    samples; boundary estimates remain unqualified. Removing probe authority
    freezes `K_hat=0.990217 bar/%` and `B_hat=0.390828 bar` immediately while
    information determinant decays from `23.542821` to `0.059641`; corrupt
    pressure applies zero speed, authorized healthy recovery resumes at
    `105.0 s`, a forced `2.3 bar/%` estimate projects to `1.5 bar/%`, and
    disabled output is zero.
  - Source project, learner contract, independent numerical qualification and
    measured README results are implemented. Current SVG, trace and summary
    artifacts are bundled and the real VS Code rendered journey is
    independently accepted in the Modules 12–13 evidence directory.
- [x] `CELP-082` Lesson 72 — Flexible-airframe robust control. Teach uncertainty
  forms, weights, worst-case gain, H-infinity norm, robust stability and robust
  performance before synthesis. Prove nominal/robust trade-offs over the
  declared uncertainty set without extrapolated guarantees.
  - Specification-first implementation (2026-08-29): Specification 38 Lesson
    72 now defines a ground vibration-test flexible-wing load-alleviation rig,
    not a flight or certification claim. It introduces structured and
    multiplicative uncertainty, sensitivity, complementary sensitivity,
    control sensitivity, engineering weights, the H-infinity norm, small gain,
    exact sampled-loop evaluation and the finite controller choice in physical
    words before notation. The declared authority is limited to 27 parameter
    cases, the residual envelope, and frequencies below the sampled Nyquist
    boundary.
  - Focused learner red/green (2026-08-29, `trust-builder`): after direct
    TypeScript compilation,
    `TRUST_REPOSITORY_ROOT=$HOME/projects/trust-platform-control-curriculum-validation node ./node_modules/mocha/bin/mocha --ui tdd --timeout 20000 out/test/suite/control-lesson-state-space.test.js --grep "qualifies sampled robust loop shaping on the flexible-wing rig"`
    first reached the intended assertion and rejected the former
    schema-version-2 placeholder. It now passes `1 passing` against the
    schema-version-4 lesson, including words-before-symbols theory, actual
    `Control.LowPass` and derivative-on-measurement `Control.PID`, exact
    zero-order-hold assumptions, predeclared measures, invalid-data handling,
    Simulation boundaries, and the causal handoff to networked control.
  - Focused native red/green (2026-08-29, `trust-builder`): exact command
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum-dedicated cargo test -p trust-runtime --test control_curriculum_flexible_airframe_robust control_curriculum_flexible_airframe_qualifies_sampled_robust_design -- --exact --nocapture`
    first reached the expected assertion and failed because the shipped lesson
    was still schema version `2`. The same command now passes `1 passed`,
    compiling and executing the actual Structured Text and shared Control
    library against two equivalent plant copies for every uncertainty case,
    while an independent exact-discrete frequency oracle checks 20,000 points
    per loop.
  - Current native Simulation evidence (2026-08-29): the aggressive nominal
    controller has mixed-sensitivity score `1050.0435`, sampled pole radius
    `1.008104`, and fails to settle in `17/27` cases. The selected robust
    candidate has weighted peaks `0.8333/0.7714/0.3117`, score `0.8333`, and
    worst sampled pole radius `0.976057`. Across the common gust experiments,
    worst nominal/robust IAE is `10.0757/0.0856 g*s`, peak bending is
    `1.6787/0.0790 g`, applied-surface travel is `1228.1/4.1 percentage
    points`, saturation is `9.30/0.00 s`, and worst robust settling is
    `2.12 s`. Bad measurement quality and disabled operation both produce the
    specified bounded behavior.
  - Source project, learner contract, independent sampled-frequency oracle and
    measured README results are implemented. Current SVG, trace and summary
    artifacts are bundled and the real VS Code rendered journey is
    independently accepted in the Modules 12–13 evidence directory.
- [ ] `CELP-M12` Module 12 gate — Independently verify nonlinear certificates,
  adaptive bounds and robust worst cases. Capture the operating-region,
  adaptation and robustness views with uncertainty clearly visible.
  - Current rendered acceptance (2026-08-30): exact native artifact update and
    no-update tests pass for Lessons 69–72; the real Extension Development Host
    copy/open/render journeys pass for all four lessons. Independent review
    rejected the first captures for a malformed table and clipped endpoint
    markers, then accepted the corrected captures. Evidence and current JSON
    interaction records live in
    `docs/internal/testing/evidence/vscode-control-curriculum/2026-08-30-modules-12-13/`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**

## 14. Networked, fault-aware, monitoring and diagnosis

Specification authority: §§3.13–3.14, Lessons 73–75.

- [x] `CELP-083` Lesson 73 — Remote pumping networked control. Teach timestamps,
  latency, jitter, loss, reorder, stale data, synchronization, buffering and
  delay margin before fault injection. Prove stability boundaries and safe
  handling from real loop data flow.
  - Specification-first implementation (2026-08-29): Specification 38 Lesson
    73 now defines one remotely coordinated high-zone pumping station with a
    local pressure controller and final local drive authority. It derives
    locally calculated conservative packet age, sequence/session limits,
    latency and jitter distinctions, zero-wait buffer policy, a linear
    model-based delay margin, conservative latency budget, fault latch, local
    fallback and explicitly approved matched-command recovery in words before
    equations. The specification separates injected Simulation packets from
    measured transport or physical-station evidence.
  - Focused learner red/green (2026-08-29, `trust-builder`): after direct
    TypeScript compilation,
    `TRUST_REPOSITORY_ROOT=$HOME/projects/trust-platform-control-curriculum-validation node ./node_modules/mocha/bin/mocha --ui tdd --timeout 20000 out/test/suite/control-lesson-state-space.test.js --grep "qualifies bounded network ownership on the remote pumping station"`
    first reached the intended contract assertion and failed `2 !== 4` against
    the sender-trusted-age schema-version-2 placeholder. It now passes
    `1 passing` against the schema-version-4 eleven-stage lesson, including
    words-before-symbols communication theory, actual Structured Text, exact
    delay calculations, defined decision measures, Control Studio procedure,
    and explicit Simulation/physical boundaries.
  - Focused native red/green (2026-08-29, `trust-builder`): exact command
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum-dedicated cargo test -p trust-runtime --test control_curriculum_networked_pumping control_curriculum_networked_pumping_bounds_age_order_and_transfer -- --exact --nocapture`
    first reached the expected behavior assertion and rejected schema version
    `2`. The same command now passes `1 passed`, compiling and executing the
    shipped ST and shared `Control.PID` against independent naive and protected
    first-order header simulations while matching an independent packet-policy
    and exact-discrete plant oracle on every scan.
  - Current native Simulation evidence (2026-08-29): calculated crossover,
    delay margin, budget and reserve are `1.984313 rad/s`, `0.854766 s`,
    `0.47 s` and `0.384766 s`. The naive/protected paths accept `6/0`
    ineligible packets; pressure IAE is `3.2508/0.0000 bar*s`; peak pressure
    departure is `0.3539/0.0000 bar`; applied-speed travel is `550.0/0.0
    percentage points`; maximum approved-recovery discontinuity is `0.00
    percentage points`; and qualified protected remote availability is
    `72.4%`. Every invalid or expired condition selects local ownership on the
    detecting `100 ms` scan.
  - Registration validation passes with `788` discovered facts and `69/69`
    registered VS Code test files. Source project, learner contract, numerical
    delay calculation, independent policy/plant qualification and measured
    README results are implemented. Current SVG, trace and summary artifacts
    are bundled and the real VS Code rendered journey is independently
    accepted in the Modules 12–13 evidence directory.
- [x] `CELP-084` Lesson 74 — Pumping multirate and fault-aware control. Teach
  resampling, freshness, plausibility, redundancy, detection, isolation,
  degradation, watchdog and recovery before testing. Prove sensor/actuator
  failures, safe modes and controlled restoration.
  - Specification-first implementation (2026-08-29): Specification 38 Lesson
    74 now continues the same high-zone pumping station with two `100 ms`
    pressure transmitters, a `500 ms` reservoir-level transmitter and `200 ms`
    independent drive feedback. It defines timestamp-preserving resampling,
    freshness, plausibility, physical and model-based analytical redundancy,
    residuals, bounded isolation, conservative slow-level projection,
    actuator watchdog, deterministic fault priority, degraded/safe modes,
    acknowledgement, cause removal, ten-scan qualification, reset authority and
    rate-limited restoration in words before equations.
  - Focused learner red/green (2026-08-29, `trust-builder`): registered test
    `isolates multirate pumping faults before supervised recovery` first
    reached the expected assertion because no executable Lesson 74 project
    existed. After direct TypeScript compilation, the same test now passes
    `1 passing` against the schema-version-4 eleven-stage lesson, actual ST,
    multirate device timeline, exact residual/predictor equations, defined
    measures, Control Studio procedure and Simulation/physical boundary.
  - Focused native red/green (2026-08-29, `trust-builder`): exact command
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum-dedicated cargo test -p trust-runtime --test control_curriculum_pumping_faults control_curriculum_pumping_faults_isolate_degrade_and_recover -- --exact --nocapture`
    first reached the expected missing-project assertion. The same command now
    passes `1 passed`, compiling and executing the new project and shared
    `Control.PID` against five connected Simulation paths while independent
    calculations reproduce the predictor, clean physical response, sample
    cadence and selection decisions.
  - Current native Simulation evidence (2026-08-29): pressure-A bias is
    isolated on scan `52` (`5.2 s`, the third biased `100 ms` sample), with
    zero healthy false trips and zero wrong protected sensor selections.
    Ambiguous pressure, stale level, fresh low level and confirmed actuator
    faults enter zero-output safe mode on scans `120`, `156`, `190` and `230`.
    The stuck actuator is confirmed at `23.0 s`, within three new `200 ms`
    feedback updates after the residual becomes eligible. Acknowledgement and
    premature reset do not clear faults; approved restoration reaches a maximum
    `8.2 %/s` against the `10 %/s` limit.
  - Registration validation passes with `789` discovered facts and `69/69`
    registered VS Code test files. Specification, source project, learner
    contract, native oracle and measured README evidence are implemented.
    Current SVG, trace and summary artifacts are bundled and the real VS Code
    rendered journey is independently accepted in the Modules 12–13 evidence
    directory.
- [x] `CELP-085` Lesson 75 — Diagnose before retuning. Teach retained baselines,
  event detection and signatures for oscillation, stiction, drift, saturation,
  interaction, mismatch and schedule transitions before diagnosis. Prove that
  tuning, maintenance, instrumentation and process remedies are distinguished.
  - Focused learner red/green (2026-08-29, `trust-builder`): exact command
    `TRUST_REPOSITORY_ROOT=/home/johannes/projects/trust-platform-control-curriculum-validation ./node_modules/.bin/mocha --ui tdd --timeout 20000 ./out/test/suite/control-lesson-industrial-structures.test.js --grep "diagnoses blinded steam-loop evidence before selecting a remedy"`
    first reached the expected assertion that the old lesson used schema `2`
    instead of the required schema `4`. After implementation, the same focused
    test passes `1 passing`; it parses the complete lesson contract, checks all
    eleven teaching stages, words-before-symbols definitions, actual Structured
    Text signals, blinded workflow, measures, decision boundaries, eight cases,
    and the sealed answer key.
  - Focused native red/green (2026-08-29, `trust-builder`): exact command
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum-dedicated cargo test -p trust-runtime --test control_curriculum_loop_diagnosis control_curriculum_loop_diagnosis_classifies_blinded_evidence -- --exact --nocapture`
    first compiled and reached the expected schema assertion (`left: 2`,
    `right: 4`). The same command now passes `1 passed`, compiles and executes
    the shipped Structured Text with `Control.PID`, loads all eight shipped
    Simulation configurations, and independently classifies all eight complete
    records with the evidence-qualified remedy. Removing stem feedback from the
    stiction record returns `Inconclusive` instead of guessing poor tuning.
  - Test registration validation passes with `790` discovered facts and
    `69/69` registered VS Code test files. Specification, source project,
    executable casebook, learner contract, native oracle and Simulation versus
    physical boundary are implemented. Current SVG, trace and summary artifacts
    are bundled and the real VS Code rendered journey is independently
    accepted in the Modules 12–13 evidence directory.
- [x] `CELP-M13` Module 13 gate — Run a failure journey from network impairment
  through diagnosis and recovery. Confirm deterministic retained evidence and
  no automatic retuning from an unqualified diagnosis.
  - Current rendered acceptance (2026-08-30): exact native artifact update and
    no-update tests pass for Lessons 73–75; the real Extension Development Host
    copy/open/render journeys pass for all three lessons. Independent review
    accepted network validity directly and accepted the fault-code and
    diagnosis views after their mappings and trace identities were made
    explicit. The diagnosis remains `Inconclusive` when stem evidence is
    removed; no automatic retuning is inferred.

## 15. Integrated capstones

Specification authority: §3.15, Lessons 76–84.

- [x] `CELP-086` Lesson 76 — Cold-storage capstone. Require independent selection
  and validation of ON/OFF thresholds, hysteresis, dwell, noise response,
  restart and safety without introducing continuous control unnecessarily.
  - Focused learner red/green (2026-08-29, `trust-builder`): exact command
    `TRUST_REPOSITORY_ROOT=/home/johannes/projects/trust-platform-control-curriculum-validation ./node_modules/.bin/mocha --ui tdd --timeout 20000 ./out/test/suite/control-lesson-capstones.test.js --grep "commissions the cold room from requirements instead of a prescribed answer"`
    first reached the expected assertion `Lesson 76 executable cold-storage
    capstone is missing`; the same command now passes `1 passing`. The
    registered contract checks the independent candidate choice, recalled
    words-before-symbols theory, actual discrete Structured Text, threshold and
    dwell derivation, complete Control Studio journey, measures, abnormal
    validation, evidence boundary and transfer to Lesson 77.
  - Focused native red/green (2026-08-29, `trust-builder`): exact command
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum-dedicated cargo test -p trust-runtime --test control_curriculum_capstones control_curriculum_cold_storage_capstone_selects_safe_discrete_control -- --exact --nocapture`
    first compiled and reached the expected missing-project assertion; the same
    command now passes `1 passed`. It compiles the shipped Structured Text,
    executes the shipped seeded Simulation plant, and independently compares
    all three candidates. Current evidence records `204/5/5` quiet-window
    applied-state transitions for single-threshold, hysteresis-only and selected
    hysteresis-plus-dwell control, `6.4/5.1 s` minimum completed on/off times,
    `2.600 degC` door-event peak, safe restart/quality/manual behavior and exact
    deterministic replay.
  - Test registration validation passes with `791` discovered facts and
    `70/70` registered VS Code test files. Specification, source project,
    learner contract, native plant execution and measured README values are
    implemented. The bundled project now contains the current SVG, trace CSV
    and summary CSV. Its real gallery/copy/render journey passes; independent
    review rejected the first plot, then accepted the recapture after product
    limits, switching thresholds and requested/applied commands were exposed.
- [x] `CELP-087` Lesson 77 — Water-heater capstone. Require physical and
  identified models, structure choice, tuning comparison, limits, anti-windup,
  noise, robustness and commissioning evidence.
  - Focused learner red/green (2026-08-29, `trust-builder`): exact command
    `TRUST_REPOSITORY_ROOT=/home/johannes/projects/trust-platform-control-curriculum-validation ./node_modules/.bin/mocha --ui tdd --timeout 20000 ./out/test/suite/control-lesson-capstones.test.js --grep "commissions the recirculating heater from models through an unapplied PI decision"`
    first reached the expected assertion `Lesson 77 executable water-heater
    capstone is missing`; after implementation it exposed and rejected a real
    teaching gap because the physical model, identified model, independent
    record and residual evidence were not connected explicitly. The same
    command now passes `1 passing` after that causal explanation was added. The
    registered contract checks all eleven teaching stages, words-before-symbols
    definitions, the real Structured Text signals, physical and identified
    modelling roles, independent validation, structure choice, SIMC candidates,
    measures, complete Control Studio workflow, unapplied recommendation,
    rollback, evidence boundary and transfer to Lesson 78.
  - Focused native red/green (2026-08-29, `trust-builder`): exact command
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum-dedicated cargo test -p trust-runtime --test control_curriculum_process_capstone control_curriculum_water_heater_capstone_commissions_unapplied_pi -- --exact --nocapture`
    first compiled and reached the expected missing-project assertion; the same
    command now passes `1 passed`. It compiles and executes four real
    `Control.PID` instances against the shipped nominal, low-gain, high-gain and
    increased-delay Simulation plants. Current evidence identifies process
    gain/time constant/dead time as `0.1994 / 4.90 s / 1.10 s`, measures
    P-plus-bias load error `2.143 degC`, Robust/Balanced PI integrated absolute
    error `23.93 / 14.17 degC*s`, and applied-power total variation
    `28.7 / 41.8 percentage points`; all uncertainty cases satisfy the declared
    PI final-window error bound, manual return is bounded, requested and applied
    signals are checked separately, and controller revision `77` remains
    unchanged.
  - Test registration validation passes with `792` discovered facts and
    `70/70` registered VS Code test files. Specification, source project,
    learner contract, native plant execution and measured README decision are
    implemented. Robust PI is retained as the unapplied recommendation because
    both candidates meet mandatory limits and it uses less actuator movement;
    Balanced PI remains a requirement-dependent faster alternative. The
    bundled SVG, trace CSV and summary CSV pass no-update drift comparison, and
    the current real gallery/copy/render journey is independently accepted in
    the capstone evidence directory.
- [x] `CELP-088` Lesson 78 — Servo capstone. Require P/PD/PID choice, root locus,
  Bode/sensitivity, digital qualification, nonidealities, noise, load and torque
  evidence.
  - Focused learner red/green (2026-08-29, `trust-builder`): exact command
    `TRUST_REPOSITORY_ROOT=/home/johannes/projects/trust-platform-control-curriculum-validation ./node_modules/.bin/mocha --ui tdd --timeout 20000 ./out/test/suite/control-lesson-capstones.test.js --grep "commissions a flexible indexing servo through time frequency and digital evidence"`
    first reached the expected assertion `Lesson 78 executable servo capstone
    is missing`; the same command now passes `1 passing`. The registered
    contract checks all eleven teaching stages, words-before-symbols signal and
    mechanical definitions, the actual composed `Control.PID`/`Control.LowPass`
    Structured Text, P/PD/PID causal selection, shaft-mode prediction,
    derivative-on-measurement and filtering, root locus, Bode, sensitivity,
    sampled/held/delayed execution, exact Control Studio journey, defined
    measures, uncertainty and abnormal cases, unapplied recommendation,
    rollback, Simulation boundary and transfer to Lesson 79.
  - Focused native red/green (2026-08-29, `trust-builder`): exact command
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum-dedicated cargo test -p trust-runtime --test control_curriculum_motion_capstone control_curriculum_servo_capstone_qualifies_filtered_pid_digitally -- --exact --nocapture`
    first compiled and reached the expected missing-project assertion; the same
    command now passes `1 passed`. It compiles and executes the actual
    Structured Text against four equivalent two-inertia table copies plus
    heavy-payload, reduced-damping and doubled-noise configurations. Independent
    mechanical and frequency oracles produce `11.180 rad/s` flexible frequency,
    `3.096 rad/s` crossover, `38.34 deg` phase margin, `1.94` gain margin and
    `2.07` maximum sensitivity. An independent linear recurrence matching the
    implemented filter, PID and RK4 plant update gives dominant pole radius
    `0.998179`. P does not settle; PD settles in `2.90 s` but fails the load-
    offset requirement; selected PID settles in `4.46 s`, overshoots `31.4 %`,
    ends with `0.0100 rad` load error and `0.0571 N*m` quiet torque RMS. The
    unchanged delayed `100 ms` candidate settles in `7.40 s` and overshoots
    `62.4 %`, so it is rejected by predeclared requirements. Heavy and reduced-
    damping errors are each `0.0100 rad`; doubled-noise torque RMS rises to
    `0.1141 N*m`; manual and invalid-quality controlled stops respect the
    fine-axis slew limit and reach zero; revision `78` remains unchanged.
  - Test registration validation passes with `793` discovered facts and
    `70/70` registered VS Code test files. Specification, source project,
    learner contract, native plant execution, frequency/discrete oracle and
    measured README decision are implemented. The bundled decision SVG, full
    trace CSV and summary CSV preserve the time, torque and frequency/discrete
    conclusions without an ornamental plot matrix. The real gallery/copy/render
    journey is independently accepted.
- [x] `CELP-089` Lesson 79 — Reactor capstone. Require a coherent startup,
  feedback/feedforward, cascade, split-range, override, tracking, constraint and
  shutdown architecture rather than independent example fragments.
  - Focused learner red/green (2026-08-29, `trust-builder`): exact command
    `TRUST_REPOSITORY_ROOT=/home/johannes/projects/trust-platform-control-curriculum-validation ./node_modules/.bin/mocha --ui tdd --timeout 20000 ./out/test/suite/control-lesson-capstones.test.js --grep "commissions one tracked reactor architecture across its operating sequence"`
    first reached the expected assertion `Lesson 79 executable reactor capstone
    is missing`; after the project and the necessary `degC/load-unit` schema
    unit were implemented, the same command passes `1 passing`. The registered
    contract checks the eleven-stage causal account, physical quantities before
    symbols, inner-first commissioning, actual composed `Control.PID` Structured
    Text, measured-load prediction, signed split range, selector and tracking,
    operating modes, defined measures, complete Control Studio journey,
    Simulation boundary, unapplied revision, rollback and transfer to Lesson 80.
  - Focused native red/green (2026-08-29, `trust-builder`): exact command
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum-dedicated cargo test -p trust-runtime --test control_curriculum_reactor_capstone control_curriculum_reactor_capstone_coordinates_tracked_authority -- --exact --nocapture`
    first compiled and reached the expected missing-project assertion; the same
    command now passes `1 passed`. It compiles and executes the actual reactor
    architecture against two equivalent two-state Simulation plants. The
    commissioned inner design has a `6.0` plant time-scale ratio and uses the
    qualified `0.525 degC/load-unit` feedforward gain. Feedback-only/integrated
    cold-load IAE is `17.52 / 6.91 degC*s`, maximum departure is
    `0.681 / 0.552 degC`, and applied-valve total variation is
    `16.18 / 35.88 percentage points`. Tracked override limits the unmeasured-
    heat peak to `84.526 degC`, maximum applied-valve jump is `5.00 percentage
    points`, controlled shutdown ends at `71.690 degC` with both utilities
    closed, invalid selected authority fails closed, and revision `79` remains
    unchanged.
  - Specification, source project, learner contract, native plant execution and
    measured README decision are implemented. The bundled SVG, full trace CSV
    and summary CSV retain reactor, jacket, feedforward, authority and utility
    evidence. The real gallery/copy/render journey passes; independent review
    rejected the first utility panel and accepted the recapture after steam and
    cooling-water trace identities were made explicit.
  - Test registration validation passes with `794` discovered facts and
    `70/70` registered VS Code test files.
  - Control Studio mode handoff red/green and rendered proof (2026-08-30):
    focused native command
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum-dedicated cargo test -p trust-runtime --test control_curriculum_reactor_capstone control_curriculum_reactor_capstone_publishes_control_studio_auto_mode -- --exact --nocapture`
    reached the expected assertion because the project exposed only its numeric
    operating-mode encoding, then passed after the shipped ST published the
    explicit Boolean `%MX124.0` `ControllerAuto` indication. The real VS Code
    Development Host journey passed `1` test in `55 s`; current evidence in
    `docs/internal/testing/evidence/vscode-control-studio/2026-08-30-reactor-capstone/`
    shows live Simulation SP/PV/MV at `80 degC / 80 degC / 50 %`, confirmed
    Auto mode, the approved `80` to `81 degC` bounded setpoint experiment with
    live Abort, restoration, and the honest automatic-design boundary. The
    automatic workflow rejects this two-state cascade because its qualified
    automatic route currently requires an FOPDT model; no gains are applied.
    Native retained CSV/SVG artifacts are present and the bundled project is
    synchronized. Independent acceptance is recorded in the capstone evidence
    directory. A future separately specified multi-loop architecture-design
    workflow remains outside this lesson; the honest FOPDT-only rejection is
    not a defect in the current automatic tuner.
- [x] `CELP-090` Lesson 80 — Gravity-tank capstone. Require fixed-gain failure,
  nonlinear/local models, scheduled design, boundary logic, tracking and full-
  range evidence.
  - Focused learner red/green (2026-08-29, `trust-builder`): exact command
    `TRUST_REPOSITORY_ROOT=/home/johannes/projects/trust-platform-control-curriculum-validation ./node_modules/.bin/mocha --ui tdd --timeout 20000 ./out/test/suite/control-lesson-capstones.test.js --grep "commissions gravity-tank scheduling from local models through tracked recovery"`
    first reached the expected assertion `Lesson 80 executable gravity-tank
    capstone is missing`; the same command now passes `1 passing`. The
    registered contract checks the complete eleven-stage account, words before
    balance and scheduling symbols, separately restored local identification,
    actual composed `Control.PID`/`Control.PIDGainScheduler` Structured Text,
    requested/applied flow, atomic records, modes and tracking, defined
    measures, exact Control Studio journey, abnormal cases, Simulation boundary,
    unapplied revision, rollback and transfer to Lesson 81.
  - Focused native red/green (2026-08-29, `trust-builder`): exact command
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum-dedicated cargo test -p trust-runtime --test control_curriculum_gravity_tank_capstone control_curriculum_gravity_tank_capstone_qualifies_full_range_schedule -- --exact --nocapture`
    first compiled and reached the expected missing-project assertion; the same
    command now passes `1 passed`. It compiles and executes the actual fixed and
    atomically scheduled controllers against two equivalent nonlinear gravity-
    tank plants for the complete `420 s` sequence and a separate abnormal run.
    Independent balance oracles produce equilibrium flows
    `0.027386 / 0.037081 / 0.044721 m3/s` and local gain/time-constant values
    `21.9089 / 29.6648 / 35.7771`. Fixed/scheduled regional IAE is
    `[9.4206, 10.4565, 8.4930, 10.0708] / [8.2063, 9.9208, 7.5196,
    6.8101] m*s`; worst IAE improves `5.12 %`. Maximum departure is
    `0.2773 / 0.2657 m`, applied-pump total variation is
    `0.25403 / 0.26092 m3/s`, maximum active-Ki step is `0.00005667`, and
    maximum authority-recovery bump is `0.00050000 m3/s`. Invalid quality,
    unavailable scheduling, unavailable pump and out-of-envelope requests fail
    closed; Manual and ControlledStop track; revision `80` remains unchanged.
  - Specification, source project, learner contract, native nonlinear-plant
    execution, independent balance oracle and measured README decision are
    implemented. The bundled SVG, trace CSV and summary CSV retain the local
    and full-range comparison, pass no-update drift comparison, and are
    independently accepted in the real gallery/copy/render journey.
  - Test registration validation passes with `795` discovered facts and
    `70/70` registered VS Code test files.
- [x] `CELP-091` Lesson 81 — Conveyor-oven delay capstone. Require identification,
  Bode/Nyquist/delay analysis and an evidence-based choice among PI,
  feedforward and Smith prediction under uncertainty.
  - Focused learner red/green (2026-08-29, `trust-builder`): exact command
    `TRUST_REPOSITORY_ROOT=/home/johannes/projects/trust-platform-control-curriculum-validation ./node_modules/.bin/mocha --ui tdd --timeout 20000 ./out/test/suite/control-lesson-capstones.test.js --grep "selects conveyor-oven delay compensation from time frequency and mismatch evidence"`
    first reached the expected missing-project assertion. During measured
    tuning, the strengthened implementation-consistency assertion also failed
    as expected while the ST still used the rejected Smith gains. The same
    command now passes `1 passing` with the selected `6 s` design present in
    the explanation, descriptor and actual Structured Text.
  - Focused native red/green (2026-08-29, `trust-builder`): exact command
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum-dedicated cargo test -p trust-runtime --test control_curriculum_conveyor_oven_capstone -- --nocapture`
    first reached the expected missing-project assertion. The implemented
    campaign then honestly rejected the initial Smith candidate at the
    behavioral gate (`45.6 s` versus `52.4 s`, short of `15 %`) and exposed a
    one-scan timestamp mismatch in the oracle. The corrected scan-aligned
    oracle and `6 s` model-based Smith tuning now pass `1 passed` while
    compiling and executing the actual lesson ST against nominal, moderate,
    severe and abnormal delayed plants.
  - Current native evidence records ordinary crossover `0.03125 rad/s`, phase
    margin `61.35 deg`, gain margin `3.14`, delay margin `34.26 s`, complete-
    contour Nyquist `N = 0` with minimum critical-point distance `0.629`, and
    rejects the next aggressive PI at `32.70 deg` phase margin. Ordinary/Smith
    setpoint IAE is `525.81/425.25 degC*s`; ordinary/feedforward regulatory IAE
    is `52.25/17.80 degC*s` (`65.9 %` better); 90-percent rise is
    `52.5/43.1 s` (`17.9 %` faster); applied-heater total variation is
    `86.85/88.35/418.65 pp`; ordinary overshoot/final error are
    `0.232/0.067 degC`; nominal/moderate peak correction is
    `2.249/3.038 degC`; and severe mismatch produces `1,119` deterministic
    measured-feedback fallback scans. Invalid upstream quality removes only
    feedforward; invalid primary quality and unavailable heater fail closed;
    Manual and ControlledStop track; revision `81` remains unapplied.
  - Specification, executable project, learner contract, actual PID/Smith
    composition, independent frequency/Nyquist oracle, mismatch/fallback
    campaign and measured README decision are implemented. The bundled decision
    SVG, trace CSV and summary CSV retain the identification, time and frequency
    conclusions and pass no-update drift comparison. The real gallery/copy/
    render journey is independently accepted.
  - Test registration validation passes with `796` discovered facts and
    `70/70` registered VS Code test files.
- [x] `CELP-092` Lesson 82 — Distillation MIMO capstone. Require interaction
  tests, RGA, pairing, decentralized baseline, decoupling, centralized candidate
  and multivariable robustness evidence.
  - Focused learner red/green (2026-08-29, `trust-builder`): exact command
    `TRUST_REPOSITORY_ROOT=/home/johannes/projects/trust-platform-control-curriculum-validation ./node_modules/.bin/mocha --ui tdd --timeout 20000 ./out/test/suite/control-lesson-capstones.test.js --grep "commissions a distillation architecture from interaction through feasibility evidence"`
    first reached the expected missing-project assertion. A later focused slice
    reached the expected missing-`ArchitectureActive` assertion before invalid-
    request fallback was implemented. The same command now passes `1 passing`.
    It checks the eleven-stage causal flow, words-before-symbols four-path/RGA/
    singular-value/sensitivity theory, exact Control Studio journey, actual ST
    composition, two distinct shipped models, modes, feasibility, evidence
    boundary, unapplied revision and transfer to Lesson 83.
  - Focused native red/green (2026-08-29, `trust-builder`): exact command
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum-dedicated cargo test -p trust-runtime --test control_curriculum_distillation_capstone control_curriculum_distillation_capstone_selects_robust_architecture -- --exact --nocapture`
    reached the expected missing-project assertion in an isolated remote
    baseline with the copied capstone directory temporarily absent and restored
    immediately afterward. The expanded test then exposed revision publication
    as zero for a located initialized variable; explicit scan publication fixed
    that behavior. The same command now passes `1 passed` in `192.06 s` while
    compiling and executing the actual two-PID/static-decoupler ST.
  - The native campaign performs independently restored reflux, reboiler and
    unused simultaneous identification records, verifies all four nominal gains
    within `0.01`, all four `63.2 %` response times within `1.0 s`, simultaneous
    prediction error within `0.01 pp`, a nonsingular determinant, diagonal-
    positive/off-diagonal-negative RGA and `Lambda11 = 1.272727` within `0.01`.
    It also checks invalid architecture fallback to active direct mapping,
    invalid quality and unavailable-valve fail-closed behavior, Manual tracking,
    ControlledStop slew/restoration and unchanged revision `82`.
  - Current common architecture evidence records nominal/stronger sensitivity
    peaks `1.005030/1.006677` for direct and `1.009405/1.007105` for static
    decoupling. Stronger simultaneous final error is
    `0.1728/0.0767/0.0767 pp` for direct/static/central coordination; direct is
    ineligible while static and central pass. Their feasible records are
    identical, so central coordination earns no `20 %` benefit. The separately
    labelled `+2/+2` record remains Infeasible at
    `1.0275/0.4174/0.2747 pp`; central scale reaches `0.8436` and preserves
    direction but does not claim the impossible target. Static decoupling is
    selected for the declared normal envelope. The central candidate is
    explicitly a joint constraint coordinator, not MPC or a falsely claimed
    centralized MIMO controller.
  - Specification, executable capstone, learner contract, actual shared
    control objects, identification/RGA, singular-direction and sensitivity
    oracles, normal/infeasible campaign, modes/fallback and measured README
    decision are implemented. The bundled decision SVG, trace CSV and summary
    CSV retain the interaction, model, robustness and feasibility evidence and
    pass no-update drift comparison. The real gallery/copy/render journey is
    independently accepted.
  - Test registration validation passes with `797` discovered facts and
    `70/70` registered VS Code test files.
- [x] `CELP-093` Lesson 83 — HVAC predictive-control capstone. Require estimator,
  constraint and optimization evidence plus a fair comparison against strong
  PI/cascade and a tested infeasibility fallback.
  - Focused learner red/green (2026-08-29, `trust-builder`): exact command
    `TRUST_REPOSITORY_ROOT=/home/johannes/projects/trust-platform-control-curriculum-validation editors/vscode/node_modules/.bin/mocha --ui tdd --timeout 20000 editors/vscode/out/test/suite/control-lesson-capstones.test.js --grep "decides whether predictive HVAC control earns its estimator and actuator cost"`
    reached the expected lesson-content assertion because the capstone README
    did not yet identify the real two-zone hydronic plant and complete causal
    teaching contract. The same command now passes `1 passing`. A preceding
    root-level `node_modules` attempt was a harness-path error and is explicitly
    excluded from red evidence.
  - Focused native red/green (2026-08-29, `trust-builder`): exact command
    `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum-dedicated cargo test -p trust-runtime --test control_curriculum_building_hvac_capstone control_curriculum_building_hvac_capstone_makes_evidence_based_decision -- --exact --nocapture`
    compiled and reached the expected missing-Lesson-83-project assertion in an
    isolated remote baseline with the capstone directory temporarily absent
    and restored immediately afterward. The first implementation run then
    exposed a real located-initializer defect: estimator innovation was
    correct, but the mapped estimator gain remained zero. Explicitly publishing
    the qualified `0.4` gain each scan corrected the behavior. The same command
    now passes `1 passed` in `269.58 s` while compiling and executing the actual
    capstone ST and four common 20-minute controller comparisons.
  - Current native evidence records PI/Balanced/actuator-conservative error
    integrals `158.67/146.72/221.57 degC*s`, tariff-weighted heating
    `46,624/46,200/45,660 cost-percent-s`, applied-heater total variation
    `18.7/160.0/80.0 pp`, zero comfort violation and peak shared heating
    `166.3/160.0/160.0 %`. Balanced and conservative MPC use about `8.6x` and
    `4.3x` PI movement, both failing the declared `2.0x` application limit, so
    PI remains the application recommendation and MPC remains unapplied.
  - The independent mismatch test passes with raw/corrected one-step residual
    RMS `0.16434/0.01363 degC` and both additive-load estimates converging to
    `-0.16335 degC/interval`. The campaign also proves 729-sequence oracle
    agreement, individual/move/shared limits, completed infeasibility, invalid-
    configuration fallback, invalid-quality and unavailable-heater fail-closed
    behavior, bounded Manual and ControlledStop authority, excessive-
    innovation rejection, next-solve recovery and unchanged revision `83`.
  - The specification, executable project, mismatch plant, learner contract,
    actual estimator/PI/finite-grid optimizer, independent oracle, fair
    architecture screening, abnormal campaign and measured README decision are
    implemented. The bundled decision SVG, trace CSV and summary CSV retain the
    model, estimator, optimization and fair-comparison evidence and pass
    no-update drift comparison. The real gallery/copy/render journey is
    independently accepted.
  - Test registration validation passes with `798` discovered facts and
    `70/70` registered VS Code test files.
- [x] `CELP-094` Lesson 84 — Independent engineering capstone. Require a learner-
  selected real plant, requirements, safety, experiments, modelling only when
  justified, simplest viable architecture, stability/robustness proof, abnormal
  validation, approved application/rollback plan and reproducible report.
  - Focused learner red/green (2026-08-29, `trust-builder`): exact command
    `TRUST_REPOSITORY_ROOT=/home/johannes/projects/trust-platform-control-curriculum-validation editors/vscode/node_modules/.bin/mocha --ui tdd --timeout 20000 editors/vscode/out/test/suite/control-lesson-capstones.test.js --grep "defends an independent compressed-air control-system recommendation"`
    first reached the expected missing-project assertion. After the initial
    reference design was analytically rejected, the revised conformance test
    reached the expected stale-`Kp = 8`, `Ki = 1.2`, `-0.30 bar/s` content
    assertion before the qualified model/tuning was implemented. The same
    command now passes `1 passing` and checks the full causal teaching flow,
    learner-selection boundary, real reference equipment, words-before-symbols
    physics, identification, architecture screening, actual ST, workflow,
    decision criteria, abnormal authority, report and physical boundary.
  - Focused native controller red/green (2026-08-29, `trust-builder`): exact
    command `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum-dedicated cargo test -p trust-runtime --test control_curriculum_independent_capstone control_curriculum_independent_capstone_defends_selected_architecture -- --exact --nocapture`
    compiled and reached the expected missing-project assertion in an isolated
    remote baseline, then exposed the missing measured-interval/`TimingValid`
    behavior. After adding the specified `0.8..1.2 s` timing authority and
    cycle-time-scaled PID/slew path, the same command passes `1 passed` in
    `1.68 s` while executing the actual two-PID ST against nominal and
    uncertainty plants.
  - Focused native identification red/green (2026-08-29, `trust-builder`): exact
    command `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/control-curriculum-dedicated cargo test -p trust-runtime --test control_curriculum_independent_capstone control_curriculum_independent_capstone_identifies_and_validates_receiver_model -- --exact --nocapture`
    reached the expected missing-project assertion in its own isolated remote
    baseline and now passes `1 passed` in `1.12 s`. Bounded restored experiments
    identify `Tc = 5.000 s` and
    `Kr = 0.005000 bar/(%*s)`; unused validation measures/predicts delivery
    `54.323/54.323 %` and pressure `7.0419/7.0419 bar`.
  - Independent design evidence records poles `-0.02848` and
    `-0.08576 +- j0.16661 1/s`, crossover `0.15864 rad/s`, phase margin
    `42.62 deg`, delay margin `4.69 s` and sensitivity peak `1.6258`. Nominal
    feedback/integrated IAE is `23.70/7.72 bar*s`, maximum departure
    `0.483/0.282 bar`, applied movement `59.3/63.2 pp` and zero normal-envelope
    violation. The integrated candidate improves IAE `67.4 %` for `6.6 %`
    additional movement.
  - With `8 s` delivery lag, `0.004 bar/(%*s)` rate gain, an unmeasured `5 %`
    leak, deterministic `+-0.005 bar` noise and alternating `0.9/1.1 s`
    intervals, feedback/integrated IAE is `28.62/16.75 bar*s`, departure
    `0.534/0.359 bar`, movement `164.1/165.4 pp` and normal-envelope violation
    `0.169/0.000 bar*s`. PI plus qualified demand feedforward passes the
    predeclared `30 %` improvement and `25 %` movement requirements and is the
    simplest selected reference architecture. Invalid demand removes only
    feedforward; invalid pressure/delivery, invalid timing and unavailable
    compressor remove automatic authority; Manual/ControlledStop/recovery are
    bounded; revision `84` remains unapplied.
  - Specification, executable reference project, nominal/uncertainty plants,
    learner contract, shared PID composition, independent identification/time/
    frequency oracles, architecture screening, abnormal campaign and measured
    README decision are implemented. The bundled decision SVG, trace CSV and
    summary CSV retain the experiment, model, time and frequency evidence and
    pass no-update drift comparison. The real gallery/copy/render journey is
    independently accepted. A future learner-selected submission is external
    course work; the shipped compressed-air reference proves the executable
    assignment route without fabricating a learner or physical-plant result.
  - Test registration validation passes with `799` discovered facts and
    `70/70` registered VS Code test files.
- [ ] `CELP-M14` Module 14 gate — Have the capstone owner defend every material
  design choice and evidence class. Reject unexplained technique selection,
  hidden precomputed behavior, or simulation presented as physical proof.
  - Accepted (2026-08-30): all nine capstones compile and execute their shipped
    Structured Text against the declared Simulation plants. Exact native
    update and no-update runs retain deterministic SVG, trace CSV and summary
    CSV evidence; the registered learner contract passes `9` tests. All nine
    real Extension Development Host gallery/copy/render journeys pass and are
    independently accepted in
    `docs/internal/testing/evidence/vscode-control-curriculum/2026-08-30-capstones/acceptance.md`.
    Each README explains why its selected or rejected architecture follows
    from declared requirements, keeps every controller revision unapplied, and
    distinguishes Simulation evidence from physical-plant proof. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**

## 16. Learning experience and documentation acceptance

- [x] `CELP-095` Verify every lesson is understandable from its own learner-
  facing material while linking to deeper shared theory without requiring the
  learner to search for missing prerequisites.
- [x] `CELP-096` Verify every practical instruction follows its theory and
  worked prediction; no button sequence appears before the reason and expected
  signal behavior are explained.
- [x] `CELP-097` Verify every plot is introduced before use and explains axes,
  units, signals, operating event, relevant region and engineering conclusion.
  Verify every README teaching figure follows Specification 38 §2.1.1,
  including physical-words-first order, alt text, caption, executable evidence,
  deterministic source, equivalent scale, and theme-safe distinctions.
- [x] `CELP-098` Verify Bode, Nyquist, root-locus, pole-zero, sensitivity,
  state-estimation, uncertainty, MIMO and MPC views remain readable with the
  main engineering evidence visually dominant.
- [x] `CELP-099` Verify numeric presentation uses appropriate decimals for
  sensor resolution and tolerances across tables, traces, tooltips, exports and
  lesson prose.
- [x] `CELP-100` Verify learner guidance is progressively disclosed and does not
  crowd professional controls or plots. Advanced parameters remain available
  but are not presented before their theory.
- [x] `CELP-101` Verify keyboard operation, focus, semantics, error, empty,
  disconnected, running, abort and recovery states for every new shared UI
  behavior introduced by the program.
- [x] `CELP-102` Complete one independent beginner review of Lessons 1–18 and
  one independent control-engineer review of each technical module. Record
  concrete findings and corrections; author self-review is not independent
  acceptance.
- [ ] `CELP-103` Verify public navigation exposes the ordered learning path,
  prerequisites, equipment and outcomes and does not advertise unimplemented
  lessons as runnable.
  - Completed evidence for CELP-095 through CELP-103 (2026-08-31): the
    registered whole-course README audit and complete 836-test Extension Host
    suite pass; 84 real gallery/copy/render journeys and their module records
    prove the current shipped presentation. Independent beginner acceptance is
    retained in the foundation evidence, and Harvey's independent engineering
    acceptance is retained for every technical module and capstone. See
    `docs/internal/testing/evidence/vscode-control-curriculum/
    2026-08-31-final-audit/acceptance.md`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**

## 17. Milestone and final gates

- [x] `CELP-104` After each module, run its focused native project tests and the
  relevant Control Studio interaction tests. Record exact commands and results
  on the completed module rows.
- [x] `CELP-105` After each visible module milestone, exercise the shipped
  extension in a real Extension Development Host and retain current wide,
  narrow, light, dark and high-contrast evidence for the shared surfaces that
  changed. Do not recapture unaffected shared surfaces per lesson.
- [x] `CELP-106` At Lessons 18, 30, 41, 52, 63, 75 and 84, run the applicable
  runtime vertical and VS Code extension suites before continuing.
- [x] `CELP-107` Validate all new VS Code tests are registered with
  `python3 scripts/check_vscode_test_registration.py`.
- [x] `CELP-108` Freeze the final implementation, synchronize it to
  `trust-builder`, record hostname/path/revision/disk preflight, then run the
  required final `just fmt`, `just clippy`, and `just test-all` gates once.
- [x] `CELP-109` Run VS Code lint, compile and complete extension tests on
  `trust-builder` for the frozen candidate.
- [x] `CELP-110` Run all applicable runtime vertical tests, numerical-oracle
  qualifications, documentation-link checks and diagram drift checks for
  ownership/data-flow changes.
- [x] `CELP-111` Verify every checked lesson row contains current direct evidence
  and every open limitation is explicit. Do not close the program while a
  lesson is represented only by prose, metadata, a screenshot, a compiling
  placeholder, or an algorithm that does not execute.
- [x] `CELP-112` Confirm the final program implements the mandatory causal
  transitions in Specification 38 §4 and that no practical step uses theory
  first introduced by a later lesson.
- [x] `CELP-113` Perform the final teaching-comment review across every shipped
  lesson ST project. Confirm that necessary comments are accurate, concise,
  consistent with the lesson theory and actual behavior, and sufficient for a
  learner to connect each important implementation decision to the engineering
  reason without turning the source into line-by-line prose.
- [ ] `CELP-114` Audit all 84 lesson READMEs against Specification 38
  §§2.1–2.1.3. Confirm each README follows the reusable practical-lesson
  conformance pattern from the P-control reference lesson: one objective, a
  reason inherited from prior evidence, physical meaning before notation, an
  unambiguous implemented law, a worked physical prediction, the concise real
  ST controller under test, an equivalent controlled comparison, exactly
  defined decision measures, evidence-qualified results, and a bounded
  plant-specific conclusion that creates the next lesson. Reject undefined
  movement/error metrics, single-sample steady-state claims, requested/applied
  output conflation, pseudocode substituted for shipped ST, universal claims
  from one plant class, or figures not generated from retained executable
  evidence. Also confirm each README is pedagogical from its declared
  prerequisites, uses only useful and reproducible diagrams or real-result
  graphs, and closes the causal transition to the following lesson. Record the
  Specification 38 §2.1.3 reusable-rule conformance disposition, executable
  documentation test, and independent learner or engineer review evidence for
  every lesson. A not-applicable disposition requires a plant-specific reason;
  a generic reference to the P lesson is insufficient.
  - Final evidence for CELP-104 through CELP-114 (2026-08-31): every module row
    links its focused native and rendered results; registration passes with 836
    facts and 71/71 files; the complete shipped VS Code suite passes `836
    passing`; all required runtime verticals, artifact replays, numerical
    oracles, documentation links and diagram drift checks pass; and all 84
    README rows pass the registered whole-course conformance audit. The final
    ST review retained concise reason comments and removed narration-only
    bloat. Exact commands and evidence boundaries are consolidated in
    `docs/internal/testing/evidence/vscode-control-curriculum/
    2026-08-31-final-audit/acceptance.md`.
  - Final frozen-candidate evidence for `CELP-108` (2026-08-31): validation ran
    on host `scena-rust-builder` in
    `/home/johannes/projects/trust-platform-curriculum-final-gate` at detached
    revision `3479807f26f56b8347d588d658cd9301d6735050`. The final preflight had
    98 GB free under `/home/johannes` and 3.1 GB under `/tmp`. `just fmt` and
    `just clippy` passed; after the final lookup-only test repair, focused
    clippy for `control_curriculum_temperature_progression` also passed. The
    exact final `just test-all` command exited 0. Earlier cache deletion and
    stale post-reorganization lesson paths were treated as harness failures,
    repaired, and never counted as behavioral red results.
  - Superseding course-wide Markdown review (2026-09-01): all 84 source and
    bundled READMEs implement the exact corrected Lesson 8 action workflow while
    retaining their own equipment, physical problem, required improvement,
    theory, prediction, running ST, decision measures, recovery rules, evidence
    boundary, conclusion, and next-lesson cause. The registered curriculum and
    module suite passed `184` tests, the full Extension Development Host suite
    passed `891` tests, registration reported `891` facts in `72/72` files, and
    84 real gallery/copy/Markdown-preview journeys passed. Current commands,
    logs, JSON records, and representative three-theme Lesson 8 captures are in
    `docs/internal/testing/evidence/vscode-control-curriculum/
    2026-09-01-course-wide-markdown-review/README.md`. **Evidence missing (2026-09-03): the cited directory does not exist in this repository.**

## 18. Fix pass 2026-09-03

This section records a later fix pass, verified directly against this
worktree on 2026-09-03 (HEAD `96038721e`), that repairs specific findings
from `docs/internal/reviews/control-course-review-2026-09-03.md`. It does not
re-certify any lesson row above. Where a row's only rendered-evidence
citation pointed at the missing `vscode-control-curriculum` directory, that
row now carries its own `Evidence missing` note added in this same pass and
its checkbox is unchecked; that is tracked on each such row, not repeated
here.

- [x] `FIXPASS-01` Workflow authority contract, fixing W-01 and W-02.
  `editors/vscode/src/test/suite/control-curriculum-authority.test.ts` reads
  all 84 `control-loop.json` files and asserts each one decodes and validates
  as `accepted` (W-01: every lesson's binding was previously rejected), and
  that each declares `experiment.execution === "project-scripted"`,
  `experiment.kind === "observation"`, `experiment.amplitude === 0`, and a
  `P1.ExperimentEnabled` activation binding (W-02: 31 lessons previously had
  no activation write).
  - Command and result, run in this worktree on 2026-09-03: `cd
    editors/vscode && npx mocha --ui tdd
    out/test/suite/control-curriculum-authority.test.js` → `1 passing`.
- [x] `FIXPASS-02` Preview-safe mathematics lint, fixing course-wide finding 3.
  `editors/vscode/src/test/suite/control-curriculum-readmes.test.ts`, test
  "all lesson READMEs use preview-safe left-aligned mathematics", scans all
  84 READMEs for the `\(...\)` / `\[...\]` delimiters that VS Code's
  markdown-math bundle does not tokenise.
  - Command and result, run in this worktree on 2026-09-03: `cd
    editors/vscode && npx mocha --ui tdd
    out/test/suite/control-curriculum-readmes.test.js --grep "preview-safe"`
    → `1 passing`.
- [x] `FIXPASS-03` Control Studio findings fixed and their unit tests.
  `control-studio-stage-availability.test.ts`, `control-studio-presentation.test.ts`
  and `control-studio-chart-visuals.test.ts` cover, by name in the test
  titles: W-09 (the value-strip tile is named from the loop's own bound
  symbol instead of a hard-coded "Controller output"), W-14 (Automatic Tune
  renders secondary-styled and demoted, not the primary action, for a
  project-scripted loop), W-15 (every runtime stop/restoration reason code
  and failure state maps to one sentence naming the recovery action, not a
  raw enum), W-16 (the value strip badges and dims while showing a saved
  result), V-01 (the axis formatter no longer reads the ECharts tick index as
  fraction digits), V-02/V-07 (one unit-display helper), V-05 (Bode/Nyquist
  draw the 0 dB / −180° references and GM/PM markers from the resolved
  crossovers), V-08 (the response plot keeps headroom and draws SP dashed
  over PV), V-10 (every empty or failed plot state carries one sentence and
  keeps its axes), and V-17 (the uncertainty band is labelled, never
  colour-only). W-12 (Abort test must stay visible while a test runs) has no
  unit test of its own; its fix is the sticky rule at
  `editors/vscode/src/controlStudio/panelStyles.ts:56`
  (`.workspace-actions:has([data-test-abort]:not(:disabled))
  {position:sticky;bottom:0;...}`, with a comment there naming "finding
  W-12"), verified only by the rendered capture in `FIXPASS-04`.
  - Command and result, run in this worktree on 2026-09-03: `cd
    editors/vscode && npx mocha --ui tdd
    out/test/suite/control-studio-stage-availability.test.js
    out/test/suite/control-studio-presentation.test.js
    out/test/suite/control-studio-chart-visuals.test.js` → `41 passing`.
- [x] `FIXPASS-04` Rendered capture, fixing W-13's absent runner and giving
  W-01, W-02, W-09, W-12 and W-15 a rendered instance. `xvfb-run -a -s
  "-screen 0 1920x1080x24" node
  editors/vscode/scripts/control-studio-evidence/run.js` drove Lesson 8
  (`08-water-heater-saturation`) through Monitor to Test to completion plus
  the wide, split, narrow, 200 percent and high-contrast layout matrix, on
  the build machine (`trust-builder`) at source commit `1c128530f`, retained
  under
  `docs/internal/testing/evidence/vscode-control-studio/2026-09-03-fix-pass/`
  (11 screenshots under `screenshots-raw/` and a machine-checkable
  `steps.json`).
  - Red evidence: that directory's `README.md` states the value strip's
    third tile read `MV · Controller output` in the first capture of this
    pass. Green evidence: the retained capture, and `steps.json` itself,
    read `MV · Applied heater power`, taken from the loop contract's own
    bound symbol.
  - Not captured: the other 83 lessons' rendered journeys — they share the
    same authority contract (`FIXPASS-01`) but are not individually rendered.
    The Identify, Tune and Validate plots (including V-03's blank Nichols tab
    and V-05's live margin markers) are not exercised because those stages
    are outside a project-scripted lesson; they are covered by the unit
    tests in `FIXPASS-03` only, not by a render.
- [x] `FIXPASS-05` Generated per-lesson board and its generator.
  `python3 scripts/build_control_curriculum_checklist.py`, run from the
  repository root, regenerates
  `docs/internal/testing/checklists/control-engineering-curriculum-implementation-checklist.md`
  by computing Authority/Presentation/Evidence per lesson directly from
  `control-loop.json`, `lesson.json`, `src/*.st`, `README.md` and `figures/`
  at generation time, instead of asserting a state. The retained board was
  generated at commit `837fb7bb7`; this worktree's HEAD (`96038721e`) is
  three commits ahead of that, including one that folds Lesson 11's retired
  Validation sub-heading into its evidence flow, so some of the open cells
  recorded in `FIXPASS-06` may already be closed. The generator was not
  re-run in this pass to confirm that; `FIXPASS-06` states the board's own
  837fb7bb7 finding, not a current claim.
- [ ] `FIXPASS-06` Open items remaining, taken from the generated board's own
  "Completion boundary" section as generated at commit `837fb7bb7`, not
  invented here: 18 of 84 lessons have at least one open computed column, and
  a row there is closed only when Authority, Presentation and Evidence all
  read `pass` and the lesson's native proof command has run on the build
  machine. Presentation reads `OPEN` ("a retired workflow sub-heading is
  present") for Lessons 11, 45, 46, 47, 48, 49, 50, 51, 52, 71, 72, 79, 80 and
  84. Evidence reads `OPEN` ("figures cannot be regenerated") for Lessons 25,
  45, 46, 47, 48, 49, 50, 51, 52, 64, 71, 72, 78, 79, 80 and 84. Presentation
  also reads `OPEN` ("Test stage does not name Abort test") for Lesson 74.
  Not captured in this pass: the generated board's own frozen final gates
  (`npm run build:examples`; `npx tsc -p ./ && npx mocha --ui tdd
  out/test/suite/control-*.test.js`; `cargo test -p trust-runtime --test
  control_curriculum_workflow_contract`; `just test-all`) were not run here
  beyond the individual focused commands in `FIXPASS-01` through
  `FIXPASS-04`.
