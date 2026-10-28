# VS Code Plant Simulation UI Implementation Checklist

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


Status: complete

Owner: VS Code Devices & Connections and runtime simulation configuration

Product authority:

- [Specification 34 - VS Code Plant Simulation UI](../../../specs/47-vscode-plant-simulation-ui.md)
- [Specification 33 - Simulation Plant Models](../../../specs/46-simulation-plant-models.md)
- [Specification 35 - VS Code Control Studio](../../../specs/35-vscode-control-studio.md)

This checklist contains only plant-specific work. Test-first development,
builder use, release hygiene, and evidence rules are defined by `AGENTS.md` and
the repository skills; they are not repeated as checklist items.

The plant UI reuses the existing Devices & Connections brick, drawer, canvas,
focus, keyboard, responsive-layout, and VS Code-derived theme system. Those
shared capabilities are not separate plant features. A representative
regression proves that the plant composes correctly with them; the checklist
does not require a plant-specific implementation or every journey repeated in
every theme.

Evidence lives under
`docs/internal/testing/evidence/vscode-plant-simulation-ui/`. A checked row
requires a direct native or Extension Host assertion and rendered interaction
evidence when the behavior is visible.

## Proven foundation

- [x] `PSUI-FOUNDATION-001` The plant contract supports capability discovery,
  saved configuration reads, validation, atomic save, status, trace, revision
  conflicts, authorization, and unavailable-state truthfulness.
  Evidence: [runtime contract](../evidence/vscode-plant-simulation-ui/phase-2-runtime-contract.md).

- [x] `PSUI-FOUNDATION-002` A saved plant is projected as an ordinary child of
  its owning runtime through the existing brick system. It has plant-specific
  identity and state data but no parallel canvas node system.
  Evidence: [plant topology](../evidence/vscode-plant-simulation-ui/phase-3-plant-topology.md).

- [x] `PSUI-FOUNDATION-003` Plant state is derived without invented live data
  and covers Draft, Configured, Disabled, Waiting, Running, Restart required,
  Faulted, and Status unavailable.
  Evidence: [plant topology](../evidence/vscode-plant-simulation-ui/phase-3-plant-topology.md).

- [x] `PSUI-FOUNDATION-004` The existing Add picker supports click, Enter,
  Space, and drag/drop onto an eligible runtime, including malformed or stale
  drag rejection and no-write cancellation.
  Evidence: [add flow](../evidence/vscode-plant-simulation-ui/phase-4-add-flow.md).

- [x] `PSUI-FOUNDATION-005` The plant drawer provides identity, starting-point
  selection, compiled PLC signal selection, manual addressing, engineering
  scaling, field-owned validation, and the advanced state-space route.
  Evidence: [guided editor](../evidence/vscode-plant-simulation-ui/phase-5-guided-editor.md) and [engineering settings](../evidence/vscode-plant-simulation-ui/phase-5-engineering-settings.md).

- [x] `PSUI-FOUNDATION-006` First order with dead time, piecewise-linear, and
  state-space editors preserve exact values and implement their specialized
  editing behavior.
  Evidence: [dynamics editors](../evidence/vscode-plant-simulation-ui/phase-6-dynamics-editors.md).

- [x] `PSUI-FOUNDATION-007` Actuator bounds/rates/deadband, ordered
  imperfections, and ordered scenario-event editing are available without
  adding simulation logic to the PLC program.
  Evidence: [advanced configuration](../evidence/vscode-plant-simulation-ui/phase-7-advanced-configuration.md).

- [x] `PSUI-FOUNDATION-008` Validation and save are atomic, preserve unrelated
  configuration, retain entered values on failure, and recover explicitly from
  stale revisions.
  Evidence: [save behavior](../evidence/vscode-plant-simulation-ui/phase-8-save.md).

- [x] `PSUI-FOUNDATION-009` A saved plant uses the existing brick Remove
  interaction. Cancel writes nothing; confirmation removes only the plant and
  its events; unrelated TOML content is preserved; focus returns to the owning
  runtime.
  Evidence: [rendered removal result](../evidence/vscode-plant-simulation-ui/full-interaction-validation-2026-08-25/remove/json/VIS-plant-remove-dark-w1440.json).

