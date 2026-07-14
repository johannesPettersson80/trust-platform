# Phase 5 Suite and Gate Audit

Generator: `phase5-suite-audit v1`
Source revision: `9eb4a3736807db53e5af03705588ca10e46254cb`
Generated: `2026-07-14T03:10:44+02:00`
Platform: `linux-x86_64`
Generated JSON SHA-256: `b6616bf535ca584d23261124f8a0df5932fb25bdda377a90f594feb830610366`
Input SHA-256: `sha256:f74c3297400753681a117b2aa0a3f5c5672be82796c1c6f1111e9f82c37c9881`

This report inventories suite ownership and routing without creating proof,
closing specification gaps, interpreting suite inheritance, or changing enforcement.

## Summary

- Inventory records: 62 (59 scanner-bound)
- Suite records: 6
- Direct suite commands: 33
- Suite inventory references: 58
- Canonical areas: 11
- Ordered taxonomy routes: 29

## Boundaries

- `report_only_enforcement_unchanged`: `true`
- `report_emits_proof`: `false`
- `report_closes_spec_gaps`: `false`
- `suite_includes_interpreted`: `false`
- `p5_000b_remains_open`: `true`

## Inventory

| ID | Source | Disposition | Suites | Enforcement | Artifact |
| --- | --- | --- | --- | --- | --- |
| `GATE_JOB_CI_ARCHITECTURE_SAFETY` | `github_workflow_job` | `assigned` | `pr` | `required` | `ci_artifact/repository_default` |
| `GATE_JOB_CI_BROWSER_ANALYSIS` | `github_workflow_job` | `assigned` | `pr` | `required` | `ci_artifact/repository_default` |
| `GATE_JOB_CI_CLIPPY` | `github_workflow_job` | `assigned` | `pr` | `required` | `ci_artifact/repository_default` |
| `GATE_JOB_CI_CONFORMANCE` | `github_workflow_job` | `assigned` | `pr` | `required` | `ci_artifact/repository_default` |
| `GATE_JOB_CI_DOCS` | `github_workflow_job` | `assigned` | `pr` | `required` | `ci_artifact/repository_default` |
| `GATE_JOB_CI_EDITOR_EXPANSION` | `github_workflow_job` | `assigned` | `pr` | `required` | `ci_artifact/repository_default` |
| `GATE_JOB_CI_FMT` | `github_workflow_job` | `assigned` | `pr` | `required` | `ci_artifact/repository_default` |
| `GATE_JOB_CI_MP001_PARITY` | `github_workflow_job` | `assigned` | `pr` | `required` | `ci_artifact/repository_default` |
| `GATE_JOB_CI_MSRV` | `github_workflow_job` | `assigned` | `pr` | `required` | `ci_artifact/repository_default` |
| `GATE_JOB_CI_RELEASE_GATE_REPORT` | `github_workflow_job` | `assigned` | `release` | `required` | `ci_artifact/repository_default` |
| `GATE_JOB_CI_SUPPLY_CHAIN` | `github_workflow_job` | `assigned` | `pr` | `required` | `ci_artifact/repository_default` |
| `GATE_JOB_CI_TEST` | `github_workflow_job` | `assigned` | `pr` | `required` | `ci_artifact/repository_default` |
| `GATE_JOB_CI_VERSION_RELEASE_GUARD` | `github_workflow_job` | `assigned` | `release` | `required` | `ci_artifact/repository_default` |
| `GATE_JOB_CI_VSCODE_EXTENSION` | `github_workflow_job` | `assigned` | `pr` | `required` | `ci_artifact/repository_default` |
| `GATE_JOB_DOCS_CAPTURES_REFRESH` | `github_workflow_job` | `excluded` | `none` | `excluded` | `ci_artifact/repository_default` |
| `GATE_JOB_DOCS_PAGES_BUILD` | `github_workflow_job` | `excluded` | `none` | `excluded` | `ci_artifact/repository_default` |
| `GATE_JOB_DOCS_PAGES_DEPLOY` | `github_workflow_job` | `excluded` | `none` | `excluded` | `none/none` |
| `GATE_JOB_HMI_LONG_SOAK` | `github_workflow_job` | `assigned` | `nightly` | `conditional` | `ci_artifact/repository_default` |
| `GATE_JOB_NIGHTLY_RELIABILITY` | `github_workflow_job` | `assigned` | `nightly` | `conditional` | `ci_artifact/repository_default` |
| `GATE_JOB_PROTOCOL_DEVICE_IN_LOOP` | `github_workflow_job` | `assigned` | `hardware_lab` | `conditional` | `lab_report/repository_default` |
| `GATE_JOB_RELEASE_BUILD` | `github_workflow_job` | `assigned` | `release` | `required` | `ci_artifact/repository_default` |
| `GATE_JOB_RELEASE_PREFLIGHT` | `github_workflow_job` | `assigned` | `release` | `required` | `ci_job_result/repository_default` |
| `GATE_JOB_RELEASE_PUBLISH` | `github_workflow_job` | `assigned` | `release` | `required` | `release_object/release_object` |
| `GATE_JOB_RELEASE_RUNTIME_VM_VALIDATION` | `github_workflow_job` | `assigned` | `release` | `required` | `ci_artifact/repository_default` |
| `GATE_JOB_RENDER_DIAGRAMS` | `github_workflow_job` | `excluded` | `none` | `excluded` | `committed_file/committed` |
| `GATE_JOB_SALSA_FUZZ_EXTENDED_NIGHTLY` | `github_workflow_job` | `assigned` | `nightly` | `required` | `ci_artifact/repository_default` |
| `GATE_JOB_SALSA_FUZZ_SMOKE` | `github_workflow_job` | `assigned` | `pr` | `required` | `ci_artifact/repository_default` |
| `GATE_JOB_SALSA_MEMORY_REGRESSION` | `github_workflow_job` | `assigned` | `pr` | `required` | `none/none` |
| `GATE_JOB_SALSA_MIRI_NIGHTLY` | `github_workflow_job` | `assigned` | `nightly` | `required` | `none/none` |
| `GATE_JOB_VERIFICATION_REPORT` | `github_workflow_job` | `report_only` | `pr` | `report_only` | `ci_artifact/repository_default` |
| `GATE_JUST_VERIFICATION_VERYQUICK` | `just_recipe` | `assigned` | `veryquick` | `planned` | `machine_local/machine_local` |
| `GATE_MUTATION_BYTECODE_VALIDATOR` | `catalog_test_command` | `assigned` | `nightly` | `planned` | `machine_local/machine_local` |
| `GATE_SCRIPT_AGGREGATE_ST_TEST_FLAKE_HISTORY` | `gate_script` | `assigned` | `nightly` | `conditional` | `machine_local/machine_local` |
| `GATE_SCRIPT_ARCHITECTURE_EXTERNAL_SAFETY_AST_GREP` | `gate_script` | `assigned` | `pr` | `required` | `machine_local/machine_local` |
| `GATE_SCRIPT_ARCHITECTURE_EXTERNAL_SAFETY_GEIGER` | `gate_script` | `excluded` | `none` | `advisory` | `machine_local/machine_local` |
| `GATE_SCRIPT_CHECK_GATE_OBSERVABILITY` | `gate_script` | `assigned` | `pr` | `required` | `none/none` |
| `GATE_SCRIPT_GENERATE_RELEASE_GATE_REPORT` | `gate_script` | `assigned` | `release` | `required` | `machine_local/machine_local` |
| `GATE_SCRIPT_PREPUSH_CI` | `gate_script` | `supporting` | `supporting_local` | `supporting` | `machine_local/machine_local` |
| `GATE_SCRIPT_RUNTIME_BOUNDARY_FAIL_CLOSED_AST_GREP` | `gate_script` | `assigned` | `pr` | `required` | `machine_local/machine_local` |
| `GATE_SCRIPT_RUNTIME_CLOUD_SECURITY_PROFILE` | `gate_script` | `assigned` | `pr` | `required` | `machine_local/machine_local` |
| `GATE_SCRIPT_RUNTIME_COMMS_BENCH` | `gate_script` | `assigned` | `pr` | `required` | `machine_local/machine_local` |
| `GATE_SCRIPT_RUNTIME_COMMS_CONFORMANCE` | `gate_script` | `assigned` | `pr` | `required` | `machine_local/machine_local` |
| `GATE_SCRIPT_RUNTIME_COMMS_FUZZ` | `gate_script` | `assigned` | `pr` | `required` | `machine_local/machine_local` |
| `GATE_SCRIPT_RUNTIME_DEVICE_IN_LOOP` | `gate_script` | `assigned` | `hardware_lab` | `conditional` | `lab_report/machine_local` |
| `GATE_SCRIPT_RUNTIME_MESH_TLS_STABILITY` | `gate_script` | `assigned` | `pr` | `required` | `machine_local/machine_local` |
| `GATE_SCRIPT_RUNTIME_MOTION_EXAMPLE_BENCH` | `gate_script` | `assigned` | `nightly` | `planned` | `machine_local/machine_local` |
| `GATE_SCRIPT_RUNTIME_SAFETY_FAIL_CLOSED_AST_GREP` | `gate_script` | `assigned` | `pr` | `required` | `machine_local/machine_local` |
| `GATE_SCRIPT_RUNTIME_VM_BENCH` | `gate_script` | `assigned` | `pr, release` | `required` | `machine_local/machine_local` |
| `GATE_SCRIPT_RUNTIME_VM_DETERMINISM_RELIABILITY` | `gate_script` | `assigned` | `nightly, pr, release` | `required` | `machine_local/machine_local` |
| `GATE_SCRIPT_RUNTIME_VM_MALFORMED_BYTECODE_FUZZ` | `gate_script` | `assigned` | `nightly` | `planned` | `machine_local/machine_local` |
| `GATE_SCRIPT_SALSA_FUZZ` | `gate_script` | `assigned` | `nightly, pr` | `required` | `machine_local/machine_local` |
| `GATE_SCRIPT_SALSA_HARDENING_PERF` | `gate_script` | `assigned` | `nightly` | `planned` | `committed_file/committed` |
| `GATE_SCRIPT_SALSA_MEMORY` | `gate_script` | `assigned` | `pr` | `required` | `committed_file/committed` |
| `GATE_SCRIPT_SALSA_MIRI` | `gate_script` | `assigned` | `nightly` | `required` | `none/none` |
| `GATE_SCRIPT_SALSA_SPIKE` | `gate_script` | `supporting` | `supporting_local` | `supporting` | `none/none` |
| `GATE_SCRIPT_UNSAFE_CONCURRENCY_GEIGER` | `gate_script` | `excluded` | `none` | `advisory` | `machine_local/machine_local` |
| `GATE_SCRIPT_UNSAFE_CONCURRENCY_MIRI` | `gate_script` | `assigned` | `nightly` | `planned` | `machine_local/machine_local` |
| `GATE_SCRIPT_UNSAFE_CONCURRENCY_SANITIZER` | `gate_script` | `excluded` | `none` | `advisory` | `machine_local/machine_local` |
| `GATE_SCRIPT_UNSAFE_CONCURRENCY_VALGRIND` | `gate_script` | `assigned` | `nightly` | `planned` | `machine_local/machine_local` |
| `GATE_SCRIPT_VERIFICATION_METADATA` | `gate_script` | `assigned` | `veryquick` | `planned` | `none/none` |
| `GATE_SCRIPT_VERIFICATION_REPORT` | `gate_script` | `report_only` | `pr` | `report_only` | `machine_local/machine_local` |
| `GATE_TEMPLATE_TRUST_RUNTIME_PROJECT_CI` | `workflow_template` | `excluded` | `none` | `non_executable` | `none/none` |

