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
- [ ] `VERIF-P1A-002` Add a source scanner/report for likely spec documents:
  `docs/specs/**`, `docs/internal/**`, top-level README, public docs,
  conformance docs, release docs, and protocol/lab notes.
- [ ] `VERIF-P1A-003` Classify each source by area, authority level, owner,
  freshness, public/internal visibility, and oracle usability.
- [ ] `VERIF-P1A-004` Emit obvious-missing-spec report by area:
  bytecode format, bytecode validator, VM value semantics, scan-cycle lifecycle,
  stop/safe-state, retain/restart, protocol status/discovery, HMI API/UI,
  source transformations, LSP sync/positions/cancellation, debug/DAP
  force-write-release lifecycle, control/RBAC/security, PLCopen import/export,
  test-harness/simulation semantics, runtime/project/HMI config schemas, CLI and
  control-socket surfaces, GPIO, runtime performance budgets, supply chain,
  platform/package behavior, and release proof.
- [x] `VERIF-P1A-005` Emit public-claim report: public/user-facing docs claims
  with no normative source, no invariant, or no proof path.
  Initial report:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-08/public-docs-truth-scan-initial.md`.
- [ ] `VERIF-P1A-006` Emit conflict/staleness report: docs that disagree, specs
  that reference removed behavior, stale checklist rows, duplicate decisions.
- [ ] `VERIF-P1A-007` For external standards that cannot be committed, record
  local path, retrieval expectation, version/date, and whether absence blocks
  proof.
- [ ] `VERIF-P1A-008` For every missing or ambiguous bytecode/VM pilot source,
  create a `spec_gap` row before cataloging tests as proof for that behavior.
- [ ] `VERIF-P1A-009` Do not mark Phase 2 catalog entries as proof-mapped unless
  their invariant can point to a spec source or spec gap.
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
  in inventoried `bytecode_vm`; exact error-code stability remains under open
  `SPEC_GAP_VM_ERROR_MODEL_001`, and no current suite is falsely assigned.
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
  commit `d8dc5728828b43a9bf7321fc89e2efcb4b3fbd54` classifies 1/3,816 scanner facts
  and reports 3,815 as debt. The sole mapped `bytecode_vm` area has two of five
  required class slots complete (`mutation` and `negative_malformed_input`);
  `failing_regression`, `iec_conformance`, and `metadata_validation` remain
  missing. Four planned case-table rows are visible under
  `metadata_validation` but do not count, and ignored/conditional generated
  facts cannot count as effectively runnable. The at-rest validator recomputes
  live scanner/catalog/matrix joins, full metadata validity, tool/schema input
  digests, canonical command/time shape, clean source-commit inputs, and the
  Markdown-to-JSON digest. Debt exits successfully and no CI enforcement was
  added. Durable report:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2-test-class-completeness.md`.
- [x] `VERIF-P2-008` Add coverage-matrix gap report with states:
  `covered`, `covered_by_fuzz`, `not_applicable`, `blocked`, `spec_gap`,
  `gap_open`, `deferred`.
  Clean-source report commit `d8dc5728828b43a9bf7321fc89e2efcb4b3fbd54`
  assesses one mapped area and eight bytecode/VM invariants: 16 of 80 required
  invariant/family slots have declared cells, 64 remain structurally
  unassigned, and one additional recorded dimension remains visible. All 17
  declared cells stay `spec_gap`. Four catalog-bound case files contribute 21
  blocked observations without upgrading any state. Missing cells receive no
  synthetic state.
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
  `d8dc5728828b43a9bf7321fc89e2efcb4b3fbd54` inventories 3,816 facts in 670
  files, 24 inclusive-threshold large-file candidates, zero reviewed mixed-
  purpose or broad-claim candidates, zero exact or normalized fact-file
  duplicate groups, six same-table structural case peer groups, one shared
  case-file reference group, and zero malformed-class overlap groups. It joins
  all 456 VS Code facts to 38 registrations and records only one reviewed
  scanner duration plus five artifact durations. Generated JSON SHA-256:
  `5127e0c590f7925ae44e2bfa20a3ff78fa51da546ed8ff887f4805f5196852c9`.

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
  `unknown` after the grace period defined per `VERIF-P14-000`.

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
  Bytecode/VM pilot requirements are pulled forward by `VERIF-P1B-001`; the
  full row closes only when every area in `test-taxonomy.md` is represented.
- [ ] `VERIF-P5-010` Add changed-file classifier. Bytecode/VM pilot
  classification is pulled forward by `VERIF-P1B-002`; the full row closes only
  when every code area has default-deny routing.

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
  `wrong_result`, `silent_corruption`, and `false_status` risks after the grace
  period defined per `VERIF-P14-000`.
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
- [ ] `VERIF-P6A-009` Add known-bad fixtures for the spec-first planner layer:
  decision-table invariant missing behavior rows, case file with unknown family,
  stale case digest, case artifact that skips a case, high-risk red/green
  evidence from a non-allowlisted producer, green proof missing its paired red
  evidence, risk downgrade without `decision_ref`, and compile-error-as-red.
- [ ] `VERIF-P6A-010` Add self-tests that assert each bypass is caught by its
  assigned layer. Do not claim the validator catches assertion strength; that is
  handled by red proof and mutation shards where available.

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
  connector status projection. The bytecode-validator pilot slice is pulled
  forward and satisfied by `VERIF-P1B-013`; the full row stays open until all
  other listed shards are defined.
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
