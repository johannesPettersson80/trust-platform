# Verification Program Policy

> **Historical Phase 0-16 record.** This document no longer owns or sequences
> current product work. Any invariant, planner, catalog, mapping, or evidence
> requirement below describes the retired verification campaign. Current work
> follows the direct `written specification -> native executable test` contract
> in `phase18-zero-debt-execution-board.md`.

This document records the former rules of the truST PLC verification program.
It remains stable and readable as historical context. Machine-readable schema
details live in `metadata-model.md`; historical implementation rows live in
`implementation-board.md`.

## Scope

Phases 0 through 15 of this program design and validate a verification control
plane. They do not themselves fix a runtime, compiler, IDE, protocol, or UI
bug. Phase 16 applies that control plane to product work, but only through the
spec-first, red-before-fix, producer-authentic evidence discipline below.

In scope:

- repo-owned verification taxonomy,
- machine-readable invariant and suite metadata,
- specification source and gap inventory,
- existing test/gate catalog,
- generated reports that expose proof gaps,
- rules for test authoring, refactoring, evidence, and release truth.

Out of scope for control-plane Phases 0 through 15:

- moving existing tests into a new tree,
- rewriting `just test-all`,
- adding runtime/compiler/LSP behavior,
- expanding public conformance claims before release/docs gates exist,
- treating line count, coverage percentage, or test count as safety proof.

Phase 16 product changes are in scope only after its execution-readiness rows
are complete. Each product change must start from a reviewed written oracle,
use a cataloged test and durable producer-authentic red evidence where behavior
must change, keep the product fix in a separate descendant commit, and record
paired green plus required gate evidence before promotion. A current-HEAD
reproduction that passes invalidates the alleged defect; it must be recorded as
characterization or behavior-lock work, never turned into an artificial red.

## Stop Gates

- [x] `VERIF-STOP-001` External review gate is cleared before implementation:
  give `fable-review-brief.md` and this document set to Fable or another senior
  reviewer and fold in required edits before any implementation row starts.
- [ ] `VERIF-STOP-002` No existing tests are moved or renamed until the catalog
  proves the destination, owning suite, and runner command. First slices add
  metadata and reports around current tests.
- [ ] `VERIF-STOP-003` No broad local Rust gates on slow local hardware. Use
  targeted local/static checks only; broad validation runs on `trust-builder`
  following repo rules.
- [ ] `VERIF-STOP-004` No safety claim may be closed by source inspection alone.
  Source inspection can open an invariant or test gap. Dangerous behavior claims
  need dynamic proof, a source-only exception with rationale, or a hardware/lab
  blocker.
- [ ] `VERIF-STOP-005` No VM/eval/engine parity result may be the only oracle for
  IEC semantics. Shared wrong behavior is a known failure mode.
- [ ] `VERIF-STOP-006` No hardware protocol claim ships as verified unless the
  hardware-lab suite has passed on real hardware or the public claim is labelled
  preview/unverified.
- [ ] `VERIF-STOP-007` No release checklist may treat generated reports under
  `target/` as durable evidence. Release proof must name CI artifacts, dated
  evidence files, or public release objects.
- [ ] `VERIF-STOP-008` No new verifier, catalog generator, architecture doctor,
  mutation runner, or gate logic is appended to an already-large mixed-purpose
  file. Add per-slice modules or scripts.
- [ ] `VERIF-STOP-009` No public docs page may claim state-of-the-art
  conformance until a public summary, known-gaps page, and release artifact path
  are reviewed.
- [ ] `VERIF-STOP-010` Do not hide ignored tests. Every ignored test must be
  classified as `red_protective`, `lab_required`, `perf_soak`, `manual`,
  `flaky_quarantined`, `obsolete`, or `unknown`, with owner and unblock
  condition.
- [ ] `VERIF-STOP-011` Test/proof comes before product code for every
  safety-relevant change. Bug fixes and new behavior require a failing or
  protective test before implementation. Refactor-only work requires
  behavior-lock tests before editing. Uncertain claims require debug/reproduce
  evidence before the test is written.
- [x] `VERIF-STOP-012` Do not update Codex skills or repo agent instructions to
  mandate this workflow until the verification program is implemented and
  validated. Before implementation, skills may point to this document set as a
  draft only. Closed atomically with `VERIF-P16-007` after the implemented
  workflow, burn-in ratchets, independent authorization, and enforcing CI gate
  were all present; the mandate does not predate enforcement.