- [x] `PSUI-FOUNDATION-010` Devices & Connections sends a typed,
  identity-only Simulation handoff to Control Studio without moving live
  values or editable plant data into the handoff.
  Evidence: [Control Studio handoff](../evidence/vscode-plant-simulation-ui/phase-9-control-studio-handoff.md).

- [x] `PSUI-FOUNDATION-011` The plant brick and drawer use the existing shared
  VS Code-derived design system. Current real-extension evidence covers dark,
  light, high-contrast narrow, and approximately 200% zoom without a
  plant-specific palette or theme implementation.
  Evidence: [responsive and theme verification](../evidence/vscode-plant-simulation-ui/phase-10-responsive-theme.md).

## Remaining plant-specific implementation

### Dynamics editors

- [x] `PSUI-P6-001` Complete Static gain editing and exact save/reopen for
  gain, input bias, and output bias.
  Evidence: registered scalar round-trip test and [dynamics editor evidence](../evidence/vscode-plant-simulation-ui/phase-6-dynamics-editors.md).

- [x] `PSUI-P6-002` Complete First order editing and exact save/reopen for
  gain, time constant, biases, and initial state.
  Evidence: registered scalar round-trip test and [dynamics editor evidence](../evidence/vscode-plant-simulation-ui/phase-6-dynamics-editors.md).

- [x] `PSUI-P6-004` Complete Integrator editing and exact save/reopen for gain,
  biases, initial state, and optional state limits.
  Evidence: `plant-editor-model.test.ts` verifies omitted and finite bounds,
  bound ordering, and exact save/reopen.

- [x] `PSUI-P6-005` Complete Second order editing and exact save/reopen for
  gain, natural frequency, damping ratio, biases, and two initial states.
  Evidence: registered scalar round-trip test and [engineering-settings evidence](../evidence/vscode-plant-simulation-ui/phase-5-engineering-settings.md).

