# Verification Program Implementation Board

Status: draft for external model review. Do not implement new runners, move
tests, or change gate semantics until `VERIF-REVIEW-004` is cleared.

This board sequences implementation. Policy lives in `policy.md`; schema and
record details live in `metadata-model.md`.

## Phase 0 - Review and Baseline Freeze

- [ ] `VERIF-P0-001` Create evidence root:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-08/`.
- [ ] `VERIF-P0-002` Save this split document set as the initial review input.
- [ ] `VERIF-P0-003` Send `fable-review-brief.md` to the reviewer.
- [ ] `VERIF-P0-004` Fold required review edits into this document set before
  any implementation row starts.
- [ ] `VERIF-P0-005` Record review verdict and unresolved questions under the
  evidence root.
- [ ] `VERIF-P0-006` Capture current counts of Rust source files, crate test
  files, fixtures, ignored tests, conformance cases, fuzz targets, CI workflows,
  specs/decision docs, and gate scripts.
- [ ] `VERIF-P0-007` Record current dirty-worktree caveat. This board must not
  overwrite unrelated implementation changes.
- [ ] `VERIF-P0-008` After folding review edits, record a fold summary and rerun
  doc-consistency checks: line counts, duplicate checkbox IDs, ignored tracked
  paths, stale links, and machine-status vocabulary scan.

Acceptance:

- External review verdict exists.
- Required edits are folded in.
- No test runner or source behavior changed.

## Phase 1 - Verification Control Plane Skeleton

- [ ] `VERIF-P1-001` Add `verification/README.md` explaining storage rules and
  relationship to crate tests, conformance, fuzz, CI artifacts, and internal
  evidence.
- [ ] `VERIF-P1-002` Add JSON schemas:
  `invariant.schema.json`, `suite.schema.json`, `catalog.schema.json`,
  `ignored-test.schema.json`, `risk-register.schema.json`,
  `evidence.schema.json`,
  `spec-source.schema.json`, and `spec-gap.schema.json`.
- [ ] `VERIF-P1-003` Add empty or seed TOML files under
  `verification/invariants/**`.
- [ ] `VERIF-P1-004` Add seed suite definitions under `verification/suites/**`.
- [ ] `VERIF-P1-005` Add `verification/spec-sources.toml`.
- [ ] `VERIF-P1-006` Add `verification/spec-gaps.toml`.
- [ ] `VERIF-P1-007` Add `verification/test-catalog.toml`.
- [ ] `VERIF-P1-008` Add `verification/ignored-tests.toml`.
- [ ] `VERIF-P1-009` Add `verification/risk-register.toml`.
- [ ] `VERIF-P1-010` Add `verification/evidence-index.toml`.
- [ ] `VERIF-P1-011` Add validation script:
  `scripts/validate_verification_metadata.py`.
- [ ] `VERIF-P1-012` Add a cheap local/CI check that validates metadata schemas
  only. This must not run Rust tests.
- [ ] `VERIF-P1-013` Document generated-report vs committed-metadata rules,
  including durable evidence: committed repo file, named CI artifact with
  retention, or public release object.
- [ ] `VERIF-P1-014` Encode coverage matrix metadata in the invariant schema.
- [ ] `VERIF-P1-015` Encode test class, oracle/spec refs, suite tier, evidence,
  and malformed-input taxonomy fields in the catalog schema.
- [ ] `VERIF-P1-016` Add `schema_version = 1` to every metadata schema and record
  fixture.
- [ ] `VERIF-P1-017` Add cross-field validation rules for status progression:
  `test_written`, `implemented`, and `validated` cannot be hand-edited without
  the required tests, proof levels, evidence refs, specified spec status, and
  closed safety coverage cells.

Acceptance:

- Metadata validates.
- Empty skeleton does not claim coverage.
- No existing tests are moved.
- Every planned metadata file has a schema.
- Evidence has a committed record type; evidence refs point to evidence IDs, not
  raw paths.
- Spec sources and spec gaps can be tracked before tests are proof-mapped.

## Phase 1A - Specification Source Inventory

This phase inventories written contracts before existing tests are treated as
proof.

- [ ] `VERIF-P1A-001` Inventory existing spec sources under
  `verification/spec-sources.toml`, starting with bytecode/VM and runtime
  safety. The first bytecode source row should point at the real committed
  `docs/specs/12-bytecode.md`; the validator semantic contract remains a
  separate spec gap until written.
- [ ] `VERIF-P1A-002` Add a source scanner/report for likely spec documents:
  `docs/specs/**`, `docs/internal/**`, top-level README, public docs,
  conformance docs, release docs, and protocol/lab notes.
- [ ] `VERIF-P1A-003` Classify each source by area, authority level, owner,
  freshness, public/internal visibility, and oracle usability.
- [ ] `VERIF-P1A-004` Emit obvious-missing-spec report by area:
  bytecode format, bytecode validator, VM value semantics, scan-cycle lifecycle,
  stop/safe-state, retain/restart, protocol status/discovery, HMI API/UI,
  source transformations, LSP sync/positions/cancellation, debug/DAP
  force-write-release lifecycle, security/supply chain, platform/package
  behavior, release proof.
- [ ] `VERIF-P1A-005` Emit public-claim report: public/user-facing docs claims
  with no normative source, no invariant, or no proof path.
- [ ] `VERIF-P1A-006` Emit conflict/staleness report: docs that disagree, specs
  that reference removed behavior, stale checklist rows, duplicate decisions.
- [ ] `VERIF-P1A-007` For external standards that cannot be committed, record
  local path, retrieval expectation, version/date, and whether absence blocks
  proof.
- [ ] `VERIF-P1A-008` For every missing or ambiguous bytecode/VM pilot source,
  create a `spec_gap` row before cataloging tests as proof for that behavior.
- [ ] `VERIF-P1A-009` Do not mark Phase 2 catalog entries as proof-mapped unless
  their invariant can point to a spec source or spec gap.

Acceptance:

- The repo can answer "what specs do we have?" before "what tests do we have?"
- Missing/stale/conflicting specifications are visible.
- Bytecode/VM pilot can separate test gaps from spec gaps.
- Public claims without specs are reported instead of accepted.

## Phase 2 - Existing Test Catalog

- [ ] `VERIF-P2-001` Add a catalog generator that scans:
  `crates/*/tests`, practical in-source `#[cfg(test)]` modules,
  `editors/vscode/src/test`, `conformance`, `fuzz`, `scripts/*gate*`, and
  `.github/workflows`.