- [ ] `VERIF-STOP-013` Do not invent expected behavior inside a test when the
  product/specification contract is missing or ambiguous. Mark the invariant
  `spec_gap`, record the question, update the owning specification or decision
  document first, then write the test against that decision.
- [ ] `VERIF-STOP-014` Do not begin meaningful test-to-claim mapping or new test
  creation for an area until that area's specification sources have been
  inventoried. Mechanical test discovery may scan files, but proof mapping
  starts from known specification/oracle sources.
- [ ] `VERIF-STOP-015` Durable evidence must be one of: a committed repo file, a
  CI artifact with named workflow/run and retention, or a public release object.
  A path that `git check-ignore` matches fails metadata validation and cannot be
  used as durable evidence.

## Vocabulary

Invariant:

- A product truth that must not regress.
- Examples: safe outputs are applied on deliberate stop; REF to a function
  return cannot escape a call frame; rename cannot change the binding target of
  a reference.

Specification source:

- A document, standard, decision record, public claim, hardware observation, or
  external reference that states expected behavior or creates a proof
  obligation.

Spec gap:

- A missing, ambiguous, stale, conflicting, or unavailable specification source
  that prevents a correct test oracle from being written.

Oracle:

- The authority for expected behavior.
- Allowed oracles: IEC/spec text, truST design contract, public protocol spec,
  safety checklist, hardware observation, external PLC comparison, reviewed
  product decision, or documented deviation.
- Disallowed sole oracle: another truST engine produced the same result.

Harness:

- Executable mechanism that checks one or more invariants: Rust test,
  conformance runner, cargo-fuzz target, Playwright script, VS Code extension
  test, hardware lab script, mutation shard, Miri/Loom run.

Gate:

- A suite that must pass for a milestone: `veryquick`, `pr`, `nightly`,
  `release`, `hardware_lab`.

Evidence:

- Durable record that a gate or proof was run against a named commit, platform,
  command, and environment.
- Durable means committed repo file, named CI artifact with retention, or public
  release object. Machine-local ignored paths are not durable evidence.

Protective red test:

- A test that documents known unsafe current behavior but remains ignored until
  its implementation slice starts. It must have a checklist row and unblock
  condition.

Proof levels:

- `S0`: Source-only suspicion. Cannot close a safety claim.
- `S1`: Source trace with line-level mechanism. Opens a test/design row.
- `D1`: Dynamic reproduction or lab observation. Can justify a failing test.
- `T1`: Failing or protective test exists and targets the reproduced behavior.
- `D2`: Design decision recorded, including owner and blast radius.
- `I1`: Implementation complete.
- `G1`: Targeted green proof.
- `G2`: Broad remote gate proof.
- `R1`: Release/public proof, if applicable.

## Code Change Discipline

After this verification program is implemented, every code change must be
routed through invariant/test discipline before production code changes land.

Default order:

1. Define or update the invariant/claim, including `contract_kind` and any
   structured behavior rows needed for the touched area.
2. If the claim is uncertain, debug or reproduce first.
3. Choose the oracle.
4. Let the planner derive required test classes and case families from the
   committed metadata. If the planner reports a spec gap, stop and update the
   owning spec/decision before writing a test.
5. Add the failing, protective, or behavior-lock test.
6. When a case file is required, generate or author committed cases from the
   behavior rows, never from product code.
7. Capture the red/protective result when behavior should change.
8. Implement the smallest production change.
9. Run targeted green proof.
10. Run the required suite tier.
11. Update verification metadata, checklist row, and evidence.

The older shorthand "tests before code" means this full chain: spec or spec gap
first, planner/case requirements second, failing or protective proof third,
implementation fourth.

## Spec-First Test Planning

After the verification tooling exists, bug fixes and features must begin by
updating the structured invariant, not by writing ad hoc tests. The invariant is
the machine-readable behavior contract. It names the prose oracle, risk,
coverage dimensions, `contract_kind`, and for `decision_table` contracts the
input partitions and typed outcomes.

Tool boundaries:

- `plan_tests.py` reads committed metadata and reports required test classes,
  required case families, existing tests, missing tests, spec gaps, waivers, and
  risk changes. It never emits expected behavior text.
- `gen_cases.py` derives case files only from behavior rows or deterministic
  transforms of committed seed artifacts. It never reads product code to decide
  what should happen.
