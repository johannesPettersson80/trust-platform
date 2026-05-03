# CODESYS/TwinCAT Parity Checklist

Status: Implemented and validated; one milestone-only evidence item remains intentionally unchecked (`B-VAL-002`).
Branch: `feature/codesys-twincat-parity`
Goal:
- Native CODESYS/TwinCAT-style authoring in truST core language/runtime.
- Strict IEC reshaping remains available as an optional adapter/export path.

Truth policy:
- Mark `[x]` only when implementation, validation, and docs are all complete.
- Leave `[ ]` if any one of those three is missing.
- Link concrete evidence before flipping any item to `[x]`.

Implementation policy:
- Tests first for every behavior change.
- Record ambiguities in `docs/IEC_DECISIONS.md`.
- Record vendor-parity behavior in `docs/IEC_DEVIATIONS.md`.
- Keep runtime/build changes scoped and minimal; do not broaden normal runtime execution semantics to satisfy test harness behavior.

Out of scope for this checklist:
- Enforcing `{attribute 'qualified_only'}` semantics.
- Rewriting the entire PLCopen importer in one pass.
- Any change that requires existing motion-library work to stop.

## 0. Governance and Staging

- [x] GOV-001 Top-level IEC logs are the single source of truth.
  Evidence: `docs/IEC_DECISIONS.md`, `docs/IEC_DEVIATIONS.md`, `scripts/check_iec_log_paths.py`, validated with `python3 scripts/check_iec_log_paths.py`.
  Target files: `docs/IEC_DECISIONS.md`, `docs/IEC_DEVIATIONS.md`, `.github/workflows/ci.yml`, `scripts/prepush_ci_gate.sh`, `scripts/check_iec_log_paths.py`.
- [x] GOV-002 This checklist is the implementation contract for parity work.
  Evidence: `docs/internal/testing/checklists/codesys-twincat-parity.md`.
  Target file: `docs/internal/testing/checklists/codesys-twincat-parity.md`.
- [x] GOV-003 Changelog and release hygiene are applied for user-visible behavior changes.
  Evidence: `CHANGELOG.md`, `Cargo.toml`, `editors/vscode/package.json`, `editors/vscode/package-lock.json`.
  Target files: `CHANGELOG.md`, `Cargo.toml`.
- [x] GOV-004 Staged cadence is followed: targeted tests during implementation, full gates at milestone boundaries, final gates before completion.
  Evidence:
  - Targeted suites were run continuously during implementation for `trust-syntax`, `trust-hir`, `trust-runtime`, `trust-ide`, and PLCopen import paths.
  - Final gates passed: `just fmt`, `just clippy`, `just test`, `just test-all`.

## 1. Milestone A: Foundations and Harness

### 1.1 Canonical IEC Logs

- [x] A-IEC-001 Red test/guard added for mixed IEC log paths.
  Evidence: `scripts/check_iec_log_paths.py`, `.github/workflows/ci.yml`, `scripts/prepush_ci_gate.sh`, validated with `python3 scripts/check_iec_log_paths.py`.
  Tests/files: `scripts/check_iec_log_paths.py`, `.github/workflows/ci.yml`, `scripts/prepush_ci_gate.sh`.
- [x] A-IEC-002 Canonical files selected as `docs/IEC_DECISIONS.md` and `docs/IEC_DEVIATIONS.md`.
  Evidence: `docs/IEC_DECISIONS.md`, `docs/IEC_DEVIATIONS.md`, `AGENTS.md`.
- [x] A-IEC-003 Internal duplicates removed or replaced with redirect stubs.
  Evidence: No tracked duplicate IEC decision/deviation log files remain in this branch; enforced by `scripts/check_iec_log_paths.py`.
- [x] A-IEC-004 All repo references updated to the canonical files.
  Evidence: `docs/guides/SIEMENS_SCL_COMPATIBILITY.md`, validated with `python3 scripts/check_iec_log_paths.py`.

### 1.2 `TEST_PROGRAM` + `CONFIGURATION`

- [x] A-TEST-001 Red regression proves `trust-runtime test` discovers and executes `TEST_PROGRAM` with `CONFIGURATION` present.
  Evidence: `crates/trust-runtime/tests/st_test_cli_command.rs`, `crates/trust-runtime/src/bin/trust-runtime/test_cmd/tests.rs`, validated with `cargo test -p trust-runtime --bin trust-runtime run_test_executes_test_program_when_configuration_is_present -- --nocapture`.
  Tests/files: `crates/trust-runtime/tests/st_test_cli_command.rs`, `crates/trust-runtime/src/bin/trust-runtime/test_cmd/tests.rs`.
