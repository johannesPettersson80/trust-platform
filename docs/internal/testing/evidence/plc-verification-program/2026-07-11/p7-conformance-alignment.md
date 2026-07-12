# Phase 7 Conformance Program Alignment

Generator: `conformance-alignment-audit v1`
Source revision: `aecec2e9a79a9f3101b5e85947a37a34f8d71517`
Generated: `2026-07-12T10:45:00+02:00`
Platform: `linux-aarch64`
Generated JSON SHA-256: `8bcc3e9334e95abe3067e0ad814cea6c405fb86ea7f1354671b5d2386c603f6f`
Input SHA-256: `sha256:7f47a6f46dc65303b6db41591eed05d2b327a2bd69366c53630adc9fffa70cf4`

This is a report-only audit of committed conformance manifests, expected
artifacts, explicit catalog links, publication posture, and the scripted
comms-determinism case. It executes no conformance case and creates no proof.

## Summary

- Categories: 16 (6 v1, 10 v2)
- Cases: 21 (11 v1, 10 v2)
- Runtime cases: 19
- Compile-error cases: 1
- Connector-status-trace cases: 1
- Program sources: 20
- Expected artifacts: 21
- Missing expected artifacts: 0
- Orphan expected artifacts: 0
- Explicitly linked cases: 0
- Unlinked cases: 21
- Coverage gaps: 10

## Categories

| Profile | Category | Cases | Case IDs |
| --- | --- | ---: | --- |
| `v1` | `timers` | 3 | `cfm_timers_tof_sequence_002`, `cfm_timers_ton_sequence_001`, `cfm_timers_tp_sequence_003` |
| `v1` | `edges` | 1 | `cfm_edges_r_f_trig_sequence_001` |
| `v1` | `scan_cycle` | 1 | `cfm_scan_cycle_ordering_visibility_001` |
| `v1` | `init_reset` | 2 | `cfm_init_reset_retain_warm_cold_002`, `cfm_init_reset_var_initialization_001` |
| `v1` | `arithmetic` | 2 | `cfm_arithmetic_conversion_compare_001`, `cfm_arithmetic_overflow_error_002` |
| `v1` | `memory_map` | 2 | `cfm_memory_map_var_config_sync_001`, `cfm_memory_map_wildcard_unresolved_002` |
| `v2` | `strings` | 1 | `cfm_strings_slice_concat_001` |
| `v2` | `arrays` | 1 | `cfm_arrays_matrix_update_001` |
| `v2` | `structs` | 1 | `cfm_structs_initializer_field_update_001` |
| `v2` | `enums` | 1 | `cfm_enums_variant_assignment_001` |
| `v2` | `nested_values` | 1 | `cfm_nested_values_matrix_struct_001` |
| `v2` | `oop_dispatch` | 1 | `cfm_oop_dispatch_interface_super_001` |
| `v2` | `references` | 1 | `cfm_references_ref_to_deref_write_001` |
| `v2` | `retain_matrix` | 1 | `cfm_retain_matrix_restart_aliases_001` |
| `v2` | `scheduler` | 1 | `cfm_scheduler_task_interval_001` |
| `v2` | `comms_determinism` | 1 | `cfm_comms_determinism_connector_projection_001` |

## Cases