- [ ] `VERIF-P2-002` Extract test names, package, command hint, file path,
  ignore attribute, and obvious checklist/evidence references.
- [ ] `VERIF-P2-003` Emit generated catalog JSON under `target/gate-artifacts`
  and concise Markdown summary under dated evidence root.
- [ ] `VERIF-P2-004` Create committed `verification/test-catalog.toml` only for
  hand-owned metadata that cannot be safely inferred.
- [ ] `VERIF-P2-005` Add stale-path checker for committed catalog entries.
- [ ] `VERIF-P2-005A` Stale catalog checks must verify file path and test name
  against scanner output; a renamed/deleted test function inside a surviving
  file must fail validation.
- [ ] `VERIF-P2-006` Add VS Code extension-test registration checker.
- [ ] `VERIF-P2-007` Add test-class completeness report.
- [ ] `VERIF-P2-008` Add coverage-matrix gap report with states:
  `covered`, `covered_by_fuzz`, `not_applicable`, `blocked`, `spec_gap`,
  `gap_open`, `deferred`.
- [ ] `VERIF-P2-009` Add malformed-input coverage report.
- [ ] `VERIF-P2-010` Do not fail CI on unmapped tests in the first slice. Report
  unmapped tests as debt.

Acceptance:

- Existing tests are discoverable.
- Ignored tests are visible.
- Generated report separates inferred facts from hand-authored intent.
- Catalog can answer what mapped tests prove and which malformed classes are
  missing.

## Phase 2A - Existing Test Refactor Plan

- [ ] `VERIF-P2A-001` Add report for large or mixed-purpose test files.
- [ ] `VERIF-P2A-002` Add report for broad tests claiming too many invariants
  without coverage dimensions.
- [ ] `VERIF-P2A-003` Add report for duplicated fixtures or near-duplicate
  malformed/boundary inputs.
- [ ] `VERIF-P2A-004` Add VS Code registration refactor report.
- [ ] `VERIF-P2A-005` Add slow-test classification report.
- [ ] `VERIF-P2A-006` Require written plan for every proposed move/split/rename:
  before command, after command, invariant IDs, fixture ownership, stale-path
  updates, expected behavior delta.
- [ ] `VERIF-P2A-007` Add catalog redirect/stale-path rule for moved/renamed
  tests.