- [x] A-TEST-002 Fix is scoped to test mode only.
  Evidence: `crates/trust-runtime/src/bin/trust-runtime/test_cmd/command.rs`, `crates/trust-runtime/src/harness/api.rs`, `crates/trust-runtime/src/harness/build.rs`.
  Target files: `crates/trust-runtime/src/bin/trust-runtime/test_cmd/command.rs`, `crates/trust-runtime/src/harness/api.rs`, `crates/trust-runtime/src/harness/build.rs`.
- [x] A-TEST-003 Normal `build/run` semantics remain IEC-correct: configured runtime still executes only configured programs.
  Evidence: `crates/trust-runtime/src/bin/trust-runtime/test_cmd/tests.rs` (`execute_test_case_keeps_unconfigured_test_program_out_of_default_runtime`), plus test-mode-only extra registration in `crates/trust-runtime/src/bin/trust-runtime/test_cmd/command.rs`.

### 1.3 Motion Checklist Sync

- [x] A-MOTION-001 Motion checklist text no longer claims `CONFIGURATION` blocks tests once A-TEST-002 is complete.
  Evidence: motion branch file `docs/internal/testing/checklists/plcopen-motion-library-implementation-checklist.md`.
  Target file: `docs/internal/testing/checklists/plcopen-motion-library-implementation-checklist.md`.

### 1.4 Milestone A Validation

- [x] A-VAL-001 Targeted suites pass.
  Evidence:
  - `cargo test -p trust-runtime --bin trust-runtime execute_test_case_keeps_unconfigured_test_program_out_of_default_runtime -- --nocapture`
  - `cargo test -p trust-runtime --bin trust-runtime execute_test_case_runs_test_program_when_session_registers_extra_program_instance -- --nocapture`
  - `cargo test -p trust-runtime --bin trust-runtime run_test_executes_test_program_when_configuration_is_present -- --nocapture`
  - `cargo test -p trust-runtime --test st_test_cli_command test_program_runs_when_configuration_is_present -- --nocapture`
  Commands:
  - `cargo test -p trust-runtime --test st_test_cli_command`
- [x] A-VAL-002 Baseline gates pass at milestone boundary.
  Evidence:
  - `just fmt`
  - `just clippy`
  - `just test`
  Commands:
  - `just fmt`
  - `just clippy`
  - `just test`

## 2. Milestone B: Native GVL Shapes and Documented Vendor Behavior

### 2.1 `PROGRAM`-Scoped `VAR_GLOBAL`

- [x] B-PROG-001 Red HIR tests cover `PROGRAM`-scoped `VAR_GLOBAL`.
  Evidence: `crates/trust-hir/tests/var_sections.rs`, `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs`.
  Tests/files: `crates/trust-hir/tests/var_sections.rs`, `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs`.
- [x] B-PROG-002 `VAR_EXTERNAL` may target a `PROGRAM`-scoped global where intended.
  Evidence: `crates/trust-hir/src/db/diagnostics/context.rs`, validated with `cargo test -p trust-hir test_var_external_matches_program_scoped_global -- --nocapture`.
  Target files: `crates/trust-hir/src/db/diagnostics/context.rs`, `crates/trust-hir/src/db/diagnostics/globals.rs`.
- [x] B-PROG-003 IEC decision recorded and specs aligned.
  Evidence: `docs/IEC_DECISIONS.md`, `docs/specs/03-variables.md`, `docs/specs/09-semantic-rules.md`, `docs/specs/10-runtime-semantics.md`.
  Target files: `docs/IEC_DECISIONS.md`, `docs/specs/03-variables.md`, `docs/specs/09-semantic-rules.md`, `docs/specs/10-runtime-semantics.md`.

### 2.2 File-Scope GVL

- [x] B-GVL-001 Red parser tests cover top-level `VAR_GLOBAL ... END_VAR`.
  Evidence: `crates/trust-syntax/tests/parser_variables.rs`, validated with `cargo test -p trust-syntax test_file_scope_var_global -- --nocapture`.
  Tests/files: `crates/trust-syntax/tests/parser_variables.rs` or `crates/trust-syntax/tests/parser_pous.rs`.