- [x] `PSUI-P6-008` Preserve unrelated values when changing models. Require
  confirmation only when edited model-owned values would be replaced; Cancel
  preserves the draft and restores focus.
  Evidence: registered starting-point planner test and [rendered confirmation journey](../evidence/vscode-plant-simulation-ui/phase-5-engineering-settings.md#shipped-vs-code-interaction).

- [x] `PSUI-P6-009` Identify the exact row, column, point, or dimension for
  matrix and ordered-list errors.
  Evidence: registered piecewise and state-space editor tests and [dynamics editor evidence](../evidence/vscode-plant-simulation-ui/phase-6-dynamics-editors.md).

- [x] `PSUI-P6-010` Continuous state-space input delays use shared fields,
  preserve omitted/explicit zero and saved values, align with input resizing,
  confirm nonzero removal, and enforce the native history bound when the step
  is known. Registered helper/preservation and CPFIX-004 rendered evidence,
  theme captures and independent review are required before UI acceptance.
  Implementation and focused red/green are complete: six delay helper cases,
  real Save/reopen and aligned error fields pass in dark, light and high
  contrast (`rendered-8slvzl`, `rendered-Xhi3ls`, `rendered-JEMZNH`). The
  implementer and root independently reviewed all six saved/invalid captures;
  final shared TypeScript compilation and the integrated50-test focused batch
  pass. [Independent review](/home/johannes/projects/.artifacts/control-platform-release-20260911/plant-input-delay-independent-review.md).
  Whole-platform UI and broad release gates remain separate.

### Actuator, imperfections, and events

- [x] `PSUI-P7-003` Complete the actuator fields defined by Specification 34,
  including lag, initial applied value, and truthful omitted-versus-zero
  behavior.
  Evidence: registered actuator test and [advanced-configuration evidence](../evidence/vscode-plant-simulation-ui/phase-7-advanced-configuration.md#scalar-actuator-red-green).

- [x] `PSUI-P7-009` Persist displayed scenario-event order exactly, including
  equal-time events; Cancel restores the prior order.
  Evidence: registered scenario-event test and [rendered scenario journey](../evidence/vscode-plant-simulation-ui/phase-7-advanced-configuration.md#scenario-event-editor-evidence).

- [x] `PSUI-P7-010` Give noise, dropout, actuator fault, and disturbance rows
  concise plant-specific explanations without adding live charts to the
  configuration drawer.
  Evidence: registered imperfection preset/help tests and [advanced-configuration evidence](../evidence/vscode-plant-simulation-ui/phase-7-advanced-configuration.md#ordered-imperfection-editor-partial-evidence).

### Save, runtime, and safety

- [x] `PSUI-P8-005` Compare saved and active configuration identities and show
  Restart required without presenting saved values as live.
  Evidence: registered topology state and live-evidence admission tests plus [plant-topology evidence](../evidence/vscode-plant-simulation-ui/phase-3-plant-topology.md).

- [x] `PSUI-P8-007` Show waiting, disconnection, and recovery states without
  stale live values.
  Evidence: registered topology/service tests and [plant-topology evidence](../evidence/vscode-plant-simulation-ui/phase-3-plant-topology.md).

- [x] `PSUI-P8-009` Reject enabled physical-I/O ownership conflicts with the
  exact address, owner, consequence, and route to the existing I/O
  configuration. Do not offer an override.
  Evidence: registered Extension Host conflict test and [save/safety evidence](../evidence/vscode-plant-simulation-ui/phase-8-save.md#physical-io-ownership-conflict).

- [x] `PSUI-P8-011` Distinguish offline Save from a live runtime change; Save
  success must never imply that a stopped or older runtime is using the new
  configuration.
  Evidence: registered Extension Host save-result test and [save/safety evidence](../evidence/vscode-plant-simulation-ui/phase-8-save.md#saved-versus-running-result).

### Control Studio boundary

- [x] `PSUI-P9-002` Use one normalized plant-status contract in the canvas and
  Control Studio so the two surfaces cannot report contradictory plant state.
  Evidence: registered topology and Control Studio plant-monitor projection tests.

- [x] `PSUI-P9-004` Handle stopped, unavailable, stale, malformed, deleted,
  wrong-project, and revision-conflict handoffs with an exact reason and a
  safe recovery action.
  Evidence: registered handoff, monitor-envelope, stopped/deleted/stale monitor,
  and saved-revision recovery tests; recovery returns to Plant configuration.

- [x] `PSUI-P9-006` Navigate from Control Studio back to the owning plant
  drawer without copying the form into Control Studio.
  Evidence: [live return journey](../evidence/vscode-plant-simulation-ui/phase-9-control-studio-handoff.md#live-runtime-and-return-journey).

- [x] `PSUI-P9-008` Prove the handoff against one real running simulation:
  saved-versus-running state, Simulation source identity, and the correct
  runtime and plant selection. Control Studio owns all monitoring after open.
  Evidence: [live runtime journey](../evidence/vscode-plant-simulation-ui/phase-9-control-studio-handoff.md#live-runtime-and-return-journey).

## Plant-specific usability verification

These rows cover behavior introduced by the plant editor. Generic canvas
selection, focus rings, theme switching, graph reframing, and brick layout are
owned by the shared canvas regression suite and are not repeated here.

- [x] `PSUI-USABILITY-001` Expanding or collapsing advanced plant sections
  changes no values, ordering, dirty state, or saved precision.
  Evidence: [engineering-settings journey](../evidence/vscode-plant-simulation-ui/phase-5-engineering-settings.md#shipped-vs-code-interaction) and registered exact model round-trip tests.

- [x] `PSUI-USABILITY-002` Every plant-specific `?` control has a
  subject-specific accessible name, opens only its own concise explanation,
  closes with Escape, and restores focus.
  Evidence: [focus-help red-green journey](../evidence/vscode-plant-simulation-ui/phase-5-engineering-settings.md#focus-help-red-green) and [actuator help](../evidence/vscode-plant-simulation-ui/phase-7-advanced-configuration.md#scalar-actuator-red-green).

- [x] `PSUI-USABILITY-003` Plant-specific list reorder, matrix editing, Save,
  Cancel, Remove, validation recovery, and Control Studio handoff are
  keyboard reachable with predictable focus.
  Evidence: registered plant editor tests and rendered Phase 5, 7, 8, and 9 journeys.

- [x] `PSUI-USABILITY-004` One learner completes the Tank path without TOML or
  unexplained control-theory notation, and one control professional completes
  the state-space/effects path without leaving the UI. Record and correct any
  observed product problem before acceptance.
  Evidence: rendered starting-point/save/live-monitor journey and the rendered state-space and ordered-effects journeys recorded in Phase 6, 7, and 9 evidence.

- [x] `PSUI-USABILITY-005` An independent reviewer accepts one representative
  dark journey, one light narrow journey, and one high-contrast keyboard/error
  journey. This is the complete plant-specific visual matrix; journeys are not
  multiplied across every theme.
  Evidence: [independent refreshed-matrix acceptance](../evidence/vscode-plant-simulation-ui/phase-10-responsive-theme.md#independent-acceptance).

## Documentation

- [x] `PSUI-DOC-001` Provide a concise Tank example using ordinary PLC `%Q`
  command and `%I` feedback signals with no simulation-specific PLC logic.
  Evidence: `docs/guides/PLC_SIMULATION_WORKFLOW.md`; public docs link and IA checks pass.

- [x] `PSUI-DOC-002` Document the seven models, exact fields and units,
  actuator behavior, imperfections, events, restart behavior, and safety
  limits in one professional reference.
  Evidence: `docs/guides/PLC_SIMULATION_WORKFLOW.md`; public docs link and IA checks pass.

- [x] `PSUI-DOC-003` Add a short learning path explaining what each starting
  point represents and its limitation without promising identification,
  tuning, stability, or physical fidelity.
  Evidence: `docs/guides/PLC_SIMULATION_WORKFLOW.md`; public docs link and IA checks pass.

- [x] `PSUI-DOC-004` Update navigation, configuration reference, architecture
  documentation, and current shipped-extension screenshots.
  Evidence: public simulation navigation includes the workflow, `docs/public/reference/config/simulation-toml.md` links the UI workflow, Control Studio/simulation diagrams are present, and Phase 9 contains current shipped-extension screenshots.

## Final validation

Run these only after the remaining behavior rows are complete and the
candidate is frozen.

- [x] `PSUI-FINAL-001` Verify the canonical instructions, sync the working
  tree to `trust-builder`, record hostname/path/free space, and run all focused
  plant runtime and registered extension tests.
  Evidence: [Phase 11 checkout, runtime, and registration gates](../evidence/vscode-plant-simulation-ui/phase-11-final-validation.md#runtime-and-registration-gates).

- [x] `PSUI-FINAL-002` Run the required runtime vertical suites and the full
  VS Code lint, compile, registration, and Extension Host suite.
  Evidence: [Phase 11 VS Code extension gate](../evidence/vscode-plant-simulation-ui/phase-11-final-validation.md#vs-code-extension-gate).

- [x] `PSUI-FINAL-003` Run the three representative real-extension journeys
  from `PSUI-USABILITY-005` and inspect the resulting screenshots and state.
  Evidence: [Phase 11 rendered journeys and independent review](../evidence/vscode-plant-simulation-ui/phase-11-final-validation.md#rendered-journeys-and-review).

- [x] `PSUI-FINAL-004` Render changed diagrams, refresh the manifest, and pass
  diagram drift validation.
  Evidence: [Phase 11 diagram gates](../evidence/vscode-plant-simulation-ui/phase-11-final-validation.md#diagrams-and-repository-gates).

- [x] `PSUI-FINAL-005` Run `just fmt`, `just clippy`, and `just test-all` on
  `trust-builder`. A disk, dependency, harness, or timeout failure remains a
  blocker and is not reported as a product failure or a green gate.
  Evidence: [Phase 11 repository gates](../evidence/vscode-plant-simulation-ui/phase-11-final-validation.md#diagrams-and-repository-gates).

## Done

The Plant Simulation UI is complete when every remaining plant-specific row
above is checked with direct evidence, the PLC program interacts only through
normal process-image I/O, Plant Properties contains configuration rather than
monitoring, no mixed physical/simulated output override exists, the three
representative rendered journeys are accepted, and the final gates are green.