## Suites

| Suite | Environment | Direct commands | Inventory refs | Includes |
| --- | --- | ---: | ---: | --- |
| `hardware_lab` | `github_or_lab_runner` | 1 | 2 | `none` |
| `nightly` | `github_nightly` | 10 | 14 | `pr` |
| `pr` | `github_matrix` | 15 | 29 | `veryquick` |
| `release` | `github_release_matrix` | 6 | 9 | `nightly` |
| `supporting_local` | `local` | 0 | 2 | `none` |
| `veryquick` | `trust_builder` | 1 | 2 | `none` |

## Canonical Areas

| Area | Owner | Direct suites | Required classes |
| --- | --- | --- | --- |
| `compiler_iec` | `language` | `pr` | `metadata_validation, unit, negative_malformed_input, iec_conformance` |
| `bytecode_vm` | `trust-runtime` | `pr` | `metadata_validation, negative_malformed_input, failing_regression, iec_conformance, mutation` |
| `runtime_safety` | `trust-runtime` | `pr` | `unit, integration, runtime_vertical` |
| `protocols` | `trust-runtime` | `pr` | `unit, integration, protocol_loopback` |
| `control_security` | `trust-runtime` | `pr` | `integration, runtime_vertical, rbac_security` |
| `editor_safety` | `editor` | `pr` | `unit, lsp_protocol, vscode_extension` |
| `plcopen_devtools` | `developer-tools` | `pr` | `unit, integration, negative_malformed_input` |
| `hmi_ui` | `hmi` | `pr` | `integration, runtime_vertical, browser_webview_visual, ui_journey_acceptance` |
| `release` | `release-engineering` | `pr` | `release_docs` |
| `supply_chain_platform` | `release-engineering` | `pr` | `supply_chain_security, platform_package` |
| `verification` | `verification` | `veryquick, pr` | `metadata_validation` |

