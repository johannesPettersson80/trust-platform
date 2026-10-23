# Control engineering user-journey documentation checklist

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

Scope: reconcile the completed Plant Simulation UI acceptance evidence, create
documentation-ready Plant Simulation journeys, and expose the completed
Simulation and Control Studio workflows in the public documentation. This
checklist changes documentation only; it does not add product behavior or
replace physical-hardware evidence.

Product authority:

- `docs/specs/46-simulation-plant-models.md`
- `docs/specs/47-vscode-plant-simulation-ui.md`
- `docs/specs/35-vscode-control-studio.md`

## Internal journey authority

- [x] `CEJ-DOC-001` Reconcile every Plant Simulation acceptance-board row with
  the completed implementation checklist, current Extension Host evidence, and
  named independent review.
- [x] `CEJ-DOC-002` Write task-focused Plant Simulation user journeys with a
  user story, preconditions, exact actions, expected visible state, safety
  assertions, and direct proof mapping.
- [x] `CEJ-DOC-003` Keep configuration, monitoring, simulation, and physical
  execution ownership explicit. Do not present the configuration drawer as a
  live monitor or simulation evidence as physical evidence.
- [x] `CEJ-DOC-004` Link the Plant Simulation and Control Studio journey sources
  from the shared acceptance board and their final verification records.

## Public documentation

- [x] `CEJ-PUBLIC-001` Extend the Simulation guide with a short visible
  workflow from adding a simulated-plant brick through opening Control Studio.
- [x] `CEJ-PUBLIC-002` Add one Control Studio guide covering Monitor, bounded
  Test, Identify, Tune, Validate, Automatic Tune, Apply, Rollback, and failure
  recovery without exposing internal test-fixture terminology.
- [x] `CEJ-PUBLIC-003` Add the Control Studio guide to documentation navigation
  and cross-link Simulation, Control Studio, runtime control, and safety pages.
- [x] `CEJ-PUBLIC-004` Preserve concise engineering-number examples and units
  without promising fixed decimal counts for values whose useful precision
  depends on scale.
- [x] `CEJ-PUBLIC-005` Update the existing unreleased control-engineering
  changelog entry without creating another version bump.

## Verification

- [x] `CEJ-VERIFY-001` Verify every new relative Markdown link resolves.
- [x] `CEJ-VERIFY-002` Run the repository documentation build or its closest
  focused navigation/snippet validation.
- [x] `CEJ-VERIFY-003` Run `git diff --check` for the documentation closure
  files.
- [x] `CEJ-VERIFY-004` Record that no product behavior changed, so no native,
  runtime, or Extension Host test rerun was required; all behavioral claims
  must point to previously green direct evidence.

## External boundaries

- [x] `CEJ-BOUNDARY-001` State that physical-plant execution still requires a
  reviewed topology, explicit authorization, and current hardware evidence.
- [x] `CEJ-BOUNDARY-002` Do not push, merge, tag, release, or operate physical
  hardware as part of this documentation task.

## Completion evidence

- Plant Simulation acceptance authority now links ten detailed user journeys
  to the completed implementation checklist, phase evidence, screenshots, and
  Harvey's independent `ux_accepted` review.
- `python3 scripts/check_public_docs_ia.py`: passed.
- `python3 scripts/check_public_docs_links.py`: passed.
- `mkdocs build`: passed and rendered
  `site/public/operate/control-studio/index.html`; the generated page contains
  the bounded-test, Automatic Tune, and physical-boundary sections.
- `mkdocs build --strict`: reached the complete site and reported only the 28
  pre-existing specification-mirror and IEC-link warnings. No warning named a
  file or link introduced by this checklist. Strict mode therefore remains a
  repository-wide documentation limitation, not green evidence for this row.
- `git diff --check`: passed for the closure files.
- No production source or behavior changed. Existing green native, Extension
  Host, rendered, and independent-review evidence was inspected directly; no
  test result was fabricated or replaced with prose.
- No push, merge, tag, release, or physical action was performed.