- [x] B-GVL-002 Red HIR tests cover file-scope globals in symbol collection and resolution.
  Evidence: `crates/trust-hir/tests/var_sections.rs`, validated with `cargo test -p trust-hir file_scope_var_global_is_accepted_across_files -- --nocapture`.
  Tests/files: `crates/trust-hir/tests/var_sections.rs`.
- [x] B-GVL-003 Red runtime tests prove shared access from multiple POUs.
  Evidence: `crates/trust-runtime/tests/vars_access.rs`, validated with `cargo test -p trust-runtime --test vars_access file_scope_globals_are_shared_across_program_and_function_blocks -- --nocapture`.
  Tests/files: `crates/trust-runtime/tests/vars_access.rs`.
- [x] B-GVL-004 Multi-file GVL aggregation is covered by tests.
  Evidence: `crates/trust-hir/tests/var_sections.rs` (`multiple_file_scope_gvls_are_aggregated`), validated with `cargo test -p trust-hir multiple_file_scope_gvls_are_aggregated -- --nocapture`.
  Test shape: `gvl_a.st`, `gvl_b.st`, third file consuming both.
- [x] B-GVL-005 Parser accepts file-scope `VAR_GLOBAL` with minimal syntax changes.
  Evidence: `crates/trust-syntax/src/parser/parser.rs`.
  Target files: `crates/trust-syntax/src/parser/parser.rs`, `crates/trust-syntax/src/parser/grammar/declarations.rs`.
- [x] B-GVL-006 Runtime collects and lowers file-scope globals.
  Evidence: `crates/trust-runtime/src/harness/compiler/config.rs`, `crates/trust-runtime/src/harness/build.rs`.
  Target files: `crates/trust-runtime/src/harness/build.rs`, `crates/trust-runtime/src/harness/compiler/config/globals_access.rs`.
- [x] B-GVL-007 Name collision rule is decided and enforced in HIR.
  Evidence: `docs/IEC_DECISIONS.md`, `crates/trust-hir/src/db/diagnostics/globals.rs`, validated with `cargo test -p trust-hir duplicate_global_names_across_scopes_are_rejected -- --nocapture`.
  Decision: reject duplicate global names across file/config/program scopes with a clear diagnostic.
  Target files: `docs/IEC_DECISIONS.md`, `crates/trust-hir/src/db/diagnostics/context.rs`, `crates/trust-hir/src/db/diagnostics/globals.rs`.
- [x] B-GVL-008 File-scope GVL support is recorded as a vendor-parity deviation.
  Evidence: `docs/IEC_DEVIATIONS.md`, `docs/specs/03-variables.md`.
  Target file: `docs/IEC_DEVIATIONS.md`.

### 2.3 Namespaced GVL

- [x] B-NS-001 Red parser tests cover `NAMESPACE ... VAR_GLOBAL ... END_NAMESPACE`.
  Evidence: `crates/trust-syntax/tests/parser_pous.rs` (`test_namespace_with_var_global`), validated with `cargo test -p trust-syntax test_namespace_with_var_global -- --nocapture`.
  Tests/files: `crates/trust-syntax/tests/parser_pous.rs`.
- [x] B-NS-002 Red HIR/runtime tests cover qualified access.
  Evidence:
  - `crates/trust-hir/tests/namespaces.rs` (`namespace_global_supports_qualified_reads_and_writes`)
  - `crates/trust-runtime/tests/vars_access.rs` (`namespaced_globals_support_qualified_access`)
  Validated with:
  - `cargo test -p trust-hir namespace_global_supports_qualified_reads_and_writes -- --nocapture`
  - `cargo test -p trust-runtime --test vars_access namespaced_globals_support_qualified_access -- --nocapture`
- [x] B-NS-003 Namespace parser accepts `VAR_GLOBAL`.
  Evidence: `crates/trust-syntax/src/parser/grammar/pou/pou_part_04.rs`.
  Target file: `crates/trust-syntax/src/parser/grammar/pou/pou_part_04.rs`.
- [x] B-NS-004 `{attribute 'qualified_only'}` is explicitly documented as parsed/ignored or unsupported, not silently claimed.
  Evidence: `docs/IEC_DEVIATIONS.md`, `docs/guides/PLCOPEN_INTEROP_COMPATIBILITY.md`.
  Target files: `docs/IEC_DEVIATIONS.md`, `docs/guides/PLCOPEN_INTEROP_COMPATIBILITY.md`.

