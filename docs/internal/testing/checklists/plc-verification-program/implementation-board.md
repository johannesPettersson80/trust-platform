# Verification Program Implementation Board

Status: reviewed. External review returned `clear-with-edits` (Fable,
2026-07-08); required edits are folded and verified, and `VERIF-REVIEW-004` is
cleared. The spec-first planner amendment follow-up review edits are also
folded, and the spec-matrix final review cleared implementation start. Phase 1
may start. The policy stop gates still govern implementation.

This board sequences implementation. Policy lives in `policy.md`; schema and
record details live in `metadata-model.md`; evidence and traceability details
live in `metadata-evidence-traceability.md`.

## Phase 0 - Review and Baseline Freeze

- [x] `VERIF-P0-001` Create evidence root:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-08/`.
- [x] `VERIF-P0-002` Save this split document set as the initial review input.
  Saved as a post-fold snapshot under `initial-review-input/`; provenance in
  `snapshot-provenance.md`; reviewed pre-fold text is quoted per finding in the
  verdict.
- [x] `VERIF-P0-003` Send `fable-review-brief.md` to the reviewer. Review
  executed in-session on 2026-07-08.
- [x] `VERIF-P0-004` Fold required review edits into this document set before
  any implementation row starts.
- [x] `VERIF-P0-005` Record review verdict and unresolved questions under the
  evidence root (`review-verdict.md`).
- [x] `VERIF-P0-006` Capture current counts of Rust source files, crate test
  files, fixtures, ignored tests, conformance cases, fuzz targets, CI workflows,
  specs/decision docs, and gate scripts (`baseline-counts.md`).
- [x] `VERIF-P0-007` Record current dirty-worktree caveat. This board must not
  overwrite unrelated implementation changes (`baseline-counts.md`).
- [x] `VERIF-P0-008` After folding review edits, record a fold summary and rerun
  doc-consistency checks: line counts, duplicate checkbox IDs, ignored tracked
  paths, stale links, and machine-status vocabulary scan
  (`fold-verification.md`).

Acceptance:

- External review verdict exists.
- Required edits are folded in.
- No test runner or source behavior changed.

## Phase 1 - Verification Control Plane Skeleton

- [x] `VERIF-P1-001` Add `verification/README.md` explaining storage rules and
  relationship to crate tests, conformance, fuzz, CI artifacts, and internal
  evidence.
- [x] `VERIF-P1-002` Add JSON schemas:
  `invariant.schema.json`, `suite.schema.json`, `catalog.schema.json`,
  `ignored-test.schema.json`, `risk-register.schema.json`,
  `evidence.schema.json`,
  `spec-source.schema.json`, `spec-gap.schema.json`, and
  `spec-matrix.schema.json`.
- [x] `VERIF-P1-003` Add empty or seed TOML files under
  `verification/invariants/**`.
- [x] `VERIF-P1-004` Add seed suite definitions under `verification/suites/**`.
- [x] `VERIF-P1-005` Add `verification/spec-sources.toml`.
- [x] `VERIF-P1-006` Add `verification/spec-gaps.toml`.
- [x] `VERIF-P1-007` Add `verification/test-catalog.toml`.
- [x] `VERIF-P1-008` Add `verification/ignored-tests.toml`.
- [x] `VERIF-P1-009` Add `verification/risk-register.toml`.
- [x] `VERIF-P1-010` Add `verification/evidence-index.toml`.
- [x] `VERIF-P1-011` Add validation script:
  `scripts/validate_verification_metadata.py`.
- [x] `VERIF-P1-012` Add a cheap local/CI check that validates metadata schemas
  only. This must not run Rust tests.
- [x] `VERIF-P1-013` Document generated-report vs committed-metadata rules,
  including durable evidence: committed repo file, named CI artifact with
  retention, or public release object.
- [x] `VERIF-P1-014` Encode coverage matrix metadata in the invariant schema.
- [x] `VERIF-P1-015` Encode test class, oracle/spec refs, suite tier, evidence,
  and malformed-input taxonomy fields in the catalog schema.
  The original seed encoded the generic test class and reference fields but
  did not yet provide a surface-specific malformed-class binding. That prior
  overstatement is corrected by `VERIF-P2-009`, which adds the reviewed
  `malformed_input_class_ids` contract and bytecode/VM pilot taxonomy.
- [x] `VERIF-P1-016` Add `schema_version = 1` to every metadata schema and record
  fixture.
- [x] `VERIF-P1-017` Add cross-field validation rules for status progression:
  `test_written`, `implemented`, and `validated` cannot be hand-edited without
  the required tests, proof levels, evidence refs, specified spec status, and
  closed safety coverage cells.
- [x] `VERIF-P1-018` Add invariant schema support for `contract_kind`, `[input]`,
  and `[[behavior]]` decision-table rows. v1 supports only single-input,
  data-shaped partitions with typed outcomes and explicit oracle/spec-gap refs.
- [x] `VERIF-P1-019` Add case-file and case-artifact schemas. Catalog records
  may reference `case_file` plus `case_file_digest`; the validator recomputes
  the digest and rejects stale or weakened case tables.
- [x] `VERIF-P1-020` Add validation rules for spec-first case derivation:
  behavior rows cannot invent expected outcomes, missing behavior rows become
  spec gaps, high-risk red/green evidence from non-allowlisted producers is
  rejected, and decision-table partitions must cover applicable domains or name
  a `spec_gap_ref`.
- [x] `VERIF-P1-021` Encode the TOML container convention in schemas and
  validator fixtures: one invariant per file under
  `verification/invariants/<area>/<ID>.toml`; flat registries use plural wrapper
  arrays such as `[[spec_sources]]`, `[[spec_gaps]]`, and `[[evidence]]`.
- [x] `VERIF-P1-022` Encode structured `delta` validation for decision-table
  rows. Do not accept stringly-typed delta blobs; v1 uses closed values for
  target, siblings, retain, process image, diagnostics, and status.
- [x] `VERIF-P1-023` Add producer allowlist validation. For `safety_critical`,
  `wrong_result`, `silent_corruption`, and `false_status` red/green proof, only
  `prove.py vN` or an approved gate can close proof.
- [x] `VERIF-P1-024` Add waiver validation: `not_applicable` coverage cells and
  waived matrix rows require `decision_ref` to an active reviewed decision or
  deviation. Risk-change reporting requires a baseline revision.

Acceptance:

- Metadata validates.
- Empty skeleton does not claim coverage.
- No existing tests are moved.
- Every planned metadata file has a schema.
- Evidence has a committed record type; evidence refs point to evidence IDs, not
  raw paths.
- Spec sources and spec gaps can be tracked before tests are proof-mapped.
- The schema can represent the spec-first planner/case/prover workflow without
  adding a separate behavior-record layer.

## Phase 1A - Specification Source Inventory

This phase inventories written contracts before existing tests are treated as
proof.

- [x] `VERIF-P1A-001` Inventory existing spec sources under
  `verification/spec-sources.toml`, starting with bytecode/VM and runtime
  safety. The first bytecode source row should point at the real committed
  `docs/specs/12-bytecode.md`; the validator semantic contract remains a
  separate spec gap until written. Initial seed landed in
  `verification/spec-sources.toml` with bytecode/VM, runtime safety, debug/DAP,
  IEC decision/deviation, source-build, and first public-claim records.
- [x] `VERIF-P1A-002` Add a source scanner/report for likely spec documents:
  `docs/specs/**`, `docs/internal/**`, top-level README, public docs,
  conformance docs, release docs, and protocol/lab notes. The mechanical
  tracked-file scanner inventories 392 authored/reachable documents and 14,102
  rendered public-prose blocks. The mutable evidence plane is excluded to
  prevent provenance cycles except for the two explicitly registered
  evidence-backed specification sources; tracked public snippet includes are
  still followed recursively.
- [ ] `VERIF-P1A-003` Classify each source by area, authority level, owner,
  freshness, public/internal visibility, and oracle usability. The 20
  registered sources are classified, but 375 discovered documents remain
  `unreviewed_candidate`; this row stays open until that denominator receives
  reviewed dispositions.
- [x] `VERIF-P1A-004` Emit obvious-missing-spec report by area:
  bytecode format, bytecode validator, VM value semantics, scan-cycle lifecycle,
  stop/safe-state, retain/restart, protocol status/discovery, HMI API/UI,
  source transformations, LSP sync/positions/cancellation, debug/DAP
  force-write-release lifecycle, control/RBAC/security, PLCopen import/export,
  test-harness/simulation semantics, runtime/project/HMI config schemas, CLI and
  control-socket surfaces, GPIO, runtime performance budgets, supply chain,
  platform/package behavior, and release proof. The ordered 21-topic ledger is
  explicit and area-bound: two source-present, eight gap, eight partial, and
  three unrepresented, with zero broken metadata references. It creates no
  source or gap from titles, paths, or prose similarity.
- [x] `VERIF-P1A-005` Emit public-claim report: public/user-facing docs claims
  with no normative source, no invariant, or no proof path.
  Initial report:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-08/public-docs-truth-scan-initial.md`.
- [ ] `VERIF-P1A-006` Emit conflict/staleness report: docs that disagree, specs
  that reference removed behavior, stale checklist rows, duplicate decisions.
  The source audit now reports explicit conflicts, registered-source review
  dates, broken local references, and duplicate decision headings, but
  checklist-row staleness and removed-behavior semantics remain unreviewed;
  this row stays open.
- [x] `VERIF-P1A-007` For external standards that cannot be committed, record
  local path, retrieval expectation, version/date, and whether absence blocks
  proof. `SPEC_IEC_61131_3_ED3_EXTERNAL_001` records Edition 3, the expected
  ignored local path, retrieval posture, publication date, non-redistribution,
  and proof-blocking absence without reading or provenance-binding local
  standard bytes; it remains non-oracle.
- [x] `VERIF-P1A-008` For every missing or ambiguous bytecode/VM pilot source,
  create a `spec_gap` row before cataloging tests as proof for that behavior.
  All 19 bytecode/VM required-spec rows resolve explicitly: ten to active
  sources and nine to actionable specification gaps, with zero broken rows.
- [x] `VERIF-P1A-009` Do not mark Phase 2 catalog entries as proof-mapped unless
  their invariant can point to a spec source or spec gap. Full metadata
  validation rejects a mapped catalog row lacking both `oracle_ref` and
  `spec_gap_ref`; the regression fixture removes both from the mapped
  bytecode-container row and observes that production rejection.
- [x] `VERIF-P1A-010` Create the required-specification matrix at
  `verification/spec-matrix.toml` with a schema. Keyed by canonical machine
  `area`; per area, list required spec kinds as coverage tags with
  `expected_authority`, `owner`, current `source_ref` or `spec_gap_ref`, and
  `blocks = test_mapping | release_claim | none_yet`. The validator fails when
  a required tag resolves to neither an active source nor an open gap, or when a
  source's `covers` list or authority does not satisfy the requirement.
  `plan_tests.py` defines "uninventoried area" as any required
  `test_mapping`-blocking tag unresolved for that area. Only bytecode/VM rows
  must be complete for Phase 1B; other areas may carry open gaps with owners.
  Resolve the `control_security` mapping before the matrix lands.

Acceptance:

- The repo can answer "what specs do we have?" before "what tests do we have?"
- Missing/stale/conflicting specifications are visible.
- Bytecode/VM pilot can separate test gaps from spec gaps.
- Public claims without specs are reported instead of accepted.

## Phase 1B - Spec-First Test Planning Pilot

This phase pulls the minimum planner prerequisites forward from later phases and
proves the model on bytecode/VM only. All non-pilot areas must fail closed as
not inventoried until their metadata exists.

- [x] `VERIF-P1B-001` Add `verification/matrix.toml` with area globs,
  area-to-risk defaults, required test classes, and required case families for
  the bytecode/VM pilot. This is the machine-readable form of
  `test-taxonomy.md`; add a drift check so prose and metadata cannot diverge.
  This row satisfies the bytecode/VM slice of `VERIF-P5-009`.
- [x] `VERIF-P1B-002` Add changed-file classifier for the pilot with
  default-deny behavior: unmapped file exits blocked, uninventoried area exits
  blocked, and unknown risk is treated as highest until classified. This row
  satisfies the bytecode/VM slice of `VERIF-P5-010`.
- [x] `VERIF-P1B-003` Add `plan_tests.py` scoped to bytecode/VM:
  `plan_tests.py --intent bugfix|feature|refactor|docs|test-refactor
  (--changed <files> | --area <area>)`. Exit codes are `0 clear`,
  `2 missing tests/cases`, `3 spec_gap`, and `4 unmapped`. The planner must not
  emit expected behavior text. Risk-change reports require `--baseline <rev>` or
  the CI pull-request merge base.
- [x] `VERIF-P1B-003A` Inventory stable error-code identifiers for pilot
  surfaces before behavior rows pin error codes. If subrange, declared-type
  conversion, string-bound, or bytecode-validation paths only expose ad-hoc
  strings, record a spec gap or a small explicitly gated product-change
  exception before `VERIF-P1B-004`.
- [x] `VERIF-P1B-003B` Harden the planning pilot before behavior rows:
  taxonomy-to-metadata drift check, planner metadata self-validation, distinct
  usage/metadata-invalid exit codes, risk/waiver reporting, runnable-test-only
  coverage counting, catalog path checks for runnable tests, orphan-open-gap
  detection, bytecode/VM error-model path coverage, and metadata document split.
- [x] `VERIF-P1B-004` Seed decision-table behavior rows for three real pilot
  bug classes: subrange runtime write, declared-type conversion on store, and
  `STRING[n]` bounded write. If a boundary decision is unwritten, keep it as a
  `spec_gap`; the pilot should prove honest exit-3 behavior.
- [x] `VERIF-P1B-004A` Fold the P1B-004 review fixes before case generation:
  behavior/test `oracle_ref` values must resolve to active non-public-claim spec
  sources, behavior `error_code` values require a stable-error-code-model spec
  source, `equals` partitions are opaque uppercase labels, and the subrange and
  `STRING[n]` invariants include `wrong_type_or_shape` coverage cells.
- [x] `VERIF-P1B-005` Add `gen_cases.py` for the pilot. It may derive boundary
  cases from decision-table rows and validate committed case files. It may only
  generate expected outcomes by copying behavior rows with oracle refs or by
  marking blocked cases with `spec_gap_ref`.
- [x] `VERIF-P1B-005A` Fold the P1B-005 review fixes before committed case
  tables: `equals` behavior rows must name an explicit `case_family`, generated
  cases must emit scenario labels separately from typed input values, mixed
  partition key sets must fail validation, metadata-invalid generation exits
  `6`, and case files carry a generator-source digest.
- [x] `VERIF-P1B-006` Add committed pilot case tables under
  `verification/cases/bytecode_vm/**` with source digests, generator-source
  digests, and catalog digests. Case family values must come from
  `test-taxonomy.md` coverage dimensions. CI/report checks for this slice must
  regenerate via the derived default path, not a caller-supplied `--check` path.
- [x] `VERIF-P1B-006A` Fold the P1B-006 review fixes before the case runner:
  case-file expected outcomes must resolve to and match oracle-backed behavior
  rows, `verification_metadata_gate.sh` must run derived-path case regeneration
  checks for cataloged case files, wrong-type/malformed generated cases must use
  shape descriptors rather than typed input fields, and case-file required-field
  enforcement must reuse the schema required-field list.
- [x] `VERIF-P1B-007` Add the small `crates/verification-cases` dev-helper
  crate with `run_case_file!` and `StateProbe`. v1 probes are limited to process
  image hash, retain hash, target and sibling variables, emitted diagnostics,
  and case-artifact emission through existing test harnesses.
- [x] `VERIF-P1B-007A` Fold the P1B-007 review fixes before `prove.py`: case
  file digest is mandatory in `RunConfig`, the default artifact directory is the
  workspace-root `target/gate-artifacts/cases`, case-file `schema_version` is
  rejected unless it is `1`, and the helper hashes the same bytes it parses.
- [x] `VERIF-P1B-008A` Define the `prove.py` proof contract before
  implementation: artifact discovery, catalog binding, red/green/lock evidence
  fields, failure-kind classification, green pairing, lock baseline/compare,
  and adversarial self-test fixtures. This row is design/metadata only; it does
  not add proof-producing `prove.py` execution.
- [x] `VERIF-P1B-008B` Fold the P1B-008A review fixes before implementation:
  same-run artifact freshness through `TRUST_VERIFY_*` stamps, proof-complete
  green pairing, contract field names aligned to the validator
  (`paired_red_evidence`, `formerly_red_case_ids`), implementable lock
  comparison basis, `decision_ref` for digest/delta exceptions, ignored-test
  refusal, no-retry rule, and expanded adversarial fixtures. This row is
  design/metadata only; it does not add proof-producing `prove.py` execution.
- [x] `VERIF-P1B-008C` Add the same-run artifact stamping foundation to
  `verification-cases`: when `TRUST_VERIFY_TEST_ID`,
  `TRUST_VERIFY_RUN_ID`, `TRUST_VERIFY_CASE_FILE_DIGEST`, and
  `TRUST_VERIFY_ARTIFACT_DIR` are present, the helper stamps them into the case
  artifact; partial or mismatched stamp environments fail before case execution.
  Update the case-artifact schema/model and keep `scripts/prove.py` absent.
- [x] `VERIF-P1B-008D` Fold the P1B-008C review fixes before `prove.py`:
  serialize and clear process-global `TRUST_VERIFY_*` environment in every
  helper test that calls `run_case_file`, make partial/mismatch stamp tests use
  runnable case files and prove no probe or runner execution occurred, add test
  coverage for `TEST_ID` and `ARTIFACT_DIR` mismatches, and document exact
  artifact-dir path matching plus unique non-empty run-id expectations.
- [x] `VERIF-P1B-008E` Add `prove.py red` only: catalog lookup, metadata
  validation before proof, planned/ignored-test refusal, stale artifact cleanup,
  `TRUST_VERIFY_*` run stamping, single command execution, same-run case-artifact
  validation, assertion-failure red evidence generation, and adversarial
  self-tests for stale/fake artifacts and non-red failure classes. `green` and
  `lock` remain unimplemented and `VERIF-P1B-008` remains open.
- [x] `VERIF-P1B-008F` Fold the P1B-008E review fixes before green/lock:
  reserve `expected_red_failure_kind` until expected-rejection proof has a
  validator-backed catalog contract, classify command timeouts as non-red
  `timeout` instead of raw tracebacks, raise the red command timeout to a cold
  cargo-safe default, and add self-tests for wrong failure classes plus case-ID
  set violations.
- [x] `VERIF-P1B-008G` Add `prove.py green` only: pair to red/protective-red
  evidence for the same test, require matching case-file digest and non-empty
  formerly-red case IDs, rerun the cataloged command with fresh
  `TRUST_VERIFY_*` stamps, require every formerly-red case to pass with no
  failed or skipped cases, write green evidence, and add committed-evidence
  pairing validation. `lock` remains unimplemented and `VERIF-P1B-008` remains
  open.
- [x] `VERIF-P1B-008H` Fold the P1B-008G review fixes before lock: pin case
  artifact result vocabulary, reject blocked or unknown-result cases from green
  proof, require paired red evidence to link exactly one test, anchor committed
  green/red pairs to the current catalog `case_file_digest`, and expand
  prover/validator self-tests for these adversarial paths.
- [x] `VERIF-P1B-008I` Add `prove.py lock --baseline|--compare`: record
  lock baselines with command exit status, catalog case-file digest,
  deterministic `case_result_digest`, raw case-artifact provenance digest, and
  per-case summary; compare reruns against `lock_baseline` evidence for the
  same test and reject wrong/missing baselines, catalog digest drift, command
  exit deltas, case-result deltas, failed cases, and blocked cases. Keep the
  umbrella `VERIF-P1B-008` open for review.
- [x] `VERIF-P1B-008J` Fold the P1B-008I review fixes before closing
  `VERIF-P1B-008`: require lock baselines to come from `prove.py vN` or an
  approved proof producer, reject baseline/catalog command drift at prove time
  and at rest, require lock baseline/compare command status `0`, recompute
  `case_result_digest` from exit status plus per-case summary, refuse
  case-file-less lock baselines, add baseline-side refusal tests, pin missing
  lock fields at rest, and extract shared proof-record writing.
- [x] `VERIF-P1B-008` Add `prove.py red|green|lock --test <TEST_ID>`. It runs
  the cataloged command, validates the case artifact and digest, distinguishes
  assertion failure from compile error or harness panic, and writes the evidence
  record itself. `prove.py green` must pair to the red evidence record and prove
  formerly-red case IDs are now green, no previously-green case regressed, the
  case-file digest matches, and no case was skipped without a waiver.
- [x] `VERIF-P1B-009` Add one deterministic bytecode transform generator:
  container truncation by section boundary, unknown opcode sweep, jump-target
  sweep, and stack-underflow cases from committed seed artifacts. Other
  generated families remain out of v1.
- [x] `VERIF-P1B-010` Add a cheap `verification-gate` report-only CI path:
  metadata validation, planner re-derivation on PR diff, `gen_cases.py --check`,
  and ratchet report for new/modified tests that lack catalog entries. Do not
  enforce outside the pilot during burn-in.
- [x] `VERIF-P1B-011` Add adversarial self-test fixtures for the planner/case
  tooling: assert-nothing red proof, skipped case, stale digest, missing oracle,
  spec-gap closure, risk downgrade without waiver, manual safety evidence,
  compile-error-as-red, uncataloged test, and unmapped file.
- [ ] `VERIF-P1B-012` Keep `VERIF-STOP-012` closed for skills/agent mandate
  until the pilot has red/green proof, report-only CI has burned in without
  false blocks, and the bytecode-validator mutation shard has reported
  survivors against case IDs. This row stays open across phases until
  `VERIF-P1B-013` and `VERIF-P1B-014` are complete.
- [x] `VERIF-P1B-013` Pull the first bytecode-validator mutation shard forward
  from Phase 10 and report survivors against case IDs. This satisfies only the
  bytecode-validator slice of `VERIF-P10-001`. The focused two-mutant shard
  reported both mutants caught and zero survivors against five associated
  committed case IDs; two decode-boundary truncation IDs remain explicitly out
  of scope. Machine report and implementation evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-08/p1b-bytecode-validator-mutation-report.json`
  and `p1b-bytecode-validator-mutation-shard.md` in the same evidence root.
- [ ] `VERIF-P1B-014` Flip the bytecode/VM pilot ratchet from report-only to
  enforcing after burn-in: at least three organic PRs or implementation slices
  run with zero false blocks, pilot red/green proof is captured, waiver/risk
  reports are reviewable, and fallback override procedure is documented.

Acceptance:

- A bytecode/VM change can ask the planner what tests and cases are required.
- Missing spec behavior blocks with exit 3 instead of letting an agent invent an
  oracle.
- The first case/proof loop can show named cases red before implementation and
  green after implementation.
- Enforcing mode is a separate ratchet row, not implied by report-only CI.
- The tooling remains thin: scripts under `scripts/verification/**`, metadata
  under `verification/**`, and a small dev-only helper crate.

## Phase 2 - Existing Test Catalog

- [x] `VERIF-P2-001` Add a catalog generator that scans:
  `crates/*/tests`, practical in-source `#[cfg(test)]` modules,
  `editors/vscode/src/test`, `conformance`, `fuzz`, `scripts/*gate*`, and
  `.github/workflows`.
