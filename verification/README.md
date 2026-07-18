# Verification Control Plane

Status: initial seed for the PLC verification program.

This directory holds machine-readable verification metadata. It does not move
or replace executable tests. Native tests stay in `crates/*/tests`,
`crates/*/src`, `editors/vscode/src/test`, `conformance`, `fuzz`, and gate
scripts.

Current seed scope:

- source-build/OpenOT public-build truth for GitHub issue #93,
- bytecode/VM specification sources,
- runtime safety specification sources,
- first public-docs claim scan with explicit gaps,
- exhaustive code-area planning and changed-file routing for `plan_tests.py`.
- bytecode/VM-only decision-table case generator pilot for `gen_cases.py`.
- validated hand-authored state-machine trace provenance for cases that cannot
  honestly use `gen_cases.py v1`.
- bytecode/VM-only transform seed artifacts under `verification/seeds/**`.
- bytecode-validator-only mutation shard metadata and survivor reporting for
  `VERIF-P1B-013`.
- mechanical existing-test discovery for `VERIF-P2-001` through
  `VERIF-P2-003`.
- hand-owned catalog intent, live path/name staleness checks, and explicit VS
  Code test registration auditing for `VERIF-P2-004` through `VERIF-P2-006`.
- report-only scanner-mapping and required-test-class completeness for
  `VERIF-P2-007`.
- report-only coverage-cell gaps, reviewed bytecode malformed-input classes,
  and exact unmapped-test identities for `VERIF-P2-008` through
  `VERIF-P2-010`.
- report-only existing-test refactor assessment and a deny-by-default reviewed
  proposal/redirect contract for `VERIF-P2A-001` through `VERIF-P2A-010`.
- exhaustive ignored-test discovery and reviewed classification for
  `VERIF-P3-001` through `VERIF-P3-005`.
- an exhaustive 44-obligation invariant seed manifest, five imported review
  risks, and report-only specification-completeness debt for Phase 4/4A.
- a live-bound inventory of existing workflow jobs and root gate scripts,
  mapped suite definitions, and 29 stable test-taxonomy routes for Phase 5.
- a report-only requirement/oracle audit for `VERIF-P6-001` through
  `VERIF-P6-006`, derived only from explicit invariant source and gap links.
- a closed verification-tooling bypass fixture contract for Phase 6A. It binds
  each known-bad case to its production catcher without claiming assertion
  strength or simulating the not-yet-implemented spec-source scanner.
- a report-only Phase 10 mutation program with six exact focused shards. The
  bytecode-validator pilot and four source-only shards are measured from bound
  artifacts; connector projection remains planned without fabricated
  delivered-binary execution.

TOML shape convention:

- invariants are one record per file under
  `verification/invariants/<area>/<INVARIANT_ID>.toml`;
- flat registries use plural wrapper arrays, for example `[[spec_sources]]`,
  `[[spec_gaps]]`, and `[[evidence]]`;
- specification sources use a closed schema-v2 locator contract.
  `locator_kind = "tracked_file"` requires a canonical, tracked,
  workspace-relative, non-symlink `path`; `locator_kind =
  "external_reference"` instead requires the external citation, expected
  ignored local path, retrieval expectation, publication date, and explicit
  non-redistribution and proof-blocking flags. External-reference records are
  non-oracle in v2; availability alone does not promote them, and their
  expected local bytes do not enter committed provenance;
- `SPEC_IEC_61131_3_ED3_EXTERNAL_001` records the unavailable IEC 61131-3
  Edition 3 authority with `oracle_eligible = false`,
  `redistributable = false`, and `absence_blocks_proof = true`. This metadata
  posture does not complete the spec-source scanner, exhaustive source
  classification, conflict scan, or public-claim inventory rows;
- `verification/invariant-seeds.toml` maps the 44 written seed obligations to
  canonical invariant records. Its three reviewed aliases are identity only;
  they create no test, proof, or gap closure.
- committed case tables live under `verification/cases/<area>/**` and are
  referenced from planned test-catalog rows by path plus SHA-256 digest.
  These rows pin planning artifacts only; they do not count as runnable proof.
- committed transform seeds live under `verification/seeds/<area>/**`. They are
  deterministic input artifacts for generators and must not be treated as
  executable proof by themselves.