### 2.4 Bare Global Access, Part 7a

- [x] B-BARE-001 Positive HIR tests cover bare access from `PROGRAM`, `FUNCTION`, `FUNCTION_BLOCK`, `CLASS`, and methods.
  Evidence:
  - `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` (`test_bare_global_access_is_accepted_across_pou_kinds`)
  - `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` (`test_bare_configuration_global_access_resolves_across_files`)
  Validated with:
  - `cargo test -p trust-hir test_bare_global_access_is_accepted_across_pou_kinds -- --nocapture`
  - `cargo test -p trust-hir test_bare_configuration_global_access_resolves_across_files -- --nocapture`
- [x] B-BARE-002 Positive runtime tests cover file/config/program/namespace global access without `VAR_EXTERNAL`.
  Evidence: `crates/trust-runtime/tests/vars_access.rs` (`globals_are_accessible_without_var_external_across_vendor_parity_scopes`), validated with `cargo test -p trust-runtime --test vars_access globals_are_accessible_without_var_external_across_vendor_parity_scopes -- --nocapture`.
  Tests/files: `crates/trust-runtime/tests/vars_access.rs`.
- [x] B-BARE-003 Deviation recorded: `VAR_EXTERNAL` is supported and type-checked, but optional for vendor parity.
  Evidence: `docs/IEC_DEVIATIONS.md`.
  Target file: `docs/IEC_DEVIATIONS.md`.
- [x] B-BARE-004 Docs/specs no longer claim explicit `VAR_EXTERNAL` is required by truST for all vendor-parity paths.
  Evidence: `docs/specs/03-variables.md`, `docs/specs/09-semantic-rules.md`, `docs/specs/10-runtime-semantics.md`.
  Target files: `docs/specs/03-variables.md`, `docs/specs/09-semantic-rules.md`, `docs/specs/10-runtime-semantics.md`.
- [x] B-BARE-005 Existing runtime fallback paths are retained intentionally.
  Evidence: `crates/trust-runtime/src/bytecode/encoder/refs.rs`, `crates/trust-runtime/src/eval/expr/access.rs`.
  Target files: `crates/trust-runtime/src/bytecode/encoder/refs.rs`, `crates/trust-runtime/src/eval/expr/access.rs`.

### 2.5 Milestone B Validation

- [x] B-VAL-001 Targeted parser/HIR/runtime suites pass.
  Evidence:
  - `cargo test -p trust-syntax --test parser_variables -- --nocapture`
  - `cargo test -p trust-syntax --test parser_pous -- --nocapture`
  - `cargo test -p trust-hir --test var_sections -- --nocapture`
  - `cargo test -p trust-hir --test namespaces -- --nocapture`
  - `cargo test -p trust-hir --test semantic_type_checking -- --nocapture`
  - `cargo test -p trust-runtime --test vars_access -- --nocapture`
  Commands:
  - `cargo test -p trust-syntax --test parser_variables -- --nocapture`
  - `cargo test -p trust-syntax --test parser_pous -- --nocapture`
  - `cargo test -p trust-hir --test var_sections -- --nocapture`
  - `cargo test -p trust-hir --test namespaces -- --nocapture`
  - `cargo test -p trust-hir --test semantic_type_checking -- --nocapture`
  - `cargo test -p trust-runtime --test vars_access`
- [ ] B-VAL-002 Additive sanity guard passes after only B-PROG/B-GVL/B-NS/B-BARE-7a work lands.
  Evidence: Not captured as a standalone mid-milestone run; left unchecked intentionally.
  Commands:
  - `cargo test -p trust-runtime --lib --tests`
- [x] B-VAL-003 Baseline gates pass at milestone boundary.
  Evidence:
  - `just fmt`
  - `just clippy`
  - `just test`
  Commands:
  - `just fmt`
  - `just clippy`
  - `just test`

## 3. Milestone C: Tightening, `VAR_STAT`, and Import Simplification

### 3.1 Bare Global Access, Part 7b

- [x] C-FB-001 Red negative tests prove undefined bare names fail in every POU kind.
  Evidence: `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs`, validated via `cargo test -p trust-hir --test semantic_type_checking -- --nocapture`.
  Tests/files: `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs`.