- `prove.py red|green|lock` runs cataloged commands itself and writes evidence
  records. Agents do not hand-author high-risk red/green evidence.
- `prove.py` binds proof by catalog `test_id`, case-file digest, and generated
  case-artifact digest. It never trusts path text alone and it never treats
  compile, harness, metadata, timeout, or infrastructure failures as behavioral
  red proof unless a written oracle explicitly says that failure class is the
  expected behavior.
- `prove.py` must bind a case artifact to the command it just ran by clearing
  stale artifacts before execution and verifying `TRUST_VERIFY_*` run stamps
  emitted by the test helper. A digest-correct artifact from an earlier run is
  not proof.
- The `verification-cases` helper makes tests consume every committed case and
  write a per-case artifact for `prove.py` to check.

Default-deny rules:

- A changed file mapped to no area blocks the planner.
- An uninventoried area blocks the planner.
- Unknown risk is treated as the highest applicable risk until classified.
- A missing behavior row for a required decision-table family is a spec gap, not
  a missing test that the agent may fill by guessing.
- A spec gap can only close by updating the owning specification, decision,
  deviation, design contract, or public claim record.
- A risk downgrade, `not_applicable` coverage cell, or waived matrix row needs
  `decision_ref` to an active reviewed decision/deviation. Free-text rationale
  is not enough.
- Risk-change reports compare against `plan_tests.py --baseline <rev>` locally
  or the pull-request merge base in CI.

Intent-specific workflow:

- Bug fix: update behavior rows or spec gap first, run planner, create failing
  regression/case proof, run `prove.py red`, implement, run `prove.py green`.
- New feature: update spec/invariant first, create acceptance/contract cases
  that fail because behavior is missing, then implement.
- Refactor-only: cite covering invariants and use `prove.py lock` or equivalent
  behavior-lock proof before and after. Do not invent fake red tests.
- Docs/test-refactor: run metadata, link, catalog, and registration checks; no
  runtime red proof is required unless behavior changes.

Typed outcome contracts:

- Accept outcomes prove the exact expected delta: target changed as specified,
  unrelated siblings unchanged, retain/output/process-image state handled as the
  oracle states, and diagnostics/status match the contract.
- Reject outcomes prove zero delta unless the oracle explicitly permits a
  change: state unchanged, no partial apply, stable error code, and visible
  diagnostic/status/fault surface.
- "Rejected" alone is not enough for PLC safety. The test must prove what did
  and did not change.

Bypass resistance:

- A test that asserts nothing cannot produce red evidence for a bug fix because
  it is green at the base commit.
- A skipped case is caught by the per-case artifact cross-check.
- A weakened case table changes a committed file and digest, which is a loud
  metadata diff.
- A green run that does not correspond to the earlier red run is caught by
  `prove.py green`: same test, same case-file digest, formerly-red cases now
  green, no previously-green case regressed, no unwaived skip.
- A green proof must pair to a red/protective-red proof for the same test ID,
  same case-file digest, and same formerly-red case IDs unless a reviewed
  case-table/spec decision explicitly permits the change.
- Any exception for changed case digests, accepted lock deltas, risk downgrades,
  or waivers requires `decision_ref` to an active reviewed decision/deviation.
  A CLI flag or prose note is not enough.
- A hand-authored or agent-authored high-risk proof is rejected by producer
  allowlist; only `prove.py vN` or an approved gate can close red/green proof
  for `safety_critical`, `wrong_result`, `silent_corruption`, or `false_status`.
- A compile error or harness panic cannot count as red evidence unless the
  oracle specifically says compile/load failure is the expected outcome.
- Weak refactor assertions are addressed by mutation shards where available; the
  planner and validator must not pretend static metadata can prove assertion
  strength.

Allowed variants:

- Bug fix: failing regression test first.
- New feature: acceptance/contract test first; it should fail because behavior
  is missing.
- Safety/runtime/protocol/compiler semantic fix: debug/reproduce first, then
  failing/protective test, then implementation.
- Refactor-only: behavior-lock tests may already pass; capture them before
  editing and prove no behavior delta after editing.
- Docs-only or metadata-only: no runtime red test is required, but schema,
  link, or checklist validation must pass.
- Hardware-only claim: if hardware cannot be run, keep the row `unproven`,
  `blocked`, or `deferred`; do not mark the claim fixed.