- `verification/spec-matrix.toml` will define required specification tags per
  canonical area. A tag is satisfied by an active spec source whose `covers`
  list includes the tag, or by an open spec gap with an owner.
- `verification/matrix.toml` defines all 11 canonical areas plus the 29 stable
  code-area routes reviewed in `test-taxonomy.md`. The changed-file classifier
  rejects absolute, escaping, backslash, control-character, and noncanonical
  paths; unmatched paths remain default-denied. Deleted paths and both sides of
  a rename are retained by the report gate. Direct suite requirements do not
  expand suite `includes` or conditional suite tiers.
- `scripts/gen_cases.py --invariant <ID>` derives case records from invariant
  behavior rows. Rows with `spec_gap_ref` become blocked cases. Rows with
  `oracle_ref` copy expected outcomes from the behavior row; the tool never
  invents expected behavior. In the bytecode/VM pilot it can also derive
  blocked transform cases from committed seed artifacts.
- `scripts/gen_cases_v2.py --invariant <ID>` extends the same behavior-row
  derivation to non-bytecode decision tables without changing the frozen
  `gen_cases.py v1` digest that existing bytecode proof records bind. Its
  source digest uses the stable invariant execution-contract projection, so
  later proof-lifecycle bookkeeping does not invalidate unchanged case inputs.
- `crates/verification-cases` is the dev-helper crate for tests that consume
  committed case tables. It records blocked cases without execution, wraps
  runnable cases with `StateProbe` snapshots, and writes JSON artifacts under
  the workspace-root `target/gate-artifacts/cases/`. It requires the cataloged
  case-file digest before execution. Generated decision tables retain exact
  generator provenance; hand-authored state-machine cases carry closed ordered
  trace steps plus a canonical trace-definition digest. Artifacts copy the
  provenance kind and trace digest. The helper does not create durable evidence;
  `prove.py` owns that phase.
- Production `prove.py red|green|lock` refuses a dirty tree, records the clean
  full HEAD SHA, and appends producer-authentic proof directly to the tracked
  `verification/evidence-index.toml`. It checks the tree again after the
  cataloged command, binds artifact provenance, and requires a distinct
  descendant green commit from the paired red commit. All proof-producing rows
  use `proof_scope = "targeted"`.
- Invariant promotion is evidence-bound: `test_written` needs targeted red,
  `implemented`/`G1` needs targeted green or lock, `G2` also needs an approved
  successful broad remote gate whose current tests remain non-ignored and whose
  passing case summaries exactly cover each current committed case file, and
  `R1` also needs an approved release object. These rules do not promote any
  existing record or change report-only CI.
- `scripts/verification_report_gate.py` is the report-only CI front door during
  pilot burn-in. It runs the metadata/case-file gate, re-derives planner output
  for changed files, and reports uncataloged changed tests without enforcing
  outside the pilot.
- `scripts/verification/adversarial_selftest_tests.py` is the pilot's
  bypass-resistance fixture suite. It checks that shortcut attempts become
  failed validation, non-red proof, or report-only findings.
- `scripts/bytecode_validator_mutation.py` adapts cargo-mutants single-file
  candidates for validator fragments assembled with `include!()`. It runs in an
  isolated Git archive, cleans only `trust-runtime` outputs in a dedicated
  mutation target between mutants, and records caught/survived/unviable/timeout
  outcomes against committed case IDs. Those IDs are associations only: blocked
  cases are not executed and mutation evidence does not close a spec gap.
- Bytecode-validator mutation evidence carries `mutation_report_path` plus a
  SHA-256 digest. The metadata validator rechecks the machine JSON against the
  cataloged runner/tool version, mutant selectors and commands, raw process
  outcomes, exhaustive case-ID partition, outcome counts, survivors, and source
  commit. Known infrastructure failures abort instead of counting as killed or
  unviable mutants.
- `scripts/scan_test_catalog.py` discovers Rust integration and in-source test
  attributes, runnable Structured Text `TEST_PROGRAM` and
  `TEST_FUNCTION_BLOCK` declarations under crate tests, literal VS Code tests,
  conformance manifests, fuzz targets, root-level `scripts/*gate*` files, and
  GitHub workflow jobs. It writes the
  machine report to `target/gate-artifacts/verification/` and a concise dated
  Markdown summary. `scripts/validate_generated_test_catalog.py` revalidates
  the schema instance, semantic identities, per-source command contracts, input
  digest, counts, and Markdown binding at rest. Generated reference candidates
  are lexical facts only and do not map tests to invariants or claims.