- [x] `VERIF-P2-002` Extract test names, package, command hint, file path,
  ignore attribute, and obvious checklist/evidence references.
- [x] `VERIF-P2-003` Emit generated catalog JSON under `target/gate-artifacts`
  and concise Markdown summary under dated evidence root.
  Clean-source evidence at commit `c4be8261d1672f146adc2420495eab5265ecc8b8`
  inventories 3,816 records: 3,021 Rust, 257 runnable Structured Text, 456 VS
  Code, 21 conformance, 2 fuzz, 29 root gate scripts, and 30 workflow jobs. It
  reports 85 unconditional ignores, one conditional ignore marker, and one
  visible VS Code runtime-skip diagnostic. The generated JSON stays under
  `target/gate-artifacts/verification/`; the indexed durable summary is
  `docs/internal/testing/evidence/plc-verification-program/2026-07-09/p2-existing-test-catalog.md`.
  Mechanical reference candidates create no proof mappings. The report
  explicitly excludes `xtask/**` Rust tests and
  crate-local fuzz workspaces pending a later reviewed scope row; its exact
  live counts are an evidence-refresh tripwire rather than CI enforcement.
- [x] `VERIF-P2-004` Create committed `verification/test-catalog.toml` only for
  hand-owned metadata that cannot be safely inferred. Catalog schema v2 uses a
  closed subject discriminator and requires review-owned expected result,
  failure mode, evidence destination, and review date. The first native row is
  `TEST_BYTECODE_CONTAINER_INVALID_MAGIC`, bound to generated fact
  `DISC_88F921D24D3708CEF3E1`. It maps only the specified STBC-magic rejection
  in inventoried `bytecode_vm`; at that checkpoint exact error-code stability
  remained under `SPEC_GAP_VM_ERROR_MODEL_001`, and no suite was falsely
  assigned.
  Four case-table rows and the bytecode-validator mutation runner use closed
  non-native artifact kinds rather than a generic scanner exemption.