Disallowed shortcuts:

- No small fix without a mapped invariant or existing test that actually proves
  the behavior.
- No source-only proof for safety-critical runtime behavior.
- No broad refactor whose only proof is `just test-all`.
- No parity-only proof where both paths can share the same bug.
- No UI-visible behavior change without extension/browser/journey proof or an
  explicit non-UI classification.
- No new user-facing workflow, public workflow, or operator/developer journey
  without a written user-story/workflow specification or an explicit spec gap.

## Specification Completeness

The verification program must improve specifications as well as tests. When a
test author cannot state expected behavior from a written oracle, the result is
a spec gap, not a normal missing-test gap.

Every safety-relevant invariant must answer:

- What exact behavior does the spec require?
- Which source defines it: IEC text, protocol standard, truST design contract,
  public docs, reviewed deviation, hardware observation, or release contract?
- What are the allowed boundaries, defaults, lifecycle states, and error cases?
- What should happen on malformed input, timeout, cancellation, restart,
  permission failure, resource exhaustion, or unsupported hardware?
- What visible status or diagnostic is emitted when behavior cannot complete
  safely?

User-story and public-workflow specifications:

- Any feature that adds or changes a user-facing workflow must have a spec
  source before implementation starts. This includes VS Code journeys, HMI
  operator flows, CLI workflows, device/protocol setup flows, debug/force/write
  workflows, install/release workflows, and public docs promises that imply a
  user can accomplish a task.
- The workflow spec must name the actor, entry point, preconditions, ordered
  user-visible steps, expected success state, expected failure/status behavior,
  safety or authz boundaries, and required acceptance evidence.
- If the workflow behavior is not yet decided, create a spec gap with class
  `missing_user_workflow` or `missing_ui_contract` and do not write tests or
  implementation that invent the workflow.
- Backend/runtime proof can support a workflow, but cannot replace journey,
  extension, browser, CLI, or public-doc evidence for the user-visible claim.

Spec gap classes:

- `missing_behavior`
- `ambiguous_behavior`
- `conflicting_sources`
- `missing_boundary`
- `missing_error_model`
- `missing_lifecycle`
- `missing_persistence`
- `missing_security`
- `missing_hardware_contract`
- `missing_ui_contract`
- `missing_user_workflow`
- `external_source_unavailable`
- `public_claim_unproven`

Spec gap workflow:

1. Mark the invariant `spec_gap`.
2. Add a `verification/spec-gaps.toml` entry.
3. Do not write a test that invents a product decision.
4. Resolve the gap by updating the owning document:
   `docs/specs/**`, `docs/IEC_DECISIONS.md`, `docs/IEC_DEVIATIONS.md`,
   `docs/PLCOPEN_DECISIONS.md`, `docs/PLCOPEN_DEVIATIONS.md`,
   `docs/internal/architecture/**`, public docs, or protocol/lab contract.
5. Change the invariant `spec.status` to `specified`, record the oracle ref, and
   only then write the test.
6. If the chosen behavior is a deliberate deviation from IEC/protocol/vendor
   behavior, record the deviation and make the test name explicit.

## Existing Test Refactor Policy

Current executable tests stay in native crate/tooling locations unless the
catalog proves a concrete maintainability or proof-quality reason to move,
split, or rename them.

Allowed reasons:

- A test file mixes unrelated responsibilities and hides which invariant failed.
- A test file is large enough that adding new cases would create a god-file.
- A broad integration test is used as proof for many distinct invariants but
  does not name covered dimensions.
- A test is flaky because setup, environment, time, network, or hardware
  concerns are mixed with pure behavior checks.
- Fixtures are duplicated across suites and have started to drift.
- A VS Code extension test file exists but is not registered in
  `editors/vscode/src/test/suite/index.ts`.
- A malformed-input or boundary suite is too implicit to show which malformed
  classes are covered.
- A slow test belongs in a nightly, soak, hardware-lab, or release tier rather
  than a fast developer tier.

Not valid reasons:

- Making all tests live in one central tree.
- Renaming files only to look uniform.
- Splitting a test without preserving behavior-lock proof.
- Hiding ignored, flaky, or hardware-dependent tests.
- Replacing a focused unit test with a broader integration test.

Required order:

1. Catalog the current test file, command, suite tier, invariant links, ignored
   status, and evidence expectation.