- Phase 2's reviewed scan surface excludes `xtask/**` Rust tests and
  crate-local fuzz workspaces such as `crates/trust-ads-server/fuzz/**`. The
  exact live census is an evidence-refresh tripwire, not CI enforcement; scope
  expansion requires a later reviewed board row.
- Catalog schema v2 separates scanner-backed `generated_test` rows from the
  closed `case_table_artifact` and `mutation_shard_runner` exceptions. Run
  `scripts/check_test_catalog_staleness.py` to re-scan live sources and reject
  stale discovery ID/path/name/source-kind bindings. Run
  `scripts/check_vscode_test_registration.py` to reject unregistered, missing,
  duplicate, dynamic, conditional, or escaping extension-test registrations.
  The registration audit also requires every mechanically discovered VS Code
  test fact to live in a registered file. Run
  `scripts/report_test_class_completeness.py` and
  `scripts/validate_test_class_completeness_report.py` to report exact scanner
  mapping debt and mapped-area required classes. Planned rows remain visible but
  never count as runnable completeness, and neither do generated rows whose
  scanner facts are ignored or conditional. These commands are not wired into
  CI enforcement in this slice.
- Run `scripts/report_coverage_matrix_gaps.py` with
  `scripts/validate_coverage_matrix_gap_report.py` to list assigned and missing
  invariant/family slots without synthesizing states. Run
  `scripts/report_malformed_input_coverage.py` with
  `scripts/validate_malformed_input_coverage_report.py` for the reviewed
  bytecode/VM taxonomy, and `scripts/report_unmapped_test_debt.py` with
  `scripts/validate_unmapped_test_debt_report.py` for every unmapped scanner
  identity. These reports reject dirty or symlinked provenance and require
  canonical JSON plus exact Markdown, while nonzero debt still exits zero.
  Each generator requires a pristine source tree before it writes output.
  Reproduce multiple committed reports in separate clean worktrees, or restore
  the previous report's tracked and untracked outputs before starting the next
  generator; an output from report N intentionally makes the same tree dirty
  for report N+1.
- Run `scripts/report_test_refactor_assessment.py` with
  `scripts/validate_test_refactor_assessment_report.py` for the Phase 2A
  assessment. It reports inclusive 1,000-line signals, reviewed catalog
  mapping diversity, multi-invariant claims, whole-file and committed-case
  structural similarities, VS Code registration structure, and explicit
  duration classifications. Size and similarity are review signals only.
  Helper functions and helper-only files are not assessed for duplication,
  and catalog v2 has no authorized broad-test coverage-dimension field.
- `verification/test-refactor-proposals.toml` is hand-owned intent. Every
  record binds exact source identity, commands, invariants, explicit
  malformed-input dimensions, fixture ownership, stale-path updates, zero
  expected behavior delta, and a SOLID/KISS/DRY review. Mechanical findings
  never authorize a move or rename; unsupported change plans may remain only
  proposed or rejected. The v1 single-identity model rejects
  `split` rather than pretending to model multiple targets.
- `verification/test-catalog-redirects.toml` records only validated historical
  move/rename edges. V1 permits one edge per test and rejects a second edge
  until `prove.py` can produce proposal-scoped lock IDs. Completed changes
  require paired, passing
  `lock_baseline`/`lock_compare` evidence bound to the catalog case file. Tests
  without `case_file` plus `case_file_digest` remain blocked from refactor
  completion. `scripts/validate_test_refactor_proposals.py` and
  `scripts/check_test_catalog_staleness.py` both recompute the live assessment;
  neither trusts a caller-supplied report. The primary metadata validator does not run the source scan required for proposal and redirect validation.
  The standalone live commands and all Phase 2A report generation and at-rest
  paths do run that contract. No Phase 2A command is wired into CI enforcement
  in this slice.