- [ ] `VERIF-P2A-008` Add before/after focused behavior-lock rule.
- [ ] `VERIF-P2A-009` Add SOLID/KISS/DRY rule for test files.
- [ ] `VERIF-P2A-010` Add first pilot refactor proposal only after bytecode/VM
  catalog exists; mark "no refactor needed" if reports show no real need.

Acceptance:

- Recommendations are based on catalog evidence, not aesthetics.
- No existing test is moved without before/after behavior-lock proof.

## Phase 3 - Ignored Test Register

- [ ] `VERIF-P3-001` Generate ignored-test inventory from Rust, Node,
  Playwright, shell, and conformance surfaces where practical.
- [ ] `VERIF-P3-002` Classify every ignored test using machine classes from
  `metadata-model.md`.
- [ ] `VERIF-P3-003` For `red_protective`, require linked row and expected red
  symptom.
- [ ] `VERIF-P3-004` For `lab_required`, require env vars, hardware topology,
  and public-claim impact.
- [ ] `VERIF-P3-005` For `flaky_quarantined`, require owner and last observed
  failure.
- [ ] `VERIF-P3-006` Fail metadata validation if any ignored test remains
  `unknown` after grace period.

## Phase 4 - Invariant Registry

- [ ] `VERIF-P4-000` Import confirmed findings from the 2026-07-04/05 runtime,
  HIR-to-VM, IDE/LSP, and comms reviews as risk-register entries and invariant
  seeds with `gap_open` or `spec_gap` status. Include timer semantics, NaN/Inf
  ingress, runtime authz, OPC UA session lifecycle, and online-change/hot-reload
  consistency.
- [ ] `VERIF-P4-001` Seed compiler/frontend invariants.
- [ ] `VERIF-P4-002` Seed HIR-to-bytecode-to-VM seam invariants.
- [ ] `VERIF-P4-003` Seed runtime-safety invariants.
- [ ] `VERIF-P4-004` Seed protocol/connectivity invariants.
- [ ] `VERIF-P4-005` Seed editor/source-transformation invariants.
- [ ] `VERIF-P4-006` Seed HMI/UI acceptance invariants.
- [ ] `VERIF-P4-007` Seed release/public-claim invariants.
- [ ] `VERIF-P4-008` Seed security/supply-chain/platform invariants.
- [ ] `VERIF-P4-009` Link each safety-critical invariant to at least one oracle
  reference or spec gap.
- [ ] `VERIF-P4-010` Mark unproven claims as `gap_open`, `blocked`,
  `deferred`, `spec_gap`, or `unproven`; do not mark them `validated`.

## Phase 4A - Specification Completeness Audit

- [ ] `VERIF-P4A-001` Add spec-gap register entries under
  `verification/spec-gaps.toml`.
- [ ] `VERIF-P4A-002` Report invariants with `spec.status != "specified"`.
- [ ] `VERIF-P4A-003` Report tests with `expected_result` but no `oracle_ref`,
  `spec_ref`, or `spec_gap_ref`.
- [ ] `VERIF-P4A-004` Report coverage dimensions marked `spec_gap`.
- [ ] `VERIF-P4A-005` Report public docs claims with no invariant and no oracle.
- [ ] `VERIF-P4A-006` Add close-out rule: spec gap closes only when owning
  spec/decision/deviation/design doc is updated and mapped tests are written or
  explicitly deferred.
- [ ] `VERIF-P4A-007` Safety-critical `spec_gap` blocks `validated`.
- [ ] `VERIF-P4A-008` For bytecode/VM pilot, classify every initial gap as test
  gap, spec gap, hardware/tool blocked, or not applicable.

## Phase 5 - Suite Definitions and Gate Mapping

- [ ] `VERIF-P5-000` Inventory existing workflows and `scripts/*gate*` into
  suite records or explicit exclusions. Include nightly reliability, HMI long
  soak, protocol device-in-loop, salsa hardening, release/version guard,
  malformed-bytecode fuzz, VM determinism/reliability, unsafe/concurrency, and
  runtime device-in-loop gates.
- [ ] `VERIF-P5-000A` Define `veryquick` environment and just-recipe mapping
  before using it in suite records; do not invent a new broad local Pi gate.
- [ ] `VERIF-P5-000B` Phase 11 hardware lab work must build on any existing
  device-in-loop workflow/test harness instead of creating a parallel source of
  truth.