| Case | Profile | Category | Kind | Program | Expected | Catalog test | Invariants |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `cfm_arithmetic_conversion_compare_001` | `v1` | `arithmetic` | `runtime` | `conformance/cases/arithmetic/cfm_arithmetic_conversion_compare_001/program.st` | `conformance/expected/arithmetic/cfm_arithmetic_conversion_compare_001.json` | `none` | none |
| `cfm_arithmetic_overflow_error_002` | `v1` | `arithmetic` | `runtime` | `conformance/cases/arithmetic/cfm_arithmetic_overflow_error_002/program.st` | `conformance/expected/arithmetic/cfm_arithmetic_overflow_error_002.json` | `none` | none |
| `cfm_arrays_matrix_update_001` | `v2` | `arrays` | `runtime` | `conformance/cases/arrays/cfm_arrays_matrix_update_001/program.st` | `conformance/expected/arrays/cfm_arrays_matrix_update_001.json` | `none` | none |
| `cfm_comms_determinism_connector_projection_001` | `v2` | `comms_determinism` | `connector_status_trace` | `none` | `conformance/expected/comms_determinism/cfm_comms_determinism_connector_projection_001.json` | `none` | none |
| `cfm_edges_r_f_trig_sequence_001` | `v1` | `edges` | `runtime` | `conformance/cases/edges/cfm_edges_r_f_trig_sequence_001/program.st` | `conformance/expected/edges/cfm_edges_r_f_trig_sequence_001.json` | `none` | none |
| `cfm_enums_variant_assignment_001` | `v2` | `enums` | `runtime` | `conformance/cases/enums/cfm_enums_variant_assignment_001/program.st` | `conformance/expected/enums/cfm_enums_variant_assignment_001.json` | `none` | none |
| `cfm_init_reset_retain_warm_cold_002` | `v1` | `init_reset` | `runtime` | `conformance/cases/init_reset/cfm_init_reset_retain_warm_cold_002/program.st` | `conformance/expected/init_reset/cfm_init_reset_retain_warm_cold_002.json` | `none` | none |
| `cfm_init_reset_var_initialization_001` | `v1` | `init_reset` | `runtime` | `conformance/cases/init_reset/cfm_init_reset_var_initialization_001/program.st` | `conformance/expected/init_reset/cfm_init_reset_var_initialization_001.json` | `none` | none |
| `cfm_memory_map_var_config_sync_001` | `v1` | `memory_map` | `runtime` | `conformance/cases/memory_map/cfm_memory_map_var_config_sync_001/program.st` | `conformance/expected/memory_map/cfm_memory_map_var_config_sync_001.json` | `none` | none |
| `cfm_memory_map_wildcard_unresolved_002` | `v1` | `memory_map` | `compile_error` | `conformance/cases/memory_map/cfm_memory_map_wildcard_unresolved_002/program.st` | `conformance/expected/memory_map/cfm_memory_map_wildcard_unresolved_002.json` | `none` | none |
| `cfm_nested_values_matrix_struct_001` | `v2` | `nested_values` | `runtime` | `conformance/cases/nested_values/cfm_nested_values_matrix_struct_001/program.st` | `conformance/expected/nested_values/cfm_nested_values_matrix_struct_001.json` | `none` | none |
| `cfm_oop_dispatch_interface_super_001` | `v2` | `oop_dispatch` | `runtime` | `conformance/cases/oop_dispatch/cfm_oop_dispatch_interface_super_001/program.st` | `conformance/expected/oop_dispatch/cfm_oop_dispatch_interface_super_001.json` | `none` | none |
| `cfm_references_ref_to_deref_write_001` | `v2` | `references` | `runtime` | `conformance/cases/references/cfm_references_ref_to_deref_write_001/program.st` | `conformance/expected/references/cfm_references_ref_to_deref_write_001.json` | `none` | none |
| `cfm_retain_matrix_restart_aliases_001` | `v2` | `retain_matrix` | `runtime` | `conformance/cases/retain_matrix/cfm_retain_matrix_restart_aliases_001/program.st` | `conformance/expected/retain_matrix/cfm_retain_matrix_restart_aliases_001.json` | `none` | none |
| `cfm_scan_cycle_ordering_visibility_001` | `v1` | `scan_cycle` | `runtime` | `conformance/cases/scan_cycle/cfm_scan_cycle_ordering_visibility_001/program.st` | `conformance/expected/scan_cycle/cfm_scan_cycle_ordering_visibility_001.json` | `none` | none |
| `cfm_scheduler_task_interval_001` | `v2` | `scheduler` | `runtime` | `conformance/cases/scheduler/cfm_scheduler_task_interval_001/program.st` | `conformance/expected/scheduler/cfm_scheduler_task_interval_001.json` | `none` | none |
| `cfm_strings_slice_concat_001` | `v2` | `strings` | `runtime` | `conformance/cases/strings/cfm_strings_slice_concat_001/program.st` | `conformance/expected/strings/cfm_strings_slice_concat_001.json` | `none` | none |
| `cfm_structs_initializer_field_update_001` | `v2` | `structs` | `runtime` | `conformance/cases/structs/cfm_structs_initializer_field_update_001/program.st` | `conformance/expected/structs/cfm_structs_initializer_field_update_001.json` | `none` | none |
| `cfm_timers_tof_sequence_002` | `v1` | `timers` | `runtime` | `conformance/cases/timers/cfm_timers_tof_sequence_002/program.st` | `conformance/expected/timers/cfm_timers_tof_sequence_002.json` | `none` | none |
| `cfm_timers_ton_sequence_001` | `v1` | `timers` | `runtime` | `conformance/cases/timers/cfm_timers_ton_sequence_001/program.st` | `conformance/expected/timers/cfm_timers_ton_sequence_001.json` | `none` | none |
| `cfm_timers_tp_sequence_003` | `v1` | `timers` | `runtime` | `conformance/cases/timers/cfm_timers_tp_sequence_003/program.st` | `conformance/expected/timers/cfm_timers_tp_sequence_003.json` | `none` | none |

## Coverage Gaps

| Category | Case present | Expected artifact | Invariant mapping | Semantic oracle | Status |
| --- | --- | --- | --- | --- | --- |
| `strings` | `true` | `true` | `missing` | `not_assessed` | `open` |
| `arrays` | `true` | `true` | `missing` | `not_assessed` | `open` |
| `structs` | `true` | `true` | `missing` | `not_assessed` | `open` |
| `enums` | `true` | `true` | `missing` | `not_assessed` | `open` |
| `nested_values` | `true` | `true` | `missing` | `not_assessed` | `open` |
| `oop_dispatch` | `true` | `true` | `missing` | `not_assessed` | `open` |
| `references` | `true` | `true` | `missing` | `not_assessed` | `open` |
| `retain_matrix` | `true` | `true` | `missing` | `not_assessed` | `open` |
| `scheduler` | `true` | `true` | `missing` | `not_assessed` | `open` |
| `comms_determinism` | `true` | `true` | `missing` | `not_assessed` | `open` |