- Run `scripts/report_ignored_test_inventory.py` to generate the Phase 3
  mechanical inventory, and validate it with
  `scripts/validate_ignored_test_inventory_report.py`. The report recognizes
  Rust ignore attributes, a uniquely enclosed VS Code `this.skip()`, and
  literal tracked Playwright skips. Unsupported ignored markers on excluded
  Rust and Node test surfaces fail visibly instead of disappearing. Shell and
  conformance limitations are explicit, and the report binds canonical JSON,
  exact Markdown, clean full-SHA provenance, source inputs, the registry, and
  both schemas.
- `verification/ignored-tests.toml` is the hand-reviewed classification plane.
  Run `scripts/check_ignored_test_staleness.py` for the exhaustive one-to-one
  live join. Unknown classifications are reported with exit zero until
  `VERIF-P14-000` defines a grace rule; malformed, duplicate, missing, stale,
  escaping, or class-incomplete records fail immediately. The primary metadata
  validator checks the static schema and class obligations but does not perform
  the source scan. These Phase 3 commands are not wired into CI enforcement.
- Run `scripts/report_invariant_seed_audit.py` and
  `scripts/validate_invariant_seed_audit_report.py` for the Phase 4 exhaustive
  seed/import join. It requires 44 written seeds, 43 canonical invariants, five
  review-risk links, durable non-public oracle sources for `gap_open`, and open
  focused gaps for `spec_gap`. Lifecycle-v1 baseline records retain the
  imported S0 posture. Only `IEC_TIMER_001` is reviewed as `execution_ready`:
  it may acquire known, back-linked tests/evidence and an evidence-supported
  proof level during the Phase 16 pilot. A terminal `validated` state is
  accepted only after full metadata health, specified source ownership, zero
  current gaps/missing obligations, closed coverage cells, and supported
  promotion evidence. Earlier states cannot hide a current gap. The lifecycle
  transition is authorization, not proof.
- Run `scripts/report_spec_completeness.py` and
  `scripts/validate_spec_completeness_report.py` for Phase 4A debt: invariants
  whose specification is not `specified`, expected-result tests without an
  oracle/gap binding, `spec_gap` coverage cells, and the bytecode pilot's
  explicit spec-gap/test-gap partition. Registered public claims are shown as
  non-exhaustive context; `VERIF-P4A-005` remains open until broad claim-source
  inventory exists. Neither report is wired into CI enforcement.
- `verification/gate-inventory.toml` binds every executable root workflow job
  and root `scripts/*gate*` fact to its live discovery identity, command,
  owner, duration, environment, artifact posture, enforcement posture, and a
  direct suite or explicit exclusion. The nested consumer workflow template is
  a non-executable exclusion. Suite records reference every assigned inventory
  row through `inventory_ids`; `command_bindings` must name every directly
  assigned `command_role = "entrypoint"` row and no helper/reference row, and
  `commands` must be their exact ordered projection.
- `veryquick` maps to `just verification-veryquick` on `trust-builder`. The
  bounded recipe runs the canonical verification Python suite, metadata/case
  consistency, existing HIR/runtime recipes, syntax and runtime-core tests,
  bytecode validation, and one deterministic conformance case. It is not a new
  broad local Raspberry Pi gate.
- Run `python3 scripts/run_verification_focused_tests.py` for the canonical
  verification-tooling suite. Discovery is recursive and deterministic, so the
  Phase 3 lexical/source-geometry regressions and future `*_tests.py` modules
  cannot be omitted by a hand-maintained command. The production module
  `metadata_validator/ignored_tests.py` is explicitly excluded.
- Run `scripts/report_requirement_oracle_audit.py` with
  `scripts/validate_requirement_oracle_audit_report.py` for the Phase 6 audit.
  The report covers all committed invariants, distinguishes active eligible
  oracle sources from open-gap placeholders, and lists future high-risk
  enforcement candidates without enforcing them. Public claims remain
  non-oracle context. The report does not close `VERIF-P6-007` through
  `VERIF-P6-010`.
- Run `python3 scripts/report_spec_source_audit.py` with
  `python3 scripts/validate_spec_source_audit_report.py` for the Phase 1A
  tracked-document and rendered-public-prose census. The audit follows tracked
  public snippet includes, binds only exact reviewed source/claim metadata,
  excludes the mutable evidence plane except for two explicitly registered
  evidence-backed sources, and reports unreviewed documents/prose without
  inferring authority or proof. Its ignored external-standard path is metadata
  only and is never read or included in report provenance.