- [ ] `VERIF-P5-001` Define `veryquick`.
- [ ] `VERIF-P5-002` Define `pr`.
- [ ] `VERIF-P5-003` Define `nightly`.
- [ ] `VERIF-P5-004` Define `release`.
- [ ] `VERIF-P5-005` Define `hardware_lab`.
- [ ] `VERIF-P5-006` Validate command owner, duration, environment, artifacts.
- [ ] `VERIF-P5-007` Ensure hardware commands are env-gated.
- [ ] `VERIF-P5-008` Ensure release commands name durable evidence or CI
  artifacts.
- [ ] `VERIF-P5-009` Encode code-area matrix in machine-readable metadata.
- [ ] `VERIF-P5-010` Add changed-file classifier.

## Phase 6 - Requirement, Oracle, and Traceability Mapping

- [ ] `VERIF-P6-001` Map IEC spec/deviation references to compiler/runtime
  conformance invariants.
- [ ] `VERIF-P6-002` Map runtime design contracts to runtime-safety invariants.
- [ ] `VERIF-P6-003` Map protocol specifications/product decisions to protocol
  invariants.
- [ ] `VERIF-P6-004` Map VS Code/LSP protocol references and product contracts
  to editor invariants.
- [ ] `VERIF-P6-005` Map security/supply-chain/platform contracts to invariants.
- [ ] `VERIF-P6-006` Add report for invariants missing oracles.
- [ ] `VERIF-P6-007` Fail only on missing oracles for `safety_critical`,
  `silent_corruption`, and `false_status` risks after grace period.
- [ ] `VERIF-P6-008` Add forward traceability report:
  spec source -> invariant -> test -> suite/gate -> evidence -> public claim.
- [ ] `VERIF-P6-009` Add reverse traceability report:
  public claim -> evidence -> suite/gate -> test -> invariant -> spec source.
- [ ] `VERIF-P6-010` Add report for orphan specs, orphan tests, orphan
  invariants, orphan public claims, and orphan evidence.

Acceptance:

- Critical invariants without oracles are visible before test writing starts.
- Public claims can be traced back to proof or an explicit gap.

## Phase 6A - Verification Tooling Self-Tests

- [ ] `VERIF-P6A-001` Add known-good metadata fixture.
- [ ] `VERIF-P6A-002` Add known-bad fixtures for missing field, unknown status,
  stale path, unknown invariant, unknown suite, schema mismatch, and public
  claim without proof/gap.
- [ ] `VERIF-P6A-002A` Add known-bad fixtures for ignored durable evidence path,
  unknown evidence ID, mapped record with empty invariants, stale test name in an
  existing file, `validated` with empty evidence, safety-critical `validated`
  with `gap_open`/`spec_gap`, and proof level below status requirement.
- [ ] `VERIF-P6A-003` Add validator tests for all known-bad fixtures.
- [ ] `VERIF-P6A-004` Add catalog scanner self-tests.
- [ ] `VERIF-P6A-005` Add spec-source scanner self-tests.
- [ ] `VERIF-P6A-006` Add changed-file classifier self-tests.
- [ ] `VERIF-P6A-007` Add report-renderer golden/protective tests.
- [ ] `VERIF-P6A-008` Add stale metadata tests for deleted/renamed tests and
  removed scripts.

Acceptance:

- The verification system can fail when its own metadata lies.

## Phase 7 - Conformance Program Alignment

- [ ] `VERIF-P7-001` Keep `conformance/contract.md` as public conformance
  contract.
- [ ] `VERIF-P7-002` Link conformance cases to invariants.
- [ ] `VERIF-P7-003` Report conformance categories, case counts, expected
  artifacts, invariant coverage.
- [ ] `VERIF-P7-004` Add gap rows for strings, arrays, structs, enums, nested
  values, OOP dispatch, references, retain matrix, scheduler, comms determinism.
- [ ] `VERIF-P7-005` Comms determinism cases use simulated/loopback scripted
  status transitions, not live sockets.
- [ ] `VERIF-P7-006` Generated conformance reports stay CI artifacts unless a
  public summary page is intentionally updated.

## Phase 8 - Runtime Anomaly Testing Program

- [ ] `VERIF-P8-001` Define fault-injection taxonomy: panic, timeout, deadline,
  watchdog, slow device, disconnect, queue full, stale data, corrupt retain,
  malformed bytecode, bad config, bad signal, partial web request, disk error,
  clock step, monotonic/wall-clock divergence, suspend/resume, timer duration
  overflow, and allocation failure/OOM.