- [x] `VERIF-P2-005` Add stale-path checker for committed catalog entries.
  `scripts/check_test_catalog_staleness.py` scans current sources in memory and
  validates all six committed paths without trusting an old target artifact.
- [x] `VERIF-P2-005A` Stale catalog checks must verify file path and test name
  against scanner output; a renamed/deleted test function inside a surviving
  file must fail validation. Generated rows resolve exactly one discovery ID
  and require its source kind, path, and name to match; fixtures also reject
  moves, duplicates, path escape, and invalid artifact exemptions while
  accepting line-only movement.
- [x] `VERIF-P2-006` Add VS Code extension-test registration checker.
  `scripts/check_vscode_test_registration.py` verifies 38/38 source test files
  and all 456 discovered VS Code facts are registered by direct literal loads
  in `suite/index.ts`; malformed boundaries, orphans, missing/duplicate/case-
  mismatched targets, dynamic/conditional loads, traversal, and symlink escape
  fail. Both new checkers remain standalone and CI remains report-only.
  The fact-level join is performed by the standalone audit itself, not only by
  a live-repository unittest; scanner facts in an out-of-suite TypeScript file
  or a JavaScript test file fail. Catalog `subject_kind` and
  `discovery_source_kind` schema enums are also drift-pinned to the validator
  vocabularies in a dedicated module, leaving validator `core.py` below 1,000
  lines.
  Implementation/evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2-catalog-binding-registration.md`.
- [x] `VERIF-P2-007` Add test-class completeness report. The closed-schema,
  report-only generator separates exact scanner-fact classification from
  mapped-area required-class completeness. Refreshed clean-source report
  commit `bb82eaf9cc3b6a2221e91ed1353bcd9fd88c6aa9` classifies 1/3,816 scanner facts
  and reports 3,815 as debt. Across 11 mapped areas, two of 32 required class
  slots are complete; both are in `bytecode_vm` (`mutation` and
  `negative_malformed_input`). Four planned case-table rows are visible under
  `metadata_validation` but do not count, and ignored/conditional generated
  facts cannot count as effectively runnable. The at-rest validator recomputes
  live scanner/catalog/matrix joins, full metadata validity, tool/schema input
  digests, canonical command/time shape, clean source-commit inputs, and the
  Markdown-to-JSON digest. Debt exits successfully and no CI enforcement was
  added. Durable report:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2-test-class-completeness.md`.
  Generated JSON SHA-256:
  `6a9a71d10ca42195e9316d2e193914a07432080aa92b6854a041404eab93be9d`.
- [x] `VERIF-P2-008` Add coverage-matrix gap report with states:
  `covered`, `covered_by_fuzz`, `not_applicable`, `blocked`, `spec_gap`,
  `gap_open`, `deferred`.
  Clean-source report commit `bb82eaf9cc3b6a2221e91ed1353bcd9fd88c6aa9`
  assesses all 11 mapped areas and 52 invariants. The authorized family model
  remains bytecode/VM-only: 16 of 80 required invariant/family slots have
  declared cells and 64 remain structurally unassigned. The other ten areas
  define no invented required families; their recorded cells remain visible as
  additional observations. In total, 61 recorded cells comprise 53
  `spec_gap` and eight `gap_open` cells. Four catalog-bound case files
  contribute 21 blocked observations without upgrading any state. Missing
  cells receive no synthetic state. Generated JSON SHA-256:
  `ef94cf871b29cd1c15f07e070786bb623e9f51db532a31cc2ed9a505fa8f7ac3`.
- [x] `VERIF-P2-009` Add malformed-input coverage report. A reviewed,
  bytecode/VM-only machine taxonomy now atomizes 28 classes and the catalog
  binds classes only through `malformed_input_class_ids`. The sole reviewed
  mapping is `TEST_BYTECODE_CONTAINER_INVALID_MAGIC` to `bad_magic`, producing
  one `covered` class. `invalid_checksum` and `unsupported_version` remain
  `gap_open` because their existing source tests are still uncataloged; 25
  validator/resource classes remain `spec_gap`. Names, paths, commands, case
  IDs, and mutation associations cannot create a mapping.
- [x] `VERIF-P2-010` Do not fail CI on unmapped tests in the first slice. The
  canonical debt report lists all 3,815 unmapped identities from 3,816 scanner
  facts, including 85 ignored and one conditional fact. Debt exits zero;
  corrupt metadata, stale joins, dirty provenance, symlinked inputs,
  noncanonical JSON, and Markdown tampering fail. No workflow enforcement was
  added.

  Implementation/evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2-coverage-matrix-gaps.md`,
  `docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2-malformed-input-coverage.md`,
  and
  `docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2-unmapped-test-debt.md`.
  The clean-source refresh at
  `bb82eaf9cc3b6a2221e91ed1353bcd9fd88c6aa9` produced malformed-input JSON
  SHA-256
  `6dc3119877543da5a11cc6c71a40e58e9808095f53e8a0faf9b7cb4ce8b91243`
  and unmapped-debt JSON SHA-256
  `37f25fe027ef0060b9e0189125c52c78ccc6fa4decffc5794672ce08834146d2`.
  Clean implementation/evidence commits and the focused plus broad remote gate
  results are bound in
  `docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2-coverage-debt-closure-validation.md`.

Acceptance:

- Existing tests are discoverable.
- Ignored tests are visible.
- Generated report separates inferred facts from hand-authored intent.
- Catalog can answer what mapped tests prove and which malformed classes are
  missing.

## Phase 2A - Existing Test Refactor Plan

- [x] `VERIF-P2A-001` Add report for large or mixed-purpose test files. The
  implementation uses the architecture policy's inclusive 1,000-line review
  threshold and only reviewed catalog area/test-class diversity for purpose;
  names and source text cannot establish mixed purpose.
- [x] `VERIF-P2A-002` Add report for broad tests claiming too many invariants
  without coverage dimensions. Catalog v2 has no authorized dimension field,
  so every multi-invariant row is a candidate and unknown fields cannot satisfy
  the check.
- [x] `VERIF-P2A-003` Add report for duplicated fixtures or near-duplicate
  malformed/boundary inputs. V1 reports whole-file exact/normalized matches,
  explicit malformed-class overlap, exact committed case inputs, same-table
  structural peers, and shared case references. Helper-level similarity is
  explicitly not assessed.
- [x] `VERIF-P2A-004` Add VS Code registration refactor report. The report joins
  all 456 scanner facts to the 38 literal registrations and projects file size,
  fact, ignored, and mapped counts without recommending a refactor.
- [x] `VERIF-P2A-005` Add slow-test classification report. Only hand-owned
  catalog `duration_class` values classify a fact; names, ignore state,
  hardware flags, and suite names never infer duration.
- [x] `VERIF-P2A-006` Require written plan for every proposed move/split/rename:
  before command, after command, invariant IDs, fixture ownership, stale-path
  updates, expected behavior delta. The closed v1 contract validates these
  fields, binds dimensions to explicit malformed-class metadata, and blocks
  split until a multi-target model exists.
- [x] `VERIF-P2A-007` Add catalog redirect/stale-path rule for moved/renamed
  tests. Redirects require one validated proposal per edge and one live
  catalog/scanner endpoint. A second edge for the same test remains blocked
  until proof evidence IDs become proposal-scoped.
- [x] `VERIF-P2A-008` Add before/after focused behavior-lock rule. Completed
  changes require distinct-revision, production-valid case-file-bound
  baseline/compare evidence. Command-changing changes and rows without case
  files remain blocked rather than receiving an invented proof model.
- [x] `VERIF-P2A-009` Add SOLID/KISS/DRY rule for test files. Every proposal has
  a closed three-principle review and fixture-ownership decision; a mechanical
  signal alone never authorizes change.
- [x] `VERIF-P2A-010` Add first pilot refactor proposal only after bytecode/VM
  catalog exists; mark "no refactor needed" if reports show no real need. The
  pilot targets `TEST_BYTECODE_CONTAINER_INVALID_MAGIC`; the reviewed
  disposition is `no_refactor_needed` because the assessment found no observed
  refactor signal for that test.

  The clean-source report at commit
  `bb82eaf9cc3b6a2221e91ed1353bcd9fd88c6aa9` inventories 3,816 facts in 670
  files, 24 inclusive-threshold large-file candidates, zero reviewed mixed-
  purpose or broad-claim candidates, zero exact or normalized fact-file
  duplicate groups, six same-table structural case peer groups, one shared
  case-file reference group, and zero malformed-class overlap groups. It joins
  all 456 VS Code facts to 38 registrations and records only one reviewed
  scanner duration plus five artifact durations. Generated JSON SHA-256:
  `e51fd2a6f8a572e37afc3193ed971a49f9783179a0c8a97052e0187799bd5a13`.

  Change dispositions remain fail-closed. Mechanical signals never authorize
  a move or rename; `split` is blocked until a multi-target contract exists;
  completed command-changing refactors and catalog rows without case-file-
  bound lock evidence are blocked; and a second redirect edge for one test is
  blocked until proof evidence IDs are proposal-scoped. No test was moved,
  split, renamed, or behaviorally changed. Durable report:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2a-test-refactor-assessment.md`.
  Focused local validation and the clean remote `fmt`/`clippy`/`test-all`
  closure are recorded in
  `docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2a-refactor-assessment-closure-validation.md`.
  Clean-full behavior-lock provenance hardening and refreshed report bindings
  are recorded in
  `docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2a-review-fixes-validation.md`.

Acceptance:

- Recommendations are based on catalog evidence, not aesthetics.
- No existing test is moved without before/after behavior-lock proof.

## Phase 3 - Ignored Test Register

- [x] `VERIF-P3-001` Generate ignored-test inventory from Rust, Node,
  Playwright, shell, and conformance surfaces where practical.
