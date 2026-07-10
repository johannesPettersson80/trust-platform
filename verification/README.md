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
- bytecode/VM-only planning matrix pilot for `plan_tests.py`.
- bytecode/VM-only decision-table case generator pilot for `gen_cases.py`.
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

TOML shape convention:

- invariants are one record per file under
  `verification/invariants/<area>/<INVARIANT_ID>.toml`;
- flat registries use plural wrapper arrays, for example `[[spec_sources]]`,
  `[[spec_gaps]]`, and `[[evidence]]`;
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
- `verification/matrix.toml` defines planning globs, risk defaults, required
  test classes, and case families. In Phase 1B it is intentionally scoped to
  `bytecode_vm`; every other area remains uninventoried and must fail closed.
- `scripts/gen_cases.py --invariant <ID>` derives case records from invariant
  behavior rows. Rows with `spec_gap_ref` become blocked cases. Rows with
  `oracle_ref` copy expected outcomes from the behavior row; the tool never
  invents expected behavior. In the bytecode/VM pilot it can also derive
  blocked transform cases from committed seed artifacts.
- `crates/verification-cases` is the dev-helper crate for tests that consume
  committed case tables. It records blocked cases without execution, wraps
  runnable cases with `StateProbe` snapshots, and writes JSON artifacts under
  the workspace-root `target/gate-artifacts/cases/`. It requires the cataloged
  case-file digest before execution. It does not create durable evidence;
  `prove.py` owns that later phase.
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
  focused gaps for `spec_gap`. Every seed remains S0 and unvalidated.
- Run `scripts/report_spec_completeness.py` and
  `scripts/validate_spec_completeness_report.py` for Phase 4A debt: invariants
  whose specification is not `specified`, expected-result tests without an
  oracle/gap binding, `spec_gap` coverage cells, and the bytecode pilot's
  explicit spec-gap/test-gap partition. Registered public claims are shown as
  non-exhaustive context; `VERIF-P4A-005` remains open until broad claim-source
  inventory exists. Neither report is wired into CI enforcement.

Do not mark records `validated` until the validators, suite definitions, and
evidence index checks described in
`docs/internal/testing/checklists/plc-verification-program/metadata-model.md`
exist and pass.