- [ ] `VERIF-P8-001A` Open spec-gap candidates for scan-cycle allocation policy
  and time base across restart kinds if no written contract exists.
- [ ] `VERIF-P8-002` Map existing runtime-safety tests to taxonomy.
- [ ] `VERIF-P8-003` Add gap report for untested runtime anomaly classes.
- [ ] `VERIF-P8-004` Classify anomaly classes as PR, nightly, release, or
  hardware_lab.
- [ ] `VERIF-P8-005` Add mutation/fault toggles only behind explicit test-only
  interfaces or harness layers.
- [ ] `VERIF-P8-006` Do not add production fault hooks without design review.

## Phase 9 - Fuzz and Malformed Input Program

- [ ] `VERIF-P9-001` Inventory fuzz targets and fuzz-like smoke tests.
- [ ] `VERIF-P9-002` Define required fuzz surfaces: ST lexer/parser,
  HIR/lowering input, PLCopen XML, bytecode container/instructions, protocol
  payloads, config files, LSP incremental edits, HMI schema payloads.
- [ ] `VERIF-P9-003` Classify each fuzz target as PR-smoke, nightly, or manual
  extended.
- [ ] `VERIF-P9-004` Define corpus storage rules.
- [ ] `VERIF-P9-005` Every minimized crash becomes deterministic regression.
- [ ] `VERIF-P9-006` Add generated fuzz coverage/gap report.

## Phase 10 - Mutation, Coverage, and Test-the-Tests

- [ ] `VERIF-P10-001` Define first mutation shards: bytecode validator, runtime
  value/type conversion, HIR diagnostics, parser recovery, retain/restart,
  connector status projection.
- [ ] `VERIF-P10-002` Coverage is adequacy signal, not release safety proof.
- [ ] `VERIF-P10-003` Add mutation survivor report format.
- [ ] `VERIF-P10-004` Safety-critical survivors require added test,
  unreachable/defensive-code rationale, or dead-code removal.
- [ ] `VERIF-P10-005` Mutation/coverage runs use delivered-build confirmation
  where relevant.
- [ ] `VERIF-P10-006` Keep first mutation gates focused.

## Phase 11 - Hardware Lab Program

- [ ] `VERIF-P11-001` Create hardware lab matrix.
- [ ] `VERIF-P11-002` Define Modbus lab cases.
- [ ] `VERIF-P11-003` Define MQTT lab cases.
- [ ] `VERIF-P11-004` Define ADS/TwinCAT lab cases.
- [ ] `VERIF-P11-005` Define EtherCAT lab cases.
- [ ] `VERIF-P11-006` Define GPIO lab cases.
- [ ] `VERIF-P11-007` Public hardware docs stay preview/unverified until lab
  row passes or is scoped.
- [ ] `VERIF-P11-008` Add hardware-lab report renderer with skipped/unproven
  rows visible.

## Phase 12 - Editor, UI, and Human Workflow Verification

- [ ] `VERIF-P12-001` Map accepted VS Code UI journeys to invariants.
- [ ] `VERIF-P12-002` Backend/runtime proof can support UI acceptance but cannot
  replace journey evidence.
- [ ] `VERIF-P12-003` Source transformations such as rename/import are
  `silent_corruption` risk unless proven otherwise.
- [ ] `VERIF-P12-004` Add LSP/editor negative tests: Unicode positions,
  cancellation, stale dirty close, eviction, partial results, blocking inline
  values.
- [ ] `VERIF-P12-005` Changed visible protocol/status/HMI surfaces invalidate
  affected journey screenshots until recaptured.
- [ ] `VERIF-P12-006` Report UI journeys with backend changes but no fresh
  visual evidence.
- [ ] `VERIF-P12-007` Add VS Code extension gate mapping and evidence rules.
- [ ] `VERIF-P12-008` UI-area invariants can reach `validated` only when linked
  journey rows are `ux_accepted`; provisional journey evidence may support but
  cannot close UI acceptance. The validator consumes acceptance-board audit
  output as evidence.

## Phase 13 - Security, Supply Chain, Platform, and Release Evidence

- [ ] `VERIF-P13-001` Define release evidence manifest: commit, branch, version,
  changelog, platform matrix, test gates, conformance summary, hardware lab
  status, UI acceptance status, security/dependency status, release workflow,
  tag, latest release.