- [x] `VERIF-P3-002` Classify every ignored test using machine classes from
  `metadata-model.md`.
- [x] `VERIF-P3-003` For `red_protective`, require linked row and expected red
  symptom.
- [x] `VERIF-P3-004` For `lab_required`, require env vars, hardware topology,
  and public-claim impact.
- [x] `VERIF-P3-005` For `flaky_quarantined`, require owner and last observed
  failure.

  The clean-source report at commit
  `bb82eaf9cc3b6a2221e91ed1353bcd9fd88c6aa9` inventories 88 observations:
  85 static Rust ignores, one conditional Rust ignore, one conditional VS Code
  runtime skip, and one literal Playwright skip. It reports zero discovery
  diagnostics and binds 535 Rust, 47 Node, six Playwright, 29 shell, and 21
  conformance files; shell and conformance remain explicit limitations rather
  than invented identities. Generated JSON SHA-256:
  `0b1421bf23a054f6e789d54fba07ff2e3532da61277fbadc49803195e3ecd9ce`.

  The hand-owned registry joins all 88 observations one-to-one and classifies
  63 as `unknown`, 15 as `perf_soak`, five as `lab_required`, and five as
  `manual`. No source name, path, comment, or lexical reference created a
  catalog mapping: all 88 records omit optional `test_id`. Closed class
  contracts require row/symptom evidence for `red_protective`, environment and
  topology plus public-claim impact for `lab_required`, and dated durable
  failure evidence for `flaky_quarantined`. The standalone live checker owns
  exhaustive source staleness; the primary metadata validator owns the static
  schema and class obligations. Neither command is wired into CI enforcement.

  Durable report:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-10/p3-ignored-test-inventory.md`.
  Focused local/remote validation, the clean mutation replay, and remote
  `fmt`/`clippy`/`test-all` closure are recorded in
  `docs/internal/testing/evidence/plc-verification-program/2026-07-10/p3-ignored-test-register-closure-validation.md`.

  Current-ledger progress (2026-07-17): six obsolete skips were removed after
  focused execution or harness review. The live exhaustive join now contains
  23 observations, all explicitly classified: 15 `perf_soak`, five
  `lab_required`, and three `manual`; zero remain `unknown`.
- [ ] `VERIF-P3-006` Fail metadata validation if any ignored test remains
  `unknown` after the grace period defined per `VERIF-P14-000`. This row stays
  open because that grace period does not yet exist; current unknown debt is
  visible and report-only.

## Phase 4 - Invariant Registry

- [x] `VERIF-P4-000` Import confirmed findings from the 2026-07-04/05 runtime,
  HIR-to-VM, IDE/LSP, and comms reviews as risk-register entries and invariant
  seeds with `gap_open` or `spec_gap` status. Include timer semantics, NaN/Inf
  ingress, runtime authz, OPC UA session lifecycle, and online-change/hot-reload
  consistency. Five V-08 findings are planned risks with a machine-typed
  provenance-only source; they create no behavior oracle or proof.
- [x] `VERIF-P4-001` Seed compiler/frontend invariants. Five seeds map to five
  canonical invariants: two `gap_open`, three `spec_gap`.
- [x] `VERIF-P4-002` Seed HIR-to-bytecode-to-VM seam invariants. Six seeds map
  to five canonical `spec_gap` invariants through the one reviewed type-seam
  merge.
- [x] `VERIF-P4-003` Seed runtime-safety invariants. Nine canonical records
  remain four `gap_open` and five `spec_gap`.
- [x] `VERIF-P4-004` Seed protocol/connectivity invariants. Six canonical
  records remain one `gap_open` and five `spec_gap`.
- [x] `VERIF-P4-005` Seed editor/source-transformation invariants. Eight
  canonical records remain one `gap_open` and seven `spec_gap`.
- [x] `VERIF-P4-006` Seed HMI/UI acceptance invariants. The one seed remains
  `spec_gap` at S0.
- [x] `VERIF-P4-007` Seed release/public-claim invariants. All three seeds
  remain `spec_gap` at S0.
- [x] `VERIF-P4-008` Seed security/supply-chain/platform invariants. All six
  seeds remain `spec_gap` at S0.
- [x] `VERIF-P4-009` Link each safety-critical invariant to at least one oracle
  reference or spec gap.
- [x] `VERIF-P4-010` Mark unproven claims as `gap_open`, `blocked`,
  `deferred`, `spec_gap`, or `unproven`; do not mark them `validated`.

  The clean-source invariant-seed audit at commit
  `bb82eaf9cc3b6a2221e91ed1353bcd9fd88c6aa9` binds all 44 written obligations
  to 43 canonical invariants: 36 new records and eight pre-existing seed
  mappings to seven canonical records. Only `VM_SEAM_TYPE_001` and
  `VM_SEAM_TYPE_002` share a canonical invariant. Seed posture is eight
  `gap_open` and 36 `spec_gap`, all at S0. Across the full registry, all 52
  invariants remain unvalidated at S0: eight `gap_open` and 44 `spec_gap`.
  All nine safety-critical invariants name an oracle-eligible active source or
  an open focused spec gap. Generated JSON SHA-256:
  `f561a3928f77cbf26be506f693e8163f423c00fd1b343e18327f468a1bf6614b`.
  Durable evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-10/p4-invariant-seed-audit.md`
  and
  `docs/internal/testing/evidence/plc-verification-program/2026-07-10/p4-confirmed-findings-source-review.md`.
  Focused local validation and clean remote `fmt`/`clippy`/`test-all` closure
  are recorded in
  `docs/internal/testing/evidence/plc-verification-program/2026-07-10/p4-invariant-spec-audit-closure-validation.md`.

## Phase 4A - Specification Completeness Audit

- [x] `VERIF-P4A-001` Add spec-gap register entries under
  `verification/spec-gaps.toml`. The register now contains 34 focused gaps,
  all open; this slice added 24 and closed none.
- [x] `VERIF-P4A-002` Report invariants with `spec.status != "specified"`. The
  report lists 44 of 52 invariants: 27 `missing` and 17 `ambiguous`.
- [x] `VERIF-P4A-003` Report tests with `expected_result` but no `oracle_ref`,
  `spec_ref`, or `spec_gap_ref`. Zero of six expected-result catalog rows are
  unbound.
- [x] `VERIF-P4A-004` Report coverage dimensions marked `spec_gap`. The report
  lists 53 of 61 coverage cells; the other eight remain `gap_open`.
- [ ] `VERIF-P4A-005` Report public docs claims with no invariant and no oracle.
  The current output shows four registered public-claim sources as explicitly
  non-exhaustive context only. It does not scan all public docs, so this row
  remains open.
- [x] `VERIF-P4A-006` Add close-out rule: spec gap closes only when owning
  spec/decision/deviation/design doc is updated and mapped tests are written or
  explicitly deferred. The primary validator requires an active oracle-eligible
  owning source on a tracked, nonignored, nonsymlinked workspace path, written
  mapped tests or an eligible reviewed deferral, exact closeout-evidence test
  links, and removal of all live gap references.
- [x] `VERIF-P4A-007` Safety-critical `spec_gap` blocks `validated`. Both direct
  invariant references and reverse gap-to-invariant links are checked.