- Run `python3 scripts/check_verification_tooling_selftests.py` for the Phase 6A
  known-good/known-bad fixture contract. Metadata fixtures exercise their
  assigned validator phase directly and then the full `Validator.validate()`
  wiring; catalog staleness, case artifacts, proof classification, and planner
  findings stay assigned to their owning production layers. Registered public
  claims require a proof-backed validated invariant or an explicit open gap;
  this is not semantic review of the exhaustive public-prose denominator still
  blocked by `VERIF-P4A-005`. Six spec-source fixtures now exercise production
  discovery and association analysis; unreviewed prose remains a report-only
  boundary, and `VERIF-P1A-003`/`VERIF-P1A-006` remain open.
- Run `python3 scripts/report_conformance_alignment.py` with
  `python3 scripts/validate_conformance_alignment_report.py` for the Phase 7
  report-only conformance audit. It binds the public suite contract, all 16
  categories, 21 manifests, 21 expected artifacts, the scripted in-process
  communication case, and CI-artifact publication posture. Invariant coverage
  comes only from an exact hand-owned catalog `discovery_id` join. The current
  result is 0/21 linked cases, so `VERIF-P7-002` remains open and the ten v2
  categories are explicit alignment-gap rows with semantic oracles
  `not_assessed`. Expected artifacts are inputs, not proof that their behavior
  is specified or correct.
- Run `python3 scripts/report_runtime_anomaly_audit.py` with
  `python3 scripts/validate_runtime_anomaly_audit_report.py` for the Phase 8
  report-only anomaly audit. The closed taxonomy contains 19 stimulus classes
  and exact reviewed Rust scanner associations. The companion denominator
  ledger partitions all 3,220 live Rust facts into exact mappings or closed
  reviewed-nonmapping rationales. Only non-ignored `direct` associations count
  as effectively runnable. Suite tiers are planned routes only. The audit
  executes no fault, creates no proof or invariant coverage, and leaves
  `VERIF-P8-005` and `VERIF-P8-006` open.
- `verification/fuzz-program.toml` owns the Phase 9 hand-reviewed fuzz
  association plane. It binds five parsed cargo-fuzz targets and six exact Rust
  fuzz/property smokes to the ordered required surfaces
  `st_lexer_parser`, `hir_lowering_input`, `plcopen_xml`,
  `bytecode_container_instructions`, `protocol_payloads`, `config_files`,
  `lsp_incremental_edits`, and `hmi_schema_payloads`. The five cargo targets
  include the three crate-local ADS targets deliberately excluded from the
  Phase 2 scanner; that historical scanner denominator is not broadened.
- Run the Phase 9 report-only audit with:

  ```text
  python3 scripts/report_fuzz_program_audit.py \
    --json-out target/gate-artifacts/verification/fuzz-program-audit.json \
    --markdown-out target/gate-artifacts/verification/fuzz-program-audit.md \
    --timestamp <fixed-iso-8601-time>
  python3 scripts/validate_fuzz_program_audit_report.py \
    --json target/gate-artifacts/verification/fuzz-program-audit.json \
    --markdown target/gate-artifacts/verification/fuzz-program-audit.md
  ```

  Every target has one primary tier from `pr_smoke`, `nightly`, or
  `manual_extended`; `additional_tiers` records only another real execution
  profile and does not imply suite inheritance or enforcement. Surface states
  distinguish direct cargo-fuzz targets, bounded-smoke-only associations,
  partial associations, and unmapped surfaces. They are target-presence debt,
  not an oracle, invariant coverage, a passing campaign, or proof.
  Execution claims are bound to the raw reviewed gate-script and workflow
  bytes, script executable modes, exact unique workflow trigger/job blocks, and
  effective Cargo default workspace membership. Matching command text in
  comments, normalized line endings, dead shell control flow, or another
  workflow block cannot satisfy those bindings.
  Each bounded Rust smoke must also resolve to a live `not_ignored` scanner
  fact; `ignored` or `conditional` facts cannot retain a wired or planned
  runnable tier.
  The Rust census remains lexical: ordinary `cfg` evaluation and parent-module
  reachability are not compiler-proven by this audit. `wired` therefore means a
  reviewed required command path, not observed execution or passing evidence.