## Ordered Taxonomy Routes

| Order | Route | Areas | Direct suites | Conditional suites |
| ---: | --- | --- | --- | --- |
| 1 | `lexer_parser` | `compiler_iec` | `pr` | `nightly` |
| 2 | `hir_type_diagnostics` | `compiler_iec` | `pr` | `none` |
| 3 | `source_lowering_host_harness` | `bytecode_vm` | `pr` | `none` |
| 4 | `bytecode_encoder_validator_container` | `bytecode_vm` | `pr` | `nightly` |
| 5 | `vm_value_backend_execution` | `bytecode_vm` | `pr` | `nightly` |
| 6 | `debug_authority_lifecycle` | `editor_safety, control_security, runtime_safety` | `pr` | `none` |
| 7 | `runtime_scheduler_lifecycle` | `runtime_safety` | `pr` | `release` |
| 8 | `retain_restart_init_reset` | `runtime_safety` | `pr` | `release` |
| 9 | `process_image_memory_map` | `runtime_safety` | `pr` | `none` |
| 10 | `modbus_mqtt_gpio_io` | `protocols` | `pr` | `hardware_lab` |
| 11 | `ethercat_io` | `protocols` | `pr` | `hardware_lab` |
| 12 | `ads_opcua_connectors` | `protocols` | `pr` | `hardware_lab` |
| 13 | `control_api_rbac_web` | `control_security` | `pr` | `none` |
| 14 | `hmi_runtime_web_ui` | `hmi_ui, control_security` | `pr` | `none` |
| 15 | `ide_source_intelligence` | `editor_safety` | `pr` | `none` |
| 16 | `lsp_protocol_boundary` | `editor_safety` | `pr` | `none` |
| 17 | `vscode_extension` | `editor_safety` | `pr` | `none` |
| 18 | `public_workflow` | `release` | `pr` | `release` |
| 19 | `plcopen_import_export` | `plcopen_devtools` | `pr` | `none` |
| 20 | `trust_dev_cli` | `plcopen_devtools` | `pr` | `none` |
| 21 | `conformance_suite` | `verification, compiler_iec, bytecode_vm, runtime_safety` | `pr` | `release` |
| 22 | `verification_tooling` | `verification` | `veryquick, pr` | `none` |
| 23 | `architecture_diagrams` | `verification` | `pr` | `none` |
| 24 | `public_docs_release_version` | `release` | `pr` | `release` |
| 25 | `unsafe_concurrency` | `verification, supply_chain_platform` | `pr` | `nightly` |
| 26 | `performance_hot_paths` | `bytecode_vm, runtime_safety, editor_safety` | `pr` | `nightly, release` |
| 27 | `security_supply_chain` | `supply_chain_platform, control_security` | `pr` | `release` |
| 28 | `platform_package` | `supply_chain_platform` | `pr` | `release` |
| 29 | `refactor_only` | `intent-only` | `pr` | `none` |

## Limitations

- This report maps existing verification surfaces; the generator emits no behavior proof and closes no specification gap.
- Suite includes and excludes are displayed but not interpreted; VERIF-P14-000B still owns composition semantics.
- Report-only and planned inventory rows remain non-enforcing; this report changes no workflow or CI setting.
- VERIF-P5-000B is live-validated from the board but excluded from the source digest because board/evidence follow-up is mutable.