- [x] C-FB-002 Existing fixture corpus is scanned before resolver tightening; expected cleanup list is recorded.
  Evidence: no additional fixture/test migrations were required after resolver tightening; validated by `cargo test -p trust-hir --test semantic_type_checking -- --nocapture`.
- [x] C-FB-003 HIR resolver is tightened so only actual globals survive bare-name resolution.
  Evidence: `crates/trust-hir/src/type_check/validation.rs`, `crates/trust-hir/src/db/symbol_import.rs`, validated by `cargo test -p trust-hir --test semantic_type_checking -- --nocapture`.
  Target files: `crates/trust-hir/src/type_check/validation.rs`, `crates/trust-hir/src/db/symbol_import.rs`, and related symbol-resolution code.

### 3.2 `VAR_STAT`

- [x] C-STAT-001 `VAR_STAT` runtime semantics are written down before coding.
  Evidence: `docs/IEC_DEVIATIONS.md`.
  Target file: `docs/IEC_DEVIATIONS.md`.
- [x] C-STAT-002 Red runtime tests cover function persistence and scope isolation.
  Evidence: `crates/trust-runtime/tests/var_stat.rs`, validated with `cargo test -p trust-runtime --test var_stat -- --nocapture`.
  Tests/files: `crates/trust-runtime/tests/var_stat.rs`.
- [x] C-STAT-003 Runtime harness accepts and lowers `VAR_STAT`.
  Evidence: `crates/trust-runtime/src/harness/compiler/vars.rs`, `crates/trust-runtime/src/harness/compiler/pou/program_vars.rs`, `crates/trust-runtime/src/harness/compiler/pou/function_vars.rs`, `crates/trust-runtime/src/harness/compiler/pou/function_block_vars.rs`, `crates/trust-runtime/src/harness/compiler/pou/class_vars.rs`.
  Target files: `crates/trust-runtime/src/harness/compiler/vars.rs`, `crates/trust-runtime/src/harness/compiler/pou/program_vars.rs`, `crates/trust-runtime/src/harness/compiler/pou/function_vars.rs`, `crates/trust-runtime/src/harness/compiler/pou/function_block_vars.rs`, `crates/trust-runtime/src/harness/compiler/pou/class_vars.rs`.
- [x] C-STAT-004 Storage/runtime plumbing matches the documented semantics.
  Evidence: `crates/trust-runtime/src/eval/locals.rs`, `crates/trust-runtime/src/eval/expr/access.rs`, `crates/trust-runtime/src/eval/calls.rs`, `crates/trust-runtime/src/bytecode/encoder/refs.rs`, `crates/trust-runtime/src/bytecode/encoder/pou/build.rs`, `crates/trust-runtime/src/instance.rs`, validated with `cargo test -p trust-runtime --test var_stat -- --nocapture`.
  Target files: `crates/trust-runtime/src/eval/locals.rs`, `crates/trust-runtime/src/eval/expr/access.rs`, `crates/trust-runtime/src/eval/calls.rs`, `crates/trust-runtime/src/bytecode/encoder/refs.rs`, `crates/trust-runtime/src/bytecode/encoder/pou/build.rs`, `crates/trust-runtime/src/instance.rs`.
- [x] C-STAT-005 Specs/docs reflect shipped runtime behavior.
  Evidence: `docs/specs/01-lexical-elements.md`, `docs/specs/10-runtime-semantics.md`, `docs/IEC_DEVIATIONS.md`.
  Target files: `docs/specs/01-lexical-elements.md`, `docs/specs/10-runtime-semantics.md`.

### 3.3 PLCopen/CODESYS Import Simplification

- [x] C-IMPORT-001 Red regressions prove CODESYS GVL import works without mandatory wrapper + `VAR_EXTERNAL` injection.
  Evidence: `crates/trust-runtime/tests/plcopen_codesys_import_runtime.rs`, validated with `cargo test -p trust-runtime --test plcopen_codesys_import_runtime -- --nocapture`.
  Tests/files: `crates/trust-runtime/tests/plcopen_migration.rs`, `crates/trust-runtime/tests/plcopen_st_complete_parity.rs`, or focused importer tests.
- [x] C-IMPORT-002 Regression covers both qualified access and bare-access POUs.
  Evidence: `crates/trust-runtime/tests/plcopen_codesys_import_runtime.rs` covers namespaced qualified access without injected externals and the default bare-global vendor-parity path; validated with `cargo test -p trust-runtime --test plcopen_codesys_import_runtime -- --nocapture`.