- Fuzz working corpora, generated seed inputs, coverage output, and raw crash
  artifacts remain under ignored workspace-local paths. Named CI uploads are
  still run artifacts unless separately bound as durable evidence. The audit
  checks storage and ignore posture but does not inspect or claim completeness
  for corpus contents. `verification/fuzz-crash-regressions.toml` is the closed
  crash ledger. The digest-bound bounded campaign fails unless every observed
  artifact joins that ledger and a mapped deterministic regression. A clean
  bounded campaign closes the handoff mechanism, not universal crash freedom.
- `verification/mutation-program.toml` owns the six Phase 10 shard definitions
  and seven selected mutants. Each mutant binds one tracked source digest, one
  exact cargo-mutants selector, focused build/test commands, invariants, and
  explicit case or scanner association IDs. Those IDs are traceability labels,
  not claims that a named test killed a mutant. Shards are capped at two
  mutants; broad workspace and `test-all` commands fail validation.
- The four reviewed source-only shards use one fail-closed executor and one
  closed artifact format. Run each shard from a clean committed checkout and
  write only to the exact `result_artifact_path` reserved by its manifest row:

  ```text
  python3 scripts/run_focused_mutation_shard.py \
    --shard-id MUTATION_SHARD_RUNTIME_VALUE_CONVERSION_001 \
    --json-out docs/internal/testing/evidence/plc-verification-program/2026-07-16/p10-runtime-value-conversion-mutation.json \
    --target-dir /absolute/generated/cargo-target
  ```

  The executor archives `HEAD`, cleans only the selected package, requires the
  focused baseline to pass, applies exactly one reviewed cargo-mutants
  selector, and runs only the bound focused test. Signals, disk/quota failures,
  unknown nonzero exits, and selected-test ambiguity are infrastructure errors,
  not adequacy results. Artifacts retain raw process output and are revalidated
  against an input-identical source commit. The connector delivered-binary
  shard is deliberately rejected by this source runner.
- Generate and validate the durable Phase 10 report with:

  ```text
  python3 scripts/report_mutation_program.py \
    --json-out docs/internal/testing/evidence/plc-verification-program/<date>/p10-mutation-survivor-report.json \
    --markdown-out docs/internal/testing/evidence/plc-verification-program/<date>/p10-mutation-survivor-report.md \
    --timestamp <fixed-iso-8601-time>
  python3 scripts/validate_mutation_program_report.py \
    --json docs/internal/testing/evidence/plc-verification-program/<date>/p10-mutation-survivor-report.json \
    --markdown docs/internal/testing/evidence/plc-verification-program/<date>/p10-mutation-survivor-report.md
  ```

  The generic report derives outcomes from raw process fields and rejects
  infrastructure failures. Every measured survivor must have exactly one
  resolved allowed action and a durable tracked reference. Coverage remains
  zero-run/no-percentage until a real campaign exists. A future measured
  connector-projection shard must bind a delivered binary SHA-256 and direct
  execution confirmation. None of these records create proof, invariant
  coverage, spec-gap closure, release evidence, or CI enforcement.
- Hardware-lab commands require the exact structured opt-in
  `TRUST_DIT_REQUIRE_HARDWARE=1`; only the strict script is an entrypoint, while
  the hosted or scheduled skip-capable workflow remains an inventory helper and
  is not lab proof. Release entrypoints must name a CI artifact, bound CI job
  result, committed/lab report, or release object and cannot use `target/**` as
  durable evidence.
  Phase 11 remains bound to the existing device-in-loop workflow, script, Rust
  harness, and JSON artifact contract. Suite `includes`/`excludes` are
  identifiers only until `VERIF-P14-000B` defines composition semantics.

Committed report source revisions need not equal the current repository HEAD
when their at-rest validators prove that the complete bound input closure is
unchanged. A report is regenerated when a bound input changes, not merely
because unrelated commits advanced HEAD.

Do not mark records `validated` until the validators, suite definitions, and
evidence index checks described in
`docs/internal/testing/checklists/plc-verification-program/metadata-model.md`
exist and pass.