2. Capture a before-refactor behavior-lock run of the smallest command that
   exercises the file.
3. Write the target ownership plan.
4. Move or split only tests named in the plan.
5. Preserve test names and snapshot names where practical.
6. Run the same focused command after the refactor and prove no intended
   behavior changed.
7. Update catalog metadata, stale-path checks, ignored-test register, and VS
   Code suite registration if relevant.
8. Update diagrams and architecture rows if ownership/data flow/gate execution
   flow changed.

Preferred organization:

- unit tests close to the module,
- integration tests in crate `tests/`,
- conformance in `conformance/**`,
- VS Code tests in `editors/vscode/src/test/suite/**`,
- hardware/lab scripts separate from mock/loopback tests,
- shared fixtures in named fixture directories with owner and provenance.

## Suite Tiers

`veryquick`:

- developer feedback within minutes,
- parser/lexer smoke, HIR diagnostic smoke, runtime core unit tests, bytecode
  validator smoke, small deterministic conformance subset,
- no hardware, network timing, long soak, full fuzz, or visual capture.

`pr`:

- normal mainline protection,
- `just fmt`, `just clippy`, `just test` or current PR-equivalent gate,
- deterministic conformance run twice,
- runtime vertical tests when runtime behavior changed,
- VS Code extension tests when extension behavior changed,
- protocol loopback gates for changed protocol surfaces,
- architecture/changed-area checks when ownership changes.

`nightly`:

- deeper bug discovery and flake detection,
- extended conformance, fuzz, selected mutation shards, Miri/sanitizer/Loom
  where supported, long reliability, reconnect/stale-data stress, performance
  trend, selected UI journey smoke.

`release`:

- shipped artifact proof,
- PR gates, full conformance, networking/TLS stability where relevant,
  platform matrix, packaged binary smoke, VSIX smoke, docs render, changelog,
  version, tag, release checks, checksums, Latest release verification.

`hardware_lab`:

- proof that mocks cannot provide,
- named devices, firmware/software versions, topology, env vars, safety
  precautions, skipped/unproven rows visible.

Phase 5 suite records bind direct entrypoints to the reviewed gate inventory.
`includes` and `excludes` do not imply command inheritance, transitive success,
or evidence reuse. `verification/governance.toml` defines `includes` as ordered
display dependencies and `excludes` as reviewed constraint labels; command,
execution, and proof inheritance are all false. A
conditional suite tier is reported to the planner but is never promoted into a
direct required suite without the condition being established by the owning
workflow or review.

The same governance record owns 30-day grace periods for unknown ignored tests,
missing oracles, and undispositioned public claims; safety-relevant mutation
survivors have a before-merge milestone and no elapsed grace. Active metadata
is stale after 90 days without review. Monthly ignored-test, release-time
hardware/security, and quarterly mutation/fuzz reviews are enforced by the
changed-file governance check. Product changes update an invariant and catalog
mapping in the same diff; public-claim changes update a spec source and
invariant. Evidence and invariant retirement is append-only through
`verification/retirements.toml`.

## Test Adequacy Metrics

Metrics are not safety proof. They find weak areas.

First-generation metrics:

- test catalog coverage,
- oracle coverage,
- spec completeness,
- spec-gap debt,
- negative/failure-mode coverage,
- ignored-test debt,
- hardware-proof debt,
- mutation survivors,
- fuzz target coverage,
- release-evidence completeness.

Do not create a single quality score. Scores hide the important reasoning.

## Final Definition of Done

The verification program board is complete only when:

- `verification/` exists with schemas, invariant records, suite definitions,
  catalog metadata, ignored-test register, risk register, evidence index,
  spec-source register, and spec-gap register.
- Existing tests and gates are cataloged without moving them unnecessarily.
- Specification sources are inventoried before tests are proof-mapped.
- Safety-critical invariants have oracles and at least one mapped test or an
  explicit test/spec/hardware gap.
- Ignored tests are classified and owned.
- Suite tiers are validated and documented.
- Bidirectional traceability reports exist.
- Conformance cases are linked to invariants.
- Fuzz, mutation, coverage, hardware, UI, supply-chain/security, platform, and
  release evidence programs have first working reports.
- Remote-builder gates validate metadata and first generated reports.
- Public claims are not expanded until release/public-doc gates are complete.
- `AGENTS.md` and relevant Codex skills are updated after implementation.