- [ ] `VERIF-P13-002` Add release summary renderer from CI artifacts and checked
  metadata.
- [ ] `VERIF-P13-003` Release evidence distinguishes local, remote-builder, CI,
  hardware-lab, and public GitHub proof.
- [ ] `VERIF-P13-004` Release summary includes known gaps and skipped lab rows.
- [ ] `VERIF-P13-005` Version bump is not complete until annotated tag, Release
  workflow, and Latest-release proof exist.
- [ ] `VERIF-P13-006` Define dependency/security gate policy: cargo/npm audit or
  deny-style checks where configured, license/provenance rules, exception owner
  and expiry.
- [ ] `VERIF-P13-007` Define release artifact identity checks: checksum,
  artifact-to-commit mapping, tested tag, package/VSIX version sync.
- [ ] `VERIF-P13-008` Define supported platform matrix and required proof for
  each supported path/package behavior.

## Phase 14 - Governance and Maintenance

- [ ] `VERIF-P14-001` Add owner rules for invariants and suites.
- [ ] `VERIF-P14-000` Define grace periods in committed validator config. Any
  "after grace period" rule must name duration or milestone, for example 30 days
  or next release.
- [ ] `VERIF-P14-000A` Define owner alias resolution for crate names, teams, and
  domain owners.
- [ ] `VERIF-P14-000B` Define suite composition semantics for `includes` and
  `excludes`.
- [ ] `VERIF-P14-000C` Add area-level coverage-dimension templates so
  non-applicable dimensions are reviewed once per area rather than filled by
  rote per invariant.
- [ ] `VERIF-P14-002` Add stale metadata check.
- [ ] `VERIF-P14-003` Every safety bug fix adds or updates invariant mapping.
- [ ] `VERIF-P14-004` Every new protocol, lifecycle feature, bytecode feature,
  source transformation, public hardware claim, security/release claim, or
  platform claim adds metadata before close.
- [ ] `VERIF-P14-005` Add periodic review cadence: monthly ignored-test audit,
  release-time hardware/security gap audit, quarterly mutation/fuzz review.
- [ ] `VERIF-P14-006` Add archive policy for obsolete evidence and retired
  invariants.

## Phase 15 - Codex Skill and Agent Instruction Sync

This phase starts only after the verification control plane, metadata
validation, and first working reports exist.

- [ ] `VERIF-P15-001` Update `AGENTS.md` with implemented workflow.
- [ ] `VERIF-P15-002` Create `.codex/skills/trust-test-authoring/SKILL.md`.
- [ ] `VERIF-P15-003` Keep `trust-test-authoring` concise; route to this
  document set for detail.
- [ ] `VERIF-P15-004` Add `agents/openai.yaml` entry if repo-local skills use UI
  metadata at that point.
- [ ] `VERIF-P15-005` Update `.codex/skills/st-lsp-solid/SKILL.md`.
- [ ] `VERIF-P15-006` Update
  `.codex/skills/trust-architecture-automation/SKILL.md`.
- [ ] `VERIF-P15-007` Update `.codex/skills/trust-remote-builder/SKILL.md`.
- [ ] `VERIF-P15-008` Update domain skills:
  `.codex/skills/trust-hmi-contracts/SKILL.md`,
  `.codex/skills/trust-vscode-quality/SKILL.md`,
  `.codex/skills/vscode-ui-acceptance/SKILL.md`,
  `.codex/skills/trust-ci-release-gates/SKILL.md`.
- [ ] `VERIF-P15-009` Move detailed skill text into reference files if any
  `SKILL.md` would become too large.
- [ ] `VERIF-P15-010` Validate new skill with dry-run prompts for bug fix,
  refactor-only, malformed-input test, VS Code behavior, runtime safety,
  hardware lab claim, docs-only, supply-chain/release claim.
- [ ] `VERIF-P15-011` Validate updated skills route correctly.
- [ ] `VERIF-P15-012` Record skill-sync evidence.

## Review Acceptance

- [ ] `VERIF-REVIEW-001` Fable review returns `clear`, `clear-with-edits`, or
  `blocked`.
- [ ] `VERIF-REVIEW-002` Every required edit is folded into this document set.
- [ ] `VERIF-REVIEW-003` Disputed recommendations are recorded with decision and
  owner.
- [ ] `VERIF-REVIEW-004` Only after review is folded in may Phase 1
  implementation start.