## Comms

- Case: `cfm_comms_determinism_connector_projection_001`
- Kind: `connector_status_trace`
- Execution mode: `scripted_in_process`
- Scripted steps: 8
- Program source present: `false`
- Live socket dependency: `false`
- Reviewed call path: `execute_case`, `execute_connector_status_trace_case`, `project_connector_status_step`
- Reviewed source paths: `crates/trust-runtime/src/bin/trust-runtime/conformance.rs`, `crates/trust-runtime/src/bin/trust-runtime/conformance/execution.rs`, `crates/trust-runtime/src/connectors/mapping.rs`
- Reviewed source SHA-256: `sha256:3af615cb312d64ce9152ab0fbd35b48f77f9fd13b03ffafe88ed8d8a2dd5d5ca`

## Contract

- Spec source: `SPEC_CONFORMANCE_CONTRACT_001`
- Area: `release`
- Owner: `verification`
- Metadata status: `mapped`
- Covers: `conformance_categories`, `summary_profiles`, `result_classification`, `deterministic_ordering`, `generated_report_artifact_policy`
- Authority: `normative_product`
- Oracle eligible: `false`
- Contract path: `conformance/contract.md`
- Contract SHA-256: `sha256:6146d826907eee75ccf4914c17e278215056165c835ff74b04e16b3c31edf511`
- Tracked: `true`
- Visibility: `public`
- Public page bound: `true`
- Reviewed runner source paths: `crates/trust-runtime/src/bin/trust-runtime/conformance.rs`, `crates/trust-runtime/src/bin/trust-runtime/conformance/discovery.rs`, `crates/trust-runtime/src/bin/trust-runtime/conformance/execution.rs`, `crates/trust-runtime/src/bin/trust-runtime/conformance/models.rs`, `crates/trust-runtime/src/bin/trust-runtime/conformance/runner.rs`, `crates/trust-runtime/src/bin/trust-runtime/conformance/series_values.rs`, `crates/trust-runtime/src/bin/trust-runtime/conformance/tests.rs`, `crates/trust-runtime/src/bin/trust-runtime/conformance/time_utils.rs`
- Reviewed runner source SHA-256: `sha256:e792eef817eb9f0ed5b43e56e4525b22415295e5c12bd2ad2e0e5399bf598051`
- Reviewed runner behaviors: `category_profile_order`, `default_program_source`, `case_id_order`, `expected_artifact_comparison`, `case_status_classification`, `summary_emission`

## Publication

- CI job: `.github/workflows/ci.yml#conformance`
- CI job SHA-256: `sha256:687db5f20cd1eef8f6e6cecc1b923a23c5ec5e14b92d9d1c3e90b4cfdf4ac96f`
- CI artifact name: `conformance-suite`
- Generated JSON glob: `gate-artifacts/conformance-pass-*.json`
- Generated Markdown glob: `gate-artifacts/conformance-pass-*.md`
- Generated report policy: `ci_artifact_only`
- Tracked report files: `conformance/reports/.gitkeep`
- Public page embeds generated result: `false`
- Public page SHA-256: `sha256:c01090a72714efc747060c0a9564dfeae4d5ceca6e1e73c737f325ff331803c3`

## Boundaries

- `report_creates_proof`: `false`
- `report_closes_spec_gaps`: `false`
- `semantic_oracles_assessed`: `false`
- `live_network_or_hardware_used`: `false`
- `p7_002_invariant_mapping_remains_open`: `true`
- `generated_reports_remain_ci_artifacts`: `true`
- `public_page_updated`: `false`
- `runtime_or_product_behavior_changed`: `false`
- `ci_enforcement_changed`: `false`

## Limitations

- Catalog associations come only from an exact discovery_id join; names, paths, and prose do not create mappings.
- All current conformance cases are explicitly reported as unlinked; VERIF-P7-002 remains open.
- Coverage-gap rows record missing invariant mappings and do not assess or invent semantic oracles.
- The comms-determinism audit checks the committed scripted in-process case shape; it performs no live socket or hardware execution.
- The public conformance page and registered contract source are bound as publication context, not as behavior proof or external conformance certification.
- Generated conformance results remain CI artifacts; tracked expected artifacts are inputs, not proof that a case passed in this audit.
- The report creates no proof, closes no specification gap, changes no CI enforcement, and changes no runtime or product behavior.
- The blocked-row posture is checked live from the implementation board, which is excluded from the digest because board and evidence closure follow report generation.