- [x] C-IMPORT-003 Strict-IEC injection path remains available as optional export/adapter behavior.
  Evidence: `crates/trust-plcopen/src/plcopen/import.rs`, `crates/trust-plcopen/src/plcopen/import_data_globals.rs`, `crates/trust-plcopen/src/plcopen/pou_externals.rs`, `docs/guides/PLCOPEN_INTEROP_COMPATIBILITY.md`, validated with `cargo test -p trust-runtime --test plcopen_codesys_import_runtime -- --nocapture`.
  Target files: `crates/trust-plcopen/src/plcopen/import.rs`, `crates/trust-plcopen/src/plcopen/import_data_globals.rs`, `crates/trust-plcopen/src/plcopen/pou_externals.rs`, `docs/guides/PLCOPEN_INTEROP_COMPATIBILITY.md`.

### 3.4 Milestone C Validation

- [x] C-VAL-001 Targeted runtime/import suites pass.
  Evidence:
  - `cargo test -p trust-runtime --test vars_access -- --nocapture`
  - `cargo test -p trust-runtime --test var_stat -- --nocapture`
  - `cargo test -p trust-runtime --lib plcopen::tests::import_ -- --nocapture`
  Commands:
  - `cargo test -p trust-runtime --test vars_access`
  - `cargo test -p trust-runtime --test var_stat`
  - `cargo test -p trust-runtime --lib plcopen::tests::import_ -- --nocapture`
- [x] C-VAL-002 Runtime vertical tests pass.
  Evidence:
  - `cargo test -p trust-runtime --test api_smoke`
  - `cargo test -p trust-runtime --test debug_control`
  - `cargo test -p trust-runtime --test complete_program`
  - `cargo test -p trust-runtime --test runtime_reliability`
  Commands:
  - `cargo test -p trust-runtime --test api_smoke`
  - `cargo test -p trust-runtime --test debug_control`
  - `cargo test -p trust-runtime --test complete_program`
  - `cargo test -p trust-runtime --test runtime_reliability`
- [x] C-VAL-003 Final gates pass.
  Evidence:
  - `just fmt`
  - `just clippy`
  - `just test`
  - `just test-all`
  Commands:
  - `just fmt`
  - `just clippy`
  - `just test`
  - `just test-all`

## 4. Architecture Acceptance Checks

- [x] ARCH-001 SOLID: test-mode harness path is isolated from normal runtime execution semantics.
  Evidence: `crates/trust-runtime/src/bin/trust-runtime/test_cmd/command.rs`, `crates/trust-runtime/src/harness/api.rs`, `crates/trust-runtime/src/harness/build.rs`.
- [x] ARCH-002 SOLID: parser/HIR/runtime changes each keep a single clear responsibility; no new “god” module is introduced.
  Evidence: parser changes stay in `crates/trust-syntax`, semantic/global rules stay in `crates/trust-hir`, runtime lowering/import behavior stays in `crates/trust-runtime`.
- [x] ARCH-003 KISS: file-scope and namespace GVL reuse existing var-block parsing/lowering paths where possible.
  Evidence: `crates/trust-syntax/src/parser/parser.rs`, `crates/trust-syntax/src/parser/grammar/pou/pou_part_04.rs`, `crates/trust-runtime/src/harness/compiler/config/globals_access.rs`.
- [x] ARCH-004 DRY: global collection/lowering does not duplicate config/file/program logic unnecessarily.
  Evidence: shared collection/lowering paths in `crates/trust-runtime/src/harness/compiler/config.rs`, `crates/trust-runtime/src/harness/build.rs`, `crates/trust-runtime/src/harness/compiler/config/globals_access.rs`.
- [x] ARCH-005 Any ownership or execution-flow diagram changes are reflected in `docs/diagrams/**/*.puml` if architectural boundaries change materially.
  Evidence: no material ownership or subsystem-boundary change was introduced beyond existing parser/HIR/runtime modules, so no diagram delta was required.

## 5. Sign-off

- Implementation owner: Codex
- Runtime owner: Codex
- HIR owner: Codex
- Docs owner: Codex
- Validation owner: Codex
- Notes:
  - `B-VAL-002` remains intentionally unchecked because the dedicated additive-only sanity run was not captured as a standalone checkpoint during Milestone B.
  - Workspace version was bumped to `0.11.0`; tag/release flow is not complete on this branch and must be done from `main` per repo policy.