- [x] `VERIF-P4A-008` For bytecode/VM pilot, classify every initial gap as test
  gap, spec gap, hardware/tool blocked, or not applicable. The explicit,
  disjoint denominator contains eight gaps: five open spec gaps and three test
  gaps for `failing_regression`, `iec_conformance`, and `metadata_validation`;
  no hardware/tool or not-applicable classification was inferred.

  The report-only completeness audit was generated from clean commit
  `bb82eaf9cc3b6a2221e91ed1353bcd9fd88c6aa9`. Generated JSON SHA-256:
  `d60b708f6df0523ca5cb41360371e1c799d21418b35f72090b951499811b15c2`.
  Durable report:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-10/p4a-specification-completeness.md`.
  The audit is report-only, creates no proof, closes no spec gap, and does not
  change CI enforcement.

## Phase 5 - Suite Definitions and Gate Mapping

- [x] `VERIF-P5-000` Inventory existing workflows and `scripts/*gate*` into
  suite records or explicit exclusions. The exhaustive live join contains 62
  records: 29 root gate scripts, 30 executable root workflow jobs, one
  non-executable nested workflow template, one exact just recipe, and one
  catalog-bound mutation command. Nightly reliability, HMI soak, device in the
  loop, salsa hardening, release/version, malformed-bytecode fuzz, VM
  reliability, and unsafe/concurrency surfaces are all represented.
- [x] `VERIF-P5-000A` Define `veryquick` environment and just-recipe mapping
  before using it in suite records; do not invent a new broad local Pi gate.
  `just verification-veryquick` is source-bound to its exact bounded command
  sequence on `trust_builder`; its canonical Python runner discovers all 41
  verification `*_tests.py` modules, including the Phase 3 review regressions
  and the Phase 6, Phase 6A, and Phase 7 contract tests.
- [ ] `VERIF-P5-000B` Phase 11 hardware lab work must build on any existing
  device-in-loop workflow/test harness instead of creating a parallel source of
  truth. Phase 5 binds the existing workflow, script, Rust harness, and JSON
  artifact contract, but this standing row remains open until Phase 11 builds
  the reviewed lab program on those sources.
- [x] `VERIF-P5-001` Define `veryquick`: one bounded direct entrypoint plus the
  metadata-gate helper, with no workflow enforcement added.
- [x] `VERIF-P5-002` Define `pr`: 15 direct workflow entrypoints and 29 total
  inventory references; the verification job remains explicitly report-only.
- [x] `VERIF-P5-003` Define `nightly`: ten direct entrypoints and 14 total
  references, including the catalog-bound bytecode-validator mutation shard.
- [x] `VERIF-P5-004` Define `release`: six direct workflow entrypoints and nine
  total references, with preflight represented by its bound CI job result.
- [x] `VERIF-P5-005` Define `hardware_lab`: only the strict device-in-loop
  script is an entrypoint; the skip-capable hosted workflow remains a helper.
- [x] `VERIF-P5-006` Validate command owner, duration, environment, artifacts.
  Closed-schema records, exhaustive direct joins, exact recipe/catalog
  commands, workflow artifact names, CI job results, and hardware output paths
  are revalidated from current source.
- [x] `VERIF-P5-007` Ensure hardware commands are env-gated. Hardware proof
  requires exact `TRUST_DIT_REQUIRE_HARDWARE=1`; the dynamic workflow default
  cannot satisfy the strict suite binding.
- [x] `VERIF-P5-008` Ensure release commands name durable evidence or CI
  artifacts. Every direct release entrypoint binds a workflow artifact,
  release object, or CI job result; `target/**` is rejected as durable output.
- [x] `VERIF-P5-009` Encode code-area matrix in machine-readable metadata.
  Bytecode/VM pilot requirements are pulled forward by `VERIF-P1B-001`; the
  schema-v2 matrix now represents all 11 canonical areas and all 29 stable
  taxonomy routes.
- [x] `VERIF-P5-010` Add changed-file classifier. Bytecode/VM pilot
  classification is pulled forward by `VERIF-P1B-002`; the full row closes only
  when every code area has default-deny routing. Specific routes take
  precedence over canonical area fallbacks; deletions and both rename endpoints
  are retained, unsafe paths fail, unmatched paths default-deny, and conditional
  suites are reported without becoming direct requirements.

  The combined report-only audit was generated independently from clean source
  commit `bb82eaf9cc3b6a2221e91ed1353bcd9fd88c6aa9`. It records 62 inventory
  rows, six suite records, 33 direct commands, 11 areas, and 29 routes. Generated
  JSON SHA-256:
  `4d2617a127a87d23ba1002b3697c806532cf7295bf98279e3e085c01c7eaf583`.
  Durable report:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-10/p5-suite-gate-routing-audit.md`.
  It emits no proof, closes no spec gap, does not interpret suite inheritance,
  and does not change workflow enforcement.

## Phase 6 - Requirement, Oracle, and Traceability Mapping

- [x] `VERIF-P6-001` Map IEC spec/deviation references to compiler/runtime
  conformance invariants. Five `compiler_iec` invariants map only through
  explicit references: two have eligible reviewed sources and three remain
  blocked by open specification gaps. This is not external IEC conformance
  proof; `VERIF-P1A-007` remains open.
- [x] `VERIF-P6-002` Map runtime design contracts to runtime-safety invariants.
  Ten `runtime_safety` invariants are mapped: four have eligible explicit
  oracles and six remain specification-gap blocked.
- [x] `VERIF-P6-003` Map protocol specifications/product decisions to protocol
  invariants. Seven `protocols` invariants are mapped: one has an eligible
  explicit oracle and six remain specification-gap blocked. Public claims are
  retained as context and cannot become oracles.
- [x] `VERIF-P6-004` Map VS Code/LSP protocol references and product contracts
  to editor invariants. All six `editor_safety` invariants are explicitly
  associated and remain specification-gap blocked; no mapping is inferred from
  names, paths, or source text.
- [x] `VERIF-P6-005` Map security/supply-chain/platform contracts to invariants.
  Six invariants across `control_security` and `supply_chain_platform` are
  explicitly associated and remain specification-gap blocked.
- [x] `VERIF-P6-006` Add report for invariants missing oracles. The report
  covers all 52 committed invariants, including 18 outside the five mapping
  scopes: eight have eligible explicit oracles and 44 are blocked by open
  specification gaps. It lists 34 future high-risk enforcement candidates but
  does not fail on them.
- [ ] `VERIF-P6-007` Fail only on missing oracles for `safety_critical`,
  `wrong_result`, `silent_corruption`, and `false_status` risks after the grace
  period defined per `VERIF-P14-000`.
- [ ] `VERIF-P6-008` Add forward traceability report:
  spec source -> invariant -> test -> suite/gate -> evidence -> public claim.
- [ ] `VERIF-P6-009` Add reverse traceability report:
  public claim -> evidence -> suite/gate -> test -> invariant -> spec source.
- [ ] `VERIF-P6-010` Add report for orphan specs, orphan tests, orphan
  invariants, orphan public claims, and orphan evidence.

  The report-only audit was generated from clean source commit
  `bb82eaf9cc3b6a2221e91ed1353bcd9fd88c6aa9`. Generated JSON SHA-256:
  `989e9cb0d7e62048f5949a528ab297473245d779142ce892ba03a54d77e73614`.
  Durable report:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-11/p6-requirement-oracle-audit.md`.
  It creates no proof, closes no specification gap, enables no enforcement,
  and leaves the non-exhaustive public-claim and live-test traceability rows
  open.

Acceptance:

- Critical invariants without oracles are visible before test writing starts.
- Public claims can be traced back to proof or an explicit gap.

## Phase 6A - Verification Tooling Self-Tests

- [x] `VERIF-P6A-001` Add known-good metadata fixture. The named fixture runs
  the unmodified committed metadata graph through full `Validator.validate()`;
  it is not a second hand-maintained copy of the 324-record graph.
- [x] `VERIF-P6A-002` Add known-bad fixtures for missing field, unknown status,
  stale path, unknown invariant, unknown suite, schema mismatch, and public
  claim without proof/gap. All seven are closed, machine-readable fixture
  records and the registered-public-claim rule traverses top-level, behavior,
  and coverage gap references before requiring validated green/lock proof.
- [x] `VERIF-P6A-002A` Add known-bad fixtures for ignored durable evidence path,
  unknown evidence ID, mapped record with empty invariants, stale test name in an
  existing file, `validated` with empty evidence, safety-critical `validated`
  with `gap_open`/`spec_gap`, and proof level below status requirement. The
  stale-name fixture uses a real fresh single-file scanner fact and is assigned
  to live catalog staleness, not the static metadata validator.
- [x] `VERIF-P6A-003` Add validator tests for all known-bad fixtures. Each
  metadata mutation reaches its direct production phase and the same signal
  through full validator wiring; other cases call their owning production API.
- [x] `VERIF-P6A-004` Add catalog scanner self-tests. Existing source-lexer,
  stable-identity, surface, diagnostic, schema, semantic-tamper, and output-
  shape fixtures cover the mechanical scanner without adding a parallel scan.
- [x] `VERIF-P6A-005` Add spec-source scanner self-tests. Six closed fixtures
  call the production scanner and association analysis: known-good, missing
  registered path, unclosed fence, stale registered claim text, escaping
  include, and the report-only unreviewed-prose boundary. These fixtures do not
  claim the still-open semantic classification or conflict-review rows.
- [x] `VERIF-P6A-006` Add changed-file classifier self-tests. Path
  normalization, single/double-star routing, overlaps, intent overlays,
  deleted paths, rename endpoints, malformed name-status input, and default-
  deny behavior are pinned.
- [x] `VERIF-P6A-007` Add report-renderer golden/protective tests. A fixed
  generated-catalog fixture is compared byte-for-byte with an independent
  committed golden, while semantic summary tampering and the existing at-rest
  report suites remain protective failures.
- [x] `VERIF-P6A-008` Add stale metadata tests for deleted/renamed tests and
  removed scripts. The gate-inventory fixture now literally removes a live
  `scripts/*gate*` source while retaining its row and observes a stale-ID
  failure.
- [x] `VERIF-P6A-009` Add known-bad fixtures for the spec-first planner layer:
  decision-table invariant missing behavior rows, case file with unknown family,
  stale case digest, case artifact that skips a case, high-risk red/green
  evidence from a non-allowlisted producer, green proof missing its paired red
  evidence, risk downgrade without `decision_ref`, and compile-error-as-red.
  Covered decision tables cannot omit all behavior rows. Risk downgrades remain
  visible planner findings and may carry only an active oracle-eligible reviewed
  decision/deviation through the optional closed matrix field.
- [x] `VERIF-P6A-010` Add self-tests that assert each bypass is caught by its
  assigned layer. Do not claim the validator catches assertion strength; that is
  handled by red proof and mutation shards where available. All honesty-critical
  fixture fields and handler assignments are drift-pinned; 27/27 fixtures
  produce one accept, 25 rejects, and one report-only risk finding.

  Implementation commit:
  `fa228977fee66537ea22a727555aee297bb28abe`. Fixture manifest SHA-256:
  `f3cad5498064c06d34c704053011fe68fff71953ce72ccafa13c1913340b446e`.
  Durable result:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-11/p6a-tooling-selftest-fixture-report.md`.
  This slice creates no product test, proof, spec-gap closure, CI enforcement,
  skill change, or runtime/product behavior change.

Acceptance:

- The verification system can fail when its own metadata lies.

## Phase 7 - Conformance Program Alignment

- [x] `VERIF-P7-001` Keep `conformance/contract.md` as public conformance
  contract.
- [x] `VERIF-P7-002` Link conformance cases to invariants. All 21 live cases
  now resolve through exact catalog discovery IDs to reviewed invariant and
  oracle associations; names, paths, source text, and expected JSON create no
  mapping, and the links make no passing-proof claim.
- [x] `VERIF-P7-003` Report conformance categories, case counts, expected
  artifacts, invariant coverage.
- [x] `VERIF-P7-004` Add gap rows for strings, arrays, structs, enums, nested
  values, OOP dispatch, references, retain matrix, scheduler, comms determinism.
- [x] `VERIF-P7-005` Comms determinism cases use simulated/loopback scripted
  status transitions, not live sockets.
- [x] `VERIF-P7-006` Generated conformance reports stay CI artifacts unless a
  public summary page is intentionally updated.

  The report-only audit at clean source commit
  `bb82eaf9cc3b6a2221e91ed1353bcd9fd88c6aa9` inventories 16 categories,
  21 manifests, 21 expected artifacts, 19 runtime cases, one compile-error
  case, one connector-status trace, and 20 program sources. All 21 cases now
  have explicit catalog/invariant mappings. The ten v2 gap rows now record
  linked mappings while retaining `not_assessed` semantic-oracle debt; the
  associations are not specification gaps or proof. The scripted
  communication case has eight in-process steps,
  no program source, and no live-socket dependency under the reviewed runner
  and connector-projection source closures. Generated reports remain CI
  artifacts under the digest-bound conformance job and public-page contract.
  Generated JSON SHA-256:
  `aa9c5d862cbafd8ef83ab4e54916a5221a34911e509606aa7dcfab9add625418`.
  Durable report:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-11/p7-conformance-alignment.md`.
  This audit executes no conformance case, creates no proof, closes no gap,
  changes no runtime or product behavior, and enables no CI enforcement.

## Phase 8 - Runtime Anomaly Testing Program

- [x] `VERIF-P8-001` Define fault-injection taxonomy: panic, timeout, deadline,
  watchdog, slow device, disconnect, queue full, stale data, corrupt retain,
  malformed bytecode, bad config, bad signal, partial web request, disk error,
  clock step, monotonic/wall-clock divergence, suspend/resume, timer duration
  overflow, and allocation failure/OOM.
- [x] `VERIF-P8-001A` Open spec-gap candidates for scan-cycle allocation policy
  and time base across restart kinds if no written contract exists.
- [x] `VERIF-P8-002` Map existing runtime-safety tests to taxonomy. The
  reviewed denominator partitions all 3,220 live Rust test facts into 133
  explicit taxonomy mappings and 3,087 explicit reviewed nonmappings. The
  partition is disjoint, exhaustive, identity-bound, and contains zero
  unreviewed facts.
- [x] `VERIF-P8-003` Add gap report for untested runtime anomaly classes.
- [x] `VERIF-P8-004` Classify anomaly classes as PR, nightly, release, or
  hardware_lab.
- [ ] `VERIF-P8-005` Add mutation/fault toggles only behind explicit test-only
  interfaces or harness layers. No general governed fault-toggle interface was
  added by this report-only slice.
- [ ] `VERIF-P8-006` Do not add production fault hooks without design review.
  This standing guard stays open because taxonomy policy is not source-level
  design-review enforcement.

  The report-only audit at clean source commit
  `bb82eaf9cc3b6a2221e91ed1353bcd9fd88c6aa9` defines the exact 19-class
  Phase 8 stimulus taxonomy and 133 explicit reviewed Rust-test associations:
  124 `direct`, seven `partial`, two `context_only`, and zero
  `protective_red`. Of those mappings, 123 are effectively runnable and one
  is ignored or conditional. The exhaustive 3,220-fact review records 3,087
  reviewed nonmappings: 1,298 outside runtime-safety scope, 719 with no
  taxonomy stimulus or response, 919 supporting internal contracts only, and
  151 in another safety domain. These associations are not invariant
  coverage, assessed oracles, test-catalog mappings, suite results, or proof.

  The report contains zero class-level test-gap rows under its explicit
  association basis. The exhaustive review establishes only the disposition
  of each Rust test fact; it does not establish semantic adequacy or proof.
  Primary planned tiers are nine `pr`,
  eight `nightly`, two `release`, and zero `hardware_lab`; conditional tier
  occurrences are eight `nightly`, fourteen `release`, and three
  `hardware_lab`. No suite command was wired or executed by the audit.

  `VERIF-P8-001A` completed its conditional review without inventing a gap:
  `SPEC_RUNTIME_ENGINE_001` already states both `dynamic allocation in hot
  path` is absent and `No heap allocation during execution`, so no duplicate
  scan-cycle allocation gap was opened. Restart/time-base behavior remains
  blocked by the existing open `SPEC_GAP_IEC_TIMER_RESTART_TIMEBASE_001`; it
  was neither duplicated nor closed. Generated JSON SHA-256:
  `4f8ded3df70ca63bfb9b078da7e25440665ca126e4e9dcec0d10851875bd180b`.
  Durable report:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-11/p8-runtime-anomaly-audit.md`.
  The audit executes no fault, adds no fault interface or production hook,
  changes no runtime, product, workflow, or CI behavior, creates no proof or
  coverage, and leaves `VERIF-P8-005` and `VERIF-P8-006` open.

## Phase 9 - Fuzz and Malformed Input Program

- [x] `VERIF-P9-001` Inventory fuzz targets and fuzz-like smoke tests. The
  exact bidirectional live join contains seventeen executable facts: eleven targets
  from two tracked cargo-fuzz manifests and six production Rust-scanner
  fuzz/property smokes. The three ADS targets are Phase 9 facts only; the
  historical Phase 2 scanner denominator is unchanged.
- [x] `VERIF-P9-002` Define required fuzz surfaces: ST lexer/parser,
  HIR/lowering input, PLCopen XML, bytecode container/instructions, protocol
  payloads, config files, LSP incremental edits, HMI schema payloads. Eighteen
  exact reviewed associations give every surface a direct cargo-fuzz target;
  associations create no oracle, invariant coverage, or proof.
- [x] `VERIF-P9-003` Classify each fuzz target as PR-smoke, nightly, or manual
  extended. Primary tiers are seven `pr_smoke`, one `nightly`, and nine
  `manual_extended`; only the two root cargo-fuzz targets also name `nightly`.
  Enforcement is seven `wired`, one `planned`, and nine `manual_only`.
  Required execution claims bind raw reviewed script/workflow bytes, script
  modes, unique workflow trigger/job blocks, effective Cargo default members,
  and `not_ignored` Rust facts. This row adds no CI or suite wiring and records
  no observed execution result.
- [x] `VERIF-P9-004` Define corpus storage rules. Both owning fuzz workspaces
  ignore exactly `artifacts/`, `corpus/`, `coverage/`, and `target/`; effective
  ignore behavior is checked and tracked generated corpus/crash paths are
  forbidden. Contents and counts remain deliberately unassessed machine-local
  discovery state, not durable evidence.
- [x] `VERIF-P9-005` Every minimized crash becomes deterministic regression.
  The committed registry and digest-bound 17-target campaign fail closed unless
  every observed crash artifact joins a mapped deterministic regression. The
  bounded campaign observed zero artifacts; this closes the handoff mechanism,
  not a universal crash-freedom claim.
- [x] `VERIF-P9-006` Add generated fuzz coverage/gap report. The report-only
  audit was generated from clean source commit
  `c25c62f87b6fe4d768c4ce47a416d1d464cff157`; JSON SHA-256 is
  `5bde7a684d85e4f660d8445436fa5e5151646210e8d10fc290fe57c9d7211de2`.
  Durable report:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-11/p9-fuzz-program-audit.md`.
  It records seventeen targets, eight surfaces, zero gaps, no campaign, no proof or
  invariant coverage, no spec-gap closure, no product/runtime behavior change,
  and no CI enforcement change.

## Phase 10 - Mutation, Coverage, and Test-the-Tests

- [x] `VERIF-P10-001` Define first mutation shards: bytecode validator, runtime
  value/type conversion, HIR diagnostics, parser recovery, retain/restart,
  connector status projection. The bytecode-validator pilot slice is pulled
  forward and satisfied by `VERIF-P1B-013`; the full row stays open until all
  other listed shards are defined.
- [x] `VERIF-P10-002` Coverage is adequacy signal, not release safety proof.
- [x] `VERIF-P10-003` Add mutation survivor report format.
- [x] `VERIF-P10-004` Safety-critical survivors require added test,
  unreachable/defensive-code rationale, or dead-code removal.
- [x] `VERIF-P10-005` Mutation/coverage runs use delivered-build confirmation
  where relevant.
- [x] `VERIF-P10-006` Keep first mutation gates focused.

  The report-only Phase 10 registry defines the six exact listed shards and
  seven single-file selectors, capped at two mutants per shard. The refreshed
  bytecode-validator pilot and four source-only shards are measured: six
  caught, zero survivors, zero unviable, zero timeouts, and zero errors. The
  source artifacts bind clean execution commit
  `56f68f2bbdb12c655f681668c5a5fddda4f4d659`; connector projection remains
  explicitly `planned` with an empty result array because no delivered-binary
  execution was performed. Selector and association binding alone is not
  execution evidence.

  The closed generic report derives outcomes from raw exit/timeout fields,
  rejects infrastructure errors, and requires every future measured survivor
  to have one resolved allowed action plus a durable tracked reference. It
  records zero coverage runs and null percentages. The connector-projection
  shard cannot become measured without a delivered artifact SHA-256 and direct
  execution confirmation. Association IDs are labels only, never killed-by or
  executed-test claims. Durable report:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-16/p10-mutation-survivor-report.md`;
  generated JSON SHA-256:
  `e893d67e1514c6bb5e117e2e57e5a391e01fb9cec4f21e5b829f90578fa3770c`.
  It binds clean report source commit
  `81f475234f199a5299f7442c8314e0b6f1d30696` and replaces the superseded
  2026-07-12 report.
  No proof, invariant promotion, spec-gap closure, product/runtime behavior,
  CI enforcement, workflow, skill, or agent-instruction change is made.

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

- [ ] `VERIF-P12-000` Inventory user stories and public workflows that imply a
  user can accomplish a task. Each workflow must have a spec source with actor,
  entry point, preconditions, visible steps, success state, failure/status
  behavior, safety/authz boundaries, and acceptance evidence, or a spec gap
  before related implementation starts.
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
- [ ] `VERIF-P12-006A` Report user stories or public workflows with
  implementation changes but no workflow spec source, linked invariant, or
  acceptance evidence.
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

## Phase 16 - Execution: Run the Program and Close Every Gap

This is the payoff phase: everything Phases 0-10 built gets used. It starts
after the Phase 10 slice closes and runs before or interleaved with Phases
11-15. Work in risk order (`safety_critical`, `wrong_result`,
`silent_corruption`, `false_status` first). Every row's done-condition is
measured from the committed registries, not from prose. Closing a
previously guarded open row here is expected once its work is real; update
the matching `REQUIRED_OPEN_ROWS`-style validator pins deliberately in the
same commit, with the closure evidence linked.

- [x] `VERIF-P16-000` Reconcile Phase 16 with the control-plane policy and
  current-HEAD reproductions. Record alleged defects that already pass as
  characterization work, keep product changes blocked, and narrow the first
  vertical to behavior that has a written oracle and a reproducible defect.
- [x] `VERIF-P16-000A` Make proof output producer-authentic and durable:
  `prove.py` writes directly to the tracked evidence index, refuses dirty or
  abbreviated proof revisions, and validation requires distinct red/green
  commits with the red commit an ancestor of green.
- [x] `VERIF-P16-000B` Add an honest hand-authored state-machine trace case
  provenance and artifact contract. It must be distinct from `gen_cases.py
  v1`, closed-schema, digest-bound, and consumable by `verification-cases` and
  `prove.py` without inventing timer outcomes.
- [x] `VERIF-P16-000C` Bind promotion levels to evidence strength: `G1` needs
  targeted green/lock evidence, `G2` additionally needs a broad remote gate,
  and `R1` additionally needs release/public evidence. Existing S0 records and
  report-only CI remain unchanged.

  Readiness implementation: `ebca97065` added durable proof output, clean
  revision ancestry, trace-case provenance, and the initial promotion contract;
  `e91c396b8`, `cb1cf2f7d`, and `be90cc29a` closed promotion causality,
  proof-metadata freezing, and case/artifact schema gaps; `551db78e4` and
  `5d3d0cbfb` wired the report-only product fence through the canonical gate,
  including root `hmi/**`. The bytecode-validator mutation shard was rerun at
  clean source `5d3d0cbfb` with 2 caught and zero survivors or infrastructure
  outcomes (report SHA-256 `61df3795ca2ca13ae239d1b63104417f46131bde988f2d93d841a4eda85a0fc7`).
  No product behavior, spec-gap status, invariant proof level, CI enforcement,
  skill, agent instruction, version, or release metadata changed. Independent
  review accepted the complete readiness implementation at `24f83f8d9` and
  found one report-only product-fence under-match. The closure adds
  `third_party/**`, root `Cargo.toml`, and root `Cargo.lock` with a tests-first
  regression fixture and records the acceptance in
  `EVID_P16_EXECUTION_READINESS_ACCEPTANCE_20260712`.
- [x] `VERIF-P16-000D` Record independent acceptance of the complete readiness
  implementation. Until this row is closed, the canonical report gate must
  surface every runtime/compiler/LSP/IDE/UI product path as blocked. Remove its
  standing-open validator pin and flip this row in the same reviewed commit;
  CI itself remains report-only until `VERIF-P16-007`.
- [x] `VERIF-P16-001` Pilot vertical: implement
  [execution-slice-001.md](execution-slice-001.md) end to end for the
  confirmed `IEC_TIMER_001` TOF post-expiry ET-hold defect: reviewed timer
  decisions, trace cases, durable red proof, minimal product fix, paired green,
  Phase 8 contract migration, broad gates, and honest promotion. The
  `VM_SEAM_DECLARED_TYPE_001` allegation currently passes and is retained as
  characterization, not manufactured into a product fix.

  The frozen lifecycle foundation and the complete E1 product vertical were
  independently accepted at reviewed checkpoint `053b0143`. The vertical
  binds the timer specification and decisions to hand-authored real-ST traces,
  producer-authentic red and green proof, the minimal TOF ET-hold correction,
  Phase 8 migration and gap closure, broad remote evidence, and honest G2
  promotion. `E1-PRE-005` is resolved by deferring retained function-block
  storage and restore semantics to separate reviewed scope; this slice asserts
  no restart outcome and does not declare current retained-instance behavior
  conformant. Acceptance is recorded in
  `EVID_P16_E1_INDEPENDENT_ACCEPTANCE_20260713`.
- [ ] `VERIF-P16-002` Close every spec gap. For each open record in
  `verification/spec-gaps.toml` (34 at time of writing): write the owning
  spec section, IEC decision, or recorded deviation per STOP-013, then flip
  `resolution_status = "closed"` with closeout evidence. Done when zero
  gaps are open.

  Reopened on 2026-07-18 after the eighteenth independent review found new
  product-contract gaps in watchdog partial-safe-state output handling,
  internally synthesized non-finite values, cross-file field rename,
  peer-topology status projection, commit-helper atomic scope, document-close
  cache invalidation, simulation-clock overflow, invalid LSP edit ranges, and
  OPC UA server write exposure. These records must be specified, tested, fixed,
  and closed before this row can return to complete.

  Bounded-value progress (2026-07-15):
  `SPEC_GAP_IEC_STRING_BINDING_BOUNDS_001` and
  `SPEC_GAP_VM_VALUE_SEMANTICS_001` are closed against updated normative
  product sources with mapped-test and durable closeout evidence. The exact
  implicit-conversion matrix, bounded string writes, ordinary subrange
  initializers, wrong-tag rejection, and no-partial-write behavior are now
  written. `SPEC_GAP_VM_ERROR_MODEL_001` remains open; this batch does not
  claim stable public VM error identifiers. The register now contains 18
  `open`, 4 `test_mapped`, and 12 `closed` records.

  Bytecode-validator progress (2026-07-15):
  `SPEC_GAP_BYTECODE_VALIDATOR_001` is closed against the normative
  validator-before-apply section in `docs/specs/12-bytecode.md`. A complete
  transform seed, seven generated product-path cases, 26 mapped test records,
  and 23 covered required malformed-input classes exercise the reviewed
  decoder/validator boundary. The clean two-mutant validator shard reports 2
  caught and zero survivors. No product acceptance defect reproduced, so this
  batch contains no manufactured red/fix pair. At that checkpoint the five
  numeric resource-limit classes and `SPEC_GAP_VM_ERROR_MODEL_001` remained
  open, with 17 `open`, 4 `test_mapped`, and 13 `closed` gap records.

  Fixed-resource-limit progress (2026-07-15):
  `SPEC_GAP_VM_DETERMINISM_RESOURCE_LIMITS_001` is closed against the fixed
  STBC version 1.x limits in `docs/specs/12-bytecode.md`. The cataloged
  nine-case product runner produced a genuine six-case red before
  `4bb98128`, then all nine cases passed with the same case and proof-contract
  digests. Stack, register, and tier-1 execution now charge the same original
  bytecode instructions across nested calls. The five resource-limit
  malformed-input classes are required and covered. The causal broad remote
  gate `EVID_BROAD_REMOTE_PR_20260715_81EA8F2854DB` promotes the invariant to
  G2; deadline/watchdog interaction and `SPEC_GAP_VM_ERROR_MODEL_001` remain
  explicit debt. The register now contains 16 `open`, 4 `test_mapped`, and 14
  `closed` records.

  Stable-error-model progress (2026-07-16):
  `docs/specs/10-runtime-semantics.md` and `docs/specs/12-bytecode.md` define
  the closed lower-snake-case bytecode, VM-trap, runtime-conversion, and HMI
  rejection identifiers. The product preserves those identities through
  `BytecodeError`, `VmTrap`, `RuntimeError`, runtime apply, and HMI admission,
  with nine exact-code tests mapped to the existing gap. A genuine seven-case
  red and green pair proves the bytecode product path. The gap is deliberately
  `spec_updated`, not `closed`: that proof's immutable contract was recorded
  while `VM_SEAM_VALID_001` and its catalog row still carried the open-gap
  oracle, so rewriting them after execution would invalidate the durable
  proof. No proof row or case digest was rewritten. Formal closeout remains
  part of `VERIF-P16-002` and requires a separately reviewed proof-contract
  migration or a new pre-bound execution contract.

  Runtime-control progress (2026-07-16):
  `SPEC_GAP_DEBUG_AUTHORIZATION_001`,
  `SPEC_GAP_CONTROL_AUTHORIZATION_MATRIX_001`,
  `SPEC_GAP_DEBUG_PAUSE_WATCHDOG_001`, and
  `SPEC_GAP_RUNTIME_FORCE_LIFECYCLE_001` are closed against the written
  runtime-engine and debug-adapter contracts. Their closeout rows bind the
  current case definitions and producer-authentic targeted evidence. The
  register now contains 15 `open`, 1 `spec_updated`, and 18 `closed` records;
  this progress does not claim that the remaining gaps are resolved.

  Protocol-truth progress (2026-07-16):
  `SPEC_GAP_PUBLIC_WIRE_CLAIM_001`,
  `SPEC_GAP_PROTOCOL_DISCOVERY_HANDSHAKE_001`,
  `SPEC_GAP_ETHERCAT_UNAVAILABLE_RESOURCE_001`, and
  `SPEC_GAP_PROTOCOL_STATUS_MODEL_001` are closed against the normative
  runtime-engine contract, public evidence-status table, three hand-authored
  trace runners, and durable closeout evidence. The register now contains 11
  `open`, 1 `spec_updated`, and 22 `closed` records. Physical device, broker,
  and EtherCAT topology evidence remains explicit invariant debt rather than a
  software-specification claim, so this row stays open.

  Editor/LSP progress (2026-07-16):
  `SPEC_GAP_EDITOR_DOCUMENT_CLOSE_001` is closed against the normative LSP
  durable-project-truth section, two mapped tests, historical red/green defect
  evidence, and a current-contract lock pair. The register now contains 11
  `open`, 1 `spec_updated`, and 23 `closed` records; this row stays open for
  the remaining gaps.

  VM seam proof progress (2026-07-17):
  `SPEC_GAP_VM_ERROR_MODEL_001` is formally closed without rewriting its
  historical red/green proof. The existing nine-test stable-error closeout
  remains source-revision evidence, while current invariant and catalog rows
  now point directly at the active bytecode, runtime, and value-semantics
  sources. `docs/specs/12-bytecode.md` also records the observed STBC 1.x
  unknown-optional-section rule, pinned by a product-apply regression. The
  register now contains 11 `open` and 24 `closed` records; this row remains
  open for the eleven unresolved specifications.

  Final workflow/release specification progress (2026-07-17): seven internal
  developer-workflow, connector-status, dependency-policy, artifact,
  conformance, and version gaps are closed against three new normative product
  sources, mapped case runners, and durable closeout evidence. Four public
  platform, behavior-lock, source-build, and hardware-claim gaps remain
  `spec_updated` behind explicit `release_claim` required-spec rows because
  local G1 proof is not published, native, or device evidence. The register
  now contains 31 `closed` and 4 `spec_updated` records; this row stays open
  for those four public-claim obligations.

  Public-claim closeout (2026-07-18): the platform, behavior-lock,
  source-build, and hardware wording boundaries are closed against
  `SPEC_RELEASE_EVIDENCE_001`, exact mapped tests, published release/CI
  records, the complete mapped-test execution, and the isolated source build.
  The platform claim remains tiered: native execution and native VSIX
  installation are not inferred from artifacts. The hardware closeout proves
  only the public evidence vocabulary and device-in-loop boundary; it
  qualifies no physical target. The register contains 35 `closed` records and
  zero unresolved gaps. Durable scope and URLs are recorded in
  `EVID_PLATFORM_PUBLIC_CLAIM_CLOSEOUT_20260718` and its three companion
  closeout rows.
- [x] `VERIF-P16-003` Map tests to every invariant. For each invariant:
  behavior rows get specified outcomes and resolving oracles; decision
  tables regenerate through gen_cases; other contract kinds get
  hand-authored cataloged tests (building the missing harness kinds as
  needed); everything is routed into suites. Done when no invariant has
  empty `tests` except those with an explicit blocked coverage cell naming
  what blocks them (hardware lab, UI acceptance).

  Bounded-value progress (2026-07-15): 14 scanner-bound catalog records map
  compiler, runtime integration, runtime-core, and VM policy tests to
  `IEC_SUBRANGE_001`, `VM_SEAM_DECLARED_TYPE_001`,
  `VM_SEAM_STRING_BOUND_001`, and `VM_SEAM_SUBRANGE_001`; the 19 existing
  STRING binding records are now linked from `IEC_STRING_001`. The affected
  Phase 4 seeds moved through the existing reviewed `execution_ready`
  lifecycle rather than bypassing seed controls.

  Bytecode-validator progress (2026-07-15): 26 mapped records now bind the
  reviewed validator, owner, reference, container, and product-apply checks to
  `VM_SEAM_VALID_001`, `VM_SEAM_OWNER_001`, and `VM_SEAM_REF_001`. The complete
  transform seed and all seven generated cases are runnable, and 11 Phase 11
  validator tests no longer carry `#[ignore]`. Stable public error identity
  remains explicit debt rather than an inferred oracle.

  Runtime-control progress (2026-07-16): four hand-authored trace runners now
  catalog authorization, pause/watchdog, force lifecycle, and scan-thread
  panic containment. They map `DEBUG_AUTH_001`, `DEBUG_PAUSE_001`,
  `SEC_AUTHZ_001`, `RT_SAFE_FORCE_001`, and `RT_SAFE_PANIC_001` to explicit
  written oracles and current-contract proof pairs. This is not an exhaustive
  mapping of the full 53-invariant denominator.

  Three-invariant behavior-lock progress (2026-07-16): the catalog now binds
  `TEST_IEC_PRECEDENCE_TRACE_001`, `TEST_PLCOPEN_IMPORT_TRACE_001`, and
  `TEST_OPCUA_CLIENT_LIFECYCLE_TRACE_001` to 21 committed cases for
  `IEC_PREC_001`, `PLCO_IMPORT_001`, and `PROTO_OPCUA_001`. The runners cover
  expression evaluation, PLCopen executable-body admission, and the OPC UA
  client lifecycle state machine against explicit written oracles. All three
  passed on unchanged product code. The full 53-invariant denominator is not
  mapped yet, and PLCopen real-vendor corpus evidence remains explicit debt,
  so this row stays open.

  Protocol-truth progress (2026-07-16): three cataloged case runners bind ADS
  and OPC UA status projection, Modbus/MQTT discovery confidence, and EtherCAT
  unavailable-resource behavior to six previously S0 protocol invariants.
  They add 12 reviewed cases without inferring physical-hardware proof. The
  full 53-invariant denominator remains open.

  Editor/LSP progress (2026-07-16): five hand-authored trace runners add 29
  committed cases for local and project rename safety, UTF-16 positions,
  diagnostic cancellation, and document-close durable-source behavior. In
  total, 23 catalog rows now map these five invariants, including the existing
  focused regressions. The full 54-invariant denominator remains open.

  VM seam proof progress (2026-07-17): six new case-backed runners and the
  existing bytecode-validator runner execute 38 committed cases for declared
  type materialization, STRING bounds, subranges, encoder fail-closed
  behavior, local-owner isolation, reference escape, and validator semantics.
  Three new case-table records pin the encoder/owner/reference generators, and
  a standalone product-apply regression pins unknown optional STBC sections.
  The full 54-invariant denominator remains open.

  Five-invariant proof progress (2026-07-17): five cataloged case runners add
  43 committed cases for parser recovery, STRING binding bounds, ordinary
  subrange diagnostics, delayed Modbus scan handoff, and restart storage
  transitions. The runners bind explicit written oracles and current-contract
  proof pairs. The full 54-invariant denominator remains open.

  Final workflow/release mapping progress (2026-07-17): fourteen cataloged
  runners add 19 committed cases for commit ownership, test discovery,
  connector status, platform and VSIX identity, behavior-lock wording,
  source-build policy, dependency exceptions, artifact provenance,
  conformance, version, and public-claim boundaries. Every one of the 54
  invariants now names at least one mapped test with an eligible oracle and a
  suite route; the zero-empty-test denominator closes this row.
- [x] `VERIF-P16-004` Red, fix, green. Run every mapped test. Every failure
  gets recorded red proof, a product fix routed through invariant
  discipline (changelog per release hygiene), and paired green proof via
  `prove.py`. The known backlog is in scope and each item ends fixed-green
  or explicitly re-scoped with rationale: STRING[n] bounds, subrange
  writes, NaN ingress, warm-restart time reset, scan-thread panic
  containment, `REF(returnvar)` escape, `emit_stmt` fail-open, rename
  soundness, UTF-16 positions, cancel-clears-diagnostics, didClose dirty
  buffer.

  Bounded-value progress (2026-07-15): tests-first probes found and fixed
  implicit typed integer-to-float rounding, missing ordinary-subrange
  initializer bounds, runtime-core rounding before policy validation,
  incompatible primitive-tag acceptance, and contextual subrange literal tag
  loss. Focused regression results and the exact product revision are recorded
  in the two 2026-07-15 closeout artifacts; producer-authentic proof remains
  open, so affected invariants stay at `S0`.

  Bytecode-validator progress (2026-07-15): all 93 focused Rust tests passed
  and both reviewed validator mutants were caught. No malformed input was
  accepted, so there is no product fix or fabricated red/green proof in this
  batch. The characterization and mutation results are durable closeout
  evidence; the three affected invariants remain at `S0` pending proof and
  broad-gate promotion work.

  Runtime-control progress (2026-07-16): the authorization trace produced a
  genuine five-case red because denied-role responses omitted the written
  `insufficient_role` code. The runtime fix centralizes reviewed operation
  classification and returns the stable code before dispatch; the paired
  green passes all eight cases with the same case and execution-contract
  digests. Pause/watchdog, force lifecycle, and panic containment passed
  current-contract baseline/compare runs, so no red or product fix was
  fabricated for those already-correct behaviors.

  Focused source-mutation progress (2026-07-16): runtime conversion, HIR
  subrange diagnostics, parser recovery, and retain/restart each caught their
  one viable reviewed mutant on `trust-builder`, with zero survivors,
  unviable outcomes, timeouts, or infrastructure errors in the accepted
  artifacts. The mutation tool's original conversion default-return selector
  did not compile because `Value` has no `Default`; that diagnostic attempt is
  retained and the shard was rerun with the viable `convert_value` identity
  comparison selector. No baseline product bug appeared, so no red proof or
  product fix was invented.

  Three-invariant behavior-lock progress (2026-07-16): all 21 new precedence,
  PLCopen import, and OPC UA lifecycle cases passed at both clean proof
  checkpoints with identical per-case results. `prove.py v1` therefore
  recorded lock-baseline/lock-compare pairs rather than manufacturing red
  evidence. No runtime, parser, or importer product fix was warranted by this
  batch.

  Protocol-truth progress (2026-07-16): the seven-case discovery trace found
  a genuine defect: an authentication-rejected MQTT CONNACK was overclassified
  as `confirmed`. `prove.py` recorded one failed case at `60e0394d`; the
  minimal runtime fix reports `likely` while preserving `auth_required`, the
  warning, clean-session CONNECT, and DISCONNECT, and the paired green passes
  all seven cases. Connector status and EtherCAT resource traces already
  matched the new written contract, so they use lock pairs rather than
  manufactured red evidence.

  Editor/LSP progress (2026-07-16): the document-close trace produced a
  genuine one-case red because the readable-file reload path retained
  semantic-token and pull-diagnostic caches from the discarded unsaved
  buffer. The two-line cache eviction fix passes all five cases at green. The
  rename, UTF-16 position, and cancellation traces already matched their
  written contracts, so they use current-contract lock pairs without
  manufactured failures.

  VM seam proof progress (2026-07-17): all 38 VM seam cases passed at clean
  baseline and comparison revisions, and the refreshed bytecode-validator
  mutation shard caught both reviewed mutants with zero survivors. No product
  acceptance defect reproduced, so this batch makes no runtime/product change
  and manufactures no red proof. Tests-first work instead found two fixture
  mistakes and two verification-tooling defects: generated case source
  provenance included mutable proof lifecycle, and `prove.py` could not emit a
  replacement lock pair without colliding with historical evidence IDs. Both
  tooling defects have focused regression tests.

  Five-invariant proof progress (2026-07-17): all 43 new parser, STRING,
  subrange, delayed-I/O, and restart-storage cases passed at clean baseline and
  descendant comparison revisions. No product defect reproduced, so this
  batch records lock pairs rather than manufacturing red evidence. Two case
  fixture mistakes and three runner integration errors were corrected before
  evidence generation; none changed product behavior.

  Final workflow/release execution progress (2026-07-17): tests-first focused
  work exposed and fixed three real defects before the clean proof baseline:
  `trust-dev commit` could absorb caller-owned staged paths, the VS Code status
  projection accepted noncanonical state/health strings, and the extension
  lockfile carried 13 npm advisories. The corrected 19 cases then produced 14
  clean `lock_baseline` and 14 descendant `lock_compare` records through
  `prove.py`. No historical red row was manufactured after the fixes, so this
  row remains open for the complete mapped-test denominator and any future
  observed failures.

  Complete mapped-test execution (2026-07-18): a clean detached
  `trust-builder` checkout executed all 242 mapped catalog commands
  independently. All 242 passed, with zero failures and zero timeouts. Because
  the complete mapped denominator produced no new red result, this closeout
  does not manufacture a product fix or red/green pair. Durable per-command
  exit status, duration, test ID, and retained-log digest are recorded in
  `p16-mapped-test-execution.json` (SHA-256
  `c70479523fca5640947d6446f497e3b5bdc67d4d78cb0dc7654a59834d8baee2`).
- [x] `VERIF-P16-005` Promote honestly. Every invariant reaches its
  evidence-supported maximum (`G1`/`G2`, `validated` where all applicable
  cells close). Done when zero invariants remain at `S0`.

  Runtime-control progress (2026-07-16): `DEBUG_AUTH_001`,
  `DEBUG_PAUSE_001`, `SEC_AUTHZ_001`, `RT_SAFE_FORCE_001`, and
  `RT_SAFE_PANIC_001` are `implemented` at G1 on current targeted proof or
  current behavior-lock evidence. The registry now contains 39 S0, 5 G1, and
  9 G2 invariants. No broad-gate result was retroactively used to manufacture
  G2 for this batch.

  Three-invariant behavior-lock progress (2026-07-16): `IEC_PREC_001`,
  `PLCO_IMPORT_001`, and `PROTO_OPCUA_001` are `implemented` at G1 on stable,
  producer-authentic lock pairs. The registry now contains 36 S0, 8 G1, and 9
  G2 invariants. The clean builder broad gates validate the batch but are not
  approved causal promotion evidence, and `PLCO_IMPORT_001` still names the
  real-vendor corpus gap; no G2 or validated status is claimed.

  Protocol-truth progress (2026-07-16): `PROTO_ADS_001`, `PROTO_MODBUS_001`,
  `PROTO_MQTT_001`, `PROTO_ETHERCAT_001`, `PROTO_DISCOVERY_TRUTH_001`, and
  `PROTO_STATUS_TRUTH_001` are `implemented` at G1 on current-oracle,
  producer-authentic lock comparisons. The registry now contains 30 S0, 14 G1,
  and 9 G2 invariants. Broad remote and physical interoperability evidence
  remains open; no G2 or validated status is claimed for this batch.

  Editor/LSP progress (2026-07-16): `EDIT_RENAME_001`, `EDIT_RENAME_002`,
  `EDIT_LSP_POS_001`, `EDIT_DIAG_CANCEL_001`, and `EDIT_DOC_CLOSE_001` are
  `implemented` at G1 on producer-authentic current-contract lock comparisons;
  the document-close defect also retains its historical red/green pair. The
  registry now contains 26 S0, 19 G1, and 9 G2 invariants. Causal broad proof
  remains open, so no G2 or validated status is claimed for this batch.

  VM seam proof progress (2026-07-17): `VM_SEAM_DECLARED_TYPE_001`,
  `VM_SEAM_ENC_001`, `VM_SEAM_OWNER_001`, `VM_SEAM_REF_001`,
  `VM_SEAM_STRING_BOUND_001`, `VM_SEAM_SUBRANGE_001`, and
  `VM_SEAM_VALID_001` are `implemented` at G1 on current producer-authentic
  lock comparisons. The generator provenance migration also replaced the
  current lock pairs for `IEC_PREC_001` and `PLCO_IMPORT_001` without deleting
  their source-revision history. The registry now contains 19 S0, 26 G1, and
  9 G2 invariants; causal broad and physical/corpus obligations remain open,
  so no G2 or validated status is claimed for this batch.

  Five-invariant proof progress (2026-07-17): `IEC_PARSE_RECOVER_001`,
  `IEC_STRING_001`, `IEC_SUBRANGE_001`, `RT_SAFE_IO_001`, and
  `RT_SAFE_RESTART_001` are `implemented` at G1 on producer-authentic
  current-contract lock comparisons. The registry now contains 14 S0, 31 G1,
  and 9 G2 invariants. Causal broad proof remains open for four invariants;
  `RT_SAFE_IO_001` also retains non-Modbus and hardware-lab latency debt, so no
  G2 or validated status is claimed.

  Final S0 promotion (2026-07-17): the remaining fourteen workflow, status,
  release, and platform invariants are `implemented` at G1 on their exact
  current-contract lock pairs. The registry now contains 0 S0, 45 G1, and 9
  G2 invariants. Native platform, extension-host, published-release,
  device-in-loop, and causal broad evidence remain explicit debt; none of the
  fourteen is promoted to G2 or `validated`.
- [x] `VERIF-P16-006` Close the audit ledgers: every ignored-test register
  entry resolved (fixed, quarantined with expiry, or retired with
  rationale); unmapped test debt mapped or retired; every incomplete fuzz
  surface gets a cargo-fuzz target and the crash-to-regression ledger closes
  `VERIF-P9-005`; conformance clause links close `VERIF-P7-002`; the
  anomaly denominator is exhaustively partitioned under `VERIF-P8-002`.

  Non-catalog ledger progress (2026-07-17): conformance is explicitly linked
  21/21 and `VERIF-P7-002` is closed; ignored-test unknown debt is zero; all
  eight fuzz surfaces have direct cargo-fuzz targets; and all 19 anomaly
  classes have at least one runnable direct test association. This row remains
  open for the deliberately deferred full test-catalog denominator. The
  runtime-safety review is complete at 133 mapped plus 3,087 reviewed
  nonmapping facts, exactly covering the 3,220-fact denominator.
  The governed seventeen-target fuzz campaign closed `VERIF-P9-005` on
  2026-07-18 with zero crash artifacts and a fail-closed crash-to-regression
  registry. The final test-catalog denominator review on 2026-07-18 partitions
  all 4,023 live scanner facts into 241 exact `generated_test` catalog mappings
  and 3,782 exact reviewed-nonmapping dispositions, with zero overlap,
  omission, stale identity, or unreviewed fact. The nonmapping population is
  retained in the raw report and does not claim an invariant, specification,
  oracle, expected result, or assertion adequacy. With the already-closed
  ignored, conformance, fuzz, crash-handoff, and runtime-anomaly ledgers, this
  completes the compound row. Clean-source report digest and independent
  closeout commands are recorded in
  `docs/internal/testing/evidence/plc-verification-program/2026-07-18/p16-test-catalog-denominator-closure.md`.
- [ ] `VERIF-P16-007` Flip enforcement. Wire the verification suites into
  CI as required gates (report-only posture ends); a red verification
  suite must block merge. Then complete Phase 15 skill sync so the
  workflow is mandated (`VERIF-STOP-012` closes here, not before).
- [ ] `VERIF-P16-008` Final closure report: zero open spec gaps, zero `S0`
  invariants, ledgers closed or explicitly scoped, CI enforcing, board
  complete. Byte-reproducible like the Phase 2-10 reports, reviewed like
  every slice.

## Review Acceptance

- [x] `VERIF-REVIEW-001` Fable review returned `clear-with-edits` (2026-07-08);
  verdict at
  `docs/internal/testing/evidence/plc-verification-program/2026-07-08/review-verdict.md`.
- [x] `VERIF-REVIEW-002` Every required edit is folded into this document set;
  fold verification with residual fixes at the evidence root
  (`fold-verification.md`).
- [x] `VERIF-REVIEW-003` Disputed recommendations: none. Decisions recorded in
  the verdict: public claims as spec-source records with
  `authority = "public_claim"`; `source_status` split from record `status`;
  coverage as `[[coverage.cells]]` with rationale; veryquick mapping deferred
  to `VERIF-P5-000A`.
- [x] `VERIF-REVIEW-004` Review folded and verified; Phase 1 implementation may
  start.
- [x] `VERIF-REVIEW-005` Spec-matrix final review returned clear for
  implementation; residual non-blocking edits are folded and recorded at
  `docs/internal/testing/evidence/plc-verification-program/2026-07-08/spec-matrix-final-review-verdict.md`.
- [x] `VERIF-REVIEW-006` Control-plane skeleton implementation review returned
  `clear-with-edits`; findings `CP-01` through `CP-17` are folded and recorded
  at
  `docs/internal/testing/evidence/plc-verification-program/2026-07-08/control-plane-review-fixes.md`.
