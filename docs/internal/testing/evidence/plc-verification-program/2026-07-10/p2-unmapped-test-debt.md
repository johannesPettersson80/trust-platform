# Unmapped Test Debt Report

Generator: `unmapped-test-debt v2`
Source revision: `f514021eef1395b1d6aed0f8a8f77eb67cd7b40a`
Generated: `2026-07-18T19:20:00+02:00`
Platform: `linux-x86_64`
Generated JSON SHA-256: `8af1ef51756563649ca2e3cb95c4434d1d72cbfd09cf7feeb390cfcc8db5fb59`
Input SHA-256: `sha256:9b0c7b6dd93e4a7197bb9100a8db7c53433d1f8e61c2b08c2ba8b9e4331dc2b2`

`complete` means the source inventory, exact catalog subtraction, and
reviewed mapped/nonmapping denominator partition all succeeded.

## Summary

- Scanner facts: 4035
- Mapped scanner facts: 254
- Unmapped scanner facts: 3781
- Reviewed nonmapping facts: 3781
- Unreviewed scanner facts: 0
- Denominator review SHA-256: `sha256:d997c825ed440ccf7981ea82a5f23a9bd3b06dd5fbfd8027bd189bc7288d4ae0`
- Generated-test catalog rows: 254
- Artifact catalog rows: 8
- Ignored unmapped facts: 23
- Conditional unmapped facts: 0
- Unreviewed debt fails this report: yes

| Source kind | Scanner facts | Mapped | Unmapped |
| --- | ---: | ---: | ---: |
| `conformance_case` | 21 | 21 | 0 |
| `fuzz_target` | 8 | 0 | 8 |
| `gate_script` | 29 | 0 | 29 |
| `github_workflow_job` | 30 | 0 | 30 |
| `rust_integration_test` | 1488 | 163 | 1325 |
| `rust_unit_test` | 1741 | 67 | 1674 |
| `structured_text_test` | 257 | 0 | 257 |
| `vscode_test` | 461 | 3 | 458 |

## Unmapped Scanner Facts

| Discovery ID | Source kind | Path | Name | Ignore state |
| --- | --- | --- | --- | --- |
| `DISC_DED48E736E3A27E72CF8` | `fuzz_target` | `fuzz/fuzz_targets/bytecode_container.rs` | `bytecode_container` | `not_ignored` |
| `DISC_2F1558433C4E38375499` | `fuzz_target` | `fuzz/fuzz_targets/hir_lowering.rs` | `hir_lowering` | `not_ignored` |
| `DISC_405FE9606792A984F799` | `fuzz_target` | `fuzz/fuzz_targets/hir_semantic.rs` | `hir_semantic` | `not_ignored` |
| `DISC_313C8C5D5404DD0AEFB0` | `fuzz_target` | `fuzz/fuzz_targets/hmi_payloads.rs` | `hmi_payloads` | `not_ignored` |
| `DISC_6EB6C1819FC67E761608` | `fuzz_target` | `fuzz/fuzz_targets/lsp_incremental.rs` | `lsp_incremental` | `not_ignored` |
| `DISC_7532206190B7F7EAE5F9` | `fuzz_target` | `fuzz/fuzz_targets/plcopen_xml.rs` | `plcopen_xml` | `not_ignored` |
| `DISC_B9E4C3E00C764E9D923B` | `fuzz_target` | `fuzz/fuzz_targets/runtime_config.rs` | `runtime_config` | `not_ignored` |
| `DISC_605EEF72F5DD36D7166C` | `fuzz_target` | `fuzz/fuzz_targets/syntax_parse.rs` | `syntax_parse` | `not_ignored` |
| `DISC_F5C98BD577899E0D29C1` | `gate_script` | `scripts/aggregate_st_test_flake_history.py` | `aggregate_st_test_flake_history` | `not_ignored` |
| `DISC_F9FB29C84C865FBB50F1` | `gate_script` | `scripts/architecture_external_safety_ast_grep_gate.sh` | `architecture_external_safety_ast_grep_gate` | `not_ignored` |
| `DISC_97EE7D808424A743C2A4` | `gate_script` | `scripts/architecture_external_safety_geiger_gate.sh` | `architecture_external_safety_geiger_gate` | `not_ignored` |
| `DISC_505636BB434FBF01DA6D` | `gate_script` | `scripts/check_gate_observability.py` | `check_gate_observability` | `not_ignored` |
| `DISC_9B0B303FB2CB432C77D6` | `gate_script` | `scripts/generate_release_gate_report.py` | `generate_release_gate_report` | `not_ignored` |
| `DISC_949CD663563705798270` | `gate_script` | `scripts/prepush_ci_gate.sh` | `prepush_ci_gate` | `not_ignored` |
| `DISC_66347619F5120E79546D` | `gate_script` | `scripts/runtime_boundary_fail_closed_ast_grep_gate.sh` | `runtime_boundary_fail_closed_ast_grep_gate` | `not_ignored` |
| `DISC_0FF3FB3A6EBC887FAE60` | `gate_script` | `scripts/runtime_cloud_security_profile_gate.sh` | `runtime_cloud_security_profile_gate` | `not_ignored` |
| `DISC_0FD65C4E8B310F5540B2` | `gate_script` | `scripts/runtime_comms_bench_gate.sh` | `runtime_comms_bench_gate` | `not_ignored` |
| `DISC_0D20084A8D2E56D2E979` | `gate_script` | `scripts/runtime_comms_conformance_gate.sh` | `runtime_comms_conformance_gate` | `not_ignored` |
| `DISC_3B168302C56C6632C0AB` | `gate_script` | `scripts/runtime_comms_fuzz_gate.sh` | `runtime_comms_fuzz_gate` | `not_ignored` |
| `DISC_C0087F7709DEE75AC84A` | `gate_script` | `scripts/runtime_device_in_loop_gate.sh` | `runtime_device_in_loop_gate` | `not_ignored` |
| `DISC_1FC8220958D8574188A9` | `gate_script` | `scripts/runtime_mesh_tls_stability_gate.sh` | `runtime_mesh_tls_stability_gate` | `not_ignored` |
| `DISC_A97FFD37D84D3397E253` | `gate_script` | `scripts/runtime_motion_example_bench_gate.sh` | `runtime_motion_example_bench_gate` | `not_ignored` |
| `DISC_DFD1BC59DE81E1E3DDBD` | `gate_script` | `scripts/runtime_safety_fail_closed_ast_grep_gate.sh` | `runtime_safety_fail_closed_ast_grep_gate` | `not_ignored` |
| `DISC_12BA2A15CA94233F4A35` | `gate_script` | `scripts/runtime_vm_bench_gate.sh` | `runtime_vm_bench_gate` | `not_ignored` |
| `DISC_8719AEF44DFFBA43F2A6` | `gate_script` | `scripts/runtime_vm_determinism_reliability_gate.sh` | `runtime_vm_determinism_reliability_gate` | `not_ignored` |
| `DISC_C6F8EB526D95DA5111CE` | `gate_script` | `scripts/runtime_vm_malformed_bytecode_fuzz_gate.sh` | `runtime_vm_malformed_bytecode_fuzz_gate` | `not_ignored` |
| `DISC_A6A56A75E4FFA72B3458` | `gate_script` | `scripts/salsa_fuzz_gate.sh` | `salsa_fuzz_gate` | `not_ignored` |
| `DISC_FC8C5E6C94A654730D6F` | `gate_script` | `scripts/salsa_hardening_perf_gate.sh` | `salsa_hardening_perf_gate` | `not_ignored` |
| `DISC_6963C4263D6C5C889504` | `gate_script` | `scripts/salsa_memory_gate.sh` | `salsa_memory_gate` | `not_ignored` |
| `DISC_2A3D1BF41A1424D3CF17` | `gate_script` | `scripts/salsa_miri_gate.sh` | `salsa_miri_gate` | `not_ignored` |
| `DISC_A4D3BFAE727696CF1764` | `gate_script` | `scripts/salsa_spike_gate.sh` | `salsa_spike_gate` | `not_ignored` |
| `DISC_A9C7C10C7114876B30B2` | `gate_script` | `scripts/unsafe_concurrency_geiger_gate.sh` | `unsafe_concurrency_geiger_gate` | `not_ignored` |
| `DISC_BD2C0185F2B39864BD9F` | `gate_script` | `scripts/unsafe_concurrency_miri_gate.sh` | `unsafe_concurrency_miri_gate` | `not_ignored` |
| `DISC_823641C6DB063FF9DFF1` | `gate_script` | `scripts/unsafe_concurrency_sanitizer_gate.sh` | `unsafe_concurrency_sanitizer_gate` | `not_ignored` |
| `DISC_F0E6B14CB78B33EBD959` | `gate_script` | `scripts/unsafe_concurrency_valgrind_gate.sh` | `unsafe_concurrency_valgrind_gate` | `not_ignored` |
| `DISC_9FA85AE5E0326DC399E6` | `gate_script` | `scripts/verification_metadata_gate.sh` | `verification_metadata_gate` | `not_ignored` |
| `DISC_C04DF07CD4649EF5B41B` | `gate_script` | `scripts/verification_report_gate.py` | `verification_report_gate` | `not_ignored` |
| `DISC_F0537B30EAC2EA70A015` | `github_workflow_job` | `.github/workflows/ci.yml` | `CI / architecture-safety` | `not_ignored` |
| `DISC_F5BA99A56A8ACF405D9E` | `github_workflow_job` | `.github/workflows/ci.yml` | `CI / browser-analysis` | `not_ignored` |
| `DISC_B250E91CEF7729F1DA08` | `github_workflow_job` | `.github/workflows/ci.yml` | `CI / clippy` | `not_ignored` |
| `DISC_45419D4DE05ECDFACE3D` | `github_workflow_job` | `.github/workflows/ci.yml` | `CI / conformance` | `not_ignored` |
| `DISC_C039EB4AC0BCC1D70240` | `github_workflow_job` | `.github/workflows/ci.yml` | `CI / docs` | `not_ignored` |
| `DISC_F1F4D6E2F5ABD3159120` | `github_workflow_job` | `.github/workflows/ci.yml` | `CI / editor-expansion` | `not_ignored` |
| `DISC_9A98BA16150EF4AF7EFD` | `github_workflow_job` | `.github/workflows/ci.yml` | `CI / fmt` | `not_ignored` |
| `DISC_A4E4D1B4B91030B942BE` | `github_workflow_job` | `.github/workflows/ci.yml` | `CI / mp001-parity` | `not_ignored` |
| `DISC_6221D7F7D33814D363FB` | `github_workflow_job` | `.github/workflows/ci.yml` | `CI / msrv` | `not_ignored` |
| `DISC_3DBDD721AE2AEA758B75` | `github_workflow_job` | `.github/workflows/ci.yml` | `CI / release-gate-report` | `not_ignored` |
| `DISC_AC66D9F7C57A787D2730` | `github_workflow_job` | `.github/workflows/ci.yml` | `CI / supply-chain` | `not_ignored` |
| `DISC_DC80DD9E764F13A86D6F` | `github_workflow_job` | `.github/workflows/ci.yml` | `CI / test` | `not_ignored` |
| `DISC_EAD3A1087B30D8FBAE21` | `github_workflow_job` | `.github/workflows/ci.yml` | `CI / version-release-guard` | `not_ignored` |
| `DISC_AF122FD96F5F78237430` | `github_workflow_job` | `.github/workflows/ci.yml` | `CI / vscode-extension` | `not_ignored` |
| `DISC_CCEB4E82D928C12FB0E6` | `github_workflow_job` | `.github/workflows/demo-pages.yml` | `Docs Pages / build` | `not_ignored` |
| `DISC_0C6190B196D300B91AD8` | `github_workflow_job` | `.github/workflows/demo-pages.yml` | `Docs Pages / deploy` | `not_ignored` |
| `DISC_3D272017765F5D73BCE4` | `github_workflow_job` | `.github/workflows/diagrams.yml` | `Render Diagrams / render` | `not_ignored` |
| `DISC_A35CF62023CA62312980` | `github_workflow_job` | `.github/workflows/docs-captures.yml` | `Docs Captures / refresh` | `not_ignored` |
| `DISC_8A5048EA5B2E3D3F24AF` | `github_workflow_job` | `.github/workflows/hmi-long-soak.yml` | `HMI Long Soak / hmi-soak` | `not_ignored` |
| `DISC_59726B8C5AB103FE2332` | `github_workflow_job` | `.github/workflows/nightly-reliability.yml` | `Nightly Reliability / reliability` | `not_ignored` |
| `DISC_E3CC7838268374AC3D36` | `github_workflow_job` | `.github/workflows/protocol-device-in-loop.yml` | `Protocol Device-In-The-Loop / protocol-device-in-loop` | `not_ignored` |
| `DISC_1CA69660B5971425B0A9` | `github_workflow_job` | `.github/workflows/release.yml` | `Release / build` | `not_ignored` |
| `DISC_DAA028DE0B613F42BE10` | `github_workflow_job` | `.github/workflows/release.yml` | `Release / publish` | `not_ignored` |
| `DISC_7ED37B60D2F64C092528` | `github_workflow_job` | `.github/workflows/release.yml` | `Release / release-preflight` | `not_ignored` |
| `DISC_1E2141EDC4C3681BC295` | `github_workflow_job` | `.github/workflows/release.yml` | `Release / runtime-vm-validation` | `not_ignored` |
| `DISC_27D47EADCB3629CA5E3A` | `github_workflow_job` | `.github/workflows/salsa-hardening.yml` | `Salsa Hardening / fuzz-extended-nightly` | `not_ignored` |
| `DISC_82FD54F3FCA28DB8AACE` | `github_workflow_job` | `.github/workflows/salsa-hardening.yml` | `Salsa Hardening / fuzz-smoke` | `not_ignored` |
| `DISC_D273DA1B1F1C3F010944` | `github_workflow_job` | `.github/workflows/salsa-hardening.yml` | `Salsa Hardening / memory-regression` | `not_ignored` |
| `DISC_184A90EEB01F34E82605` | `github_workflow_job` | `.github/workflows/salsa-hardening.yml` | `Salsa Hardening / miri-nightly` | `not_ignored` |
| `DISC_5BA1C91EA33A535ADB89` | `github_workflow_job` | `.github/workflows/verification-gate.yml` | `Verification Gate / verification-report` | `not_ignored` |
| `DISC_F22B2C0601B2CEF89B24` | `rust_integration_test` | `crates/trust-dev/tests/cli_smoke.rs` | `trust_dev_help_surfaces_workbench_commands` | `not_ignored` |
| `DISC_CB317388FEC6A3A731FC` | `rust_integration_test` | `crates/trust-dev/tests/cli_smoke.rs` | `trust_dev_subcommand_help_is_stable` | `not_ignored` |
| `DISC_8AA07FE10410373FC27E` | `rust_integration_test` | `crates/trust-hir/tests/declaration_catalog.rs` | `declaration_catalog_exposes_qualified_declarations_roles_and_sources` | `not_ignored` |
| `DISC_EB3EE710B80C9D3A6147` | `rust_integration_test` | `crates/trust-hir/tests/declaration_catalog.rs` | `declaration_catalog_marks_project_imports_and_translated_type_identity` | `not_ignored` |
| `DISC_623217085BD92965FE67` | `rust_integration_test` | `crates/trust-hir/tests/generic_types.rs` | `any_groups` | `not_ignored` |
| `DISC_39AB329C1EF74B96CF4F` | `rust_integration_test` | `crates/trust-hir/tests/iec_table_coverage.rs` | `iec_table_coverage_report` | `not_ignored` |
| `DISC_8B3E808EDBF93ECB63E3` | `rust_integration_test` | `crates/trust-hir/tests/namespaces.rs` | `cross_file_using_ambiguous_name_reports_cannot_resolve_without_choosing_import` | `not_ignored` |
| `DISC_317F0DEDA5DF7A3D08F8` | `rust_integration_test` | `crates/trust-hir/tests/namespaces.rs` | `iec_table64` | `not_ignored` |
| `DISC_476A021591262987DFA1` | `rust_integration_test` | `crates/trust-hir/tests/namespaces.rs` | `iec_table66` | `not_ignored` |
| `DISC_69D26B173E1254C6CC59` | `rust_integration_test` | `crates/trust-hir/tests/namespaces.rs` | `namespace_global_supports_qualified_reads_and_writes` | `not_ignored` |
| `DISC_EF8A426CE4FC01441600` | `rust_integration_test` | `crates/trust-hir/tests/namespaces.rs` | `namespace_qualified_type_reference_matches_type_check_resolution` | `not_ignored` |
| `DISC_9232DE2EB3C0FBDDFADD` | `rust_integration_test` | `crates/trust-hir/tests/namespaces.rs` | `namespaced_extends_member_resolution_prefers_scoped_base_over_global` | `not_ignored` |
| `DISC_4CD9432FAACD75BDDCFA` | `rust_integration_test` | `crates/trust-hir/tests/namespaces.rs` | `namespaced_type_reference_prefers_scoped_type_over_global_bare_name` | `not_ignored` |
| `DISC_50F86E61A7A2E56C397F` | `rust_integration_test` | `crates/trust-hir/tests/namespaces.rs` | `same_named_pou_constants_in_different_namespaces_do_not_collide` | `not_ignored` |
| `DISC_B03A434068FF9092FD36` | `rust_integration_test` | `crates/trust-hir/tests/namespaces.rs` | `same_named_pou_constants_in_different_namespaces_keep_distinct_values` | `not_ignored` |
| `DISC_AF8BD8921B2A937C1313` | `rust_integration_test` | `crates/trust-hir/tests/namespaces.rs` | `using_ambiguous_assignment_target_reports_one_primary` | `not_ignored` |
| `DISC_E091D70576BE1F5753AF` | `rust_integration_test` | `crates/trust-hir/tests/namespaces.rs` | `using_ambiguous_name_reports_cannot_resolve_without_choosing_candidate` | `not_ignored` |
| `DISC_5DD1A0E7DA639EF0052E` | `rust_integration_test` | `crates/trust-hir/tests/namespaces.rs` | `using_ambiguous_value_reports_cannot_resolve_without_undefined_variable` | `not_ignored` |
| `DISC_CAA955CD2012CC2649C7` | `rust_integration_test` | `crates/trust-hir/tests/semantic_interfaces.rs` | `namespaced_function_block_implements_sibling_interface_not_global_bare_name` | `not_ignored` |
| `DISC_E6CB77436AFC77B01994` | `rust_integration_test` | `crates/trust-hir/tests/semantic_interfaces.rs` | `namespaced_interface_extends_sibling_interface_not_global_bare_name` | `not_ignored` |
| `DISC_B6EC367C2AEC080CD7D4` | `rust_integration_test` | `crates/trust-hir/tests/semantic_interfaces.rs` | `test_cross_file_concrete_implements_interface_assignment` | `not_ignored` |
| `DISC_260C548E20CAA6F8A4C3` | `rust_integration_test` | `crates/trust-hir/tests/semantic_interfaces.rs` | `test_function_block_extends_cycle_error` | `not_ignored` |
| `DISC_3337D09B89CC0C03B90F` | `rust_integration_test` | `crates/trust-hir/tests/semantic_interfaces.rs` | `test_function_block_extends_final_class_error` | `not_ignored` |
| `DISC_4966104D5CA163226F50` | `rust_integration_test` | `crates/trust-hir/tests/semantic_interfaces.rs` | `test_function_block_extends_invalid_type_error` | `not_ignored` |
| `DISC_08261C6721F17E842D4A` | `rust_integration_test` | `crates/trust-hir/tests/semantic_interfaces.rs` | `test_interface_conformance_cross_file` | `not_ignored` |
| `DISC_F4A699ACCEF7CBED8A17` | `rust_integration_test` | `crates/trust-hir/tests/semantic_interfaces.rs` | `test_interface_extends_cycle_error` | `not_ignored` |
| `DISC_7BF982D21B8FD81719BD` | `rust_integration_test` | `crates/trust-hir/tests/semantic_interfaces.rs` | `test_interface_extends_non_interface_error` | `not_ignored` |
| `DISC_0D7FF02AEE92435AA3FC` | `rust_integration_test` | `crates/trust-hir/tests/semantic_interfaces.rs` | `test_interface_missing_method_allowed_on_abstract_class` | `not_ignored` |
| `DISC_4DC9E89EBC56F7B8B9D8` | `rust_integration_test` | `crates/trust-hir/tests/semantic_interfaces.rs` | `test_interface_missing_method_error` | `not_ignored` |
| `DISC_B185F1B11295C285D515` | `rust_integration_test` | `crates/trust-hir/tests/semantic_interfaces.rs` | `test_interface_property_accessor_error` | `not_ignored` |
| `DISC_61D8EE489849A686D304` | `rust_integration_test` | `crates/trust-hir/tests/semantic_interfaces.rs` | `test_interface_signature_mismatch_error` | `not_ignored` |
| `DISC_C2E7CD2BC5C00EFF830C` | `rust_integration_test` | `crates/trust-hir/tests/semantic_interfaces.rs` | `test_interface_visibility_error` | `not_ignored` |
| `DISC_88246CDAABC1FBB361FD` | `rust_integration_test` | `crates/trust-hir/tests/semantic_name_resolution.rs` | `test_duplicate_declaration` | `not_ignored` |
| `DISC_6A8F0D0E93C0D592A8C1` | `rust_integration_test` | `crates/trust-hir/tests/semantic_name_resolution.rs` | `test_invalid_identifier` | `not_ignored` |
| `DISC_5B9269CD368A640ABFEC` | `rust_integration_test` | `crates/trust-hir/tests/semantic_name_resolution.rs` | `test_multiple_variables_same_type` | `not_ignored` |
| `DISC_E1D35246AE82A7ED5E2A` | `rust_integration_test` | `crates/trust-hir/tests/semantic_name_resolution.rs` | `test_property_name_with_underscore_backing_var` | `not_ignored` |
| `DISC_E85F3FA2978297950949` | `rust_integration_test` | `crates/trust-hir/tests/semantic_name_resolution.rs` | `test_undefined_variable` | `not_ignored` |
| `DISC_2964988170D20F8E70E9` | `rust_integration_test` | `crates/trust-hir/tests/semantic_name_resolution.rs` | `test_variable_in_scope` | `not_ignored` |
| `DISC_17D32E7F13E3462FBD21` | `rust_integration_test` | `crates/trust-hir/tests/semantic_name_resolution.rs` | `test_variable_in_scope_test_function_block` | `not_ignored` |
| `DISC_ACF6575D2219D1F346C7` | `rust_integration_test` | `crates/trust-hir/tests/semantic_name_resolution.rs` | `test_variable_in_scope_test_program` | `not_ignored` |
| `DISC_0352C8114517C76C75FE` | `rust_integration_test` | `crates/trust-hir/tests/semantic_parameters.rs` | `test_function_block_with_outputs` | `not_ignored` |
| `DISC_EA71618D4952FDAF7F83` | `rust_integration_test` | `crates/trust-hir/tests/semantic_parameters.rs` | `test_function_parameters_collected` | `not_ignored` |
| `DISC_CAB4E15101E18208E1DD` | `rust_integration_test` | `crates/trust-hir/tests/semantic_scope.rs` | `test_action_body_type_checked` | `not_ignored` |
| `DISC_E7CE5E419F7725A8472F` | `rust_integration_test` | `crates/trust-hir/tests/semantic_scope.rs` | `test_inherited_member_resolution` | `not_ignored` |
| `DISC_BD560EA6B7A32C1886E3` | `rust_integration_test` | `crates/trust-hir/tests/semantic_scope.rs` | `test_internal_member_access_inside_namespace_ok` | `not_ignored` |
| `DISC_6E9054996B3466A9C5DF` | `rust_integration_test` | `crates/trust-hir/tests/semantic_scope.rs` | `test_internal_member_access_outside_namespace_error` | `not_ignored` |
| `DISC_864A8087002B21079481` | `rust_integration_test` | `crates/trust-hir/tests/semantic_scope.rs` | `test_method_scope_resolution` | `not_ignored` |
| `DISC_6BC58413F8B850D0FFD8` | `rust_integration_test` | `crates/trust-hir/tests/semantic_scope.rs` | `test_private_member_access_error` | `not_ignored` |
| `DISC_191BB5A62DA24F2032D3` | `rust_integration_test` | `crates/trust-hir/tests/semantic_scope.rs` | `test_property_missing_getter_error` | `not_ignored` |
| `DISC_6EC3AF14BCC339E5C85B` | `rust_integration_test` | `crates/trust-hir/tests/semantic_scope.rs` | `test_property_setter_only_assignment_ok` | `not_ignored` |
| `DISC_317B57AC00EF0F86F0B9` | `rust_integration_test` | `crates/trust-hir/tests/semantic_scope.rs` | `test_scope_resolution` | `not_ignored` |
| `DISC_4D1F763D18DBB4DA3CD4` | `rust_integration_test` | `crates/trust-hir/tests/semantic_scope.rs` | `test_super_field_access` | `not_ignored` |
| `DISC_AD21C818FED907C8EE3C` | `rust_integration_test` | `crates/trust-hir/tests/semantic_scope.rs` | `test_test_function_block_scope_resolution` | `not_ignored` |
| `DISC_591D1F80D69EE11A5354` | `rust_integration_test` | `crates/trust-hir/tests/semantic_scope.rs` | `test_test_program_scope_resolution` | `not_ignored` |
| `DISC_0E5F84AD4BA39757FCC5` | `rust_integration_test` | `crates/trust-hir/tests/semantic_standard_functions.rs` | `test_assert_equal_accepts_char_and_wchar` | `not_ignored` |
| `DISC_18C281121C8F2A2D7F07` | `rust_integration_test` | `crates/trust-hir/tests/semantic_standard_functions.rs` | `test_assert_equal_requires_comparable_types` | `not_ignored` |
| `DISC_627DE6D71C0A08570A9A` | `rust_integration_test` | `crates/trust-hir/tests/semantic_standard_functions.rs` | `test_assert_equal_wrong_arity` | `not_ignored` |
| `DISC_878AE6E89F760D397C78` | `rust_integration_test` | `crates/trust-hir/tests/semantic_standard_functions.rs` | `test_assert_greater_requires_comparable_types` | `not_ignored` |
| `DISC_A52B459EE90798F4BFC0` | `rust_integration_test` | `crates/trust-hir/tests/semantic_standard_functions.rs` | `test_assert_less_or_equal_wrong_arity` | `not_ignored` |
| `DISC_00FE726C106CD316AFAD` | `rust_integration_test` | `crates/trust-hir/tests/semantic_standard_functions.rs` | `test_assert_near_requires_numeric_types` | `not_ignored` |
| `DISC_DD4BF499E87F5702426C` | `rust_integration_test` | `crates/trust-hir/tests/semantic_standard_functions.rs` | `test_assert_standard_functions_ok` | `not_ignored` |
| `DISC_891331AB23256646E56A` | `rust_integration_test` | `crates/trust-hir/tests/semantic_standard_functions.rs` | `test_assert_true_requires_bool` | `not_ignored` |
| `DISC_3230112186D6D25A3FEB` | `rust_integration_test` | `crates/trust-hir/tests/semantic_standard_functions.rs` | `test_continue_outside_loop_error` | `not_ignored` |
| `DISC_DDD068CC588688F7B717` | `rust_integration_test` | `crates/trust-hir/tests/semantic_standard_functions.rs` | `test_converted_numeric_expression_accepts_bit_to_numeric_scaling` | `not_ignored` |
| `DISC_663E06F446673A48B34C` | `rust_integration_test` | `crates/trust-hir/tests/semantic_standard_functions.rs` | `test_duplicate_case_label_error` | `not_ignored` |
| `DISC_983CBB9F52DE6C7EA64C` | `rust_integration_test` | `crates/trust-hir/tests/semantic_standard_functions.rs` | `test_duplicate_label_declaration_error` | `not_ignored` |
| `DISC_7323CC5D9F0D99E4FF3D` | `rust_integration_test` | `crates/trust-hir/tests/semantic_standard_functions.rs` | `test_exit_outside_loop_error` | `not_ignored` |
| `DISC_4445FA250BC640392A56` | `rust_integration_test` | `crates/trust-hir/tests/semantic_standard_functions.rs` | `test_for_loop_bounds_type_mismatch_error` | `not_ignored` |
| `DISC_D16DDAE93185E8E31371` | `rust_integration_test` | `crates/trust-hir/tests/semantic_standard_functions.rs` | `test_for_loop_control_var_modified_error` | `not_ignored` |
| `DISC_CE23427A26330185FA03` | `rust_integration_test` | `crates/trust-hir/tests/semantic_standard_functions.rs` | `test_is_valid_bcd_rejects_bool` | `not_ignored` |
| `DISC_7A9765F1F0939C67DD38` | `rust_integration_test` | `crates/trust-hir/tests/semantic_standard_functions.rs` | `test_is_valid_bcd_rejects_non_bit_string` | `not_ignored` |
| `DISC_735D1722DAF91AF18050` | `rust_integration_test` | `crates/trust-hir/tests/semantic_standard_functions.rs` | `test_is_valid_requires_real_argument` | `not_ignored` |
| `DISC_357364B5F017E40114CD` | `rust_integration_test` | `crates/trust-hir/tests/semantic_standard_functions.rs` | `test_jmp_label_ok` | `not_ignored` |
| `DISC_65CDD30C08848EC032A9` | `rust_integration_test` | `crates/trust-hir/tests/semantic_standard_functions.rs` | `test_jmp_unknown_label_error` | `not_ignored` |
| `DISC_9B2B2CFFDB0CAE77D5D9` | `rust_integration_test` | `crates/trust-hir/tests/semantic_standard_functions.rs` | `test_new_delete_calls_ok` | `not_ignored` |
| `DISC_1794E9163BCDE842A3E7` | `rust_integration_test` | `crates/trust-hir/tests/semantic_standard_functions.rs` | `test_new_requires_type_error` | `not_ignored` |
| `DISC_4692A562D9C8CBB0FB94` | `rust_integration_test` | `crates/trust-hir/tests/semantic_standard_functions.rs` | `test_standard_conversion_functions` | `not_ignored` |
| `DISC_91E6375542C2C252AB6B` | `rust_integration_test` | `crates/trust-hir/tests/semantic_standard_functions.rs` | `test_standard_function_type_mismatch` | `not_ignored` |
| `DISC_43C5B1FEB74DB222685B` | `rust_integration_test` | `crates/trust-hir/tests/semantic_standard_functions.rs` | `test_standard_function_wrong_arity` | `not_ignored` |
| `DISC_8E5985C4581B11E36E88` | `rust_integration_test` | `crates/trust-hir/tests/semantic_standard_functions.rs` | `test_standard_numeric_and_bitwise_functions` | `not_ignored` |
| `DISC_C0593BCD93E5C722C8A0` | `rust_integration_test` | `crates/trust-hir/tests/semantic_standard_functions.rs` | `test_standard_string_and_time_functions` | `not_ignored` |
| `DISC_69DB613F94392509F186` | `rust_integration_test` | `crates/trust-hir/tests/semantic_standard_functions.rs` | `test_standard_validate_functions` | `not_ignored` |
| `DISC_181BE6A42CEEAC4A3CA8` | `rust_integration_test` | `crates/trust-hir/tests/semantic_standard_functions.rs` | `test_typed_conversion_accepts_positional_outer_named_inner_call` | `not_ignored` |
| `DISC_0A5ACC1E184595B117F3` | `rust_integration_test` | `crates/trust-hir/tests/semantic_standard_functions.rs` | `test_using_directive_nested_namespace_not_imported` | `not_ignored` |
| `DISC_32B113369BE0E5AF3F95` | `rust_integration_test` | `crates/trust-hir/tests/semantic_standard_functions.rs` | `test_using_directive_resolves_type` | `not_ignored` |
| `DISC_4E4AC1E5F94C1FC77609` | `rust_integration_test` | `crates/trust-hir/tests/semantic_standard_functions.rs` | `test_using_directive_unknown_namespace_error` | `not_ignored` |
| `DISC_8A5E65EAEF4F08036DD2` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/array_wildcard_compatibility.rs` | `concrete_array_param_still_rejects_mismatched_bounds` | `not_ignored` |
| `DISC_285B9436CE0FDA88577E` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/array_wildcard_compatibility.rs` | `parse_array_star_in_regular_var_is_rejected` | `not_ignored` |
| `DISC_AE6E3D5AA4597CFD61C2` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/array_wildcard_compatibility.rs` | `parse_array_star_requires_single_dimension` | `not_ignored` |
| `DISC_CDE6C28C2FE2B22E490F` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/array_wildcard_compatibility.rs` | `pointer_to_concrete_array_still_rejects_mismatched_bounds` | `not_ignored` |
| `DISC_387A16F1CA5BE0E19F0C` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/array_wildcard_compatibility.rs` | `pointer_to_pointer_explicit_mismatch_diagnostic` | `not_ignored` |
| `DISC_187A04A789AD9B0005F5` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/array_wildcard_compatibility.rs` | `pointer_to_wildcard_array_accepts_adr_of_any_sized_array` | `not_ignored` |
| `DISC_6DF338CAC731E11F8304` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/array_wildcard_compatibility.rs` | `wildcard_array_param_accepts_any_concrete_bounds` | `not_ignored` |
| `DISC_C71F38BBC2871DE9F57C` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/array_wildcard_compatibility.rs` | `wildcard_array_param_rejects_element_mismatch` | `not_ignored` |
| `DISC_A1F314CA70078621F788` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/array_wildcard_compatibility.rs` | `wildcard_array_returned_is_rejected` | `not_ignored` |
| `DISC_EA519348BF8D386CC212` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_array_bounds_constant_expression` | `not_ignored` |
| `DISC_3CFA685E6C5E71E23D97` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_array_bounds_enum_values` | `not_ignored` |
| `DISC_933D6414381B36343249` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_array_index_const_eval_error_reports_primary_diagnostic` | `not_ignored` |
| `DISC_F25FF98FCAA0C51E01B4` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_array_index_dimension_too_few` | `not_ignored` |
| `DISC_A25BB3FEB5C48857A3A0` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_array_index_dimension_too_many` | `not_ignored` |
| `DISC_266BDE9C118EC67BB33F` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_array_index_literal_out_of_bounds` | `not_ignored` |
| `DISC_A723C3C42DD87072CBAB` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_array_index_requires_integer` | `not_ignored` |
| `DISC_148B9004958C094D1ADE` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_array_index_subrange_out_of_bounds` | `not_ignored` |
| `DISC_916E8E1AEE73915D81C8` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_array_index_subrange_within_bounds` | `not_ignored` |
| `DISC_20450D5B3C155324F612` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_assignment_to_function_name_error` | `not_ignored` |
| `DISC_D93EC5C1154A43BAAA69` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_assignment_to_this_error` | `not_ignored` |
| `DISC_D44B1F395E7B10D63EB7` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_assignment_unknown_source_suppression_has_primary_diagnostic` | `not_ignored` |
| `DISC_B2F352B41F805B1CFF92` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_at_wildcard_not_allowed_in_var_input` | `not_ignored` |
| `DISC_906D16FC0F2F6AA1C891` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_at_wildcard_requires_var_config` | `not_ignored` |
| `DISC_7F10D036AA57DD9CA059` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_at_wildcard_var_config_mapping_ok` | `not_ignored` |
| `DISC_C2284EC7ED5A57B09C4C` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_at_wildcard_var_config_requires_full_address` | `not_ignored` |
| `DISC_A56798A166E9CB026B2B` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_bare_configuration_global_access_resolves_across_files` | `not_ignored` |
| `DISC_702676C840E67C86A5CF` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_bare_global_access_is_accepted_across_pou_kinds` | `not_ignored` |
| `DISC_78FDED6BD0A910665414` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_binary_unknown_operand_suppression_has_primary_diagnostic` | `not_ignored` |
| `DISC_80C19AA8855CBC688A50` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_class_method_bare_missing_name_is_rejected` | `not_ignored` |
| `DISC_F163E5391F5ECFDAF74C` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_cross_file_global_import_collision_reports_duplicate` | `not_ignored` |
| `DISC_ADA1DFE377B61260A27B` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_direct_address_binding_recorded` | `not_ignored` |
| `DISC_D838E1A80019B5529AC1` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_direct_address_type_mismatch` | `not_ignored` |
| `DISC_5A99CAF69A09AC2CBBF3` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_direct_address_usage` | `not_ignored` |
| `DISC_B21F5C37F39989E6BB2E` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_function_assignment_sets_return_value` | `not_ignored` |
| `DISC_839D7AD4F8CBEA3493F9` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_function_bare_missing_name_is_rejected` | `not_ignored` |
| `DISC_0405AC69FDA9A33D8EC7` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_function_bare_return_allowed_after_assigning_return_target_on_same_path` | `not_ignored` |
| `DISC_2687F2B4F5BD983D1FEC` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_function_bare_return_rejected_when_return_target_not_definitely_assigned` | `not_ignored` |
| `DISC_982FDFA05912B6129986` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_function_block_bare_missing_name_is_rejected` | `not_ignored` |
| `DISC_9662CDDCC80EF91F7944` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_function_missing_return_value` | `not_ignored` |
| `DISC_3E30A5FF2E4FF97C2996` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_function_return_expr_sets_return_value` | `not_ignored` |
| `DISC_46AC5FD9A1E8B9AD8D17` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_function_return_variable_can_be_read_inside_function` | `not_ignored` |
| `DISC_E13B12CDFD4026B26A05` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_index_unknown_base_suppression_has_primary_diagnostic` | `not_ignored` |
| `DISC_BEE3D8D8CDCAC399298F` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_invalid_assignment_target_field_of_call` | `not_ignored` |
| `DISC_7D11879D4D25CE646C91` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_method_return_variable_can_be_read_inside_method` | `not_ignored` |
| `DISC_58657131263ADA238CEA` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_numeric_widening_assignment_uses_compatibility_matrix` | `not_ignored` |
| `DISC_35641259617B088E7110` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_program_bare_missing_name_is_rejected` | `not_ignored` |
| `DISC_553CD15CB61A4C5EE0BC` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_program_config_wrong_kind_type_reports_diagnostic` | `not_ignored` |
| `DISC_ED48D3A6C29D0134C466` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_program_with_unknown_task_error` | `not_ignored` |
| `DISC_DEAB6C204895ED7E684F` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_property_get_return_type_checked` | `not_ignored` |
| `DISC_D254A54EF3C1DDB3A633` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_property_set_rejects_return_value` | `not_ignored` |
| `DISC_6E73437927ED1FC5CF7D` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_property_without_setter_assignment_error` | `not_ignored` |
| `DISC_A46A89E48C5A55B24EDE` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_string_indexing_is_allowed` | `not_ignored` |
| `DISC_B0A03B1D1843D11B16EF` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_string_indexing_requires_single_index` | `not_ignored` |
| `DISC_676E12E0C907FAD3DC06` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_subrange_assignment_const_eval_error_reports_primary_diagnostic` | `not_ignored` |
| `DISC_8ED6C53BA67D63DC82C5` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_task_interval_requires_time_literal` | `not_ignored` |
| `DISC_4579522B74850B18E113` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_task_missing_priority_error` | `not_ignored` |
| `DISC_C64638DE898B4D38A929` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_task_single_requires_bool_literal` | `not_ignored` |
| `DISC_F49C57548B9263F3DA63` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_unary_unknown_operand_suppression_has_primary_diagnostic` | `not_ignored` |
| `DISC_6009B09186D471879FE3` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_var_access_global_name_collision_reports_duplicate_and_ambiguous` | `not_ignored` |
| `DISC_BCEDF6BF0C7B94A24FE3` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_var_access_read_only_rejects_assignment` | `not_ignored` |
| `DISC_286D45125033A2950669` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_var_access_type_mismatch` | `not_ignored` |
| `DISC_FE271CA2643E08F31F5B` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_var_access_undefined_target_error` | `not_ignored` |
| `DISC_CAB6C998CEDE23859D69` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_var_config_cross_file_program_instance_target_resolves_after_project_merge` | `not_ignored` |
| `DISC_9D7A3396634285D05552` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_var_config_duplicate_program_instance_name_is_ambiguous` | `not_ignored` |
| `DISC_BE7699ECA8D48C582DAA` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_var_config_nested_access` | `not_ignored` |
| `DISC_5C205C60F0B7B960B40B` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_var_config_rejects_constant_init` | `not_ignored` |
| `DISC_521C418BB5C7591125BB` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_var_config_type_mismatch` | `not_ignored` |
| `DISC_7C576DDA927BE34C0078` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_var_config_undefined_target_error` | `not_ignored` |
| `DISC_1731BB0F53C1CE70FC56` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_var_constant_retain_conflict` | `not_ignored` |
| `DISC_409C18862E9FA10313E0` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_var_external_matches_program_scoped_global` | `not_ignored` |
| `DISC_135BEAA8BE8CC2B1CEA6` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_var_external_missing_global` | `not_ignored` |
| `DISC_2143CC87349EE8431CFA` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_var_external_rejects_initializer` | `not_ignored` |
| `DISC_EE2E09D0036806381701` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_var_external_requires_constant_for_global_constant` | `not_ignored` |
| `DISC_64BCE9DC108AF3F643B5` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_var_external_type_mismatch` | `not_ignored` |
| `DISC_291FF7F6CDF6929C30FD` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_var_input_assignment_error` | `not_ignored` |
| `DISC_890955589321461672C0` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_var_persistent_allowed` | `not_ignored` |
| `DISC_D95E7B21AF31C6F8F95A` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_var_retain_non_retain_conflict` | `not_ignored` |
| `DISC_E5E2384B651DDA5B5C7E` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | `test_var_retain_not_allowed_in_in_out` | `not_ignored` |
| `DISC_320B8B612E00D578B1EF` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs` | `test_boolean_condition_ok` | `not_ignored` |
| `DISC_63E20FCA299BB19B3CDC` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs` | `test_boolean_condition_required` | `not_ignored` |
| `DISC_D3B656D6EE809158ADAA` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs` | `test_constant_modification` | `not_ignored` |
| `DISC_3616DA1D8CE0C84A804C` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs` | `test_constant_struct_field_modification` | `not_ignored` |
| `DISC_AD314494CD5DF8A645EE` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs` | `test_contextual_int_literal_assignment` | `not_ignored` |
| `DISC_99F818A0DB55C307039A` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs` | `test_contextual_int_literal_return` | `not_ignored` |
| `DISC_42DF0CA426C392ED63B1` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs` | `test_contextual_real_literal_assignment` | `not_ignored` |
| `DISC_91EF5CC65F82D5BEC113` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs` | `test_contextual_real_literal_return` | `not_ignored` |
| `DISC_2C44A6BD090A298A986F` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs` | `test_cyclomatic_complexity_warning` | `not_ignored` |
| `DISC_81F50F11BC9CEA912E5A` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs` | `test_float_equality_warning` | `not_ignored` |
| `DISC_D8884F6D99A8815BD78E` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs` | `test_function_block_used_as_type_no_unused_pou_warning` | `not_ignored` |
| `DISC_2DFEDFEC29F17FE1112D` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs` | `test_implicit_conversion_warning` | `not_ignored` |
| `DISC_0403B98FE1234DCCE01C` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs` | `test_integer_equality_has_no_float_warning` | `not_ignored` |
| `DISC_05823154BDE8023B60B5` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs` | `test_literal_division_by_zero_warning` | `not_ignored` |
| `DISC_8C9C4E93BD3D4BC00356` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs` | `test_nondeterministic_io_warning` | `not_ignored` |
| `DISC_D75E73531DFCD61CAE79` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs` | `test_nondeterministic_time_date_warning` | `not_ignored` |
| `DISC_1AAB9E70E67BABDE2A77` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs` | `test_nonliteral_division_has_no_zero_literal_warning` | `not_ignored` |
| `DISC_73A6D8AF8E73403C4CB3` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs` | `test_real_literal_in_real_arithmetic` | `not_ignored` |
| `DISC_03D27C2E887CA2C0B7CC` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs` | `test_real_literal_in_standard_numeric_function` | `not_ignored` |
| `DISC_6C61B6311AF16C7AC595` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs` | `test_shared_global_task_hazard_single_task_no_warning` | `not_ignored` |
| `DISC_50C8A203171136A8E2F1` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs` | `test_shared_global_task_hazard_warning` | `not_ignored` |
| `DISC_DEFB3533B646090DEA05` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs` | `test_subrange_assignment_in_range` | `not_ignored` |
| `DISC_44D2AECEE13EB41E3AA0` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs` | `test_subrange_assignment_out_of_range` | `not_ignored` |
| `DISC_C838D2A471BDA2E07343` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs` | `test_subrange_bounds_invalid_order` | `not_ignored` |
| `DISC_7F6536CCC3214E277F47` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs` | `test_subrange_bounds_undefined_names_report_primary_diagnostic` | `not_ignored` |
| `DISC_7FA0B63348DB21C29575` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs` | `test_unreachable_code_warning` | `not_ignored` |
| `DISC_5B9E394D6F14D6EB075C` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs` | `test_unreachable_elsif_false_branch_warning` | `not_ignored` |
| `DISC_30A0436BC3442442B59F` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs` | `test_unreachable_if_false_branch_warning` | `not_ignored` |
| `DISC_84C79FF30C860C65E495` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs` | `test_unused_parameter_warning` | `not_ignored` |
| `DISC_F870FD664CEADFFE9F1C` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs` | `test_unused_pou_warning` | `not_ignored` |
| `DISC_A7E81F7AF274C9B3DB2A` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs` | `test_unused_variable_warning` | `not_ignored` |
| `DISC_01ECCC4C3A484770CBE4` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs` | `test_used_function_no_unused_pou_warning` | `not_ignored` |
| `DISC_E7026BA173A548514365` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/basics_and_warnings.rs` | `test_var_config_marks_symbol_used_across_files` | `not_ignored` |
| `DISC_FC37E5CDDECDA03F76F4` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_binary_operator_precedence` | `not_ignored` |
| `DISC_910A76F9F6474D5CB95C` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_call_argument_unknown_type_suppression_has_primary_diagnostic` | `not_ignored` |
| `DISC_8A34998BB82A7BE12F75` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_call_rejects_ref_assign_argument` | `not_ignored` |
| `DISC_31A7A59BD2089A29CC68` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_case_enum_exhaustive_no_warning` | `not_ignored` |
| `DISC_94DC77ECFCD32D9460D1` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_case_enum_label_ok` | `not_ignored` |
| `DISC_8675DBEE39BD40CD94D0` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_case_label_requires_literal_or_constant` | `not_ignored` |
| `DISC_3722DEE9DEC0CF960DCD` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_case_missing_else_warning` | `not_ignored` |
| `DISC_7A3D0E16B75B231CBC47` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_case_selector_requires_elementary` | `not_ignored` |
| `DISC_A0B21B49B9F11C8B6276` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_case_string_label_ok` | `not_ignored` |
| `DISC_B63F829A04E4F7623FF5` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_case_string_subrange_rejected` | `not_ignored` |
| `DISC_6A9CD62A4367A22AA671` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_case_subrange_requires_literal_bounds` | `not_ignored` |
| `DISC_6A13B0BA6CB4B05BAA6D` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_case_wstring_label_ok` | `not_ignored` |
| `DISC_E34583F8A8710C3004C3` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_fixed_length_string_comparison_operators_ok` | `not_ignored` |
| `DISC_12FBBA867087892C33D8` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_for_loop_bounds_integer` | `not_ignored` |
| `DISC_9AD3A0A5801D87347F2A` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_formal_call_allows_missing_arguments` | `not_ignored` |
| `DISC_11368D137023990F666F` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_formal_call_duplicate_parameter_error` | `not_ignored` |
| `DISC_A25C195E057B8A779695` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_formal_call_requires_in_out_binding` | `not_ignored` |
| `DISC_CCF2E71745750928E44A` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_formal_call_unknown_parameter_error` | `not_ignored` |
| `DISC_E32C0305FC5026249B23` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_function_block_instance_call` | `not_ignored` |
| `DISC_DFF7EBFC812B52E29D88` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_infix_ampersand_matches_and_for_bit_strings` | `not_ignored` |
| `DISC_0F4E67F7778399AB29C5` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_infix_bitwise_any_bit_expressions_are_allowed` | `not_ignored` |
| `DISC_5620EB02D5DB2632E895` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_infix_bitwise_mixed_width_results_cannot_shrink_silently` | `not_ignored` |
| `DISC_C52DB304C3322CDCC99C` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_mitsubishi_edge_alias_function_blocks` | `not_ignored` |
| `DISC_2F419A3897A77158E368` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_named_argument_order` | `not_ignored` |
| `DISC_FA2E0A43EF999F848775` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_named_argument_order_allows_positional_first` | `not_ignored` |
| `DISC_D0C160614872004F05DB` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_non_formal_call_allows_output_positional` | `not_ignored` |
| `DISC_372666A5A2ECF4DCFF6D` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_non_formal_call_rejects_en_eno_positional` | `not_ignored` |
| `DISC_E759EBA0E631D4B62400` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_non_formal_call_requires_complete_arguments` | `not_ignored` |
| `DISC_1E6661FF05BEC9165CB9` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_non_formal_call_skips_en_eno` | `not_ignored` |
| `DISC_B0D122E021CDE47BCF10` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_output_connection_rejects_input_parameter` | `not_ignored` |
| `DISC_F60D99D8E46622A7C966` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_output_parameter_connection_ok` | `not_ignored` |
| `DISC_D2291154CC389C0542E7` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_output_parameter_requires_arrow` | `not_ignored` |
| `DISC_56E8298D65AF7EFA2521` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_standard_bistable_function_block_type_error` | `not_ignored` |
| `DISC_2B94DB972E5F92A17B67` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_standard_bistable_function_blocks` | `not_ignored` |
| `DISC_47691F18637E26462AFB` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_standard_bit_unknown_argument_suppression_has_primary_diagnostic` | `not_ignored` |
| `DISC_7B978F576B59F641A0D8` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_standard_counter_function_block_type_error` | `not_ignored` |
| `DISC_08EE2DE5F62843CCF172` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_standard_counter_function_blocks` | `not_ignored` |
| `DISC_B763F04D6AE6F8361438` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_standard_edge_function_blocks` | `not_ignored` |
| `DISC_C3770F55FA1E6C13D79A` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_standard_numeric_unknown_argument_suppression_has_primary_diagnostic` | `not_ignored` |
| `DISC_FE0EFFB9BFAD7E7C4A1D` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_standard_string_unknown_argument_suppression_has_primary_diagnostic` | `not_ignored` |
| `DISC_62EE1C62EF5E3188BDF8` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_standard_timer_function_block_call` | `not_ignored` |
| `DISC_2C6F30D796182B3FDC95` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_standard_timer_function_block_ltime_overload` | `not_ignored` |
| `DISC_9E6E418E8BE3CA167421` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_standard_timer_function_block_ltime_type_error` | `not_ignored` |
| `DISC_B38C4517332CE2B52F72` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_standard_timer_function_block_ltime_variant` | `not_ignored` |
| `DISC_F520633EBC36610C0EEF` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_standard_unary_unknown_argument_suppression_has_primary_diagnostic` | `not_ignored` |
| `DISC_DA1C5558573916A12D99` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_string_comparison_operators_ok` | `not_ignored` |
| `DISC_C7F60E89EA6FB5139963` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_struct_field_in_callee_position_is_not_callable` | `not_ignored` |
| `DISC_F4BCD3E53C09C579146A` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs` | `test_typed_literal_prefix` | `not_ignored` |
| `DISC_F25EDEAE0459001ED5B7` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/enum_unqualified_in_expressions.rs` | `test_ambiguous_unqualified_enum_variant_in_constant_initializer_is_rejected` | `not_ignored` |
| `DISC_7AEACE8EA3B2E1BCCAAE` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/enum_unqualified_in_expressions.rs` | `test_unqualified_enum_variant_in_assignment_rvalue_type_checks` | `not_ignored` |
| `DISC_5B224949604858E91F2E` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/enum_unqualified_in_expressions.rs` | `test_unqualified_enum_variant_in_binary_comparison_type_checks` | `not_ignored` |
| `DISC_AD999AFE35064CE0FB2A` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/enum_unqualified_in_expressions.rs` | `test_unqualified_enum_variant_in_var_initializer_type_checks` | `not_ignored` |
| `DISC_C7ABF66EECB32176096D` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/hir_mutation_hardening.rs` | `aggregate_initializer_unknown_target_type_does_not_emit_cascade_field_errors` | `not_ignored` |
| `DISC_C4769092001F614617AE` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/hir_mutation_hardening.rs` | `array_default_rejects_non_repeat_call_expression` | `not_ignored` |
| `DISC_982B4D691EABE58FD09F` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/hir_mutation_hardening.rs` | `array_index_const_eval_matrix_reports_exact_out_of_range_values` | `not_ignored` |
| `DISC_E1DD2C9E5FF5ADCB4FE7` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/hir_mutation_hardening.rs` | `case_label_const_eval_matrix_reports_exact_duplicate_case_diagnostics` | `not_ignored` |
| `DISC_4CB529FD38059EF7CB0B` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/hir_mutation_hardening.rs` | `case_label_local_const_scope_chain_reports_exact_duplicate_case_diagnostic` | `not_ignored` |
| `DISC_4A0DBC01127B549E09FF` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/hir_mutation_hardening.rs` | `cross_file_import_creates_scope_for_source_only_namespace` | `not_ignored` |
| `DISC_B76FF1A248A058BE8009` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/hir_mutation_hardening.rs` | `cross_file_import_preserves_all_compound_type_shapes_under_type_id_collisions` | `not_ignored` |
| `DISC_9BC6755D7DDDDBA846C8` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/hir_mutation_hardening.rs` | `cross_file_import_preserves_namespace_scope_and_merges_existing_namespaces` | `not_ignored` |
| `DISC_DB291FC1382E2BF74CD8` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/hir_mutation_hardening.rs` | `cross_file_import_preserves_scalar_array_struct_union_and_alias_chain_types` | `not_ignored` |
| `DISC_7E3FC94CFB5AD97A01BA` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/hir_mutation_hardening.rs` | `cross_file_import_translates_callable_symbol_kind_type_ids` | `not_ignored` |
| `DISC_69863245C005005E80AA` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/hir_mutation_hardening.rs` | `cross_file_import_translates_union_variant_default_initializer` | `not_ignored` |
| `DISC_1815F34E6383478FA4F3` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/hir_mutation_hardening.rs` | `direct_array_repeat_default_validates_repeated_element_type` | `not_ignored` |
| `DISC_6ED3295B9320278DB577` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/hir_mutation_hardening.rs` | `function_block_initializer_rejects_forbidden_member_kinds_with_locked_messages` | `not_ignored` |
| `DISC_B089CF63CC6C34B39C61` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/hir_mutation_hardening.rs` | `integer_default_bounds_matrix_reports_each_out_of_range_type` | `not_ignored` |
| `DISC_A346F7BB1128BD58295C` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/hir_mutation_hardening.rs` | `nested_struct_and_union_defaults_validate_member_required_types` | `not_ignored` |
| `DISC_B536EE59A9A2EEE83D86` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/hir_mutation_hardening.rs` | `reference_null_default_is_allowed_but_non_null_reference_default_is_rejected` | `not_ignored` |
| `DISC_3470D15CD11D722E8027` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/hir_mutation_hardening.rs` | `subrange_default_bounds_are_enforced_at_both_edges` | `not_ignored` |
| `DISC_397C97F8D897AB5C62C8` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/hir_mutation_hardening.rs` | `type_level_array_defaults_validate_elements_and_repetition` | `not_ignored` |
| `DISC_B58AB55CA6FEAF9446CA` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/hir_mutation_hardening.rs` | `type_resolution_const_eval_errors_report_primary_diagnostic` | `not_ignored` |
| `DISC_464D77E530E7EDA41D18` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/hir_mutation_hardening.rs` | `type_resolution_const_eval_matrix_preserves_integer_expression_values` | `not_ignored` |
| `DISC_0E475FE523B9735F24EB` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/hir_mutation_hardening.rs` | `union_aggregate_initializer_validates_variant_names_and_locations` | `not_ignored` |
| `DISC_7E8E2FE6B5F06512AD97` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/hir_mutation_hardening.rs` | `var_block_collection_preserves_parameter_visibility_and_non_config_scope` | `not_ignored` |
| `DISC_53FB74DD728EF27C5DF0` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/hir_mutation_hardening.rs` | `wstring_field_default_length_reports_e304_on_literal` | `not_ignored` |
| `DISC_2307026A9DA4ACA20908` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/parameter_constant_qualifier.rs` | `assign_through_var_in_out_constant_array_index_is_rejected` | `not_ignored` |
| `DISC_8D4C8BB143EC378C8E75` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/parameter_constant_qualifier.rs` | `assign_through_var_in_out_constant_pointer_slot_and_deref_have_distinct_rules` | `not_ignored` |
| `DISC_9B66D6501600FA25CA65` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/parameter_constant_qualifier.rs` | `assign_through_var_in_out_constant_struct_field_is_rejected` | `not_ignored` |
| `DISC_6699DD76A0F26B644FC3` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/parameter_constant_qualifier.rs` | `assign_through_var_input_pointer_whose_target_is_constant_array_is_accepted` | `not_ignored` |
| `DISC_57F4ECDAFAA86E363BA7` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/parameter_constant_qualifier.rs` | `assign_to_var_in_out_constant_is_rejected` | `not_ignored` |
| `DISC_85BAE64389639F443AA6` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/parameter_constant_qualifier.rs` | `assign_to_var_input_constant_is_rejected` | `not_ignored` |
| `DISC_C6E0E43C5535FC99AAE2` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/parameter_constant_qualifier.rs` | `assign_to_var_output_constant_is_rejected` | `not_ignored` |
| `DISC_B139A6AD136789FBDB72` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/parameter_constant_qualifier.rs` | `assign_to_var_temp_constant_is_rejected` | `not_ignored` |
| `DISC_6DEA62C519CA801DF06C` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/parameter_constant_qualifier.rs` | `fb_instance_in_var_global_constant_is_rejected` | `not_ignored` |
| `DISC_0270C8290A0D5AD93C1A` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/parameter_constant_qualifier.rs` | `fb_instance_in_var_in_out_constant_is_rejected` | `not_ignored` |
| `DISC_3C93CF6C560C37CE28BB` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/parameter_constant_qualifier.rs` | `fb_instance_in_var_input_constant_is_rejected` | `not_ignored` |
| `DISC_99516D1BA9CC12F01931` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/parameter_constant_qualifier.rs` | `fb_instance_in_var_temp_constant_is_rejected` | `not_ignored` |
| `DISC_B0927C9B0A0ED50D6BB1` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/parameter_constant_qualifier.rs` | `var_constant_local_remains_symbol_kind_constant` | `not_ignored` |
| `DISC_E02CF25F3D5FDAAF22E7` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/parameter_constant_qualifier.rs` | `var_external_constant_regression_still_works` | `not_ignored` |
| `DISC_A1981DA854E2080AC9BA` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/parameter_constant_qualifier.rs` | `var_global_constant_is_precollected` | `not_ignored` |
| `DISC_C4D475F7F979054481DD` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/parameter_constant_qualifier.rs` | `var_global_constant_remains_symbol_kind_constant` | `not_ignored` |
| `DISC_FBAE3604ED51DFC8770C` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/parameter_constant_qualifier.rs` | `var_in_out_constant_accepts_caller_storage_argument` | `not_ignored` |
| `DISC_6A3ED8213919EEE19589` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/parameter_constant_qualifier.rs` | `var_in_out_constant_call_site_binding_unchanged` | `not_ignored` |
| `DISC_53EBC7EDC0F663FBD96F` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/parameter_constant_qualifier.rs` | `var_in_out_constant_is_parameter_with_is_constant_flag` | `not_ignored` |
| `DISC_F8D1E5D3DD37AB914876` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/parameter_constant_qualifier.rs` | `var_input_constant_is_not_precollected_as_compile_time_expression` | `not_ignored` |
| `DISC_F326F2DFED4741689B18` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/parameter_constant_qualifier.rs` | `var_input_constant_is_parameter_with_is_constant_flag` | `not_ignored` |
| `DISC_F6CEA3F9DB2B552E5004` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/parameter_constant_qualifier.rs` | `var_input_constant_participates_in_call_argument_resolution` | `not_ignored` |
| `DISC_BA0F79DCA2E65962E085` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/parameter_constant_qualifier.rs` | `var_output_constant_is_parameter_with_is_constant_flag` | `not_ignored` |
| `DISC_C41498B9CB6C648630FE` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/parameter_constant_qualifier.rs` | `var_temp_constant_is_not_precollected_as_compile_time_expression` | `not_ignored` |
| `DISC_D5F71F78B2798CF5DEAE` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/parameter_constant_qualifier.rs` | `var_temp_constant_is_variable_with_is_constant_flag` | `not_ignored` |
| `DISC_994DEE1F0AE6ACFDDC9F` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/pointer_param_write_through.rs` | `assign_through_var_input_pointer_array_deref_is_accepted` | `not_ignored` |
| `DISC_FB21D0CB9C474F1E0F3B` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/pointer_param_write_through.rs` | `assign_through_var_input_pointer_deref_is_accepted` | `not_ignored` |
| `DISC_C096582CCBB65B04A27C` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/pointer_param_write_through.rs` | `assign_through_var_input_pointer_struct_deref_is_accepted` | `not_ignored` |
| `DISC_1C693AFB3F36A1A88DD2` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/pointer_param_write_through.rs` | `assign_through_var_input_pointer_to_nested_index_is_accepted` | `not_ignored` |
| `DISC_7295A585C38C58818171` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/pointer_param_write_through.rs` | `assign_to_field_of_var_input_fb_instance_is_still_rejected` | `not_ignored` |
| `DISC_F2141AA02D7E55F42BDB` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/pointer_param_write_through.rs` | `assign_to_var_input_pointer_itself_is_rejected` | `not_ignored` |
| `DISC_8C8F5D73C5B1B31209EB` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/sizeof_semantics.rs` | `test_sizeof_accepts_explicit_type_operand` | `not_ignored` |
| `DISC_4C9C6997C6EE2F7F8390` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/sizeof_semantics.rs` | `test_sizeof_accepts_this_field_operand_inside_method` | `not_ignored` |
| `DISC_8DB83BB72C2E74B92546` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/sizeof_semantics.rs` | `test_sizeof_accepts_variable_operand` | `not_ignored` |
| `DISC_A66624B2647624C71D9E` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/sizeof_semantics.rs` | `test_sizeof_accepts_variable_operand_in_array_bounds` | `not_ignored` |
| `DISC_CD91D86D4CE0BFC8D056` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/sizeof_semantics.rs` | `test_sizeof_ambiguous_using_value_reports_primary_only` | `not_ignored` |
| `DISC_26DE6D321E0FA18651C2` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/sizeof_semantics.rs` | `test_sizeof_bare_name_prefers_variable_over_top_level_type_in_array_bounds` | `not_ignored` |
| `DISC_C72AEE9B47EE9E5081DB` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/sizeof_semantics.rs` | `test_sizeof_pointer_contract_matches_platform_word_size` | `not_ignored` |
| `DISC_5F3745A6B40D778304BD` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/sizeof_semantics.rs` | `test_sizeof_pointer_operand_const_folds_in_array_bounds` | `not_ignored` |
| `DISC_61D3C06E888D779B78F5` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/sizeof_semantics.rs` | `test_sizeof_rejects_call_operand` | `not_ignored` |
| `DISC_467B5ADEBDFA21D5CE3A` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/sizeof_semantics.rs` | `test_sizeof_rejects_function_block_instance_operand` | `not_ignored` |
| `DISC_21270660F91BF3496239` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/sizeof_semantics.rs` | `test_sizeof_rejects_non_lvalue_expression_operand` | `not_ignored` |
| `DISC_5D57DB4565E59FCCC5E5` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/sizeof_semantics.rs` | `test_sizeof_rejects_open_array_operand` | `not_ignored` |
| `DISC_48D9A8095F1C58B410DD` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/sizeof_semantics.rs` | `test_sizeof_rejects_this_operand_for_unsupported_receiver_storage_size` | `not_ignored` |
| `DISC_68F815D41F8F071CF379` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/sizeof_semantics.rs` | `test_sizeof_rejects_unknown_identifier_cleanly` | `not_ignored` |
| `DISC_7F3022276811380924D0` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/sizeof_semantics.rs` | `test_sizeof_string_length_const_eval_error_reports_primary_diagnostic` | `not_ignored` |
| `DISC_44224568DC68773ABB94` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/struct_initializers.rs` | `aggregate_field_order_and_case_are_independent` | `not_ignored` |
| `DISC_624432DAC34BA77705E6` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/struct_initializers.rs` | `aggregate_initializer_against_non_aggregate_target_reports_e201` | `not_ignored` |
| `DISC_628C7FD00A3545FFD54E` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/struct_initializers.rs` | `class_call_style_aggregate_initializer_reports_e202` | `not_ignored` |
| `DISC_95C8803A54C0FAF1B7DB` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/struct_initializers.rs` | `const_forward_reference_is_deterministic` | `not_ignored` |
| `DISC_6FC28E86A7FB1B6FC73F` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/struct_initializers.rs` | `constant_default_divide_by_zero_reports_e202` | `not_ignored` |
| `DISC_39B82970C3B9F0F0AFE0` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/struct_initializers.rs` | `constant_default_overflow_reports_e202` | `not_ignored` |
| `DISC_C81FA8B01C3CE4BE3762` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/struct_initializers.rs` | `cross_file_const_can_feed_field_default` | `not_ignored` |
| `DISC_CAEC6E0AD9F0471F5394` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/struct_initializers.rs` | `cross_file_import_translates_struct_field_default_initializer` | `not_ignored` |
| `DISC_7B47E982621133BE9340` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/struct_initializers.rs` | `cyclic_constant_default_reports_e305_not_silent_none` | `not_ignored` |
| `DISC_CB3F93E58B89EA3300AB` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/struct_initializers.rs` | `field_default_bool_mismatch_reports_e201` | `not_ignored` |
| `DISC_0CC3775A2FCEAEE55CB5` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/struct_initializers.rs` | `field_default_divide_by_zero_reports_e202` | `not_ignored` |
| `DISC_BDA3D387D422A3B6E6B6` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/struct_initializers.rs` | `field_default_edit_invalidates_hir_initializer_catalog` | `not_ignored` |
| `DISC_F28B547132B0C7329CD7` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/struct_initializers.rs` | `field_default_non_constant_reference_is_rejected` | `not_ignored` |
| `DISC_C7312942D0B1A1F3DB1F` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/struct_initializers.rs` | `field_default_sizeof_time_and_date_literals_are_accepted` | `not_ignored` |
| `DISC_3DCC92807A5EF738B8DA` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/struct_initializers.rs` | `field_default_string_length_uses_out_of_range` | `not_ignored` |
| `DISC_00DA11957432B7BAF833` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/struct_initializers.rs` | `field_default_target_range_reports_e304` | `not_ignored` |
| `DISC_C48792A6DCC0980CF87A` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/struct_initializers.rs` | `function_block_initializer_allows_inputs_outputs_and_public_vars` | `not_ignored` |
| `DISC_8B078D0B388F89904A67` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/struct_initializers.rs` | `function_block_initializer_rejects_var_in_out_member` | `not_ignored` |
| `DISC_6D17DEE37A264E5CD0FD` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/struct_initializers.rs` | `hir_struct_field_default_initializer_is_recorded` | `not_ignored` |
| `DISC_C78C8311671D28530A1A` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/struct_initializers.rs` | `hir_type_level_default_initializer_is_recorded` | `not_ignored` |
| `DISC_C1A2C63A377FD9A8C760` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/struct_initializers.rs` | `hir_union_variant_default_initializer_is_recorded` | `not_ignored` |
| `DISC_55C842274DE04C03BFE1` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/struct_initializers.rs` | `named_struct_initializer_duplicate_field_reports_e108` | `not_ignored` |
| `DISC_D927C08738A3AED695E3` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/struct_initializers.rs` | `named_struct_initializer_unknown_field_reports_e107` | `not_ignored` |
| `DISC_7A7CCCF80312F0BF1C71` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/struct_initializers.rs` | `reference_member_default_ref_expression_is_rejected` | `not_ignored` |
| `DISC_1C334B9F78A6CC903E42` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/struct_initializers.rs` | `type_level_aggregate_default_fields_are_checked` | `not_ignored` |
| `DISC_A2D327C0D3B20178490E` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/struct_initializers.rs` | `valid_named_struct_initializer_has_no_hir_error` | `not_ignored` |
| `DISC_EDA48D90FA61FC9B67B1` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/types_and_references.rs` | `test_adr_requires_lvalue` | `not_ignored` |
| `DISC_8240892424BB45B1CDE8` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/types_and_references.rs` | `test_cyclic_cross_file_type_import_reports_primary_diagnostic` | `not_ignored` |
| `DISC_E618C56EF4F9469EA27E` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/types_and_references.rs` | `test_deep_alias_chain_resolves_to_base_type` | `not_ignored` |
| `DISC_52CC9307E084C238A2A2` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/types_and_references.rs` | `test_delete_unknown_operand_suppression_has_primary_diagnostic` | `not_ignored` |
| `DISC_EC8BAD0A64AE8A89BFB5` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/types_and_references.rs` | `test_deref_unknown_operand_suppression_has_primary_diagnostic` | `not_ignored` |
| `DISC_AE244DAA6B1AE9F13051` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/types_and_references.rs` | `test_method_call_on_instance` | `not_ignored` |
| `DISC_2A64027AC33FA85FD1AD` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/types_and_references.rs` | `test_new_ambiguous_using_type_reports_cannot_resolve_not_undefined_type` | `not_ignored` |
| `DISC_FD75EFD192A3FB8FDE42` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/types_and_references.rs` | `test_null_assignment_to_reference` | `not_ignored` |
| `DISC_0BFFA6B86F7C48F9B120` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/types_and_references.rs` | `test_null_comparison_reference` | `not_ignored` |
| `DISC_A99473E37DB519A2555B` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/types_and_references.rs` | `test_parenthesized_string_length_constant_expression` | `not_ignored` |
| `DISC_1B2B256D14EDF473D16D` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/types_and_references.rs` | `test_ref_assignment_allows_reference_source` | `not_ignored` |
| `DISC_EC380387F1335ACEA8F2` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/types_and_references.rs` | `test_ref_assignment_requires_reference_source` | `not_ignored` |
| `DISC_1837416D9FF42555A848` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/types_and_references.rs` | `test_ref_assignment_requires_reference_target` | `not_ignored` |
| `DISC_040086F27F6121D463D9` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/types_and_references.rs` | `test_ref_rejects_constant` | `not_ignored` |
| `DISC_73D4FC496FF38C284D6D` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/types_and_references.rs` | `test_ref_rejects_function_local_variable` | `not_ignored` |
| `DISC_94100DB31F1CB2D16C0F` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/types_and_references.rs` | `test_ref_rejects_function_return_variable` | `not_ignored` |
| `DISC_F1BFAEF8054FF79967E4` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/types_and_references.rs` | `test_ref_rejects_method_return_variable` | `not_ignored` |
| `DISC_B89551062EF4F02B30E8` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/types_and_references.rs` | `test_ref_rejects_temp_variable` | `not_ignored` |
| `DISC_882185357A8BB84CF42E` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/types_and_references.rs` | `test_ref_requires_lvalue` | `not_ignored` |
| `DISC_17EB3E39FF47AD5E50B6` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/types_and_references.rs` | `test_ref_returns_reference` | `not_ignored` |
| `DISC_47E03CF1B00678F0CE62` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/types_and_references.rs` | `test_sizeof_expression` | `not_ignored` |
| `DISC_DF1743DCE4E87A1B80F7` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/types_and_references.rs` | `test_string_length_assignment_between_lengths` | `not_ignored` |
| `DISC_F23C7F5C2A94D67F6ADF` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/types_and_references.rs` | `test_string_length_constant_expression` | `not_ignored` |
| `DISC_043C2FC1EDE569A47229` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/types_and_references.rs` | `test_string_literal_length_in_assignment` | `not_ignored` |
| `DISC_DA541FC99C60C0B346BB` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/types_and_references.rs` | `test_string_literal_length_in_initializer` | `not_ignored` |
| `DISC_B16B8569CA0E62ACCBCE` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/types_and_references.rs` | `test_type_alias_numeric_ops` | `not_ignored` |
| `DISC_F9051038AB4E86CEA686` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/types_and_references.rs` | `test_type_position_uses_type_namespace_when_value_has_same_name` | `not_ignored` |
| `DISC_FD9EDD216CEF1CF02AB0` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/types_and_references.rs` | `test_unknown_typed_literal_prefix_reports_undefined_type` | `not_ignored` |
| `DISC_DDE96C31A0FC0AC6C4D2` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/wrong_kind_resolution.rs` | `callable_used_as_variable_reports_wrong_kind_not_undefined_variable` | `not_ignored` |
| `DISC_E3F40AEFBB255E2AEDDB` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/wrong_kind_resolution.rs` | `imported_namespace_used_as_callable_reports_not_callable_not_cannot_resolve` | `not_ignored` |
| `DISC_74ED8153A453892337A4` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/wrong_kind_resolution.rs` | `imported_namespaced_function_used_as_type_reports_wrong_kind_not_undefined_type` | `not_ignored` |
| `DISC_6CCD3E18E6F1411D86E6` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/wrong_kind_resolution.rs` | `imported_namespaced_type_used_as_value_reports_wrong_kind_not_undefined_variable` | `not_ignored` |
| `DISC_EA1F874941C90147ED61` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/wrong_kind_resolution.rs` | `namespace_used_as_callable_reports_not_callable_not_cannot_resolve` | `not_ignored` |
| `DISC_3C9FB0334CCC48B7763E` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/wrong_kind_resolution.rs` | `parenthesized_value_used_as_callable_reports_not_callable` | `not_ignored` |
| `DISC_240362AB3FBFF10EA988` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/wrong_kind_resolution.rs` | `type_used_as_value_reports_wrong_kind_not_undefined_variable` | `not_ignored` |
| `DISC_2D08F2BEA4A7BD5AD967` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/wrong_kind_resolution.rs` | `value_used_as_type_reports_wrong_kind_not_undefined_type` | `not_ignored` |
| `DISC_BE31C41C9BB087B1219D` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_definitions.rs` | `test_abstract_class_instantiation_error` | `not_ignored` |
| `DISC_CC5F4851F4641E53ADD9` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_definitions.rs` | `test_abstract_class_requires_abstract_method` | `not_ignored` |
| `DISC_9FD395E7E09E777B05CE` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_definitions.rs` | `test_abstract_method_requires_abstract_class` | `not_ignored` |
| `DISC_7C04501265F4C5AE6DF1` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_definitions.rs` | `test_class_extends_final_error` | `not_ignored` |
| `DISC_C61DE54FC81406EC69A0` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_definitions.rs` | `test_class_modifiers_and_visibility_collected` | `not_ignored` |
| `DISC_2B4D8356395B590349D0` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_definitions.rs` | `test_class_type_registered` | `not_ignored` |
| `DISC_1475FF85C78990F634A3` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_definitions.rs` | `test_enum_type` | `not_ignored` |
| `DISC_E5992CDD6D2EEED9B588` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_definitions.rs` | `test_inherited_variable_name_conflict_error` | `not_ignored` |
| `DISC_AFD5A3B97B31234A82BB` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_definitions.rs` | `test_method_conflicts_with_inherited_variable_error` | `not_ignored` |
| `DISC_7DBEC5262646CCC13FE0` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_definitions.rs` | `test_method_override_modifier_collected` | `not_ignored` |
| `DISC_2DDD92D625C24A77C05E` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_definitions.rs` | `test_non_abstract_class_missing_abstract_base_method_error` | `not_ignored` |
| `DISC_1BA7CDE86489DB127B9E` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_definitions.rs` | `test_override_final_method_error` | `not_ignored` |
| `DISC_D28A718472C0CC80FD84` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_definitions.rs` | `test_override_requires_base_method` | `not_ignored` |
| `DISC_31C65E944C3DA7501A0A` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_definitions.rs` | `test_override_requires_override_keyword` | `not_ignored` |
| `DISC_C80429A6AF0ECB69BB70` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_definitions.rs` | `test_override_signature_mismatch_error` | `not_ignored` |
| `DISC_96086852002E488097FA` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_definitions.rs` | `test_override_visibility_mismatch_error` | `not_ignored` |
| `DISC_F8AC6A333D1B789B55C3` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_definitions.rs` | `test_struct_field_access` | `not_ignored` |
| `DISC_FE0CE8BCDC3E801A0117` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_definitions.rs` | `test_struct_type_fields` | `not_ignored` |
| `DISC_2CE7B5383A9C21BFBCA9` | `rust_integration_test` | `crates/trust-hir/tests/var_sections.rs` | `duplicate_file_scope_global_names_are_rejected_by_collector_path` | `not_ignored` |
| `DISC_9C0C2F70A1D215235A23` | `rust_integration_test` | `crates/trust-hir/tests/var_sections.rs` | `duplicate_global_names_across_scopes_are_rejected` | `not_ignored` |
| `DISC_BDCCC287C855F7EC5951` | `rust_integration_test` | `crates/trust-hir/tests/var_sections.rs` | `file_scope_var_global_is_accepted_across_files` | `not_ignored` |
| `DISC_C2C62B3BC3F7EE7F3F36` | `rust_integration_test` | `crates/trust-hir/tests/var_sections.rs` | `iec_table13` | `not_ignored` |
| `DISC_605F2D13F6D415B70C17` | `rust_integration_test` | `crates/trust-hir/tests/var_sections.rs` | `multiple_file_scope_gvls_are_aggregated` | `not_ignored` |
| `DISC_A813361CA42CEB99D8F0` | `rust_integration_test` | `crates/trust-hir/tests/var_sections.rs` | `program_var_global_is_accepted` | `not_ignored` |
| `DISC_16A735ECF80A8CBB0D36` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_01.rs` | `test_completion_includes_symbols` | `not_ignored` |
| `DISC_5D4509FDE6AB307874E8` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_01.rs` | `test_completion_member_access_struct_fields` | `not_ignored` |
| `DISC_F89996479B6E0216244C` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_01.rs` | `test_completion_statement_context` | `not_ignored` |
| `DISC_1D7138D60615B3D26F12` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_01.rs` | `test_completion_top_level` | `not_ignored` |
| `DISC_C5B67CC2C4F29C08B795` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_01.rs` | `test_completion_type_annotation` | `not_ignored` |
| `DISC_F25AB3FCF2B4342BF587` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_01.rs` | `test_goto_definition_boundary_positions_for_typed_literal_and_local_var` | `not_ignored` |
| `DISC_5779B5AB0D6AFACA8DA6` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_01.rs` | `test_goto_definition_struct_in_namespace` | `not_ignored` |
| `DISC_E319CED1E00F76CC81E0` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_01.rs` | `test_goto_implementation_interface` | `not_ignored` |
| `DISC_A9B73A0BFB139DD0E182` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_01.rs` | `test_references_simple_variable` | `not_ignored` |
| `DISC_930594EE0197B7B41405` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_02.rs` | `test_references_different_scopes_same_name` | `not_ignored` |
| `DISC_DAB2F32529DD44DDDEB9` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_02.rs` | `test_references_member_access` | `not_ignored` |
| `DISC_91442A73AD68714FE264` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_02.rs` | `test_references_type_reference` | `not_ignored` |
| `DISC_F9C66C63FFD119E1F673` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_02.rs` | `test_references_unknown_symbol_no_fallback` | `not_ignored` |
| `DISC_3A5A9B9D172671F4780E` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_02.rs` | `test_rename_basic` | `not_ignored` |
| `DISC_208CB1F2A60D4A4D0661` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_02.rs` | `test_rename_rejects_invalid_name` | `not_ignored` |
| `DISC_65D46E6268EF4E8D4DAE` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_02.rs` | `test_rename_rejects_keywords` | `not_ignored` |
| `DISC_1E10178DCFD82C695E1E` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_02.rs` | `test_rename_struct_field` | `not_ignored` |
| `DISC_F3A60C8FC839A4FA2ECF` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_03.rs` | `test_rename_function_block_from_usage_site_updates_declaration` | `not_ignored` |
| `DISC_B89E05A13857EDF6943F` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_03.rs` | `test_rename_function_block_updates_type_usage_in_other_file` | `not_ignored` |
| `DISC_7C47A23CACD11047203B` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_03.rs` | `test_semantic_tokens_function` | `not_ignored` |
| `DISC_0580026649DA618B6D8B` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_03.rs` | `test_struct_field_definition_and_references_across_files` | `not_ignored` |
| `DISC_F9805C47AF06FB002E38` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_03.rs` | `test_symbol_navigation_and_rename_from_punctuation_adjacent_positions` | `not_ignored` |
| `DISC_2A01B1CF92020C525D37` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_04.rs` | `test_semantic_tokens_constant` | `not_ignored` |
| `DISC_49264DF5347B24EB21E3` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_04.rs` | `test_semantic_tokens_enum_member` | `not_ignored` |
| `DISC_66D016792A2D14156D4A` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_04.rs` | `test_semantic_tokens_keywords` | `not_ignored` |
| `DISC_843CA284F40489132057` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_04.rs` | `test_semantic_tokens_method_member` | `not_ignored` |
| `DISC_0323C2F722B1550EEF8A` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_04.rs` | `test_semantic_tokens_parameter` | `not_ignored` |
| `DISC_C1073FFC7DDC10D8C21F` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_04.rs` | `test_semantic_tokens_struct_field_member` | `not_ignored` |
| `DISC_52D89EC9E2E42CB4F3FE` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_04.rs` | `test_semantic_tokens_type_reference` | `not_ignored` |
| `DISC_610D81D7FF0AFAE6235C` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_04.rs` | `test_semantic_tokens_variable` | `not_ignored` |
| `DISC_3CEC92A4335F764EB3B6` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_05.rs` | `test_goto_definition_method_member` | `not_ignored` |
| `DISC_9E3DB0B6D4323DEA4404` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_05.rs` | `test_hover_function_block_uses_declared_type_when_type_resolution_is_unknown` | `not_ignored` |
| `DISC_C8D46F7BD6CB4F92865B` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_05.rs` | `test_hover_initializers_and_retention` | `not_ignored` |
| `DISC_D25DD80A487FC9F7CB84` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_05.rs` | `test_hover_task_priority` | `not_ignored` |
| `DISC_6946E4303DE59B1A2B47` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_05.rs` | `test_hover_type_definitions_and_fb_interface` | `not_ignored` |
| `DISC_07ECA802A95A2D059198` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_06.rs` | `test_completion_constant_parameter_uses_constant_kind` | `not_ignored` |
| `DISC_3D367EFF910515896062` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_06.rs` | `test_hover_function_block_member_sections_show_constant_headers_and_array_star` | `not_ignored` |
| `DISC_B77EDB8B58E3538E9B59` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_06.rs` | `test_hover_parameter_constant_mentions_constant_and_array_star` | `not_ignored` |
| `DISC_1C1018E1F1D9C68A1990` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_06.rs` | `test_ide_diagnostics_allow_var_input_pointer_write_through` | `not_ignored` |
| `DISC_B2AB26258D65621FC06A` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_06.rs` | `test_ide_diagnostics_reject_array_wildcard_outside_parameter_sections` | `not_ignored` |
| `DISC_80BF5745A4ACDBD92FA3` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_06.rs` | `test_semantic_tokens_parameter_constant_is_readonly` | `not_ignored` |
| `DISC_D57913B7EEBE8B2B524E` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_06.rs` | `test_semantic_tokens_var_temp_constant_is_readonly` | `not_ignored` |
| `DISC_A312B4EC5E936559AF09` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_06.rs` | `test_signature_help_method_var_input_mentions_method_parameters` | `not_ignored` |
| `DISC_0E98E873534C7C4F4CFF` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_06.rs` | `test_signature_help_validate_function` | `not_ignored` |
| `DISC_290B1BE92719219785B6` | `rust_integration_test` | `crates/trust-ide/tests/ide_features/ide_features_part_06.rs` | `test_signature_help_var_in_out_constant_mentions_constant` | `not_ignored` |
| `DISC_21F6B9D48C995213768C` | `rust_integration_test` | `crates/trust-ide/tests/refactor_regression.rs` | `refactor_convert_function_block_to_function_edit_snapshot` | `not_ignored` |
| `DISC_2D509E898867085B2E22` | `rust_integration_test` | `crates/trust-ide/tests/refactor_regression.rs` | `refactor_convert_function_to_function_block_edit_snapshot` | `not_ignored` |
| `DISC_184CB81D352EAD1003D9` | `rust_integration_test` | `crates/trust-ide/tests/refactor_regression.rs` | `refactor_extract_method_edit_snapshot` | `not_ignored` |
| `DISC_7008EE70827923565DE7` | `rust_integration_test` | `crates/trust-ide/tests/refactor_regression.rs` | `refactor_generate_stubs_edit_snapshot` | `not_ignored` |
| `DISC_DA37E2549ACE71F92167` | `rust_integration_test` | `crates/trust-ide/tests/refactor_regression.rs` | `refactor_inline_symbol_edit_snapshot` | `not_ignored` |
| `DISC_F5E6BE8C4339D8336286` | `rust_integration_test` | `crates/trust-ide/tests/refactor_regression.rs` | `refactor_move_namespace_edit_snapshot` | `not_ignored` |
| `DISC_189FCFE72219BBE0FF65` | `rust_integration_test` | `crates/trust-ide/tests/stdlib_coverage.rs` | `coverage_doc_includes_all_stdlib_names` | `not_ignored` |
| `DISC_2750FE1E8A1AEE6DF9CE` | `rust_integration_test` | `crates/trust-lsp/tests/spec_drift.rs` | `technical_spec_lists_lsp_capabilities` | `not_ignored` |
| `DISC_CD7F5ED58A7CDC82E91A` | `rust_integration_test` | `crates/trust-lsp/tests/spec_drift.rs` | `technical_spec_mentions_index_cache_and_diagnostics_toggles` | `not_ignored` |
| `DISC_C47D1035E66A5711A91D` | `rust_integration_test` | `crates/trust-runtime/tests/ads_cli_command.rs` | `ads_add_route_cli_does_not_echo_stdin_password_when_wire_feature_is_absent` | `not_ignored` |
| `DISC_8714D8F079C23B9476C3` | `rust_integration_test` | `crates/trust-runtime/tests/ads_cli_command.rs` | `ads_add_route_cli_rejects_password_argument` | `not_ignored` |
| `DISC_E454ABA5331BBBD3FE22` | `rust_integration_test` | `crates/trust-runtime/tests/ads_cli_command.rs` | `ads_browse_cli_requires_ads_wire_feature_without_faking_results` | `not_ignored` |
| `DISC_66249B74D0B590FB23C7` | `rust_integration_test` | `crates/trust-runtime/tests/ads_cli_command.rs` | `ads_discover_cli_requires_ads_wire_feature_without_faking_results` | `not_ignored` |
| `DISC_77757C7D508029CEF91D` | `rust_integration_test` | `crates/trust-runtime/tests/ads_cli_command.rs` | `ads_doctor_cli_requires_ads_wire_feature_without_faking_results` | `not_ignored` |
| `DISC_37EB78BF38C80DB04EFB` | `rust_integration_test` | `crates/trust-runtime/tests/ads_cli_command.rs` | `ads_import_symbols_cli_requires_ads_wire_feature_without_faking_results` | `not_ignored` |
| `DISC_F439BD12F2EB0ADE7096` | `rust_integration_test` | `crates/trust-runtime/tests/ads_cli_command.rs` | `ads_route_remove_cli_emits_removal_artifact_json` | `not_ignored` |
| `DISC_04A19410BC5774E73F8E` | `rust_integration_test` | `crates/trust-runtime/tests/ads_cli_command.rs` | `ads_route_script_cli_emits_static_routes_json` | `not_ignored` |
| `DISC_9FD64B080F8C036E9FAC` | `rust_integration_test` | `crates/trust-runtime/tests/ads_cli_command.rs` | `ads_server_doctor_cli_requires_external_proof_pair` | `not_ignored` |
| `DISC_62FF6025F8CBB54C8A4F` | `rust_integration_test` | `crates/trust-runtime/tests/ads_cli_command.rs` | `ads_server_route_script_cli_emits_server_specific_static_routes_json` | `not_ignored` |
| `DISC_628BCC9D5291C04D92EF` | `rust_integration_test` | `crates/trust-runtime/tests/ads_cli_command.rs` | `ads_server_route_script_cli_human_output_uses_server_wording` | `not_ignored` |
| `DISC_2FAB998C72FC7E709E79` | `rust_integration_test` | `crates/trust-runtime/tests/ads_cli_command.rs` | `ads_validate_cli_returns_nonzero_for_missing_snapshot_symbol` | `not_ignored` |
| `DISC_37A326E268D0B49F5700` | `rust_integration_test` | `crates/trust-runtime/tests/ads_cli_command.rs` | `ads_validate_cli_returns_nonzero_for_type_mismatch` | `not_ignored` |
| `DISC_4107870DD0A2782B8BCA` | `rust_integration_test` | `crates/trust-runtime/tests/ads_cli_command.rs` | `ads_validate_cli_returns_nonzero_with_first_diff_for_generated_drift` | `not_ignored` |
| `DISC_016A7FD66DC071E76D88` | `rust_integration_test` | `crates/trust-runtime/tests/ads_cli_command.rs` | `ads_validate_live_cli_requires_ads_wire_feature_without_reading_files` | `not_ignored` |
| `DISC_EC51372847454451E8D4` | `rust_integration_test` | `crates/trust-runtime/tests/ads_web_api.rs` | `ads_server_web_doctor_job_start_and_status_route_to_control_job_store` | `not_ignored` |
| `DISC_AE633989730EAD51BC42` | `rust_integration_test` | `crates/trust-runtime/tests/ads_web_api.rs` | `ads_server_web_status_symbols_and_route_plan_route_to_control` | `not_ignored` |
| `DISC_DAFCF3BB68BD0F595DFE` | `rust_integration_test` | `crates/trust-runtime/tests/ads_web_api.rs` | `ads_setup_page_assets_are_served_without_runtime_chooser` | `not_ignored` |
| `DISC_4A21D68A73FE4FBAEC0D` | `rust_integration_test` | `crates/trust-runtime/tests/ads_web_api.rs` | `ads_web_doctor_job_start_and_status_route_to_control_job_store` | `not_ignored` |
| `DISC_5B48F27D1BC7B03F64AD` | `rust_integration_test` | `crates/trust-runtime/tests/ads_web_api.rs` | `ads_web_route_add_derives_local_trusted_channel_and_does_not_echo_password` | `not_ignored` |
| `DISC_310AD2B4269939FFE836` | `rust_integration_test` | `crates/trust-runtime/tests/ads_web_api.rs` | `ads_web_status_and_import_symbols_route_to_control` | `not_ignored` |
| `DISC_1AA3F1B1F8D8639669F1` | `rust_integration_test` | `crates/trust-runtime/tests/ads_web_api.rs` | `ads_web_status_uses_internal_token_for_local_web_auth` | `not_ignored` |
| `DISC_5DE8737AB55FFD7FA7D3` | `rust_integration_test` | `crates/trust-runtime/tests/ads_web_api.rs` | `control_proxy_does_not_forward_local_internal_control_token` | `not_ignored` |
| `DISC_2F281903B225BBF480A5` | `rust_integration_test` | `crates/trust-runtime/tests/agent_command/harness_execute.rs` | `agent_serve_supports_harness_execute_for_pou_and_project_fixtures` | `not_ignored` |
| `DISC_AF4A9B6344EC39C3976B` | `rust_integration_test` | `crates/trust-runtime/tests/agent_command/lsp.rs` | `agent_serve_supports_ast_canonicalize_and_similarity` | `not_ignored` |
| `DISC_C7C3B90F6650C556BDA2` | `rust_integration_test` | `crates/trust-runtime/tests/agent_command/lsp.rs` | `agent_serve_supports_lsp_diagnostics_and_format_preview` | `not_ignored` |
| `DISC_B0E196E2AA7150913606` | `rust_integration_test` | `crates/trust-runtime/tests/agent_command/runtime_harness_loop.rs` | `agent_serve_supports_runtime_project_commands_and_harness_loop` | `not_ignored` |
| `DISC_1D12E277CDA6F7100F49` | `rust_integration_test` | `crates/trust-runtime/tests/agent_command/runtime_reload.rs` | `agent_serve_runtime_compile_reload_blocks_on_diagnostics` | `not_ignored` |
| `DISC_3575D0ABD28A87B82E7E` | `rust_integration_test` | `crates/trust-runtime/tests/agent_command/runtime_reload.rs` | `agent_serve_runtime_compile_reload_rebuilds_and_reloads_a_live_runtime` | `not_ignored` |
| `DISC_436234C5C6BCCA6EC051` | `rust_integration_test` | `crates/trust-runtime/tests/agent_command/runtime_reload.rs` | `agent_serve_runtime_compile_reload_reports_reload_failure` | `not_ignored` |
| `DISC_E1C78D1B49EEC73B475E` | `rust_integration_test` | `crates/trust-runtime/tests/agent_command/runtime_reload.rs` | `agent_serve_runtime_reload_rebuilds_and_reloads_a_live_runtime` | `not_ignored` |
| `DISC_AD782C3AD1D808D4C4EE` | `rust_integration_test` | `crates/trust-runtime/tests/agent_command/workspace_info.rs` | `agent_serve_reports_run_until_timeout_with_stable_code` | `not_ignored` |
| `DISC_6C30ED8D75D47A2A1C97` | `rust_integration_test` | `crates/trust-runtime/tests/agent_command/workspace_info.rs` | `agent_serve_reports_workspace_project_info` | `not_ignored` |
| `DISC_96073DBF244CB47A0CB8` | `rust_integration_test` | `crates/trust-runtime/tests/agent_command/workspace_protocol.rs` | `agent_serve_reports_method_and_path_errors_with_stable_codes` | `not_ignored` |
| `DISC_7345AEBCBDF3F725CEE3` | `rust_integration_test` | `crates/trust-runtime/tests/agent_command/workspace_protocol.rs` | `agent_serve_supports_describe_write_and_read_roundtrip` | `not_ignored` |
| `DISC_AA270B58E52FFB7F3182` | `rust_integration_test` | `crates/trust-runtime/tests/agent_command/workspace_protocol.rs` | `trust_runtime_agent_alias_forwards_to_trust_dev` | `not_ignored` |
| `DISC_A7805653302309DFDE9B` | `rust_integration_test` | `crates/trust-runtime/tests/api_smoke.rs` | `loads_runtime` | `not_ignored` |
| `DISC_BA233F91FA4D55C7BE2D` | `rust_integration_test` | `crates/trust-runtime/tests/api_smoke.rs` | `runtime_execution_backend_defaults_and_lazy_vm_materialization` | `not_ignored` |
| `DISC_56F28FCA5A66344E1C22` | `rust_integration_test` | `crates/trust-runtime/tests/api_smoke.rs` | `runtime_metrics_snapshot_tracks_vm_backend_selection` | `not_ignored` |
| `DISC_39CADCE0EA6E2CCADFB3` | `rust_integration_test` | `crates/trust-runtime/tests/boundary_resolver.rs` | `resolver_reads_indexed_and_dotted_program_paths` | `not_ignored` |
| `DISC_06771ADC38B4CC93B5E3` | `rust_integration_test` | `crates/trust-runtime/tests/boundary_resolver.rs` | `resolver_reports_ambiguous_unqualified_program_var` | `not_ignored` |
| `DISC_243DD1FFC684D3B7D8EF` | `rust_integration_test` | `crates/trust-runtime/tests/boundary_resolver.rs` | `resolver_reports_unknown_path_without_null_fallback` | `not_ignored` |
| `DISC_18496B8779AF7C3D150A` | `rust_integration_test` | `crates/trust-runtime/tests/build.rs` | `workspace_builds` | `not_ignored` |
| `DISC_D6DAC722D68243D27E1F` | `rust_integration_test` | `crates/trust-runtime/tests/build_unbound_program.rs` | `configuration_without_program_instance_errors_by_default` | `not_ignored` |
| `DISC_F06EBA3CB4306F18298B` | `rust_integration_test` | `crates/trust-runtime/tests/build_unbound_program.rs` | `explicit_extra_program_instance_keeps_test_builder_opt_in` | `not_ignored` |
| `DISC_B71ADAD1F3E8F1FF0373` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_container.rs` | `section_table_validation` | `not_ignored` |
| `DISC_4BABD0A9C6328EC16A6A` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_decode_resource_bounds.rs` | `allocation_pressure_from_declared_counts_fails_before_reservation` | `not_ignored` |
| `DISC_11C0595A088CC9249A4C` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_encoder/bytecode_encoder_part_01.rs` | `encoder_emits_method_tables` | `not_ignored` |
| `DISC_BDED3CC98816CD7074F2` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_encoder/bytecode_encoder_part_01.rs` | `encoder_roundtrip_validates` | `not_ignored` |
| `DISC_067960017E02589D06C7` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_encoder/bytecode_encoder_part_02.rs` | `encoder_emits_composite_types` | `not_ignored` |
| `DISC_EE0B39B38488ADEA0E66` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_encoder/bytecode_encoder_part_02.rs` | `encoder_emits_interface_methods` | `not_ignored` |
| `DISC_06B6FF75287B38841ED5` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_encoder/bytecode_encoder_part_03.rs` | `encoder_emits_debug_map` | `not_ignored` |
| `DISC_F6B002D7F1C29462475E` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_encoder/bytecode_encoder_part_03.rs` | `encoder_emits_param_defaults` | `not_ignored` |
| `DISC_2738788B07F09C960EBE` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_encoder/bytecode_encoder_part_03.rs` | `encoder_emits_program_field_var_meta_for_validation` | `not_ignored` |
| `DISC_50150C551F9A0DC86A74` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_encoder/bytecode_encoder_part_03.rs` | `encoder_emits_scoped_function_local_var_meta` | `not_ignored` |
| `DISC_F90C2FDE3F5C7C22034D` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_encoder/bytecode_encoder_part_03.rs` | `encoder_emits_var_meta_and_retain_init` | `not_ignored` |
| `DISC_0BB090B37C66328B2C12` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_encoder/bytecode_encoder_part_03.rs` | `encoder_validates_enum_constant_payloads` | `not_ignored` |
| `DISC_961A5558A908EDD96486` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_encoder/bytecode_encoder_part_04.rs` | `encoder_emits_control_flow_jumps` | `not_ignored` |
| `DISC_46B4E5748C6BE2308F35` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_encoder/bytecode_encoder_part_04.rs` | `encoder_emits_if_with_string_literal_elsif_condition` | `not_ignored` |
| `DISC_8283D50A273A87BD859C` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_encoder/bytecode_encoder_part_04.rs` | `encoder_emits_local_refs_for_functions_and_methods` | `not_ignored` |
| `DISC_0890944607C63CA2D2AF` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_encoder/bytecode_encoder_part_04.rs` | `encoder_preserves_label_only_statement_as_explicit_nop` | `not_ignored` |
| `DISC_60E4B48300011262D968` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_encoder/bytecode_encoder_part_05.rs` | `encoder_accepts_case_insensitive_names_in_call_heavy_if_blocks` | `not_ignored` |
| `DISC_520CF24D542197A0E073` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_encoder/bytecode_encoder_part_05.rs` | `encoder_bytes_roundtrip_from_source` | `not_ignored` |
| `DISC_B59FD15ABE4CE9EE4D2A` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_encoder/bytecode_encoder_part_05.rs` | `encoder_emits_dynamic_instance_access` | `not_ignored` |
| `DISC_0D28FE4B5184643A4B0E` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_encoder/bytecode_encoder_part_05.rs` | `encoder_emits_io_map` | `not_ignored` |
| `DISC_CAD04A3C0FEFF0D699E4` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_encoder/bytecode_encoder_part_05.rs` | `encoder_resource_meta_sizes_follow_io_bindings` | `not_ignored` |
| `DISC_83A61D5405748EA17972` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_metadata.rs` | `apply_bytecode_bytes` | `not_ignored` |
| `DISC_78CE926574401B29FA63` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_metadata.rs` | `fb_refs_from_container` | `not_ignored` |
| `DISC_5976C1A40D0F935CFFE5` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_metadata.rs` | `pou_associations` | `not_ignored` |
| `DISC_406C3141C3DBCD4AE62F` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_metadata.rs` | `resources_from_container` | `not_ignored` |
| `DISC_EDCC9A5FBD857AF162D3` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_metadata.rs` | `task_associations` | `not_ignored` |
| `DISC_6FE0F58BEA859BF48739` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_metadata.rs` | `tasks_from_metadata` | `not_ignored` |
| `DISC_5660557A5593ABE109FB` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_metadata.rs` | `version_gate` | `not_ignored` |
| `DISC_F90D4502D7B68E02847C` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_roundtrip.rs` | `corruption_rejected` | `not_ignored` |
| `DISC_64BA741F973C743A5E1D` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_roundtrip.rs` | `roundtrip` | `not_ignored` |
| `DISC_3215EEE2C7E4BE1F73F4` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_sections.rs` | `const_pool_decode` | `not_ignored` |
| `DISC_10FC0967AAC88DCD2303` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_sections.rs` | `debug_map_decode` | `not_ignored` |
| `DISC_EEE01FA9C839F448C16E` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_sections.rs` | `debug_string_table_decode` | `not_ignored` |
| `DISC_112C93A691CA5CE18B7B` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_sections.rs` | `io_map_decode` | `not_ignored` |
| `DISC_FA74E403D9C2B08CF16E` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_sections.rs` | `pou_index_decode` | `not_ignored` |
| `DISC_7654E96A31EAA5D4A45B` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_sections.rs` | `ref_table_decode` | `not_ignored` |
| `DISC_9A4201E718C7271623D5` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_sections.rs` | `resource_meta_decode` | `not_ignored` |
| `DISC_EDFC1245E61FEA06D134` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_sections.rs` | `string_table_decode` | `not_ignored` |
| `DISC_CB10FB386D6143E4F283` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_sections.rs` | `string_table_padding` | `not_ignored` |
| `DISC_C3573E2945A5A4514EEA` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_sections.rs` | `type_table_decode` | `not_ignored` |
| `DISC_54B9D55A890BE64BBC2C` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_sections.rs` | `var_meta_decode` | `not_ignored` |
| `DISC_790E7CD2B51AC6800D26` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_validation.rs` | `debug_map_validation` | `not_ignored` |
| `DISC_57639696D571884A9785` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_validation.rs` | `jump_validation` | `not_ignored` |
| `DISC_644141E632E9E3CD1370` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_validation.rs` | `opcode_validation` | `not_ignored` |
| `DISC_A7D063C4AAD7DB68FD24` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_validation.rs` | `opcode_validation_extended` | `not_ignored` |
| `DISC_A6FCBE2677A8C090BECF` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_validation.rs` | `validator_rejects_unsupported_runtime_opcodes_before_dispatch` | `not_ignored` |
| `DISC_EE4152F4361F181CB55B` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_validation.rs` | `var_meta_rejects_duplicate_ref_idx` | `not_ignored` |
| `DISC_6B26ADB3DF20D72F0DB6` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_validation.rs` | `var_meta_rejects_duplicate_textual_name_at_different_string_indices` | `not_ignored` |
| `DISC_9466D8EBC1CB0CF5F987` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_validation.rs` | `var_meta_rejects_local_ref_outside_every_pou_range` | `not_ignored` |
| `DISC_ABC6710BB8234C3E95BB` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_validation.rs` | `var_meta_rejects_local_retain_and_initializer_state` | `not_ignored` |
| `DISC_1ACB18866C2FC624B037` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/call_and_sizeof_validation.rs` | `vm_rejects_invalid_call_native_method_missing_receiver_payload` | `not_ignored` |
| `DISC_B4A1DBB63C27E22B3380` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/call_and_sizeof_validation.rs` | `vm_rejects_invalid_call_native_symbol_index` | `not_ignored` |
| `DISC_76C0ACD9DD9B4A02A05D` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/call_and_sizeof_validation.rs` | `vm_rejects_legacy_sizeof_value_opcode_with_empty_stack` | `not_ignored` |
| `DISC_FBBCEDC5A9C31E03C92A` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/call_and_sizeof_validation.rs` | `vm_rejects_load_dynamic_with_non_reference_operand` | `not_ignored` |
| `DISC_54FBE4E54CB84F9EA31A` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/call_and_sizeof_validation.rs` | `vm_rejects_sizeof_type_with_excessive_non_cyclic_alias_depth` | `not_ignored` |
| `DISC_70C232CD199EC1EEFB47` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/call_and_sizeof_validation.rs` | `vm_validator_rejects_invalid_ref_field_string_index` | `not_ignored` |
| `DISC_06C78E4FD9548CDFEC91` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/call_and_sizeof_validation.rs` | `vm_validator_rejects_invalid_sizeof_type_index` | `not_ignored` |
| `DISC_55D5957DD6826E1A6C0F` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/deadline.rs` | `vm_enforces_execution_deadline` | `not_ignored` |
| `DISC_E35FF38A598A52B9A748` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/deadline.rs` | `vm_forward_only_instruction_stream_enforces_execution_deadline` | `not_ignored` |
| `DISC_FB4371C17A9F9FB83CA9` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/fuzz_stack_call.rs` | `vm_malformed_bytecode_fuzz_smoke_budget` | `not_ignored` |
| `DISC_FEFED9A28C00D7D4A293` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/fuzz_stack_call.rs` | `vm_rejects_stack_overflow` | `not_ignored` |
| `DISC_8D25BBBA705FEE70C359` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/fuzz_stack_call.rs` | `vm_rejects_stack_underflow` | `not_ignored` |
| `DISC_547557A88A1751976E8D` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/fuzz_stack_call.rs` | `vm_validator_rejects_legacy_call_even_when_target_exists` | `not_ignored` |
| `DISC_4E8E4144D9AEFFEEE9C4` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/lowering_and_constants.rs` | `vm_enforces_instruction_budget` | `not_ignored` |
| `DISC_E8BDC2A27F6EC334ADB8` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/lowering_and_constants.rs` | `vm_rejects_invalid_string_const_utf8_payload` | `not_ignored` |
| `DISC_3308821824B623536278` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/lowering_and_constants.rs` | `vm_rejects_invalid_wstring_const_utf16_payload` | `not_ignored` |
| `DISC_FF8083458DA680264905` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/positive_paths.rs` | `vm_call_native_builtin_function_block_executes_body_and_copies_outputs` | `not_ignored` |
| `DISC_B25737EE83EB92376183` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/positive_paths.rs` | `vm_call_native_direct_binding_module_swap_reloads_default_metadata` | `not_ignored` |
| `DISC_F87BB0817C7C6B0D9ECF` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/positive_paths.rs` | `vm_call_native_direct_binding_preserves_named_default_out_and_inout_contracts` | `not_ignored` |
| `DISC_7A821AC093817FA4DE5D` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/positive_paths.rs` | `vm_call_native_method_polymorphic_receiver_dispatch_remains_correct` | `not_ignored` |
| `DISC_DA376209F7BAA2B22C10` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/positive_paths.rs` | `vm_executes_program_with_stack_and_pc_progression` | `not_ignored` |
| `DISC_E8E819DA022BA9E44082` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/positive_paths.rs` | `vm_opcode_positive_path_covers_arith_logical_branch_jump_load_store_ref` | `not_ignored` |
| `DISC_1D0B0F01E2BB2D9923F1` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/positive_paths.rs` | `vm_opcode_positive_path_covers_call_native_oop_dispatch` | `not_ignored` |
| `DISC_F5A9D54CD1CD046BC7B8` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/positive_paths.rs` | `vm_opcode_positive_path_covers_call_native_stdlib_dispatch` | `not_ignored` |
| `DISC_E20A636B92260667BC4A` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/positive_paths.rs` | `vm_opcode_positive_path_covers_dynamic_reference_and_nested_chains` | `not_ignored` |
| `DISC_065AA6C85532B9F6F104` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/positive_paths.rs` | `vm_opcode_positive_path_covers_nested_field_chain_assignments` | `not_ignored` |
| `DISC_E9E96D61B996E3376790` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/positive_paths.rs` | `vm_opcode_positive_path_covers_nested_field_index_assignments` | `not_ignored` |
| `DISC_68799F0150BA51B5C348` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/positive_paths.rs` | `vm_opcode_positive_path_covers_non_ascii_string_and_wstring_index_reads` | `not_ignored` |
| `DISC_B93F8641C78A05799F02` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/positive_paths.rs` | `vm_opcode_positive_path_covers_sizeof_type_and_storage_operands` | `not_ignored` |
| `DISC_C6163DD277DA618BCB8B` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/positive_paths.rs` | `vm_opcode_positive_path_covers_string_and_wstring_index_reads` | `not_ignored` |
| `DISC_5CC1E92E3E2A8DB6FBCC` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/positive_paths.rs` | `vm_opcode_positive_path_covers_string_and_wstring_literals` | `not_ignored` |
| `DISC_F1F0C6C285477DAB1239` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/ref_validation.rs` | `vm_rejects_invalid_jump_target` | `not_ignored` |
| `DISC_6D15F1D43CA3BDA8C505` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/ref_validation.rs` | `vm_rejects_invalid_opcode` | `not_ignored` |
| `DISC_51482BB47DB5575280CE` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/ref_validation.rs` | `vm_rejects_malformed_operands` | `not_ignored` |
| `DISC_FDE1B5B9C014CC5BF7BC` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/ref_validation.rs` | `vm_validator_rejects_duplicate_pou_ids` | `not_ignored` |
| `DISC_3A6CAB92BC8F60C11C2F` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/ref_validation.rs` | `vm_validator_rejects_invalid_const_index_operand` | `not_ignored` |
| `DISC_2017F195D5EEFC4FC922` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/ref_validation.rs` | `vm_validator_rejects_invalid_ref_index_operand` | `not_ignored` |
| `DISC_66836DF45D5768143A94` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_core/ref_validation.rs` | `vm_validator_rejects_unsupported_call_method_opcode` | `not_ignored` |
| `DISC_5FE5A5E7AEECAD1E3875` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_differential.rs` | `register_and_stack_paths_match_for_composite_value_program` | `not_ignored` |
| `DISC_11A95B0F67F0AD60B921` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_differential.rs` | `register_and_stack_paths_match_for_deep_ref_chain_field_index_parity` | `not_ignored` |
| `DISC_DB36633EAE7190016564` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_differential.rs` | `register_and_stack_paths_match_for_string_wstring_edge_indices` | `not_ignored` |
| `DISC_9FCB086B2BAFA238ABD3` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_differential.rs` | `register_and_stack_paths_match_for_unqualified_enum_case_labels` | `not_ignored` |
| `DISC_573B089D4C9B1FFCF17D` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_differential.rs` | `register_and_stack_paths_surface_same_deep_ref_chain_index_trap` | `not_ignored` |
| `DISC_2C9E5D4186ACD1FA970C` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_differential.rs` | `register_and_stack_paths_surface_same_modulo_by_zero_error` | `not_ignored` |
| `DISC_620FE84AA15888BDC6AF` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_differential.rs` | `register_and_stack_paths_surface_same_string_wstring_index_traps` | `not_ignored` |
| `DISC_E620776DF0DA52F9FA84` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_enum_unqualified.rs` | `enum_alias_and_mixed_case_literals_share_canonical_identity` | `not_ignored` |
| `DISC_FD2E985779D61315CEF8` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_enum_unqualified.rs` | `unqualified_enum_assignment_respects_same_named_local_shadowing` | `not_ignored` |
| `DISC_5835FB53AE5F042D7A23` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_enum_unqualified.rs` | `unqualified_enum_comparison_respects_same_named_local_shadowing` | `not_ignored` |
| `DISC_FFEC9A61A416CA84AFE5` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_enum_unqualified.rs` | `unqualified_enum_initializer_respects_same_named_constant_shadowing` | `not_ignored` |
| `DISC_43F06CBDE81310965D6F` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_enum_unqualified.rs` | `unqualified_enum_variant_case_label_with_field_selector` | `not_ignored` |
| `DISC_5E7436E6790939F624CC` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_enum_unqualified.rs` | `unqualified_enum_variant_case_label_with_indexed_field_selector` | `not_ignored` |
| `DISC_252FB763E5007FF4E50A` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_enum_unqualified.rs` | `unqualified_enum_variant_case_label_with_indexed_selector` | `not_ignored` |
| `DISC_FBEFC87ADDCCA23C1783` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_enum_unqualified.rs` | `unqualified_enum_variant_comparison_matches_when_values_equal` | `not_ignored` |
| `DISC_360CC21BEBAEE44F2F01` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_enum_unqualified.rs` | `unqualified_enum_variant_initializes_var_to_declared_variant` | `not_ignored` |
| `DISC_C247C2CDC33D1522F56C` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_enum_unqualified.rs` | `unqualified_enum_variant_rvalue_assigns_expected_variant` | `not_ignored` |
| `DISC_399FD9A166489E54F964` | `rust_integration_test` | `crates/trust-runtime/tests/bytecode_vm_enum_unqualified.rs` | `var_initialized_enum_compares_equal_to_its_declared_variant` | `not_ignored` |
| `DISC_CA31730D2D29CE8A35D2` | `rust_integration_test` | `crates/trust-runtime/tests/check_command.rs` | `check_accepts_project_without_writing_program_stbc` | `not_ignored` |
| `DISC_F64E321237F508FFC2AC` | `rust_integration_test` | `crates/trust-runtime/tests/check_command.rs` | `check_reports_compile_error_as_json_issue` | `not_ignored` |
| `DISC_6F3F154A1A548EAC462B` | `rust_integration_test` | `crates/trust-runtime/tests/check_command.rs` | `check_reports_config_error_as_json_issue` | `not_ignored` |
| `DISC_627DBD026621561D0A46` | `rust_integration_test` | `crates/trust-runtime/tests/ci_cicd_contract.rs` | `ci_clean_setup_first_passing_test_is_under_ten_minutes` | `not_ignored` |
| `DISC_C1D0E6F5C2D69B6C4E7D` | `rust_integration_test` | `crates/trust-runtime/tests/ci_cicd_contract.rs` | `ci_flake_probe_script_emits_machine_readable_sample` | `not_ignored` |
| `DISC_85AF4E1985BEEBE3A2A6` | `rust_integration_test` | `crates/trust-runtime/tests/ci_cicd_contract.rs` | `ci_json_summary_mode_contract_is_stable` | `not_ignored` |
| `DISC_121D3E8004F58F16B876` | `rust_integration_test` | `crates/trust-runtime/tests/ci_cicd_contract.rs` | `ci_nightly_workflow_exposes_dispatch_artifacts_and_gate_enforcement` | `not_ignored` |
| `DISC_71086B098E124BEC1075` | `rust_integration_test` | `crates/trust-runtime/tests/ci_cicd_contract.rs` | `ci_release_gate_report_fails_when_required_gate_artifact_is_missing` | `not_ignored` |
| `DISC_A602B8224521637DF221` | `rust_integration_test` | `crates/trust-runtime/tests/ci_cicd_contract.rs` | `ci_release_gate_report_passes_when_required_gate_artifacts_are_present` | `not_ignored` |
| `DISC_DB05019779BB1704907F` | `rust_integration_test` | `crates/trust-runtime/tests/ci_cicd_contract.rs` | `ci_reliability_summary_gate_returns_non_zero_on_budget_breach` | `not_ignored` |
| `DISC_26D85E382B6B90BB76E6` | `rust_integration_test` | `crates/trust-runtime/tests/ci_cicd_contract.rs` | `ci_run_all_tests_scales_roughly_linearly_for_small_projects` | `not_ignored` |
| `DISC_A7027A42E2C68847E276` | `rust_integration_test` | `crates/trust-runtime/tests/ci_cicd_contract.rs` | `ci_single_test_feedback_is_under_two_seconds_for_small_project` | `not_ignored` |
| `DISC_6047076294C275FF229D` | `rust_integration_test` | `crates/trust-runtime/tests/ci_cicd_contract.rs` | `ci_template_file_contains_expected_command_sequence` | `not_ignored` |
| `DISC_AFC7F8F303A4F231290C` | `rust_integration_test` | `crates/trust-runtime/tests/ci_cicd_contract.rs` | `ci_template_workflow_fails_on_broken_fixture_with_expected_code_and_report` | `not_ignored` |
| `DISC_4692745A9A337DAF089C` | `rust_integration_test` | `crates/trust-runtime/tests/ci_cicd_contract.rs` | `ci_template_workflow_passes_on_green_fixture` | `not_ignored` |
| `DISC_4E461DFE780B510B8F1D` | `rust_integration_test` | `crates/trust-runtime/tests/ci_cicd_contract.rs` | `ci_vscode_extension_job_contract_wires_failure_to_release_gate` | `not_ignored` |
| `DISC_DC570CD7C3B32C99226A` | `rust_integration_test` | `crates/trust-runtime/tests/commit_command.rs` | `trust_dev_commit_dry_run_reports_project_changes` | `not_ignored` |
| `DISC_3F2F79014B107FAB0E5D` | `rust_integration_test` | `crates/trust-runtime/tests/commit_command.rs` | `trust_runtime_commit_alias_forwards_to_trust_dev_with_deprecation_warning` | `not_ignored` |
| `DISC_2417C39109096931D2C6` | `rust_integration_test` | `crates/trust-runtime/tests/communication_examples_cli.rs` | `communication_examples_build_and_validate` | `not_ignored` |
| `DISC_95016C0E7A51761E8C27` | `rust_integration_test` | `crates/trust-runtime/tests/compile_time_constants.rs` | `cross_file_global_constants_drive_string_lengths_in_runtime_and_vm` | `not_ignored` |
| `DISC_E5D19CF990ACA21C2072` | `rust_integration_test` | `crates/trust-runtime/tests/compile_time_constants.rs` | `named_constants_drive_parenthesized_string_lengths_in_runtime_and_vm` | `not_ignored` |
| `DISC_E39377BC8B380088AEB9` | `rust_integration_test` | `crates/trust-runtime/tests/complete_program.rs` | `complete_program_compiles_without_errors` | `not_ignored` |
| `DISC_5171DC86A5538D301CC4` | `rust_integration_test` | `crates/trust-runtime/tests/config_schema_command.rs` | `validate_accepts_canonical_schema_fixture` | `not_ignored` |
| `DISC_4F670FA3EEEE0B796B4C` | `rust_integration_test` | `crates/trust-runtime/tests/config_schema_command.rs` | `validate_rejects_io_type_error_after_offline_edit` | `not_ignored` |
| `DISC_D8345C96D795C55B4317` | `rust_integration_test` | `crates/trust-runtime/tests/config_schema_command.rs` | `validate_rejects_runtime_range_error_after_offline_edit` | `not_ignored` |
| `DISC_06644349C95B13287BCC` | `rust_integration_test` | `crates/trust-runtime/tests/config_schema_command.rs` | `validate_rejects_runtime_unknown_key_after_offline_edit` | `not_ignored` |
| `DISC_684DA7560289227FCEF1` | `rust_integration_test` | `crates/trust-runtime/tests/conformance_cli_command.rs` | `conformance_command_supports_update_verify_and_mismatch_taxonomy` | `not_ignored` |
| `DISC_A11A3EC89B0848F97F9F` | `rust_integration_test` | `crates/trust-runtime/tests/connectors_status.rs` | `active_ads_device_snapshot_projects_point_rows_and_counts` | `not_ignored` |
| `DISC_5A83F98038D22DC3CF2D` | `rust_integration_test` | `crates/trust-runtime/tests/connectors_status.rs` | `ads_point_statuses_project_into_connector_point_quality` | `not_ignored` |
| `DISC_A693DC52B922A4949B98` | `rust_integration_test` | `crates/trust-runtime/tests/connectors_status.rs` | `ads_state_mapping_covers_worker_and_report_states` | `not_ignored` |
| `DISC_0C0EB84CEED3AB9EB582` | `rust_integration_test` | `crates/trust-runtime/tests/connectors_status.rs` | `ads_status_report_projects_reconnect_stale_fault_and_failure_details` | `not_ignored` |
| `DISC_D101156555E69D07F53D` | `rust_integration_test` | `crates/trust-runtime/tests/connectors_status.rs` | `ads_status_report_projects_role_endpoint_and_point_counts` | `not_ignored` |
| `DISC_2B71CDA6D428E5AA3600` | `rust_integration_test` | `crates/trust-runtime/tests/connectors_status.rs` | `connector_status_report_serializes_stable_schema` | `not_ignored` |
| `DISC_673146F9407CFA0E2A7B` | `rust_integration_test` | `crates/trust-runtime/tests/connectors_status.rs` | `discovery_confidence_serializes_honest_tcp_only_label` | `not_ignored` |
| `DISC_4FBDF6E7EFCD79AAF13A` | `rust_integration_test` | `crates/trust-runtime/tests/connectors_status.rs` | `io_driver_health_mapping_honors_error_policy` | `not_ignored` |
| `DISC_6541A691564505602302` | `rust_integration_test` | `crates/trust-runtime/tests/connectors_status.rs` | `opcua_client_status_projects_point_quality_and_metadata` | `not_ignored` |
| `DISC_67AFA6F8A2DB5766FF6D` | `rust_integration_test` | `crates/trust-runtime/tests/connectors_status.rs` | `opcua_mapping_covers_client_and_server_states` | `not_ignored` |
| `DISC_52F76E1274CF56E0704C` | `rust_integration_test` | `crates/trust-runtime/tests/connectors_status.rs` | `process_image_protocol_mappings_cover_mqtt_modbus_and_ethercat` | `not_ignored` |
| `DISC_7ABC26CF210FA536482F` | `rust_integration_test` | `crates/trust-runtime/tests/connectors_status.rs` | `stale_connector_state_and_stale_point_quality_are_distinct_fields` | `not_ignored` |
| `DISC_2E9172DF9B9DA9975F9C` | `rust_integration_test` | `crates/trust-runtime/tests/datetime_profile.rs` | `default_profile` | `not_ignored` |
| `DISC_976F3E8A9E42B3047406` | `rust_integration_test` | `crates/trust-runtime/tests/datetime_profile.rs` | `timezone_naive` | `not_ignored` |
| `DISC_A5E6FC01E9F799591FEB` | `rust_integration_test` | `crates/trust-runtime/tests/datetime_range.rs` | `out_of_range_error` | `not_ignored` |
| `DISC_CCBBCCECC2ABCFE340BA` | `rust_integration_test` | `crates/trust-runtime/tests/datetime_types.rs` | `ltime_epoch_and_units` | `not_ignored` |
| `DISC_74070CA2ED3F2DF9C39C` | `rust_integration_test` | `crates/trust-runtime/tests/debug_control.rs` | `breakpoint_emits_stop_event` | `not_ignored` |
| `DISC_92B081AFB9BC90E206D6` | `rust_integration_test` | `crates/trust-runtime/tests/debug_control.rs` | `breakpoint_generation_increments_on_clear` | `not_ignored` |
| `DISC_2C744D790225D90AF217` | `rust_integration_test` | `crates/trust-runtime/tests/debug_control.rs` | `breakpoint_pauses_execution` | `not_ignored` |
| `DISC_E2618AAFF9EE0C3C6E82` | `rust_integration_test` | `crates/trust-runtime/tests/debug_control.rs` | `conditional_breakpoint_pauses_when_true` | `not_ignored` |
| `DISC_8CAB7A4337CA6BE04E7F` | `rust_integration_test` | `crates/trust-runtime/tests/debug_control.rs` | `conditional_breakpoint_skips_when_false` | `not_ignored` |
| `DISC_85B4ED878073A5E30340` | `rust_integration_test` | `crates/trust-runtime/tests/debug_control.rs` | `frame_location_tracks_current_frame` | `not_ignored` |
| `DISC_6E1D8338155F08DD3A00` | `rust_integration_test` | `crates/trust-runtime/tests/debug_control.rs` | `hit_count_breakpoint_pauses_on_threshold` | `not_ignored` |
| `DISC_E74444F2E944A7346A1B` | `rust_integration_test` | `crates/trust-runtime/tests/debug_control.rs` | `logpoint_emits_output_without_pausing` | `not_ignored` |
| `DISC_8CC0C32A02C75700E3A8` | `rust_integration_test` | `crates/trust-runtime/tests/debug_control.rs` | `logpoint_sender_drop_buffers_log_in_debug_control` | `not_ignored` |
| `DISC_E8FF497B1411878B1442` | `rust_integration_test` | `crates/trust-runtime/tests/debug_control.rs` | `pause_preserves_task_order` | `not_ignored` |
| `DISC_6A54BECFBCEE4FD899FE` | `rust_integration_test` | `crates/trust-runtime/tests/debug_control.rs` | `resolve_breakpoint_next_statement` | `not_ignored` |
| `DISC_2A96A0E41EBCE64E1BC9` | `rust_integration_test` | `crates/trust-runtime/tests/debug_control.rs` | `resolve_breakpoint_prefers_inner_statement` | `not_ignored` |
| `DISC_409932463EB8BCB34BAF` | `rust_integration_test` | `crates/trust-runtime/tests/debug_control.rs` | `runtime_event_sender_drop_buffers_event_in_debug_control` | `not_ignored` |
| `DISC_5AA958453C04F575A73F` | `rust_integration_test` | `crates/trust-runtime/tests/debug_control.rs` | `runtime_resolves_breakpoint_position_to_statement_start` | `not_ignored` |
| `DISC_EED9C5DD30F925D42EE7` | `rust_integration_test` | `crates/trust-runtime/tests/debug_control.rs` | `runtime_resolves_breakpoint_using_index` | `not_ignored` |
| `DISC_886FA242D29116F3F256` | `rust_integration_test` | `crates/trust-runtime/tests/debug_control.rs` | `statement_locations_use_first_token_in_if_block` | `not_ignored` |
| `DISC_2FE8A15EB7130A04B428` | `rust_integration_test` | `crates/trust-runtime/tests/debug_control.rs` | `step_once_pauses_again` | `not_ignored` |
| `DISC_32C281174ED571C3F5BA` | `rust_integration_test` | `crates/trust-runtime/tests/debug_control.rs` | `step_out_pauses_after_return` | `not_ignored` |
| `DISC_7A66519B6D69169281B8` | `rust_integration_test` | `crates/trust-runtime/tests/debug_control.rs` | `step_over_pauses_at_same_depth` | `not_ignored` |
| `DISC_E0B5E92D4B643A8AE264` | `rust_integration_test` | `crates/trust-runtime/tests/debug_control.rs` | `watch_changes_reported_between_stops` | `not_ignored` |
| `DISC_F843BBAC95455806869E` | `rust_integration_test` | `crates/trust-runtime/tests/debug_stepping.rs` | `breakpoint_only_triggers_for_taken_branch` | `not_ignored` |
| `DISC_50499CED3E53E85658F0` | `rust_integration_test` | `crates/trust-runtime/tests/debug_stepping.rs` | `breakpoint_rehits_each_cycle_after_continue` | `not_ignored` |
| `DISC_AB9912DEAE5F1A0ED30E` | `rust_integration_test` | `crates/trust-runtime/tests/debug_stepping.rs` | `breakpoint_set_after_launch_hits_next_cycle` | `not_ignored` |
| `DISC_B85454F23BEECE60D719` | `rust_integration_test` | `crates/trust-runtime/tests/debug_stepping.rs` | `breakpoint_set_while_running_hits_on_subsequent_cycle` | `not_ignored` |
| `DISC_6AB566D57B66572F3581` | `rust_integration_test` | `crates/trust-runtime/tests/debug_stepping.rs` | `breakpoint_triggers_for_executed_branch` | `not_ignored` |
| `DISC_B5892FF80FD31175F681` | `rust_integration_test` | `crates/trust-runtime/tests/debug_stepping.rs` | `step_over_stops_in_caller_after_call` | `not_ignored` |
| `DISC_DE236DFA7B3C8EF00B7F` | `rust_integration_test` | `crates/trust-runtime/tests/debug_stepping.rs` | `vm_breakpoint_populates_debug_snapshot_for_stack_queries` | `not_ignored` |
| `DISC_17F0B2537903A955BC0C` | `rust_integration_test` | `crates/trust-runtime/tests/determinism.rs` | `ordered_execution` | `not_ignored` |
| `DISC_FE262D7963202D5AFB9D` | `rust_integration_test` | `crates/trust-runtime/tests/device_in_the_loop.rs` | `ads_lab_twincat_doctor_records_status_json` | `ignored` |
| `DISC_26934332D29338103AFD` | `rust_integration_test` | `crates/trust-runtime/tests/device_in_the_loop.rs` | `ethercat_lab_hardware_discovery_records_topology` | `ignored` |
| `DISC_A007AD886E7ABB17EF37` | `rust_integration_test` | `crates/trust-runtime/tests/device_in_the_loop.rs` | `ethercat_lab_pdu_storage_stress_records_artifact` | `ignored` |
| `DISC_EC41FC62FBEF09EFDFE8` | `rust_integration_test` | `crates/trust-runtime/tests/device_in_the_loop.rs` | `modbus_lab_target_confirms_protocol_probe` | `ignored` |
| `DISC_75F421CF89F3935418D6` | `rust_integration_test` | `crates/trust-runtime/tests/device_in_the_loop.rs` | `mqtt_lab_broker_records_auth_tls_reconnect_and_disconnect` | `ignored` |
| `DISC_C9CC8AC45A88639AE35D` | `rust_integration_test` | `crates/trust-runtime/tests/docs_command.rs` | `docs_command_generates_markdown_and_html` | `not_ignored` |
| `DISC_7E1297E7B0ECD808A43A` | `rust_integration_test` | `crates/trust-runtime/tests/docs_command.rs` | `trust_runtime_docs_alias_forwards_to_trust_dev` | `not_ignored` |
| `DISC_A5FDC9D4784BC1D2C9C6` | `rust_integration_test` | `crates/trust-runtime/tests/e2e_basic.rs` | `from_source_full_pipeline` | `not_ignored` |
| `DISC_A7161A39E0C608451D8E` | `rust_integration_test` | `crates/trust-runtime/tests/e2e_comments.rs` | `comment_tolerance` | `not_ignored` |
| `DISC_EBDD9094F4FA5BB09D4F` | `rust_integration_test` | `crates/trust-runtime/tests/e2e_configuration.rs` | `resource_tasks` | `not_ignored` |
| `DISC_32C5B1A7D0C87A189635` | `rust_integration_test` | `crates/trust-runtime/tests/e2e_full_grammar.rs` | `var_and_stmt_coverage` | `not_ignored` |
| `DISC_8FFAEF4683A05CC09B12` | `rust_integration_test` | `crates/trust-runtime/tests/e2e_multifile.rs` | `duplicate_program_name_errors` | `not_ignored` |
| `DISC_ABFDD7450BAC9E9C9097` | `rust_integration_test` | `crates/trust-runtime/tests/e2e_multifile.rs` | `namespace_resolution` | `not_ignored` |
| `DISC_8AAC146F12AF0309C40B` | `rust_integration_test` | `crates/trust-runtime/tests/e2e_multifile.rs` | `namespaced_pous_resolve_sibling_interfaces` | `not_ignored` |
| `DISC_9DE91547140FFDC2347C` | `rust_integration_test` | `crates/trust-runtime/tests/e2e_multifile.rs` | `namespaced_programs_are_runtime_entry_points` | `not_ignored` |
| `DISC_F6508FDFBB3BD1D4F440` | `rust_integration_test` | `crates/trust-runtime/tests/e2e_scheduler.rs` | `backward_clock_step_does_not_replay_or_overrun_periodic_task` | `not_ignored` |
| `DISC_D8D92D6082D7DAB2FA44` | `rust_integration_test` | `crates/trust-runtime/tests/e2e_scheduler.rs` | `overrun_and_fault` | `not_ignored` |
| `DISC_B02C23A795AF36ACE7B5` | `rust_integration_test` | `crates/trust-runtime/tests/e2e_scheduler.rs` | `periodic_and_event` | `not_ignored` |
| `DISC_891F1D234DBDE29429A4` | `rust_integration_test` | `crates/trust-runtime/tests/e2e_scheduler.rs` | `resumed_clock_jump_runs_once_and_records_missed_intervals` | `not_ignored` |
| `DISC_6CBE8E64A10088366CF5` | `rust_integration_test` | `crates/trust-runtime/tests/e2e_scheduler.rs` | `trace_determinism` | `not_ignored` |
| `DISC_AAE6B259EF303AE3948D` | `rust_integration_test` | `crates/trust-runtime/tests/e2e_semicolons.rs` | `trailing_semicolons` | `not_ignored` |
| `DISC_CB751FF7780AC7752130` | `rust_integration_test` | `crates/trust-runtime/tests/errors_policy.rs` | `error_policy` | `not_ignored` |
| `DISC_06821893F4327F36D9E8` | `rust_integration_test` | `crates/trust-runtime/tests/ethercat_driver.rs` | `ethercat_cycle_warn_threshold_reports_degraded_health` | `not_ignored` |
| `DISC_B8264EC4F849EE66B2CA` | `rust_integration_test` | `crates/trust-runtime/tests/ethercat_driver.rs` | `ethercat_ignore_policy_degrades_without_runtime_cycle_error` | `not_ignored` |
| `DISC_6F665C0AE3F1C64063FF` | `rust_integration_test` | `crates/trust-runtime/tests/ethercat_driver.rs` | `ethercat_image_size_mismatch_faults_under_warn_policy` | `not_ignored` |
| `DISC_B9ACE9B650FE5BFE0DC6` | `rust_integration_test` | `crates/trust-runtime/tests/ethercat_driver.rs` | `ethercat_missing_adapter_post_allocation_failure_is_terminal_until_rebuild` | `not_ignored` |
| `DISC_083918C0A1695BEDB92E` | `rust_integration_test` | `crates/trust-runtime/tests/ethercat_driver.rs` | `ethercat_missing_adapter_records_pdu_storage_retry_baseline` | `ignored` |
| `DISC_791BAC7290836FAA6F59` | `rust_integration_test` | `crates/trust-runtime/tests/ethercat_driver.rs` | `ethercat_mock_profile_maps_ek1100_elx008_process_image` | `not_ignored` |
| `DISC_C5B92BB21025CAE02946` | `rust_integration_test` | `crates/trust-runtime/tests/ethercat_driver.rs` | `ethercat_warn_policy_degrades_without_runtime_cycle_error` | `not_ignored` |
| `DISC_C61007734167A2A4CE7E` | `rust_integration_test` | `crates/trust-runtime/tests/expr_calls.rs` | `function_block_call` | `not_ignored` |
| `DISC_D125E8A7C82805AEE276` | `rust_integration_test` | `crates/trust-runtime/tests/expr_calls.rs` | `function_call_expr` | `not_ignored` |
| `DISC_4670B9A0570719575EB2` | `rust_integration_test` | `crates/trust-runtime/tests/expr_calls.rs` | `function_call_named_args` | `not_ignored` |
| `DISC_3FEF14488929BE4E886D` | `rust_integration_test` | `crates/trust-runtime/tests/expr_calls.rs` | `function_call_output_positional` | `not_ignored` |
| `DISC_2D17DC9BA441E47099C3` | `rust_integration_test` | `crates/trust-runtime/tests/expr_calls.rs` | `function_in_out_with_conversion_expression_regression_issue_13` | `not_ignored` |
| `DISC_9754A8A2443EE9A1BED9` | `rust_integration_test` | `crates/trust-runtime/tests/expr_calls.rs` | `function_in_out_without_conversion_expression_baseline` | `not_ignored` |
| `DISC_B0436E5C5E4D4AF3C0B8` | `rust_integration_test` | `crates/trust-runtime/tests/expr_calls.rs` | `stdlib_named_args` | `not_ignored` |
| `DISC_F0AE877FD378B8B562E5` | `rust_integration_test` | `crates/trust-runtime/tests/fb_bistable.rs` | `sr_rs` | `not_ignored` |
| `DISC_DF647144552A51EC968B` | `rust_integration_test` | `crates/trust-runtime/tests/fb_counters.rs` | `ctu_ctd_ctud` | `not_ignored` |
| `DISC_83A89DA2CD75420C5772` | `rust_integration_test` | `crates/trust-runtime/tests/fb_counters_full.rs` | `counter_variants` | `not_ignored` |
| `DISC_783A92F25FF61F57AAA5` | `rust_integration_test` | `crates/trust-runtime/tests/fb_counters_full.rs` | `generic_counter_uses_call_value_type_after_null_default` | `not_ignored` |
| `DISC_AA62E0EB2E654D81531D` | `rust_integration_test` | `crates/trust-runtime/tests/fb_edges.rs` | `difu_difd_aliases_match_edge_behavior` | `not_ignored` |
| `DISC_D74EF325BFDD0EF22B1F` | `rust_integration_test` | `crates/trust-runtime/tests/fb_edges.rs` | `r_trig_f_trig` | `not_ignored` |
| `DISC_2CE44F0CAE069A77A47E` | `rust_integration_test` | `crates/trust-runtime/tests/fb_timers.rs` | `ton_tof_tp` | `not_ignored` |
| `DISC_F5D5DA61FED4441EC645` | `rust_integration_test` | `crates/trust-runtime/tests/gpio_safe_state.rs` | `gpio_safe_state_writes_outputs_on_fault` | `not_ignored` |
| `DISC_229337EE83AC8DC7C44C` | `rust_integration_test` | `crates/trust-runtime/tests/harness.rs` | `from_source` | `not_ignored` |
| `DISC_7175167F66DE6A7A9F78` | `rust_integration_test` | `crates/trust-runtime/tests/harness.rs` | `io_by_address` | `not_ignored` |
| `DISC_809C3B3E7FA84C5C1498` | `rust_integration_test` | `crates/trust-runtime/tests/harness.rs` | `io_by_name` | `not_ignored` |
| `DISC_F10AD29A2D63DBFAF259` | `rust_integration_test` | `crates/trust-runtime/tests/harness.rs` | `run_controls` | `not_ignored` |
| `DISC_2ADD66FDDFB48846A2D9` | `rust_integration_test` | `crates/trust-runtime/tests/harness.rs` | `run_until_max_panics_when_limit_is_exceeded` | `not_ignored` |
| `DISC_F613924421CB2A4B18CB` | `rust_integration_test` | `crates/trust-runtime/tests/harness.rs` | `run_until_returns_immediately_when_condition_is_already_true` | `not_ignored` |
| `DISC_2F780CF67C0C68670372` | `rust_integration_test` | `crates/trust-runtime/tests/harness_fail_closed.rs` | `bind_direct_typo_returns_boundary_error_not_silent_binding` | `not_ignored` |
| `DISC_05CC41C865D947DD859F` | `rust_integration_test` | `crates/trust-runtime/tests/harness_fail_closed.rs` | `declared_null_like_values_are_not_missing_name_errors` | `not_ignored` |
| `DISC_EDBEFF5DC3190173D71B` | `rust_integration_test` | `crates/trust-runtime/tests/harness_fail_closed.rs` | `set_input_typo_returns_boundary_error_not_silent_global_create` | `not_ignored` |
| `DISC_4AE9A8D353C1E6E0C683` | `rust_integration_test` | `crates/trust-runtime/tests/hmi_readonly_integration/hmi_readonly_integration_part_01.rs` | `hmi_dashboard_routes_render_without_manual_layout` | `not_ignored` |
| `DISC_8AC7EF9736D2433E07EF` | `rust_integration_test` | `crates/trust-runtime/tests/hmi_readonly_integration/hmi_readonly_integration_part_01.rs` | `hmi_schema_exposes_section_spans_and_widget_spans_for_web_layout` | `not_ignored` |
| `DISC_8195EB0C9952F58F0A13` | `rust_integration_test` | `crates/trust-runtime/tests/hmi_readonly_integration/hmi_readonly_integration_part_02.rs` | `hmi_standalone_export_bundle_contains_assets_routes_and_config` | `not_ignored` |
| `DISC_764455DC84D00D7010ED` | `rust_integration_test` | `crates/trust-runtime/tests/hmi_readonly_integration/hmi_readonly_integration_part_02.rs` | `hmi_standalone_export_bundle_includes_resolved_descriptor_when_hmi_dir_present` | `not_ignored` |
| `DISC_DF679A6EE690D3521B7D` | `rust_integration_test` | `crates/trust-runtime/tests/hmi_readonly_integration/hmi_readonly_integration_part_03_part_01.rs` | `hmi_standalone_export_bundle_validates_offline_bootstrap_with_embedded_schema` | `not_ignored` |
| `DISC_4B362FC80A7CC92583FD` | `rust_integration_test` | `crates/trust-runtime/tests/hmi_readonly_integration/hmi_readonly_integration_part_04.rs` | `hmi_websocket_forced_failure_polling_recovers_within_one_interval` | `not_ignored` |
| `DISC_2E2E22285F0FD5F1666B` | `rust_integration_test` | `crates/trust-runtime/tests/hmi_readonly_integration/hmi_readonly_integration_part_04.rs` | `hmi_websocket_pushes_values_schema_revision_and_alarm_events` | `not_ignored` |
| `DISC_1F69D92EC8C260892A74` | `rust_integration_test` | `crates/trust-runtime/tests/hmi_readonly_integration/hmi_readonly_integration_part_04.rs` | `hmi_websocket_reconnect_churn_remains_stable` | `not_ignored` |
| `DISC_50F0CBE7F09ADCF6125A` | `rust_integration_test` | `crates/trust-runtime/tests/hmi_readonly_integration/hmi_readonly_integration_part_04.rs` | `hmi_websocket_slow_consumers_do_not_block_control_plane` | `not_ignored` |
| `DISC_54F6B200F7B5A343EBED` | `rust_integration_test` | `crates/trust-runtime/tests/hmi_readonly_integration/hmi_readonly_integration_part_04.rs` | `hmi_websocket_value_push_meets_local_latency_slo` | `not_ignored` |
| `DISC_FD6A0B0E51FD882E157B` | `rust_integration_test` | `crates/trust-runtime/tests/hmi_readonly_integration/hmi_readonly_integration_part_05.rs` | `hmi_process_binding_transforms_update_fill_opacity_text_y_and_height` | `not_ignored` |
| `DISC_FB3425E0F781D985BEBE` | `rust_integration_test` | `crates/trust-runtime/tests/hmi_readonly_integration/hmi_readonly_integration_part_05.rs` | `hmi_process_page_schema_and_svg_asset_route_render` | `not_ignored` |
| `DISC_2C92CDF668AE8249D137` | `rust_integration_test` | `crates/trust-runtime/tests/hmi_readonly_integration/hmi_readonly_integration_part_06.rs` | `hmi_process_renderer_handles_malformed_svg_without_crash` | `not_ignored` |
| `DISC_BEC262C381A423B431B6` | `rust_integration_test` | `crates/trust-runtime/tests/hmi_readonly_integration/hmi_readonly_integration_part_06.rs` | `hmi_process_renderer_rewrites_relative_svg_asset_references` | `not_ignored` |
| `DISC_415F390FC293AA24F1ED` | `rust_integration_test` | `crates/trust-runtime/tests/hmi_readonly_integration/hmi_readonly_integration_part_07.rs` | `hmi_connector_summary_renders_shared_status_contract` | `not_ignored` |
| `DISC_0CB6E85705D0168A2FE0` | `rust_integration_test` | `crates/trust-runtime/tests/hmi_readonly_integration/hmi_readonly_integration_part_07.rs` | `hmi_widget_renderers_handle_null_stale_and_good_values` | `not_ignored` |
| `DISC_DB4609D5670B83F6907D` | `rust_integration_test` | `crates/trust-runtime/tests/hmi_readonly_integration/hmi_readonly_integration_part_08.rs` | `hmi_polling_stays_under_cycle_budget` | `not_ignored` |
| `DISC_1A571724656D51DD106C` | `rust_integration_test` | `crates/trust-runtime/tests/hmi_readonly_integration/hmi_readonly_integration_part_08.rs` | `hmi_process_asset_pack_templates_and_bindings_align` | `not_ignored` |
| `DISC_0623C8F5E23E0C5DB233` | `rust_integration_test` | `crates/trust-runtime/tests/hmi_readonly_integration/hmi_readonly_integration_part_08.rs` | `hmi_responsive_layout_breakpoint_classes_cover_mobile_tablet_desktop` | `not_ignored` |
| `DISC_4204FDA9FA20ED44B4D7` | `rust_integration_test` | `crates/trust-runtime/tests/hmi_readonly_integration/hmi_readonly_integration_part_09.rs` | `hmi_polling_soak_remains_stable` | `not_ignored` |
| `DISC_E067B79A22B8500C7619` | `rust_integration_test` | `crates/trust-runtime/tests/hot_reload.rs` | `hot_reload_changed_body_restarts_at_entrypoint_policy` | `not_ignored` |
| `DISC_6881AED77BE0CFD99ED5` | `rust_integration_test` | `crates/trust-runtime/tests/hot_reload.rs` | `hot_reload_cycle_boundary_contract_holds_for_vm_reload` | `not_ignored` |
| `DISC_536B258421ADD8E350A4` | `rust_integration_test` | `crates/trust-runtime/tests/hot_reload.rs` | `hot_reload_invalid_module_reports_deterministic_error` | `not_ignored` |
| `DISC_6D63E21F9D9B39D9AF6D` | `rust_integration_test` | `crates/trust-runtime/tests/hot_reload.rs` | `hot_reload_migrates_retain_and_resets_nonretain_and_instances` | `not_ignored` |
| `DISC_4F2995A2CBD7C4CB4598` | `rust_integration_test` | `crates/trust-runtime/tests/hot_reload.rs` | `hot_reload_rebinds_instance_backed_io_after_warm_restart` | `not_ignored` |
| `DISC_12CDA0A01DC2A2690C4A` | `rust_integration_test` | `crates/trust-runtime/tests/iec_counters.rs` | `counter_examples` | `not_ignored` |
| `DISC_0E7B87A0E23C74E7BA73` | `rust_integration_test` | `crates/trust-runtime/tests/iec_examples.rs` | `conversion_usage_examples` | `not_ignored` |
| `DISC_E3109D9552128C78FFF0` | `rust_integration_test` | `crates/trust-runtime/tests/iec_examples.rs` | `logic_usage_example` | `not_ignored` |
| `DISC_F7BDA9084DD4136AE69D` | `rust_integration_test` | `crates/trust-runtime/tests/iec_examples.rs` | `table_examples` | `not_ignored` |
| `DISC_FB9C21E77224EA856F14` | `rust_integration_test` | `crates/trust-runtime/tests/iec_timers.rs` | `timing_diagrams` | `not_ignored` |
| `DISC_5CB0738F4F5DD699DBEF` | `rust_integration_test` | `crates/trust-runtime/tests/init_fail_closed.rs` | `debug_queued_global_write_unknown_target_fails` | `not_ignored` |
| `DISC_6EB69C59453ED035612E` | `rust_integration_test` | `crates/trust-runtime/tests/init_fail_closed.rs` | `debug_queued_lvalue_write_failure_is_observable` | `not_ignored` |
| `DISC_585BAF2FD96F5CBFED48` | `rust_integration_test` | `crates/trust-runtime/tests/init_fail_closed.rs` | `interface_param_defaults_to_null_reference` | `not_ignored` |
| `DISC_2783C316D82A0848BAF3` | `rust_integration_test` | `crates/trust-runtime/tests/initializer_architecture.rs` | `dependency_boundaries_for_initializer_metadata_hold` | `not_ignored` |
| `DISC_DE303F4803C88366FF3C` | `rust_integration_test` | `crates/trust-runtime/tests/initializer_architecture.rs` | `dynamic_ref_partial_index_does_not_clone_entire_value_ref` | `not_ignored` |
| `DISC_4E33E17E561F4F8DCF87` | `rust_integration_test` | `crates/trust-runtime/tests/initializer_architecture.rs` | `hir_collection_and_import_do_not_drop_member_initializers` | `not_ignored` |
| `DISC_9691E5C8497202DB3D11` | `rust_integration_test` | `crates/trust-runtime/tests/initializer_architecture.rs` | `init_benchmark_cli_and_fixture_are_reproducible` | `not_ignored` |
| `DISC_A845C42691950F68DF66` | `rust_integration_test` | `crates/trust-runtime/tests/initializer_architecture.rs` | `initializer_service_size_caps_hold` | `not_ignored` |
| `DISC_91A12E0E36E5061B6FB4` | `rust_integration_test` | `crates/trust-runtime/tests/initializer_architecture.rs` | `register_ir_decode_uses_inline_operand_storage` | `not_ignored` |
| `DISC_3F08AAEDE73E13EFBEAC` | `rust_integration_test` | `crates/trust-runtime/tests/initializer_architecture.rs` | `runtime_initializer_service_is_the_source_level_funnel` | `not_ignored` |
| `DISC_70215A066C5072737EF7` | `rust_integration_test` | `crates/trust-runtime/tests/initializer_architecture.rs` | `runtime_pou_registration_is_hir_catalog_driven` | `not_ignored` |
| `DISC_B09ED647DF73C9CDBAFD` | `rust_integration_test` | `crates/trust-runtime/tests/initializer_architecture.rs` | `runtime_var_decl_parts_are_structural_not_positional_tuples` | `not_ignored` |
| `DISC_D86241E702A7EA6EDD5D` | `rust_integration_test` | `crates/trust-runtime/tests/initializer_architecture.rs` | `syntax_classifier_helpers_delegate_to_central_api` | `not_ignored` |
| `DISC_B7E1C10B92C087CE4A9D` | `rust_integration_test` | `crates/trust-runtime/tests/initializer_architecture.rs` | `tier1_dynamic_ref_field_borrows_reference_registers` | `not_ignored` |
| `DISC_87C24D157037ADE1FEDD` | `rust_integration_test` | `crates/trust-runtime/tests/initializer_architecture.rs` | `vm_function_block_ref_execution_reads_reference_without_clone` | `not_ignored` |
| `DISC_DA2DB8790F50358D600E` | `rust_integration_test` | `crates/trust-runtime/tests/initializer_architecture.rs` | `vm_local_init_does_not_create_runtime_storage_frames` | `not_ignored` |
| `DISC_3433CBCDCC39F3EEBA52` | `rust_integration_test` | `crates/trust-runtime/tests/instances.rs` | `instance_state_persists` | `not_ignored` |
| `DISC_F56D40C3AAF2688BD524` | `rust_integration_test` | `crates/trust-runtime/tests/io_address.rs` | `bit_and_word_access` | `not_ignored` |
| `DISC_54528961A849760A419E` | `rust_integration_test` | `crates/trust-runtime/tests/io_address.rs` | `parse_addresses` | `not_ignored` |
| `DISC_C71E61C9603198FD7EA8` | `rust_integration_test` | `crates/trust-runtime/tests/io_cycle.rs` | `io_read_write_order` | `not_ignored` |
| `DISC_C105DE057DE08E3E5527` | `rust_integration_test` | `crates/trust-runtime/tests/io_cycle.rs` | `io_snapshot_emitted_after_cycle_output_commit` | `not_ignored` |
| `DISC_E5EF572EAAC5CCAF56CD` | `rust_integration_test` | `crates/trust-runtime/tests/io_driver.rs` | `composed_drivers_are_invoked_in_order` | `not_ignored` |
| `DISC_ED22008C5FE975CA75EA` | `rust_integration_test` | `crates/trust-runtime/tests/io_driver.rs` | `io_driver_reads_and_writes_at_cycle_bounds` | `not_ignored` |
| `DISC_344B0884C064AD660CED` | `rust_integration_test` | `crates/trust-runtime/tests/io_fb_vars.rs` | `fb_instance_io` | `not_ignored` |
| `DISC_72F64CA47C13D2F4C203` | `rust_integration_test` | `crates/trust-runtime/tests/io_fb_vars.rs` | `fb_instance_wildcard_requires_config` | `not_ignored` |
| `DISC_FF081237FFA2A8F3966E` | `rust_integration_test` | `crates/trust-runtime/tests/io_hierarchy.rs` | `iec_6_5_5_2` | `not_ignored` |
| `DISC_2BAB34A13985E33271F6` | `rust_integration_test` | `crates/trust-runtime/tests/io_multidriver_live.rs` | `broker_lifetime_outlasts_test_phases` | `not_ignored` |
| `DISC_FE38938F6FB54A1B26E7` | `rust_integration_test` | `crates/trust-runtime/tests/io_multidriver_live.rs` | `runtime_composes_modbus_and_mqtt_drivers_live` | `not_ignored` |
| `DISC_BD09C186FE58BC9DDBA1` | `rust_integration_test` | `crates/trust-runtime/tests/io_struct_array.rs` | `io_struct_array` | `not_ignored` |
| `DISC_417C1603DFFB9ABF5E89` | `rust_integration_test` | `crates/trust-runtime/tests/io_wildcard.rs` | `wildcard_area_mismatch` | `not_ignored` |
| `DISC_F97EF4F9DB47AEB251C4` | `rust_integration_test` | `crates/trust-runtime/tests/io_wildcard.rs` | `wildcard_memory_area_mismatch` | `not_ignored` |
| `DISC_5B55DFA34BC5F32F856B` | `rust_integration_test` | `crates/trust-runtime/tests/io_wildcard.rs` | `wildcard_not_allowed_in_var_input` | `not_ignored` |
| `DISC_3D33C5B5358A9FD34150` | `rust_integration_test` | `crates/trust-runtime/tests/io_wildcard.rs` | `wildcard_requires_var_config` | `not_ignored` |
| `DISC_7B6C7E58ECC7AA80C109` | `rust_integration_test` | `crates/trust-runtime/tests/memory_frames.rs` | `frame_push_pop` | `not_ignored` |
| `DISC_D37433064542ABB0E52D` | `rust_integration_test` | `crates/trust-runtime/tests/memory_globals.rs` | `read_write_globals` | `not_ignored` |
| `DISC_93F2C7D4A468CF120F03` | `rust_integration_test` | `crates/trust-runtime/tests/memory_lifetime.rs` | `var_temp_resets` | `not_ignored` |
| `DISC_5151047889EFA5EB8711` | `rust_integration_test` | `crates/trust-runtime/tests/modbus_driver.rs` | `modbus_cold_start_before_first_response_returns_bounded_no_data` | `not_ignored` |
| `DISC_711C040BD66EBFF93B84` | `rust_integration_test` | `crates/trust-runtime/tests/modbus_driver.rs` | `modbus_cold_start_no_data_follows_on_error_policy` | `not_ignored` |
| `DISC_22FFF435A5C79CFB4226` | `rust_integration_test` | `crates/trust-runtime/tests/modbus_driver.rs` | `modbus_driver_drop_is_bounded_while_worker_waits_for_first_response` | `not_ignored` |
| `DISC_6E23CF19296BECF5CAB1` | `rust_integration_test` | `crates/trust-runtime/tests/modbus_driver.rs` | `modbus_driver_ignore_policy_degrades_without_transport_error` | `not_ignored` |
| `DISC_99C2FF0BA7EA7EADF15C` | `rust_integration_test` | `crates/trust-runtime/tests/modbus_driver.rs` | `modbus_driver_reads_and_writes_default_register_functions` | `not_ignored` |
| `DISC_788265591A86ECBA1247` | `rust_integration_test` | `crates/trust-runtime/tests/modbus_driver.rs` | `modbus_driver_warn_policy_degrades_without_transport_error` | `not_ignored` |
| `DISC_84211DB0157DDFA2DAD8` | `rust_integration_test` | `crates/trust-runtime/tests/modbus_driver.rs` | `modbus_driver_write_failure_follows_on_error_policy` | `not_ignored` |
| `DISC_A3BEFC6AEFD5123C849D` | `rust_integration_test` | `crates/trust-runtime/tests/modbus_driver.rs` | `modbus_exception_is_not_reported_as_generic_transport` | `not_ignored` |
| `DISC_115B36B27D6276F2E2C0` | `rust_integration_test` | `crates/trust-runtime/tests/modbus_driver.rs` | `modbus_explicit_input_functions_cover_fc01_fc02_fc03` | `not_ignored` |
| `DISC_859C42B742CE618E4C76` | `rust_integration_test` | `crates/trust-runtime/tests/modbus_driver.rs` | `modbus_explicit_output_functions_cover_fc05_fc06_fc15_fc16` | `not_ignored` |
| `DISC_398610728B7609607B57` | `rust_integration_test` | `crates/trust-runtime/tests/modbus_driver.rs` | `modbus_output_handoff_is_bounded_when_scan_outpaces_worker` | `not_ignored` |
| `DISC_F8896C1E5734037D4BF4` | `rust_integration_test` | `crates/trust-runtime/tests/modbus_driver.rs` | `modbus_point_map_reads_scaled_registers_and_coils` | `not_ignored` |
| `DISC_7CCA5C983F3BC05339EC` | `rust_integration_test` | `crates/trust-runtime/tests/modbus_driver.rs` | `modbus_point_map_rejects_invalid_type_and_scaling_config` | `not_ignored` |
| `DISC_456E5DFC92F78865CD94` | `rust_integration_test` | `crates/trust-runtime/tests/modbus_driver.rs` | `modbus_point_map_writes_scaled_registers_and_coils` | `not_ignored` |
| `DISC_8CF1458AD37783E8FCF4` | `rust_integration_test` | `crates/trust-runtime/tests/modbus_driver.rs` | `modbus_raw_register_mode_uses_wire_big_endian_order` | `not_ignored` |
| `DISC_A89C5FA2356D682836D2` | `rust_integration_test` | `crates/trust-runtime/tests/modbus_driver.rs` | `modbus_reconnect_backoff_is_bounded_and_non_spinning` | `not_ignored` |
| `DISC_2CE088F1926B33175423` | `rust_integration_test` | `crates/trust-runtime/tests/modbus_driver.rs` | `modbus_rejects_unknown_function_config` | `not_ignored` |
| `DISC_41FCECEDF690EB121E27` | `rust_integration_test` | `crates/trust-runtime/tests/modbus_driver.rs` | `modbus_safe_state_handoff_succeeds_when_worker_confirms_delivery` | `not_ignored` |
| `DISC_B3EBD5115E6DBD66F946` | `rust_integration_test` | `crates/trust-runtime/tests/modbus_driver.rs` | `modbus_stale_snapshot_is_returned_when_worker_reconnects` | `not_ignored` |
| `DISC_A4A87F179D0CC41E298C` | `rust_integration_test` | `crates/trust-runtime/tests/opcua_client_runtime.rs` | `opcua_client_accepts_vs_code_global_var_names` | `not_ignored` |
| `DISC_C43441A87579223BDF23` | `rust_integration_test` | `crates/trust-runtime/tests/opcua_client_runtime.rs` | `opcua_client_subscription_api_surface_is_available_for_phase3_worker` | `not_ignored` |
| `DISC_6847FE4C45F88000E0B4` | `rust_integration_test` | `crates/trust-runtime/tests/opcua_integration.rs` | `opcua_interop_reads_exposed_scalars_with_reference_client` | `not_ignored` |
| `DISC_B45DE574A2218F802B88` | `rust_integration_test` | `crates/trust-runtime/tests/opcua_integration.rs` | `opcua_security_enforces_user_auth_and_certificate_trust` | `not_ignored` |
| `DISC_6236F0B25551D48929F7` | `rust_integration_test` | `crates/trust-runtime/tests/opcua_integration.rs` | `opcua_server_cold_starts_before_first_runtime_snapshot` | `not_ignored` |
| `DISC_7F7F305082D2D125BF56` | `rust_integration_test` | `crates/trust-runtime/tests/openot_capstone.rs` | `openot_capstone_consumer_process` | `ignored` |
| `DISC_0CEF2FE058BD7204A591` | `rust_integration_test` | `crates/trust-runtime/tests/openot_capstone.rs` | `openot_capstone_fenced_cross_process` | `not_ignored` |
| `DISC_2EEB083ACDA59C7173DC` | `rust_integration_test` | `crates/trust-runtime/tests/openot_capstone.rs` | `openot_capstone_producer_process` | `ignored` |
| `DISC_076A01316BE505A43D6C` | `rust_integration_test` | `crates/trust-runtime/tests/openot_capstone.rs` | `openot_capstone_unfenced_contrast` | `ignored` |
| `DISC_FF6192193C07A9BA50D6` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_st_authoring_api_pou_passes` | `not_ignored` |
| `DISC_8B747AB8C02DFC05D399` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_st_batch_recipe_vectors_are_byte_exact` | `not_ignored` |
| `DISC_1D2324459B69165E352B` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_st_condition_lifecycle_vectors_are_byte_exact` | `not_ignored` |
| `DISC_5394BEFE6396BEBC259C` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_st_regulated_vectors_are_byte_exact` | `not_ignored` |
| `DISC_A5CBEEAB7ADB82512958` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_st_scan_records_burst_pou_passes` | `not_ignored` |
| `DISC_0B1EEEC8B076A747628B` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_st_value_changed_vectors_are_byte_exact` | `not_ignored` |
| `DISC_8DF3012306C164700947` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_st_value_sampling_pou_passes` | `not_ignored` |
| `DISC_DFEE0D11D0FD2F454862` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_telemetry_authoring_batch_recipe_round_trip` | `not_ignored` |
| `DISC_85D8F431C6D7171A28E3` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_telemetry_authoring_condition_ack_without_activation_fails_closed` | `not_ignored` |
| `DISC_D90515BA21C5932FF6BB` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_telemetry_authoring_condition_comment_oversize_fails_closed` | `not_ignored` |
| `DISC_A0E33262D0E1DFF6E869` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_telemetry_authoring_condition_confirm_without_activation_fails_closed` | `not_ignored` |
| `DISC_D8F0D576C2AC03F32D41` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_telemetry_authoring_condition_lifecycle_round_trip` | `not_ignored` |
| `DISC_AA0CB942585BF1365272` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_telemetry_authoring_condition_oos_while_inactive_emits` | `not_ignored` |
| `DISC_6758712467FC11514039` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_telemetry_authoring_condition_priority_while_inactive_emits` | `not_ignored` |
| `DISC_40094F22ECF98AF9682E` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_telemetry_authoring_condition_reset_without_activation_fails_closed` | `not_ignored` |
| `DISC_ECEF9A7E30A36C832CB6` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_telemetry_authoring_condition_shelve_without_activation_fails_closed` | `not_ignored` |
| `DISC_A40C00543402993CAAD4` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_telemetry_authoring_condition_suppress_while_inactive_emits` | `not_ignored` |
| `DISC_D2D46354A1D5DBD5EC2B` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_telemetry_authoring_condition_unshelve_without_activation_fails_closed` | `not_ignored` |
| `DISC_8B929DF0EEF9547A35D0` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_telemetry_authoring_condition_unsuppress_and_in_service_while_inactive_emit` | `not_ignored` |
| `DISC_949D8A93709B5D0F05D4` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_telemetry_authoring_esignature_cross_scan_attests_prior_event` | `not_ignored` |
| `DISC_FABFE69E280524A5BCAC` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_telemetry_authoring_esignature_never_emitted_target_fails_closed` | `not_ignored` |
| `DISC_994CA7224D5CAB9B96B2` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_telemetry_authoring_esignature_same_scan_is_phased_last` | `not_ignored` |
| `DISC_7693198AD83E8FA17AB3` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_telemetry_authoring_fixed_width_values_round_trip` | `not_ignored` |
| `DISC_DA405AB2E07EDD84ACE2` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_telemetry_authoring_message_args_and_condition_correlation_round_trip` | `not_ignored` |
| `DISC_49D744C60E280A8991A9` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_telemetry_authoring_operator_regulated_round_trip` | `not_ignored` |
| `DISC_FDF33FA61E5B3AEAD3D8` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_telemetry_authoring_parameter_change_round_trip` | `not_ignored` |
| `DISC_5B43AF2B1181A8401F05` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_telemetry_authoring_sampling_policy_round_trips_definition` | `not_ignored` |
| `DISC_D3D13B8BFEA158194925` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_telemetry_authoring_showcase_renders_typed_audit_log` | `not_ignored` |
| `DISC_2538F1244D6A2B3BC786` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_telemetry_drains_multi_program_source_high_water_to_one_ring` | `not_ignored` |
| `DISC_A40E325C95451F9B5D89` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_telemetry_esignature_attestation_state_resets_on_epoch_transition` | `not_ignored` |
| `DISC_F9301797A6BE1A82FEDB` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_telemetry_parameter_change_oversize_drop_keeps_baseline` | `not_ignored` |
| `DISC_373D303A11B90D676717` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_telemetry_publish_failure_is_fail_closed` | `not_ignored` |
| `DISC_9BBDE7C18FAF59882F0F` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_telemetry_publishes_crc_valid_heartbeats` | `not_ignored` |
| `DISC_2D97E68146D7F62004CC` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_telemetry_publishes_multi_program_authoring_sources_to_one_ring` | `not_ignored` |
| `DISC_3C70AFA1EDEC1DB96BE1` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_telemetry_publishes_real_st_producer_records` | `not_ignored` |
| `DISC_14815B681BC20DBFF8B2` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_telemetry_publishes_st_transition_burst` | `not_ignored` |
| `DISC_F2F601D4451641BE75CE` | `rust_integration_test` | `crates/trust-runtime/tests/openot_telemetry.rs` | `openot_telemetry_st_fb_multi_record_scan_is_fail_closed` | `not_ignored` |
| `DISC_C237C360912937D935CF` | `rust_integration_test` | `crates/trust-runtime/tests/oscat_oop_examples.rs` | `oscat_aggregate_manifest_uses_toml_safe_dependency_paths` | `not_ignored` |
| `DISC_F2E7DDFFFDCE1AFA0C54` | `rust_integration_test` | `crates/trust-runtime/tests/oscat_oop_examples.rs` | `oscat_airport_baggage_namespace_aggregate_trigger_passes` | `not_ignored` |
| `DISC_EA2023180F78FA392438` | `rust_integration_test` | `crates/trust-runtime/tests/oscat_oop_examples.rs` | `oscat_example_child_lines_include_pid_project_and_elapsed_context` | `not_ignored` |
| `DISC_478B61E2A9C81B1BD0F3` | `rust_integration_test` | `crates/trust-runtime/tests/oscat_oop_examples.rs` | `oscat_example_gate_reports_active_project_before_running_child` | `not_ignored` |
| `DISC_B4EA90EBBD57B1BA842B` | `rust_integration_test` | `crates/trust-runtime/tests/oscat_oop_examples.rs` | `oscat_examples_use_grouped_oop_non_oop_layout` | `not_ignored` |
| `DISC_B0E363553D57310C9D63` | `rust_integration_test` | `crates/trust-runtime/tests/oscat_oop_examples.rs` | `oscat_oop_example_st_unit_tests_pass` | `ignored` |
| `DISC_E1C519C3BCCEC897820A` | `rust_integration_test` | `crates/trust-runtime/tests/oscat_oop_examples.rs` | `oscat_oop_examples_contain_claimed_pattern_structures` | `not_ignored` |
| `DISC_BDEAFF431FC2B9A44DFD` | `rust_integration_test` | `crates/trust-runtime/tests/phase10_performance.rs` | `phase10_ads_opcua_publish_clone_partial_baseline` | `ignored` |
| `DISC_8A83ABE26CDE3A14E938` | `rust_integration_test` | `crates/trust-runtime/tests/phase10_performance.rs` | `phase10_debug_snapshot_overhead_baseline` | `ignored` |
| `DISC_9DA8F1E2D659F4E0F5E7` | `rust_integration_test` | `crates/trust-runtime/tests/phase10_performance.rs` | `phase10_retain_fsync_impact_baseline` | `ignored` |
| `DISC_7DBEF68950F9AC3B5F29` | `rust_integration_test` | `crates/trust-runtime/tests/phase11_seam_contract.rs` | `ref_return_name_is_rejected_before_runtime_lowering` | `not_ignored` |
| `DISC_106B06B9E9AC0F00A1CD` | `rust_integration_test` | `crates/trust-runtime/tests/plant_demo_io.rs` | `plant_demo_configuration_binds_io_and_tasks` | `not_ignored` |
| `DISC_04E3AE8E579EE7353332` | `rust_integration_test` | `crates/trust-runtime/tests/platform_std.rs` | `monotonic_time` | `not_ignored` |
| `DISC_AC89BA6FAA7BC42ACCBC` | `rust_integration_test` | `crates/trust-runtime/tests/platform_std.rs` | `scheduler_ignores_wall_time_when_monotonic_clock_is_fixed` | `not_ignored` |
| `DISC_27187199BD3E588FC6BB` | `rust_integration_test` | `crates/trust-runtime/tests/platform_std.rs` | `sleep_not_in_tests` | `not_ignored` |
| `DISC_3D9BC624619267817197` | `rust_integration_test` | `crates/trust-runtime/tests/platform_std.rs` | `time_builtin_uses_runtime_clock` | `not_ignored` |
| `DISC_84FCA7DE99160D25038D` | `rust_integration_test` | `crates/trust-runtime/tests/plcopen_codesys_import_runtime.rs` | `import_codesys_global_vars_and_project_structure_into_application_folder` | `not_ignored` |
| `DISC_11AA6727B8481F0C7CFC` | `rust_integration_test` | `crates/trust-runtime/tests/plcopen_codesys_import_runtime.rs` | `import_codesys_method_objects_into_function_block_source` | `not_ignored` |
| `DISC_D161A8256FACEA1961D8` | `rust_integration_test` | `crates/trust-runtime/tests/plcopen_codesys_import_runtime.rs` | `import_codesys_qualified_globals_into_namespaced_gvl_without_var_external_injection_and_function_result_assignment` | `not_ignored` |
| `DISC_59F1D40C7F63F81C2A2A` | `rust_integration_test` | `crates/trust-runtime/tests/plcopen_codesys_import_runtime.rs` | `import_injects_var_external_for_qualified_globals_and_function_result_assignment` | `not_ignored` |
| `DISC_FB9F285A2AD8FA0A3FAB` | `rust_integration_test` | `crates/trust-runtime/tests/plcopen_codesys_import_runtime.rs` | `import_synthesizes_codesys_body_only_and_empty_plaintext_pous` | `not_ignored` |
| `DISC_04B923076AA8CF91500E` | `rust_integration_test` | `crates/trust-runtime/tests/plcopen_codesys_import_runtime.rs` | `import_tc6_multiple_bodies_and_extended_interface_sections` | `not_ignored` |
| `DISC_BF9E9F0DB4780106B366` | `rust_integration_test` | `crates/trust-runtime/tests/plcopen_command.rs` | `plcopen_export_and_import_round_trip_via_cli` | `not_ignored` |
| `DISC_143454B2FC319D8BA62A` | `rust_integration_test` | `crates/trust-runtime/tests/plcopen_command.rs` | `plcopen_export_import_json_reports_include_compatibility_diagnostics` | `not_ignored` |
| `DISC_D01FB47BC5C8523C52F9` | `rust_integration_test` | `crates/trust-runtime/tests/plcopen_command.rs` | `plcopen_export_siemens_target_generates_scl_bundle` | `not_ignored` |
| `DISC_9D2B8EFC26FB91B93EF1` | `rust_integration_test` | `crates/trust-runtime/tests/plcopen_command.rs` | `plcopen_export_target_generates_adapter_report_and_default_target_path` | `not_ignored` |
| `DISC_6927F7CB479F1A7195AA` | `rust_integration_test` | `crates/trust-runtime/tests/plcopen_command.rs` | `plcopen_import_fails_for_missing_input` | `not_ignored` |
| `DISC_EC08DF4942B2E64E8495` | `rust_integration_test` | `crates/trust-runtime/tests/plcopen_command.rs` | `plcopen_import_json_detects_openplc_ecosystem_and_shims` | `not_ignored` |
| `DISC_B114AF6BF1B27DD82EB5` | `rust_integration_test` | `crates/trust-runtime/tests/plcopen_command.rs` | `plcopen_import_json_reports_applied_vendor_library_shims` | `not_ignored` |
| `DISC_9F9C54D5425826CC8238` | `rust_integration_test` | `crates/trust-runtime/tests/plcopen_command.rs` | `plcopen_openplc_fixture_in_st_complete_bundle_import_export_smoke` | `not_ignored` |
| `DISC_28B62203DD44F1A78720` | `rust_integration_test` | `crates/trust-runtime/tests/plcopen_command.rs` | `plcopen_profile_json_emits_contract` | `not_ignored` |
| `DISC_301EDA33666512C82363` | `rust_integration_test` | `crates/trust-runtime/tests/plcopen_migration.rs` | `migration_import_codesys_fixture_reports_coverage_and_loss` | `not_ignored` |
| `DISC_AAD54BEEA40B87DE33E6` | `rust_integration_test` | `crates/trust-runtime/tests/plcopen_migration.rs` | `migration_import_openplc_fixture_reports_vendor_coverage` | `not_ignored` |
| `DISC_C25C8C90385A10DF98DA` | `rust_integration_test` | `crates/trust-runtime/tests/plcopen_migration.rs` | `migration_import_rockwell_fixture_reports_vendor_coverage` | `not_ignored` |
| `DISC_25F483302A5EF4F26FB0` | `rust_integration_test` | `crates/trust-runtime/tests/plcopen_migration.rs` | `migration_import_schneider_fixture_detects_vendor_precedence` | `not_ignored` |
| `DISC_91CEB1B564AAB5846E55` | `rust_integration_test` | `crates/trust-runtime/tests/plcopen_migration.rs` | `migration_import_siemens_fixture_reports_vendor_coverage` | `not_ignored` |
| `DISC_ADAC5790426C0B7CA267` | `rust_integration_test` | `crates/trust-runtime/tests/plcopen_migration.rs` | `migration_import_twincat_fixture_handles_vendor_variants` | `not_ignored` |
| `DISC_2C78FE7C3C3E99984F0A` | `rust_integration_test` | `crates/trust-runtime/tests/plcopen_migration.rs` | `migration_semantic_loss_scoring_reflects_import_completeness` | `not_ignored` |
| `DISC_EF4F4CDF033D80C69B87` | `rust_integration_test` | `crates/trust-runtime/tests/plcopen_motion_oop_library.rs` | `plcopen_motion_oop_single_axis_st_unit_tests_pass` | `not_ignored` |
| `DISC_8DEF54CCCE9994233D00` | `rust_integration_test` | `crates/trust-runtime/tests/plcopen_st_complete_parity.rs` | `plcopen_codesys_st_complete_large_parity` | `not_ignored` |
| `DISC_31ABDD3E296B44F4976F` | `rust_integration_test` | `crates/trust-runtime/tests/plcopen_st_complete_parity.rs` | `plcopen_codesys_st_complete_medium_parity` | `not_ignored` |
| `DISC_D9822A35CDB64036BFA5` | `rust_integration_test` | `crates/trust-runtime/tests/plcopen_st_complete_parity.rs` | `plcopen_codesys_st_complete_small_parity` | `not_ignored` |
| `DISC_E56695229775E9941BDB` | `rust_integration_test` | `crates/trust-runtime/tests/pou_class.rs` | `class_instances` | `not_ignored` |
| `DISC_ED51C13EAA7B01ED516A` | `rust_integration_test` | `crates/trust-runtime/tests/pou_interface.rs` | `interface_assignment_works_across_files_with_properties` | `not_ignored` |
| `DISC_414C8E39446859CB438E` | `rust_integration_test` | `crates/trust-runtime/tests/pou_interface.rs` | `interface_conformance` | `not_ignored` |
| `DISC_5ADCF87560E7530EFD10` | `rust_integration_test` | `crates/trust-runtime/tests/pou_interface.rs` | `method_can_return_owned_function_block_as_interface` | `not_ignored` |
| `DISC_3AE90ECEF2EF651667A9` | `rust_integration_test` | `crates/trust-runtime/tests/pou_methods.rs` | `method_calls` | `not_ignored` |
| `DISC_EA12445944521D23E88B` | `rust_integration_test` | `crates/trust-runtime/tests/pou_oop.rs` | `polymorphism` | `not_ignored` |
| `DISC_39BF472F3F881FC99B0C` | `rust_integration_test` | `crates/trust-runtime/tests/pou_program.rs` | `program_cycle` | `not_ignored` |
| `DISC_1AD2B71A88B0C9EC311E` | `rust_integration_test` | `crates/trust-runtime/tests/process_image.rs` | `invalid_metadata_task_does_not_partially_resize_process_image` | `not_ignored` |
| `DISC_FAE868571332DB63BB86` | `rust_integration_test` | `crates/trust-runtime/tests/process_image.rs` | `metadata_size_above_process_image_cap_is_rejected` | `not_ignored` |
| `DISC_C9C529835FD206ECA81B` | `rust_integration_test` | `crates/trust-runtime/tests/process_image.rs` | `sized_from_metadata` | `not_ignored` |
| `DISC_67E6BEE860EC6D210FB2` | `rust_integration_test` | `crates/trust-runtime/tests/process_image.rs` | `source_binding_above_process_image_cap_is_rejected` | `not_ignored` |
| `DISC_89E7DE8C62E066814C73` | `rust_integration_test` | `crates/trust-runtime/tests/prometheus_integration.rs` | `prometheus_endpoint_exposes_runtime_and_historian_metrics` | `not_ignored` |
| `DISC_3D7E211119AC55A342FA` | `rust_integration_test` | `crates/trust-runtime/tests/prometheus_integration.rs` | `prometheus_endpoint_requires_auth_when_token_mode_enabled` | `not_ignored` |
| `DISC_B269AFAF1D0DDC79A33B` | `rust_integration_test` | `crates/trust-runtime/tests/protocol_envelope.rs` | `watch_snapshot_uses_per_entry_error_for_unknown_paths` | `not_ignored` |
| `DISC_EE994842B520CF20330A` | `rust_integration_test` | `crates/trust-runtime/tests/real_world.rs` | `samples` | `not_ignored` |
| `DISC_89CD61302AF3809955E3` | `rust_integration_test` | `crates/trust-runtime/tests/realtime_t0_integration.rs` | `realtime_t0_determinism_holds_under_cloud_budget_pressure` | `not_ignored` |
| `DISC_D0737A7878036279A9A4` | `rust_integration_test` | `crates/trust-runtime/tests/realtime_t0_integration.rs` | `realtime_t0_multi_process_shm_exchange_succeeds` | `not_ignored` |
| `DISC_6627DC479523620038C1` | `rust_integration_test` | `crates/trust-runtime/tests/realtime_t0_integration.rs` | `realtime_t0_route_does_not_fallback_to_mesh_ip_path` | `not_ignored` |
| `DISC_42AE8211EF71BC9B2C1E` | `rust_integration_test` | `crates/trust-runtime/tests/registry_command.rs` | `registry_private_access_control_requires_token` | `not_ignored` |
| `DISC_64A4991F06885FE8F756` | `rust_integration_test` | `crates/trust-runtime/tests/registry_command.rs` | `registry_profile_json_matches_contract` | `not_ignored` |
| `DISC_C65C7279DC207BDD19A0` | `rust_integration_test` | `crates/trust-runtime/tests/registry_command.rs` | `registry_publish_download_verify_round_trip` | `not_ignored` |
| `DISC_BEB3FCC15BB5DB682280` | `rust_integration_test` | `crates/trust-runtime/tests/resources.rs` | `global_sync` | `not_ignored` |
| `DISC_D78A8DAFA8D77EE7FB2F` | `rust_integration_test` | `crates/trust-runtime/tests/resources.rs` | `multiple_resources` | `not_ignored` |
| `DISC_75EFF22ADED7B9F53606` | `rust_integration_test` | `crates/trust-runtime/tests/retain_integrity.rs` | `retain_bounded_string_is_canonicalized_on_load` | `not_ignored` |
| `DISC_FC0227884281CC4F9C2C` | `rust_integration_test` | `crates/trust-runtime/tests/retain_integrity.rs` | `retain_orphan_global_emits_runtime_event` | `not_ignored` |
| `DISC_86D6D1369E5C7793C53F` | `rust_integration_test` | `crates/trust-runtime/tests/retain_integrity.rs` | `retain_scalar_widening_migrates_with_runtime_event` | `not_ignored` |
| `DISC_181CD2176E3A15433E44` | `rust_integration_test` | `crates/trust-runtime/tests/retain_integrity.rs` | `retain_snapshot_migration_failure_does_not_partially_apply_earlier_values` | `not_ignored` |
| `DISC_4FB5821EF62B7655873F` | `rust_integration_test` | `crates/trust-runtime/tests/retain_integrity.rs` | `retain_store_loads_legacy_v1_snapshot` | `not_ignored` |
| `DISC_D57F0AA703E76450F141` | `rust_integration_test` | `crates/trust-runtime/tests/retain_integrity.rs` | `retain_store_rejects_payload_mutation` | `not_ignored` |
| `DISC_6536BD14A5FAEA351CCF` | `rust_integration_test` | `crates/trust-runtime/tests/retain_integrity.rs` | `retain_store_rejects_trailing_garbage` | `not_ignored` |
| `DISC_32E18E31451D4BF469C6` | `rust_integration_test` | `crates/trust-runtime/tests/retain_integrity.rs` | `retain_store_reports_real_path_failures_without_silent_defaults` | `not_ignored` |
| `DISC_5CE1CFAF7B148E819A77` | `rust_integration_test` | `crates/trust-runtime/tests/retain_integrity.rs` | `retain_struct_added_field_uses_declared_default_with_migration_event` | `not_ignored` |
| `DISC_27D4369EB8A25ADDB6A8` | `rust_integration_test` | `crates/trust-runtime/tests/retain_integrity.rs` | `retain_struct_removed_field_drops_with_migration_event` | `not_ignored` |
| `DISC_9CFF33D681B6CEEA6ABA` | `rust_integration_test` | `crates/trust-runtime/tests/retain_store.rs` | `retain_store_missing_file_returns_default` | `not_ignored` |
| `DISC_919CC2ACD5005149FFE0` | `rust_integration_test` | `crates/trust-runtime/tests/retain_store.rs` | `retain_store_roundtrip` | `not_ignored` |
| `DISC_8630EBEA6FFFB7EEFE8A` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_cloud_architecture.rs` | `realtime_t0_hot_path_keeps_mesh_apis_and_key_parsing_out_of_band` | `not_ignored` |
| `DISC_36DB8F61C1B0BF3590BA` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_cloud_architecture.rs` | `runtime_cloud_core_modules_do_not_import_transport_layers` | `not_ignored` |
| `DISC_E9D61CF6548E1BEC9D4A` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_cloud_architecture.rs` | `runtime_cloud_dispatch_route_uses_contract_preflight_before_dispatch_mapping` | `not_ignored` |
| `DISC_C7EFBF9043236155380C` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_cloud_architecture.rs` | `runtime_cloud_proxy_routes_are_policy_first_adapters` | `not_ignored` |
| `DISC_FA958AC0CC2925AC422E` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_cloud_architecture.rs` | `runtime_cloud_state_adapters_delegate_domain_state_to_policy_modules` | `not_ignored` |
| `DISC_BAD54416BD0F91BC3776` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_core_behavior_lock.rs` | `cycle_boundary_latches_inputs_once_and_commits_outputs_after_ready_programs` | `not_ignored` |
| `DISC_08E82F9BB3CCCB23ECD6` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_core_behavior_lock.rs` | `cycle_boundary_reads_every_driver_before_any_driver_writes_outputs` | `not_ignored` |
| `DISC_268F47FD499F57BFF436` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_core_behavior_lock.rs` | `stable_bytecode_fixture_loads_on_runtime_core_path` | `not_ignored` |
| `DISC_116A738C606DD35D5B09` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_core_behavior_lock.rs` | `vm_fixture_execution_image_status_and_values_are_stable` | `not_ignored` |
| `DISC_04CF6B406AB66EB7B40B` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_core_behavior_lock.rs` | `watchdog_and_fault_policy_decisions_are_stable` | `not_ignored` |
| `DISC_3EFD5D5E736CB0C47DD6` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_core_behavior_lock.rs` | `watchdog_timeout_preserves_fault_snapshot_and_safe_state_contract` | `not_ignored` |
| `DISC_C1673FF1CD8755876585` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_events.rs` | `runtime_event_fault_emitted` | `not_ignored` |
| `DISC_5408211286BED66DF67B` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_events.rs` | `runtime_event_overrun_emitted` | `not_ignored` |
| `DISC_44FD45F9CC3B04A52735` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_events.rs` | `runtime_events_include_cycle_and_task` | `not_ignored` |
| `DISC_EE4014B513F4E29CDCC3` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_reliability.rs` | `e2e_retain_roundtrip_restart` | `not_ignored` |
| `DISC_B02D0E3CFEC34E755FC0` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_reliability.rs` | `e2e_startup_io_restart` | `not_ignored` |
| `DISC_B0616A747435CAE04D2A` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_reliability.rs` | `retain_power_loss_does_not_persist_unsaved` | `not_ignored` |
| `DISC_C101BBAEC06311987C74` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_reliability.rs` | `watchdog_faults_resource_on_overrun` | `not_ignored` |
| `DISC_2CC9B67E44CB8284B149` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_restart.rs` | `in_process_restart_preserves_monotonic_time` | `not_ignored` |
| `DISC_8C34C3928AD85D534B78` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_restart.rs` | `restart_with_retain_store_persists_values` | `not_ignored` |
| `DISC_3A32AF1C7CD243DB55AC` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_restart.rs` | `warm_restart_does_not_report_pre_restart_time_as_new_overrun` | `not_ignored` |
| `DISC_6F92DA55FCF24372A33A` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_safety_fail_closed.rs` | `deliberate_stop_and_fault_safe_state_paths_are_explicit` | `not_ignored` |
| `DISC_CB7C88F571D35D725730` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_safety_fail_closed.rs` | `modbus_fault_policy_faults_resource_state` | `not_ignored` |
| `DISC_027D1EBF00A35926B528` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_safety_fail_closed.rs` | `modbus_warn_and_ignore_policy_do_not_fault_resource_state` | `not_ignored` |
| `DISC_C946B9FD799F9E4578A3` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_safety_fail_closed.rs` | `panic_does_not_follow_ordinary_restart_policy` | `not_ignored` |
| `DISC_D4793CC01E1740F97415` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_safety_fail_closed.rs` | `panic_in_shared_resource_faults_without_poison_detection` | `not_ignored` |
| `DISC_EBC7D491061F23BDD8B6` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_safety_fail_closed.rs` | `persistent_fault_restart_policy_escalates_to_visible_fault` | `not_ignored` |
| `DISC_EFB8B3C332B85B1B4615` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_safety_fail_closed.rs` | `requested_stop_applies_safe_outputs_before_thread_exits` | `not_ignored` |
| `DISC_CAF0750D558B8114BEC6` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_safety_fail_closed.rs` | `requested_stop_retain_save_failure_is_visible` | `not_ignored` |
| `DISC_B25F48AA676D50A29BB2` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_safety_fail_closed.rs` | `retain_save_failure_prevents_output_commit_when_due` | `not_ignored` |
| `DISC_2D0B462DA31DE705F6EC` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_safety_fail_closed.rs` | `safe_state_write_failure_is_reported_without_losing_root_fault` | `not_ignored` |
| `DISC_8BAF8461F95A94A773A6` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_safety_fail_closed.rs` | `watchdog_deadline_breach_before_commit_prevents_output_write` | `not_ignored` |
| `DISC_8B901DEF0D4408B99942` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_safety_fail_closed.rs` | `watchdog_restart_action_escalates_to_visible_fault` | `not_ignored` |
| `DISC_5619D086FCBB5DB1B5D3` | `rust_integration_test` | `crates/trust-runtime/tests/scheduler_harness.rs` | `deterministic_clock` | `not_ignored` |
| `DISC_F69FD326CB0CD7B232FE` | `rust_integration_test` | `crates/trust-runtime/tests/scheduler_resource.rs` | `resource_runs_in_thread` | `not_ignored` |
| `DISC_835007939944941C657C` | `rust_integration_test` | `crates/trust-runtime/tests/scheduler_state.rs` | `fault_stops_resource` | `not_ignored` |
| `DISC_FEA845C2B43DA1DC7766` | `rust_integration_test` | `crates/trust-runtime/tests/setup_command.rs` | `setup_browser_local_rejects_non_loopback_bind` | `not_ignored` |
| `DISC_5AE666AEEC1AE4669071` | `rust_integration_test` | `crates/trust-runtime/tests/setup_command.rs` | `setup_browser_remote_dry_run_shows_token_requirements` | `not_ignored` |
| `DISC_227E5D33FA9515195C47` | `rust_integration_test` | `crates/trust-runtime/tests/setup_command.rs` | `setup_browser_remote_rejects_loopback_bind` | `not_ignored` |
| `DISC_FF07D8745DB8ACEAF9AB` | `rust_integration_test` | `crates/trust-runtime/tests/setup_command.rs` | `setup_cancel_mode_exits_successfully` | `not_ignored` |
| `DISC_E69573B752C8306A05AE` | `rust_integration_test` | `crates/trust-runtime/tests/setup_command.rs` | `setup_cli_mode_writes_artifacts_and_next_steps` | `not_ignored` |
| `DISC_97129C80B6AD9BF1E0C3` | `rust_integration_test` | `crates/trust-runtime/tests/signal_smoke.rs` | `sigint_requests_bounded_graceful_stop_in_child_runtime` | `not_ignored` |
| `DISC_392A863355C41402FB4C` | `rust_integration_test` | `crates/trust-runtime/tests/signal_smoke.rs` | `sigterm_requests_bounded_graceful_stop_in_child_runtime` | `not_ignored` |
| `DISC_F6F3665F8B379102EFEE` | `rust_integration_test` | `crates/trust-runtime/tests/simulation_workflow.rs` | `accelerated_clock_keeps_watchdog_semantics` | `not_ignored` |
| `DISC_2F42A392CE80CD36FBB2` | `rust_integration_test` | `crates/trust-runtime/tests/simulation_workflow.rs` | `coupling_applies_threshold_with_delay` | `not_ignored` |
| `DISC_5079C513733275372753` | `rust_integration_test` | `crates/trust-runtime/tests/simulation_workflow.rs` | `deterministic_trace_with_same_simulation_config` | `not_ignored` |
| `DISC_7CB55CD7268E209C7A59` | `rust_integration_test` | `crates/trust-runtime/tests/simulation_workflow.rs` | `physics_feedback_target_conflicts_are_rejected` | `not_ignored` |
| `DISC_2A57C3244B2F34DF39D6` | `rust_integration_test` | `crates/trust-runtime/tests/simulation_workflow.rs` | `physics_revolute_joint_queues_encoder_feedback_through_io_boundary` | `not_ignored` |
| `DISC_F045598C3E23F947792D` | `rust_integration_test` | `crates/trust-runtime/tests/simulation_workflow.rs` | `physics_revolute_trace_is_deterministic_for_same_seed` | `not_ignored` |
| `DISC_2BE10D5D71EBD6034F0B` | `rust_integration_test` | `crates/trust-runtime/tests/simulation_workflow.rs` | `scripted_fault_disturbance_faults_runtime` | `not_ignored` |
| `DISC_D62F2B0357AA33DDC060` | `rust_integration_test` | `crates/trust-runtime/tests/simulation_workflow.rs` | `simulation_toml_model_parses_physics_joints` | `not_ignored` |
| `DISC_8887FE3BA0A4120A7E2F` | `rust_integration_test` | `crates/trust-runtime/tests/simulation_workflow.rs` | `simulation_toml_model_parses_rules_and_disturbances` | `not_ignored` |
| `DISC_287040A6A4B257F5AFE7` | `rust_integration_test` | `crates/trust-runtime/tests/sizeof_semantics.rs` | `sizeof_bare_name_prefers_variable_over_top_level_type_name` | `not_ignored` |
| `DISC_D4CD4DB4F0ABE4F9AC85` | `rust_integration_test` | `crates/trust-runtime/tests/sizeof_semantics.rs` | `sizeof_complete_program_fixture_supports_variable_and_type_operands` | `not_ignored` |
| `DISC_161D069E1577EEC2900F` | `rust_integration_test` | `crates/trust-runtime/tests/sizeof_semantics.rs` | `sizeof_pointer_and_reference_operands_use_platform_pointer_size` | `not_ignored` |
| `DISC_EF21DF378857C06E6050` | `rust_integration_test` | `crates/trust-runtime/tests/sizeof_semantics.rs` | `sizeof_pointer_contract_matches_platform_word_size` | `not_ignored` |
| `DISC_1F217C2C37FCE653A5AA` | `rust_integration_test` | `crates/trust-runtime/tests/sizeof_semantics.rs` | `sizeof_pointer_operands_const_fold_in_array_bounds` | `not_ignored` |
| `DISC_0CBBAFF5CE1809A27D80` | `rust_integration_test` | `crates/trust-runtime/tests/sizeof_semantics.rs` | `sizeof_rejects_call_operands_during_build` | `not_ignored` |
| `DISC_695AC41055CF4A56A894` | `rust_integration_test` | `crates/trust-runtime/tests/sizeof_semantics.rs` | `sizeof_rejects_function_block_instance_operands_during_build` | `not_ignored` |
| `DISC_68A1E47807FECFE21598` | `rust_integration_test` | `crates/trust-runtime/tests/sizeof_semantics.rs` | `sizeof_rejects_unknown_identifiers_during_build` | `not_ignored` |
| `DISC_C572BF3D31314A3E0739` | `rust_integration_test` | `crates/trust-runtime/tests/sizeof_semantics.rs` | `sizeof_variable_and_type_operands_build_and_run` | `not_ignored` |
| `DISC_31545A2E57BCE258D0ED` | `rust_integration_test` | `crates/trust-runtime/tests/sizeof_semantics.rs` | `sizeof_variable_shadows_type_name_and_qualified_type_remains_available` | `not_ignored` |
| `DISC_F36EF5E7612D4F9FC6AA` | `rust_integration_test` | `crates/trust-runtime/tests/sizeof_semantics.rs` | `sizeof_works_in_array_bounds_for_variable_operands` | `not_ignored` |
| `DISC_253D75705176F76907C4` | `rust_integration_test` | `crates/trust-runtime/tests/st_test_cli_command.rs` | `build_accepts_recent_language_regression_cases` | `not_ignored` |
| `DISC_7D2DD5D31B5A5FC44DBC` | `rust_integration_test` | `crates/trust-runtime/tests/st_test_cli_command.rs` | `filter_zero_message_is_clear_in_human_output` | `not_ignored` |
| `DISC_613C825C95E9BD7589DA` | `rust_integration_test` | `crates/trust-runtime/tests/st_test_cli_command.rs` | `json_output_includes_duration_fields` | `not_ignored` |
| `DISC_0C37F6CE3A863AA20738` | `rust_integration_test` | `crates/trust-runtime/tests/st_test_cli_command.rs` | `list_flag_lists_tutorial_10_tests_without_executing` | `not_ignored` |
| `DISC_97B2FF40560E0E22B910` | `rust_integration_test` | `crates/trust-runtime/tests/st_test_cli_command.rs` | `test_program_runs_when_configuration_is_present` | `not_ignored` |
| `DISC_660F8403764F3D351C9F` | `rust_integration_test` | `crates/trust-runtime/tests/st_test_cli_command.rs` | `timeout_budget_does_not_count_project_recompilation_per_case` | `not_ignored` |
| `DISC_0BD76C389DCA0387316F` | `rust_integration_test` | `crates/trust-runtime/tests/st_test_cli_command.rs` | `timeout_flag_reports_error_for_infinite_loop_test` | `not_ignored` |
| `DISC_78FE3302CE5410E49FC9` | `rust_integration_test` | `crates/trust-runtime/tests/st_test_cli_command.rs` | `trust_runtime_test_alias_forwards_to_trust_dev` | `not_ignored` |
| `DISC_0E80FAE410A3F0F71E40` | `rust_integration_test` | `crates/trust-runtime/tests/st_test_runtime_determinism.rs` | `assertion_result_is_deterministic_with_manual_clock` | `not_ignored` |
| `DISC_52B1B39F56D1300B1907` | `rust_integration_test` | `crates/trust-runtime/tests/stdlib_assertions.rs` | `assertion_comparison_functions_coerce_numeric_types` | `not_ignored` |
| `DISC_E81C8F54843E0E21C2F2` | `rust_integration_test` | `crates/trust-runtime/tests/stdlib_assertions.rs` | `assertion_failure_messages_use_user_facing_value_strings` | `not_ignored` |
| `DISC_0D3C1E2B84926E0438C7` | `rust_integration_test` | `crates/trust-runtime/tests/stdlib_assertions.rs` | `assertion_functions_fail_with_assertion_error` | `not_ignored` |
| `DISC_DE11FCAE13C370F4D857` | `rust_integration_test` | `crates/trust-runtime/tests/stdlib_assertions.rs` | `assertion_functions_pass_when_conditions_hold` | `not_ignored` |
| `DISC_CDD39BC7EF926214C7A2` | `rust_integration_test` | `crates/trust-runtime/tests/stdlib_bit_full.rs` | `bit_full` | `not_ignored` |
| `DISC_11144C19050BB255758E` | `rust_integration_test` | `crates/trust-runtime/tests/stdlib_conv.rs` | `conversion_functions` | `not_ignored` |
| `DISC_2243B8C73C3BFACC97DB` | `rust_integration_test` | `crates/trust-runtime/tests/stdlib_conv_full.rs` | `conversion_full` | `not_ignored` |
| `DISC_DD3343407B4FA9D0336A` | `rust_integration_test` | `crates/trust-runtime/tests/stdlib_conv_full.rs` | `string_to_real_rejects_non_finite_text` | `not_ignored` |
| `DISC_1608E778CF662F4E2257` | `rust_integration_test` | `crates/trust-runtime/tests/stdlib_enum_validate.rs` | `enum_comparisons` | `not_ignored` |
| `DISC_CF850EDA342B7B7DDEAD` | `rust_integration_test` | `crates/trust-runtime/tests/stdlib_enum_validate.rs` | `is_valid_bcd_bit_strings` | `not_ignored` |
| `DISC_33D9A3283BBB67B67BF1` | `rust_integration_test` | `crates/trust-runtime/tests/stdlib_enum_validate.rs` | `is_valid_real_values` | `not_ignored` |
| `DISC_797A3E52A2E7B2E9149F` | `rust_integration_test` | `crates/trust-runtime/tests/stdlib_numeric.rs` | `numeric_functions` | `not_ignored` |
| `DISC_46F60767D29EA212389C` | `rust_integration_test` | `crates/trust-runtime/tests/stdlib_numeric_full.rs` | `numeric_full` | `not_ignored` |
| `DISC_1EB42F045CD283D59583` | `rust_integration_test` | `crates/trust-runtime/tests/stdlib_numeric_full.rs` | `split_functions` | `not_ignored` |
| `DISC_8A3B6F48459300F8DE14` | `rust_integration_test` | `crates/trust-runtime/tests/stdlib_select.rs` | `selection_functions` | `not_ignored` |
| `DISC_583B8002BFC2F8376DD5` | `rust_integration_test` | `crates/trust-runtime/tests/stdlib_select_full.rs` | `selection_full` | `not_ignored` |
| `DISC_76542CA09E936B3579DA` | `rust_integration_test` | `crates/trust-runtime/tests/stdlib_split_locals.rs` | `function_local_initializer_runs_in_runtime_and_vm` | `not_ignored` |
| `DISC_32089A1392ACCCEAB8FD` | `rust_integration_test` | `crates/trust-runtime/tests/stdlib_split_locals.rs` | `split_date_writes_function_local_outputs_in_runtime_and_vm` | `not_ignored` |
| `DISC_86419F8A4B09368E6ABB` | `rust_integration_test` | `crates/trust-runtime/tests/stdlib_string.rs` | `bounded_string_assignment_and_concat_respect_declared_capacity` | `not_ignored` |
| `DISC_7C293FB77D79F5A48A48` | `rust_integration_test` | `crates/trust-runtime/tests/stdlib_string.rs` | `string_functions` | `not_ignored` |
| `DISC_ED3256A163AF5E79A376` | `rust_integration_test` | `crates/trust-runtime/tests/stdlib_string_full.rs` | `string_full` | `not_ignored` |
| `DISC_3C12488A93595292DFE3` | `rust_integration_test` | `crates/trust-runtime/tests/stdlib_string_full.rs` | `string_full_non_ascii_uses_character_elements` | `not_ignored` |
| `DISC_39C0DDEC611B86D08778` | `rust_integration_test` | `crates/trust-runtime/tests/stmt_assign_attempt.rs` | `assign_attempt` | `not_ignored` |
| `DISC_BB62DFC8BC8F6537A4DF` | `rust_integration_test` | `crates/trust-runtime/tests/stmt_full.rs` | `function_return_statement_uses_assigned_return_value_in_vm` | `not_ignored` |
| `DISC_02DE5694125E55EF3356` | `rust_integration_test` | `crates/trust-runtime/tests/stmt_full.rs` | `iec_table72` | `not_ignored` |
| `DISC_0037A0491E4B5384D3E1` | `rust_integration_test` | `crates/trust-runtime/tests/stmt_jmp.rs` | `jmp_flow` | `not_ignored` |
| `DISC_1A61C36FC0DB5422C732` | `rust_integration_test` | `crates/trust-runtime/tests/stmt_jmp.rs` | `jmp_to_empty_label` | `not_ignored` |
| `DISC_E29D23512AA82F3656E4` | `rust_integration_test` | `crates/trust-runtime/tests/struct_initializers.rs` | `array_of_structs_and_repetition_materialize_defaults` | `not_ignored` |
| `DISC_4B99744E60555A81AA18` | `rust_integration_test` | `crates/trust-runtime/tests/struct_initializers.rs` | `case_insensitive_field_matching_materializes_same_value` | `not_ignored` |
| `DISC_D6176A3A492A8CDEA314` | `rust_integration_test` | `crates/trust-runtime/tests/struct_initializers.rs` | `function_block_instance_initializer_applies_allowed_member_overrides` | `not_ignored` |
| `DISC_1484BD61D812C7FA07B5` | `rust_integration_test` | `crates/trust-runtime/tests/struct_initializers.rs` | `function_block_instance_initializer_rejects_var_in_out_member` | `not_ignored` |
| `DISC_A5E98ADF2D2B4599D420` | `rust_integration_test` | `crates/trust-runtime/tests/struct_initializers.rs` | `function_local_fb_initializer_applies_in_vm_local_init` | `not_ignored` |
| `DISC_31505AD0C1677A7EB176` | `rust_integration_test` | `crates/trust-runtime/tests/struct_initializers.rs` | `function_local_initializers_can_read_vm_frame_params_and_prior_locals` | `not_ignored` |
| `DISC_2D8A1719266B4C0E62D3` | `rust_integration_test` | `crates/trust-runtime/tests/struct_initializers.rs` | `global_fb_initializer_applies_allowed_member_overrides` | `not_ignored` |
| `DISC_6AA47EBC66D637C6B732` | `rust_integration_test` | `crates/trust-runtime/tests/struct_initializers.rs` | `multi_name_fb_initializer_instances_are_independent` | `not_ignored` |
| `DISC_2FAC0BF53BD3A073B6C6` | `rust_integration_test` | `crates/trust-runtime/tests/struct_initializers.rs` | `multi_name_struct_initializer_values_are_independent` | `not_ignored` |
| `DISC_1AFE5E7426D600A74064` | `rust_integration_test` | `crates/trust-runtime/tests/struct_initializers.rs` | `named_struct_initializer_materializes_runtime_value` | `not_ignored` |
| `DISC_229C78DC1329BDBE8E8B` | `rust_integration_test` | `crates/trust-runtime/tests/struct_initializers.rs` | `retained_struct_value_wins_over_defaults_on_warm_restart` | `not_ignored` |
| `DISC_C2320E4CC76D2E8B5988` | `rust_integration_test` | `crates/trust-runtime/tests/struct_initializers.rs` | `self_referential_ref_to_field_defaults_to_null_without_recursive_expansion` | `not_ignored` |
| `DISC_9B037F3332CA5921F727` | `rust_integration_test` | `crates/trust-runtime/tests/struct_initializers.rs` | `struct_field_defaults_feed_default_and_partial_aggregate_values` | `not_ignored` |
| `DISC_9AE28939D52140299CBD` | `rust_integration_test` | `crates/trust-runtime/tests/struct_initializers.rs` | `type_level_aggregate_default_materializes_alias_value` | `not_ignored` |
| `DISC_C5D7E155E16209E766B8` | `rust_integration_test` | `crates/trust-runtime/tests/struct_initializers.rs` | `type_level_array_of_struct_default_materializes` | `not_ignored` |
| `DISC_82B225A6A2EEF8FEE8BD` | `rust_integration_test` | `crates/trust-runtime/tests/struct_initializers.rs` | `type_name_call_initializer_materializes_runtime_value` | `not_ignored` |
| `DISC_8C2C2A9551785A641D71` | `rust_integration_test` | `crates/trust-runtime/tests/struct_initializers.rs` | `union_variant_default_materializes` | `not_ignored` |
| `DISC_1B3136267D1273A31259` | `rust_integration_test` | `crates/trust-runtime/tests/struct_initializers.rs` | `var_config_aggregate_override_wins_over_defaults` | `not_ignored` |
| `DISC_729DDCCE2637891A3049` | `rust_integration_test` | `crates/trust-runtime/tests/struct_initializers.rs` | `var_global_and_direct_address_aggregate_initializers_materialize` | `not_ignored` |
| `DISC_32ACC57324FC2710E856` | `rust_integration_test` | `crates/trust-runtime/tests/struct_initializers.rs` | `variable_level_ref_initializer_materializes_reference` | `not_ignored` |
| `DISC_5408231998FAE5B3DBBF` | `rust_integration_test` | `crates/trust-runtime/tests/tasks.rs` | `background_programs` | `not_ignored` |
| `DISC_E5A8E88590930205016C` | `rust_integration_test` | `crates/trust-runtime/tests/tasks.rs` | `event_edge_coalescing_between_samples` | `not_ignored` |
| `DISC_0903734F0B881C683F37` | `rust_integration_test` | `crates/trust-runtime/tests/tasks.rs` | `event_single_rise` | `not_ignored` |
| `DISC_EC4FAA3AB1619C66308A` | `rust_integration_test` | `crates/trust-runtime/tests/tasks.rs` | `fifo_order_by_due_time_within_priority` | `not_ignored` |
| `DISC_DDDF62B4558EEA7A608D` | `rust_integration_test` | `crates/trust-runtime/tests/tasks.rs` | `interval_zero_disables_periodic` | `not_ignored` |
| `DISC_3EA55BAB26D4720B4E89` | `rust_integration_test` | `crates/trust-runtime/tests/tasks.rs` | `periodic_interval` | `not_ignored` |
| `DISC_7513AC3A47E09CF764C8` | `rust_integration_test` | `crates/trust-runtime/tests/tasks.rs` | `priority_order` | `not_ignored` |
| `DISC_9ACE52E9206C80559114` | `rust_integration_test` | `crates/trust-runtime/tests/tasks.rs` | `single_blocks_periodic` | `not_ignored` |
| `DISC_22429344F278C5ABDD0A` | `rust_integration_test` | `crates/trust-runtime/tests/tasks.rs` | `task_overrun_drops_missed_intervals` | `not_ignored` |
| `DISC_12148E835FA57CC2824D` | `rust_integration_test` | `crates/trust-runtime/tests/tasks_fb.rs` | `fb_instance_runs_under_task_control` | `not_ignored` |
| `DISC_2651B1AEC636DC7CF852` | `rust_integration_test` | `crates/trust-runtime/tests/trust_harness_command.rs` | `trust_harness_advance_time_then_cycle_exposes_timer_progress` | `not_ignored` |
| `DISC_B70EF024D4E07AEF31C8` | `rust_integration_test` | `crates/trust-runtime/tests/trust_harness_command.rs` | `trust_harness_cycle_dt_ms_advances_virtual_time` | `not_ignored` |
| `DISC_BBEC2B1385A5107B3D01` | `rust_integration_test` | `crates/trust-runtime/tests/trust_harness_command.rs` | `trust_harness_protocol_version_1_keeps_legacy_watch_shape` | `not_ignored` |
| `DISC_011D4189FD4F4352D144` | `rust_integration_test` | `crates/trust-runtime/tests/trust_harness_command.rs` | `trust_harness_rejects_negative_dt_ms` | `not_ignored` |
| `DISC_88AB5F5BCE7A774B6BC0` | `rust_integration_test` | `crates/trust-runtime/tests/trust_harness_command.rs` | `trust_harness_reload_preserves_retain_state` | `not_ignored` |
| `DISC_5A83D1A380180FF9C29E` | `rust_integration_test` | `crates/trust-runtime/tests/trust_harness_command.rs` | `trust_harness_run_until_supports_success_and_bounded_timeout` | `not_ignored` |
| `DISC_4A7882D8E31A0C005650` | `rust_integration_test` | `crates/trust-runtime/tests/trust_harness_command.rs` | `trust_harness_set_input_then_get_output_roundtrips` | `not_ignored` |
| `DISC_766AB409A1732EE2D129` | `rust_integration_test` | `crates/trust-runtime/tests/tutorial_examples.rs` | `ethercat_ek1100_elx008_v1_example_parse_typecheck_and_compile_to_bytecode` | `not_ignored` |
| `DISC_0F9C3956D0F60788ACBE` | `rust_integration_test` | `crates/trust-runtime/tests/tutorial_examples.rs` | `mitsubishi_gxworks3_v1_example_parse_typecheck_and_compile_to_bytecode` | `not_ignored` |
| `DISC_0A3883680CDCFD2D87E6` | `rust_integration_test` | `crates/trust-runtime/tests/tutorial_examples.rs` | `plcopen_xml_st_complete_example_parse_typecheck_and_compile_to_bytecode` | `not_ignored` |
| `DISC_422C0C3613F04F4BA6AA` | `rust_integration_test` | `crates/trust-runtime/tests/tutorial_examples.rs` | `siemens_scl_v1_example_parse_typecheck_and_compile_to_bytecode` | `not_ignored` |
| `DISC_F46BC74C7DE6111123BE` | `rust_integration_test` | `crates/trust-runtime/tests/tutorial_examples.rs` | `tutorial_blinker_ton_timing_behavior` | `not_ignored` |
| `DISC_B1B41EFAE7B35700E367` | `rust_integration_test` | `crates/trust-runtime/tests/tutorial_examples.rs` | `tutorial_examples_parse_typecheck_and_compile_to_bytecode` | `not_ignored` |
| `DISC_CE8EBF020908F5AAF44F` | `rust_integration_test` | `crates/trust-runtime/tests/tutorial_examples.rs` | `tutorial_motor_starter_latch_and_unlatch` | `not_ignored` |
| `DISC_CA2458841CE10965BD22` | `rust_integration_test` | `crates/trust-runtime/tests/tutorial_examples.rs` | `tutorial_traffic_light_state_sequence` | `not_ignored` |
| `DISC_D0D08CBB515A257F07A1` | `rust_integration_test` | `crates/trust-runtime/tests/tutorial_examples.rs` | `visual_examples_compile_generated_companion_and_runtime_entry` | `not_ignored` |
| `DISC_62D9EE3567C3A125CD43` | `rust_integration_test` | `crates/trust-runtime/tests/types_bit_access.rs` | `bit_access_on_struct_byte_via_inout` | `not_ignored` |
| `DISC_D9CED73696DA3B83949E` | `rust_integration_test` | `crates/trust-runtime/tests/types_bit_access.rs` | `table17` | `not_ignored` |
| `DISC_57C633DBEE5FE05CEA07` | `rust_integration_test` | `crates/trust-runtime/tests/types_bit_access.rs` | `table17_fb` | `not_ignored` |
| `DISC_FA3777FFB0112B9DE247` | `rust_integration_test` | `crates/trust-runtime/tests/types_pointer.rs` | `pointer_to_string_supports_indexed_deref_read_and_write_in_runtime_and_vm` | `not_ignored` |
| `DISC_FFA1440FD18614F0A903` | `rust_integration_test` | `crates/trust-runtime/tests/types_pointer.rs` | `pointer_types_support_adr_deref_index_and_null_in_runtime_and_vm` | `not_ignored` |
| `DISC_788764AF3F34DD21DB12` | `rust_integration_test` | `crates/trust-runtime/tests/types_ref.rs` | `iec_table12` | `not_ignored` |
| `DISC_476D902920E3B57EC3E3` | `rust_integration_test` | `crates/trust-runtime/tests/types_struct_at.rs` | `struct_field_io` | `not_ignored` |
| `DISC_F8D227FBC6708EFDEA3E` | `rust_integration_test` | `crates/trust-runtime/tests/types_user.rs` | `iec_table11` | `not_ignored` |
| `DISC_C200F62A12C0B9F10052` | `rust_integration_test` | `crates/trust-runtime/tests/ui_no_input_smoke.rs` | `trust_runtime_ui_no_input_smoke` | `not_ignored` |
| `DISC_08567B380046C447396C` | `rust_integration_test` | `crates/trust-runtime/tests/value_defaults.rs` | `default_values_table10` | `not_ignored` |
| `DISC_A074772252F21C79F0CE` | `rust_integration_test` | `crates/trust-runtime/tests/value_defaults.rs` | `enum_defaults` | `not_ignored` |
| `DISC_3AFA5C16A3F5B14D5F11` | `rust_integration_test` | `crates/trust-runtime/tests/value_types.rs` | `supports_elementary_types` | `not_ignored` |
| `DISC_47A63A1480766092BD77` | `rust_integration_test` | `crates/trust-runtime/tests/var_constants.rs` | `iec_6_5_4` | `not_ignored` |
| `DISC_A66166BFEB9763DBB6E1` | `rust_integration_test` | `crates/trust-runtime/tests/var_constants.rs` | `parameter_constant_runtime_call_end_to_end` | `not_ignored` |
| `DISC_64155A401AC80B11DB4F` | `rust_integration_test` | `crates/trust-runtime/tests/var_init.rs` | `declaration_array_initializer_end_to_end` | `not_ignored` |
| `DISC_BB30FD4AFD91E52B8068` | `rust_integration_test` | `crates/trust-runtime/tests/var_init.rs` | `declaration_partial_array_initializer_default_fills_remaining_elements` | `not_ignored` |
| `DISC_228F27E52C6E7C5770DC` | `rust_integration_test` | `crates/trust-runtime/tests/var_init.rs` | `declaration_repetition_array_initializer_expands_group` | `not_ignored` |
| `DISC_76C36D65723E0778AA23` | `rust_integration_test` | `crates/trust-runtime/tests/var_init.rs` | `iec_table14` | `not_ignored` |
| `DISC_B36D3ADDC42E98CDE294` | `rust_integration_test` | `crates/trust-runtime/tests/var_stat.rs` | `function_var_stat_persists_across_calls` | `not_ignored` |
| `DISC_6F56B3D41F6C57F50DDE` | `rust_integration_test` | `crates/trust-runtime/tests/var_stat.rs` | `method_var_stat_is_isolated_per_instance` | `not_ignored` |
| `DISC_64DF53BDDC9047B2097E` | `rust_integration_test` | `crates/trust-runtime/tests/var_vla.rs` | `iec_table15` | `not_ignored` |
| `DISC_EA5C66EDBD8D79E5752C` | `rust_integration_test` | `crates/trust-runtime/tests/vars_access.rs` | `access_path_mapping` | `not_ignored` |
| `DISC_EA6765163DC75D504506` | `rust_integration_test` | `crates/trust-runtime/tests/vars_access.rs` | `encoder_rejects_forced_access_map_binding_that_shadows_global_name` | `not_ignored` |
| `DISC_235D59FA001748508DAD` | `rust_integration_test` | `crates/trust-runtime/tests/vars_access.rs` | `file_scope_globals_are_shared_across_program_and_function_blocks` | `not_ignored` |
| `DISC_261CF481723D966B163B` | `rust_integration_test` | `crates/trust-runtime/tests/vars_access.rs` | `globals_are_accessible_without_var_external_across_vendor_parity_scopes` | `not_ignored` |
| `DISC_CFED6C337EE5017DEAA1` | `rust_integration_test` | `crates/trust-runtime/tests/vars_access.rs` | `memory_variants_sync_via_var_config_wildcards` | `not_ignored` |
| `DISC_45460E6E5D9F531D3608` | `rust_integration_test` | `crates/trust-runtime/tests/vars_access.rs` | `namespaced_globals_support_qualified_access` | `not_ignored` |
| `DISC_0759FB4B6E9DDFC3E864` | `rust_integration_test` | `crates/trust-runtime/tests/vars_access.rs` | `var_access_global_name_collision_fails_before_bytecode` | `not_ignored` |
| `DISC_29115CC5D5982DB2D2E4` | `rust_integration_test` | `crates/trust-runtime/tests/vars_access.rs` | `var_config_memory_binding_syncs_with_program_storage` | `not_ignored` |
| `DISC_A8D2BD3F8B99BD3B50F1` | `rust_integration_test` | `crates/trust-runtime/tests/vars_at.rs` | `iec_6_5_5` | `not_ignored` |
| `DISC_6DD50B772CED1C582EC6` | `rust_integration_test` | `crates/trust-runtime/tests/vars_retain.rs` | `iec_6_5_6` | `not_ignored` |
| `DISC_A77CDB943FD0784CC65A` | `rust_integration_test` | `crates/trust-runtime/tests/web_dispatch_hol_probe.rs` | `incomplete_body_does_not_block_unrelated_hmi_request` | `not_ignored` |
| `DISC_7B7DA852E4D9CE86AF04` | `rust_integration_test` | `crates/trust-runtime/tests/web_dispatch_hol_probe.rs` | `saturated_body_lane_rejects_promptly_without_blocking_hmi_and_recovers` | `not_ignored` |
| `DISC_71687ECB877C3D7C3908` | `rust_integration_test` | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_01.rs` | `web_ide_auth_and_session_contract` | `not_ignored` |
| `DISC_6EF448DFF15CBE99481F` | `rust_integration_test` | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_01.rs` | `web_ide_project_open_endpoint_supports_no_bundle_startup` | `not_ignored` |
| `DISC_30DFAFB99FA2F174D99C` | `rust_integration_test` | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_01.rs` | `web_ide_project_open_requires_editor_and_approved_base` | `not_ignored` |
| `DISC_7E40F37C484F42B3485C` | `rust_integration_test` | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_01.rs` | `web_ide_shell_serves_local_hashed_assets_without_cdn_dependency` | `not_ignored` |
| `DISC_207B274F176363CF8993` | `rust_integration_test` | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_02.rs` | `web_ide_collaborative_conflict_contract` | `not_ignored` |
| `DISC_A620D5348ADF73D5D7F7` | `rust_integration_test` | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_02.rs` | `web_ide_viewer_sessions_are_read_only_and_editor_sessions_can_write` | `not_ignored` |
| `DISC_B6B405B11078BF3B5FA4` | `rust_integration_test` | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_03.rs` | `web_ide_latency_and_resource_budget_contract` | `not_ignored` |
| `DISC_93181B5A901F36A97B10` | `rust_integration_test` | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_04.rs` | `web_ide_reference_performance_gates_contract` | `not_ignored` |
| `DISC_E67CDF1BC0452A1B4204` | `rust_integration_test` | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_05.rs` | `web_ide_analysis_and_health_endpoints_contract` | `not_ignored` |
| `DISC_8C2FA84351B10CC7CA56` | `rust_integration_test` | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_05.rs` | `web_ide_format_endpoint_contract` | `not_ignored` |
| `DISC_3CD0633DDEACB34C65AA` | `rust_integration_test` | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_06.rs` | `web_ide_tree_and_filesystem_endpoints_contract` | `not_ignored` |
| `DISC_A27D37D9624F9A5963A2` | `rust_integration_test` | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_07.rs` | `pair_claim_rejects_oversized_json_body` | `not_ignored` |
| `DISC_C9A33997939425A42529` | `rust_integration_test` | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_07.rs` | `web_ide_security_and_path_traversal_contract` | `not_ignored` |
| `DISC_5ED869E68F0E60F2CA1F` | `rust_integration_test` | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_08.rs` | `web_ide_build_test_and_validate_task_endpoints_contract` | `not_ignored` |
| `DISC_FDFF3C8B7FDE96BF0A43` | `rust_integration_test` | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_08.rs` | `web_ide_navigation_search_and_rename_endpoints_contract` | `not_ignored` |
| `DISC_C9FFA0F5E342456F1897` | `rust_integration_test` | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_09/hardware.rs` | `unified_shell_exposes_mqtt_connectivity_probe_api` | `not_ignored` |
| `DISC_A09342D95623D8B56793` | `rust_integration_test` | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_09/hardware.rs` | `unified_shell_hardware_module_exposes_runtime_cloud_link_transport_projection` | `not_ignored` |
| `DISC_0CBDE2A7B553C0654F26` | `rust_integration_test` | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_09/hardware.rs` | `unified_shell_serves_composed_ide_modules_required_for_bootstrap` | `not_ignored` |
| `DISC_105F7B3C79DD106058B2` | `rust_integration_test` | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_09/io_control.rs` | `unified_shell_control_proxy_supports_runtime_status_forwarding` | `not_ignored` |
| `DISC_432185877835FBC0AC24` | `rust_integration_test` | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_09/io_control.rs` | `unified_shell_ide_io_config_post_writes_active_workspace_io_file` | `not_ignored` |
| `DISC_E4AD64BB7850412329E9` | `rust_integration_test` | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_09/io_control.rs` | `unified_shell_ide_io_config_route_tracks_active_workspace` | `not_ignored` |
| `DISC_10D32A58A8974C3437F9` | `rust_integration_test` | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_09/settings.rs` | `unified_shell_settings_module_exposes_realtime_link_configuration_fields` | `not_ignored` |
| `DISC_5ED9ED988073F5371660` | `rust_integration_test` | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_09/shell_api.rs` | `unified_shell_html_contract_contains_tab_panels_and_status_bar` | `not_ignored` |
| `DISC_89BDE58ED2758547A89B` | `rust_integration_test` | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_09/shell_api.rs` | `unified_shell_ide_client_supports_wrapped_and_direct_api_payloads` | `not_ignored` |
| `DISC_BCE4605CFAE7B1031C86` | `rust_integration_test` | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_09/shell_api.rs` | `unified_shell_removes_legacy_fleet_routes` | `not_ignored` |
| `DISC_2F1B45B0EBC8AFE8C307` | `rust_integration_test` | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_09/shell_modules.rs` | `unified_shell_base_css_enforces_hidden_attribute_contract` | `not_ignored` |
| `DISC_2E72F9869B8235838DD5` | `rust_integration_test` | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_09/shell_modules.rs` | `unified_shell_entry_routes_redirect_to_ide` | `not_ignored` |
| `DISC_762A97917AE0CAFB87B0` | `rust_integration_test` | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_09/shell_modules.rs` | `unified_shell_header_uses_compact_toolbar_with_overflow_menu` | `not_ignored` |
| `DISC_22F3824C7A76391C9EB7` | `rust_integration_test` | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_09/shell_modules.rs` | `unified_shell_online_module_defaults_connection_to_same_origin_and_auto_connects` | `not_ignored` |
| `DISC_FFBE62E4915C3AFE7559` | `rust_integration_test` | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_09/shell_modules.rs` | `unified_shell_serves_all_ide_tab_modules` | `not_ignored` |
| `DISC_E9FD35EEE9AE946AB5E8` | `rust_integration_test` | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_09/shell_modules.rs` | `unified_shell_tab_deep_links_serve_ide_html` | `not_ignored` |
| `DISC_D483FA51A0F5707E782B` | `rust_integration_test` | `crates/trust-runtime/tests/web_ide_integration/web_ide_integration_part_09/shell_modules.rs` | `unified_shell_tab_module_enforces_tab_aria_contract` | `not_ignored` |
| `DISC_F6E17543C6E5EB5E79BA` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_01.rs` | `io_config_endpoint_rejects_invalid_driver_params_shape` | `not_ignored` |
| `DISC_73C09A6E6332374DDD48` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_01.rs` | `io_config_endpoint_round_trips_multi_driver_payload` | `not_ignored` |
| `DISC_E93FABAF7C59E8624A8B` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_01.rs` | `runtime_cloud_state_endpoint_exposes_context_and_topology_contract` | `not_ignored` |
| `DISC_F40798D55C3053BE0BBB` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_02.rs` | `runtime_cloud_topology_devices_get_route_removed_returns_404` | `not_ignored` |
| `DISC_FF389AFB0C0F656C2142` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_02.rs` | `runtime_cloud_topology_devices_post_route_removed_returns_404` | `not_ignored` |
| `DISC_999A7BAE310BC7C506F6` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_03.rs` | `runtime_cloud_link_transport_endpoint_switches_edge_mode_and_checks_same_host` | `not_ignored` |
| `DISC_C3132F3F3FE9F9343A74` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_08.rs` | `runtime_cloud_config_agent_reconciles_desired_reported_meta_and_status` | `not_ignored` |
| `DISC_3356BAB1EC7425B49914` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_08.rs` | `runtime_cloud_config_desired_write_enforces_revision_and_etag_conflict` | `not_ignored` |
| `DISC_0BE4515DD096557819FD` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_08.rs` | `runtime_cloud_discovery_endpoint_exposes_secure_transport_metadata` | `not_ignored` |
| `DISC_FE959E7B21A272DDC249` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_08.rs` | `runtime_cloud_state_marks_fresh_mesh_disconnect_as_stale_before_partitioned` | `not_ignored` |
| `DISC_71AAD9DC8B0010E7C6C4` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_09.rs` | `runtime_cloud_config_conflict_rebase_retry_applies_latest_desired` | `not_ignored` |
| `DISC_FB97F87B230CB3AAE6BE` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_09.rs` | `runtime_cloud_config_reconcile_surfaces_error_state_for_invalid_desired_payload` | `not_ignored` |
| `DISC_6B8BB5D664891D81B35F` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_10.rs` | `runtime_cloud_config_partial_desired_subtree_write_keeps_existing_keys` | `not_ignored` |
| `DISC_F6B45DEF6650A34B17B3` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_11_part_01.rs` | `runtime_cloud_rollout_state_machine_covers_happy_failed_and_aborted_paths` | `not_ignored` |
| `DISC_7EB0FA737B6541B5F4BA` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_12.rs` | `runtime_cloud_config_agent_recovers_pending_state_after_restart` | `not_ignored` |
| `DISC_483F5AE4BFD292D7C36A` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_12.rs` | `runtime_cloud_preflight_rejects_non_json_content_type` | `not_ignored` |
| `DISC_C0E6C62D6C83294C9BEA` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_12.rs` | `runtime_cloud_preflight_returns_deterministic_unreachable_denial` | `not_ignored` |
| `DISC_E7A707C5FC176B559C9B` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_12.rs` | `runtime_cloud_state_requires_secure_profile_transport_in_plant_mode` | `not_ignored` |
| `DISC_D74A24E1A5D2F78A6E2D` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_13.rs` | `runtime_cloud_dispatch_cancels_fanout_when_query_budget_is_exhausted` | `not_ignored` |
| `DISC_F3F082D6F583D954B0F2` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_13.rs` | `runtime_cloud_dispatch_unreachable_target_does_not_fallback_to_local_apply` | `not_ignored` |
| `DISC_E1DC6075139AEC3F81D4` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_13.rs` | `runtime_cloud_preflight_rejects_cross_origin_post` | `not_ignored` |
| `DISC_1C1CA5C9E00FF9ADC3C6` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_13.rs` | `runtime_cloud_preflight_rejects_oversized_json_body` | `not_ignored` |
| `DISC_9DD6A4A53CAF3EEAA884` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_14.rs` | `runtime_cloud_link_transport_preferences_change_is_audited_and_roundtrips` | `not_ignored` |
| `DISC_1B94D1E7121F3D517CD2` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_14.rs` | `runtime_cloud_preflight_allows_cross_site_cfg_apply_with_allowlist` | `not_ignored` |
| `DISC_20EA5D2499082C921BE3` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_14.rs` | `runtime_cloud_preflight_denies_cross_site_cfg_apply_without_allowlist` | `not_ignored` |
| `DISC_16EC92EDB75F1BB6C5EF` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_14.rs` | `runtime_cloud_preflight_wan_requires_secure_profile_preconditions` | `not_ignored` |
| `DISC_799A11C490E310F77621` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_14.rs` | `runtime_cloud_wan_allowlist_policy_change_is_audited` | `not_ignored` |
| `DISC_A87E1CF2D7FC307115AB` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_15.rs` | `runtime_cloud_dispatch_keeps_local_cfg_apply_operational_when_peer_is_partitioned` | `not_ignored` |
| `DISC_DFE0F3CCEBCE33E52202` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_15.rs` | `runtime_cloud_preflight_denies_cfg_apply_for_viewer_with_deterministic_acl_code` | `not_ignored` |
| `DISC_A607E4845FB824B33FC1` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_15.rs` | `runtime_cloud_preflight_marks_partial_partition_target_as_stale` | `not_ignored` |
| `DISC_526714F31DC8382E8F14` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_16.rs` | `runtime_cloud_dispatch_reaches_remote_runtime_and_propagates_audit_correlation_id` | `not_ignored` |
| `DISC_3362A72E05A959F7FFB3` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_16.rs` | `runtime_cloud_dispatch_routes_cfg_apply_to_local_runtime` | `not_ignored` |
| `DISC_8D54213D4C85673C089A` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_16.rs` | `runtime_cloud_preflight_denies_cmd_invoke_for_viewer_with_permission_denied_code` | `not_ignored` |
| `DISC_CA23666027C91B2552EE` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_17.rs` | `runtime_cloud_control_proxy_reads_remote_runtime_status` | `not_ignored` |
| `DISC_CB4DC2C360EA8DE45593` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_17.rs` | `runtime_cloud_dispatch_reads_remote_runtime_status_via_connected_runtime` | `not_ignored` |
| `DISC_32C133895EF215A753BF` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_17.rs` | `runtime_cloud_io_config_proxy_reads_remote_runtime_config` | `not_ignored` |
| `DISC_33A2FF91EF8E84727972` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_18.rs` | `runtime_cloud_io_config_proxy_writes_remote_runtime_config` | `not_ignored` |
| `DISC_9E7E128921D261CC0DDE` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_18.rs` | `runtime_cloud_remote_dispatch_emits_audit_for_success_and_failure_paths` | `not_ignored` |
| `DISC_7B17DF848976C1D85555` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_19.rs` | `runtime_cloud_ha_dual_output_prevention_blocks_standby_dispatch` | `not_ignored` |
| `DISC_D8E0650A3CFEE8322AF6` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_19.rs` | `runtime_cloud_ha_lease_expiry_demotes_active_runtime_preflight` | `not_ignored` |
| `DISC_F32834939D3E45E83A60` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_19.rs` | `runtime_cloud_ha_split_brain_preflight_denies_dual_active_candidates` | `not_ignored` |
| `DISC_FA02B66534732B4B60AE` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_20.rs` | `runtime_cloud_ha_replay_guard_deduplicates_and_rejects_stale_seq` | `not_ignored` |
| `DISC_396939BD2756D55428A1` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_21.rs` | `config_ui_live_targets_and_live_state_endpoints_roundtrip` | `not_ignored` |
| `DISC_04EC41D487CA069D2A59` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_21.rs` | `config_ui_mode_serves_project_state_and_topology_projection` | `not_ignored` |
| `DISC_16961D8EE3934288AF91` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_21.rs` | `config_ui_runtime_config_write_conflict_is_reported` | `not_ignored` |
| `DISC_FF99B05A385D3919D209` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_21.rs` | `config_ui_runtime_create_and_delete_roundtrip` | `not_ignored` |
| `DISC_FF0739B00985201E2FDB` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_21.rs` | `config_ui_runtime_lifecycle_endpoints_report_workspace_runtimes` | `not_ignored` |
| `DISC_830B0FCD52472FC05488` | `rust_integration_test` | `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_21.rs` | `config_ui_st_file_write_and_validate_roundtrip` | `not_ignored` |
| `DISC_EA95B65B17CA05D25CB2` | `rust_integration_test` | `crates/trust-runtime/tests/web_tls_integration.rs` | `web_tls_handshake_and_downgrade_prevention` | `not_ignored` |
| `DISC_575043E7381C1B116FBF` | `rust_integration_test` | `crates/trust-syntax/tests/lexer_common.rs` | `iec_6_1` | `not_ignored` |
| `DISC_5FA62CC42AACE9953F96` | `rust_integration_test` | `crates/trust-syntax/tests/lexer_common.rs` | `numeric_dot_and_range_tokens_do_not_need_int_literal_dot_rewrite` | `not_ignored` |
| `DISC_BBB1B7E2434B4F7B2DFA` | `rust_integration_test` | `crates/trust-syntax/tests/lexer_literals.rs` | `iec_table5` | `not_ignored` |
| `DISC_1B0D02B039D6704057D4` | `rust_integration_test` | `crates/trust-syntax/tests/lexer_literals.rs` | `iec_tables6_7` | `not_ignored` |
| `DISC_8715868A7C5CBEDBE314` | `rust_integration_test` | `crates/trust-syntax/tests/lexer_literals.rs` | `iec_tables8_9` | `not_ignored` |
| `DISC_AAADEA7806AA1F78A9A3` | `rust_integration_test` | `crates/trust-syntax/tests/lexer_pragmas.rs` | `iec_6_2` | `not_ignored` |
| `DISC_7ED161A7FAC81F254D67` | `rust_integration_test` | `crates/trust-syntax/tests/parser_complex.rs` | `test_complete_function_block` | `not_ignored` |
| `DISC_CB4F519F73BE3B2118DE` | `rust_integration_test` | `crates/trust-syntax/tests/parser_error_recovery.rs` | `test_hash_without_identifier` | `not_ignored` |
| `DISC_C0ABE0A15530B5A2F5C0` | `rust_integration_test` | `crates/trust-syntax/tests/parser_error_recovery.rs` | `test_invalid_signed_based_typed_literal` | `not_ignored` |
| `DISC_BD20C08C66256CFB638D` | `rust_integration_test` | `crates/trust-syntax/tests/parser_error_recovery.rs` | `test_missing_end_if` | `not_ignored` |
| `DISC_F861248D3482A78B0E6F` | `rust_integration_test` | `crates/trust-syntax/tests/parser_error_recovery.rs` | `test_missing_end_test_program` | `not_ignored` |
| `DISC_5AFBA0A9B09308B307A8` | `rust_integration_test` | `crates/trust-syntax/tests/parser_error_recovery.rs` | `test_missing_semicolon` | `not_ignored` |
| `DISC_F627D87564ECA0E3906F` | `rust_integration_test` | `crates/trust-syntax/tests/parser_error_recovery.rs` | `test_missing_then` | `not_ignored` |
| `DISC_1C42DB389FAA2ED580BB` | `rust_integration_test` | `crates/trust-syntax/tests/parser_expressions.rs` | `exponentiation_is_left_associative_per_iec_table_71` | `not_ignored` |
| `DISC_97D61BB7559A7DD6E76E` | `rust_integration_test` | `crates/trust-syntax/tests/parser_expressions.rs` | `test_adr_sizeof` | `not_ignored` |
| `DISC_92D9633307186FC98E4B` | `rust_integration_test` | `crates/trust-syntax/tests/parser_expressions.rs` | `test_arithmetic_operators` | `not_ignored` |
| `DISC_80070FE89B24B89216E5` | `rust_integration_test` | `crates/trust-syntax/tests/parser_expressions.rs` | `test_array_indexing` | `not_ignored` |
| `DISC_369516AA6B664D5C95D8` | `rust_integration_test` | `crates/trust-syntax/tests/parser_expressions.rs` | `test_comparison_operators` | `not_ignored` |
| `DISC_DE0A37F5DBCEA9EC22BD` | `rust_integration_test` | `crates/trust-syntax/tests/parser_expressions.rs` | `test_field_access` | `not_ignored` |
| `DISC_DEAB54E384071EC060B1` | `rust_integration_test` | `crates/trust-syntax/tests/parser_expressions.rs` | `test_function_call` | `not_ignored` |
| `DISC_0C64B66155D18AB6A349` | `rust_integration_test` | `crates/trust-syntax/tests/parser_expressions.rs` | `test_logical_operators` | `not_ignored` |
| `DISC_A24D196C0A5E43F2BAC6` | `rust_integration_test` | `crates/trust-syntax/tests/parser_expressions.rs` | `test_operator_precedence` | `not_ignored` |
| `DISC_040FA7FD5A9C4FA012E9` | `rust_integration_test` | `crates/trust-syntax/tests/parser_expressions.rs` | `test_pointer_dereference` | `not_ignored` |
| `DISC_844BE5EF314ABC05FC5D` | `rust_integration_test` | `crates/trust-syntax/tests/parser_expressions.rs` | `test_siemens_hash_prefixed_locals` | `not_ignored` |
| `DISC_3B724A69637CEE49AF44` | `rust_integration_test` | `crates/trust-syntax/tests/parser_expressions.rs` | `test_sizeof_call_operand_is_expression_not_type_ref` | `not_ignored` |
| `DISC_660B3271451CCA419643` | `rust_integration_test` | `crates/trust-syntax/tests/parser_expressions.rs` | `test_sizeof_deref_operand_is_expression_not_type_ref` | `not_ignored` |
| `DISC_7952C429413C84BEC774` | `rust_integration_test` | `crates/trust-syntax/tests/parser_expressions.rs` | `test_sizeof_explicit_array_type_operand_is_type_ref` | `not_ignored` |
| `DISC_C30D90A345E81DB8C44F` | `rust_integration_test` | `crates/trust-syntax/tests/parser_expressions.rs` | `test_sizeof_explicit_builtin_type_operand_is_type_ref` | `not_ignored` |
| `DISC_A5EA38429E32754E180A` | `rust_integration_test` | `crates/trust-syntax/tests/parser_expressions.rs` | `test_sizeof_field_operand_is_expression_not_type_ref` | `not_ignored` |
| `DISC_242B246E9CD7517FDEF5` | `rust_integration_test` | `crates/trust-syntax/tests/parser_expressions.rs` | `test_sizeof_index_operand_is_expression_not_type_ref` | `not_ignored` |
| `DISC_67045A053B4987BD9299` | `rust_integration_test` | `crates/trust-syntax/tests/parser_expressions.rs` | `test_sizeof_variable_operand_is_expression_not_type_ref` | `not_ignored` |
| `DISC_91B00FC87E16AEF9D815` | `rust_integration_test` | `crates/trust-syntax/tests/parser_expressions.rs` | `test_this_super` | `not_ignored` |
| `DISC_A1A6F0948A4713FAF153` | `rust_integration_test` | `crates/trust-syntax/tests/parser_expressions.rs` | `test_time_builtin_call` | `not_ignored` |
| `DISC_4DF938F54F2FA1306C1E` | `rust_integration_test` | `crates/trust-syntax/tests/parser_expressions.rs` | `test_unary_operators` | `not_ignored` |
| `DISC_72A4635A2F50FA9B6F66` | `rust_integration_test` | `crates/trust-syntax/tests/parser_expressions.rs` | `unary_minus_binds_tighter_than_exponentiation_per_iec_table_71` | `not_ignored` |
| `DISC_930CC415CFEB0A8D08F0` | `rust_integration_test` | `crates/trust-syntax/tests/parser_literals.rs` | `test_boolean_literals` | `not_ignored` |
| `DISC_B2E4F66FB0D35F64903E` | `rust_integration_test` | `crates/trust-syntax/tests/parser_literals.rs` | `test_date_literals` | `not_ignored` |
| `DISC_8B565ECC1E11CAE65DF5` | `rust_integration_test` | `crates/trust-syntax/tests/parser_literals.rs` | `test_integer_literals` | `not_ignored` |
| `DISC_912A1D0AEA9E0C84B9A9` | `rust_integration_test` | `crates/trust-syntax/tests/parser_literals.rs` | `test_real_literals` | `not_ignored` |
| `DISC_6B03272519B37089A6DD` | `rust_integration_test` | `crates/trust-syntax/tests/parser_literals.rs` | `test_string_literals` | `not_ignored` |
| `DISC_805F2C665E75A8E33AB7` | `rust_integration_test` | `crates/trust-syntax/tests/parser_literals.rs` | `test_time_literals` | `not_ignored` |
| `DISC_6452349A9EE83ECBBC37` | `rust_integration_test` | `crates/trust-syntax/tests/parser_literals.rs` | `test_typed_literals` | `not_ignored` |
| `DISC_0B4EB1177A669105BB48` | `rust_integration_test` | `crates/trust-syntax/tests/parser_pous.rs` | `test_action` | `not_ignored` |
| `DISC_69B10FABD36B24FE599A` | `rust_integration_test` | `crates/trust-syntax/tests/parser_pous.rs` | `test_class_declaration` | `not_ignored` |
| `DISC_9280A9739478FD9C554E` | `rust_integration_test` | `crates/trust-syntax/tests/parser_pous.rs` | `test_class_qualified_extends_implements` | `not_ignored` |
| `DISC_D9946A99F129EC3AE4F1` | `rust_integration_test` | `crates/trust-syntax/tests/parser_pous.rs` | `test_configuration` | `not_ignored` |
| `DISC_DE62025A26A86F67F39D` | `rust_integration_test` | `crates/trust-syntax/tests/parser_pous.rs` | `test_empty_program` | `not_ignored` |
| `DISC_B06CB96D55F06E4E7B93` | `rust_integration_test` | `crates/trust-syntax/tests/parser_pous.rs` | `test_function_block` | `not_ignored` |
| `DISC_6B6C2005B158D524747F` | `rust_integration_test` | `crates/trust-syntax/tests/parser_pous.rs` | `test_function_block_extends` | `not_ignored` |
| `DISC_132C51368691E4ACA7A4` | `rust_integration_test` | `crates/trust-syntax/tests/parser_pous.rs` | `test_function_block_implements` | `not_ignored` |
| `DISC_7E641C521D52CB31C083` | `rust_integration_test` | `crates/trust-syntax/tests/parser_pous.rs` | `test_function_block_top_level_statements_form_stmt_list` | `not_ignored` |
| `DISC_157C2D9DE0C16280F1FC` | `rust_integration_test` | `crates/trust-syntax/tests/parser_pous.rs` | `test_function_with_return_type` | `not_ignored` |
| `DISC_2DD47463E38CD0C6B50A` | `rust_integration_test` | `crates/trust-syntax/tests/parser_pous.rs` | `test_interface` | `not_ignored` |
| `DISC_314498D9CCD74BBC02FE` | `rust_integration_test` | `crates/trust-syntax/tests/parser_pous.rs` | `test_method_with_body` | `not_ignored` |
| `DISC_48EBB052B5162666BA2B` | `rust_integration_test` | `crates/trust-syntax/tests/parser_pous.rs` | `test_namespace` | `not_ignored` |
| `DISC_28E425C23DE358335943` | `rust_integration_test` | `crates/trust-syntax/tests/parser_pous.rs` | `test_namespace_qualified_name` | `not_ignored` |
| `DISC_F712646D984E58F6E0A8` | `rust_integration_test` | `crates/trust-syntax/tests/parser_pous.rs` | `test_namespace_with_var_global` | `not_ignored` |
| `DISC_4CE47E9236C3393B599F` | `rust_integration_test` | `crates/trust-syntax/tests/parser_pous.rs` | `test_program_with_var_block` | `not_ignored` |
| `DISC_E04E23863B97B597D39E` | `rust_integration_test` | `crates/trust-syntax/tests/parser_pous.rs` | `test_property` | `not_ignored` |
| `DISC_1641D22035FF1021837D` | `rust_integration_test` | `crates/trust-syntax/tests/parser_pous.rs` | `test_test_function_block` | `not_ignored` |
| `DISC_19D2CE16E17DF11154B6` | `rust_integration_test` | `crates/trust-syntax/tests/parser_pous.rs` | `test_test_program` | `not_ignored` |
| `DISC_D7A9ABD35C01112BF887` | `rust_integration_test` | `crates/trust-syntax/tests/parser_pous.rs` | `test_using_directive` | `not_ignored` |
| `DISC_49687F4FF7933F5F2ADB` | `rust_integration_test` | `crates/trust-syntax/tests/parser_pous.rs` | `test_var_access_with_index_and_bit` | `not_ignored` |
| `DISC_ABC89CA7DBB5226C25B0` | `rust_integration_test` | `crates/trust-syntax/tests/parser_project_examples.rs` | `test_examples_parse` | `not_ignored` |
| `DISC_0F2DDCAB9BE1767C6D17` | `rust_integration_test` | `crates/trust-syntax/tests/parser_project_examples.rs` | `test_pragmas_parse_and_preserve_tokens` | `not_ignored` |
| `DISC_5F406CAEB978B707827D` | `rust_integration_test` | `crates/trust-syntax/tests/parser_statements.rs` | `test_assignment_attempt` | `not_ignored` |
| `DISC_8E26DA86493D7F27B8A5` | `rust_integration_test` | `crates/trust-syntax/tests/parser_statements.rs` | `test_case_statement` | `not_ignored` |
| `DISC_9980C535C02811084344` | `rust_integration_test` | `crates/trust-syntax/tests/parser_statements.rs` | `test_exit_continue` | `not_ignored` |
| `DISC_0D125EFC814B673B762D` | `rust_integration_test` | `crates/trust-syntax/tests/parser_statements.rs` | `test_for_loop` | `not_ignored` |
| `DISC_EAAB547DE413E9A8A8FB` | `rust_integration_test` | `crates/trust-syntax/tests/parser_statements.rs` | `test_if_elsif_else` | `not_ignored` |
| `DISC_9634CE251F783796BE54` | `rust_integration_test` | `crates/trust-syntax/tests/parser_statements.rs` | `test_if_statement` | `not_ignored` |
| `DISC_D94000CA9175B64CE654` | `rust_integration_test` | `crates/trust-syntax/tests/parser_statements.rs` | `test_jmp_statement` | `not_ignored` |
| `DISC_291C3EBEC7E833C93A5C` | `rust_integration_test` | `crates/trust-syntax/tests/parser_statements.rs` | `test_label_statement` | `not_ignored` |
| `DISC_7E1CB500E92AD2FEAB80` | `rust_integration_test` | `crates/trust-syntax/tests/parser_statements.rs` | `test_output_connection` | `not_ignored` |
| `DISC_8D24202CA10A5562C5CF` | `rust_integration_test` | `crates/trust-syntax/tests/parser_statements.rs` | `test_repeat_loop` | `not_ignored` |
| `DISC_DFF275CC5DC79F7F2B11` | `rust_integration_test` | `crates/trust-syntax/tests/parser_statements.rs` | `test_return_statement` | `not_ignored` |
| `DISC_01AED98E2F97A54B9555` | `rust_integration_test` | `crates/trust-syntax/tests/parser_statements.rs` | `test_siemens_hash_prefixed_statement_forms` | `not_ignored` |
| `DISC_0925AFD6015516772D6D` | `rust_integration_test` | `crates/trust-syntax/tests/parser_statements.rs` | `test_while_loop` | `not_ignored` |
| `DISC_9F5C3B5A29DEB7FCDA89` | `rust_integration_test` | `crates/trust-syntax/tests/parser_types.rs` | `test_array_type` | `not_ignored` |
| `DISC_EE8ECC9BD204BC50BEEA` | `rust_integration_test` | `crates/trust-syntax/tests/parser_types.rs` | `test_enum_type` | `not_ignored` |
| `DISC_45DD3DD6BD6F1B83A7F7` | `rust_integration_test` | `crates/trust-syntax/tests/parser_types.rs` | `test_pointer_type` | `not_ignored` |
| `DISC_6DA2B9381B2A0F3A9DC1` | `rust_integration_test` | `crates/trust-syntax/tests/parser_types.rs` | `test_string_type_with_length` | `not_ignored` |
| `DISC_BC855A3FE686F66002C1` | `rust_integration_test` | `crates/trust-syntax/tests/parser_types.rs` | `test_string_type_with_parenthesized_length` | `not_ignored` |
| `DISC_98D93551073158A07EE3` | `rust_integration_test` | `crates/trust-syntax/tests/parser_types.rs` | `test_struct_type` | `not_ignored` |
| `DISC_B053583EF10943BA7A9C` | `rust_integration_test` | `crates/trust-syntax/tests/parser_types.rs` | `test_type_alias` | `not_ignored` |
| `DISC_EB8709AACC1F37E9B154` | `rust_integration_test` | `crates/trust-syntax/tests/parser_types.rs` | `test_type_level_defaults_cover_directly_derived_shapes` | `not_ignored` |
| `DISC_6CC791442390F88B2DA9` | `rust_integration_test` | `crates/trust-syntax/tests/parser_types.rs` | `test_type_level_named_aggregate_defaults` | `not_ignored` |
| `DISC_C88643F355A4CFA49DF0` | `rust_integration_test` | `crates/trust-syntax/tests/parser_variables.rs` | `parse_array_star_in_pointer_to_array` | `not_ignored` |
| `DISC_AA1F7E39CA7ECDE9AEC2` | `rust_integration_test` | `crates/trust-syntax/tests/parser_variables.rs` | `parse_array_star_in_var_in_out` | `not_ignored` |
| `DISC_BAFFBB88330C39CA4020` | `rust_integration_test` | `crates/trust-syntax/tests/parser_variables.rs` | `parse_array_star_in_var_input` | `not_ignored` |
| `DISC_9DFCF021E78DDDE1500A` | `rust_integration_test` | `crates/trust-syntax/tests/parser_variables.rs` | `test_call_arguments_remain_call_arguments` | `not_ignored` |
| `DISC_F85DA6E835C2F33830D1` | `rust_integration_test` | `crates/trust-syntax/tests/parser_variables.rs` | `test_fb_instance_aggregate_initializer_parse` | `not_ignored` |
| `DISC_7B935FCCC5563522FBA4` | `rust_integration_test` | `crates/trust-syntax/tests/parser_variables.rs` | `test_file_scope_var_global` | `not_ignored` |
| `DISC_17EF3CAC510BCE1AC88C` | `rust_integration_test` | `crates/trust-syntax/tests/parser_variables.rs` | `test_initializer_parser_is_not_used_for_enum_values_or_calls` | `not_ignored` |
| `DISC_21449A3BBD5F3F55D531` | `rust_integration_test` | `crates/trust-syntax/tests/parser_variables.rs` | `test_initializer_recovery_property_smoke_for_generated_positional_shapes` | `not_ignored` |
| `DISC_D1BD83DB9C4B98C948C7` | `rust_integration_test` | `crates/trust-syntax/tests/parser_variables.rs` | `test_positional_and_empty_aggregate_recovery_is_bounded` | `not_ignored` |
| `DISC_BB91AC7A18EC0953D601` | `rust_integration_test` | `crates/trust-syntax/tests/parser_variables.rs` | `test_positional_initializer_recovery_preserves_declaration_boundaries` | `not_ignored` |
| `DISC_0A9EB4B9437059E82C8B` | `rust_integration_test` | `crates/trust-syntax/tests/parser_variables.rs` | `test_var_at_address` | `not_ignored` |
| `DISC_9B43339BCCE6BCB56DD5` | `rust_integration_test` | `crates/trust-syntax/tests/parser_variables.rs` | `test_var_at_wildcard_address` | `not_ignored` |
| `DISC_B89969D2FDCA98F4AE82` | `rust_integration_test` | `crates/trust-syntax/tests/parser_variables.rs` | `test_var_block_types` | `not_ignored` |
| `DISC_F9EBDCFECF196E95F8F7` | `rust_integration_test` | `crates/trust-syntax/tests/parser_variables.rs` | `test_var_global_aggregate_initializer_parse` | `not_ignored` |
| `DISC_8ED4AABC4BD8CC5CCE12` | `rust_integration_test` | `crates/trust-syntax/tests/parser_variables.rs` | `test_var_initializer_aggregate_shapes_and_recovery` | `not_ignored` |
| `DISC_76D56500266DDABE87F7` | `rust_integration_test` | `crates/trust-syntax/tests/parser_variables.rs` | `test_var_modifiers` | `not_ignored` |
| `DISC_F68CDE0188626C53A2F6` | `rust_integration_test` | `crates/trust-syntax/tests/parser_variables.rs` | `test_var_with_array_initializer` | `not_ignored` |
| `DISC_A667418E5CB415D18B4A` | `rust_integration_test` | `crates/trust-syntax/tests/parser_variables.rs` | `test_var_with_initializer` | `not_ignored` |
| `DISC_B697B33479C2C3CD7B91` | `rust_integration_test` | `crates/trust-syntax/tests/parser_variables.rs` | `test_var_with_named_aggregate_initializer` | `not_ignored` |
| `DISC_C2687A2FEF31E4256BAA` | `rust_integration_test` | `crates/trust-syntax/tests/parser_variables.rs` | `test_var_with_partial_array_initializer` | `not_ignored` |
| `DISC_305267835695FA492638` | `rust_integration_test` | `crates/trust-syntax/tests/parser_variables.rs` | `test_var_with_repetition_array_initializer` | `not_ignored` |
| `DISC_C95ABDE21A1B821DAE52` | `rust_integration_test` | `crates/trust-wasm-analysis/tests/mp010_parity/mp010_parity_part_01.rs` | `completion_for_statement_prefixes_exposes_program_variables` | `not_ignored` |
| `DISC_E989A5E3C6EEBA05C959` | `rust_integration_test` | `crates/trust-wasm-analysis/tests/mp010_parity/mp010_parity_part_01.rs` | `completion_for_struct_member_access_returns_expected_members` | `not_ignored` |
| `DISC_F4BFE0A1FF6217DDE594` | `rust_integration_test` | `crates/trust-wasm-analysis/tests/mp010_parity/mp010_parity_part_01.rs` | `diagnostics_parity_matches_native_analysis` | `not_ignored` |
| `DISC_69339D4C4A34D9C89FA5` | `rust_integration_test` | `crates/trust-wasm-analysis/tests/mp010_parity/mp010_parity_part_01.rs` | `hover_and_completion_parity_matches_native_analysis` | `not_ignored` |
| `DISC_444644407843BD3E78CF` | `rust_integration_test` | `crates/trust-wasm-analysis/tests/mp010_parity/mp010_parity_part_01.rs` | `hover_function_block_signature_in_wasm_uses_declared_types` | `not_ignored` |
| `DISC_1B95A902523C7F562253` | `rust_integration_test` | `crates/trust-wasm-analysis/tests/mp010_parity/mp010_parity_part_02.rs` | `definition_references_and_rename_work_with_plain_demo_uris` | `not_ignored` |
| `DISC_99FF235CF921E4EAC7BD` | `rust_integration_test` | `crates/trust-wasm-analysis/tests/mp010_parity/mp010_parity_part_02.rs` | `definition_supports_boundary_cursor_positions_with_plain_demo_uris` | `not_ignored` |
| `DISC_A8F7307EC90531FA4E6E` | `rust_integration_test` | `crates/trust-wasm-analysis/tests/mp010_parity/mp010_parity_part_02.rs` | `references_and_rename_work_with_plain_demo_uris` | `not_ignored` |
| `DISC_5A3DC26ADD3078D93400` | `rust_integration_test` | `crates/trust-wasm-analysis/tests/mp010_parity/mp010_parity_part_03.rs` | `browser_host_smoke_apply_documents_then_diagnostics_round_trip` | `not_ignored` |
| `DISC_26C78820E692F5088A29` | `rust_integration_test` | `crates/trust-wasm-analysis/tests/mp010_parity/mp010_parity_part_03.rs` | `definition_for_pump_controller_type_with_plain_demo_uris_returns_target_uri` | `not_ignored` |
| `DISC_B7BD910E8880AC232E6C` | `rust_integration_test` | `crates/trust-wasm-analysis/tests/mp010_parity/mp010_parity_part_03.rs` | `definition_references_and_rename_accept_punctuation_adjacent_cursor_positions` | `not_ignored` |
| `DISC_7332F28B7116535E5F30` | `rust_integration_test` | `crates/trust-wasm-analysis/tests/mp010_parity/mp010_parity_part_03.rs` | `document_highlight_for_local_symbol_returns_multiple_occurrences` | `not_ignored` |
| `DISC_54137EE63E1773E87DC7` | `rust_integration_test` | `crates/trust-wasm-analysis/tests/mp010_parity/mp010_parity_part_03.rs` | `references_for_program_variable_work_with_plain_demo_uris` | `not_ignored` |
| `DISC_DEC01B9AB53AB9359DC7` | `rust_integration_test` | `crates/trust-wasm-analysis/tests/mp010_parity/mp010_parity_part_03.rs` | `wasm_json_adapter_contract_is_stable` | `not_ignored` |
| `DISC_51CCA149C08CFD61083F` | `rust_integration_test` | `crates/trust-wasm-analysis/tests/mp010_parity/mp010_parity_part_04.rs` | `browser_analysis_latency_budget_against_native_is_within_spike_limits` | `not_ignored` |
| `DISC_56EE618A96CC07A8EE56` | `rust_integration_test` | `crates/trust-wasm-analysis/tests/mp010_parity/mp010_parity_part_04.rs` | `multi_document_incremental_update_flow_handles_realistic_edit_streams` | `not_ignored` |
| `DISC_F7350A6B5413D070B89F` | `rust_integration_test` | `crates/trust-wasm-analysis/tests/mp010_parity/mp010_parity_part_05_part_01.rs` | `representative_corpus_memory_budget_gate` | `not_ignored` |
| `DISC_3737DBBECC6A1F11CE4B` | `rust_unit_test` | `crates/trust-ads-core/src/mapping.rs` | `rejects_shape_type_and_length_mismatches` | `not_ignored` |
| `DISC_E9971665AB644E8D9A59` | `rust_unit_test` | `crates/trust-ads-core/src/mapping.rs` | `scalar_array_mapping_round_trips_dimensions` | `not_ignored` |
| `DISC_96DA4A9A9A08F096DFD3` | `rust_unit_test` | `crates/trust-ads-core/src/mapping.rs` | `scalar_mapping_round_trips_every_supported_type` | `not_ignored` |
| `DISC_480337BF8DA6C4EF9244` | `rust_unit_test` | `crates/trust-ads-core/src/mapping.rs` | `string_mapping_uses_declared_capacity_and_terminator` | `not_ignored` |
| `DISC_51CFC48F4ADD7ADC16CC` | `rust_unit_test` | `crates/trust-ads-core/src/quality.rs` | `cold_start_status_is_stale` | `not_ignored` |
| `DISC_E77A750D7CCA1DF62C00` | `rust_unit_test` | `crates/trust-ads-core/src/quality.rs` | `quality_transitions_clear_and_preserve_fields` | `not_ignored` |
| `DISC_1B897D7D7FAFF9D11C23` | `rust_unit_test` | `crates/trust-ads-core/src/quality.rs` | `stale_at_preserves_last_update_timestamp` | `not_ignored` |
| `DISC_3ACEF1562B2872038FA3` | `rust_unit_test` | `crates/trust-ads-core/src/routing.rs` | `plain_transport_round_trips_as_explicit_policy_data` | `not_ignored` |
| `DISC_4CED1DB1E4217E1FB272` | `rust_unit_test` | `crates/trust-ads-core/src/routing.rs` | `route_security_serializes_reserved_secure_by_default` | `not_ignored` |
| `DISC_C0715309D41FFB6A342D` | `rust_unit_test` | `crates/trust-ads-core/src/symbols.rs` | `imported_point_model_round_trips` | `not_ignored` |
| `DISC_0375AA510F7EFD9916A1` | `rust_unit_test` | `crates/trust-ads-core/src/symbols.rs` | `snapshot_serialization_is_byte_identical_after_reordering` | `not_ignored` |
| `DISC_579B6529D2570159E0DD` | `rust_unit_test` | `crates/trust-ads-core/src/symbols.rs` | `validates_array_byte_size_against_type_descriptor` | `not_ignored` |
| `DISC_1D6E58EA1086E540DC5C` | `rust_unit_test` | `crates/trust-ads-core/src/symbols.rs` | `validates_endpoint_byte_size_against_type_descriptor` | `not_ignored` |
| `DISC_FA2E7ED1E85CF453C1DC` | `rust_unit_test` | `crates/trust-ads-server/src/ams.rs` | `ams_frame_round_trips_valid_read_request_bytes` | `not_ignored` |
| `DISC_E2199B0876F480E769E4` | `rust_unit_test` | `crates/trust-ads-server/src/ams.rs` | `parser_rejects_ams_data_length_mismatch` | `not_ignored` |
| `DISC_092311EA917C2AD0A2D0` | `rust_unit_test` | `crates/trust-ads-server/src/ams.rs` | `parser_rejects_frame_over_configured_cap_before_payload_use` | `not_ignored` |
| `DISC_05D523F0DA7935D57DE2` | `rust_unit_test` | `crates/trust-ads-server/src/ams.rs` | `parser_rejects_invalid_state_flags` | `not_ignored` |
| `DISC_B5A766CA369835DBF015` | `rust_unit_test` | `crates/trust-ads-server/src/ams.rs` | `parser_rejects_length_smaller_than_ams_header` | `not_ignored` |
| `DISC_B6384B6437C6E3D10534` | `rust_unit_test` | `crates/trust-ads-server/src/ams.rs` | `parser_rejects_nonzero_reserved_bytes` | `not_ignored` |
| `DISC_7CFE43849CD4D888527D` | `rust_unit_test` | `crates/trust-ads-server/src/ams.rs` | `parser_rejects_truncated_ams_tcp_header` | `not_ignored` |
| `DISC_5E92A205D4C06F7F9D0D` | `rust_unit_test` | `crates/trust-ads-server/src/ams.rs` | `parser_rejects_truncated_payload` | `not_ignored` |
| `DISC_99FE2332E12E8705D917` | `rust_unit_test` | `crates/trust-ads-server/src/ams.rs` | `parser_rejects_unknown_command_id` | `not_ignored` |
| `DISC_6A52B2B733E82F7C3C0D` | `rust_unit_test` | `crates/trust-ads-server/src/ams.rs` | `response_header_swaps_source_and_target` | `not_ignored` |
| `DISC_835A0E99643AE75A673A` | `rust_unit_test` | `crates/trust-ads-server/src/ams.rs` | `serializer_rejects_header_payload_mismatch` | `not_ignored` |
| `DISC_B83CCC5A096B9CF04367` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `add_notification_accepts_beckhoff_dotnet_v7_compact_request` | `not_ignored` |
| `DISC_236AACDE2C73778B2D82` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `add_notification_accepts_online_change_count_handle_as_symbol_version` | `not_ignored` |
| `DISC_68057C768F3050798092` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `add_notification_accepts_smaller_watch_and_rejects_too_large_watch` | `not_ignored` |
| `DISC_1543E8ECC7FA00971457` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `add_notification_accepts_supported_modes_and_delete_releases_handle` | `not_ignored` |
| `DISC_188A4358A0C5780CB643` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `add_notification_accepts_symbol_value_handle_for_pyads` | `not_ignored` |
| `DISC_112E6BF15D50D3844B08` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `add_notification_accepts_symbol_version_watch` | `not_ignored` |
| `DISC_C05BC7B2A4A9CD3EC008` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `add_notification_accepts_task_count_handle_as_static_system_bytes` | `not_ignored` |
| `DISC_3049C22451F18EA9853D` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `add_notification_enforces_per_client_limit` | `not_ignored` |
| `DISC_3B96E40F70EFE5CEEFD2` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `device_data_time_base_read_matches_twincat_probe` | `not_ignored` |
| `DISC_B9F032F8DD9205102FFF` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `device_notification_payload_matches_wire_matrix` | `not_ignored` |
| `DISC_05A7008CE3458525EDA2` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `direct_read_honors_smaller_requested_length` | `not_ignored` |
| `DISC_3DFFB54EA290BAF6694F` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `direct_read_returns_value_bytes` | `not_ignored` |
| `DISC_C6EC6958532936658358` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `direct_value_read_pads_to_requested_length` | `not_ignored` |
| `DISC_1FF0D99CE336CD8B31E6` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `direct_write_enforces_write_byte_limit` | `not_ignored` |
| `DISC_237501D47D14528C76F2` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `handle_by_name_accepts_symbol_name_without_nul_terminator` | `not_ignored` |
| `DISC_E009F44621E41926DA2C` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `handle_by_name_enforces_handle_limit` | `not_ignored` |
| `DISC_620800BCF1A5430D2BC0` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `handle_by_name_then_read_by_handle` | `not_ignored` |
| `DISC_59F74FFF6348AC0EA2D6` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `handle_by_name_wraps_without_aliasing_live_handles` | `not_ignored` |
| `DISC_E9A8919424A38D658F47` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `invalidated_notification_sample_has_zero_size_data` | `not_ignored` |
| `DISC_94DD82DDCE273D1B8621` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `online_change_count_handle_by_name_reads_symbol_version` | `not_ignored` |
| `DISC_5046CCAF84CBD7CD0F96` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `online_change_count_info_by_name_ex_returns_hidden_system_symbol` | `not_ignored` |
| `DISC_72275022EAF715391AAA` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `raw_task_info_base_address_reads_full_task_info_block` | `not_ignored` |
| `DISC_FBD17B9DC28F329693F4` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `raw_task_info_base_address_reads_obj_id_prefix_for_short_read` | `not_ignored` |
| `DISC_8A60A08AE62583C6969B` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `read_device_info_response_is_wire_shaped` | `not_ignored` |
| `DISC_6AB49FC0F33229379814` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `release_handle_accepts_payload_handle_form` | `not_ignored` |
| `DISC_0FEDA11BB46AD6F25B25` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `release_handle_accepts_twincat_index_offset_form` | `not_ignored` |
| `DISC_4D7E4AAFB7A3739C0AF8` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `sumup_read_enforces_item_limit` | `not_ignored` |
| `DISC_6A3280B7A303857F2177` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `sumup_read_ex_returns_result_lengths_for_ads_rs_read_multi` | `not_ignored` |
| `DISC_6D38BDA1CDF25D27E285` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `sumup_read_keeps_good_items_when_one_fails` | `not_ignored` |
| `DISC_91F504B1B2647C4B7BAD` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `sumup_readwrite_returns_metadata_then_concatenated_data` | `not_ignored` |
| `DISC_9F2B16521891F30BA95B` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `sumup_write_returns_per_item_results` | `not_ignored` |
| `DISC_A5B5DCA7705ED9EA81E6` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `symbol_info_by_name_ex_accepts_symbol_name_without_nul_terminator` | `not_ignored` |
| `DISC_0D09CBB2CCDA81EB36D4` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `symbol_info_by_name_ex_returns_ads_symbol_entry_for_pyads_cache` | `not_ignored` |
| `DISC_BDC99D3F319E2CC1FE25` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `symbol_info_by_name_returns_compact_index_and_size_tuple_for_ads_rs` | `not_ignored` |
| `DISC_E96E000877DE707F29BB` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `symbol_value_by_name_pads_value_to_requested_length` | `not_ignored` |
| `DISC_25792CEDF38CF4C2BDA9` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `symbol_value_by_name_returns_task_count_without_nul_terminator` | `not_ignored` |
| `DISC_C52F1508263EF214C1B0` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `symbol_version_read_returns_u32_version` | `not_ignored` |
| `DISC_2D35C79AFD53FCD4A7C1` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `task_info_ads_port_field_reports_runtime_port` | `not_ignored` |
| `DISC_0CA1398DEC1CF1C88711` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `task_info_block_by_name_returns_full_struct_bytes` | `not_ignored` |
| `DISC_2A66BFE1789CFD2F1B2C` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `upload_info2_pads_to_requested_length_for_ads_rs` | `not_ignored` |
| `DISC_0294E20BA84EA9ADD572` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `upload_info2_reports_symbol_and_datatype_table_sizes` | `not_ignored` |
| `DISC_0F09708F6C2B653947B9` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `write_enqueues_and_audits_writable_symbol` | `not_ignored` |
| `DISC_19C536453F9F4C1E4C07` | `rust_unit_test` | `crates/trust-ads-server/src/commands/tests.rs` | `write_rejects_readonly_symbol_without_runtime_mutation` | `not_ignored` |
| `DISC_B49A0E320309773224CF` | `rust_unit_test` | `crates/trust-ads-server/src/error.rs` | `service_not_supported_uses_beckhoff_srvnotsupp_value` | `not_ignored` |
| `DISC_39F8B45C2D735347C01B` | `rust_unit_test` | `crates/trust-ads-server/src/identify.rs` | `identify_responder_replies_with_runtime_identity_tags` | `not_ignored` |
| `DISC_9FA7CF293A0795E86309` | `rust_unit_test` | `crates/trust-ads-server/src/identify.rs` | `route_add_responder_acks_tcat_route_workflow_without_granting_access` | `not_ignored` |
| `DISC_CD96125A074599D0AF34` | `rust_unit_test` | `crates/trust-ads-server/src/lib.rs` | `audit_event_records_policy_failures_without_runtime_types` | `not_ignored` |
| `DISC_EC49721E7051C47358F2` | `rust_unit_test` | `crates/trust-ads-server/src/lib.rs` | `boundary_traits_use_core_types_and_raw_bytes` | `not_ignored` |
| `DISC_0A97A0F0172B859013CF` | `rust_unit_test` | `crates/trust-ads-server/src/listener.rs` | `ams_net_id_text_conversion_round_trips` | `not_ignored` |
| `DISC_22C7C52311DB7DA1A9AD` | `rust_unit_test` | `crates/trust-ads-server/src/listener.rs` | `tcp_listener_delivers_server_cycle_notification` | `not_ignored` |
| `DISC_A400A45ABBB0D7CCE20A` | `rust_unit_test` | `crates/trust-ads-server/src/listener.rs` | `tcp_listener_dispatches_direct_read` | `not_ignored` |
| `DISC_0CD4F6E675C29CF345CC` | `rust_unit_test` | `crates/trust-ads-server/src/listener.rs` | `tcp_listener_enforces_notification_limits` | `not_ignored` |
| `DISC_DAEE09612892723F2F46` | `rust_unit_test` | `crates/trust-ads-server/src/listener.rs` | `tcp_listener_notifications_keep_registering_ads_port_on_multiplexed_connection` | `not_ignored` |
| `DISC_0376748F4EDA16DC094A` | `rust_unit_test` | `crates/trust-ads-server/src/listener.rs` | `tcp_listener_rejects_symbol_read_on_system_service_port` | `not_ignored` |
| `DISC_37BA40215DC8CA1E63B2` | `rust_unit_test` | `crates/trust-ads-server/src/listener.rs` | `tcp_listener_rejects_unknown_router_metadata_offset` | `not_ignored` |
| `DISC_73429B49E1D0F67890B6` | `rust_unit_test` | `crates/trust-ads-server/src/listener.rs` | `tcp_listener_rejects_wrong_target_port` | `not_ignored` |
| `DISC_0A2D8A3961E27560BE4C` | `rust_unit_test` | `crates/trust-ads-server/src/listener.rs` | `tcp_listener_reports_bind_conflict` | `not_ignored` |
| `DISC_6F9473DD0E46FD58D708` | `rust_unit_test` | `crates/trust-ads-server/src/listener.rs` | `tcp_listener_serves_read_device_info` | `not_ignored` |
| `DISC_DE976A1A8F7E4DBB441D` | `rust_unit_test` | `crates/trust-ads-server/src/listener.rs` | `tcp_listener_serves_router_device_info` | `not_ignored` |
| `DISC_7BCD3AD6E26B21A5E0C0` | `rust_unit_test` | `crates/trust-ads-server/src/listener.rs` | `tcp_listener_serves_router_metadata_read` | `not_ignored` |
| `DISC_B16F94677813FF77F579` | `rust_unit_test` | `crates/trust-ads-server/src/listener.rs` | `tcp_listener_serves_router_tcpip_metadata_read` | `not_ignored` |
| `DISC_5C29A38157E82A53D660` | `rust_unit_test` | `crates/trust-ads-server/src/listener.rs` | `tcp_listener_serves_router_tcpip_metadata_table_with_runtime_port` | `not_ignored` |
| `DISC_F30BDFE825C2777A6CE7` | `rust_unit_test` | `crates/trust-ads-server/src/listener.rs` | `tcp_listener_serves_system_service_read_state` | `not_ignored` |
| `DISC_2A1785CA9EA684A568E5` | `rust_unit_test` | `crates/trust-ads-server/src/listener.rs` | `tcp_listener_serves_tcom_browser_probe` | `not_ignored` |
| `DISC_C583B205C64331F8787B` | `rust_unit_test` | `crates/trust-ads-server/src/notify.rs` | `filetime_conversion_matches_unix_epoch_offset` | `not_ignored` |
| `DISC_99D4D4DE65F8AB0DEBD5` | `rust_unit_test` | `crates/trust-ads-server/src/notify.rs` | `read_error_invalidates_handle` | `not_ignored` |
| `DISC_B1D362169562C6D4070C` | `rust_unit_test` | `crates/trust-ads-server/src/notify.rs` | `sampler_slices_values_to_registered_watch_length` | `not_ignored` |
| `DISC_E34696B5EF594D3633AB` | `rust_unit_test` | `crates/trust-ads-server/src/notify.rs` | `server_cycle_emits_every_due_tick` | `not_ignored` |
| `DISC_0831D05B72B23BD0AAF9` | `rust_unit_test` | `crates/trust-ads-server/src/notify.rs` | `server_on_change_coalesces_equal_values` | `not_ignored` |
| `DISC_8D17CD3D42D86B01457C` | `rust_unit_test` | `crates/trust-ads-server/src/notify.rs` | `symbol_version_notification_samples_symbol_source_version` | `not_ignored` |
| `DISC_CFE38C649F984DC9993E` | `rust_unit_test` | `crates/trust-ads-server/src/notify.rs` | `system_bytes_notification_samples_without_runtime_read` | `not_ignored` |
| `DISC_8F09CAC7CAE02F675C72` | `rust_unit_test` | `crates/trust-ads-server/src/symbols.rs` | `deterministic_snapshot_json_is_stable` | `not_ignored` |
| `DISC_D50931A36E03795CD198` | `rust_unit_test` | `crates/trust-ads-server/src/symbols.rs` | `duplicate_symbol_names_are_rejected` | `not_ignored` |
| `DISC_6949DB3ADCAEC396777B` | `rust_unit_test` | `crates/trust-ads-server/src/symbols.rs` | `flags_are_preserved_in_assigned_symbols` | `not_ignored` |
| `DISC_E9D51D3A34F9AEAC865A` | `rust_unit_test` | `crates/trust-ads-server/src/symbols.rs` | `symbol_assignment_is_stable_across_input_order` | `not_ignored` |
| `DISC_014676BA92F26374FB1B` | `rust_unit_test` | `crates/trust-ads-server/src/symbols.rs` | `symbol_version_bumps_only_when_layout_changes` | `not_ignored` |
| `DISC_0FE21931EF5D71E9C8F4` | `rust_unit_test` | `crates/trust-debug/src/adapter/stop.rs` | `breakpoint_stop_is_dropped_when_generation_mismatches` | `not_ignored` |
| `DISC_CD493AEA579984A9D671` | `rust_unit_test` | `crates/trust-debug/src/adapter/stop.rs` | `breakpoint_stop_is_emitted_without_pause_expected_when_generation_matches` | `not_ignored` |
| `DISC_2C565AD9DB6A270EB008` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_01.rs` | `dispatch_breakpoint_locations_returns_statement_starts` | `not_ignored` |
| `DISC_533209B69D00C10C27D8` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_01.rs` | `dispatch_io_state_emits_event` | `not_ignored` |
| `DISC_C3B10C3CC7C696D73965` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_01.rs` | `dispatch_io_write_accepts_configured_real_and_time_values` | `not_ignored` |
| `DISC_2BB8A4884F60DC195108` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_01.rs` | `dispatch_io_write_updates_input` | `not_ignored` |
| `DISC_6E029464243111D82B64` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_01.rs` | `dispatch_set_breakpoints_in_if_block_targets_inner_stmt` | `not_ignored` |
| `DISC_E557C58A57187D2B19A4` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_01.rs` | `dispatch_set_breakpoints_returns_adjusted_positions` | `not_ignored` |
| `DISC_0CCCBD1333093016A183` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_01.rs` | `stdio_roundtrip` | `not_ignored` |
| `DISC_19F254231ED02F7830F7` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_02.rs` | `attach_set_expression_forwards_remote_io_force_and_release` | `not_ignored` |
| `DISC_FC6FA54BA863AEA3F13C` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_02.rs` | `attach_st_io_force_and_release_forward_remote_io_force_and_release` | `not_ignored` |
| `DISC_15E90DB37D230716C87C` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_02.rs` | `debug_control_server_serves_hmi_schema_and_values` | `not_ignored` |
| `DISC_8A5B38193F3984D765B5` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_02.rs` | `debug_control_server_uses_launch_project_root_for_comm_apply` | `not_ignored` |
| `DISC_0109D9626BFB2A78BE9E` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_02.rs` | `dispatch_initialize_emits_initialized_event` | `not_ignored` |
| `DISC_62E10889DF37D4356FDE` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_02.rs` | `dispatch_launch_does_not_emit_initialized_event_without_initialize` | `not_ignored` |
| `DISC_D2E77A9B751E229EBCCE` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_02.rs` | `dispatch_run_controls_update_debug_mode` | `not_ignored` |
| `DISC_4BE9B46DC6CFA5C3B12D` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_02.rs` | `dispatch_set_expression_force_supports_direct_instance_field_live` | `not_ignored` |
| `DISC_A2C9D66B36B454724868` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_02.rs` | `dispatch_set_expression_force_supports_direct_instance_field_paused` | `not_ignored` |
| `DISC_4FC64DD2D519F3527474` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_02.rs` | `dispatch_set_expression_force_supports_output_and_memory_io` | `not_ignored` |
| `DISC_B31BC0A4A82144E40B10` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_02.rs` | `dispatch_set_expression_write_once_rejects_output_io` | `not_ignored` |
| `DISC_BB52325726A7D14EB420` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_02.rs` | `launch_fails_when_control_server_endpoint_is_already_in_use` | `not_ignored` |
| `DISC_C34D35743634CEF66F48` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_02.rs` | `launch_io_state_includes_configured_source_provenance` | `not_ignored` |
| `DISC_5331E1A4C032DB8D4969` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_03.rs` | `debug_runner_respects_task_interval_pacing` | `not_ignored` |
| `DISC_D19DDDC8E91E94B488BE` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_03.rs` | `dispatch_continue_then_immediate_pause_emits_pause_stop` | `not_ignored` |
| `DISC_7F88A4693605D97548E2` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_03.rs` | `dispatch_pause_falls_back_to_global_when_no_active_thread` | `not_ignored` |
| `DISC_692E1F8CB32389F17180` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_03.rs` | `dispatch_threads_maps_tasks` | `not_ignored` |
| `DISC_CA0A1F4641F085475FA1` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_04.rs` | `dap_breakpoint_stops_and_resumes_with_task_order` | `not_ignored` |
| `DISC_E86D5C43EF1D596363AA` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_04.rs` | `reload_while_runner_active_does_not_emit_pre_scan_io_state` | `not_ignored` |
| `DISC_DD128012730C8C7C3673` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_04.rs` | `reload_while_runner_active_reports_coherent_conveyor_io_state` | `not_ignored` |
| `DISC_2B916048F2A0AE4C1649` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_05.rs` | `dispatch_threads_stack_scopes_variables` | `not_ignored` |
| `DISC_6B1D94FCC2E1824BC420` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_05.rs` | `io_state_then_stack_trace_do_not_block_when_runtime_mutex_is_held` | `not_ignored` |
| `DISC_B36669CB797EB957D551` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_05.rs` | `scopes_for_synthetic_main_frame_are_graceful` | `not_ignored` |
| `DISC_AA3B37B9D96BAA1AC34E` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_05.rs` | `stack_trace_falls_back_to_main_frame_when_no_storage_frames_exist` | `not_ignored` |
| `DISC_341AA2E5567E5648E91E` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_05.rs` | `stack_trace_falls_back_to_main_frame_when_thread_id_mismatches` | `not_ignored` |
| `DISC_9A49DA74FE0074A96269` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_05.rs` | `stack_trace_returns_default_main_frame_without_location` | `not_ignored` |
| `DISC_F409829BD04C24BC2621` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_06.rs` | `dispatch_evaluate_allows_pure_stdlib_calls` | `not_ignored` |
| `DISC_88DB109C895639DD3D9D` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_06.rs` | `dispatch_evaluate_honors_using_for_types` | `not_ignored` |
| `DISC_053202EA0C2A12E5392F` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_06.rs` | `dispatch_evaluate_rejects_calls` | `not_ignored` |
| `DISC_1055BCACCD9C778708C0` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_06.rs` | `dispatch_evaluate_resolves_instance_and_retain` | `not_ignored` |
| `DISC_A6096264A6A03F467D35` | `rust_unit_test` | `crates/trust-debug/src/adapter/tests_part_06.rs` | `dispatch_evaluate_returns_value` | `not_ignored` |
| `DISC_7D3ABA3654CF165E489A` | `rust_unit_test` | `crates/trust-debug/src/adapter/variables/format.rs` | `format_value_uses_user_facing_primitive_strings` | `not_ignored` |
| `DISC_40BA8F3408FA931A8236` | `rust_unit_test` | `crates/trust-debug/src/protocol.rs` | `response_uses_request_seq_field` | `not_ignored` |
| `DISC_F2B2253F051B54913D6D` | `rust_unit_test` | `crates/trust-debug/src/session/tests.rs` | `expands_brace_globs` | `not_ignored` |
| `DISC_7658FF45C8266D90B633` | `rust_unit_test` | `crates/trust-debug/src/session/tests.rs` | `expands_nested_braces` | `not_ignored` |
| `DISC_33C7071BB09B72378528` | `rust_unit_test` | `crates/trust-debug/src/session/tests.rs` | `parse_hit_condition_supports_basic_operators` | `not_ignored` |
| `DISC_C1698C7AFE06D3D21CCE` | `rust_unit_test` | `crates/trust-debug/src/session/tests.rs` | `session_accepts_logpoint_templates` | `not_ignored` |
| `DISC_C728AA4BAF25A1ED9D15` | `rust_unit_test` | `crates/trust-debug/src/session/tests.rs` | `session_rejects_invalid_log_message` | `not_ignored` |
| `DISC_E456322AE90A1745420C` | `rust_unit_test` | `crates/trust-debug/src/session/tests.rs` | `session_reload_applies_project_io_toml_drivers` | `not_ignored` |
| `DISC_A518F832B954B2EAACDC` | `rust_unit_test` | `crates/trust-debug/src/session/tests.rs` | `session_reload_clears_breakpoints_without_requests` | `not_ignored` |
| `DISC_1D5C39608522BAB060C6` | `rust_unit_test` | `crates/trust-debug/src/session/tests.rs` | `session_reload_revalidates_breakpoints` | `not_ignored` |
| `DISC_5084C25C82C2BA11131A` | `rust_unit_test` | `crates/trust-debug/src/session/tests.rs` | `session_reload_validates_project_ads_toml_bindings` | `not_ignored` |
| `DISC_2ABF394F32784AF2F01F` | `rust_unit_test` | `crates/trust-debug/src/session/tests.rs` | `session_resolves_breakpoints_to_statement_start` | `not_ignored` |
| `DISC_947B741926E9DFCEDD23` | `rust_unit_test` | `crates/trust-debug/src/session/tests.rs` | `session_resolves_if_header_breakpoint_to_if_statement` | `not_ignored` |
| `DISC_28E0524003CC8D14B2C6` | `rust_unit_test` | `crates/trust-debug/src/session/tests.rs` | `session_revalidates_breakpoints_after_source_registration` | `not_ignored` |
| `DISC_0D5CD2CD5C500A4B47E5` | `rust_unit_test` | `crates/trust-debug/src/session/tests.rs` | `session_snaps_breakpoints_inside_indent` | `not_ignored` |
| `DISC_445E9FB214F60DDD3057` | `rust_unit_test` | `crates/trust-debug/src/session/tests.rs` | `source_display_name_is_project_relative_but_path_stays_absolute` | `not_ignored` |
| `DISC_651A49CDA6DA6C0F24CB` | `rust_unit_test` | `crates/trust-dev/src/agent.rs` | `decode_json_value_supports_typed_scalars` | `not_ignored` |
| `DISC_7F8703362219E8B48F52` | `rust_unit_test` | `crates/trust-dev/src/agent.rs` | `encode_value_emits_typed_payload` | `not_ignored` |
| `DISC_42675B2664AA5984F2BC` | `rust_unit_test` | `crates/trust-dev/src/agent.rs` | `normalize_workspace_path_collapses_current_dir_segments` | `not_ignored` |
| `DISC_0F6E676B1604DEA82BD7` | `rust_unit_test` | `crates/trust-dev/src/agent.rs` | `normalize_workspace_path_rejects_parent_escape` | `not_ignored` |
| `DISC_0F53FF30DE58125172CA` | `rust_unit_test` | `crates/trust-dev/src/ci.rs` | `classify_build_failure_code` | `not_ignored` |
| `DISC_F7ADB2E6CC8FFFDEC287` | `rust_unit_test` | `crates/trust-dev/src/ci.rs` | `classify_internal_code` | `not_ignored` |
| `DISC_AE3B2CE73EA47E47BBD6` | `rust_unit_test` | `crates/trust-dev/src/ci.rs` | `classify_invalid_config_code` | `not_ignored` |
| `DISC_4E668027AD73DFAD5522` | `rust_unit_test` | `crates/trust-dev/src/ci.rs` | `classify_test_failure_code` | `not_ignored` |
| `DISC_BC0A3822943D6722F48D` | `rust_unit_test` | `crates/trust-dev/src/ci.rs` | `classify_timeout_code` | `not_ignored` |
| `DISC_FB45A22AC9F9C736ED98` | `rust_unit_test` | `crates/trust-dev/src/ci.rs` | `classify_with_command_falls_back_for_internal` | `not_ignored` |
| `DISC_DFEEA0B862E8D1A4B44B` | `rust_unit_test` | `crates/trust-dev/src/commit.rs` | `commit_rejects_pre_staged_path_inside_project_without_mutation` | `not_ignored` |
| `DISC_DDA759DC285FFED0BB01` | `rust_unit_test` | `crates/trust-dev/src/commit.rs` | `commit_scopes_commit_to_project_path_without_sweeping_pre_staged_files` | `not_ignored` |
| `DISC_9921E8E6B44B04E436F6` | `rust_unit_test` | `crates/trust-dev/src/commit.rs` | `dry_run_with_pre_staged_collision_reports_without_mutation` | `not_ignored` |
| `DISC_6769CA7C879039E59098` | `rust_unit_test` | `crates/trust-dev/src/commit.rs` | `git_status_accepts_non_utf8_project_path_without_panic` | `not_ignored` |
| `DISC_5CAA8F58E6FE55E25F56` | `rust_unit_test` | `crates/trust-dev/src/commit.rs` | `git_status_decodes_quoted_porcelain_paths_for_summary` | `not_ignored` |
| `DISC_8ECA38A3365D2E70B739` | `rust_unit_test` | `crates/trust-dev/src/commit.rs` | `repository_root_commit_rejects_any_pre_staged_path` | `not_ignored` |
| `DISC_AAFDB90C094ED2DF65C1` | `rust_unit_test` | `crates/trust-dev/src/docs/tests.rs` | `broken_tag_diagnostics_are_reported` | `not_ignored` |
| `DISC_952225E5AAF2BB2865EB` | `rust_unit_test` | `crates/trust-dev/src/docs/tests.rs` | `html_output_snapshot` | `not_ignored` |
| `DISC_A23C24FDCE070D769396` | `rust_unit_test` | `crates/trust-dev/src/docs/tests.rs` | `markdown_output_snapshot` | `not_ignored` |
| `DISC_27A271222EA4416B6300` | `rust_unit_test` | `crates/trust-dev/src/docs/tests.rs` | `parser_extraction_for_tagged_comments` | `not_ignored` |
| `DISC_30935290BBA229A21A1C` | `rust_unit_test` | `crates/trust-dev/src/test_cmd/tests.rs` | `ci_mode_defaults_human_output_to_junit` | `not_ignored` |
| `DISC_EAD77AB30DBE5CC32785` | `rust_unit_test` | `crates/trust-dev/src/test_cmd/tests.rs` | `discovery_finds_test_pous_with_namespace_qualification` | `not_ignored` |
| `DISC_9DA43D9B2EA2A3411356` | `rust_unit_test` | `crates/trust-dev/src/test_cmd/tests.rs` | `discovery_ignores_comments_after_test_name` | `not_ignored` |
| `DISC_60C521F0B9A55EB09F58` | `rust_unit_test` | `crates/trust-dev/src/test_cmd/tests.rs` | `execute_test_case_keeps_unconfigured_test_program_out_of_default_runtime` | `not_ignored` |
| `DISC_3C712A9292ED0E143765` | `rust_unit_test` | `crates/trust-dev/src/test_cmd/tests.rs` | `execute_test_case_returns_execution_timeout_for_deadline_overrun` | `not_ignored` |
| `DISC_6B3FF05ABB6C948EE468` | `rust_unit_test` | `crates/trust-dev/src/test_cmd/tests.rs` | `execute_test_case_runs_test_program_when_session_registers_extra_program_instance` | `not_ignored` |
| `DISC_ACAFF4C2AD9CAB8BCD04` | `rust_unit_test` | `crates/trust-dev/src/test_cmd/tests.rs` | `execution_isolated_per_test_case` | `not_ignored` |
| `DISC_9AA7DC431E1144E0A7E4` | `rust_unit_test` | `crates/trust-dev/src/test_cmd/tests.rs` | `execution_reports_assertion_failure_for_test_program` | `not_ignored` |
| `DISC_123A365A92C6D2454A3C` | `rust_unit_test` | `crates/trust-dev/src/test_cmd/tests.rs` | `execution_runs_test_function_block` | `not_ignored` |
| `DISC_703DCE897FB1F94D0882` | `rust_unit_test` | `crates/trust-dev/src/test_cmd/tests.rs` | `human_output_filter_zero_message_is_clear` | `not_ignored` |
| `DISC_50BC83EE8F3F488A7088` | `rust_unit_test` | `crates/trust-dev/src/test_cmd/tests.rs` | `human_output_shows_failure_summary_with_source_context` | `not_ignored` |
| `DISC_BF500DA3AFCE3D874EB3` | `rust_unit_test` | `crates/trust-dev/src/test_cmd/tests.rs` | `json_output_contract` | `not_ignored` |
| `DISC_02C391EA4F1A00481337` | `rust_unit_test` | `crates/trust-dev/src/test_cmd/tests.rs` | `junit_output_contract` | `not_ignored` |
| `DISC_BB40B7FB0D69EC752F0E` | `rust_unit_test` | `crates/trust-dev/src/test_cmd/tests.rs` | `list_output_contract` | `not_ignored` |
| `DISC_024034593826EFD0DC25` | `rust_unit_test` | `crates/trust-dev/src/test_cmd/tests.rs` | `load_sources_finds_mixed_case_extensions_under_literal_glob_chars` | `not_ignored` |
| `DISC_EBA64FF9E66D25570E07` | `rust_unit_test` | `crates/trust-dev/src/test_cmd/tests.rs` | `prepared_runtime_cold_restarts_between_cases` | `not_ignored` |
| `DISC_67620B4E7A1553062EB3` | `rust_unit_test` | `crates/trust-dev/src/test_cmd/tests.rs` | `run_test_executes_test_program_when_configuration_is_present` | `not_ignored` |
| `DISC_649A189BA85E9C36D6E7` | `rust_unit_test` | `crates/trust-dev/src/test_cmd/tests.rs` | `tap_output_contract` | `not_ignored` |
| `DISC_00613F30CB0263C50E35` | `rust_unit_test` | `crates/trust-dev/src/test_cmd/tests.rs` | `timeout_message_pluralization` | `not_ignored` |
| `DISC_0B8FF43D4A58EAAA3781` | `rust_unit_test` | `crates/trust-hir/src/db/diagnostics/context.rs` | `action_context_classifies_missing_owner` | `not_ignored` |
| `DISC_44EAB88F41330B778E1A` | `rust_unit_test` | `crates/trust-hir/src/db/diagnostics/context.rs` | `expression_context_classifies_missing_pou_owner` | `not_ignored` |
| `DISC_051E7C6E43B61B5CABA5` | `rust_unit_test` | `crates/trust-hir/src/db/diagnostics/context.rs` | `pou_context_classifies_missing_name` | `not_ignored` |
| `DISC_8F13FB42BD277C8728D6` | `rust_unit_test` | `crates/trust-hir/src/db/diagnostics/context.rs` | `pou_context_classifies_missing_owner_scope` | `not_ignored` |
| `DISC_B9D584EC48ADE325F027` | `rust_unit_test` | `crates/trust-hir/src/db/diagnostics/context.rs` | `pou_context_classifies_missing_owner_symbol` | `not_ignored` |
| `DISC_D76E451E4EE9B8FDDE1E` | `rust_unit_test` | `crates/trust-hir/src/db/diagnostics/context.rs` | `pou_context_resolves_function_return_type` | `not_ignored` |
| `DISC_0C246CDC2FDB4C8D9D9B` | `rust_unit_test` | `crates/trust-hir/src/db/diagnostics/context.rs` | `pou_context_resolves_method_return_type` | `not_ignored` |
| `DISC_39A22200124D3A06E0E6` | `rust_unit_test` | `crates/trust-hir/src/db/diagnostics/context.rs` | `type_resolution_outcome_classifies_wrong_kind` | `not_ignored` |
| `DISC_30016D0562D6DF5A1745` | `rust_unit_test` | `crates/trust-hir/src/db/diagnostics/oop/mod.rs` | `extends_resolution_does_not_fallback_to_global_when_owner_scope_is_missing` | `not_ignored` |
| `DISC_06F0D6A24B4385C7480B` | `rust_unit_test` | `crates/trust-hir/src/db/diagnostics/oop/mod.rs` | `function_block_cycle_detection_walks_mixed_class_links` | `not_ignored` |
| `DISC_E8D7CC22D0BB37566B15` | `rust_unit_test` | `crates/trust-hir/src/db/diagnostics/tests.rs` | `test_database_basic` | `not_ignored` |
| `DISC_AD62EB9709721F5352E3` | `rust_unit_test` | `crates/trust-hir/src/db/diagnostics/tests.rs` | `test_expr_id_type_of` | `not_ignored` |
| `DISC_8946B1A35AC7692574C4` | `rust_unit_test` | `crates/trust-hir/src/db/diagnostics/tests.rs` | `test_expr_id_type_of_based_literal` | `not_ignored` |
| `DISC_F418E6116BFA348F8D2A` | `rust_unit_test` | `crates/trust-hir/src/db/diagnostics/tests.rs` | `test_type_of_cache_invalidates_on_change` | `not_ignored` |
| `DISC_3E8F6F733694567C072D` | `rust_unit_test` | `crates/trust-hir/src/db/diagnostics/type_check.rs` | `type_check_reports_missing_pou_owner_instead_of_global_fallback` | `not_ignored` |
| `DISC_3B73575D7AF4F2A0AE79` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_01.rs` | `analyze_salsa_accepts_cross_file_global_constants_in_string_lengths` | `not_ignored` |
| `DISC_8E3B875CAED1159162CE` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_01.rs` | `analyze_salsa_accepts_cross_file_root_global_struct_field_access` | `not_ignored` |
| `DISC_2AC49BE8DFBF12438277` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_01.rs` | `analyze_salsa_accepts_namespaced_using_cross_file_root_global_struct_field_access` | `not_ignored` |
| `DISC_52E9612D2351EC1B734A` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_01.rs` | `analyze_salsa_cross_file_struct_types_support_member_access_inside_pou_bodies` | `not_ignored` |
| `DISC_6F51ED05AB2DFF692BC6` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_01.rs` | `analyze_salsa_imported_root_type_names_do_not_collide_with_local_variables` | `not_ignored` |
| `DISC_B2F699B2233921406F8D` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_01.rs` | `analyze_salsa_keeps_cross_file_function_block_body_bound_to_real_pou_scope` | `not_ignored` |
| `DISC_A3D8113CC8D54279E113` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_01.rs` | `analyze_salsa_recomputes_after_target_edit` | `not_ignored` |
| `DISC_5F25B5249C37A9B6B026` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_01.rs` | `analyze_salsa_returns_expected_cross_file_result` | `not_ignored` |
| `DISC_E9DA8651E25B65355F9A` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_01.rs` | `analyze_salsa_reuses_result_without_edits` | `not_ignored` |
| `DISC_CEDB547DF99CE96704C8` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_01.rs` | `diagnostics_salsa_recomputes_after_target_edit` | `not_ignored` |
| `DISC_6ED255B85FA70D60E5E0` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_01.rs` | `diagnostics_salsa_reports_duplicate_cross_file_type_declarations` | `not_ignored` |
| `DISC_9777E107974E583810C8` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_01.rs` | `diagnostics_salsa_reuses_result_without_edits` | `not_ignored` |
| `DISC_3D10347AB70DF279D3DF` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_01.rs` | `file_symbols_attach_cross_file_root_global_struct_type_during_collection` | `not_ignored` |
| `DISC_4E7BBD81CF41695D86E4` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_01.rs` | `file_symbols_recomputes_when_its_file_changes` | `not_ignored` |
| `DISC_B7BE80E8A226361217E5` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_01.rs` | `file_symbols_reuses_unchanged_file_across_unrelated_edit` | `not_ignored` |
| `DISC_92FB8CF2319C24B288B0` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_02.rs` | `analyze_syncs_stale_salsa_state_revision` | `not_ignored` |
| `DISC_F12F27493E238BDC078F` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_02.rs` | `remove_and_readd_source_restores_cross_file_resolution` | `not_ignored` |
| `DISC_5318121FE410058BD406` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_02.rs` | `remove_missing_source_keeps_source_revision` | `not_ignored` |
| `DISC_5742DB20C063BA3291DE` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_02.rs` | `remove_source_text_clears_single_file_queries` | `not_ignored` |
| `DISC_A78C49246D0A90AA6D83` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_02.rs` | `remove_source_text_invalidates_cross_file_dependency` | `not_ignored` |
| `DISC_4545BD0A979DC02532F3` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_02.rs` | `set_source_text_existing_file_skips_project_input_resync` | `not_ignored` |
| `DISC_89C7287D5AC65F94277E` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_02.rs` | `set_source_text_same_content_keeps_source_revision` | `not_ignored` |
| `DISC_CDEEBCE8C0C7A1C5EF3D` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_02.rs` | `source_text_and_symbols_stay_consistent_after_edit` | `not_ignored` |
| `DISC_178CBF421792D48245C3` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_02.rs` | `type_of_salsa_recomputes_after_dependency_edit` | `not_ignored` |
| `DISC_6D8B56B30030FFE511AD` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_02.rs` | `type_of_salsa_returns_expected_type_for_cross_file_call` | `not_ignored` |
| `DISC_4B5DB49410AED7C18AE0` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_02.rs` | `type_of_salsa_stable_across_unrelated_edit` | `not_ignored` |
| `DISC_99ADCDF761C2A8EDB12E` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_03.rs` | `cancellation_requests_keep_queries_stable` | `not_ignored` |
| `DISC_2AFEB68C8EB2917F1FAB` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_03.rs` | `concurrent_edit_and_query_loops_do_not_panic` | `not_ignored` |
| `DISC_18368E8DA185F340EC17` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_03.rs` | `dependent_type_edit_reinvalidates_cross_file_global_struct_analysis` | `not_ignored` |
| `DISC_90429FB4381C04264AD9` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_03.rs` | `expr_id_at_offset_returns_none_for_missing_file` | `not_ignored` |
| `DISC_BB8C17F714B437C1B166` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_03.rs` | `expr_id_at_offset_tracks_updated_source` | `not_ignored` |
| `DISC_54F981BD4197331DB020` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_03.rs` | `project_type_catalog_reuses_result_when_type_preludes_are_unchanged` | `not_ignored` |
| `DISC_20D315283932DCEA6311` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_03.rs` | `query_boundary_sequence_no_longer_panics` | `not_ignored` |
| `DISC_E0E415B7CEFBD76DFB16` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_03.rs` | `salsa_event_counters_emit_query_categories` | `not_ignored` |
| `DISC_910D2F6B494AD553522A` | `rust_unit_test` | `crates/trust-hir/src/db/queries/database/database_tests_part_03.rs` | `semantic_kernel_cross_file_resolution_tracks_lsp_style_dependency_edits` | `not_ignored` |
| `DISC_251A2BD74865B4BDB6A6` | `rust_unit_test` | `crates/trust-hir/src/diagnostics.rs` | `test_diagnostic_builder` | `not_ignored` |
| `DISC_282A522E3AB3D261C0F6` | `rust_unit_test` | `crates/trust-hir/src/diagnostics.rs` | `test_diagnostic_creation` | `not_ignored` |
| `DISC_4551DD37AAD48961CBC9` | `rust_unit_test` | `crates/trust-hir/src/ident.rs` | `test_invalid_identifiers` | `not_ignored` |
| `DISC_2325CC8431152C1A31EB` | `rust_unit_test` | `crates/trust-hir/src/ident.rs` | `test_reserved_keywords` | `not_ignored` |
| `DISC_F92A4D327BBD44F97201` | `rust_unit_test` | `crates/trust-hir/src/ident.rs` | `test_valid_identifiers` | `not_ignored` |
| `DISC_6C2CA96E6F63768B3CD3` | `rust_unit_test` | `crates/trust-hir/src/openot_authoring.rs` | `accepts_canonical_procedural_model_enum_states` | `not_ignored` |
| `DISC_315A9A89911D494BA9B2` | `rust_unit_test` | `crates/trust-hir/src/openot_authoring.rs` | `accepts_documented_values` | `not_ignored` |
| `DISC_FA991ADAC870585849F4` | `rust_unit_test` | `crates/trust-hir/src/openot_authoring.rs` | `rejects_batch_enum_that_is_not_batch_state` | `not_ignored` |
| `DISC_D6E228314F5B17835F49` | `rust_unit_test` | `crates/trust-hir/src/openot_authoring.rs` | `rejects_invalid_audited_value_attributes` | `not_ignored` |
| `DISC_4D4B5D737C5FD8D0D393` | `rust_unit_test` | `crates/trust-hir/src/openot_authoring.rs` | `rejects_invalid_batch_recipe_attributes` | `not_ignored` |
| `DISC_EDD197D4B3BF8911E272` | `rust_unit_test` | `crates/trust-hir/src/openot_authoring.rs` | `rejects_invalid_condition_lifecycle_attributes` | `not_ignored` |
| `DISC_F0ADB9FC9FDC8BD2EE40` | `rust_unit_test` | `crates/trust-hir/src/openot_authoring.rs` | `rejects_invalid_esignature_attributes` | `not_ignored` |
| `DISC_4D6A06DBEDBDE31DFA68` | `rust_unit_test` | `crates/trust-hir/src/openot_authoring.rs` | `rejects_invalid_operator_regulated_attributes` | `not_ignored` |
| `DISC_9C0F92513658AAAD135E` | `rust_unit_test` | `crates/trust-hir/src/openot_authoring.rs` | `rejects_model_without_explicit_procedural_category` | `not_ignored` |
| `DISC_DB66B809E70D159C17C9` | `rust_unit_test` | `crates/trust-hir/src/openot_authoring.rs` | `rejects_procedural_category_without_model` | `not_ignored` |
| `DISC_F45C52725DCACC47B10B` | `rust_unit_test` | `crates/trust-hir/src/openot_authoring.rs` | `rejects_procedural_model_enum_states_that_are_not_canonical` | `not_ignored` |
| `DISC_58DE673EF0C888BADDD3` | `rust_unit_test` | `crates/trust-hir/src/openot_authoring.rs` | `rejects_unknown_message_arg_reference` | `not_ignored` |
| `DISC_F842A5D4BA29AEABEF11` | `rust_unit_test` | `crates/trust-hir/src/openot_authoring.rs` | `rejects_unknown_value_unit` | `not_ignored` |
| `DISC_030A23BFD97B43EC2358` | `rust_unit_test` | `crates/trust-hir/src/openot_authoring.rs` | `rejects_value_attributes_for_unsupported_value_types` | `not_ignored` |
| `DISC_6105B02E0F815A70832D` | `rust_unit_test` | `crates/trust-hir/src/openot_authoring.rs` | `validates_audited_value_references_and_budget` | `not_ignored` |
| `DISC_A14B32D75DC89BAA53C0` | `rust_unit_test` | `crates/trust-hir/src/openot_authoring.rs` | `validates_batch_recipe_references` | `not_ignored` |
| `DISC_EAEBB71090C80FC9A3A4` | `rust_unit_test` | `crates/trust-hir/src/openot_authoring.rs` | `validates_condition_lifecycle_references` | `not_ignored` |
| `DISC_545C10CFBE46B086F294` | `rust_unit_test` | `crates/trust-hir/src/openot_authoring.rs` | `validates_esignature_references` | `not_ignored` |
| `DISC_B7F59B0CD8582D9157D9` | `rust_unit_test` | `crates/trust-hir/src/openot_authoring.rs` | `validates_known_and_unknown_state_category` | `not_ignored` |
| `DISC_8D106C670C6583CD943A` | `rust_unit_test` | `crates/trust-hir/src/openot_authoring.rs` | `validates_message_args_and_alarm_cause_references` | `not_ignored` |
| `DISC_CDE7BF4366619CE64898` | `rust_unit_test` | `crates/trust-hir/src/openot_authoring.rs` | `validates_operator_regulated_references` | `not_ignored` |
| `DISC_AD9D9D8A4F208629E1C7` | `rust_unit_test` | `crates/trust-hir/src/openot_authoring.rs` | `validates_value_sampling_policy_rules` | `not_ignored` |
| `DISC_60F71A306D5D29CF6A1A` | `rust_unit_test` | `crates/trust-hir/src/project.rs` | `insert_with_id_existing_key_returns_existing_id` | `not_ignored` |
| `DISC_B09611EF8DC7A641FF2E` | `rust_unit_test` | `crates/trust-hir/src/project.rs` | `insert_with_id_rejects_file_id_collision` | `not_ignored` |
| `DISC_4919138CAFD3CB9BF960` | `rust_unit_test` | `crates/trust-hir/src/project.rs` | `noncanonical_fallback_path_does_not_collide_with_canonical_path` | `not_ignored` |
| `DISC_628F0DE10BC63E245E80` | `rust_unit_test` | `crates/trust-hir/src/project.rs` | `noncanonical_fallback_path_removes_current_dir_components` | `not_ignored` |
| `DISC_FD5BA075631D52F582EF` | `rust_unit_test` | `crates/trust-hir/src/semantic.rs` | `qualified_name_rejects_empty_and_splits_dotted_names` | `not_ignored` |
| `DISC_3F236472C9146436CC2A` | `rust_unit_test` | `crates/trust-hir/src/semantic.rs` | `semantic_outcome_is_resolved_only_for_resolved_variant` | `not_ignored` |
| `DISC_FA0F9CDA4CB0B2F03825` | `rust_unit_test` | `crates/trust-hir/src/semantic.rs` | `semantic_outcome_map_preserves_non_resolved_classification` | `not_ignored` |
| `DISC_71F14A5F46F90BC06BAC` | `rust_unit_test` | `crates/trust-hir/src/symbols/table.rs` | `alias_resolution_outcome_reports_cycle_explicitly` | `not_ignored` |
| `DISC_3393EA0128B577459A9C` | `rust_unit_test` | `crates/trust-hir/src/symbols/table.rs` | `test_symbol_table` | `not_ignored` |
| `DISC_2749DD3C21DD774C038E` | `rust_unit_test` | `crates/trust-hir/src/symbols/table.rs` | `top_level_symbol_lookup_policy_is_first_writer_for_all_insert_apis` | `not_ignored` |
| `DISC_EF2B71D171F964538552` | `rust_unit_test` | `crates/trust-hir/src/type_check/compatibility.rs` | `missing_array_element_type_identity_is_not_assignment_compatible` | `not_ignored` |
| `DISC_0A678B365C9975C76F0E` | `rust_unit_test` | `crates/trust-hir/src/type_check/compatibility.rs` | `missing_pointer_target_type_identity_is_not_assignment_compatible` | `not_ignored` |
| `DISC_4FA43D135F60B1489A44` | `rust_unit_test` | `crates/trust-hir/src/type_check/mod.rs` | `test_binary_op_from_node` | `not_ignored` |
| `DISC_32B7466F13C5D7E6A5AF` | `rust_unit_test` | `crates/trust-hir/src/types/defs.rs` | `test_type_helpers` | `not_ignored` |
| `DISC_F14EBB9DAE1DC0399077` | `rust_unit_test` | `crates/trust-hir/src/types/registry.rs` | `missing_array_element_type_identity_is_not_assignable` | `not_ignored` |
| `DISC_EDCA384439F6894CF97E` | `rust_unit_test` | `crates/trust-hir/src/types/registry.rs` | `test_type_compatibility` | `not_ignored` |
| `DISC_5B4DD226CBA8B7AFA460` | `rust_unit_test` | `crates/trust-hir/src/types/registry.rs` | `test_type_registry` | `not_ignored` |
| `DISC_F25FDFBDB630D14A9B91` | `rust_unit_test` | `crates/trust-hir/src/types/registry.rs` | `type_name_prefers_canonical_user_type_name` | `not_ignored` |
| `DISC_A1D6CE9F5B88ED95770E` | `rust_unit_test` | `crates/trust-ide/src/call_hierarchy.rs` | `call_hierarchy_outgoing_collects_calls` | `not_ignored` |
| `DISC_109FAAD8AC081F2B6D69` | `rust_unit_test` | `crates/trust-ide/src/call_hierarchy.rs` | `call_hierarchy_respects_allowed_files` | `not_ignored` |
| `DISC_2A09CB773FBDB0CE2348` | `rust_unit_test` | `crates/trust-ide/src/call_hierarchy.rs` | `call_hierarchy_tracks_fb_instance_calls` | `not_ignored` |
| `DISC_73BA3AC8A270FFD74AB3` | `rust_unit_test` | `crates/trust-ide/src/completion/tests.rs` | `test_completion_recovery_in_statement_context_keeps_scope_symbols` | `not_ignored` |
| `DISC_9C8C3EE0F029A7F04554` | `rust_unit_test` | `crates/trust-ide/src/completion/tests.rs` | `test_member_completion_respects_visibility` | `not_ignored` |
| `DISC_04FF5F6842990C88D1DF` | `rust_unit_test` | `crates/trust-ide/src/completion/tests.rs` | `test_parameter_name_completion_in_call` | `not_ignored` |
| `DISC_73FC8CCC097ED8A4C6FE` | `rust_unit_test` | `crates/trust-ide/src/completion/tests.rs` | `test_parameter_name_completion_in_method_call` | `not_ignored` |
| `DISC_68C24ED019533CBFC5EA` | `rust_unit_test` | `crates/trust-ide/src/completion/tests.rs` | `test_parameter_name_completion_skips_used_formal` | `not_ignored` |
| `DISC_70971C3D5B5A125CA08B` | `rust_unit_test` | `crates/trust-ide/src/completion/tests.rs` | `test_standard_function_completion` | `not_ignored` |
| `DISC_AC9E3607BCE11C52F5B8` | `rust_unit_test` | `crates/trust-ide/src/completion/tests.rs` | `test_top_level_keywords` | `not_ignored` |
| `DISC_EFB84ADCB37BDE32498B` | `rust_unit_test` | `crates/trust-ide/src/completion/tests.rs` | `test_type_keywords` | `not_ignored` |
| `DISC_AB274A98FD02A5D1A403` | `rust_unit_test` | `crates/trust-ide/src/completion/tests.rs` | `test_typed_literal_completion` | `not_ignored` |
| `DISC_B516350856505B05A639` | `rust_unit_test` | `crates/trust-ide/src/completion/tests.rs` | `test_typed_literal_completion_after_prefix` | `not_ignored` |
| `DISC_8564C372E8CB6EC19099` | `rust_unit_test` | `crates/trust-ide/src/completion/tests.rs` | `test_using_namespace_completion_info` | `not_ignored` |
| `DISC_1F05897BAD4211C59F3F` | `rust_unit_test` | `crates/trust-ide/src/hover/config_and_tests.rs` | `test_format_type` | `not_ignored` |
| `DISC_CDA6FD4EBC484F816CA4` | `rust_unit_test` | `crates/trust-ide/src/hover/config_and_tests.rs` | `test_hover_namespace_ambiguity_info` | `not_ignored` |
| `DISC_7FC8B5CA0F8163576341` | `rust_unit_test` | `crates/trust-ide/src/hover/config_and_tests.rs` | `test_hover_namespace_using_info` | `not_ignored` |
| `DISC_00C874BB52FB4151F38B` | `rust_unit_test` | `crates/trust-ide/src/hover/config_and_tests.rs` | `test_hover_standard_function_doc` | `not_ignored` |
| `DISC_1D878A77F883EDC99F43` | `rust_unit_test` | `crates/trust-ide/src/hover/config_and_tests.rs` | `test_hover_typed_literal_doc` | `not_ignored` |
| `DISC_584D1D3E3776ECCB0A6B` | `rust_unit_test` | `crates/trust-ide/src/hover/config_and_tests.rs` | `test_hover_validate_function_doc` | `not_ignored` |
| `DISC_37BD60DD3438EA1F7E90` | `rust_unit_test` | `crates/trust-ide/src/inlay_hints.rs` | `inlay_hints_allow_named_args_after_positional` | `not_ignored` |
| `DISC_64A661229EA41069FB39` | `rust_unit_test` | `crates/trust-ide/src/inlay_hints.rs` | `inlay_hints_provide_parameter_names_for_positional_args` | `not_ignored` |
| `DISC_6C919359BB26957477E5` | `rust_unit_test` | `crates/trust-ide/src/inline_values.rs` | `inline_value_hints_for_external_constant` | `not_ignored` |
| `DISC_BB1019B839DE37C4ECC0` | `rust_unit_test` | `crates/trust-ide/src/inline_values.rs` | `inline_value_hints_for_var_temp_constant` | `not_ignored` |
| `DISC_99FA897202570DCD9715` | `rust_unit_test` | `crates/trust-ide/src/linked_editing.rs` | `linked_editing_filters_by_spelling` | `not_ignored` |
| `DISC_33D97BE4E557D831D760` | `rust_unit_test` | `crates/trust-ide/src/refactor/operations/tests.rs` | `convert_function_block_to_function_requires_no_instances` | `not_ignored` |
| `DISC_19D87AAF04BD1CF62C76` | `rust_unit_test` | `crates/trust-ide/src/refactor/operations/tests.rs` | `convert_function_block_to_function_updates_signature` | `not_ignored` |
| `DISC_6FBF630581D689D22934` | `rust_unit_test` | `crates/trust-ide/src/refactor/operations/tests.rs` | `convert_function_to_function_block_updates_calls` | `not_ignored` |
| `DISC_DB5FB90044BB71F89AF2` | `rust_unit_test` | `crates/trust-ide/src/refactor/operations/tests.rs` | `convert_function_to_function_block_updates_expression_calls` | `not_ignored` |
| `DISC_77B320477851DD08182E` | `rust_unit_test` | `crates/trust-ide/src/refactor/operations/tests.rs` | `extract_method_creates_method_and_call` | `not_ignored` |
| `DISC_39ED543E3F724DF841BC` | `rust_unit_test` | `crates/trust-ide/src/refactor/operations/tests.rs` | `extract_pou_creates_function` | `not_ignored` |
| `DISC_F27908A2E617F8499B69` | `rust_unit_test` | `crates/trust-ide/src/refactor/operations/tests.rs` | `extract_pou_expression_infers_return_type` | `not_ignored` |
| `DISC_3749626F8574DCE99810` | `rust_unit_test` | `crates/trust-ide/src/refactor/operations/tests.rs` | `extract_property_creates_property` | `not_ignored` |
| `DISC_805291BFE2C8291536AE` | `rust_unit_test` | `crates/trust-ide/src/refactor/operations/tests.rs` | `generate_interface_stubs_inserts_missing_members` | `not_ignored` |
| `DISC_972E092A99E24D5C2C36` | `rust_unit_test` | `crates/trust-ide/src/refactor/operations/tests.rs` | `inline_constant_across_files` | `not_ignored` |
| `DISC_547A155C368ECC4CEB98` | `rust_unit_test` | `crates/trust-ide/src/refactor/operations/tests.rs` | `inline_variable_initialized_from_var_input_constant` | `not_ignored` |
| `DISC_14BC1CE4E9E9F95209A4` | `rust_unit_test` | `crates/trust-ide/src/refactor/operations/tests.rs` | `inline_variable_initialized_from_var_temp_constant` | `not_ignored` |
| `DISC_99AD166BA624409A9C22` | `rust_unit_test` | `crates/trust-ide/src/refactor/operations/tests.rs` | `inline_variable_with_literal_initializer` | `not_ignored` |
| `DISC_80464DFE5FC0B6B52243` | `rust_unit_test` | `crates/trust-ide/src/refactor/operations/tests.rs` | `move_namespace_updates_using_and_qualified_names` | `not_ignored` |
| `DISC_09EE6EA77422464F810A` | `rust_unit_test` | `crates/trust-ide/src/rename.rs` | `test_is_valid_identifier` | `not_ignored` |
| `DISC_11083FC725D628FA2D58` | `rust_unit_test` | `crates/trust-ide/src/rename.rs` | `test_reserved_keywords_rejected` | `not_ignored` |
| `DISC_8B95EAA0BD51A6EDB40F` | `rust_unit_test` | `crates/trust-ide/src/selection_range.rs` | `selection_range_has_parent_chain` | `not_ignored` |
| `DISC_826C28D9112EEFDF486C` | `rust_unit_test` | `crates/trust-ide/src/semantic_tokens.rs` | `test_semantic_token_creation` | `not_ignored` |
| `DISC_BFF1C613624FE8F8C6F4` | `rust_unit_test` | `crates/trust-ide/src/semantic_tokens.rs` | `test_semantic_token_modifiers` | `not_ignored` |
| `DISC_9F067950161009E33DB6` | `rust_unit_test` | `crates/trust-ide/src/text_range.rs` | `extend_range_to_line_end_includes_trailing_newline` | `not_ignored` |
| `DISC_C1799A5C8145E8BB0431` | `rust_unit_test` | `crates/trust-ide/src/text_range.rs` | `text_for_range_trims_segment` | `not_ignored` |
| `DISC_7359722765B15C2650E4` | `rust_unit_test` | `crates/trust-ide/src/type_hierarchy.rs` | `type_hierarchy_extends_and_implements` | `not_ignored` |
| `DISC_AFACDFEF190DE0AB18CC` | `rust_unit_test` | `crates/trust-ide/src/type_hierarchy.rs` | `type_hierarchy_resolves_interfaces_in_namespace` | `not_ignored` |
| `DISC_E2994F34C7E155E07813` | `rust_unit_test` | `crates/trust-ide/src/var_decl.rs` | `declared_type_extracts_plain_and_initialized_declarations` | `not_ignored` |
| `DISC_3AC78E5D2C713D76F746` | `rust_unit_test` | `crates/trust-ide/src/var_decl.rs` | `var_decl_info_carries_declared_type_for_symbol_range` | `not_ignored` |
| `DISC_CEC0161E2F77088C280B` | `rust_unit_test` | `crates/trust-lsp/src/config/tests.rs` | `enforces_git_host_allowlist_policy` | `not_ignored` |
| `DISC_28EEC58CD63EE9D59EE7` | `rust_unit_test` | `crates/trust-lsp/src/config/tests.rs` | `indexing_roots_replaces_root_when_include_paths_set` | `not_ignored` |
| `DISC_B3ADF8F361CC5702B3B0` | `rust_unit_test` | `crates/trust-lsp/src/config/tests.rs` | `indexing_roots_uses_root_when_no_include_paths` | `not_ignored` |
| `DISC_16DA97C10A1BBB7A6052` | `rust_unit_test` | `crates/trust-lsp/src/config/tests.rs` | `loads_project_config_with_includes_and_libraries` | `not_ignored` |
| `DISC_0D62852C5B576922BFFC` | `rust_unit_test` | `crates/trust-lsp/src/config/tests.rs` | `locked_mode_requires_pin_or_lock_entry_for_git_dependencies` | `not_ignored` |
| `DISC_386829510508042A3F21` | `rust_unit_test` | `crates/trust-lsp/src/config/tests.rs` | `mitsubishi_vendor_profile_keeps_default_diagnostics_enabled` | `not_ignored` |
| `DISC_7CA6B0C95F982C23D82F` | `rust_unit_test` | `crates/trust-lsp/src/config/tests.rs` | `offline_locked_mode_uses_cached_lock_resolution` | `not_ignored` |
| `DISC_1C61E9E32200E8773426` | `rust_unit_test` | `crates/trust-lsp/src/config/tests.rs` | `reports_dependency_missing_path_and_version_mismatch` | `not_ignored` |
| `DISC_52FF6B332DD098BB7477` | `rust_unit_test` | `crates/trust-lsp/src/config/tests.rs` | `resolves_git_dependencies_with_rev_tag_and_branch_pinning` | `not_ignored` |
| `DISC_BFA576973B67FA552A59` | `rust_unit_test` | `crates/trust-lsp/src/config/tests.rs` | `resolves_local_dependencies_transitively` | `not_ignored` |
| `DISC_86FED92D1557884B4A55` | `rust_unit_test` | `crates/trust-lsp/src/config/tests.rs` | `vendor_profile_applies_diagnostic_defaults` | `not_ignored` |
| `DISC_797AB8669758B1BD45AA` | `rust_unit_test` | `crates/trust-lsp/src/handlers/commands/path_ranges_and_tests.rs` | `hmi_bindings_command_rejects_invalid_argument_shape` | `not_ignored` |
| `DISC_3E699D2B7BF1E67D509E` | `rust_unit_test` | `crates/trust-lsp/src/handlers/commands/path_ranges_and_tests.rs` | `hmi_bindings_command_with_mock_context_returns_external_contract_catalog` | `not_ignored` |
| `DISC_4BA27FCFF2E34916C2BB` | `rust_unit_test` | `crates/trust-lsp/src/handlers/commands/path_ranges_and_tests.rs` | `hmi_init_command_rejects_invalid_style` | `not_ignored` |
| `DISC_68D34BB5C3484AD12D57` | `rust_unit_test` | `crates/trust-lsp/src/handlers/commands/path_ranges_and_tests.rs` | `hmi_init_command_with_mock_context_generates_scaffold` | `not_ignored` |
| `DISC_0088F667AC781E5DA6FB` | `rust_unit_test` | `crates/trust-lsp/src/handlers/commands/path_ranges_and_tests.rs` | `namespace_move_apply_phases_keep_create_and_edits_before_delete` | `not_ignored` |
| `DISC_5DF3B787ADD9F36548D9` | `rust_unit_test` | `crates/trust-lsp/src/handlers/commands/path_ranges_and_tests.rs` | `namespace_move_with_mock_context_generates_expected_operations` | `not_ignored` |
| `DISC_B246CACD21397A296F8D` | `rust_unit_test` | `crates/trust-lsp/src/handlers/commands/path_ranges_and_tests.rs` | `project_info_server_state_and_context_paths_match` | `not_ignored` |
| `DISC_726F6BAC5F8149DE6988` | `rust_unit_test` | `crates/trust-lsp/src/handlers/commands/path_ranges_and_tests.rs` | `project_info_with_mock_context_uses_uri_mapping` | `not_ignored` |
| `DISC_BE2C042F9FA1F804D7B7` | `rust_unit_test` | `crates/trust-lsp/src/handlers/config.rs` | `alias_lookup_prefers_first_key_and_ignores_wrong_types` | `not_ignored` |
| `DISC_6F0D45350672CC2CBF21` | `rust_unit_test` | `crates/trust-lsp/src/handlers/config.rs` | `lsp_section_prefers_stlsp_then_trust_lsp_aliases` | `not_ignored` |
| `DISC_7E7BA06ED630D46B70F1` | `rust_unit_test` | `crates/trust-lsp/src/handlers/config.rs` | `runtime_section_supports_nested_and_top_level_runtime` | `not_ignored` |
| `DISC_7DB094D04324A7CA0852` | `rust_unit_test` | `crates/trust-lsp/src/handlers/diagnostics/collection_and_filters.rs` | `numeric_hazard_filter_controls_numeric_warning_codes` | `not_ignored` |
| `DISC_30CC3FBF93DA6AEAE39C` | `rust_unit_test` | `crates/trust-lsp/src/handlers/diagnostics/publish_hmi_and_tests.rs` | `hmi_toml_diagnostics_avoid_false_positives_for_valid_page` | `not_ignored` |
| `DISC_8C6BBEE8911DE894B25B` | `rust_unit_test` | `crates/trust-lsp/src/handlers/diagnostics/publish_hmi_and_tests.rs` | `hmi_toml_diagnostics_report_type_widget_and_property_issues` | `not_ignored` |
| `DISC_887F80B5FD3173D88B00` | `rust_unit_test` | `crates/trust-lsp/src/handlers/diagnostics/publish_hmi_and_tests.rs` | `hmi_toml_diagnostics_report_unknown_bind_with_near_match_hint` | `not_ignored` |
| `DISC_899E7B19DFD2C8B935D6` | `rust_unit_test` | `crates/trust-lsp/src/handlers/diagnostics/publish_hmi_and_tests.rs` | `suggestion_ranking_prefers_closest_match` | `not_ignored` |
| `DISC_FA853F7CD3A23C7208A6` | `rust_unit_test` | `crates/trust-lsp/src/handlers/diagnostics/publish_hmi_and_tests.rs` | `suggestion_ranking_suppresses_low_confidence_noise` | `not_ignored` |
| `DISC_7B19E9606F2D91033B07` | `rust_unit_test` | `crates/trust-lsp/src/handlers/formatting/tests.rs` | `format_document_aligns_var_colons` | `not_ignored` |
| `DISC_A5C7A8645F843052ECF0` | `rust_unit_test` | `crates/trust-lsp/src/handlers/formatting/tests.rs` | `format_document_compact_spacing` | `not_ignored` |
| `DISC_43A687383B0EA9286039` | `rust_unit_test` | `crates/trust-lsp/src/handlers/formatting/tests.rs` | `format_document_indented_end_keywords` | `not_ignored` |
| `DISC_ADD18DF30F9F10CBB6D3` | `rust_unit_test` | `crates/trust-lsp/src/handlers/formatting/tests.rs` | `format_document_normalizes_spacing` | `not_ignored` |
| `DISC_1C8BA0F4F27F6656997F` | `rust_unit_test` | `crates/trust-lsp/src/handlers/formatting/tests.rs` | `format_document_preserves_mixed_pragma_lines` | `not_ignored` |
| `DISC_D8A9CFEB73E6A2863E6F` | `rust_unit_test` | `crates/trust-lsp/src/handlers/formatting/tests.rs` | `format_document_respects_var_alignment_groups` | `not_ignored` |
| `DISC_EBF6CFEA2266F5C87E13` | `rust_unit_test` | `crates/trust-lsp/src/handlers/formatting/tests.rs` | `format_document_skips_wrapping_string_literal_lines` | `not_ignored` |
| `DISC_8BF526DFC16E34DFDDD6` | `rust_unit_test` | `crates/trust-lsp/src/handlers/lsp_utils.rs` | `eof_position_after_trailing_newline_round_trips_to_content_len` | `not_ignored` |
| `DISC_DA60EF06AEC4FA918DF0` | `rust_unit_test` | `crates/trust-lsp/src/handlers/sync.rs` | `apply_content_changes_full_sync` | `not_ignored` |
| `DISC_48283CA64FC8CD44B7FF` | `rust_unit_test` | `crates/trust-lsp/src/handlers/sync.rs` | `apply_content_changes_inserts_text` | `not_ignored` |
| `DISC_F70C8F0D8FAA5011305F` | `rust_unit_test` | `crates/trust-lsp/src/handlers/sync.rs` | `apply_content_changes_replaces_range` | `not_ignored` |
| `DISC_F4247FE3896F6E29B9CF` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/code_actions_and_commands_part_01.rs` | `lsp_code_action_create_type` | `not_ignored` |
| `DISC_F577EB055901303C4283` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/code_actions_and_commands_part_01.rs` | `lsp_code_action_create_var` | `not_ignored` |
| `DISC_EA7B1EC0AFED023AF015` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/code_actions_and_commands_part_01.rs` | `lsp_code_action_implicit_conversion` | `not_ignored` |
| `DISC_6BE69B91DEB4E42CEA89` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/code_actions_and_commands_part_01.rs` | `lsp_code_action_missing_else` | `not_ignored` |
| `DISC_726D59670B4AB65D3153` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/code_actions_and_commands_part_02.rs` | `lsp_code_action_convert_call_style` | `not_ignored` |
| `DISC_CC2F881DE0190A4E385F` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/code_actions_and_commands_part_02.rs` | `lsp_code_action_incompatible_assignment_conversion` | `not_ignored` |
| `DISC_A30DB3B5FB3204AD0F89` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/code_actions_and_commands_part_02.rs` | `lsp_code_action_namespace_move` | `not_ignored` |
| `DISC_D02962F6813125E31539` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/code_actions_and_commands_part_02.rs` | `lsp_code_action_reorder_positional_first_call` | `not_ignored` |
| `DISC_1814EF436197C0D2E62B` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/code_actions_and_commands_part_03.rs` | `lsp_code_action_convert_function_to_function_block` | `not_ignored` |
| `DISC_AA09BEDDA1970BDAD27A` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/code_actions_and_commands_part_03.rs` | `lsp_code_action_extract_method` | `not_ignored` |
| `DISC_905EC24892D218951BBC` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/code_actions_and_commands_part_03.rs` | `lsp_code_action_generate_interface_stubs` | `not_ignored` |
| `DISC_C594CCB304BE7DF21DDC` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/code_actions_and_commands_part_03.rs` | `lsp_code_action_inline_variable` | `not_ignored` |
| `DISC_8C82EC070BB1D560AF3B` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/code_actions_and_commands_part_04.rs` | `lsp_code_action_convert_function_block_to_function` | `not_ignored` |
| `DISC_429F0DF112CC824DE184` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/code_actions_and_commands_part_04.rs` | `lsp_execute_command_namespace_move_workspace_edit` | `not_ignored` |
| `DISC_ED1B5DC304989E4D3912` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/code_actions_and_commands_part_04.rs` | `lsp_project_info_exposes_build_and_targets` | `not_ignored` |
| `DISC_17C5D17CA6A8BC71CF83` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/code_actions_and_commands_part_05.rs` | `lsp_code_action_adds_openot_logging_by_declared_type` | `not_ignored` |
| `DISC_2F44FEB7EB3ED624F418` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/code_actions_and_commands_part_05.rs` | `lsp_code_action_namespace_disambiguation` | `not_ignored` |
| `DISC_9D86737DA32D043858A7` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/code_actions_and_commands_part_05.rs` | `lsp_code_action_namespace_disambiguation_project_using` | `not_ignored` |
| `DISC_44E674E3EC185C985F24` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/completion_hover.rs` | `lsp_completion_respects_stdlib_allowlist` | `not_ignored` |
| `DISC_D35795BBB474A23146F7` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/completion_hover.rs` | `lsp_completion_respects_stdlib_profile_none` | `not_ignored` |
| `DISC_56BF2D27BF6000E54620` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/completion_hover.rs` | `lsp_completion_returns_none_when_request_ticket_is_cancelled` | `not_ignored` |
| `DISC_EA79111DF9EA0E78F7FC` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/completion_hover.rs` | `lsp_completion_suggests_method_formal_parameters` | `not_ignored` |
| `DISC_E1731DCFB04E3EC3CB6E` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/completion_hover.rs` | `lsp_hover_member_method_and_property` | `not_ignored` |
| `DISC_000C0BA97DBB5C15E10E` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/completion_hover.rs` | `lsp_hover_respects_stdlib_filter` | `not_ignored` |
| `DISC_E0093CE789BADAA24BC3` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_01.rs` | `lsp_diagnostics_no_burst_baseline_reports_real_errors` | `not_ignored` |
| `DISC_B23AA386CB36FC319E6A` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_01.rs` | `lsp_hover_variable` | `not_ignored` |
| `DISC_2A0201A115722E554C2D` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_01.rs` | `lsp_pull_diagnostics_returns_unchanged_and_explainer` | `not_ignored` |
| `DISC_ADDC546497AC53112B2A` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_01.rs` | `lsp_references_variable` | `not_ignored` |
| `DISC_C596BB3431D51DF995AF` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_01.rs` | `lsp_rename_namespace_path_updates_using_and_qualified_names` | `not_ignored` |
| `DISC_D3AA682468B2771890AE` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_01.rs` | `lsp_rename_primary_pou_renames_file` | `not_ignored` |
| `DISC_72ABE3B11D9A7B908D59` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_01.rs` | `lsp_rename_variable` | `not_ignored` |
| `DISC_FBF5DC2C2862B1071328` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_02.rs` | `lsp_diagnostics_respect_config_toggles` | `not_ignored` |
| `DISC_B4CD0C6FCA0496645AA2` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_02.rs` | `lsp_learner_diagnostics_include_did_you_mean_and_conversion_guidance` | `not_ignored` |
| `DISC_AACA19C5027E37F0FBA5` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_02.rs` | `lsp_supports_virtual_document_uris` | `not_ignored` |
| `DISC_08D7CC9F32151B6D0883` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_03.rs` | `lsp_config_diagnostics_report_dependency_cycle_issues` | `not_ignored` |
| `DISC_52F84709AF8DBC4185D5` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_03.rs` | `lsp_config_diagnostics_report_library_dependency_issues` | `not_ignored` |
| `DISC_359C2C932DD143B6A94D` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_03.rs` | `lsp_learner_diagnostics_include_syntax_habit_hints` | `not_ignored` |
| `DISC_5108A0BFFC5F80D53C99` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_03.rs` | `lsp_learner_diagnostics_no_hint_noise_on_valid_code` | `not_ignored` |
| `DISC_A8ED715318640DA93A0E` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_04.rs` | `lsp_document_symbols_include_configuration_hierarchy` | `not_ignored` |
| `DISC_414E6D26E88310EAB561` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_04.rs` | `lsp_external_diagnostics_provide_quick_fixes` | `not_ignored` |
| `DISC_2299CAC2F904F85FB282` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_04.rs` | `lsp_workspace_symbols_include_dependency_sources` | `not_ignored` |
| `DISC_DD95439A5C1FC7F35099` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_05.rs` | `lsp_document_symbols_include_members` | `not_ignored` |
| `DISC_29AFB7D7177472E38F54` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_05.rs` | `lsp_hmi_toml_diagnostics_use_open_source_buffers` | `not_ignored` |
| `DISC_5E1F500FAB91DA314921` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_05.rs` | `lsp_hmi_toml_local_property_diagnostics_do_not_require_runtime_compile` | `not_ignored` |
| `DISC_3BE6E8E1A5DF63668F60` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_05.rs` | `lsp_memory_budget_eviction_keeps_closed_dependency_semantically_indexed` | `not_ignored` |
| `DISC_A7753810AB937625D6AA` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_05.rs` | `lsp_oop_access_diagnostics_include_explainer_and_hint` | `not_ignored` |
| `DISC_1D1F7883FDC052B6FFB0` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_05.rs` | `lsp_push_sync_refreshes_dependent_open_document_diagnostics` | `not_ignored` |
| `DISC_818364CD7F8F3F86CD28` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_05.rs` | `lsp_will_rename_files_updates_pou_name` | `not_ignored` |
| `DISC_5DC993BF389C806BEE3B` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_05.rs` | `lsp_workspace_diagnostics_supports_unchanged_reports` | `not_ignored` |
| `DISC_0025E404D4C651A13A89` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_06.rs` | `lsp_document_highlight_variable` | `not_ignored` |
| `DISC_75651FE091A83D17F3BA` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_06.rs` | `lsp_references_partial_result_token_returns_empty_final_response` | `not_ignored` |
| `DISC_8EFABE39D3C261DB52B2` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_06.rs` | `lsp_semantic_tokens_delta` | `not_ignored` |
| `DISC_71F51CF28CFFE96F622C` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_06.rs` | `lsp_will_rename_files_updates_using_namespace` | `not_ignored` |
| `DISC_F928598D5BBAA90A687C` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_06.rs` | `lsp_workspace_symbols` | `not_ignored` |
| `DISC_6BABF1F5BCB555CD8662` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_06.rs` | `lsp_workspace_symbols_partial_result_token_returns_empty_final_response` | `not_ignored` |
| `DISC_ADA1F6D7C200549FC079` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_06.rs` | `lsp_workspace_symbols_respect_root_visibility_and_priority` | `not_ignored` |
| `DISC_269A71689690AD6442AC` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_07.rs` | `lsp_inlay_hints_parameters` | `not_ignored` |
| `DISC_F2EEF121410262A63852` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_07.rs` | `lsp_inline_values_constants` | `not_ignored` |
| `DISC_CEE6064FCE98892EE0B0` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_07.rs` | `lsp_inline_values_fetch_runtime_values_from_control_stub` | `not_ignored` |
| `DISC_B695B2DE5B42BDCF6C0E` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_07.rs` | `lsp_linked_editing_ranges` | `not_ignored` |
| `DISC_27320C78A7ADFAE6DCA2` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_08.rs` | `lsp_inline_values_merge_instances_into_locals` | `not_ignored` |
| `DISC_630A4F126B28C4B9ED98` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_08.rs` | `lsp_inline_values_runtime_override_accepts_camel_case_client_settings` | `not_ignored` |
| `DISC_F60A22ECF20B175C891E` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_08.rs` | `lsp_inline_values_runtime_override_accepts_snake_case_client_settings` | `not_ignored` |
| `DISC_7CD986673360C8A05E91` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_08.rs` | `lsp_inline_values_runtime_override_prefers_camel_case_when_aliases_conflict` | `not_ignored` |
| `DISC_BD0392DD8ECDFB73D297` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_08.rs` | `lsp_inline_values_silent_runtime_endpoint_returns_bounded_empty_result` | `not_ignored` |
| `DISC_FE98D0D47CA087189BA5` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_09.rs` | `lsp_inline_values_merge_instances_with_namespace` | `not_ignored` |
| `DISC_2372C0F335CE19502006` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_09.rs` | `lsp_tutorial_examples_no_unexpected_diagnostics_snapshot` | `not_ignored` |
| `DISC_D267DFF2A19BCADE84F0` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_10.rs` | `lsp_siemens_hash_prefixed_example_has_no_unexpected_diagnostics` | `not_ignored` |
| `DISC_E0E22C0EC0B61775EEED` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_11.rs` | `lsp_mitsubishi_gxworks3_example_has_no_unexpected_diagnostics` | `not_ignored` |
| `DISC_C102AA7AC4D326E5EF0C` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_12.rs` | `lsp_completion_constant_parameter_uses_constant_kind` | `not_ignored` |
| `DISC_20DCDAF4C10E2884645C` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_12.rs` | `lsp_diagnostics_report_fb_instance_in_constant_sections` | `not_ignored` |
| `DISC_9FBD644A53793A237C50` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_12.rs` | `lsp_hover_constant_parameter_mentions_constant_and_array_star` | `not_ignored` |
| `DISC_1963CFE4E5CC7C5C1A44` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_12.rs` | `lsp_openot_completion_returns_documented_values_and_keys` | `not_ignored` |
| `DISC_FFB68EFC63EA80BC37D9` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_12.rs` | `lsp_openot_inlay_hint_shows_emitted_record` | `not_ignored` |
| `DISC_09B899390D251D20E37A` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_12.rs` | `lsp_openot_validation_reports_bad_value_and_accepts_good_value` | `not_ignored` |
| `DISC_73F2A71E13AE1ADCC856` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_12.rs` | `lsp_signature_help_constant_parameter_mentions_constant` | `not_ignored` |
| `DISC_1B3F554E51C6AABBA763` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_12.rs` | `lsp_signature_help_method_var_input_mentions_method_parameters` | `not_ignored` |
| `DISC_28630BCD54272FF6DDE7` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/core_part_12.rs` | `lsp_workspace_symbols_mark_constant_parameters_as_constants` | `not_ignored` |
| `DISC_40165F8BB5FE63464494` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/formatting_and_navigation.rs` | `lsp_call_hierarchy_cross_file_incoming` | `not_ignored` |
| `DISC_5DF9EF18D1F99FE65BF0` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/formatting_and_navigation.rs` | `lsp_call_hierarchy_cross_file_incoming_named_args` | `not_ignored` |
| `DISC_A372FF340F8A226AC746` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/formatting_and_navigation.rs` | `lsp_call_hierarchy_incoming_outgoing` | `not_ignored` |
| `DISC_79CCBEA7A72A638897FD` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/formatting_and_navigation.rs` | `lsp_code_lens_references` | `not_ignored` |
| `DISC_0275143E75B1079C7211` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/formatting_and_navigation.rs` | `lsp_document_link_config_paths` | `not_ignored` |
| `DISC_104C72D317FDBB6DF3C1` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/formatting_and_navigation.rs` | `lsp_document_link_using_directive` | `not_ignored` |
| `DISC_9EAAA513ECC904544491` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/formatting_and_navigation.rs` | `lsp_formatting_accepts_snake_case_client_keys` | `not_ignored` |
| `DISC_9EB4703390B73A453CC4` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/formatting_and_navigation.rs` | `lsp_formatting_mitsubishi_profile_keeps_spaced_style` | `not_ignored` |
| `DISC_4661BEFF4881B6DF3CE0` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/formatting_and_navigation.rs` | `lsp_formatting_prefers_camel_case_when_both_aliases_present` | `not_ignored` |
| `DISC_A127E92307EF6386753A` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/formatting_and_navigation.rs` | `lsp_formatting_siemens_profile_preserves_hash_prefixed_references` | `not_ignored` |
| `DISC_CE20A3E088B544103B47` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/formatting_and_navigation.rs` | `lsp_formatting_snapshot` | `not_ignored` |
| `DISC_42C658CC8540869BD592` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/formatting_and_navigation.rs` | `lsp_formatting_vendor_profile_applies_keyword_case` | `not_ignored` |
| `DISC_039A9AD340FED57CD45E` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/formatting_and_navigation.rs` | `lsp_on_type_formatting_formats_line` | `not_ignored` |
| `DISC_E5F7D79D17EBF6461997` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/formatting_and_navigation.rs` | `lsp_range_formatting_aligns_assignment_groups` | `not_ignored` |
| `DISC_F510A67ED09C71BA2B67` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/formatting_and_navigation.rs` | `lsp_range_formatting_expands_to_syntax_block` | `not_ignored` |
| `DISC_31D26138463F34C5DDB0` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/formatting_and_navigation.rs` | `lsp_range_formatting_formats_selection` | `not_ignored` |
| `DISC_F3D37E3B7FBBF716E413` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/formatting_and_navigation.rs` | `lsp_signature_help_snapshot` | `not_ignored` |
| `DISC_7C31C1C33ECF79D74AA2` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/formatting_and_navigation.rs` | `lsp_type_hierarchy_super_and_subtypes` | `not_ignored` |
| `DISC_3E5707F43CB98F0AD8D8` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/mod_part_01_part_01.rs` | `lsp_golden_multi_root_protocol_snapshot` | `not_ignored` |
| `DISC_B6FEF7AB6FD69C80DBEC` | `rust_unit_test` | `crates/trust-lsp/src/handlers/tests/mod_part_02.rs` | `lsp_code_action_namespace_disambiguation_non_call` | `not_ignored` |
| `DISC_8E433C071D750B5DA8DF` | `rust_unit_test` | `crates/trust-lsp/src/index_cache.rs` | `cache_rejects_same_size_same_second_mtime_collision` | `not_ignored` |
| `DISC_20DBBB2EE37474C8E99C` | `rust_unit_test` | `crates/trust-lsp/src/index_cache.rs` | `cache_round_trip_and_invalidate_on_change` | `not_ignored` |
| `DISC_C0F04355293B2882A1F0` | `rust_unit_test` | `crates/trust-lsp/src/library_graph.rs` | `library_dependency_issues_report_cycles` | `not_ignored` |
| `DISC_6F5F5107DDE61D3C1E43` | `rust_unit_test` | `crates/trust-lsp/src/perf.rs` | `perf_completion_budget` | `ignored` |
| `DISC_A5BFD4D2FEC4A5FE7C73` | `rust_unit_test` | `crates/trust-lsp/src/perf.rs` | `perf_diagnostics_budget` | `ignored` |
| `DISC_F7D08DE71C821B2157CF` | `rust_unit_test` | `crates/trust-lsp/src/perf.rs` | `perf_document_highlight_scaling_budget` | `ignored` |
| `DISC_08CEAF52EF3509019BFF` | `rust_unit_test` | `crates/trust-lsp/src/perf.rs` | `perf_edit_loop_budget` | `ignored` |
| `DISC_B65B577CE97DDD0DC665` | `rust_unit_test` | `crates/trust-lsp/src/perf.rs` | `perf_hover_budget` | `ignored` |
| `DISC_D5AB69F6A0EBB17DA479` | `rust_unit_test` | `crates/trust-lsp/src/perf.rs` | `perf_large_workspace_index_budget` | `ignored` |
| `DISC_0D7E1103BA7F27B6CD0E` | `rust_unit_test` | `crates/trust-lsp/src/perf.rs` | `perf_rename_budget` | `ignored` |
| `DISC_E0C98D6962CBF3D5E78F` | `rust_unit_test` | `crates/trust-lsp/src/perf.rs` | `perf_semantic_tokens_scaling_budget` | `ignored` |
| `DISC_F4D12F76BE0C9EC8F0EF` | `rust_unit_test` | `crates/trust-lsp/src/perf.rs` | `perf_workspace_navigation_scaling_budget` | `ignored` |
| `DISC_CE3D5672C7ABFFB9A685` | `rust_unit_test` | `crates/trust-lsp/src/perf.rs` | `perf_workspace_symbol_budget` | `ignored` |
| `DISC_CB5F57C9D8ACA804E2DE` | `rust_unit_test` | `crates/trust-lsp/src/state/mod.rs` | `background_requests_run_when_limiter_is_closed` | `not_ignored` |
| `DISC_D2135B66BA9B954B91FB` | `rust_unit_test` | `crates/trust-lsp/src/state/mod.rs` | `background_requests_serialize` | `not_ignored` |
| `DISC_95AF2D413250F36F3DAD` | `rust_unit_test` | `crates/trust-lsp/src/state/mod.rs` | `diagnostic_cache_reuses_result_id_for_identical_hashes` | `not_ignored` |
| `DISC_11CDA813DFA1A9669B98` | `rust_unit_test` | `crates/trust-lsp/src/state/mod.rs` | `document_lifecycle_open_update_close_rename_remove` | `not_ignored` |
| `DISC_0CB85393CF5F1C93EC04` | `rust_unit_test` | `crates/trust-lsp/src/state/mod.rs` | `evicts_closed_documents_over_budget` | `not_ignored` |
| `DISC_CA8F06CC56AFA5775266` | `rust_unit_test` | `crates/trust-lsp/src/state/mod.rs` | `library_docs_cache_reuses_entries_until_workspace_config_changes` | `not_ignored` |
| `DISC_1511FB43E74E887DCC31` | `rust_unit_test` | `crates/trust-lsp/src/state/mod.rs` | `mark_index_first_pass_done_is_idempotent` | `not_ignored` |
| `DISC_A14C9116E67DFD0D61A8` | `rust_unit_test` | `crates/trust-lsp/src/state/mod.rs` | `path_to_uri_strips_extended_length_prefix` | `not_ignored` |
| `DISC_812ED0CC7B4EAB354F6E` | `rust_unit_test` | `crates/trust-lsp/src/state/mod.rs` | `path_uri_roundtrip_handles_spaces_and_fragments` | `not_ignored` |
| `DISC_06F21CDCB3CA7DD26BAF` | `rust_unit_test` | `crates/trust-lsp/src/state/mod.rs` | `uri_to_path_decodes_drive_letter` | `not_ignored` |
| `DISC_CC45E653570797C32038` | `rust_unit_test` | `crates/trust-lsp/src/state/mod.rs` | `wait_for_index_first_pass_blocks_until_marked` | `not_ignored` |
| `DISC_E5AEEE66289976CE10B0` | `rust_unit_test` | `crates/trust-lsp/src/state/mod.rs` | `wait_for_index_first_pass_returns_immediately_when_already_marked` | `not_ignored` |
| `DISC_E63AB1473BE8E8CB031D` | `rust_unit_test` | `crates/trust-lsp/src/telemetry.rs` | `telemetry_writes_metrics` | `not_ignored` |
| `DISC_A108B907F544B3DEAAAD` | `rust_unit_test` | `crates/trust-plcopen/src/plcopen/tests/export_profile.rs` | `export_emits_codesys_global_vars_and_project_structure_metadata` | `not_ignored` |
| `DISC_5F7C56DD80D372BE46BC` | `rust_unit_test` | `crates/trust-plcopen/src/plcopen/tests/export_profile.rs` | `export_emits_codesys_method_metadata_on_standard_pous_and_roundtrips` | `not_ignored` |
| `DISC_4C1F7E6CFDEB8C495CC1` | `rust_unit_test` | `crates/trust-plcopen/src/plcopen/tests/export_profile.rs` | `export_emits_codesys_pou_add_data_metadata` | `not_ignored` |
| `DISC_DB8B3550451ED603EF91` | `rust_unit_test` | `crates/trust-plcopen/src/plcopen/tests/export_profile.rs` | `export_reinjects_vendor_extension_hook_file` | `not_ignored` |
| `DISC_956052E097086C1F1216` | `rust_unit_test` | `crates/trust-plcopen/src/plcopen/tests/export_profile.rs` | `export_siemens_target_emits_scl_bundle_and_program_ob_mapping` | `not_ignored` |
| `DISC_2E0C4F00C6F5D55A425C` | `rust_unit_test` | `crates/trust-plcopen/src/plcopen/tests/export_profile.rs` | `export_with_vendor_target_emits_adapter_report_and_metadata` | `not_ignored` |
| `DISC_235287B1F32A8ABCB1E3` | `rust_unit_test` | `crates/trust-plcopen/src/plcopen/tests/export_profile.rs` | `import_rejects_malformed_xml` | `not_ignored` |
| `DISC_D1F203125AB5E09674C3` | `rust_unit_test` | `crates/trust-plcopen/src/plcopen/tests/export_profile.rs` | `profile_declares_strict_subset_contract` | `not_ignored` |
| `DISC_E842B57853EE4EA7403B` | `rust_unit_test` | `crates/trust-plcopen/src/plcopen/tests/roundtrip_import.rs` | `import_accepts_st_body_with_body_level_adddata_metadata` | `not_ignored` |
| `DISC_AE70D7BD1B3A3D4AF3C7` | `rust_unit_test` | `crates/trust-plcopen/src/plcopen/tests/roundtrip_import.rs` | `import_rejects_non_st_bodies_with_named_diagnostics` | `not_ignored` |
| `DISC_831A1BDFCC60EA9890D7` | `rust_unit_test` | `crates/trust-plcopen/src/plcopen/tests/roundtrip_import.rs` | `import_reports_unsupported_nodes_and_preserves_vendor_extensions` | `not_ignored` |
| `DISC_59C58D10EEA63CF2DA67` | `rust_unit_test` | `crates/trust-plcopen/src/plcopen/tests/roundtrip_import.rs` | `import_supports_data_type_subset_and_generates_type_source` | `not_ignored` |
| `DISC_C256D0AA8F2B315BE2DD` | `rust_unit_test` | `crates/trust-plcopen/src/plcopen/tests/roundtrip_import.rs` | `round_trip_export_import_export_preserves_pou_subset` | `not_ignored` |
| `DISC_00B66157915C56E44641` | `rust_unit_test` | `crates/trust-plcopen/src/plcopen/tests/shims.rs` | `import_applies_siemens_library_shims_and_reports_them` | `not_ignored` |
| `DISC_3BE66B444B69BE64121D` | `rust_unit_test` | `crates/trust-plcopen/src/plcopen/tests/shims.rs` | `library_shim_rewrites_type_and_call_sites_only` | `not_ignored` |
| `DISC_EFE9A8C1424C348DDF8F` | `rust_unit_test` | `crates/trust-runtime-core/src/bytecode/mod.rs` | `bytecode_alignment_helpers_preserve_zero_padding_contract` | `not_ignored` |
| `DISC_9B528C376417D3CDD0D2` | `rust_unit_test` | `crates/trust-runtime-core/src/bytecode/mod.rs` | `bytecode_format_records_preserve_raw_discriminants` | `not_ignored` |
| `DISC_C01864A789B8447D5D0E` | `rust_unit_test` | `crates/trust-runtime-core/src/bytecode/mod.rs` | `bytecode_metadata_resource_lookup_is_case_insensitive` | `not_ignored` |
| `DISC_89053FD5D616E169829F` | `rust_unit_test` | `crates/trust-runtime-core/src/bytecode/mod.rs` | `bytecode_reader_preserves_little_endian_contract_and_eof` | `not_ignored` |
| `DISC_248BA198613ED4EEA4CB` | `rust_unit_test` | `crates/trust-runtime-core/src/cycle.rs` | `ready_task_sort_preserves_priority_due_time_and_stable_index_order` | `not_ignored` |
| `DISC_E21E21DCD1EF068951BC` | `rust_unit_test` | `crates/trust-runtime-core/src/datetime.rs` | `rejects_invalid_month_length` | `not_ignored` |
| `DISC_3BACCC2F19E340EBE5E8` | `rust_unit_test` | `crates/trust-runtime-core/src/datetime.rs` | `rejects_invalid_non_leap_day` | `not_ignored` |
| `DISC_9A7AE24526A19A0BB7EE` | `rust_unit_test` | `crates/trust-runtime-core/src/error_code.rs` | `stable_error_codes_use_lower_snake_case` | `not_ignored` |
| `DISC_7079FED218012C4E6274` | `rust_unit_test` | `crates/trust-runtime-core/src/memory.rs` | `memory_identity_values_preserve_equality_and_hash_shape` | `not_ignored` |
| `DISC_25B0EF01FE0B077934F8` | `rust_unit_test` | `crates/trust-runtime-core/src/numeric.rs` | `integer_conversions_preserve_overflow_and_signedness_errors` | `not_ignored` |
| `DISC_4FB7BEEED7E435C425AD` | `rust_unit_test` | `crates/trust-runtime-core/src/numeric.rs` | `numeric_kind_and_rank_preserve_existing_widening_order` | `not_ignored` |
| `DISC_E33F3DC4673627AFE4DD` | `rust_unit_test` | `crates/trust-runtime-core/src/program_model/expr.rs` | `lvalue_root_and_qualified_name_contracts_hold` | `not_ignored` |
| `DISC_18C19FAF109755473E29` | `rust_unit_test` | `crates/trust-runtime-core/src/program_model/initializers.rs` | `initializer_catalog_preserves_record_and_type_default_lookup` | `not_ignored` |
| `DISC_6D40BFBAF4EAAA3E07CF` | `rust_unit_test` | `crates/trust-runtime-core/src/program_model/ops.rs` | `non_numeric_comparisons_preserve_runtime_contract` | `not_ignored` |
| `DISC_541AE5910E7918B284BF` | `rust_unit_test` | `crates/trust-runtime-core/src/program_model/ops.rs` | `numeric_ops_preserve_checked_runtime_contract` | `not_ignored` |
| `DISC_A2A766C143194CC4493B` | `rust_unit_test` | `crates/trust-runtime-core/src/program_model/util.rs` | `property_setter_names_keep_hidden_prefix_contract` | `not_ignored` |
| `DISC_9054611BF656BB57E6D9` | `rust_unit_test` | `crates/trust-runtime-core/src/program_model/util.rs` | `static_storage_names_keep_existing_prefix_contract` | `not_ignored` |
| `DISC_29ADADCF1C76C91107AB` | `rust_unit_test` | `crates/trust-runtime-core/src/retain.rs` | `retain_policy_preserves_default_and_warm_restart_contract` | `not_ignored` |
| `DISC_EE2DE2F42D3ED0AFDBD9` | `rust_unit_test` | `crates/trust-runtime-core/src/retain.rs` | `retain_snapshot_preserves_insert_order_and_values` | `not_ignored` |
| `DISC_24468EFBB50806F0709E` | `rust_unit_test` | `crates/trust-runtime-core/src/scaffold.rs` | `scaffold_stage_is_pre_move` | `not_ignored` |
| `DISC_F7C7D0DE2F4FBC9DE014` | `rust_unit_test` | `crates/trust-runtime-core/src/scheduler.rs` | `resource_state_preserves_default_and_lifecycle_order_contract` | `not_ignored` |
| `DISC_8E4DAF369D706056A1CC` | `rust_unit_test` | `crates/trust-runtime-core/src/task.rs` | `task_config_preserves_periodic_and_event_fields` | `not_ignored` |
| `DISC_CD6A594B4D0B29C8C3AD` | `rust_unit_test` | `crates/trust-runtime-core/src/task.rs` | `task_readiness_coalesces_forward_jump_after_host_pause` | `not_ignored` |
| `DISC_0D4BF89810AFBF92D9ED` | `rust_unit_test` | `crates/trust-runtime-core/src/task.rs` | `task_readiness_ignores_backward_clock_step_until_prior_baseline` | `not_ignored` |
| `DISC_A8E6873E91C94109A6C0` | `rust_unit_test` | `crates/trust-runtime-core/src/task.rs` | `task_readiness_prefers_earlier_due_time_when_event_and_periodic_overlap` | `not_ignored` |
| `DISC_F9FC5A9157982659A367` | `rust_unit_test` | `crates/trust-runtime-core/src/task.rs` | `task_readiness_tracks_event_edges_without_repeating_high_level` | `not_ignored` |
| `DISC_6F2849AC1484DA9AB8A8` | `rust_unit_test` | `crates/trust-runtime-core/src/task.rs` | `task_readiness_tracks_periodic_due_time_and_overrun` | `not_ignored` |
| `DISC_20C7A00A677C7B66E53F` | `rust_unit_test` | `crates/trust-runtime-core/src/value/datetime.rs` | `combine_date_and_tod_rejects_timezone_metadata` | `not_ignored` |
| `DISC_B16B2E4975EE4298C84D` | `rust_unit_test` | `crates/trust-runtime-core/src/value/datetime.rs` | `date_time_ticks_and_long_values_round_trip` | `not_ignored` |
| `DISC_9C18A14B3C03C130F6A9` | `rust_unit_test` | `crates/trust-runtime-core/src/value/datetime.rs` | `duration_preserves_nanosecond_and_millisecond_views` | `not_ignored` |
| `DISC_06DC15AD820C31D8EF00` | `rust_unit_test` | `crates/trust-runtime-core/src/value/datetime.rs` | `tick_conversion_rejects_out_of_range_values` | `not_ignored` |
| `DISC_10AF4254EC4ADCF69C33` | `rust_unit_test` | `crates/trust-runtime-core/src/value/defaults.rs` | `defaults_for_core_elementary_values_match_runtime_contract` | `not_ignored` |
| `DISC_CFF3746E003C4AA060F1` | `rust_unit_test` | `crates/trust-runtime-core/src/value/defaults.rs` | `defaults_reject_unknown_type_ids` | `not_ignored` |
| `DISC_466DB0CC95D9F0DC137D` | `rust_unit_test` | `crates/trust-runtime-core/src/value/defaults.rs` | `interface_defaults_to_explicit_null_reference` | `not_ignored` |
| `DISC_9B52EF2D81BCA8CD8A8B` | `rust_unit_test` | `crates/trust-runtime-core/src/value/partial_access.rs` | `reads_partial_bits_and_words_with_bounds_errors` | `not_ignored` |
| `DISC_A0C636EB85EAC33BCC35` | `rust_unit_test` | `crates/trust-runtime-core/src/value/partial_access.rs` | `writes_partial_bits_and_bytes_without_touching_other_bits` | `not_ignored` |
| `DISC_21EFDB47ADA7E3AA6A24` | `rust_unit_test` | `crates/trust-runtime-core/src/value/reference.rs` | `array_offset_handles_extreme_bounds_without_overflow` | `not_ignored` |
| `DISC_777A9BC260F88071F3F2` | `rust_unit_test` | `crates/trust-runtime-core/src/value/reference.rs` | `checked_array_offset_preserves_bounds_error` | `not_ignored` |
| `DISC_7A5735CC670818E7B6B3` | `rust_unit_test` | `crates/trust-runtime-core/src/value/reference.rs` | `common_ref_path_helpers_preserve_segment_order` | `not_ignored` |
| `DISC_19F83BC6AA57441C4B3F` | `rust_unit_test` | `crates/trust-runtime-core/src/value/reference.rs` | `partial_access_parser_accepts_iec_suffixes_and_bare_bits` | `not_ignored` |
| `DISC_2A71C823139F87D5184A` | `rust_unit_test` | `crates/trust-runtime-core/src/value/size.rs` | `string_value_size_counts_character_elements` | `not_ignored` |
| `DISC_57FB35474C4EE46899FD` | `rust_unit_test` | `crates/trust-runtime-core/src/value/string_semantics.rs` | `narrow_string_index_reads_and_writes_single_byte_chars` | `not_ignored` |
| `DISC_EF1816FF5BB0935AC957` | `rust_unit_test` | `crates/trust-runtime-core/src/value/string_semantics.rs` | `narrow_string_index_rejects_out_of_range_chars` | `not_ignored` |
| `DISC_C914D2330D1D8FD81820` | `rust_unit_test` | `crates/trust-runtime-core/src/value/string_semantics.rs` | `narrow_string_semantics_count_elements_not_utf8_bytes` | `not_ignored` |
| `DISC_AB054456F6E59419DEFA` | `rust_unit_test` | `crates/trust-runtime-core/src/value/string_semantics.rs` | `wide_string_index_reads_and_writes_unicode_scalar_elements` | `not_ignored` |
| `DISC_B3C963C8C11F4A16A1C6` | `rust_unit_test` | `crates/trust-runtime-core/src/value/types/tests.rs` | `array_value_clone_and_equality_preserve_shape_and_elements` | `not_ignored` |
| `DISC_79387EEE458FEB850E9E` | `rust_unit_test` | `crates/trust-runtime-core/src/value/types/tests.rs` | `array_value_mutators_preserve_shape_contract` | `not_ignored` |
| `DISC_ECABDC048A89F3E0411B` | `rust_unit_test` | `crates/trust-runtime-core/src/value/types/tests.rs` | `array_value_new_canonicalizes_alias_and_rejects_shape_or_type_drift` | `not_ignored` |
| `DISC_D2A232D405F0370886BF` | `rust_unit_test` | `crates/trust-runtime-core/src/value/types/tests.rs` | `array_value_new_validates_array_of_struct_elements` | `not_ignored` |
| `DISC_9A1D90DEEC7083B44DED` | `rust_unit_test` | `crates/trust-runtime-core/src/value/types/tests.rs` | `enum_value_from_serialized_parts_canonicalizes_and_validates_numeric_value` | `not_ignored` |
| `DISC_F86EF88A024AC5ED9B9E` | `rust_unit_test` | `crates/trust-runtime-core/src/value/types/tests.rs` | `enum_value_new_resolves_alias_to_canonical_enum_type` | `not_ignored` |
| `DISC_9CDCE19E230C7B923BF7` | `rust_unit_test` | `crates/trust-runtime-core/src/value/types/tests.rs` | `interface_type_accepts_null_and_instance_values` | `not_ignored` |
| `DISC_87A93280853227BA7711` | `rust_unit_test` | `crates/trust-runtime-core/src/value/types/tests.rs` | `normalize_assignment_materializes_safe_numeric_widening_tags` | `not_ignored` |
| `DISC_F0334E65106785B971B6` | `rust_unit_test` | `crates/trust-runtime-core/src/value/types/tests.rs` | `struct_value_clone_and_equality_preserve_field_identity` | `not_ignored` |
| `DISC_1E8C1A88234EE493A54F` | `rust_unit_test` | `crates/trust-runtime-core/src/value/types/tests.rs` | `struct_value_mutator_updates_existing_fields_only` | `not_ignored` |
| `DISC_FEE0FCC8ED6D6FC3B206` | `rust_unit_test` | `crates/trust-runtime-core/src/value/types/tests.rs` | `struct_value_new_canonicalizes_alias_fields_and_rejects_type_drift` | `not_ignored` |
| `DISC_B00E9C0F09D249210AD0` | `rust_unit_test` | `crates/trust-runtime-core/src/vm/mod.rs` | `frame_stack_preserves_lifo_and_call_depth_contracts` | `not_ignored` |
| `DISC_9C72DD2482F10E7D5AEB` | `rust_unit_test` | `crates/trust-runtime-core/src/vm/mod.rs` | `operand_stack_preserves_lifo_pair_and_swap_contracts` | `not_ignored` |
| `DISC_1C96EBCA5E35D76524D9` | `rust_unit_test` | `crates/trust-runtime-core/src/vm/mod.rs` | `vm_const_pool_decoder_preserves_primitive_enum_and_alias_contracts` | `not_ignored` |
| `DISC_1AC3BE6D73E6E539D693` | `rust_unit_test` | `crates/trust-runtime-core/src/vm/mod.rs` | `vm_const_pool_decoder_rejects_bad_payload_and_type_shapes` | `not_ignored` |
| `DISC_7636C85FE76F5DB55BB4` | `rust_unit_test` | `crates/trust-runtime-core/src/vm/mod.rs` | `vm_dispatch_ops_preserve_stack_jump_and_operand_decode_contracts` | `not_ignored` |
| `DISC_581C93F7374276CD8F2E` | `rust_unit_test` | `crates/trust-runtime-core/src/vm/mod.rs` | `vm_frame_preserves_local_slot_bounds_and_materialization_contracts` | `not_ignored` |
| `DISC_7860D27AECD7349B4C5A` | `rust_unit_test` | `crates/trust-runtime-core/src/vm/mod.rs` | `vm_helpers_preserve_opcode_and_borrow_materialization_contracts` | `not_ignored` |
| `DISC_1869429B84BBC37F3DA2` | `rust_unit_test` | `crates/trust-runtime-core/src/vm/mod.rs` | `vm_sizeof_helpers_preserve_type_table_contracts` | `not_ignored` |
| `DISC_0182F7ACD68B4D33DF7A` | `rust_unit_test` | `crates/trust-runtime-core/src/vm/mod.rs` | `vm_trap_preserves_runtime_error_mapping` | `not_ignored` |
| `DISC_A35DCA0B1CE21BC721A6` | `rust_unit_test` | `crates/trust-runtime-core/src/watchdog.rs` | `enabled_watchdog_policy_normalizes_zero_timeout` | `not_ignored` |
| `DISC_4C7C595026E1FD708546` | `rust_unit_test` | `crates/trust-runtime-core/src/watchdog.rs` | `watchdog_and_fault_policy_decisions_are_stable` | `not_ignored` |
| `DISC_DBFE8331AF87D467BFB3` | `rust_unit_test` | `crates/trust-runtime-core/src/watchdog.rs` | `watchdog_and_fault_subsystems_preserve_state_contracts` | `not_ignored` |
| `DISC_6CD92085A672A8B2C395` | `rust_unit_test` | `crates/trust-runtime-core/src/watchdog.rs` | `watchdog_policy_default_is_disabled_safe_halt` | `not_ignored` |
| `DISC_758E974EC4C2E0D330B5` | `rust_unit_test` | `crates/trust-runtime-core/src/watchdog.rs` | `watchdog_retain_and_fault_policy_parsers_match_config_contracts` | `not_ignored` |
| `DISC_85C67E74CD7EE13584EC` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/ads.rs` | `ads_import_refuses_to_overwrite_changed_source_without_force` | `not_ignored` |
| `DISC_843D6E3A504812FC041C` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/ads.rs` | `ads_import_writes_and_validates_generated_source_offline` | `not_ignored` |
| `DISC_F47D11C740336763E06D` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/bench/tests.rs` | `dispatch_bench_table_output_contains_fanout_and_audit_metrics` | `not_ignored` |
| `DISC_E16E1C842329936229F2` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/bench/tests.rs` | `histogram_includes_overflow_bucket` | `not_ignored` |
| `DISC_0B58E404259C24FF2228` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/bench/tests.rs` | `init_bench_json_output_contains_startup_latency_fields` | `not_ignored` |
| `DISC_484F59ADC1000039905D` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/bench/tests.rs` | `mesh_workload_rejects_out_of_range_rates` | `not_ignored` |
| `DISC_D7993C135016DD61B7C6` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/bench/tests.rs` | `mesh_zenoh_bench_json_output_contains_loss_and_reorder_fields` | `not_ignored` |
| `DISC_1345A0C877D39456F030` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/bench/tests.rs` | `project_bench_json_output_contains_budget_and_watched_globals` | `not_ignored` |
| `DISC_C31463CD233F1ACDB756` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/bench/tests.rs` | `project_bench_json_output_omits_tier1_executor_stats_by_default` | `not_ignored` |
| `DISC_DDD42B78809E4A86096E` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/bench/tests.rs` | `project_bench_output_contains_tier1_compile_failure_reasons` | `not_ignored` |
| `DISC_53D4A7CA606AD4638ADC` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/bench/tests.rs` | `project_bench_table_output_contains_tier1_executor_stats_when_enabled` | `not_ignored` |
| `DISC_304148B8830BC92C411B` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/bench/tests.rs` | `project_workload_rejects_missing_project_folder` | `not_ignored` |
| `DISC_AC58EF6C393A0C878A3E` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/bench/tests.rs` | `project_workload_rejects_zero_samples` | `not_ignored` |
| `DISC_86F0B6FEB1C15C695FA7` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/bench/tests.rs` | `summarize_ns_computes_quantiles` | `not_ignored` |
| `DISC_C7955D838D665531D4E1` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/bench/tests.rs` | `t0_shm_bench_json_output_contains_latency_and_overrun_fields` | `not_ignored` |
| `DISC_E460255CD7B49B3642C9` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/ci.rs` | `classify_build_failure_code` | `not_ignored` |
| `DISC_4F9702E99826A84EDB5F` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/ci.rs` | `classify_internal_code` | `not_ignored` |
| `DISC_D8D72B4595EF10C8BC08` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/ci.rs` | `classify_invalid_config_code` | `not_ignored` |
| `DISC_4EC2BCF75B5E61513924` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/ci.rs` | `classify_test_failure_code` | `not_ignored` |
| `DISC_9157E4205B877173F071` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/ci.rs` | `classify_timeout_code` | `not_ignored` |
| `DISC_F78A146E0A56A995782C` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/ci.rs` | `classify_with_command_falls_back_for_internal` | `not_ignored` |
| `DISC_A95D4B4BCB8D35800A86` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_ads_add_route_password_stdin_command` | `not_ignored` |
| `DISC_46DACC7E48BBDC5C531F` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_ads_add_route_requires_no_password_argument` | `not_ignored` |
| `DISC_D96D92F4C86DFE0FCD13` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_ads_browse_command` | `not_ignored` |
| `DISC_69AD36A7A2A98E2859DE` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_ads_discover_command` | `not_ignored` |
| `DISC_599A651C94F4B8F1A6F9` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_ads_doctor_command` | `not_ignored` |
| `DISC_61C813EDB460F6E9100B` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_ads_doctor_guarded_write_probe_command` | `not_ignored` |
| `DISC_4FE6FED1EBCF1EE95C22` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_ads_import_command` | `not_ignored` |
| `DISC_E25566DAD91C9F28AE8F` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_ads_import_symbols_command` | `not_ignored` |
| `DISC_1691B3B84A0BBDCA3614` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_ads_route_remove_command` | `not_ignored` |
| `DISC_815A8962E2B241E7D833` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_ads_route_script_command` | `not_ignored` |
| `DISC_720B6B368E697E6B906C` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_ads_server_doctor_external_evidence_command` | `not_ignored` |
| `DISC_2E0A7F7A097414C587CD` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_ads_server_route_script_command` | `not_ignored` |
| `DISC_5E4438BEDD75DD2138F5` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_ads_server_status_command` | `not_ignored` |
| `DISC_B9A5F7EE8D91C3695562` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_ads_server_symbols_command` | `not_ignored` |
| `DISC_1AEAA1027B1757F68626` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_ads_validate_live_command` | `not_ignored` |
| `DISC_D2271C8F01D4CF4DF239` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_ads_validate_offline_command` | `not_ignored` |
| `DISC_78B109A95193A3453485` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_agent_serve_command` | `not_ignored` |
| `DISC_DFC80F3AC267EA958402` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_bench_mesh_zenoh_command` | `not_ignored` |
| `DISC_BCFDD40A0C59D9C46709` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_bench_project_command` | `not_ignored` |
| `DISC_07458BC787B61B34DF2F` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_bench_project_command_with_tier1` | `not_ignored` |
| `DISC_6C1DE56E06CAA87A99AD` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_bench_t0_shm_command` | `not_ignored` |
| `DISC_3E24745EDD37F61AE492` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_build_ci_flag` | `not_ignored` |
| `DISC_B4945DE758C04096C82C` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_check_json_and_ci_flags` | `not_ignored` |
| `DISC_EBE5388114C3775A84B9` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_comm_apply_command` | `not_ignored` |
| `DISC_8536F588F5A64F2886AF` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_comm_browse_symbols_command` | `not_ignored` |
| `DISC_05B1A42E1C02DD24C690` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_comm_discover_command` | `not_ignored` |
| `DISC_046DBAC5114867D5A4A9` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_comm_schema_protocol_filter_command` | `not_ignored` |
| `DISC_EFDA5D336AF7B6C6CDDC` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_config_ui_serve_command` | `not_ignored` |
| `DISC_5E259824ED1B08ADD052` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_conformance_command` | `not_ignored` |
| `DISC_A780A932AA1245866343` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_docs_command` | `not_ignored` |
| `DISC_F95DF735E782C1FFF3A2` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_fleet_list_command` | `not_ignored` |
| `DISC_24CAD09D429B178C5142` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_fleet_runtime_add_command` | `not_ignored` |
| `DISC_6B637583A37DE194C8B5` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_fleet_runtime_lifecycle_commands` | `not_ignored` |
| `DISC_26CB71E7EB9AFB5A69B9` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_hmi_init_command` | `not_ignored` |
| `DISC_AF455501E8518CEAB703` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_hmi_reset_command` | `not_ignored` |
| `DISC_EA19A368372ED6E20DFA` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_hmi_update_command` | `not_ignored` |
| `DISC_9AAF5CBFE67E2EDBAC33` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_ide_serve_command` | `not_ignored` |
| `DISC_BF7B13205AB37D264FBA` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_play_execution_backend_rejects_interpreter_flag` | `not_ignored` |
| `DISC_F17D6CED661DFFBBF8AB` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_play_simulation_flags` | `not_ignored` |
| `DISC_B8F497BF1C09A994972B` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_plcopen_export_command` | `not_ignored` |
| `DISC_2FB22EB8DBED81E65CC5` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_plcopen_export_target_command` | `not_ignored` |
| `DISC_B949B4DB69A7CBD4B4C3` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_plcopen_import_command` | `not_ignored` |
| `DISC_1F4D63D96B56673D2897` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_registry_private_init_command` | `not_ignored` |
| `DISC_C31ECFAB52CC964E85A2` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_run_execution_backend_flag` | `not_ignored` |
| `DISC_93C2C9528A08F43AB52D` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_run_execution_backend_rejects_interpreter_flag` | `not_ignored` |
| `DISC_481DEF56B45C16EC6E4A` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_setup_cancel_mode` | `not_ignored` |
| `DISC_6104838CB207FB98C2A4` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_test_ci_flag` | `not_ignored` |
| `DISC_F58BCD751504115571EB` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `parse_validate_ci_flag` | `not_ignored` |
| `DISC_F5FBFB925CD26F2D0DED` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | `runtime_help_names_trust_dev_workbench_commands_and_removal_window` | `not_ignored` |
| `DISC_DBB8F51366F4AF107241` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/conformance/tests.rs` | `case_id_validation_matches_naming_rules` | `not_ignored` |
| `DISC_85B7540565F4E1069033` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/conformance/tests.rs` | `parse_typed_values_supports_core_manifest_types` | `not_ignored` |
| `DISC_2CA0AB30FF939EE75DBE` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/conformance/tests.rs` | `summary_contract_moves_to_v2_for_expanded_categories` | `not_ignored` |
| `DISC_855A1CF3F17E6F840144` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/conformance/tests.rs` | `summary_contract_remains_v1_for_legacy_category_only_suites` | `not_ignored` |
| `DISC_41FE62340968DB3F5242` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/conformance/tests.rs` | `unix_split_produces_epoch` | `not_ignored` |
| `DISC_24384BED6AEB52A7F254` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/fleet.rs` | `empty_template_uses_valid_loopback_io` | `not_ignored` |
| `DISC_98A2692A0CDA76565FE7` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/fleet.rs` | `fleet_runtime_add_creates_simulated_project_and_manifest` | `not_ignored` |
| `DISC_A7DD53CC8FD10F25B5E4` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/fleet.rs` | `fleet_runtime_add_rejects_any_manifest_port_collision` | `not_ignored` |
| `DISC_0BCC1154B4C4689BF731` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/fleet.rs` | `fleet_runtime_add_rejects_duplicate_name_without_rewriting` | `not_ignored` |
| `DISC_BD46DE8F7A7923BE2A3F` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/lifecycle.rs` | `fleet_runtime_logs_return_requested_tail` | `not_ignored` |
| `DISC_EA1F36511801C6B5FC9E` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/lifecycle.rs` | `fleet_runtime_status_reports_stopped_when_endpoint_unreachable` | `not_ignored` |
| `DISC_DC83ED83D64D175E03A7` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/run/tests.rs` | `ads_runtime_start_spawns_worker_and_scan_applies_mock_data` | `not_ignored` |
| `DISC_4B4E5752827124306312` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/run/tests.rs` | `execution_backend_selection_cli_overrides_bundle` | `not_ignored` |
| `DISC_F6F2D8C0F05DA912D458` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/run/tests.rs` | `execution_backend_selection_defaults_to_vm` | `not_ignored` |
| `DISC_F87A85B1DA5F238164FF` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/run/tests.rs` | `execution_backend_selection_prefers_cli_override` | `not_ignored` |
| `DISC_04C3E5C754B5BACD20F6` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/run/tests.rs` | `execution_backend_selection_uses_bundle_when_cli_absent` | `not_ignored` |
| `DISC_5DFDEC6B9BF7CE3038B7` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/run/tests.rs` | `modbus_io_source_label_uses_register_direction` | `not_ignored` |
| `DISC_2AD9428FF97F3FA04D55` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/run/tests.rs` | `mqtt_io_source_label_uses_directional_topic` | `not_ignored` |
| `DISC_4685ADC459FF023B939A` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/run/tests.rs` | `project_runtime_load_includes_local_dependencies` | `not_ignored` |
| `DISC_63C4C5C26503EF8BCCB4` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/run/tests.rs` | `signal_abstraction_requests_regular_runtime_shutdown` | `not_ignored` |
| `DISC_0DDFCA476A330F0BE047` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/run/tests.rs` | `simulation_warning_includes_mode_and_safety_note` | `not_ignored` |
| `DISC_3C5AC4C3361E223612AF` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/run/tests.rs` | `simulation_warning_omitted_in_production_mode` | `not_ignored` |
| `DISC_DFC00804D131838F18B2` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/run/tests.rs` | `startup_retain_load_respects_restart_mode` | `not_ignored` |
| `DISC_598D28DD4E4B50652577` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/run/tests.rs` | `unix_signal_mapping_covers_sigint_and_sigterm` | `not_ignored` |
| `DISC_57D298DD438F5E6F5975` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/run/tests.rs` | `unix_signal_mapping_rejects_unreviewed_shutdown_signals` | `not_ignored` |
| `DISC_1824299BA46AC857D6B7` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/setup/tests.rs` | `browser_profile_local_enforces_loopback_and_no_token` | `not_ignored` |
| `DISC_D913D8612C7C76CAA539` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/setup/tests.rs` | `browser_profile_remote_requires_non_loopback_and_token_ttl` | `not_ignored` |
| `DISC_796AD132D4C3D75B7A4A` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/setup_web.rs` | `apply_setup_persists_runtime_artifacts` | `not_ignored` |
| `DISC_CC1AD4BF41A52AFCB096` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/setup_web.rs` | `setup_access_allows_when_token_not_required` | `not_ignored` |
| `DISC_D186E0054EA8B270621D` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/setup_web.rs` | `setup_access_enforces_expiry` | `not_ignored` |
| `DISC_96EA4541C1842E8410D3` | `rust_unit_test` | `crates/trust-runtime/src/bin/trust-runtime/setup_web.rs` | `setup_access_requires_matching_token` | `not_ignored` |
| `DISC_17D696332973EB469E9F` | `rust_unit_test` | `crates/trust-runtime/src/bytecode/validate/resource_limits.rs` | `instruction_limit_accepts_boundary_and_rejects_next_instruction` | `not_ignored` |
| `DISC_730482B298A86765CE71` | `rust_unit_test` | `crates/trust-runtime/src/bytecode/validate/resource_limits.rs` | `stack_limit_accepts_boundary_and_rejects_next_value` | `not_ignored` |
| `DISC_550DA4990A850DF843DF` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `io_schema_accepts_disabled_multi_driver_entries` | `not_ignored` |
| `DISC_CB9B91A0E26106BEFDA2` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `io_schema_accepts_driver_alias_inside_multi_driver_entries` | `not_ignored` |
| `DISC_C152A2FAE2D0D3C74BD8` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `io_schema_accepts_multiple_drivers` | `not_ignored` |
| `DISC_4AF6DB3BEA7577D7AFA9` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `io_schema_rejects_empty_multi_driver_list` | `not_ignored` |
| `DISC_0CC3A7C4087E3AA2A1ED` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `io_schema_rejects_mixed_single_and_multi_driver_fields` | `not_ignored` |
| `DISC_1F840E9260BED8DD2744` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `io_schema_rejects_unknown_keys` | `not_ignored` |
| `DISC_54D25B098B86A44CAAB3` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `io_schema_requires_table_params` | `not_ignored` |
| `DISC_6C9501BC73C1EAACECE1` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `opcua_client_toml_accepts_connection_points` | `not_ignored` |
| `DISC_0E137A539912A47B65AA` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_bundle_loads_ads_toml_when_enabled` | `not_ignored` |
| `DISC_28B1BD23981F3512A2B0` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_bundle_loads_opcua_client_toml_when_enabled` | `not_ignored` |
| `DISC_092CA89ADE8ED2E68BB5` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_bundle_rejects_missing_enabled_ads_toml` | `not_ignored` |
| `DISC_FD76407FFDD9B3B6BB9B` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_bundle_rejects_missing_enabled_opcua_client_toml` | `not_ignored` |
| `DISC_403B3B02FB4161DE7AC2` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_config_load_defaults_execution_backend_source_when_omitted` | `not_ignored` |
| `DISC_F21FDAC05E6EC4A79F61` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_config_load_records_execution_backend_source_from_config` | `not_ignored` |
| `DISC_1A92B0F578B8DC0DED06` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_accepts_ads_section` | `not_ignored` |
| `DISC_09722C5C25D1F7983647` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_accepts_ads_server_empty_fail_closed_runtime` | `not_ignored` |
| `DISC_C94187733AA6247CC447` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_accepts_ads_server_section` | `not_ignored` |
| `DISC_1175EEE248919AE3A199` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_accepts_ads_server_source_cidr_client` | `not_ignored` |
| `DISC_ED1030D1DBFDE8AE1FEF` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_accepts_ads_server_unpinned_clients_with_lab_override` | `not_ignored` |
| `DISC_0E0745F0FA5EFEE0DE4E` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_accepts_discovery_host_group_and_cloud_link_preferences` | `not_ignored` |
| `DISC_504644863A38C9727018` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_accepts_extended_cloud_link_transports` | `not_ignored` |
| `DISC_7AB655BAF940FB462961` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_accepts_hmi_persistence_section` | `not_ignored` |
| `DISC_182D8126262938F884B5` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_accepts_opcua_client_pointer` | `not_ignored` |
| `DISC_4A204805320443DF9552` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_accepts_opcua_secure_profile_with_user_credentials` | `not_ignored` |
| `DISC_A268A04CAD2A5DD42EC3` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_accepts_openot_st_fb_producer_instances` | `not_ignored` |
| `DISC_28C3E4FAE432C44F6DEC` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_accepts_openot_st_fb_source` | `not_ignored` |
| `DISC_A1BB307CBD6543E97A54` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_accepts_openot_telemetry_section` | `not_ignored` |
| `DISC_0405662960DACD7993A1` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_accepts_openot_unfenced_with_proof_opt_in` | `not_ignored` |
| `DISC_F679DD723147AD3D477E` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_accepts_preempt_rt_profile_section` | `not_ignored` |
| `DISC_4F7320235213CE034132` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_accepts_vm_execution_backend` | `not_ignored` |
| `DISC_4F859E1A1B25D2B599A6` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_accepts_web_tls_with_self_managed_cert_paths` | `not_ignored` |
| `DISC_1DDB52B5AC2D7B25C6DD` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_defaults_ads_disabled` | `not_ignored` |
| `DISC_9EFAA9ABCB79F54A1627` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_defaults_execution_backend_when_omitted` | `not_ignored` |
| `DISC_63C256CF98DBC46CDFD6` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_defaults_openot_telemetry_disabled` | `not_ignored` |
| `DISC_9F0787490FFD6691CA71` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_rejects_ads_server_enabled_without_listen` | `not_ignored` |
| `DISC_E16F3DF4CF33C1F4E47C` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_rejects_ads_server_plain_without_ack` | `not_ignored` |
| `DISC_2D0D3C29FED95BAD5CA7` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_rejects_ads_server_public_bind_without_override` | `not_ignored` |
| `DISC_9988B7F333E93FF247AC` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_rejects_ads_server_structured_client_without_source_pin` | `not_ignored` |
| `DISC_417EA9369DFC6305F174` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_rejects_ads_server_unpinned_bare_clients_by_default` | `not_ignored` |
| `DISC_FB189AF8B53A9ACA5316` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_rejects_ads_server_wildcard_listen` | `not_ignored` |
| `DISC_E3AE5E47AB124048A94E` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_rejects_ads_server_writable_not_exposed` | `not_ignored` |
| `DISC_C7CD0E2BD09743A9A515` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_rejects_allowlist_without_patterns` | `not_ignored` |
| `DISC_97F01C37478B90D58DB0` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_rejects_duplicate_openot_st_fb_producer_instances` | `not_ignored` |
| `DISC_FB6AAA82601D2E03DD83` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_rejects_duplicate_realtime_cpu_affinity` | `not_ignored` |
| `DISC_B366EB6DB436744B82DB` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_rejects_empty_cloud_link_source` | `not_ignored` |
| `DISC_EB709EDB9C5543F99BBE` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_rejects_empty_hmi_persistence_window` | `not_ignored` |
| `DISC_BCD49DB646AE38F7AEAC` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_rejects_empty_wan_allow_write_rule_target` | `not_ignored` |
| `DISC_73ED36E12D4A99742020` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_rejects_enabled_openot_without_path` | `not_ignored` |
| `DISC_0BC4EC583C569B8D12F6` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_rejects_enabled_realtime_section_without_fifo_or_rr_scheduler` | `not_ignored` |
| `DISC_1B1B7F9FF75E901293D8` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_rejects_interpreter_execution_backend_for_production` | `not_ignored` |
| `DISC_87F27A3DFEC6758D39DE` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_rejects_invalid_cloud_link_transport` | `not_ignored` |
| `DISC_0FA542ED1A847F540F44` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_rejects_invalid_execution_backend` | `not_ignored` |
| `DISC_C9AB817C3804CDAD2BAF` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_rejects_invalid_ranges` | `not_ignored` |
| `DISC_0F6928F8958A65C48EFA` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_rejects_opcua_endpoint_path_without_leading_slash` | `not_ignored` |
| `DISC_72551A20B1AE70779134` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_rejects_openot_producer_instance_for_heartbeat_source` | `not_ignored` |
| `DISC_563EF04D6FEFB8530861` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_rejects_openot_producer_instances_for_heartbeat_source` | `not_ignored` |
| `DISC_E719E2DAB5B30F5F6D8C` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_rejects_openot_st_fb_both_producer_aliases` | `not_ignored` |
| `DISC_3A153DB10EAF7401F6C8` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_rejects_openot_st_fb_without_producer_instance` | `not_ignored` |
| `DISC_EF3E21C5F3C34B90C2E0` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_rejects_openot_unfenced_without_proof_opt_in` | `not_ignored` |
| `DISC_0B08D7DC07B9B8BC9145` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_rejects_prometheus_path_without_leading_slash` | `not_ignored` |
| `DISC_6579863E14F231336694` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_rejects_provisioned_tls_without_ca_path` | `not_ignored` |
| `DISC_C1ABC331239F1B94DA7B` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_rejects_remote_web_without_tls_when_required` | `not_ignored` |
| `DISC_78139822FEDCB8E1A379` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_rejects_unknown_keys` | `not_ignored` |
| `DISC_E91AC922269A5969DC75` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_rejects_unknown_runtime_cloud_profile` | `not_ignored` |
| `DISC_484F79FC90D8262DC489` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_rejects_unqualified_openot_producer_instance` | `not_ignored` |
| `DISC_9068F7DC1DA9467B09CC` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_rejects_zero_ads_worker_tick` | `not_ignored` |
| `DISC_A05C64AD0252D8DF4AD6` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_rejects_zero_openot_capacity` | `not_ignored` |
| `DISC_41CC7CE651B6B3E80F2F` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_requires_control_auth_for_tcp_endpoints` | `not_ignored` |
| `DISC_37BE7FDC9BD54FF418A4` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_requires_deploy_keyring_when_signed_deploy_enabled` | `not_ignored` |
| `DISC_1BFF32209422803BD36C` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_requires_opcua_credentials_or_anonymous_when_enabled` | `not_ignored` |
| `DISC_2056D2557E1B36DD46A1` | `rust_unit_test` | `crates/trust-runtime/src/config/tests.rs` | `runtime_schema_requires_tls_credentials_when_tls_enabled` | `not_ignored` |
| `DISC_76FBF37CE59AEBC07E5A` | `rust_unit_test` | `crates/trust-runtime/src/control/ads_handlers/import_symbols.rs` | `import_symbols_apply_acknowledged_writable_symbols_as_read_write` | `not_ignored` |
| `DISC_E009E0DDE50DEB5A3A95` | `rust_unit_test` | `crates/trust-runtime/src/control/ads_handlers/import_symbols.rs` | `import_symbols_apply_route_missing_report_carries_route_plan` | `not_ignored` |
| `DISC_E5FEFE8DC596318F2FFF` | `rust_unit_test` | `crates/trust-runtime/src/control/ads_handlers/import_symbols.rs` | `import_symbols_apply_writes_exact_selected_symbols_to_project_files` | `not_ignored` |
| `DISC_3E83F4A2B9B9C082FACB` | `rust_unit_test` | `crates/trust-runtime/src/control/comm_handlers/apply/io_file.rs` | `render_io_toml_adds_nested_drivers_to_safe_state_only_file` | `not_ignored` |
| `DISC_582131201CEABD9E328A` | `rust_unit_test` | `crates/trust-runtime/src/control/comm_handlers/apply/validation.rs` | `ethercat_selected_channels_is_an_allowed_array_param` | `not_ignored` |
| `DISC_0308B0ECC8880250F9CC` | `rust_unit_test` | `crates/trust-runtime/src/control/comm_handlers/apply/validation.rs` | `mqtt_broker_validation_uses_mqtt_port_example` | `not_ignored` |
| `DISC_9F24EED7F5885A7C98EB` | `rust_unit_test` | `crates/trust-runtime/src/control/comm_handlers/apply/validation.rs` | `mqtt_mtls_cert_and_key_are_validated_together` | `not_ignored` |
| `DISC_9184E80A519CE2602578` | `rust_unit_test` | `crates/trust-runtime/src/control/comm_handlers/apply/validation.rs` | `mqtt_tls_fields_require_tls_enabled` | `not_ignored` |
| `DISC_2470C2CC6EB8FFE1AA3F` | `rust_unit_test` | `crates/trust-runtime/src/control/comm_handlers/apply/validation.rs` | `mqtt_tls_without_ca_path_is_field_specific` | `not_ignored` |
| `DISC_182C09A5F08A014AE5A8` | `rust_unit_test` | `crates/trust-runtime/src/control/comm_handlers/browse_symbols.rs` | `ads_browse_target_accepts_project_target_net_id_alias` | `not_ignored` |
| `DISC_E5ED2585FE5C4C5F8AC6` | `rust_unit_test` | `crates/trust-runtime/src/control/comm_handlers/browse_symbols.rs` | `ads_cached_snapshot_returns_tree_and_existing_import_shape` | `not_ignored` |
| `DISC_EB106B12C640EB64DFF4` | `rust_unit_test` | `crates/trust-runtime/src/control/comm_handlers/browse_symbols.rs` | `ads_symbol_upload_timeout_returns_route_missing_response` | `not_ignored` |
| `DISC_207001D0F6692D151B6A` | `rust_unit_test` | `crates/trust-runtime/src/control/comm_handlers/browse_symbols.rs` | `ethercat_channel_browse_returns_configured_module_channels` | `not_ignored` |
| `DISC_1056BAD010C75259B977` | `rust_unit_test` | `crates/trust-runtime/src/control/comm_handlers/browse_symbols.rs` | `local_project_symbol_picker_returns_declared_globals` | `not_ignored` |
| `DISC_4FF3F269F0E7DDA49B63` | `rust_unit_test` | `crates/trust-runtime/src/control/comm_handlers/browse_symbols.rs` | `opcua_client_browse_error_response_carries_structured_code` | `not_ignored` |
| `DISC_3508EFC2ACC5AC3036EC` | `rust_unit_test` | `crates/trust-runtime/src/control/comm_handlers/browse_symbols.rs` | `opcua_client_browse_leaf_exposes_raw_node_id_and_apply_data_type` | `not_ignored` |
| `DISC_0F59C6B5C25634381AF9` | `rust_unit_test` | `crates/trust-runtime/src/control/comm_handlers/contract.rs` | `communication_contract_serializes_stable_status_and_action_ids` | `not_ignored` |
| `DISC_51426D3B81C90181EA47` | `rust_unit_test` | `crates/trust-runtime/src/control/comm_handlers/discover_tests.rs` | `ethercat_discovery_requires_runtime_origin` | `not_ignored` |
| `DISC_CE734C80134AC815A815` | `rust_unit_test` | `crates/trust-runtime/src/control/comm_handlers/discover_tests.rs` | `known_unimplemented_protocol_returns_warning_not_error` | `not_ignored` |
| `DISC_AAC24666A37789CB72A0` | `rust_unit_test` | `crates/trust-runtime/src/control/comm_handlers/discover_tests.rs` | `modbus_discovery_rejects_large_cidr` | `not_ignored` |
| `DISC_DD34C44DC5B46FEE53AD` | `rust_unit_test` | `crates/trust-runtime/src/control/comm_handlers/discover_tests.rs` | `modbus_discovery_reports_tcp_listener_as_port_reachable_only` | `not_ignored` |
| `DISC_E34A3761D137CCCB425B` | `rust_unit_test` | `crates/trust-runtime/src/control/comm_handlers/discover_tests.rs` | `opcua_discovery_is_server_only_warning` | `not_ignored` |
| `DISC_81E439A8EA2F04181A77` | `rust_unit_test` | `crates/trust-runtime/src/control/comm_handlers/discover_tests.rs` | `runtime_ethercat_discovery_reports_mock_bus_modules` | `not_ignored` |
| `DISC_1EC9ED349ADB5937ED4B` | `rust_unit_test` | `crates/trust-runtime/src/control/comm_handlers/discover_tests.rs` | `targeted_mqtt_discovery_reports_tcp_listener_as_port_reachable_only` | `not_ignored` |
| `DISC_6EC95CC15CB8EE065C5B` | `rust_unit_test` | `crates/trust-runtime/src/control/comm_handlers/discover_tests.rs` | `unknown_protocol_still_errors` | `not_ignored` |
| `DISC_BE2E7DD0C24F76859578` | `rust_unit_test` | `crates/trust-runtime/src/control/comm_handlers/discovery_probe.rs` | `modbus_device_id_probe_confirms_protocol_response` | `not_ignored` |
| `DISC_21731A12FF8ED9E6061D` | `rust_unit_test` | `crates/trust-runtime/src/control/comm_handlers/discovery_probe.rs` | `modbus_safe_read_probe_confirms_when_device_id_is_unavailable` | `not_ignored` |
| `DISC_525B0CAA2D24D97F68E3` | `rust_unit_test` | `crates/trust-runtime/src/control/comm_handlers/discovery_probe.rs` | `mqtt_probe_classifies_auth_rejected_connack_separately` | `not_ignored` |
| `DISC_BDBE504F0C4ECC88ABE7` | `rust_unit_test` | `crates/trust-runtime/src/control/comm_handlers/discovery_probe.rs` | `mqtt_probe_uses_clean_session_and_disconnects_after_connack` | `not_ignored` |
| `DISC_D3689261B07904BDAAE3` | `rust_unit_test` | `crates/trust-runtime/src/control/comm_handlers/probe.rs` | `modbus_probe_reports_tcp_success_and_failure` | `not_ignored` |
| `DISC_809B92F6AB3FEEA8E167` | `rust_unit_test` | `crates/trust-runtime/src/control/comm_handlers/probe.rs` | `mqtt_probe_reports_tcp_success_and_field_failures` | `not_ignored` |
| `DISC_01D6D1A9FD91E06876E4` | `rust_unit_test` | `crates/trust-runtime/src/control/comm_handlers/probe.rs` | `opcua_client_probe_reports_structured_unreachable_error` | `not_ignored` |
| `DISC_F5194DD64AD1E6467153` | `rust_unit_test` | `crates/trust-runtime/src/control/comm_handlers/schema.rs` | `native_schema_protocols_match_io_driver_contract_names` | `not_ignored` |
| `DISC_3B655BA3C32E90F046DE` | `rust_unit_test` | `crates/trust-runtime/src/control/comm_handlers/schema.rs` | `schema_defaults_cover_runtime_io_contract_fields` | `not_ignored` |
| `DISC_E62E2952FF182DD28C98` | `rust_unit_test` | `crates/trust-runtime/src/control/comm_handlers/schema.rs` | `schema_v4_exposes_categories_config_homes_and_ads_protocols_without_profiles` | `not_ignored` |
| `DISC_7A5A4A61FEA7C1CD98DF` | `rust_unit_test` | `crates/trust-runtime/src/control/fleet_handlers/tests.rs` | `ads_server_params_humanize_allowed_clients_without_raw_pin_objects` | `not_ignored` |
| `DISC_EE2684C84A8E510D7E97` | `rust_unit_test` | `crates/trust-runtime/src/control/fleet_handlers/tests.rs` | `host_name_normalization_trims_whitespace_and_trailing_dot` | `not_ignored` |
| `DISC_11EF0C4F73B1544D8B3F` | `rust_unit_test` | `crates/trust-runtime/src/control/fleet_handlers/tests.rs` | `host_name_uses_os_hostname_before_literal_fallback` | `not_ignored` |
| `DISC_A2D72CD1E66CDA51A5A6` | `rust_unit_test` | `crates/trust-runtime/src/control/operation_registry.rs` | `debug_surface_classification_is_owned_by_the_operation_registry` | `not_ignored` |
| `DISC_E0EA51662EC8697B47D6` | `rust_unit_test` | `crates/trust-runtime/src/control/operation_registry.rs` | `reviewed_operation_registry_has_unique_names_and_default_deny_boundary` | `not_ignored` |
| `DISC_B4CE6789A4A5D02DE0ED` | `rust_unit_test` | `crates/trust-runtime/src/control/policy.rs` | `ads_connector_status_and_ads_mutations_keep_role_boundaries` | `not_ignored` |
| `DISC_E089E5D43399F96CC63F` | `rust_unit_test` | `crates/trust-runtime/src/control/policy.rs` | `ads_import_symbols_requires_engineer_for_live_target_only` | `not_ignored` |
| `DISC_151AF83E7ACDF303EB01` | `rust_unit_test` | `crates/trust-runtime/src/control/policy.rs` | `comm_browse_symbols_requires_engineer_for_live_target_only` | `not_ignored` |
| `DISC_68DA65888D7C2439D11F` | `rust_unit_test` | `crates/trust-runtime/src/control/policy.rs` | `connectors_status_requires_viewer_role` | `not_ignored` |
| `DISC_A45E3E19BAC099017A19` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/ads_product_surfaces.rs` | `generated_ads_globals_are_first_class_product_surface_variables` | `not_ignored` |
| `DISC_85C1B6864B76D834F59A` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/audit_durability.rs` | `control_audit_send_failure_records_audit_dropped_event` | `not_ignored` |
| `DISC_C2C7721DF77E1F95004E` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/audit_durability.rs` | `debug_feature_disabled_returns_structured_feature_disabled_response` | `not_ignored` |
| `DISC_2CAF3FB378ED1968BC05` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/connectors.rs` | `connectors_status_ads_projection_matches_legacy_ads_status` | `not_ignored` |
| `DISC_4C19A9BC8EB40CFF9C71` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/connectors.rs` | `connectors_status_authz_requires_viewer_and_preserves_local_unix_read` | `not_ignored` |
| `DISC_59437AD1E0698611D045` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/connectors.rs` | `connectors_status_reports_ads_client_and_server_roles` | `not_ignored` |
| `DISC_4FD6BEEC35F23C0FE962` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/connectors.rs` | `connectors_status_reports_opcua_client_points_with_quality` | `not_ignored` |
| `DISC_A4B53D5EAD921AF1BEA5` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/connectors.rs` | `connectors_status_reports_process_image_drivers_without_mutating_legacy_status` | `not_ignored` |
| `DISC_98C072DEE99220B94B98` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `ads_control_requests_reject_missing_params_for_parameterized_commands` | `not_ignored` |
| `DISC_7CFA25E00041BF572099` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `ads_discover_control_request_reports_wire_requirement_without_manual_ams_id` | `not_ignored` |
| `DISC_0C0E896B727531F1A82B` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `ads_discover_control_request_supports_manual_target_without_broadcast` | `not_ignored` |
| `DISC_A542BF00CD94824D5DB6` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `ads_doctor_control_request_reports_wire_requirement_without_ads_wire` | `not_ignored` |
| `DISC_D23A413C4CDBA3912B2A` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `ads_doctor_start_and_status_expose_failed_job_without_ads_wire` | `not_ignored` |
| `DISC_CE772515BBC4E8ED75DB` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `ads_doctor_status_reports_unknown_job` | `not_ignored` |
| `DISC_5AE966D7C1D799B3E57B` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `ads_identity_control_request_derives_runtime_host_source_identity` | `not_ignored` |
| `DISC_0A2739933846E78D331C` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `ads_identity_control_request_rejects_invalid_target_ip` | `not_ignored` |
| `DISC_60B5CBBD1ECE5C9944FA` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `ads_import_symbols_control_request_shapes_cached_snapshot` | `not_ignored` |
| `DISC_A883DECF8E1190C71056` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `ads_import_symbols_live_without_wire_reports_requirement` | `not_ignored` |
| `DISC_C208A78C1E567F7718E8` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `ads_import_symbols_rejects_snapshot_for_different_connection` | `not_ignored` |
| `DISC_520566DBDE6E2D534351` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `ads_route_add_rejects_untrusted_channel_without_echoing_secret` | `not_ignored` |
| `DISC_BA0F502408090591CE25` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `ads_route_add_trusted_channel_reports_wire_requirement_without_echoing_secret` | `not_ignored` |
| `DISC_EF57202AEECCC8B4A11F` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `ads_route_plan_control_request_returns_runtime_identity_artifacts` | `not_ignored` |
| `DISC_AB26B7AC4A51D8EED5D7` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `ads_route_remove_returns_removal_artifact` | `not_ignored` |
| `DISC_55168C276A4C53920EE1` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `ads_server_doctor_control_request_returns_server_report_without_external_proof` | `not_ignored` |
| `DISC_A6203BCF8543279FF710` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `ads_server_doctor_start_and_status_use_job_poll_surface` | `not_ignored` |
| `DISC_813C96FB339546DE27C7` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `ads_server_route_plan_uses_server_wording` | `not_ignored` |
| `DISC_5B315E82E10D347A6883` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `ads_server_status_control_request_returns_server_surface` | `not_ignored` |
| `DISC_9C6A8FAA71CF3BDB36C8` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `ads_server_status_reflects_last_external_doctor_evidence_without_overclaiming` | `not_ignored` |
| `DISC_82E0BEAE2894897049D6` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `ads_server_symbols_control_request_returns_exposed_snapshot` | `not_ignored` |
| `DISC_3AFAAD8268EF8605C83D` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `ads_status_control_request_returns_runtime_ads_status_schema` | `not_ignored` |
| `DISC_B250BBD3C58F1A2851EA` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `breakpoints_set_accepts_project_relative_source_path` | `not_ignored` |
| `DISC_530A26285BCF28ED4AF1` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `comm_apply_accepts_empty_and_comment_only_io_toml_as_editable_bases` | `not_ignored` |
| `DISC_03857A00BDAAF507DA59` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `comm_apply_add_accepts_safe_state_only_io_toml` | `not_ignored` |
| `DISC_420774E0476946F9E120` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `comm_apply_blocks_non_io_secret_fields_on_untrusted_channel` | `not_ignored` |
| `DISC_95ED1F72F2DD2E43B7EB` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `comm_apply_blocks_secret_fields_on_untrusted_channel` | `not_ignored` |
| `DISC_A65A0E027C52EED46FBF` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `comm_apply_disable_preserves_driver_as_disabled` | `not_ignored` |
| `DISC_073FDF0BB5A12B8679DA` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `comm_apply_edit_is_idempotent_and_preserves_unrelated_instances` | `not_ignored` |
| `DISC_5BC2CA3EA617D63261AC` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `comm_apply_edit_requires_selected_instance` | `not_ignored` |
| `DISC_9F8C16B2B9365CA53931` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `comm_apply_ethercat_migrates_single_driver_bootstrap_to_multi_driver_topology` | `not_ignored` |
| `DISC_530B33357229300BC67E` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `comm_apply_is_policy_gated_and_audited_without_secret_values` | `not_ignored` |
| `DISC_BA6FF20972DDD9F2B981` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `comm_apply_rejects_cross_protocol_instance_ids` | `not_ignored` |
| `DISC_749CA6AED7A343292238` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `comm_apply_remove_last_driver_deletes_io_toml` | `not_ignored` |
| `DISC_53C714E98C473BD45451` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `comm_apply_remove_one_instance_preserves_unrelated_instances` | `not_ignored` |
| `DISC_7613107496D3BF22106E` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `comm_apply_returns_field_errors_and_dry_run_does_not_write` | `not_ignored` |
| `DISC_D751BCFA967C0670BBCD` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `comm_apply_writes_io_toml_and_preserves_unrelated_instances` | `not_ignored` |
| `DISC_0A9DE2CB6B4FD8E11B69` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `comm_apply_writes_runtime_toml_without_returning_secret_values` | `not_ignored` |
| `DISC_E01E368482A6A89AD208` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `comm_capabilities_control_request_reports_stable_protocol_statuses` | `not_ignored` |
| `DISC_ECCE0545B05F771C46D8` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `comm_schema_reports_io_driver_fields_without_secret_defaults` | `not_ignored` |
| `DISC_E01AA552585E9F9CD297` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `comm_schema_reports_non_io_file_protocols` | `not_ignored` |
| `DISC_0A2FB7B200058C256206` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `comm_test_reports_structured_results_without_external_network` | `not_ignored` |
| `DISC_22E1E8DEC8F529228B6E` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `config_set_rejects_runtime_backend_switch_during_live_control` | `not_ignored` |
| `DISC_19A9F891A4E5DE671019` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `config_set_reports_cross_field_auth_diagnostic` | `not_ignored` |
| `DISC_22D364962DA502F1F150` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `config_set_reports_field_level_diagnostics_for_unknown_and_type_errors` | `not_ignored` |
| `DISC_20C70AC01ED7B2D5389F` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `debug_program_and_io_handlers_preserve_behavior` | `not_ignored` |
| `DISC_863CEC807E61FD6C8118` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `default_visible_comm_schema_protocols_are_built_on_this_platform` | `not_ignored` |
| `DISC_AB15FD1DE9EB680AA061` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `fleet_topology_does_not_mark_enabled_services_green_without_bound_evidence` | `not_ignored` |
| `DISC_BB77A8E5A9B254AF059F` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `fleet_topology_reports_runtime_hosts_endpoints_and_links_without_secrets` | `not_ignored` |
| `DISC_6C400AA4FB0624C16EEB` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `fleet_topology_uses_project_config_for_roles_and_counterparts` | `not_ignored` |
| `DISC_FF41ACE26383D908BC8A` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `historian_query_and_alert_control_requests_return_contract_payloads` | `not_ignored` |
| `DISC_E597196254404C3948A6` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `invalid_and_malformed_requests_return_negative_responses` | `not_ignored` |
| `DISC_3A2F95FDE41ACD3BCF80` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `io_force_rejects_process_image_addresses_above_area_cap` | `not_ignored` |
| `DISC_53875B6D77CB51AF9D79` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `io_read_uses_force_marks_from_the_cached_snapshot` | `not_ignored` |
| `DISC_82D16E2E14705AC71B52` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `io_write_rejects_process_image_addresses_above_area_cap` | `not_ignored` |
| `DISC_05DA441ADF7E4C1EE01E` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `offline_comm_apply_creates_runtime_toml_when_absent` | `not_ignored` |
| `DISC_103D43E9BCBFBD54FAFA` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `offline_comm_apply_preserves_ads_server_client_pins_and_projects_them` | `not_ignored` |
| `DISC_72C0F29AA82A1DA03039` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `offline_comm_apply_writes_ads_runtime_and_ads_toml` | `not_ignored` |
| `DISC_D3ED6DD3482AAFDD867A` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `offline_comm_apply_writes_opcua_client_runtime_and_sidecar` | `not_ignored` |
| `DISC_BB7CC57529DE3A792861` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `offline_comm_schema_apply_and_topology_work_without_runtime` | `not_ignored` |
| `DISC_6EA437AE31ACD25CB497` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `offline_topology_includes_configured_openot_endpoint` | `not_ignored` |
| `DISC_8A5177A2CAABE2525469` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `rbac_authorization_matrix_enforces_sensitive_endpoint_roles` | `not_ignored` |
| `DISC_E7009F6DA35E4F274C36` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `request_routing_contract_dispatches_core_handler_modules` | `not_ignored` |
| `DISC_CBBB107A42464313E885` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `runtime_health_projection_contract_marks_faulted_driver_unhealthy` | `not_ignored` |
| `DISC_DE73AD9B8C12C51D78FC` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `runtime_status_projection_contract_reports_resource_metrics_realtime_and_io_health` | `not_ignored` |
| `DISC_D4393238D4D14C70E439` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `status_and_config_get_report_same_backend_selection` | `not_ignored` |
| `DISC_587D7703C609EB1CE7A0` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `status_and_config_get_surface_realtime_defaults` | `not_ignored` |
| `DISC_74B77F20409806BB46F0` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `status_reports_execution_backend_selection_and_metrics_tag` | `not_ignored` |
| `DISC_F55812F3561AC14DE4EE` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `unauthenticated_remote_control_defaults_to_viewer_without_admin_token` | `not_ignored` |
| `DISC_06C20CB12B91B0E18977` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `unauthenticated_unix_control_defaults_to_admin_without_admin_token` | `not_ignored` |
| `DISC_326FED38FACAFFC3CAF3` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/core.rs` | `unix_control_with_viewer_pairing_token_keeps_viewer_capabilities` | `not_ignored` |
| `DISC_DA6AA3E5882CA7E92AD7` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/debug_boundary.rs` | `debug_boundary_instance_values_use_type_names` | `not_ignored` |
| `DISC_865910EE487D4A787E2F` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/debug_boundary.rs` | `debug_boundary_io_snapshot_poison_is_an_error` | `not_ignored` |
| `DISC_21F0572D97CA98C8E472` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/debug_boundary.rs` | `debug_boundary_requests_fail_closed_for_stale_or_missing_names` | `not_ignored` |
| `DISC_46D2B3306A125A72A5B4` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/goldens.rs` | `phase0_ads_status_matches_disabled_goldens` | `not_ignored` |
| `DISC_92B2B9132BF6D0826E57` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/goldens.rs` | `phase0_discovery_matches_current_goldens` | `not_ignored` |
| `DISC_A0759CF3F2D0078D6A64` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/goldens.rs` | `phase0_io_driver_status_matches_legacy_golden` | `not_ignored` |
| `DISC_55D437E9E329FF76C784` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/goldens.rs` | `phase0_missing_or_failed_connectors_do_not_report_healthy` | `not_ignored` |
| `DISC_4C7F7BF0E967926694CF` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/goldens.rs` | `phase0_opcua_status_matches_capability_goldens` | `not_ignored` |
| `DISC_A7154671A2D6EC7E8616` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/hmi_descriptor_update.rs` | `hmi_descriptor_update_writes_files_and_bumps_schema_revision` | `not_ignored` |
| `DISC_2C037FCFB82B4E1EF360` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/hmi_descriptor_update.rs` | `hmi_scaffold_reset_regenerates_required_pages_and_revision` | `not_ignored` |
| `DISC_43019878280730A8C1EB` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/hmi_descriptor_watch.rs` | `hmi_descriptor_get_returns_inferred_layout_when_files_missing` | `not_ignored` |
| `DISC_C390FAFB290EA56F1AFD` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/hmi_descriptor_watch.rs` | `hmi_descriptor_watcher_handles_rapid_file_changes_without_deadlock` | `not_ignored` |
| `DISC_6FD023528E1B252027AF` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/hmi_descriptor_watch.rs` | `hmi_descriptor_watcher_retains_last_good_schema_on_invalid_toml` | `not_ignored` |
| `DISC_088942ADF2B8C26C6DA4` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/hmi_descriptor_watch.rs` | `hmi_descriptor_watcher_updates_schema_without_runtime_restart` | `not_ignored` |
| `DISC_6D307CA045E0FD7731EB` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/hmi_descriptor_watch.rs` | `hmi_trends_and_alarm_contracts_support_ack_flow` | `not_ignored` |
| `DISC_228754DDEE91A137B538` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/hmi_values_write.rs` | `hmi_operator_write_uses_hmi_policy_and_audit_path` | `not_ignored` |
| `DISC_DC04CC15DC6756C0F0F4` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/hmi_values_write.rs` | `hmi_runtime_read_port_is_code_backed_without_json_transport` | `not_ignored` |
| `DISC_3B0DF3186F53721354E3` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/hmi_values_write.rs` | `hmi_runtime_write_port_queues_allowlisted_write_without_json_transport` | `not_ignored` |
| `DISC_82FA70B7294E47A2FFE2` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/hmi_values_write.rs` | `hmi_schema_contract_includes_required_mapping` | `not_ignored` |
| `DISC_45DB690F322968387871` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/hmi_values_write.rs` | `hmi_values_contract_returns_timestamp_quality_and_typed_values` | `not_ignored` |
| `DISC_E391A5BE442E661DFD23` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/hmi_values_write.rs` | `hmi_write_is_disabled_in_read_only_mode` | `not_ignored` |
| `DISC_823AF3E2C3D2B40FF980` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/hmi_values_write.rs` | `hmi_write_processing_stays_under_cycle_budget` | `not_ignored` |
| `DISC_0DE0CA028C404CD81DB2` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/hmi_values_write.rs` | `hmi_write_queues_allowlisted_program_variable_write` | `not_ignored` |
| `DISC_A864D7153154B46359C6` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/hmi_values_write.rs` | `hmi_write_rejects_non_allowlisted_target` | `not_ignored` |
| `DISC_013C0E328B3E8D1DF885` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/hmi_values_write.rs` | `hmi_write_supports_path_allowlist_and_alias_param` | `not_ignored` |
| `DISC_6BD58A110ADF5AF44CEA` | `rust_unit_test` | `crates/trust-runtime/src/control/types.rs` | `io_snapshot_json_includes_optional_source` | `not_ignored` |
| `DISC_166ED313DD8C0A197E71` | `rust_unit_test` | `crates/trust-runtime/src/control/types.rs` | `io_snapshot_json_includes_optional_value_type` | `not_ignored` |
| `DISC_504C36D9A71CF032CA99` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/live_state.rs` | `alarm_deadband_requires_reentry_window_before_clear` | `not_ignored` |
| `DISC_5961E116E6F00267072E` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/live_state.rs` | `alarm_state_machine_covers_raise_ack_clear_history` | `not_ignored` |
| `DISC_9918717DC90273C38201` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/live_state.rs` | `alarm_state_uses_configured_alarm_label_without_renaming_widget` | `not_ignored` |
| `DISC_30D51AB56DEA87F77B9D` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/live_state.rs` | `hmi_event_stream_deduplicates_alarm_payloads` | `not_ignored` |
| `DISC_73587E255F98F0DAD50F` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/live_state.rs` | `hmi_event_stream_emits_changed_values_only` | `not_ignored` |
| `DISC_3F63F41C5FE5DCAF6DCE` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/live_state.rs` | `hmi_event_stream_tracks_schema_revision_and_widget_ids` | `not_ignored` |
| `DISC_8BE26ED74E9E29257128` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/live_state.rs` | `hmi_persistence_reloads_bounded_trends_and_alarm_history` | `not_ignored` |
| `DISC_C57EE1D5AAED1932269B` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/live_state.rs` | `trend_downsample_preserves_bounds_and_window` | `not_ignored` |
| `DISC_8A1C3AD3283CE417195B` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/scaffold_generation/core_tests.rs` | `scaffold_includes_external_symbols_and_excludes_internals` | `not_ignored` |
| `DISC_99E36DCF43E6418FAE63` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/scaffold_generation/core_tests.rs` | `scaffold_local_only_program_uses_inferred_interface_fallback` | `not_ignored` |
| `DISC_514A3C7CD34EA66EF422` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/scaffold_generation/core_tests.rs` | `scaffold_output_is_deterministic_for_same_input` | `not_ignored` |
| `DISC_6888480526071237330C` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/scaffold_generation/core_tests.rs` | `scaffold_overview_enforces_budget_and_config_version` | `not_ignored` |
| `DISC_03C3F84A5662C6BC7856` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/scaffold_generation/core_tests.rs` | `scaffold_widget_mapping_respects_type_and_writability` | `not_ignored` |
| `DISC_ABE7DAE6A97B593AB295` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/scaffold_generation/mapping_tests.rs` | `annotation_parser_handles_valid_invalid_and_missing_fields` | `not_ignored` |
| `DISC_29C51953C29C5A3C9B0D` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/scaffold_generation/mapping_tests.rs` | `widget_mapping_covers_required_type_buckets` | `not_ignored` |
| `DISC_1D3FFE0AC4DD2D438812` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/scaffold_generation/mode_tests.rs` | `scaffold_init_fails_when_hmi_dir_exists_without_force` | `not_ignored` |
| `DISC_516BF822DC021E128AF5` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/scaffold_generation/mode_tests.rs` | `scaffold_reset_creates_backup_snapshot` | `not_ignored` |
| `DISC_8552316C0519D652C171` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/scaffold_generation/page_tests.rs` | `scaffold_generates_control_and_process_pages` | `not_ignored` |
| `DISC_B125CFFB42E0AB8AD7F5` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/scaffold_generation/page_tests.rs` | `scaffold_groups_repeated_instance_prefixes_into_separate_sections` | `not_ignored` |
| `DISC_A41FCA8CADE662269BE8` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/scaffold_generation/page_tests.rs` | `scaffold_process_auto_svg_uses_grid_aligned_instrument_templates` | `not_ignored` |
| `DISC_6C3A98A6193F1AB55DFE` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/scaffold_generation/page_tests.rs` | `scaffold_process_toml_binds_level_fill_y_and_height` | `not_ignored` |
| `DISC_D40539D01A3F9B67B8F8` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/scaffold_generation/update_tests.rs` | `scaffold_update_merges_missing_signals_without_overwriting_custom_widgets` | `not_ignored` |
| `DISC_391D789A39B180F44CAF` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/scaffold_generation/update_tests.rs` | `scaffold_update_preserves_existing_page_and_fills_missing_files` | `not_ignored` |
| `DISC_15CF6989FA7FE7B04ADE` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/scaffold_generation/update_tests.rs` | `scaffold_update_skips_default_control_when_no_writable_points` | `not_ignored` |
| `DISC_428C54C8A32982652B76` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/scaffold_generation/update_tests.rs` | `scaffold_update_skips_default_process_when_custom_process_page_exists` | `not_ignored` |
| `DISC_FA32E85174615AB4B548` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/scaffold_legacy.rs` | `load_customization_uses_legacy_toml_when_hmi_dir_missing` | `not_ignored` |
| `DISC_11618980C47A176C3E67` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/scaffold_loading.rs` | `hmi_dir_loader_discovers_and_sorts_pages` | `not_ignored` |
| `DISC_07BE5E4888E58F70D0DC` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/scaffold_loading.rs` | `hmi_dir_loader_promotes_process_auto_svg_to_custom_asset` | `not_ignored` |
| `DISC_88F0ECD08508A3F6235B` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/scaffold_loading.rs` | `hmi_dir_loader_returns_none_for_invalid_toml` | `not_ignored` |
| `DISC_5B6A847B671D780BCDBF` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/scaffold_loading.rs` | `load_customization_prefers_hmi_dir_over_legacy_toml` | `not_ignored` |
| `DISC_D9E035B58115474E9893` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/scaffold_loading.rs` | `schema_merge_applies_defaults_annotations_and_file_overrides` | `not_ignored` |
| `DISC_61D3612B066D535CEB88` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/schema.rs` | `allowlisted_hmi_write_targets_are_marked_writable_in_schema` | `not_ignored` |
| `DISC_29F8EA2860B0DF680038` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/schema.rs` | `hmi_dir_alarm_thresholds_map_to_widget_limits` | `not_ignored` |
| `DISC_DA61C9B02F7C758142A4` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/schema.rs` | `hmi_dir_schema_snapshot_includes_rich_metadata` | `not_ignored` |
| `DISC_5B31D2D7385F9ABA19FF` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/schema.rs` | `layout_overrides_keep_widget_ids_stable` | `not_ignored` |
| `DISC_F6A7018579E453D375B0` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/schema.rs` | `resolve_write_point_supports_id_and_path_matches` | `not_ignored` |
| `DISC_58A279694D7D227A1A32` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/schema.rs` | `theme_snapshot_uses_default_fallbacks` | `not_ignored` |
| `DISC_04D8D25BB63297663445` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/schema.rs` | `validate_hmi_bindings_reports_unknown_paths_widgets_and_mismatches` | `not_ignored` |
| `DISC_A5669A1CE775557EF0B5` | `rust_unit_test` | `crates/trust-runtime/src/hmi/tests/schema.rs` | `write_customization_parses_enabled_and_allowlist` | `not_ignored` |
| `DISC_A2A783E87EAC3F28B5B7` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/backend_ads_rs.rs` | `connect_rejects_auto_add_route_before_network` | `not_ignored` |
| `DISC_62643936BD9A3CA2B2D8` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/backend_ads_rs.rs` | `connect_rejects_secure_transport_before_network` | `not_ignored` |
| `DISC_DC7BB67BE6A01029EF15` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/backend_ads_rs.rs` | `persistent_ads_symbol_sets_retain_guardrail_flag` | `not_ignored` |
| `DISC_02981F393856BA51831F` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/backend_ads_rs.rs` | `readonly_ads_symbol_does_not_get_write_flag` | `not_ignored` |
| `DISC_7E2D1AE394089750241B` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/backend_ads_rs.rs` | `subscribe_rejects_poll_mode_before_connection` | `not_ignored` |
| `DISC_26B2B95CF2D4AA13215E` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/backend_ads_rs.rs` | `unsupported_compound_symbol_is_not_a_bindable_descriptor` | `not_ignored` |
| `DISC_4D94E392A2A94D009E35` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/diagnostics/tests.rs` | `committed_golden_fixtures_match_rust_schema` | `not_ignored` |
| `DISC_8669D7D3B6B9DFB74ECD` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/diagnostics/tests.rs` | `credential_channel_classification_enforces_secret_boundary` | `not_ignored` |
| `DISC_991D32566D53109352EC` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/diagnostics/tests.rs` | `error_classifier_maps_common_failures_to_remediation_and_actions` | `not_ignored` |
| `DISC_F49BF2C6ED0365258046` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/diagnostics/tests.rs` | `evidence_hash_inputs_are_stable_and_complete` | `not_ignored` |
| `DISC_F33E95C098D12455A1C5` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/diagnostics/tests.rs` | `next_action_kinds_match_contract_names` | `not_ignored` |
| `DISC_592E9E21356943F44CDA` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/diagnostics/tests.rs` | `production_evidence_builder_hashes_declared_inputs` | `not_ignored` |
| `DISC_5D62DC5218997E909928` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/diagnostics/tests.rs` | `production_readiness_is_not_ready_without_evidence` | `not_ignored` |
| `DISC_35686A92EB060E3C3E53` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/diagnostics/tests.rs` | `production_readiness_needs_recheck_on_mismatch_fault_or_expiry` | `not_ignored` |
| `DISC_43A10D51EBCF1E560953` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/diagnostics/tests.rs` | `production_readiness_requires_matching_deployed_status` | `not_ignored` |
| `DISC_53701DB3E65A41F5CC42` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/diagnostics/tests.rs` | `production_ready_requires_pass_and_evidence` | `not_ignored` |
| `DISC_2E8E3FFC948D00E1FD02` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/diagnostics/tests.rs` | `reports_contain_no_secret_fields_by_construction` | `not_ignored` |
| `DISC_E696AB1E44976DD9D6CC` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/diagnostics/tests.rs` | `schema_reserves_server_role_without_server_modules` | `not_ignored` |
| `DISC_30A6266AEE6EA407106E` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/diagnostics/tests.rs` | `serializes_documented_missing_route_shape_deterministically` | `not_ignored` |
| `DISC_5870F66F23B67BE26C7C` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/diagnostics/tests.rs` | `server_production_ready_requires_independent_client_evidence` | `not_ignored` |
| `DISC_9B2850A6DAD6497D7A59` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/diagnostics/tests.rs` | `server_step_ids_match_contract_names` | `not_ignored` |
| `DISC_82101BAA11DC10486A75` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/diagnostics/tests.rs` | `skip_reasons_match_contract_names` | `not_ignored` |
| `DISC_8158660BFF570618628B` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/diagnostics/tests.rs` | `v1_client_evidence_json_still_deserializes_with_server_defaults` | `not_ignored` |
| `DISC_A0A0EB9C09BCA4B8AFC5` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/errors.rs` | `connection_refused_is_not_route_missing` | `not_ignored` |
| `DISC_F70C5D323F9AB05F0A00` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/errors.rs` | `empty_symbol_table_response_is_not_route_missing` | `not_ignored` |
| `DISC_BBE68843CB4A347D72CC` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/errors.rs` | `upload_timeout_is_classified_as_missing_return_route` | `not_ignored` |
| `DISC_9220A43D4A6A5E30654E` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/import.rs` | `apply_import_merges_second_connection_and_is_idempotent` | `not_ignored` |
| `DISC_83DB149CB87C57BB2CF0` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/import.rs` | `apply_import_pins_local_net_id_and_generates_single_file` | `not_ignored` |
| `DISC_F98A4868272A8E9EB1BE` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/import.rs` | `apply_import_rejects_empty_selection` | `not_ignored` |
| `DISC_8D2E72DA273FCDEB8E98` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/import.rs` | `apply_import_rejects_write_binding_without_acknowledgement` | `not_ignored` |
| `DISC_009E3F7DCAB6C08B4647` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/import.rs` | `build_import_response_honors_explicit_symbol_list` | `not_ignored` |
| `DISC_11CC9DCC882AD8C5D6B8` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/tests.rs` | `active_device_read_only_uses_live_status_and_never_opens_wire_connection` | `not_ignored` |
| `DISC_7F563991DB202BC4CB6A` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/tests.rs` | `default_local_ams_net_id_uses_selected_ipv4_address` | `not_ignored` |
| `DISC_690C7DF438D0E6476C46` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/tests.rs` | `directed_broadcast_collects_multiple_ads_targets_from_one_subnet` | `not_ignored` |
| `DISC_3BFD24318FBA81E4F38C` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/tests.rs` | `directed_broadcast_is_optional_and_deduplicated` | `not_ignored` |
| `DISC_EAABEE6636C013C2F4F9` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/tests.rs` | `directed_identify_collects_target_fields_from_wire` | `not_ignored` |
| `DISC_CB3C67B272F0DACF5248` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/tests.rs` | `doctor_happy_path_runs_required_steps_and_skips_write_probe_by_default` | `not_ignored` |
| `DISC_0216063A0BFB7D61C08B` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/tests.rs` | `doctor_job_reports_progress_and_cancellation` | `not_ignored` |
| `DISC_E31A8CA6D670053A7B8F` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/tests.rs` | `doctor_maps_named_failures_to_the_failed_step_and_blocks_dependents` | `not_ignored` |
| `DISC_99D3658AD50177DF8FD3` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/tests.rs` | `doctor_runs_guarded_write_probe_only_when_explicit` | `not_ignored` |
| `DISC_BFD9FB1995656161931F` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/tests.rs` | `doctor_with_manual_target_identity_does_not_require_udp_identify` | `not_ignored` |
| `DISC_7411E7C5399A4991C990` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/tests.rs` | `full_doctor_against_active_device_requires_explicit_pause` | `not_ignored` |
| `DISC_0AB19F84E6EA0EBEE298` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/tests.rs` | `identity_derivation_honors_advanced_local_net_id_override` | `not_ignored` |
| `DISC_B48F3AB69A7B85887EA3` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/tests.rs` | `identity_derivation_selects_runtime_host_source_and_candidates` | `not_ignored` |
| `DISC_ED7D50FF10AB547C5352` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/tests.rs` | `manual_discovery_path_does_not_depend_on_udp_identify` | `not_ignored` |
| `DISC_14096880787F2BCF0190` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/tests.rs` | `mock_failure_scenarios_cover_named_field_failures` | `not_ignored` |
| `DISC_168785104BF2541D31E6` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/tests.rs` | `mock_wire_happy_path_covers_required_operations` | `not_ignored` |
| `DISC_E4F8E0421394E0F9C0ED` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/tests.rs` | `network_classification_covers_lan_vpn_tailscale_loopback_public_and_nat` | `not_ignored` |
| `DISC_053B96EA0F674CD00FDD` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/tests.rs` | `onboarding_boundary_keeps_raw_ads_types_out_of_public_schema` | `not_ignored` |
| `DISC_A5FD0AFD9167AC6BCEA5` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/tests.rs` | `production_ready_requires_pass_evidence_and_live_deployed_status` | `not_ignored` |
| `DISC_31037757C37203F9D1E7` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/tests.rs` | `required_doctor_steps_and_timeouts_match_spec` | `not_ignored` |
| `DISC_B997C8DDAE889146717B` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/tests.rs` | `route_add_policy_allows_trusted_channel_to_call_wire` | `not_ignored` |
| `DISC_5A4BA740451F97547B6C` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/tests.rs` | `route_add_policy_rejects_nat_identity_before_wire_call` | `not_ignored` |
| `DISC_99A800DC00F6F512E4F8` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/tests.rs` | `route_add_policy_rejects_untrusted_channel_before_wire_call` | `not_ignored` |
| `DISC_464F365B71A0732B92E5` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/tests.rs` | `route_artifacts_preserve_unrelated_routes_and_report_encoding_changes` | `not_ignored` |
| `DISC_7DD1CC2DA1A775ABD85B` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/tests.rs` | `route_plan_disables_automatic_route_for_untrusted_or_nat_identity` | `not_ignored` |
| `DISC_D9D7AD1359220602322D` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/tests.rs` | `route_plan_uses_runtime_host_identity_and_generates_all_fallbacks` | `not_ignored` |
| `DISC_FA3E1817B26BA64B129E` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/tests.rs` | `server_route_artifacts_do_not_include_client_mode_1861_warning` | `not_ignored` |
| `DISC_F0541139A7F353BD69AD` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/tests.rs` | `source_probe_rejects_invalid_target_ip_without_network_io` | `not_ignored` |
| `DISC_A0984F0368FF8A8697D0` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/onboarding/tests.rs` | `wire_error_classification_keeps_remediation_machine_readable` | `not_ignored` |
| `DISC_E66354DC3BDE6BDD2D3C` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/server/tests.rs` | `audit_sink_records_accepted_and_rejected_ads_write_details` | `not_ignored` |
| `DISC_E2A35826BCDD97347312` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/server/tests.rs` | `client_policy_records_refused_attempts_for_wait_for_client_flow` | `not_ignored` |
| `DISC_727C28CE6777A7111F67` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/server/tests.rs` | `client_policy_requires_ams_net_id_and_source_pin` | `not_ignored` |
| `DISC_CA5166C15D944272B0AB` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/server/tests.rs` | `descriptor_for_array_preserves_bounds_and_scalar_type` | `not_ignored` |
| `DISC_70D206FE7E013466AE46` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/server/tests.rs` | `lifecycle_refreshes_symbols_without_rebinding_ads_socket` | `not_ignored` |
| `DISC_5F678E560F98443DDE86` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/server/tests.rs` | `lifecycle_starts_not_ready_without_snapshot_and_refreshes_when_snapshot_appears` | `not_ignored` |
| `DISC_795BA5426207369E9BB8` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/server/tests.rs` | `lifecycle_starts_tcp_listener_from_runtime_integration` | `not_ignored` |
| `DISC_5D87E3264428E8BE3DB4` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/server/tests.rs` | `publisher_marks_old_snapshot_quality_stale` | `not_ignored` |
| `DISC_FB42328FBA5DEE657222` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/server/tests.rs` | `publisher_reads_snapshot_and_marks_good_quality` | `not_ignored` |
| `DISC_2C13C2D2C45DD82BBC2A` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/server/tests.rs` | `publisher_rejects_missing_snapshot_without_touching_scan_thread` | `not_ignored` |
| `DISC_5CEABF396E21A6FF0B83` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/server/tests.rs` | `runtime_symbol_source_refresh_bumps_version_and_swaps_snapshot` | `not_ignored` |
| `DISC_2B4D4E8F343185DE898D` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/server/tests.rs` | `runtime_symbols_expose_configured_globals_and_writable_flags` | `not_ignored` |
| `DISC_CCD89416E66C09771A51` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/server/tests.rs` | `runtime_symbols_skip_unsupported_values` | `not_ignored` |
| `DISC_C29404A5CFE967581298` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/server/tests.rs` | `server_doctor_blocks_empty_expose_and_empty_clients` | `not_ignored` |
| `DISC_459303D3A3FACBC7F32D` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/server/tests.rs` | `server_doctor_requires_twincat_external_client_for_production_ready` | `not_ignored` |
| `DISC_C006ED96FD30C958CE27` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/server/tests.rs` | `server_doctor_self_test_does_not_grant_production_ready` | `not_ignored` |
| `DISC_5FB9C415B78619D5262A` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/server/tests.rs` | `temporary_self_test_policy_is_removed` | `not_ignored` |
| `DISC_AD194281D3C80ADC6682` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/server/tests.rs` | `write_port_accepts_guarded_write_and_coalesces_same_target` | `not_ignored` |
| `DISC_1D7B395AC6E9786437ED` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/server/tests.rs` | `write_port_rejects_faulted_runtime_without_mutation` | `not_ignored` |
| `DISC_2576E43F51C994EBD63F` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/server/tests.rs` | `write_port_rejects_policy_failure_without_mutation` | `not_ignored` |
| `DISC_85DD16C333A177DB683D` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/server/tests.rs` | `write_port_rejects_read_only_gate_without_mutation` | `not_ignored` |
| `DISC_605167FDAF897C8B15AE` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `bridge_applies_reads_at_input_phase_and_publishes_writes_at_output_phase` | `not_ignored` |
| `DISC_AF5192BD3500E5B08263` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `declared_binding_resolution_rejects_missing_global` | `not_ignored` |
| `DISC_780808E07663D754A287` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `declared_binding_resolution_rejects_type_mismatch` | `not_ignored` |
| `DISC_EE423FD67130C2A5AB06` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `generated_ads_interface_compiles_multiple_connections_in_one_file` | `not_ignored` |
| `DISC_93D92C1FACB63DF335B0` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `generated_ads_interface_compiles_offline_without_plc` | `not_ignored` |
| `DISC_F87275DFEEFBE028A8FD` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `generates_deterministic_ads_interface_from_snapshot_and_config` | `not_ignored` |
| `DISC_9DDC7C59193E011A4994` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `generator_emits_string_and_array_types` | `not_ignored` |
| `DISC_1C4565B9F7D755C5FDC4` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `generator_rejects_quality_name_collision` | `not_ignored` |
| `DISC_0B4466D1F550FB760DB7` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `generator_rejects_reserved_words_as_generated_identifiers` | `not_ignored` |
| `DISC_9346EADF597039BBA3B4` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `generator_rejects_snapshot_symbol_byte_size_mismatch` | `not_ignored` |
| `DISC_084D2365ADC0D06BF8F2` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `mock_transport_reports_missing_read_value_as_point_quality_error` | `not_ignored` |
| `DISC_DA1DF2BDC6EA59AF3BC1` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `mock_transport_resolves_reads_writes_and_tracks_symbol_version` | `not_ignored` |
| `DISC_E3B9811811AF5F882E6F` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `notify_binding_refresh_disconnects_before_resubscribe` | `not_ignored` |
| `DISC_9EE335C6641FD8CE1DCC` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `notify_binding_resubscribes_after_symbol_version_change` | `not_ignored` |
| `DISC_35EBC88B808176035E2E` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `notify_binding_subscribes_skips_poll_and_applies_at_scan_boundary` | `not_ignored` |
| `DISC_28C45C94601F0C40AE4D` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `offline_validation_accepts_generated_ads_interface` | `not_ignored` |
| `DISC_1A1F998AA3DD2B019B46` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `offline_validation_rejects_stale_generated_ads_interface` | `not_ignored` |
| `DISC_CC631BC07E3E1C0DBC54` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `parses_ads_toml_and_applies_security_and_point_defaults` | `not_ignored` |
| `DISC_8B6476CD755F8CCA7D4D` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `plain_transport_ack_surfaces_security_warning` | `not_ignored` |
| `DISC_B4166FA49B34F5DE4D3D` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `production_local_identity_validation_accepts_matching_local_net_id` | `not_ignored` |
| `DISC_08BB8962E4D418E212B4` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `production_local_identity_validation_rejects_mismatched_local_net_id` | `not_ignored` |
| `DISC_AB23FA48326AC3D9C37F` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `production_local_identity_validation_requires_pinned_local_net_id` | `not_ignored` |
| `DISC_CEA3094F832276A38452` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `read_bindings_do_not_publish_outputs` | `not_ignored` |
| `DISC_6AE69D4EE6B0066A3B7E` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `read_write_binding_seeds_write_baseline_from_first_good_read` | `not_ignored` |
| `DISC_0964F1D88F24D71AD970` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `reconnect_marks_points_stale_until_backoff_allows_connect` | `not_ignored` |
| `DISC_84E095B38F4B67BE9863` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `rejects_duplicate_declared_variable_binding` | `not_ignored` |
| `DISC_633E318B8B17D641CDB5` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `rejects_insecure_ack_without_plain_transport` | `not_ignored` |
| `DISC_C36170A07EEA2E1E98C3` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `rejects_mixed_symbol_and_index_addressing` | `not_ignored` |
| `DISC_7B8A3EF626FDEACB6679` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `rejects_notification_mode_without_notify_mode` | `not_ignored` |
| `DISC_D957A8465E9D2F11BDB0` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `rejects_plain_transport_without_explicit_ack` | `not_ignored` |
| `DISC_86724EABFC2DF40EA2D0` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `retain_read_requires_opt_in_and_allowed_binding_starts_stale` | `not_ignored` |
| `DISC_24642D3BA618D6761478` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `spawned_worker_drains_output_queue_off_scan_thread` | `not_ignored` |
| `DISC_97C0CDB92AAF33864139` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `spawned_worker_updates_cache_and_scan_applies_snapshot_only` | `not_ignored` |
| `DISC_FB4CC4B0804482F7B41B` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `symbol_version_change_revalidates_and_resolves_new_handles` | `not_ignored` |
| `DISC_C0F9AA0E78C1B00B3907` | `rust_unit_test` | `crates/trust-runtime/src/host/ads/tests.rs` | `symbol_version_type_mismatch_faults_bridge` | `not_ignored` |
| `DISC_E726DF21E9F7F87134B0` | `rust_unit_test` | `crates/trust-runtime/src/host/bundle_builder/tests.rs` | `build_accepts_cross_file_root_global_struct_field_access` | `not_ignored` |
| `DISC_36E7E7C235C77FD764A5` | `rust_unit_test` | `crates/trust-runtime/src/host/bundle_builder/tests.rs` | `build_fails_for_cyclic_dependencies` | `not_ignored` |
| `DISC_45CB6C9BAD8DCCF0A59F` | `rust_unit_test` | `crates/trust-runtime/src/host/bundle_builder/tests.rs` | `build_fails_for_missing_dependency_path` | `not_ignored` |
| `DISC_5D5805D749354AFB4996` | `rust_unit_test` | `crates/trust-runtime/src/host/bundle_builder/tests.rs` | `build_fails_for_version_mismatch` | `not_ignored` |
| `DISC_4CFAF7598FF6ED520833` | `rust_unit_test` | `crates/trust-runtime/src/host/bundle_builder/tests.rs` | `build_includes_transitive_dependency_sources` | `not_ignored` |
| `DISC_E585CA0FF1F35FD3CC14` | `rust_unit_test` | `crates/trust-runtime/src/host/bundle_builder/tests.rs` | `check_compiles_without_writing_program_stbc` | `not_ignored` |
| `DISC_FF69B42F579111AE16BF` | `rust_unit_test` | `crates/trust-runtime/src/host/bundle_builder/tests.rs` | `dependency_resolution_order_is_deterministic` | `not_ignored` |
| `DISC_4E78FA1FE054B4A0AD08` | `rust_unit_test` | `crates/trust-runtime/src/host/bundle_builder/tests.rs` | `resolve_sources_root_prefers_src_directory` | `not_ignored` |
| `DISC_DC6A59A72613FBA3CFA8` | `rust_unit_test` | `crates/trust-runtime/src/host/bundle_builder/tests.rs` | `resolve_sources_root_rejects_legacy_sources_directory` | `not_ignored` |
| `DISC_2BF93534ED0410D8DF91` | `rust_unit_test` | `crates/trust-runtime/src/host/debug/breakpoints.rs` | `breakpoints_do_not_match_non_overlapping_location` | `not_ignored` |
| `DISC_E048C15F73C978C98194` | `rust_unit_test` | `crates/trust-runtime/src/host/debug/breakpoints.rs` | `breakpoints_match_on_overlapping_location` | `not_ignored` |
| `DISC_E1EC3E8E0342A900E592` | `rust_unit_test` | `crates/trust-runtime/src/host/debug/control/tests.rs` | `breakpoint_clears_pending_pause` | `not_ignored` |
| `DISC_5DEB59E8A56CC1A8A797` | `rust_unit_test` | `crates/trust-runtime/src/host/debug/control/tests.rs` | `pause_after_continue_while_waiting_emits_pause_stop` | `not_ignored` |
| `DISC_2136591B45F301824416` | `rust_unit_test` | `crates/trust-runtime/src/host/debug/resolve.rs` | `resolve_breakpoint_prefers_statement_on_line` | `not_ignored` |
| `DISC_B06A00D5DAE37EE1F610` | `rust_unit_test` | `crates/trust-runtime/src/host/discovery.rs` | `discovery_entry_maps_properties` | `not_ignored` |
| `DISC_E0259D69BD86E74ECDD9` | `rust_unit_test` | `crates/trust-runtime/src/host/discovery.rs` | `discovery_service_info_enables_auto_addresses` | `not_ignored` |
| `DISC_09089C6266089B87945C` | `rust_unit_test` | `crates/trust-runtime/src/host/discovery.rs` | `service_removed_match_accepts_instance_suffix_runtime_name` | `not_ignored` |
| `DISC_916203572C0A2EE851BE` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/expr/call/tests.rs` | `bind_split_args_rejects_unnamed_named_call_without_panic` | `not_ignored` |
| `DISC_C91F55BE079B11CAC1AF` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/expr/call/tests.rs` | `bind_stdlib_named_args_rejects_unnamed_arg_without_panic` | `not_ignored` |
| `DISC_53CCB54BCE9617D27B26` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/errors.rs` | `datetime_range` | `not_ignored` |
| `DISC_A087DAFE68CD64E878B7` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/errors.rs` | `div_overflow` | `not_ignored` |
| `DISC_FDFF5424770C2D53EE41` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/errors.rs` | `evaluator_unknown_assignment_fails_without_creating_global` | `not_ignored` |
| `DISC_887F012B2BFBF638AD9A` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/errors.rs` | `index_and_null_ref` | `not_ignored` |
| `DISC_1E6A2F0D3A7A7A070049` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/errors.rs` | `neg_overflow_returns_runtime_error` | `not_ignored` |
| `DISC_48BC1C3F1E89DE140EC8` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/errors.rs` | `type_errors` | `not_ignored` |
| `DISC_2940007E0DA001A15AFF` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/expr_access.rs` | `index_and_field` | `not_ignored` |
| `DISC_093CA239F0F5693F16D2` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/expr_access.rs` | `nested_index_and_field_chains` | `not_ignored` |
| `DISC_C80A436C302CA90BDEFC` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/expr_bool.rs` | `short_circuit` | `not_ignored` |
| `DISC_FA319B47C81EF7436A0E` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/expr_coercion.rs` | `mixed_numeric_ops` | `not_ignored` |
| `DISC_6BA1C73101476F763277` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/expr_full.rs` | `iec_7_3_2` | `not_ignored` |
| `DISC_EAFE5F8331FD7818FA95` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/expr_full.rs` | `super_uses_parent_instance` | `not_ignored` |
| `DISC_1A2EF0C57F50EF332783` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/expr_literals.rs` | `literal_eval` | `not_ignored` |
| `DISC_C630382DF98C3E362E03` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/expr_literals.rs` | `name_ref_eval` | `not_ignored` |
| `DISC_5041EF737308280FA4C5` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/expr_ops.rs` | `bitwise_ops` | `not_ignored` |
| `DISC_EE30657D4DF4C2656824` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/expr_ops.rs` | `power_operator` | `not_ignored` |
| `DISC_9E6F3F5C918C834568BB` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/expr_ops.rs` | `precedence` | `not_ignored` |
| `DISC_3EAF3FF2C56261514EAA` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/expr_ops.rs` | `string_and_bool_compare` | `not_ignored` |
| `DISC_362EF7CB0DE3531228D5` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/expr_time_ops.rs` | `time_arithmetic_and_compare` | `not_ignored` |
| `DISC_2A07B5C831CBD45C2673` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/mod.rs` | `debug_hook_fires_once_per_statement` | `not_ignored` |
| `DISC_D7D624AE5A9BD3BF5B97` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/pou_en_eno.rs` | `en_eno_semantics` | `not_ignored` |
| `DISC_D69368F3E865C26363E8` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/pou_fb.rs` | `fb_omitted_var_input_reuses_stored_value_after_explicit_update` | `not_ignored` |
| `DISC_047702AC35B146A24FFA` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/pou_fb.rs` | `fb_stateful` | `not_ignored` |
| `DISC_0B7986105E0E71A865A1` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/pou_fb.rs` | `pointer_to_wildcard_array_writes_through_correctly` | `not_ignored` |
| `DISC_59D95E3597564B9958D6` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/pou_fb.rs` | `var_input_pointer_deref_write_mutates_callers_storage` | `not_ignored` |
| `DISC_670508222190BB8225DC` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/pou_fb.rs` | `wildcard_array_var_in_out_writes_through_correctly` | `not_ignored` |
| `DISC_5FFA262B7BD078E8BA91` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/pou_function.rs` | `call_function_exec` | `not_ignored` |
| `DISC_3546DDBC36A22F00D5A5` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/pou_function.rs` | `function_input_interface_defaults_to_null` | `not_ignored` |
| `DISC_BDDA9C1300650D82D275` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/pou_function.rs` | `function_interface_return_defaults_to_null` | `not_ignored` |
| `DISC_A2B372EFAFFC18A26346` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/pou_function.rs` | `function_local_interface_defaults_to_null` | `not_ignored` |
| `DISC_E87798626ACC02A27BFB` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/pou_params.rs` | `param_binding` | `not_ignored` |
| `DISC_311FEE088EA3D278C1A4` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/reference.rs` | `default_null_reference` | `not_ignored` |
| `DISC_E900E662F1AE4167D4E6` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/reference.rs` | `ref_and_deref` | `not_ignored` |
| `DISC_17A37F2EEB895334986E` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/stmt_basic.rs` | `assignment` | `not_ignored` |
| `DISC_F83166C54C9F362F6648` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/stmt_case.rs` | `case_labels` | `not_ignored` |
| `DISC_23D59686CEEEB29BF430` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/stmt_case.rs` | `case_string_labels` | `not_ignored` |
| `DISC_9CD07AD36ACC67C7CFB8` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/stmt_if.rs` | `if_branches` | `not_ignored` |
| `DISC_8C70125151B9E81BADA4` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/stmt_loops.rs` | `loop_control` | `not_ignored` |
| `DISC_03A94A1E711B99FC2BC4` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/stmt_return.rs` | `default_result` | `not_ignored` |
| `DISC_D52F4CAA5A2528313FE8` | `rust_unit_test` | `crates/trust-runtime/src/host/execution_backend.rs` | `parse_accepts_case_insensitive_values` | `not_ignored` |
| `DISC_A31A953F14C30537DB64` | `rust_unit_test` | `crates/trust-runtime/src/host/execution_backend.rs` | `parse_accepts_trimmed_values` | `not_ignored` |
| `DISC_58824C6943F8514F682A` | `rust_unit_test` | `crates/trust-runtime/src/host/execution_backend.rs` | `parse_rejects_empty_and_invalid_values` | `not_ignored` |
| `DISC_698A7E9D7430C17901D5` | `rust_unit_test` | `crates/trust-runtime/src/host/execution_backend.rs` | `parse_rejects_interpreter_values` | `not_ignored` |
| `DISC_EF19112CA33319B9B7F0` | `rust_unit_test` | `crates/trust-runtime/src/host/helper_eval/const_expr.rs` | `array_repetition_initializer_uses_expanded_value_shape` | `not_ignored` |
| `DISC_45AF9C23886B40798672` | `rust_unit_test` | `crates/trust-runtime/src/host/helper_eval/const_expr.rs` | `evaluates_nested_const_expression` | `not_ignored` |
| `DISC_76C106F96179CB42CA7F` | `rust_unit_test` | `crates/trust-runtime/src/host/helper_eval/const_expr.rs` | `rejects_non_const_access` | `not_ignored` |
| `DISC_98087D9555A778C96FB9` | `rust_unit_test` | `crates/trust-runtime/src/host/helper_eval/const_expr.rs` | `resolves_named_const_with_resolver` | `not_ignored` |
| `DISC_56C32A98B0CE62F0D1BE` | `rust_unit_test` | `crates/trust-runtime/src/host/helper_eval/storage_expr.rs` | `allows_pure_stdlib_calls_when_stdlib_is_provided` | `not_ignored` |
| `DISC_FE3CF701DB5C862F2AA8` | `rust_unit_test` | `crates/trust-runtime/src/host/helper_eval/storage_expr.rs` | `array_repetition_initializer_uses_expanded_value_shape` | `not_ignored` |
| `DISC_0EEA83257F8DEA9CBB11` | `rust_unit_test` | `crates/trust-runtime/src/host/helper_eval/storage_expr.rs` | `rejects_calls_without_stdlib_surface` | `not_ignored` |
| `DISC_30BF7A25E82D62C835F3` | `rust_unit_test` | `crates/trust-runtime/src/host/helper_eval/storage_lvalue.rs` | `unknown_name_write_fails_without_creating_global` | `not_ignored` |
| `DISC_CE784ABF7B27AE9C4159` | `rust_unit_test` | `crates/trust-runtime/src/host/helper_eval/storage_lvalue.rs` | `writes_array_element_without_eval_context` | `not_ignored` |
| `DISC_03E1DAE7AB789D0A6E07` | `rust_unit_test` | `crates/trust-runtime/src/host/helper_eval/storage_lvalue.rs` | `writes_deref_target_without_eval_context` | `not_ignored` |
| `DISC_C1A3069D1C3D57477C15` | `rust_unit_test` | `crates/trust-runtime/src/host/helper_eval/storage_lvalue.rs` | `writes_local_name_without_eval_context` | `not_ignored` |
| `DISC_DBAC045DFB5C89D47B28` | `rust_unit_test` | `crates/trust-runtime/src/host/helper_eval/storage_lvalue.rs` | `writes_nested_array_of_struct_field_without_eval_context` | `not_ignored` |
| `DISC_32A8F3A0CAF2ADE8A7D6` | `rust_unit_test` | `crates/trust-runtime/src/host/helper_eval/storage_lvalue.rs` | `writes_nested_struct_array_element_without_eval_context` | `not_ignored` |
| `DISC_747F9C39D91F23073B1D` | `rust_unit_test` | `crates/trust-runtime/src/host/helper_eval/storage_lvalue.rs` | `writes_struct_field_without_eval_context` | `not_ignored` |
| `DISC_2E17F96D48D9A1F05349` | `rust_unit_test` | `crates/trust-runtime/src/host/historian/tests.rs` | `alert_threshold_debounce_and_file_hook_contract` | `not_ignored` |
| `DISC_1B9EF96839057E1619C0` | `rust_unit_test` | `crates/trust-runtime/src/host/historian/tests.rs` | `allowlist_mode_records_matching_paths_only` | `not_ignored` |
| `DISC_A86EAC2CC2873C2FE31D` | `rust_unit_test` | `crates/trust-runtime/src/host/historian/tests.rs` | `persistent_backend_reloads_across_service_restart` | `not_ignored` |
| `DISC_EFFABC152F3EA9FFB736` | `rust_unit_test` | `crates/trust-runtime/src/host/historian/tests.rs` | `prometheus_render_includes_runtime_and_historian_metrics` | `not_ignored` |
| `DISC_1777F6DE6144940DA855` | `rust_unit_test` | `crates/trust-runtime/src/host/historian/tests.rs` | `recording_fidelity_and_sample_interval_are_enforced` | `not_ignored` |
| `DISC_EAD5FD579C31A947C9F5` | `rust_unit_test` | `crates/trust-runtime/src/host/instance.rs` | `create_fb_instance_honors_declared_var_input_initializer` | `not_ignored` |
| `DISC_41CDECC8FB4FF1BD5027` | `rust_unit_test` | `crates/trust-runtime/src/host/linux_rt.rs` | `boot_config_parser_distinguishes_rt_and_non_rt` | `not_ignored` |
| `DISC_A11CF066E9E5E81F32DB` | `rust_unit_test` | `crates/trust-runtime/src/host/linux_rt.rs` | `proc_stat_scheduler_parser_reads_user_space_rt_priority` | `not_ignored` |
| `DISC_42C493F6531C0B5BC617` | `rust_unit_test` | `crates/trust-runtime/src/host/linux_rt.rs` | `proc_status_parser_reads_vm_lock_field` | `not_ignored` |
| `DISC_C0B22EE60362AF065B2B` | `rust_unit_test` | `crates/trust-runtime/src/host/linux_rt.rs` | `proc_version_parser_distinguishes_rt_and_non_rt` | `not_ignored` |
| `DISC_129EF130404117824DB6` | `rust_unit_test` | `crates/trust-runtime/src/host/linux_rt.rs` | `realtime_config_rejects_duplicate_affinity_entries` | `not_ignored` |
| `DISC_C0D68A73DC9FC34E8D96` | `rust_unit_test` | `crates/trust-runtime/src/host/linux_rt.rs` | `realtime_config_requires_realtime_scheduler_when_enabled` | `not_ignored` |
| `DISC_688E963173245BF331E2` | `rust_unit_test` | `crates/trust-runtime/src/host/linux_rt.rs` | `scheduler_observation_accepts_matching_fifo_priority` | `not_ignored` |
| `DISC_2D14E258160D15A75F5E` | `rust_unit_test` | `crates/trust-runtime/src/host/linux_rt.rs` | `scheduler_policy_parser_accepts_documented_values` | `not_ignored` |
| `DISC_E2B6649C00670F6E008E` | `rust_unit_test` | `crates/trust-runtime/src/host/linux_rt.rs` | `strict_hook_returns_error_when_profile_verification_fails` | `not_ignored` |
| `DISC_41148C19CE009B84842F` | `rust_unit_test` | `crates/trust-runtime/src/host/mesh/tests.rs` | `liveliness_registry_tracks_join_and_leave_transitions` | `not_ignored` |
| `DISC_67F6EF08D13CF7F1F503` | `rust_unit_test` | `crates/trust-runtime/src/host/mesh/tests.rs` | `mesh_cloud_ready_wait_times_out_for_degraded_state` | `not_ignored` |
| `DISC_8DA120AC16B352A11CE5` | `rust_unit_test` | `crates/trust-runtime/src/host/mesh/tests.rs` | `mesh_payload_accepts_numeric_target_boundaries` | `not_ignored` |
| `DISC_A6037EE5CFAA0C4994D2` | `rust_unit_test` | `crates/trust-runtime/src/host/mesh/tests.rs` | `mesh_payload_encode_decode_fuzz_smoke_budget` | `not_ignored` |
| `DISC_024ABE2EBFBC15F69AC4` | `rust_unit_test` | `crates/trust-runtime/src/host/mesh/tests.rs` | `mesh_payload_propagates_source_identity_and_sequence_metadata` | `not_ignored` |
| `DISC_B04B53DCFB50233B360A` | `rust_unit_test` | `crates/trust-runtime/src/host/mesh/tests.rs` | `mesh_payload_rejects_integer_narrowing_overflow` | `not_ignored` |
| `DISC_17A3C97ED5EF6F1D342D` | `rust_unit_test` | `crates/trust-runtime/src/host/mesh/tests.rs` | `mesh_snapshot_timeout_is_not_a_successful_empty_snapshot` | `not_ignored` |
| `DISC_6B27D6FD6368F3863353` | `rust_unit_test` | `crates/trust-runtime/src/host/mesh/tests.rs` | `mesh_subscribe_mapping_requires_peer_and_remote_key` | `not_ignored` |
| `DISC_19E22E422BC38C306B99` | `rust_unit_test` | `crates/trust-runtime/src/host/mesh/tests.rs` | `mesh_tls_publish_applies_updates` | `not_ignored` |
| `DISC_9AE4A6759E75A869499F` | `rust_unit_test` | `crates/trust-runtime/src/host/mesh/tests.rs` | `mixed_version_policy_rejects_minor_mismatch` | `not_ignored` |
| `DISC_0798611423046E924E5B` | `rust_unit_test` | `crates/trust-runtime/src/host/mesh/tests.rs` | `qos_profile_mapping_aligns_with_active_cfg_and_diag_zones` | `not_ignored` |
| `DISC_264B35B776A6C01675B6` | `rust_unit_test` | `crates/trust-runtime/src/host/mesh/tests.rs` | `queryables_are_available_when_mesh_session_starts` | `not_ignored` |
| `DISC_96B3235911C7C0E1D57D` | `rust_unit_test` | `crates/trust-runtime/src/host/mesh/version.rs` | `version_policy_accepts_matching_release_family` | `not_ignored` |
| `DISC_D265CDCB534920D15105` | `rust_unit_test` | `crates/trust-runtime/src/host/mesh/version.rs` | `version_policy_rejects_mixed_major_minor_for_router_plugins` | `not_ignored` |
| `DISC_65395FAFC221AD5E0FE4` | `rust_unit_test` | `crates/trust-runtime/src/host/metrics.rs` | `cycle_percentiles_track_recent_window` | `not_ignored` |
| `DISC_ED49AC4813C08951E6A1` | `rust_unit_test` | `crates/trust-runtime/src/host/metrics.rs` | `profiling_records_call_entries_with_cycle_contribution` | `not_ignored` |
| `DISC_23B82380B693BF86A795` | `rust_unit_test` | `crates/trust-runtime/src/host/metrics.rs` | `profiling_toggle_disables_and_reenables_collection` | `not_ignored` |
| `DISC_0B198B435020505184EB` | `rust_unit_test` | `crates/trust-runtime/src/host/metrics.rs` | `profiling_top_contributors_ranked_by_cycle_budget` | `not_ignored` |
| `DISC_607FE044E9F299285455` | `rust_unit_test` | `crates/trust-runtime/src/host/opcua/tests.rs` | `connected_detail_reports_timeout_negotiation_or_documented_gap` | `not_ignored` |
| `DISC_21730EA1408811561B32` | `rust_unit_test` | `crates/trust-runtime/src/host/opcua/tests.rs` | `maps_enum_values_as_string_variants` | `not_ignored` |
| `DISC_DA883AC380548C276E7F` | `rust_unit_test` | `crates/trust-runtime/src/host/opcua/tests.rs` | `maps_scalar_numeric_and_string_types` | `not_ignored` |
| `DISC_CC23E54B30499BB0CB0A` | `rust_unit_test` | `crates/trust-runtime/src/host/opcua/tests.rs` | `opcua_client_explicit_trust_promotes_rejected_certificates` | `not_ignored` |
| `DISC_6645454C7C95B5F6EA9F` | `rust_unit_test` | `crates/trust-runtime/src/host/opcua/tests.rs` | `opcua_client_rejected_security_policy_during_login_prompts_for_auth` | `not_ignored` |
| `DISC_112536618DD96F545DC5` | `rust_unit_test` | `crates/trust-runtime/src/host/opcua/tests.rs` | `opcua_client_trust_store_can_be_listed_and_cleared` | `not_ignored` |
| `DISC_D074D829A8C6B93DAC92` | `rust_unit_test` | `crates/trust-runtime/src/host/opcua/tests.rs` | `opcua_event_queue_rejects_saturation_and_recovers_after_drain` | `not_ignored` |
| `DISC_EF47F02553AABA1EAA0E` | `rust_unit_test` | `crates/trust-runtime/src/host/opcua/tests.rs` | `parses_security_policy_and_mode_aliases` | `not_ignored` |
| `DISC_58C4F9FEBAD2FEFEAA75` | `rust_unit_test` | `crates/trust-runtime/src/host/opcua/tests.rs` | `persistent_worker_applies_subscription_updates_without_reconnecting_per_scan` | `not_ignored` |
| `DISC_BDE300CE810CD01E9DFE` | `rust_unit_test` | `crates/trust-runtime/src/host/opcua/tests.rs` | `persistent_worker_batches_writes_without_reconnecting_per_write` | `not_ignored` |
| `DISC_5F738C6D18E335AC385C` | `rust_unit_test` | `crates/trust-runtime/src/host/opcua/tests.rs` | `persistent_worker_marks_stale_then_recovers_on_subscription_update` | `not_ignored` |
| `DISC_D0022CD29DB5061F146B` | `rust_unit_test` | `crates/trust-runtime/src/host/opcua/tests.rs` | `persistent_worker_reconnects_after_session_loss_without_scan_thread_io` | `not_ignored` |
| `DISC_825048A5CCB4563DE83C` | `rust_unit_test` | `crates/trust-runtime/src/host/opcua/tests.rs` | `persistent_worker_recreates_subscription_after_server_restart` | `not_ignored` |
| `DISC_45DCF63A60FA87D72979` | `rust_unit_test` | `crates/trust-runtime/src/host/opcua/tests.rs` | `persistent_worker_rejected_write_marks_point_without_reconnecting` | `not_ignored` |
| `DISC_D2C8A6DEB11E476D80F6` | `rust_unit_test` | `crates/trust-runtime/src/host/opcua/tests.rs` | `persistent_worker_uses_recovery_hook_to_reestablish_subscriptions` | `not_ignored` |
| `DISC_BE2359D13DCE953DC558` | `rust_unit_test` | `crates/trust-runtime/src/host/opcua/tests.rs` | `rejects_invalid_security_profile_combinations` | `not_ignored` |
| `DISC_CA24079FB9ACE8346755` | `rust_unit_test` | `crates/trust-runtime/src/host/opcua/tests.rs` | `rejects_non_scalar_or_protocol_specific_types` | `not_ignored` |
| `DISC_A2F34DF30D8EC15DD686` | `rust_unit_test` | `crates/trust-runtime/src/host/opcua/tests.rs` | `secure_profile_defaults_to_signed_and_encrypted_policy` | `not_ignored` |
| `DISC_94AB7B525581745D39D1` | `rust_unit_test` | `crates/trust-runtime/src/host/realtime/realtime_tests_part_01.rs` | `t0_shm_contract_mismatch_is_rejected_before_run` | `not_ignored` |
| `DISC_E97822CE4B2200DD8928` | `rust_unit_test` | `crates/trust-runtime/src/host/realtime/realtime_tests_part_01.rs` | `t0_shm_header_fuzz_rejects_corruption_budget` | `not_ignored` |
| `DISC_E6866C276F854DE962A8` | `rust_unit_test` | `crates/trust-runtime/src/host/realtime/realtime_tests_part_01.rs` | `t0_shm_readiness_metadata_matches_meta_shm_channels_contract` | `not_ignored` |
| `DISC_734111DCA27247D1378A` | `rust_unit_test` | `crates/trust-runtime/src/host/realtime/realtime_tests_part_01.rs` | `t0_shm_registration_fails_fast_when_required_pinning_is_unavailable` | `not_ignored` |
| `DISC_2E815DCD279686A7B9C8` | `rust_unit_test` | `crates/trust-runtime/src/host/realtime/realtime_tests_part_01.rs` | `t0_shm_registration_fails_fast_when_root_path_is_not_directory` | `not_ignored` |
| `DISC_34C0BCBE4BD82D8B80D0` | `rust_unit_test` | `crates/trust-runtime/src/host/realtime/realtime_tests_part_02.rs` | `qos_tier_route_legality_matrix_matches_contract` | `not_ignored` |
| `DISC_9D4A488E4E903C1DC0B2` | `rust_unit_test` | `crates/trust-runtime/src/host/realtime/realtime_tests_part_02.rs` | `t0_bind_enforces_schema_hash_and_fixed_layout_contract` | `not_ignored` |
| `DISC_55CAADD3CCB4DC311F46` | `rust_unit_test` | `crates/trust-runtime/src/host/realtime/realtime_tests_part_02.rs` | `t0_bind_rejects_non_t0_route_and_denies_fallback` | `not_ignored` |
| `DISC_81373063FE7D545520F8` | `rust_unit_test` | `crates/trust-runtime/src/host/realtime/realtime_tests_part_02.rs` | `t0_error_codes_map_to_canonical_comms_contract_codes` | `not_ignored` |
| `DISC_8C06F16AC40A8DA036BF` | `rust_unit_test` | `crates/trust-runtime/src/host/realtime/realtime_tests_part_02.rs` | `t0_publish_and_read_track_overrun_and_latest_payload` | `not_ignored` |
| `DISC_5E088D53395CC4F852E5` | `rust_unit_test` | `crates/trust-runtime/src/host/realtime/realtime_tests_part_02.rs` | `t0_read_surfaces_stale_after_bounded_misses_and_spin_limit` | `not_ignored` |
| `DISC_6E5750801DC19C4E3BDA` | `rust_unit_test` | `crates/trust-runtime/src/host/realtime/realtime_tests_part_03.rs` | `t0_cycle_scheduler_enforces_pre_post_order_and_cloud_budget` | `not_ignored` |
| `DISC_E1D3B90728072A5E5DE4` | `rust_unit_test` | `crates/trust-runtime/src/host/realtime/realtime_tests_part_03.rs` | `t0_publish_and_read_reject_uninitialized_or_unpinned_channel_state` | `not_ignored` |
| `DISC_678ACD2B87F05E88AA1A` | `rust_unit_test` | `crates/trust-runtime/src/host/realtime/realtime_tests_part_03.rs` | `t0_scheduler_budget_isolated_under_cloud_stress_across_cycles` | `not_ignored` |
| `DISC_D6A7D909604F60CEEBCD` | `rust_unit_test` | `crates/trust-runtime/src/host/registry/tests.rs` | `registry_profile_covers_required_endpoints` | `not_ignored` |
| `DISC_870863E508DE2AB1E2C2` | `rust_unit_test` | `crates/trust-runtime/src/host/security/pairing.rs` | `pairing_claim_cycle` | `not_ignored` |
| `DISC_454E2B975FDEE42FC567` | `rust_unit_test` | `crates/trust-runtime/src/host/security/pairing.rs` | `pairing_expiry_rejects` | `not_ignored` |
| `DISC_D38D6B71B00105F1DED1` | `rust_unit_test` | `crates/trust-runtime/src/host/security/pairing.rs` | `pairing_token_expiry_disables_old_token` | `not_ignored` |
| `DISC_311210C0D28DB24C63E4` | `rust_unit_test` | `crates/trust-runtime/src/host/ui/tests.rs` | `command_routing_covers_settings_beginner_guard_and_pause` | `not_ignored` |
| `DISC_BF2DE69B33FFEE01A40C` | `rust_unit_test` | `crates/trust-runtime/src/host/ui/tests.rs` | `input_navigation_handles_prompt_and_read_only_mode` | `not_ignored` |
| `DISC_FA59629171A8BDBB5DDC` | `rust_unit_test` | `crates/trust-runtime/src/host/ui/tests.rs` | `parse_settings_includes_cycle_interval_field` | `not_ignored` |
| `DISC_5BA23B9BF6BB39B2EE7E` | `rust_unit_test` | `crates/trust-runtime/src/host/ui/tests.rs` | `parse_settings_includes_simulation_fields` | `not_ignored` |
| `DISC_6E27A40949DC18DEBBE9` | `rust_unit_test` | `crates/trust-runtime/src/host/ui/tests.rs` | `parse_snapshot_includes_tasks_io_and_events` | `not_ignored` |
| `DISC_86C9899C0207C2AFB831` | `rust_unit_test` | `crates/trust-runtime/src/host/ui/tests.rs` | `parse_status_accepts_io_drivers_field` | `not_ignored` |
| `DISC_780380F2A944A080F4B5` | `rust_unit_test` | `crates/trust-runtime/src/host/ui/tests.rs` | `parse_status_includes_simulation_mode_fields` | `not_ignored` |
| `DISC_9BC4BE4862E992370053` | `rust_unit_test` | `crates/trust-runtime/src/host/ui/tests.rs` | `render_dashboard_snapshot_matches_layout` | `not_ignored` |
| `DISC_A55F43B94FBE8F142878` | `rust_unit_test` | `crates/trust-runtime/src/io/ethercat/tests.rs` | `ethercat_config_accepts_hardware_adapter_name` | `not_ignored` |
| `DISC_2CDFD00CD4BD8C8471A8` | `rust_unit_test` | `crates/trust-runtime/src/io/ethercat/tests.rs` | `ethercat_config_defaults_cover_ek1100_elx008` | `not_ignored` |
| `DISC_B6FF8A186F0A1C6B7D26` | `rust_unit_test` | `crates/trust-runtime/src/io/ethercat/tests.rs` | `ethercat_driver_fault_policy_propagates_driver_failure` | `not_ignored` |
| `DISC_D7D8D7D25E83930E81B1` | `rust_unit_test` | `crates/trust-runtime/src/io/ethercat/tests.rs` | `ethercat_driver_mock_reads_and_writes_images` | `not_ignored` |
| `DISC_FBF8AF35E1047258FE7E` | `rust_unit_test` | `crates/trust-runtime/src/io/ethercat/tests.rs` | `ethercat_driver_warn_policy_degrades_and_reports_error` | `not_ignored` |
| `DISC_73C6354A8A53C9B983CB` | `rust_unit_test` | `crates/trust-runtime/src/io/ethercat/tests.rs` | `ethercat_hardware_open_failure_faults_without_blocking_startup` | `not_ignored` |
| `DISC_AAC32F62FD46D1A39C78` | `rust_unit_test` | `crates/trust-runtime/src/io/gpio.rs` | `gpio_read_failure_updates_driver_health` | `not_ignored` |
| `DISC_E944C2D055EB2EDB0E79` | `rust_unit_test` | `crates/trust-runtime/src/io/gpio.rs` | `gpio_write_failure_updates_driver_health` | `not_ignored` |
| `DISC_B66C105004A8A1831D52` | `rust_unit_test` | `crates/trust-runtime/src/io/gpio.rs` | `parse_gpio_config_accepts_basic_inputs` | `not_ignored` |
| `DISC_04BE0F933AAEBA00DF54` | `rust_unit_test` | `crates/trust-runtime/src/io/gpio.rs` | `parse_gpio_config_keeps_sysfs_selectable_for_legacy_hosts` | `not_ignored` |
| `DISC_FEC5B699FE0A81B32CDD` | `rust_unit_test` | `crates/trust-runtime/src/io/gpio.rs` | `rejects_non_bit_addresses` | `not_ignored` |
| `DISC_B9925C29A1388A76C8B3` | `rust_unit_test` | `crates/trust-runtime/src/io/interface.rs` | `process_image_over_reads_are_zero_filled_and_do_not_resize` | `not_ignored` |
| `DISC_07CD1835ED81602972AA` | `rust_unit_test` | `crates/trust-runtime/src/io/interface.rs` | `process_image_try_resize_rejects_areas_above_cap` | `not_ignored` |
| `DISC_5CC9ABB5F96BEB6BEB7C` | `rust_unit_test` | `crates/trust-runtime/src/io/interface.rs` | `process_image_write_rejects_addresses_above_area_cap` | `not_ignored` |
| `DISC_0DCD26A7C1BC6F590DFA` | `rust_unit_test` | `crates/trust-runtime/src/io/interface.rs` | `snapshot_carries_and_formats_typed_bindings` | `not_ignored` |
| `DISC_4A6DE20E85BEBEC3ECCC` | `rust_unit_test` | `crates/trust-runtime/src/io/interface.rs` | `snapshot_carries_optional_binding_source` | `not_ignored` |
| `DISC_D0012FE1D341E4EF0E85` | `rust_unit_test` | `crates/trust-runtime/src/io/interface.rs` | `string_process_image_write_rejects_payload_larger_than_declared_window` | `not_ignored` |
| `DISC_6432C669FA212BD403B1` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests.rs` | `contract_test_reads_and_writes_payloads` | `not_ignored` |
| `DISC_28CC714B2635869E869C` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests.rs` | `cycle_impact_test_driver_calls_are_non_blocking_without_session` | `not_ignored` |
| `DISC_2A6A13834385AB7C9A1D` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests.rs` | `fail_closed_connect_failure_is_observable` | `not_ignored` |
| `DISC_C3A91E2860C30B08BED4` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests.rs` | `fail_closed_disconnected_read_returns_freshness_error` | `not_ignored` |
| `DISC_A58F9A1B38CF9A02367C` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests.rs` | `fail_closed_publish_failure_returns_output_error` | `not_ignored` |
| `DISC_34FD22A3D82FA5008A39` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests.rs` | `mqtt_driver_drop_is_bounded_while_session_is_connecting` | `not_ignored` |
| `DISC_F5E2BC57F764A6FF6DD2` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests.rs` | `mqtt_no_fresh_payload_follows_on_error_policy` | `not_ignored` |
| `DISC_36444246A468C534110A` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests.rs` | `mqtt_output_handoff_is_bounded_when_scan_outpaces_worker` | `not_ignored` |
| `DISC_ECD88F595B660F325E42` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests.rs` | `mqtt_read_inputs_returns_within_scan_bound_without_fresh_payload` | `not_ignored` |
| `DISC_8A42BABA8B61292EA5BC` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests.rs` | `mqtt_reconnect_backoff_is_bounded_and_non_spinning` | `not_ignored` |
| `DISC_ADB41A6325C30D12D0AD` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests.rs` | `mqtt_stale_snapshot_is_returned_without_fresh_payload` | `not_ignored` |
| `DISC_D2A5AD5E1D2E5D50D9E4` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests.rs` | `mqtt_write_outputs_returns_within_scan_bound_while_session_connecting` | `not_ignored` |
| `DISC_5B3814C108D3BE9E961E` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests.rs` | `on_error_ignore_publish_failure_degrades_without_runtime_error` | `not_ignored` |
| `DISC_ABF55C65457D6C2E3DC5` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests.rs` | `on_error_warn_connect_failure_degrades_without_runtime_error` | `not_ignored` |
| `DISC_2CD8F1025F7B3F7971AD` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests.rs` | `reconnection_test_retries_after_connect_failure` | `not_ignored` |
| `DISC_10EE742635D02FEE1FD0` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests.rs` | `security_test_allows_empty_alpn_list_when_tls_disabled` | `not_ignored` |
| `DISC_15CF0633A64EC02B3DA8` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests.rs` | `security_test_allows_remote_broker_when_tls_configured` | `not_ignored` |
| `DISC_B06CCB06F527B14FBDB7` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests.rs` | `security_test_mqtts_scheme_implies_tls` | `not_ignored` |
| `DISC_7A6E42CE1E45D7851EBA` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests.rs` | `security_test_rejects_partial_mtls_pair` | `not_ignored` |
| `DISC_6E9637EF82BD697EE58B` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests.rs` | `security_test_rejects_remote_insecure_broker` | `not_ignored` |
| `DISC_13707800B91C846699A7` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests.rs` | `security_test_rejects_tls_fields_when_tls_disabled` | `not_ignored` |
| `DISC_7F1A31260AC9C4EA430B` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests.rs` | `security_test_rejects_tls_without_ca_path` | `not_ignored` |
| `DISC_1F9BDE1F517843DD2034` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests.rs` | `sparkplug_payload_encoding_matches_tahu_wire_shape_for_scalar_metrics` | `not_ignored` |
| `DISC_638110A71180A4185830` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests.rs` | `sparkplug_profile_configures_namespace_topics_and_last_will` | `not_ignored` |
| `DISC_0E8A8AC3DB17328C023E` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests.rs` | `sparkplug_profile_publishes_birth_then_data_payload` | `not_ignored` |
| `DISC_3DF881287BC144F6C4BD` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests.rs` | `sparkplug_profile_rejects_unsupported_shapes` | `not_ignored` |
| `DISC_6A687E889288280B591C` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests.rs` | `tls_transport_test_builds_rumqttc_tls_transport` | `not_ignored` |
| `DISC_ED01117B4E7CC028C957` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests.rs` | `typed_point_map_reads_json_text_and_binary_payloads` | `not_ignored` |
| `DISC_86A13C88CFA096648AFB` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests.rs` | `typed_point_map_rejects_invalid_config` | `not_ignored` |
| `DISC_9F063632F635A83377F5` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests.rs` | `typed_point_map_writes_json_text_and_binary_payloads` | `not_ignored` |
| `DISC_ED86AD6897709C84EF42` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests/safe_state.rs` | `mqtt_safe_state_handoff_succeeds_when_worker_confirms_publish` | `not_ignored` |
| `DISC_E1984AB7BA18E2EA3F4A` | `rust_unit_test` | `crates/trust-runtime/src/io/registry.rs` | `alias_resolves_to_canonical_driver_name` | `not_ignored` |
| `DISC_2A1FE3B0F9A175317636` | `rust_unit_test` | `crates/trust-runtime/src/io/registry.rs` | `canonical_driver_names_are_sorted_unique` | `not_ignored` |
| `DISC_E9E51ECA704D26D35216` | `rust_unit_test` | `crates/trust-runtime/src/memory/tests.rs` | `borrowed_value_ref_helpers_match_owned_helpers` | `not_ignored` |
| `DISC_B107BE8F6A6D5BCBE3AE` | `rust_unit_test` | `crates/trust-runtime/src/memory/tests.rs` | `declared_instance_field_offset_reuses_type_layout_for_declared_fields` | `not_ignored` |
| `DISC_BF7E6BB515C5DD73BED7` | `rust_unit_test` | `crates/trust-runtime/src/memory/tests.rs` | `declared_instance_field_offset_skips_inherited_fields` | `not_ignored` |
| `DISC_0B2EF754DB448B6AD3AB` | `rust_unit_test` | `crates/trust-runtime/src/memory/tests.rs` | `direct_instance_field_miss_cache_invalidates_on_new_insert` | `not_ignored` |
| `DISC_D8948B3221BCB14EF064` | `rust_unit_test` | `crates/trust-runtime/src/memory/tests.rs` | `direct_instance_field_offset_reads_and_writes_without_value_ref` | `not_ignored` |
| `DISC_A122DD61BD08854DA843` | `rust_unit_test` | `crates/trust-runtime/src/memory/tests.rs` | `direct_slot_helpers_cover_global_local_and_instance_locations` | `not_ignored` |
| `DISC_C40B82EF2E40FB976CA8` | `rust_unit_test` | `crates/trust-runtime/src/memory/tests.rs` | `direct_slot_helpers_match_empty_path_ref_helpers` | `not_ignored` |
| `DISC_5B2577C9099ED6AC210B` | `rust_unit_test` | `crates/trust-runtime/src/memory/tests.rs` | `instance_field_cache_is_scoped_per_instance` | `not_ignored` |
| `DISC_CF68122CFF8B2562AA1D` | `rust_unit_test` | `crates/trust-runtime/src/memory/tests.rs` | `read_and_write_by_ref_handle_extreme_array_bounds_without_overflow` | `not_ignored` |
| `DISC_430AF663A79A7D319743` | `rust_unit_test` | `crates/trust-runtime/src/memory/tests.rs` | `read_and_write_by_ref_non_ascii_string_uses_character_elements` | `not_ignored` |
| `DISC_04C4A7BC104AAEA187FF` | `rust_unit_test` | `crates/trust-runtime/src/memory/tests.rs` | `recursive_instance_field_cache_invalidates_when_child_adds_shadowing_field` | `not_ignored` |
| `DISC_FA0F9BAEDAA26D0A7E7D` | `rust_unit_test` | `crates/trust-runtime/src/memory/tests.rs` | `recursive_lookup_does_not_cache_parent_chain_miss` | `not_ignored` |
| `DISC_40CA4A0F3E9B495A1A30` | `rust_unit_test` | `crates/trust-runtime/src/memory/tests.rs` | `resolved_instance_field_ref_prefers_direct_field_before_parent_fallback` | `not_ignored` |
| `DISC_BFF56775AA7A39010698` | `rust_unit_test` | `crates/trust-runtime/src/memory/tests.rs` | `variable_storage_clone_recovers_from_poisoned_cache_lock` | `not_ignored` |
| `DISC_AC10CF406B3B1695588B` | `rust_unit_test` | `crates/trust-runtime/src/memory/tests.rs` | `write_by_ref_path_preserves_struct_copy_on_write_isolation` | `not_ignored` |
| `DISC_03AC685F06772395F05A` | `rust_unit_test` | `crates/trust-runtime/src/openot_authoring/tests.rs` | `batch_recipe_lowering_uses_procedure_op_and_raw_definition_tables` | `not_ignored` |
| `DISC_5BBB12B235856C1C45CC` | `rust_unit_test` | `crates/trust-runtime/src/openot_authoring/tests.rs` | `compile_session_surfaces_openot_validation_failure_instead_of_building_uninstrumented_bytecode` | `not_ignored` |
| `DISC_CD6997A0F5D223035730` | `rust_unit_test` | `crates/trust-runtime/src/openot_authoring/tests.rs` | `condition_lifecycle_lowering_inherits_parent_and_emits_after_alarm_phase` | `not_ignored` |
| `DISC_C3BD87557BC4FF520AC2` | `rust_unit_test` | `crates/trust-runtime/src/openot_authoring/tests.rs` | `enum_state_definition_includes_enum_set` | `not_ignored` |
| `DISC_16AA1A9D6581D67CE487` | `rust_unit_test` | `crates/trust-runtime/src/openot_authoring/tests.rs` | `enum_state_lowering_uses_hir_enum_values` | `not_ignored` |
| `DISC_27F37A4E3AA95CD0CB99` | `rust_unit_test` | `crates/trust-runtime/src/openot_authoring/tests.rs` | `explicit_sourceid_collision_between_programs_is_rejected` | `not_ignored` |
| `DISC_CC8F224B4E548B15AD95` | `rust_unit_test` | `crates/trust-runtime/src/openot_authoring/tests.rs` | `generated_definition_contains_hash` | `not_ignored` |
| `DISC_59A85EA1355E3C706B83` | `rust_unit_test` | `crates/trust-runtime/src/openot_authoring/tests.rs` | `generated_definition_rejects_unsupported_value_types` | `not_ignored` |
| `DISC_FAAC8CE08EE4F121BF0B` | `rust_unit_test` | `crates/trust-runtime/src/openot_authoring/tests.rs` | `generated_definition_uses_canonical_unit_ids` | `not_ignored` |
| `DISC_B91E25CDD98DD7A64FD2` | `rust_unit_test` | `crates/trust-runtime/src/openot_authoring/tests.rs` | `generated_event_types_match_openot_reference_canonical_schema` | `not_ignored` |
| `DISC_E979CB5D7A257C7962FD` | `rust_unit_test` | `crates/trust-runtime/src/openot_authoring/tests.rs` | `hir_and_runtime_authoring_accept_omitted_sourceids_units_and_payload_widths` | `not_ignored` |
| `DISC_1466DAFE96EF828CB3AF` | `rust_unit_test` | `crates/trust-runtime/src/openot_authoring/tests.rs` | `hir_and_runtime_authoring_report_explicit_sourceid_collisions_consistently` | `not_ignored` |
| `DISC_E45EABA362C60CF7F26D` | `rust_unit_test` | `crates/trust-runtime/src/openot_authoring/tests.rs` | `instruments_fixed_width_value_types_with_generic_value_op` | `not_ignored` |
| `DISC_2EB871F64EF0DF0BFAAB` | `rust_unit_test` | `crates/trust-runtime/src/openot_authoring/tests.rs` | `instruments_simple_program_attributes` | `not_ignored` |
| `DISC_EC921A6789B607F963A4` | `rust_unit_test` | `crates/trust-runtime/src/openot_authoring/tests.rs` | `multi_program_omitted_sourceids_are_assigned_distinctly` | `not_ignored` |
| `DISC_3D6126C1DD08F111BACE` | `rust_unit_test` | `crates/trust-runtime/src/openot_authoring/tests.rs` | `operator_regulated_lowering_uses_regulated_op_and_auth_symbols` | `not_ignored` |
| `DISC_393BF53218BD4FC17756` | `rust_unit_test` | `crates/trust-runtime/src/openot_authoring/tests.rs` | `pinned_ids_are_stable_when_declarations_are_reordered` | `not_ignored` |
| `DISC_445B5349EE273251602F` | `rust_unit_test` | `crates/trust-runtime/src/openot_authoring/tests.rs` | `state_category_defaults_to_process` | `not_ignored` |
| `DISC_DB3D37B0B8FABFF4F1C7` | `rust_unit_test` | `crates/trust-runtime/src/openot_authoring/tests.rs` | `unpinned_ids_follow_declaration_order` | `not_ignored` |
| `DISC_E642186822BCFD962E89` | `rust_unit_test` | `crates/trust-runtime/src/openot_authoring/tests.rs` | `value_quality_semantic_role_and_previous_lowering_are_explicit` | `not_ignored` |
| `DISC_9E26CAE8415CE2A02092` | `rust_unit_test` | `crates/trust-runtime/src/openot_authoring/tests.rs` | `value_sampling_policy_lowering_uses_existing_definition_field` | `not_ignored` |
| `DISC_402CC51B509F2C268C12` | `rust_unit_test` | `crates/trust-runtime/src/retain/store_tests.rs` | `file_retain_store_open_and_read_failures_are_visible` | `not_ignored` |
| `DISC_55C765213824F6A01A03` | `rust_unit_test` | `crates/trust-runtime/src/retain/store_tests.rs` | `file_retain_store_parent_sync_failure_is_visible_after_publish` | `not_ignored` |
| `DISC_A8FA0CAF53A7D6D0134E` | `rust_unit_test` | `crates/trust-runtime/src/retain/store_tests.rs` | `file_retain_store_pre_publish_failure_matrix_preserves_last_good_snapshot` | `not_ignored` |
| `DISC_414C6A35015C7813CC49` | `rust_unit_test` | `crates/trust-runtime/src/retain/store_tests.rs` | `file_retain_store_replaces_snapshot_atomically` | `not_ignored` |
| `DISC_65D98FB688680CA72C3E` | `rust_unit_test` | `crates/trust-runtime/src/retain/store_tests.rs` | `retain_manager_retries_failed_save_without_a_new_dirty_mark` | `not_ignored` |
| `DISC_197779F9246D3ACBFDA1` | `rust_unit_test` | `crates/trust-runtime/src/runtime/ads_subsystem.rs` | `active_device_snapshot_matches_configured_route_without_socket_io` | `not_ignored` |
| `DISC_FC0B82AB84AA926CB0AC` | `rust_unit_test` | `crates/trust-runtime/src/runtime/ads_subsystem.rs` | `empty_ads_subsystem_reports_disabled_status` | `not_ignored` |
| `DISC_5486EDCA7EBB23E006DC` | `rust_unit_test` | `crates/trust-runtime/src/runtime/ads_subsystem.rs` | `status_overall_reports_degraded_for_stale_or_degraded_points` | `not_ignored` |
| `DISC_F90E9372AD59D3E7BD4E` | `rust_unit_test` | `crates/trust-runtime/src/runtime/ads_subsystem.rs` | `status_overall_reports_fault_before_degraded_or_healthy` | `not_ignored` |
| `DISC_8EE97F3B6BB7F26EC5C5` | `rust_unit_test` | `crates/trust-runtime/src/runtime/ads_subsystem.rs` | `status_report_includes_deployed_ads_config_hash` | `not_ignored` |
| `DISC_5EDF42B452D0B1EDDA96` | `rust_unit_test` | `crates/trust-runtime/src/runtime/restart.rs` | `apply_retain_snapshot_canonicalizes_array_of_struct_and_rejects_bad_element` | `not_ignored` |
| `DISC_82C2C9A40CD6B6417DA6` | `rust_unit_test` | `crates/trust-runtime/src/runtime/restart.rs` | `apply_retain_snapshot_canonicalizes_legacy_enum_type_name` | `not_ignored` |
| `DISC_7CD5D444A8A4F7601610` | `rust_unit_test` | `crates/trust-runtime/src/runtime/restart.rs` | `apply_retain_snapshot_canonicalizes_nested_enum_in_struct` | `not_ignored` |
| `DISC_5B7344B729E5C62F8DE3` | `rust_unit_test` | `crates/trust-runtime/src/runtime/restart.rs` | `apply_retain_snapshot_rejects_struct_field_type_drift` | `not_ignored` |
| `DISC_24F5361F09E346062428` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/budget.rs` | `budget_accepts_exact_remaining_work_and_rejects_the_next_instruction` | `not_ignored` |
| `DISC_2EEF1852335396B08229` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/budget.rs` | `rejected_charge_does_not_underflow_the_remaining_budget` | `not_ignored` |
| `DISC_A8CA6B14637EBEF73098` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/argument_binding.rs` | `bind_conversion_value_accepts_positional_and_named_in_only` | `not_ignored` |
| `DISC_39BAF5ED40CAFA87A0FD` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/argument_binding.rs` | `bind_vm_function_block_arguments_skips_omitted_inout_without_field_resolution` | `not_ignored` |
| `DISC_43A1951229C19A1BB4B5` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/argument_binding.rs` | `bind_vm_function_block_arguments_skips_omitted_out_without_field_resolution` | `not_ignored` |
| `DISC_C66AD9C183B5B6475D87` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/argument_binding.rs` | `preparse_native_symbol_spec_caches_conversion_spec` | `not_ignored` |
| `DISC_0CAFC9BC4204EE9621E6` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/argument_binding.rs` | `preparse_native_symbol_spec_parses_named_and_target_args` | `not_ignored` |
| `DISC_3A46BEEAEFA832F8DEC8` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/argument_binding.rs` | `resolve_named_arg_index_advances_nonzero_ordered_match` | `not_ignored` |
| `DISC_EEBAACB3A0063F87C1BE` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/argument_binding.rs` | `resolve_named_arg_index_falls_back_for_out_of_order_named_arguments` | `not_ignored` |
| `DISC_095588957CC371875700` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/argument_binding.rs` | `resolve_named_arg_index_handles_omitted_middle_parameter` | `not_ignored` |
| `DISC_71E1DAB63FB565A32B8D` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/argument_binding.rs` | `resolve_named_arg_index_prefers_in_order_next_argument` | `not_ignored` |
| `DISC_EEE4C38C8574AE5F00F0` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/argument_binding.rs` | `resolve_named_arg_index_skips_consumed_prefix_and_stops_at_end` | `not_ignored` |
| `DISC_BFD7A113895A6E114A12` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/argument_binding.rs` | `unpack_native_call_payload_preserves_receiver_and_argument_order` | `not_ignored` |
| `DISC_44265FFE3BEC9F51A755` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/function_blocks.rs` | `bind_builtin_function_block_arguments_accepts_exact_positional_and_rejects_extra` | `not_ignored` |
| `DISC_8F8A5B483EC9081D0057` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/function_blocks.rs` | `bind_builtin_function_block_arguments_allows_omitted_positional_outputs` | `not_ignored` |
| `DISC_F07630D1EBBC98AB8CF4` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/function_blocks.rs` | `bind_builtin_function_block_arguments_binds_named_inputs_and_outputs` | `not_ignored` |
| `DISC_9B7320934E9CC91E517C` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/function_blocks.rs` | `bind_builtin_function_block_arguments_preserves_omitted_inputs` | `not_ignored` |
| `DISC_899B2E90305040F3AAC2` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/function_blocks.rs` | `bind_builtin_function_block_arguments_supports_inout_rebinding` | `not_ignored` |
| `DISC_D4BB43F8E69973A04CEE` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/function_blocks.rs` | `bind_vm_call_arguments_accepts_exact_positional_and_rejects_extra` | `not_ignored` |
| `DISC_33CDA38D12DF0B5A47DF` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/function_blocks.rs` | `bind_vm_call_arguments_allows_omitted_trailing_positional_input` | `not_ignored` |
| `DISC_78576619FCCC49506243` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/function_blocks.rs` | `bind_vm_call_arguments_rejects_legacy_untyped_string_output_target` | `not_ignored` |
| `DISC_2DA737B4BF7113D48C66` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/function_blocks.rs` | `bind_vm_call_arguments_rejects_string_inout_capacity_mismatch_before_copy_in` | `not_ignored` |
| `DISC_C4EF6F27B3AA9F8191F8` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/function_blocks.rs` | `bind_vm_function_block_arguments_accepts_exact_positional_and_rejects_extra` | `not_ignored` |
| `DISC_744D0B68EABC74FACF20` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/function_blocks.rs` | `bind_vm_function_block_arguments_preserves_omitted_input_field` | `not_ignored` |
| `DISC_0950D387955107F02BF3` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/function_blocks.rs` | `bind_vm_function_block_arguments_rejects_string_inout_capacity_mismatch_before_copy_in` | `not_ignored` |
| `DISC_DD63F8BE8583CC36BF9D` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/function_blocks.rs` | `bind_vm_function_block_arguments_supports_mixed_out_and_inout_rebinding` | `not_ignored` |
| `DISC_C956B7E85B79CD07BBF3` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/function_blocks.rs` | `output_binding_rejects_conflicting_reference_types_for_nonstring_target` | `not_ignored` |
| `DISC_4B315CC616CA1DDAE2AC` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/function_blocks.rs` | `output_copyback_rejects_nonstring_for_declared_string_null_target` | `not_ignored` |
| `DISC_5BCBCB959CB5A90481AA` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/function_blocks.rs` | `output_copyback_rejects_untyped_null_string_target_without_writing` | `not_ignored` |
| `DISC_7B49CF6FDD6AE7918B4E` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/stdlib_binding.rs` | `bind_stdlib_named_values_fixed_reorders_by_parameter_order` | `not_ignored` |
| `DISC_8831D3C8B1B39949E02B` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/stdlib_binding.rs` | `bind_stdlib_named_values_rejects_duplicate_named_argument` | `not_ignored` |
| `DISC_9021DDAE534AEED37274` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/stdlib_binding.rs` | `bind_stdlib_named_values_variadic_rejects_hole` | `not_ignored` |
| `DISC_4F81E47208CE82A45991` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/stdlib_binding.rs` | `bind_stdlib_named_values_variadic_reorders_suffixes` | `not_ignored` |
| `DISC_B4C78E564D3865C8D906` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/stdlib_binding.rs` | `bind_stdlib_named_values_variadic_reports_exact_count_edges` | `not_ignored` |
| `DISC_C6A793B61519F636E044` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/stdlib_binding.rs` | `bind_stdlib_positional_values_enforces_fixed_plus_variadic_minimum` | `not_ignored` |
| `DISC_4C97BA2A4E0E7380B24A` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/stdlib_binding.rs` | `bind_vm_call_arguments_keeps_omitted_middle_named_input_as_null` | `not_ignored` |
| `DISC_7939BBA149F9D8D5A5D0` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/stdlib_binding.rs` | `bind_vm_call_arguments_rejects_too_many_positional_arguments` | `not_ignored` |
| `DISC_B4E31A6E3B65DA4DBD81` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/stdlib_binding.rs` | `dispatch_native_split_date_positional_writes_outputs_and_checks_arity` | `not_ignored` |
| `DISC_EA4879AE140322424ACE` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/stdlib_binding.rs` | `dispatch_native_split_named_rejects_duplicate_output_name` | `not_ignored` |
| `DISC_AE6834FE65142B81BCBC` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/stdlib_binding.rs` | `dispatch_native_split_named_variants_write_outputs` | `not_ignored` |
| `DISC_1074D31C3333C3DF116C` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/stdlib_binding.rs` | `dispatch_native_stdlib_binds_fixed_and_variadic_positional_values` | `not_ignored` |
| `DISC_C0962576D2DD5F0601B8` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/stdlib_binding.rs` | `dispatch_native_stdlib_runtime_clock_accepts_zero_args_only` | `not_ignored` |
| `DISC_C0B16744A7C2D1EB8BCA` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/stdlib_binding.rs` | `resolve_native_symbol_specs_caches_resolved_function_id` | `not_ignored` |
| `DISC_55AB5B2B6B331C1E345C` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/write_targets.rs` | `preparse_native_symbol_spec_preserves_parse_error_message` | `not_ignored` |
| `DISC_26463DA494D8A83680CC` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/write_targets.rs` | `read_vm_target_value_avoids_clone_counter_for_scalar_direct_target` | `not_ignored` |
| `DISC_A6480B30C5B364488520` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/write_targets.rs` | `read_vm_target_value_matches_generic_reference_path_across_reference_shapes` | `not_ignored` |
| `DISC_CC040147DEC6F2E25DFB` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/write_targets.rs` | `vm_fb_field_binding_out_source_falls_back_to_reference_for_inherited_fields` | `not_ignored` |
| `DISC_8AEE4E16496D824A39F7` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/write_targets.rs` | `vm_fb_field_binding_out_source_uses_direct_for_declared_fields` | `not_ignored` |
| `DISC_79A62C001BA92D78CAA2` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/write_targets.rs` | `vm_fb_field_binding_reads_writes_and_reports_invalid_direct_offset` | `not_ignored` |
| `DISC_ECBCBD0F46056B36F68D` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/write_targets.rs` | `vm_fb_out_source_reads_direct_instance_field` | `not_ignored` |
| `DISC_8B03EE7D41D24D63F4A4` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/write_targets.rs` | `vm_fb_out_source_reads_reference_field` | `not_ignored` |
| `DISC_94C188AC355894A377EA` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/write_targets.rs` | `vm_write_target_keeps_nested_path_targets_on_reference_fallback` | `not_ignored` |
| `DISC_7897703E07BFB721A4ED` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/write_targets.rs` | `vm_write_target_uses_caller_local_direct_for_empty_path_vm_locals` | `not_ignored` |
| `DISC_3CCB4095994DA45DF913` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/write_targets.rs` | `vm_write_target_uses_direct_storage_for_empty_path_global_refs` | `not_ignored` |
| `DISC_959F4DDA54FBE3A5A5D4` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/write_targets.rs` | `vm_write_target_uses_direct_storage_for_empty_path_instance_refs` | `not_ignored` |
| `DISC_2A9A0F79204A6ED7D3B3` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/write_targets.rs` | `write_output_int_inspects_target_type_without_read_clone` | `not_ignored` |
| `DISC_93D139E1CD65732E6283` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/write_targets.rs` | `write_output_int_preserves_integer_target_widths` | `not_ignored` |
| `DISC_2AE7AD93B6461478B0A1` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/write_targets.rs` | `write_output_int_rejects_unsigned_negative_values` | `not_ignored` |
| `DISC_DBB7825CF82673CCDABB` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/call/tests/write_targets.rs` | `write_vm_reference_updates_nested_vm_local_path` | `not_ignored` |
| `DISC_4D564DAB0E3D3BCE1516` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/dispatch.rs` | `stack_deadline_stride_checks_first_and_stride_boundaries` | `not_ignored` |
| `DISC_55C34DE0597D9D5F1B55` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/dispatch_refs.rs` | `dynamic_ref_field_resolves_instance_field_reference` | `not_ignored` |
| `DISC_BC38A7D2DDC566E14F49` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/dispatch_refs.rs` | `dynamic_ref_index_extends_nested_partial_index_against_array_shape` | `not_ignored` |
| `DISC_C7D15FFABDA348B42627` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/dispatch_refs.rs` | `dynamic_ref_index_extends_partial_index_against_array_shape` | `not_ignored` |
| `DISC_42E9DE659419345AAE27` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/dispatch_refs.rs` | `peek_dynamic_ref_borrows_global_storage_value` | `not_ignored` |
| `DISC_E373A62882AB2F3A1763` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/dispatch_refs.rs` | `peek_dynamic_ref_borrows_local_sentinel_value` | `not_ignored` |
| `DISC_231B58665A0477064F62` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/dispatch_refs.rs` | `read_and_write_value_path_handle_extreme_array_bounds_without_overflow` | `not_ignored` |
| `DISC_4B09734A8D1856C5E38A` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/dispatch_refs.rs` | `read_and_write_value_path_non_ascii_string_uses_character_elements` | `not_ignored` |
| `DISC_C618FB07BBB103B201F0` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/local_init.rs` | `vm_interface_return_slot_defaults_to_null` | `not_ignored` |
| `DISC_2D00101FB1C43F085F63` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/local_init.rs` | `vm_local_interface_defaults_to_null` | `not_ignored` |
| `DISC_4AB7155B7181CF058808` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/mod.rs` | `infer_primary_instance_owner_returns_none_for_ambiguous_owners` | `not_ignored` |
| `DISC_B95DF7DD235A4312B411` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/mod.rs` | `infer_primary_instance_owner_scans_partial_access_operands` | `not_ignored` |
| `DISC_1D15CD11714BA5CEFA4B` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/backend_parity_followup.rs` | `vmpar_instruction_budget_faults_at_the_same_original_instruction_boundary` | `not_ignored` |
| `DISC_8EF2B55E3EBA03FF4321` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/backend_parity_followup.rs` | `vmpar_nested_function_call_shares_the_top_level_instruction_budget` | `not_ignored` |
| `DISC_4C3F5D7171073A654EA8` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/backend_parity_followup.rs` | `vmpar_register_deadline_traps_without_fallback_or_commit` | `not_ignored` |
| `DISC_769ABE73CC183605DE35` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/backend_parity_followup.rs` | `vmpar_stack_deadline_traps_before_forward_workload_commits` | `not_ignored` |
| `DISC_B2DECE92E4F87D0C7CDB` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/backend_parity_followup.rs` | `vmpar_stack_register_and_tier1_paths_produce_expected_forward_value` | `not_ignored` |
| `DISC_7E40E58CEF1BE918297B` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/backend_parity_followup.rs` | `vmpar_tier1_deadline_traps_in_compiled_block_without_commit` | `not_ignored` |
| `DISC_6B5592847914F04017A7` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/diagnostics.rs` | `diagnostic_execute_corpus_through_register_ir` | `not_ignored` |
| `DISC_E5636422E644F381730F` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/diagnostics.rs` | `diagnostic_find_fallback_opcodes_in_corpus` | `not_ignored` |
| `DISC_286A2DB4867D072EBD81` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/diagnostics.rs` | `diagnostic_register_ir_callee_path_populates_lowering_cache` | `not_ignored` |
| `DISC_2CF4382E09B4A85E80F0` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/function_blocks.rs` | `register_executor_runs_program_with_complex_local_fields_without_fallback` | `not_ignored` |
| `DISC_07FBB7D506873509F989` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/function_blocks.rs` | `register_executor_tier1_specialized_executor_executes_array_ref_blocks` | `not_ignored` |
| `DISC_3911B97AE9075CB3498D` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/function_blocks.rs` | `register_ir_lowering_fuses_self_field_dynamic_load_store` | `not_ignored` |
| `DISC_6521C6CB6BC337195C4C` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/function_blocks.rs` | `register_ir_lowering_handles_function_block_self_fields_without_fallback` | `not_ignored` |
| `DISC_84B31731670E73C95CA6` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/function_blocks.rs` | `register_lowering_error_fallback_reason_includes_pou_name_and_message` | `not_ignored` |
| `DISC_C60D95E911C945A83FF2` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/function_blocks.rs` | `tier1_compiler_accepts_function_block_index_dynamic_ops` | `not_ignored` |
| `DISC_15830047C9E8379021A0` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/function_blocks.rs` | `tier1_compiler_accepts_function_block_self_field_dynamic_ops` | `not_ignored` |
| `DISC_72910D0980A8D74C8153` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/decode_and_lowering.rs` | `register_executor_runs_case_program_without_fallback` | `not_ignored` |
| `DISC_3F17806BDE8F610DDEC9` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/decode_and_lowering.rs` | `register_executor_runs_string_case_program_without_fallback` | `not_ignored` |
| `DISC_4B79F627B3277CB50A70` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/decode_and_lowering.rs` | `register_ir_decode_leaders_exclude_exit_and_unconditional_fallthrough` | `not_ignored` |
| `DISC_B51A2D04101F1C97AEAB` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/decode_and_lowering.rs` | `register_ir_decode_rejects_conflicting_block_entry_depths` | `not_ignored` |
| `DISC_709175C8FDB03A84A575` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/decode_and_lowering.rs` | `register_ir_decode_rejects_rot_underflow_and_accepts_exact_depth` | `not_ignored` |
| `DISC_0A54FF147F9DDC1E6583` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/decode_and_lowering.rs` | `register_ir_decode_return_stops_entry_depth_propagation` | `not_ignored` |
| `DISC_425B24E21E8320A00248` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/decode_and_lowering.rs` | `register_ir_lowering_accepts_valid_call_native_and_swap_stack_depths` | `not_ignored` |
| `DISC_F6B80C0E8905DA0DE6B4` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/decode_and_lowering.rs` | `register_ir_lowering_covers_nop_null_and_full_binary_opcode_family` | `not_ignored` |
| `DISC_BA4DB3794DFC2D2FA96E` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/decode_and_lowering.rs` | `register_ir_lowering_does_not_normalize_after_return` | `not_ignored` |
| `DISC_7CB7D841D9F98BAA3FB6` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/decode_and_lowering.rs` | `register_ir_lowering_emits_control_flow_blocks_for_loops` | `not_ignored` |
| `DISC_33A8AF3EB0435C5EE571` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/decode_and_lowering.rs` | `register_ir_lowering_handles_case_selector_live_across_branch_blocks` | `not_ignored` |
| `DISC_4E20FF414E04DD825461` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/decode_and_lowering.rs` | `register_ir_lowering_handles_string_case_selector` | `not_ignored` |
| `DISC_EFC56644DA203DABB116` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/decode_and_lowering.rs` | `register_ir_lowering_preserves_fallback_operands` | `not_ignored` |
| `DISC_42DFD55B7394E22F01DF` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/decode_and_lowering.rs` | `register_ir_stack_normalization_preserves_protected_registers_and_cycles` | `not_ignored` |
| `DISC_7BF20E25879691FCCF92` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/executor_cases.rs` | `register_executor_fb_omitted_input_uses_initializer_then_reuses_stored_value` | `not_ignored` |
| `DISC_19EFDD0282153F53CD71` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/executor_cases.rs` | `register_executor_progresses_motion_demo_to_step_40_without_error_by_cycle_three` | `not_ignored` |
| `DISC_07DB65B8436213FA048D` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/executor_cases.rs` | `register_executor_runs_case_branch_with_nested_if_without_fallback` | `not_ignored` |
| `DISC_91B5920B6F9D0770DD5B` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/executor_cases.rs` | `register_executor_runs_multi_label_case_program_without_fallback` | `not_ignored` |
| `DISC_6068C36CBB0942531FE5` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/fusion.rs` | `register_ir_fuse_covers_compare_jump_guards` | `not_ignored` |
| `DISC_6FB497689ED784E5DCF5` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/fusion.rs` | `register_ir_fuse_covers_ref_binary_variants_and_guard_failures` | `not_ignored` |
| `DISC_51FAD8D70276D6AD9276` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/fusion.rs` | `register_ir_fuse_instruction_read_detection_covers_all_operands` | `not_ignored` |
| `DISC_A8306A2EE373A53ADB86` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/fusion.rs` | `register_ir_fuse_rejects_partial_self_field_dynamic_windows` | `not_ignored` |
| `DISC_E04E58B180F17322D827` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/fusion_support.rs` | `register_ir_fuse_preserves_unmatched_windows_and_fused_tail` | `not_ignored` |
| `DISC_01FF4844DCFE49E1E325` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/fusion_support.rs` | `register_ir_fusion_preserves_original_bytecode_instruction_costs` | `not_ignored` |
| `DISC_286FF29DA9DB9D790C4F` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/fusion_support.rs` | `register_ir_lowering_handles_linear_arithmetic_main` | `not_ignored` |
| `DISC_9ECC8ADE95FD125D2ECC` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/verifier_and_parity.rs` | `dint_mod_zero_fast_path_matches_generic_error_contract` | `not_ignored` |
| `DISC_A45D628700DA0E8AEA0A` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/verifier_and_parity.rs` | `register_ir_lowering_rejects_invalid_jump_target` | `not_ignored` |
| `DISC_9033961FDC7ED097C3EE` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/verifier_and_parity.rs` | `register_ir_parity_matches_stack_subset_linear_program` | `not_ignored` |
| `DISC_B6AA0AA3D972CE219704` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/verifier_and_parity.rs` | `register_ir_parity_matches_stack_subset_loop_program` | `not_ignored` |
| `DISC_0CC70C150675D457490D` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/verifier_and_parity.rs` | `register_ir_verifier_rejects_missing_instruction_costs` | `not_ignored` |
| `DISC_5B912DAD1263AC34BCDC` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/verifier_and_parity.rs` | `register_ir_verifier_rejects_move_destination_out_of_bounds` | `not_ignored` |
| `DISC_D200FA08C237098387EF` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/verifier_and_parity.rs` | `register_ir_verifier_rejects_original_instruction_count_drift` | `not_ignored` |
| `DISC_5B87087311D6A3361AE7` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/verifier_and_parity.rs` | `register_ir_verifier_rejects_undefined_source_register` | `not_ignored` |
| `DISC_AB59020941E4FCFAB21B` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/lowering/verifier_and_parity.rs` | `register_ir_verifier_rejects_unknown_block_target` | `not_ignored` |
| `DISC_B45F29B68E0EF26A04A2` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/profile.rs` | `read_register_with_counts_records_clone_then_move_reads` | `not_ignored` |
| `DISC_1339C3F34D81F9037EFA` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/profile.rs` | `register_executor_falls_back_when_lowering_contains_unsupported_opcode` | `not_ignored` |
| `DISC_F0D6D66A63F032BFA86B` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/profile.rs` | `register_executor_profile_avoids_clone_counter_for_scalar_load_const` | `not_ignored` |
| `DISC_5E8B591951B2F5A69CC0` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/profile.rs` | `register_executor_profile_avoids_clone_counters_for_borrowed_ref_const_binary_guard` | `not_ignored` |
| `DISC_56FC1BE4A7F0D7E98847` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/profile.rs` | `register_executor_profile_avoids_clone_counters_for_borrowed_ref_const_non_dint_binary` | `not_ignored` |
| `DISC_3FEA24E8B45831504EB9` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/profile.rs` | `register_executor_profile_avoids_clone_counters_for_borrowed_ref_ref_non_dint_binary` | `not_ignored` |
| `DISC_65BCA9FA03DA39C710D6` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/profile.rs` | `register_executor_profile_avoids_clone_counters_for_struct_inout_function_block` | `not_ignored` |
| `DISC_44B8176DE36391A902EA` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/profile.rs` | `register_executor_profile_records_dynamic_ref_and_instance_lookup_counters` | `not_ignored` |
| `DISC_8CFCAC0A0ACB9913F461` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/profile.rs` | `register_executor_profile_records_fallback_reason` | `not_ignored` |
| `DISC_4A5C67E6A2C6696EAFE0` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/profile.rs` | `register_executor_profile_records_function_block_call_counters` | `not_ignored` |
| `DISC_4929C23DF5020BC6D864` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/profile.rs` | `register_executor_profile_records_hot_blocks_for_supported_program` | `not_ignored` |
| `DISC_59E213549872293A9FBF` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/profile.rs` | `register_executor_profile_records_ref_op_counters_for_load_ref_store_ref_program` | `not_ignored` |
| `DISC_287E423F22997ACB5D6A` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/profile.rs` | `register_executor_runs_supported_program` | `not_ignored` |
| `DISC_198AC339350EE7862D7A` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/support.rs` | `block_index_from_id_rejects_missing_and_mismatched_blocks` | `not_ignored` |
| `DISC_568746B1EBFDBEB42545` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/support.rs` | `deadline_exceeded_distinguishes_missing_past_and_future_deadlines` | `not_ignored` |
| `DISC_43B4A11BDCA200636374` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/support.rs` | `interpreted_ref_field_reports_null_reference_base` | `not_ignored` |
| `DISC_1859BF1BBDB76B7B736B` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/support.rs` | `next_linear_block_target_uses_following_block_not_current_block` | `not_ignored` |
| `DISC_02A9C126BCBCA60B89FC` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/support.rs` | `parse_env_bool_accepts_explicit_true_false_and_defaults` | `not_ignored` |
| `DISC_AE02817D6CB569DFC8B4` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/support.rs` | `prepare_register_file_resizes_truncates_and_preserves_values` | `not_ignored` |
| `DISC_E21BF0CC3C3A6301FDBA` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/support.rs` | `register_execution_buffers_return_clean_buffers_and_respect_pool_limit` | `not_ignored` |
| `DISC_536B1B2E12F5B93E76BA` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/support.rs` | `register_execution_rejects_initial_locals_beyond_frame_capacity` | `not_ignored` |
| `DISC_E9436E698BCD4E02A514` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/support.rs` | `register_read_helpers_preserve_bool_and_null_reference_errors` | `not_ignored` |
| `DISC_E6C75C05D60B64CA90BD` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/support.rs` | `register_statement_location_resolves_vm_debug_map_entries` | `not_ignored` |
| `DISC_6DBF2D449DC1A9C28F63` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/tier1/cache_and_guards.rs` | `register_executor_tier1_specialized_executor_keeps_startup_path_cold_until_hot_threshold` | `not_ignored` |
| `DISC_E29B24D47064A2C62712` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/tier1/cache_and_guards.rs` | `register_lowering_cache_caches_lowering_errors` | `not_ignored` |
| `DISC_9E3B0A1B92760F8F0534` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/tier1/cache_and_guards.rs` | `register_lowering_cache_hits_after_first_execution` | `not_ignored` |
| `DISC_7FE662C0F161BEA58691` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/tier1/cache_and_guards.rs` | `tier1_compiler_accepts_all_fused_binary_register_forms` | `not_ignored` |
| `DISC_C426CBEF307FB53860CD` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/tier1/cache_and_guards.rs` | `tier1_compiler_accepts_cmp_ref_const_jump_only_for_comparisons` | `not_ignored` |
| `DISC_0E8F1F2572E5DD79079E` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/tier1/cache_and_guards.rs` | `tier1_dint_binary_guard_declines_unsupported_inputs` | `not_ignored` |
| `DISC_387301D63907D4396FD8` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/tier1/cache_and_guards.rs` | `tier1_dint_binary_guard_returns_exact_arithmetic_results` | `not_ignored` |
| `DISC_DC445C31BA17DE2A5BC0` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/tier1/cache_and_guards.rs` | `tier1_dint_binary_guard_returns_exact_comparison_results` | `not_ignored` |
| `DISC_0472C3A256BD5B1E49EA` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/tier1/calls_failures_cache.rs` | `register_executor_tier1_specialized_executor_cache_capacity_evicts_old_blocks` | `not_ignored` |
| `DISC_D3B4B765BDE229C714C6` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/tier1/calls_failures_cache.rs` | `register_executor_tier1_specialized_executor_cache_hits_reuse_compiled_block_arc` | `not_ignored` |
| `DISC_21C1B61DB62C55B76619` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/tier1/calls_failures_cache.rs` | `register_executor_tier1_specialized_executor_executes_function_block_call_block` | `not_ignored` |
| `DISC_ADD0FD8E22F548C5E643` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/tier1/calls_failures_cache.rs` | `register_executor_tier1_specialized_executor_executes_function_call_block` | `not_ignored` |
| `DISC_28297EEF6C889EF7ED14` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/tier1/calls_failures_cache.rs` | `register_executor_tier1_specialized_executor_executes_non_dint_binary_without_deopt` | `not_ignored` |
| `DISC_B36860C46DD99C291E82` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/tier1/calls_failures_cache.rs` | `register_executor_tier1_specialized_executor_records_compile_failure_reason_for_unsupported_instruction` | `not_ignored` |
| `DISC_9C79BE6B7B68C9D23E15` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/tier1/calls_failures_cache.rs` | `tier1_compiler_accepts_call_native_function_blocks` | `not_ignored` |
| `DISC_3A6CE282AD96D19AA152` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/tier1/load_ref_super_bool.rs` | `register_executor_tier1_specialized_executor_executes_bool_or_without_deopt` | `not_ignored` |
| `DISC_605A00C537D72F41FA81` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/tier1/load_ref_super_bool.rs` | `register_executor_tier1_specialized_executor_executes_load_ref_addr_block` | `not_ignored` |
| `DISC_A6504910A2B67139A825` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/tier1/load_ref_super_bool.rs` | `register_executor_tier1_specialized_executor_executes_load_super_block` | `not_ignored` |
| `DISC_BAB3807A7E053B2BC5D4` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/tier1/load_ref_super_bool.rs` | `tier1_compiler_accepts_load_ref_addr_dynamic_block` | `not_ignored` |
| `DISC_C306D435B2BA7854C0B3` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/tier1/load_ref_super_bool.rs` | `tier1_compiler_accepts_load_super_dynamic_block` | `not_ignored` |
| `DISC_775E89618CD46526919D` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/tier1/load_ref_super_bool.rs` | `tier1_executor_cmp_ref_const_jump_takes_matching_branch` | `not_ignored` |
| `DISC_DACDD3A6D0D36B117FEA` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/tier1/load_ref_super_bool.rs` | `tier1_executor_jump_if_takes_matching_branch` | `not_ignored` |
| `DISC_6FADE7559951DE07EFB2` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/tier1/load_ref_super_bool.rs` | `tier1_executor_rejects_null_reference_ref_field` | `not_ignored` |
| `DISC_C44FB76BB87F2D4F4799` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/tier1/state_deadline_buffers.rs` | `register_deadline_stride_checks_first_and_stride_boundaries` | `not_ignored` |
| `DISC_BA5F722E76D5C8AB5564` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/tier1/state_deadline_buffers.rs` | `register_execution_buffers_reuse_clears_frames_and_register_files` | `not_ignored` |
| `DISC_29F852A3670A263B4BBA` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/tier1/state_deadline_buffers.rs` | `register_executor_tier1_env_parsers_accept_tokens_and_defaults` | `not_ignored` |
| `DISC_98548DA31025D64429F2` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/tier1/state_deadline_buffers.rs` | `register_executor_tier1_state_defaults_disabled` | `not_ignored` |
| `DISC_ABD235BEB6C3569D387B` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/tier1/state_deadline_buffers.rs` | `register_executor_tier1_state_from_env_reads_threshold_and_cache` | `not_ignored` |
| `DISC_5BB73AD55EC9E14E7E44` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/register_ir/tests/tier1/state_deadline_buffers.rs` | `register_executor_tier1_state_reset_clears_cache_and_counters` | `not_ignored` |
| `DISC_BC4FC03D5856D96E169D` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/config_policy.rs` | `desired_write_merges_json_and_sets_pending_status` | `not_ignored` |
| `DISC_BD7A4A6BD216C0A42F18` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/contracts.rs` | `api_version_parsing_and_compatibility_follow_contract_rules` | `not_ignored` |
| `DISC_204BDEE375FADCE699C6` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/contracts.rs` | `catalog_epoch_cache_requests_refresh_only_on_monotonic_increase` | `not_ignored` |
| `DISC_FBA69AA21BE4D0FA6460` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/contracts.rs` | `cloud_contract_payloads_round_trip_with_reason_codes` | `not_ignored` |
| `DISC_073A7799860E865C0654` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/contracts.rs` | `schema_layout_accepts_forward_additive_changes_and_rejects_breaking_changes` | `not_ignored` |
| `DISC_3345DCAA227ED6CDA264` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/control_proxy_policy.rs` | `proxy_plan_rejects_breaking_api_version` | `not_ignored` |
| `DISC_D3DAB148A8A39E7D4E1C` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/control_proxy_policy.rs` | `proxy_plan_uses_cfg_apply_for_config_set` | `not_ignored` |
| `DISC_79A1F9CF986CC031A573` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/control_proxy_policy.rs` | `proxy_plan_uses_generated_request_id_when_missing` | `not_ignored` |
| `DISC_40C3EA8EB0937846D830` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/control_proxy_policy.rs` | `proxy_plan_uses_status_read_for_viewer_control_request` | `not_ignored` |
| `DISC_E87D0C9A69CB1198199F` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/control_proxy_policy.rs` | `proxy_role_denial_uses_cfg_write_code_for_config_set` | `not_ignored` |
| `DISC_C6180DB055825AFDF39E` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/ha/tests.rs` | `crash_mid_command_can_retry_after_state_roundtrip` | `not_ignored` |
| `DISC_163AA34BC9E780208B26` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/ha/tests.rs` | `dual_host_requires_external_consistent_lease_authority` | `not_ignored` |
| `DISC_9DBC029B6F5A323DB0F1` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/ha/tests.rs` | `lease_loss_for_active_runtime_requires_demoted_safe_behavior` | `not_ignored` |
| `DISC_3A2F61FF4E9BAB7722CF` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/ha/tests.rs` | `parse_action_ha_request_accepts_optional_payload` | `not_ignored` |
| `DISC_2B7F7617315F5D2755DC` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/ha/tests.rs` | `replay_guard_deduplicates_and_rejects_stale_sequences` | `not_ignored` |
| `DISC_4D7E58F0B6F5D569062C` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/ha/tests.rs` | `split_brain_candidates_are_detected_and_rejected` | `not_ignored` |
| `DISC_00CDCF8476E6105C4E24` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/io_proxy_policy.rs` | `read_plan_uses_legacy_target_error_text` | `not_ignored` |
| `DISC_9D30AD217D36C3F1BE8D` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/io_proxy_policy.rs` | `read_plan_uses_status_read_action` | `not_ignored` |
| `DISC_3167F00C647153D0BC6A` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/io_proxy_policy.rs` | `write_plan_rejects_empty_actor` | `not_ignored` |
| `DISC_6004AE00254B84F48B3E` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/io_proxy_policy.rs` | `write_plan_uses_cfg_apply_action` | `not_ignored` |
| `DISC_0FE676D2386971450A4E` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/io_proxy_policy.rs` | `write_plan_validates_target_before_actor` | `not_ignored` |
| `DISC_E85D40089AC2740B2BA6` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/keyspace.rs` | `active_namespace_publish_requires_active_role` | `not_ignored` |
| `DISC_15B10B662DFC222CDA32` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/keyspace.rs` | `canonical_key_layout_matches_runtime_site_shape` | `not_ignored` |
| `DISC_A8E53EA19FFDC5F6B0B9` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/keyspace.rs` | `config_hierarchy_and_alias_paths_are_canonical` | `not_ignored` |
| `DISC_5E33E9799127CA989332` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/keyspace.rs` | `default_stale_timeout_is_max_of_twice_period_and_two_seconds` | `not_ignored` |
| `DISC_744B6713C35574084736` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/keyspace.rs` | `meta_key_helpers_cover_identity_catalog_shm_and_config_schema` | `not_ignored` |
| `DISC_581958D84F0FCF915D3B` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/keyspace.rs` | `reserved_zone_contract_is_meta_svc_and_diag` | `not_ignored` |
| `DISC_E90887BB780CFA9EB5F6` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/keyspace.rs` | `retained_last_value_is_ui_only_not_control` | `not_ignored` |
| `DISC_72808A2E94EB0751BF95` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/keyspace.rs` | `svc_key_helpers_cover_liveliness_role_and_lease_state` | `not_ignored` |
| `DISC_35D311B31025634B0ABE` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/profile_policy.rs` | `allowlist_supports_prefix_and_suffix_patterns` | `not_ignored` |
| `DISC_794F59E9A339F867023D` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/profile_policy.rs` | `wan_allowlist_parser_fuzz_smoke_budget` | `not_ignored` |
| `DISC_B7198EDB89BA61543797` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/profile_policy.rs` | `wan_profile_denies_write_without_matching_rule` | `not_ignored` |
| `DISC_DC8050B0CDE74CE80FC6` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/projection.rs` | `feature_flags_omitted_when_empty` | `not_ignored` |
| `DISC_FFAFE42736EA1CCDF677` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/projection.rs` | `feature_flags_roundtrips_when_present` | `not_ignored` |
| `DISC_2D34CFBB4B28655A887B` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/projection.rs` | `host_groups_omitted_when_empty` | `not_ignored` |
| `DISC_71BD6203CD4506887B5A` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/projection.rs` | `host_groups_roundtrips_when_present` | `not_ignored` |
| `DISC_AEB69E123677252EDA72` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/projection.rs` | `missing_host_groups_deserializes_to_empty` | `not_ignored` |
| `DISC_1F4A6A7E004592367309` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/projection.rs` | `presence_projection_contract_does_not_stale_future_heartbeat` | `not_ignored` |
| `DISC_F3555CD7ED8E088DF038` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/projection.rs` | `presence_projection_transitions_stale_before_partitioned` | `not_ignored` |
| `DISC_7057CE85A02EC006B362` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/projection.rs` | `projection_marks_stale_peers_and_creates_warning_timeline` | `not_ignored` |
| `DISC_C52577EFC3476E086205` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/projection.rs` | `projection_marks_unseen_unreachable_peer_offline` | `not_ignored` |
| `DISC_9066050A3ECFD3606627` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/projection.rs` | `runtime_cloud_projection_contract_reports_topology_edges_and_warnings` | `not_ignored` |
| `DISC_3FD9DCA298624C97001E` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/rollout_policy.rs` | `runtime_cloud_rollout_applying_timeout_transitions_to_failed` | `not_ignored` |
| `DISC_7ECDDD8AA0645E3A1B9F` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/routing.rs` | `cfg_apply_link_transport_protected_key_requires_admin_role` | `not_ignored` |
| `DISC_79634BF32F96637A99EF` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/routing.rs` | `cfg_apply_root_level_protected_key_requires_admin_role` | `not_ignored` |
| `DISC_0A04A77D33D20737366A` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/routing.rs` | `map_action_cfg_apply_emits_config_set_control_request` | `not_ignored` |
| `DISC_918F868742DF0030CC3E` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/routing.rs` | `map_action_status_read_emits_status_control_request` | `not_ignored` |
| `DISC_156940CFF0E479F4765D` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/routing.rs` | `preflight_applies_acl_denial_for_cfg_write` | `not_ignored` |
| `DISC_AB884D4523212262D0A8` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/routing.rs` | `preflight_connected_via_mismatch_has_deterministic_precedence` | `not_ignored` |
| `DISC_2DA9BE4DA13C0958AB94` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/routing.rs` | `preflight_denies_unreachable_target_with_reason_code` | `not_ignored` |
| `DISC_49D6842FE830D483460D` | `rust_unit_test` | `crates/trust-runtime/src/runtime_cloud/routing.rs` | `runtime_cloud_api_payload_fuzz_smoke_budget` | `not_ignored` |
| `DISC_CBA94D5DCAD0113FFD98` | `rust_unit_test` | `crates/trust-runtime/src/scheduler/clock.rs` | `manual_clock_recovers_from_poisoned_lock` | `not_ignored` |
| `DISC_88A430D6A4852CF7B968` | `rust_unit_test` | `crates/trust-runtime/src/scheduler/runner_loop.rs` | `automatic_restart_backoff_is_bounded_and_monotonic` | `not_ignored` |
| `DISC_BD22925B0FA814DDB2F3` | `rust_unit_test` | `crates/trust-runtime/src/scheduler/runner_loop.rs` | `automatic_restart_limiter_escalates_after_retry_budget` | `not_ignored` |
| `DISC_34CD5052A10F2588D30B` | `rust_unit_test` | `crates/trust-runtime/src/scheduler/runner_loop.rs` | `cycle_deadline_from_zero_timeout_is_immediately_expired` | `not_ignored` |
| `DISC_BF01EBAA4B0DFA45370D` | `rust_unit_test` | `crates/trust-runtime/src/scheduler/runner_loop.rs` | `cycle_deadline_guard_restores_after_runtime_error_and_contained_panic` | `not_ignored` |
| `DISC_4F5DDFB0E5B6F9E7F4D4` | `rust_unit_test` | `crates/trust-runtime/src/scheduler/runner_loop.rs` | `cycle_deadline_guard_restores_previous_deadlines_after_success` | `not_ignored` |
| `DISC_E72EFAA018DAB54FAAC0` | `rust_unit_test` | `crates/trust-runtime/src/scheduler/runner_loop.rs` | `disabled_watchdog_does_not_arm_cycle_deadlines` | `not_ignored` |
| `DISC_FB2DC1C843F8F36273CB` | `rust_unit_test` | `crates/trust-runtime/src/scheduler/runner_loop.rs` | `scheduler_arms_execution_deadline_during_cycle` | `not_ignored` |
| `DISC_5573E9D6196C5D5E2DD2` | `rust_unit_test` | `crates/trust-runtime/src/scheduler/runner_loop.rs` | `state_and_error_helpers_recover_poisoned_mutexes` | `not_ignored` |
| `DISC_D18EE90064D1E7FF0AF2` | `rust_unit_test` | `crates/trust-runtime/src/scheduler/runner_loop.rs` | `update_watchdog_zero_timeout_is_normalized_before_use` | `not_ignored` |
| `DISC_7CB21BE5022F12CC473F` | `rust_unit_test` | `crates/trust-runtime/src/scheduler/tests.rs` | `scaled_clock_clamps_zero_to_one` | `not_ignored` |
| `DISC_2418E946F38C19B5F5A8` | `rust_unit_test` | `crates/trust-runtime/src/scheduler/tests.rs` | `scaled_clock_now_is_monotonic` | `not_ignored` |
| `DISC_257E44188295A196A2E5` | `rust_unit_test` | `crates/trust-runtime/src/scheduler/tests.rs` | `scaled_clock_sleeps_faster_than_std_clock` | `not_ignored` |
| `DISC_3FAA65B02EFFD2CCBFC1` | `rust_unit_test` | `crates/trust-runtime/src/value/display.rs` | `format_user_value_hides_rust_value_debug_names` | `not_ignored` |
| `DISC_C59152675AB1C2D5DD96` | `rust_unit_test` | `crates/trust-runtime/src/value/reference.rs` | `array_offset_handles_extreme_bounds_without_overflow` | `not_ignored` |
| `DISC_A79C89EAF1D56A37DADD` | `rust_unit_test` | `crates/trust-runtime/src/value/reference.rs` | `checked_array_offset_preserves_bounds_error` | `not_ignored` |
| `DISC_237287639D3028A4B38A` | `rust_unit_test` | `crates/trust-runtime/src/value/reference.rs` | `common_ref_path_helpers_preserve_segment_order` | `not_ignored` |
| `DISC_AA8907A7D140CFCE1677` | `rust_unit_test` | `crates/trust-runtime/src/web/ads_routes.rs` | `route_add_channel_is_derived_server_side_and_overwrites_client_claim` | `not_ignored` |
| `DISC_124D4256A1FBFDC950E4` | `rust_unit_test` | `crates/trust-runtime/src/web/ads_routes.rs` | `route_add_with_server_derived_untrusted_channel_is_rejected` | `not_ignored` |
| `DISC_A329F289E8A626A9A4C4` | `rust_unit_test` | `crates/trust-runtime/src/web/ads_routes.rs` | `setup_channel_classification_matches_security_matrix` | `not_ignored` |
| `DISC_D28EB96F3DFC6F6DA6F8` | `rust_unit_test` | `crates/trust-runtime/src/web/deploy/tests.rs` | `apply_deploy_accepts_valid_signature_policy` | `not_ignored` |
| `DISC_BD48F3EE077D732CE042` | `rust_unit_test` | `crates/trust-runtime/src/web/deploy/tests.rs` | `apply_deploy_normalizes_src_prefixed_source_paths` | `not_ignored` |
| `DISC_F92D0AC46AB9677C0663` | `rust_unit_test` | `crates/trust-runtime/src/web/deploy/tests.rs` | `apply_deploy_rejects_invalid_runtime_schema` | `not_ignored` |
| `DISC_C197FED6D009F608FB1D` | `rust_unit_test` | `crates/trust-runtime/src/web/deploy/tests.rs` | `apply_deploy_rejects_tampered_payload_signature` | `not_ignored` |
| `DISC_88B391164A19B06E9C4D` | `rust_unit_test` | `crates/trust-runtime/src/web/deploy/tests.rs` | `apply_deploy_rejects_unknown_or_expired_signing_keys` | `not_ignored` |
| `DISC_5A32B19C64E526E15E67` | `rust_unit_test` | `crates/trust-runtime/src/web/deploy/tests.rs` | `apply_deploy_writes_files` | `not_ignored` |
| `DISC_1869347ABBD613D20201` | `rust_unit_test` | `crates/trust-runtime/src/web/deploy/tests.rs` | `sanitize_accepts_nested` | `not_ignored` |
| `DISC_9324CF8857C479A25FA2` | `rust_unit_test` | `crates/trust-runtime/src/web/deploy/tests.rs` | `sanitize_rejects_parent` | `not_ignored` |
| `DISC_4E4E7A58760F6CB9F2A0` | `rust_unit_test` | `crates/trust-runtime/src/web/deploy/tests.rs` | `signature_errors_do_not_echo_key_secrets` | `not_ignored` |
| `DISC_BCEC0C5226A82B9337BC` | `rust_unit_test` | `crates/trust-runtime/src/web/hmi_ws.rs` | `hmi_control_error_payload_is_structured` | `not_ignored` |
| `DISC_876E6034839392A6D5EA` | `rust_unit_test` | `crates/trust-runtime/src/web/ide/tests.rs` | `auth_and_session_lifecycle_contract` | `not_ignored` |
| `DISC_9AD707B19160A7FD2BF1` | `rust_unit_test` | `crates/trust-runtime/src/web/ide/tests.rs` | `collaborative_conflict_detected_with_expected_version` | `not_ignored` |
| `DISC_43401F86E3F67EFE45F3` | `rust_unit_test` | `crates/trust-runtime/src/web/ide/tests.rs` | `diagnostics_hover_and_completion_contracts_are_exposed` | `not_ignored` |
| `DISC_B5C3A44385BA4AA415A2` | `rust_unit_test` | `crates/trust-runtime/src/web/ide/tests.rs` | `format_source_endpoint_returns_formatted_content_without_write` | `not_ignored` |
| `DISC_ACA2536C60E9E890B1D5` | `rust_unit_test` | `crates/trust-runtime/src/web/ide/tests.rs` | `format_structured_text_document_indents_common_blocks` | `not_ignored` |
| `DISC_C904CFCD52B5FA9A470E` | `rust_unit_test` | `crates/trust-runtime/src/web/ide/tests.rs` | `frontend_telemetry_is_aggregated_in_health_snapshot` | `not_ignored` |
| `DISC_9986D0D71E4258F226A6` | `rust_unit_test` | `crates/trust-runtime/src/web/ide/tests.rs` | `fs_audit_log_tracks_mutating_operations` | `not_ignored` |
| `DISC_217AE14D247156974889` | `rust_unit_test` | `crates/trust-runtime/src/web/ide/tests.rs` | `health_snapshot_reports_active_state` | `not_ignored` |
| `DISC_F7E0FC1F8C9E909B567D` | `rust_unit_test` | `crates/trust-runtime/src/web/ide/tests.rs` | `latency_and_resource_budgets_are_enforced` | `not_ignored` |
| `DISC_EAA907AC28C8DEE7526D` | `rust_unit_test` | `crates/trust-runtime/src/web/ide/tests.rs` | `project_selection_and_switch_flow_updates_active_root` | `not_ignored` |
| `DISC_C56D550A193E29EE59C4` | `rust_unit_test` | `crates/trust-runtime/src/web/ide/tests.rs` | `rooted_workspace_paths_are_rejected_as_absolute` | `not_ignored` |
| `DISC_EB70C1C0D6A55614E33C` | `rust_unit_test` | `crates/trust-runtime/src/web/ide/tests.rs` | `session_activity_renews_ttl_and_idle_expiry_still_applies` | `not_ignored` |
| `DISC_12FB1FA9B1886B8200E1` | `rust_unit_test` | `crates/trust-runtime/src/web/ide/tests.rs` | `session_limit_evicts_oldest_inactive_session` | `not_ignored` |
| `DISC_E64C2B4C2ABA69A03C33` | `rust_unit_test` | `crates/trust-runtime/src/web/ide/tests.rs` | `workspace_search_respects_include_and_exclude_globs` | `not_ignored` |
| `DISC_EE0085F578726D8C4DC5` | `rust_unit_test` | `crates/trust-runtime/src/web/ide_tasks.rs` | `build_task_command_matches_cli_project_only_contract` | `not_ignored` |
| `DISC_BBB813AD4C37C729BCEE` | `rust_unit_test` | `crates/trust-runtime/src/web/ide_tasks.rs` | `parse_task_location_line_extracts_st_coordinates` | `not_ignored` |
| `DISC_2C70074B55CD6EF2694C` | `rust_unit_test` | `crates/trust-runtime/src/web/ide_tasks.rs` | `parse_task_locations_deduplicates_repeated_hits` | `not_ignored` |
| `DISC_47496872EE8C6B06A2D9` | `rust_unit_test` | `crates/trust-runtime/src/web/request_dispatch.rs` | `body_lane_classification_is_conservative` | `not_ignored` |
| `DISC_873FF335D3EDBB6B16B1` | `rust_unit_test` | `crates/trust-runtime/src/web/request_dispatch.rs` | `body_permits_are_bounded_and_reusable` | `not_ignored` |
| `DISC_B8BD2473FCB0E4078F9E` | `rust_unit_test` | `crates/trust-runtime/src/web/runtime_cloud_dispatch.rs` | `runtime_cloud_select_preferred_peer_address_prefers_ipv4_loopback` | `not_ignored` |
| `DISC_22FA3EE114DF0F0ED96E` | `rust_unit_test` | `crates/trust-runtime/src/web/runtime_cloud_dispatch.rs` | `runtime_cloud_select_preferred_peer_address_prefers_non_link_local_ipv6` | `not_ignored` |
| `DISC_391F410D2DA36AABCC29` | `rust_unit_test` | `crates/trust-runtime/src/web/runtime_cloud_dispatch.rs` | `runtime_cloud_target_status_revalidates_stale_peer_with_live_socket` | `not_ignored` |
| `DISC_EAA9662D22E61CD56999` | `rust_unit_test` | `crates/trust-runtime/src/web/runtime_cloud_dispatch.rs` | `runtime_cloud_url_host_wraps_ipv6` | `not_ignored` |
| `DISC_72E65032998D488D9425` | `rust_unit_test` | `crates/trust-runtime/src/web/runtime_cloud_state/config.rs` | `runtime_cloud_corrupt_config_state_does_not_reset_to_default` | `not_ignored` |
| `DISC_8F2DF7D8F1922F6075B1` | `rust_unit_test` | `crates/trust-runtime/src/web/runtime_cloud_state/links.rs` | `apply_preferences_adds_t0_overlay_edge_without_removing_mesh` | `not_ignored` |
| `DISC_546D15549466883007E9` | `rust_unit_test` | `crates/trust-runtime/src/web/runtime_cloud_state/links.rs` | `apply_preferences_overrides_channel_for_extended_transports` | `not_ignored` |
| `DISC_BCAC21D8C597BEDB10BE` | `rust_unit_test` | `crates/trust-runtime/src/web/runtime_cloud_state/links.rs` | `compute_host_groups_deterministic_ordering` | `not_ignored` |
| `DISC_7F12BD1BAE19E058FD8F` | `rust_unit_test` | `crates/trust-runtime/src/web/runtime_cloud_state/links.rs` | `compute_host_groups_empty_discovery` | `not_ignored` |
| `DISC_63D4274275297A9EA780` | `rust_unit_test` | `crates/trust-runtime/src/web/runtime_cloud_state/links.rs` | `compute_host_groups_three_mixed` | `not_ignored` |
| `DISC_2122A72BE22BE930E2F2` | `rust_unit_test` | `crates/trust-runtime/src/web/runtime_cloud_state/links.rs` | `compute_host_groups_two_same_host` | `not_ignored` |
| `DISC_63EBEFD812F5AB3FC972` | `rust_unit_test` | `crates/trust-runtime/src/web/runtime_cloud_state/links.rs` | `link_transport_preference_roundtrips_from_state` | `not_ignored` |
| `DISC_8025CD944623F1E140BA` | `rust_unit_test` | `crates/trust-runtime/src/web/runtime_cloud_state/links.rs` | `same_host_check_prefers_host_group_when_present` | `not_ignored` |
| `DISC_7611BD2E3D0A2A6183E0` | `rust_unit_test` | `crates/trust-runtime/src/web/runtime_cloud_state/links.rs` | `same_host_check_uses_discovery_address_overlap` | `not_ignored` |
| `DISC_438AB1014422D2463FB6` | `rust_unit_test` | `crates/trust-runtime/src/web/runtime_cloud_state/links.rs` | `seed_link_transport_preferences_applies_and_removes_toml_actor_entries` | `not_ignored` |
| `DISC_DE56078510150DD0132E` | `rust_unit_test` | `crates/trust-runtime/src/web/runtime_cloud_state/links.rs` | `topology_feature_flags_dev_all_enabled` | `not_ignored` |
| `DISC_350FBA48B87CF570391C` | `rust_unit_test` | `crates/trust-runtime/src/web/runtime_cloud_state/links.rs` | `topology_feature_flags_plant_only_host_containers` | `not_ignored` |
| `DISC_78AB14705E214669153E` | `rust_unit_test` | `crates/trust-syntax/src/lexer/mod.rs` | `test_full_function_block` | `not_ignored` |
| `DISC_A8F478A6F211F9C66E48` | `rust_unit_test` | `crates/trust-syntax/src/lexer/mod.rs` | `test_lex_with_text` | `not_ignored` |
| `DISC_203E3C6D49CF492D3D2A` | `rust_unit_test` | `crates/trust-syntax/src/lexer/mod.rs` | `test_lexer_basic` | `not_ignored` |
| `DISC_6ED0A8EFB4331B322B7F` | `rust_unit_test` | `crates/trust-syntax/src/lexer/mod.rs` | `test_lexer_preserves_positions` | `not_ignored` |
| `DISC_00F2B1BC34C7C2ACDA25` | `rust_unit_test` | `crates/trust-syntax/src/lexer/tokens/tests_part_01.rs` | `test_additional_keywords` | `not_ignored` |
| `DISC_D82B295C4A65C3BCAD41` | `rust_unit_test` | `crates/trust-syntax/src/lexer/tokens/tests_part_01.rs` | `test_basic_operators` | `not_ignored` |
| `DISC_0B978170E12C27335ABD` | `rust_unit_test` | `crates/trust-syntax/src/lexer/tokens/tests_part_01.rs` | `test_comments` | `not_ignored` |
| `DISC_BBA944E431B83031025C` | `rust_unit_test` | `crates/trust-syntax/src/lexer/tokens/tests_part_01.rs` | `test_direct_addresses` | `not_ignored` |
| `DISC_428BC41F4244BD519279` | `rust_unit_test` | `crates/trust-syntax/src/lexer/tokens/tests_part_01.rs` | `test_function_block_keywords` | `not_ignored` |
| `DISC_BE8F0C03B8CC8E1BA276` | `rust_unit_test` | `crates/trust-syntax/src/lexer/tokens/tests_part_01.rs` | `test_hash_prefixed_identifier_tokens` | `not_ignored` |
| `DISC_79C1719748D61267FE90` | `rust_unit_test` | `crates/trust-syntax/src/lexer/tokens/tests_part_01.rs` | `test_integer_literals` | `not_ignored` |
| `DISC_C890AB43F85463C870EC` | `rust_unit_test` | `crates/trust-syntax/src/lexer/tokens/tests_part_01.rs` | `test_invalid_string_escapes` | `not_ignored` |
| `DISC_4864D3A83B25E2F6F9C9` | `rust_unit_test` | `crates/trust-syntax/src/lexer/tokens/tests_part_01.rs` | `test_keywords_case_insensitive` | `not_ignored` |
| `DISC_F95E04B5EBBA40FEFF64` | `rust_unit_test` | `crates/trust-syntax/src/lexer/tokens/tests_part_01.rs` | `test_real_literals` | `not_ignored` |
| `DISC_5240E6D16BA95593A301` | `rust_unit_test` | `crates/trust-syntax/src/lexer/tokens/tests_part_01.rs` | `test_string_escapes` | `not_ignored` |
| `DISC_DA13E9944A5FE3461161` | `rust_unit_test` | `crates/trust-syntax/src/lexer/tokens/tests_part_01.rs` | `test_strings` | `not_ignored` |
| `DISC_6C99393FB3439F625208` | `rust_unit_test` | `crates/trust-syntax/src/lexer/tokens/tests_part_01.rs` | `test_time_literals` | `not_ignored` |
| `DISC_801DBE5D2DF24B68A7D6` | `rust_unit_test` | `crates/trust-syntax/src/lexer/tokens/tests_part_02.rs` | `test_class_and_configuration_keywords` | `not_ignored` |
| `DISC_559831E7FED8B46A6B78` | `rust_unit_test` | `crates/trust-syntax/src/lexer/tokens/tests_part_02.rs` | `test_pragma_content_preserved` | `not_ignored` |
| `DISC_2AF2E848622EF1F5885B` | `rust_unit_test` | `crates/trust-syntax/src/lexer/tokens/tests_part_02.rs` | `test_pragma_with_code` | `not_ignored` |
| `DISC_3BED96886BD89FD6AA6D` | `rust_unit_test` | `crates/trust-syntax/src/lexer/tokens/tests_part_02.rs` | `test_pragmas` | `not_ignored` |
| `DISC_BAFF3FACA3A10D2B10F1` | `rust_unit_test` | `crates/trust-syntax/src/lexer/tokens/tests_part_02.rs` | `test_test_pou_keywords` | `not_ignored` |
| `DISC_BA158A230C7F9B64E008` | `rust_unit_test` | `crates/trust-syntax/src/lexer/tokens/tests_part_02.rs` | `test_var_keywords` | `not_ignored` |
| `DISC_B29671FE018210E9C48E` | `rust_unit_test` | `crates/trust-syntax/src/parser/parser.rs` | `test_bounded_recovery_closes_after_nested_bracket_is_balanced` | `not_ignored` |
| `DISC_24926AF7B506A67CE190` | `rust_unit_test` | `crates/trust-syntax/src/parser/parser.rs` | `test_bounded_recovery_closes_after_nested_paren_is_balanced` | `not_ignored` |
| `DISC_3C5D1667AD9E1B86A142` | `rust_unit_test` | `crates/trust-syntax/src/parser/parser.rs` | `test_bounded_recovery_does_not_close_on_rparen_inside_unclosed_bracket` | `not_ignored` |
| `DISC_5F59509A6C430C5EE9F8` | `rust_unit_test` | `crates/trust-syntax/src/parser/parser.rs` | `test_bounded_top_level_scan_ignores_commas_inside_brackets` | `not_ignored` |
| `DISC_7A7978359CD7CD06FA6F` | `rust_unit_test` | `crates/trust-syntax/src/parser/parser.rs` | `test_bounded_top_level_scan_stops_at_boundary_inside_unclosed_bracket` | `not_ignored` |
| `DISC_F1888E05720F069BDF08` | `rust_unit_test` | `crates/trust-syntax/src/parser/parser.rs` | `test_missing_end_case_recovery` | `not_ignored` |
| `DISC_D32DA38076E458C4708A` | `rust_unit_test` | `crates/trust-syntax/src/parser/parser.rs` | `test_missing_semicolon_insertion` | `not_ignored` |
| `DISC_06CDEF4D0691179B3E41` | `rust_unit_test` | `crates/trust-syntax/src/parser/parser.rs` | `test_parse_call_statement` | `not_ignored` |
| `DISC_8AE7A5B056EE2E7F70DA` | `rust_unit_test` | `crates/trust-syntax/src/parser/parser.rs` | `test_parse_case_enum_labels` | `not_ignored` |
| `DISC_3F7FEE1079FC4DAA4C99` | `rust_unit_test` | `crates/trust-syntax/src/parser/parser.rs` | `test_parse_empty` | `not_ignored` |
| `DISC_D2FC7E65D38845C6F9CA` | `rust_unit_test` | `crates/trust-syntax/src/parser/parser.rs` | `test_parse_function_block` | `not_ignored` |
| `DISC_2EB16A9C2DFD62339DD6` | `rust_unit_test` | `crates/trust-syntax/src/parser/parser.rs` | `test_parse_simple_program` | `not_ignored` |
| `DISC_A0EC931ED2C6B4EB9E70` | `rust_unit_test` | `crates/trust-syntax/src/parser/parser.rs` | `test_parse_typed_literal_and_deref` | `not_ignored` |
| `DISC_983EFB233B4BC682D499` | `rust_unit_test` | `crates/trust-syntax/src/syntax/mod.rs` | `test_initializer_classifier_sets` | `not_ignored` |
| `DISC_9F02D3D1E974062EFE3F` | `rust_unit_test` | `crates/trust-syntax/src/syntax/mod.rs` | `test_is_token_vs_node` | `not_ignored` |
| `DISC_80FBBC2C7BCEFE097E26` | `rust_unit_test` | `crates/trust-syntax/src/syntax/mod.rs` | `test_is_trivia` | `not_ignored` |
| `DISC_1F0F8F8014CF763080CA` | `rust_unit_test` | `crates/trust-syntax/src/syntax/mod.rs` | `test_pou_declaration_classifier_set` | `not_ignored` |
| `DISC_DB2158CD7AE4815BB669` | `rust_unit_test` | `crates/trust-syntax/src/syntax/mod.rs` | `test_token_kind_to_syntax_kind` | `not_ignored` |
| `DISC_FA5CEF011E792047CDD9` | `rust_unit_test` | `crates/trust-wasm-analysis/src/lib/lib_part_05.rs` | `line_character_offset_roundtrip_ascii` | `not_ignored` |
| `DISC_4D3B92E9D49D06F3333B` | `rust_unit_test` | `crates/trust-wasm-analysis/src/lib/lib_part_05.rs` | `line_character_offset_roundtrip_utf16` | `not_ignored` |
| `DISC_CDAEFC4CD0CA84F72A78` | `rust_unit_test` | `crates/trust-wasm-analysis/src/lib/lib_part_05.rs` | `position_to_offset_clamps_inside_utf16_surrogate_pair` | `not_ignored` |
| `DISC_BF353CF9E6FDD42E514D` | `rust_unit_test` | `crates/trust-wasm-analysis/src/lib/lib_part_06.rs` | `canonical_ast_similarity_clears_high_threshold_for_structural_equivalence` | `not_ignored` |
| `DISC_51DF2D1159582D47FC75` | `rust_unit_test` | `crates/trust-wasm-analysis/src/lib/lib_part_06.rs` | `canonical_ast_similarity_drops_below_contamination_threshold_for_structural_change` | `not_ignored` |
| `DISC_AA77C85739530FB74348` | `rust_unit_test` | `crates/trust-wasm-analysis/src/lib/lib_part_06.rs` | `canonical_ast_strips_comments_and_identifier_values` | `not_ignored` |
| `DISC_B88734C266501D2AF32C` | `rust_unit_test` | `crates/verification-cases/src/case_trace.rs` | `finite_toml_float_is_rejected_before_trace_digesting` | `not_ignored` |
| `DISC_A6A172528FBDB9B2EF6F` | `rust_unit_test` | `crates/verification-cases/src/case_trace.rs` | `generated_v2_provenance_is_accepted` | `not_ignored` |
| `DISC_DE0884DB9274146F41BB` | `rust_unit_test` | `crates/verification-cases/src/case_trace.rs` | `hand_authored_trace_provenance_is_artifact_ready` | `not_ignored` |
| `DISC_31C7CA5475CFABF17E4A` | `rust_unit_test` | `crates/verification-cases/src/case_trace.rs` | `unicode_trace_digest_matches_metadata_validator_contract` | `not_ignored` |
| `DISC_87302EF69E1E2E42E300` | `rust_unit_test` | `crates/verification-cases/src/lib.rs` | `blocked_cases_are_recorded_without_executing_the_runner` | `not_ignored` |
| `DISC_1A7BDF97311D84EE779B` | `rust_unit_test` | `crates/verification-cases/src/lib.rs` | `default_artifact_dir_is_workspace_target_gate_artifacts` | `not_ignored` |
| `DISC_9F1F50D5688D15E30213` | `rust_unit_test` | `crates/verification-cases/src/lib.rs` | `expected_case_file_digest_is_enforced_before_execution` | `not_ignored` |
| `DISC_0ADA17CB60051E6BC46E` | `rust_unit_test` | `crates/verification-cases/src/lib.rs` | `mismatched_trust_verify_artifact_dir_fails_before_execution` | `not_ignored` |
| `DISC_1303DC4F31325352DB04` | `rust_unit_test` | `crates/verification-cases/src/lib.rs` | `mismatched_trust_verify_case_file_digest_fails_before_execution` | `not_ignored` |
| `DISC_8D208B055237565CCF60` | `rust_unit_test` | `crates/verification-cases/src/lib.rs` | `mismatched_trust_verify_test_id_fails_before_execution` | `not_ignored` |
| `DISC_042D4F35C060424831AE` | `rust_unit_test` | `crates/verification-cases/src/lib.rs` | `partial_trust_verify_env_stamps_fail_before_execution` | `not_ignored` |
| `DISC_3418DE949E799427C04E` | `rust_unit_test` | `crates/verification-cases/src/lib.rs` | `runnable_cases_capture_snapshots_and_state_delta` | `not_ignored` |
| `DISC_D097DB1E2FB45715FC5E` | `rust_unit_test` | `crates/verification-cases/src/lib.rs` | `schema_version_mismatch_is_rejected_before_execution` | `not_ignored` |
| `DISC_247AAB98DD35F057E1EA` | `rust_unit_test` | `crates/verification-cases/src/lib.rs` | `stamped_artifact_dir_overrides_compile_time_workspace_path` | `not_ignored` |
| `DISC_FD838D2673518C6E333A` | `rust_unit_test` | `crates/verification-cases/src/lib.rs` | `trust_verify_env_stamps_are_recorded_in_artifact` | `not_ignored` |
| `DISC_C932AA981C465EDBACC6` | `rust_unit_test` | `crates/verification-cases/src/lib.rs` | `workspace_absolute_case_path_is_recorded_as_committed_relative_identity` | `not_ignored` |
| `DISC_C19E83C321C481A8A26B` | `rust_unit_test` | `crates/verification-cases/src/model.rs` | `case_file_rejects_unknown_root_and_case_fields` | `not_ignored` |
| `DISC_BF51D273F54F7F04E17B` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/ci/broken/src/tests.st` | `CI_Fails` | `not_ignored` |
| `DISC_5D5012DB3A3EBBFDB859` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/ci/green/src/tests.st` | `CI_AlsoPasses` | `not_ignored` |
| `DISC_72893A98D712EA02D73B` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/ci/green/src/tests.st` | `CI_Passes` | `not_ignored` |
| `DISC_89B842645CCF7208661E` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/03_data_types/tests.st` | `oscat_data_types_are_available` | `not_ignored` |
| `DISC_3386E983A74D9FD280CC` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/04_other_functions/tests.st` | `oscat_constants_resolve_and_match_ported_values` | `not_ignored` |
| `DISC_98D98FF7A1AC0CC491FB` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/04_other_functions/tests.st` | `oscat_esr_monitors_and_collector_behave` | `not_ignored` |
| `DISC_86DA0C1ABFB308AFF6CF` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/04_other_functions/tests.st` | `oscat_fb_omitted_input_defaults_follow_iec_state_rules` | `not_ignored` |
| `DISC_9D8BBC05BDC211D9FD58` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/04_other_functions/tests.st` | `oscat_status_to_esr_and_version_behave` | `not_ignored` |
| `DISC_1AF72AB45B64F13D0ED9` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/05_mathematics/tests.st` | `oscat_advanced_math_helpers_behave` | `not_ignored` |
| `DISC_06CD4BDA9BB5C320D966` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/05_mathematics/tests.st` | `oscat_extended_math_helpers_behave` | `not_ignored` |
| `DISC_2732B9ABAF2640F1FC10` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/05_mathematics/tests.st` | `oscat_math_helpers_behave` | `not_ignored` |
| `DISC_AE56520CB05D8FC48ADF` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/05_mathematics/tests.st` | `oscat_probability_and_sequence_math_helpers_behave` | `not_ignored` |
| `DISC_51B7C8F8C51246903991` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/05_mathematics/tests.st` | `oscat_random_math_helpers_behave` | `not_ignored` |
| `DISC_9920F3AF6490E2A85EFA` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/06_arrays/tests.st` | `oscat_array_mutation_helpers_behave` | `not_ignored` |
| `DISC_915E6425BF889A9A806C` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/06_arrays/tests.st` | `oscat_array_ordering_helpers_behave` | `not_ignored` |
| `DISC_EB742EDB5CACBC8A1948` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/06_arrays/tests.st` | `oscat_array_statistics_helpers_behave` | `not_ignored` |
| `DISC_EE40F3E02D42B3E741AA` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/07_complex_mathematics/tests.st` | `oscat_complex_exponential_logarithmic_and_power_helpers_behave` | `not_ignored` |
| `DISC_587C054C1B4FEFCC83B7` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/07_complex_mathematics/tests.st` | `oscat_complex_inverse_helpers_behave` | `not_ignored` |
| `DISC_AAE5A205998F28D91AE2` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/07_complex_mathematics/tests.st` | `oscat_complex_number_construction_and_algebra_behave` | `not_ignored` |
| `DISC_CFCF22520581A6F4A8EF` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/07_complex_mathematics/tests.st` | `oscat_complex_trigonometric_and_hyperbolic_helpers_behave` | `not_ignored` |
| `DISC_7CCF951963FB6C53D3D2` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/08_arithmetics_with_double_precision/tests.st` | `oscat_math_real2_helpers_behave` | `not_ignored` |
| `DISC_652895D50E7EE553F0B2` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/09_arithmetic_functions/tests.st` | `oscat_math_frmp_b_behaves` | `not_ignored` |
| `DISC_03E47CE77AAB4AB24E0B` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/09_arithmetic_functions/tests.st` | `oscat_math_ft_avg_behaves` | `not_ignored` |
| `DISC_ED6830F3C72524889834` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/09_arithmetic_functions/tests.st` | `oscat_math_ft_min_max_and_ramp_helpers_behave` | `not_ignored` |
| `DISC_49C39FB64E8AADADD597` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/09_arithmetic_functions/tests.st` | `oscat_math_interpolation_helpers_behave` | `not_ignored` |
| `DISC_6E6ACEE655B28790C02B` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/09_arithmetic_functions/tests.st` | `oscat_math_linear_and_polynomial_helpers_behave` | `not_ignored` |
| `DISC_3102C87D326C39AEA03C` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/10_geometric_functions/tests.st` | `oscat_math_geometry_helpers_behave` | `not_ignored` |
| `DISC_98A6B84C9618B3D0FEBC` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/11_vector_mathematics/tests.st` | `oscat_vector_algebra_helpers_behave` | `not_ignored` |
| `DISC_1D55A2F971631D473A2A` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/11_vector_mathematics/tests.st` | `oscat_vector_angle_helpers_behave` | `not_ignored` |
| `DISC_E34A7AD0DB8A62C966EC` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/12_time_and_date/tests.st` | `oscat_calendar_event_and_clock_helpers_behave` | `not_ignored` |
| `DISC_B9393EFC507BCC358B5B` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/12_time_and_date/tests.st` | `oscat_date_add_and_work_week_behave` | `not_ignored` |
| `DISC_5F017397CE86DBB8ECE9` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/12_time_and_date/tests.st` | `oscat_date_boundary_helpers_behave` | `not_ignored` |
| `DISC_D46072007E8F2295AC39` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/12_time_and_date/tests.st` | `oscat_date_component_helpers_behave` | `not_ignored` |
| `DISC_64F510FB9FFA14884DD8` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/12_time_and_date/tests.st` | `oscat_days_delta_behaves` | `not_ignored` |
| `DISC_5E75368C0210135F9F63` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/12_time_and_date/tests.st` | `oscat_days_in_month_and_year_behave` | `not_ignored` |
| `DISC_8661109114572A488F20` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/12_time_and_date/tests.st` | `oscat_dt_component_helpers_behave` | `not_ignored` |
| `DISC_78608A627C82E835A4D6` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/12_time_and_date/tests.st` | `oscat_easter_behaves` | `not_ignored` |
| `DISC_3123309B644B4C79FE38` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/12_time_and_date/tests.st` | `oscat_structured_date_time_helpers_behave` | `not_ignored` |
| `DISC_EC75EC650D795904BDC2` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/12_time_and_date/tests.st` | `oscat_sun_and_julian_helpers_behave` | `not_ignored` |
| `DISC_72E805067EF6B6701330` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/12_time_and_date/tests.st` | `oscat_time_and_date_weekday_and_timecheck_behave` | `not_ignored` |
| `DISC_52ACFEC76E3C12567789` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/12_time_and_date/tests.st` | `oscat_time_conversion_helpers_behave` | `not_ignored` |
| `DISC_553AE4932950B3D53A00` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/12_time_and_date/tests.st` | `oscat_time_multiplication_helper_behaves` | `not_ignored` |
| `DISC_EC9721245BDAF218ADC9` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/12_time_and_date/tests.st` | `oscat_time_zone_period_and_dst_helpers_behave` | `not_ignored` |
| `DISC_C500D12A19DEBBF19F9A` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/12_time_and_date/tests.st` | `oscat_tod_and_dt_constructor_helpers_behave` | `not_ignored` |
| `DISC_B176B0AD01FFF79881E7` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/13_string_functions/tests.st` | `oscat_dt_to_strf_behaves` | `not_ignored` |
| `DISC_DFCEBB33BE9546C8D304` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/13_string_functions/tests.st` | `oscat_month_and_weekday_strings_behave` | `not_ignored` |
| `DISC_5F4A6EDF688993FAF85F` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/13_string_functions/tests.st` | `oscat_nested_field_index_regression_behaves` | `not_ignored` |
| `DISC_70DC6810EE64D989ABAA` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/13_string_functions/tests.st` | `oscat_string_binary_hex_and_octal_decoders_behave` | `not_ignored` |
| `DISC_64385FCC397747102949` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/13_string_functions/tests.st` | `oscat_string_bit_and_hex_formatters_behave` | `not_ignored` |
| `DISC_134F589D3D6BCF252BFE` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/13_string_functions/tests.st` | `oscat_string_case_conversion_helpers_behave` | `not_ignored` |
| `DISC_9916C791EAC0CBF928D0` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/13_string_functions/tests.st` | `oscat_string_case_regression_behaves` | `not_ignored` |
| `DISC_90E0B4A62BFC17EE189C` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/13_string_functions/tests.st` | `oscat_string_classifier_helpers_behave` | `not_ignored` |
| `DISC_B65740F749091460C738` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/13_string_functions/tests.st` | `oscat_string_cleanup_and_count_helpers_behave` | `not_ignored` |
| `DISC_338CF9BDEE7CC6F9182B` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/13_string_functions/tests.st` | `oscat_string_code_delete_and_umlaut_helpers_behave` | `not_ignored` |
| `DISC_99E77D2DADDCE7E4BE83` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/13_string_functions/tests.st` | `oscat_string_comparison_regression_behaves` | `not_ignored` |
| `DISC_635BF552538B01B6DD02` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/13_string_functions/tests.st` | `oscat_string_decimal_decoders_behave` | `not_ignored` |
| `DISC_C86B93AE3082636093CC` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/13_string_functions/tests.st` | `oscat_string_exec_and_message_helpers_behave` | `not_ignored` |
| `DISC_2D14B1282FE999A97D1F` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/13_string_functions/tests.st` | `oscat_string_formatted_value_parsers_behave` | `not_ignored` |
| `DISC_769AA0D4BED8048D5091` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/13_string_functions/tests.st` | `oscat_string_formatting_helpers_behave` | `not_ignored` |
| `DISC_6776E42E95919D111540` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/13_string_functions/tests.st` | `oscat_string_html_name_helpers_behave` | `not_ignored` |
| `DISC_9ACAA0CC5BD0DE3A66E7` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/13_string_functions/tests.st` | `oscat_string_mirror_and_replacement_helpers_behave` | `not_ignored` |
| `DISC_2406B2F7A281CB143BBD` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/13_string_functions/tests.st` | `oscat_string_search_helpers_behave` | `not_ignored` |
| `DISC_3004FB36F9436F6CEAB0` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/13_string_functions/tests.st` | `oscat_string_ticker_fb_scrolls_text` | `not_ignored` |
| `DISC_22D31926F0E98E91D400` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/13_string_functions/tests.st` | `oscat_string_trim_helpers_behave` | `not_ignored` |
| `DISC_FFF73572685349BF943D` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/14_memory_modules/tests.st` | `oscat_logic_memory_helpers_behave` | `not_ignored` |
| `DISC_4BFC8131EFD3BF5632C3` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/15_pulse_generators/tests.st` | `oscat_logic_dead_time_pulse_helper_behaves` | `not_ignored` |
| `DISC_EF81B4D7B9B46A88F845` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/15_pulse_generators/tests.st` | `oscat_logic_generator_click_and_divider_helpers_behave` | `not_ignored` |
| `DISC_9717E82AFE94FEF4CB3C` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/15_pulse_generators/tests.st` | `oscat_logic_generator_pattern_and_cycle_helpers_behave` | `not_ignored` |
| `DISC_4FC7221A2FDE3F8473C3` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/15_pulse_generators/tests.st` | `oscat_logic_generator_triggers_behave` | `not_ignored` |
| `DISC_E7EDF4129A05681EB62F` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/15_pulse_generators/tests.st` | `oscat_logic_programmable_clock_behaves` | `not_ignored` |
| `DISC_75760A63F8ECB5694597` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/15_pulse_generators/tests.st` | `oscat_logic_scheduler_helpers_behave` | `not_ignored` |
| `DISC_D2CC4D8853FC6AE1AE52` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/15_pulse_generators/tests.st` | `oscat_logic_sequence4_helpers_behave` | `not_ignored` |
| `DISC_3CBE50E285837AFBCFC4` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/15_pulse_generators/tests.st` | `oscat_logic_sequence8_helpers_behave` | `not_ignored` |
| `DISC_5DF001C89CC94CE79402` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/16_logic_modules/tests.st` | `oscat_logic_bit_cast_and_reflection_helpers_behave` | `not_ignored` |
| `DISC_E7FC36CB15F5512C8F51` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/16_logic_modules/tests.st` | `oscat_logic_bit_helpers_behave` | `not_ignored` |
| `DISC_AFB097B826FBFDCC85E1` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/16_logic_modules/tests.st` | `oscat_logic_bit_load_helpers_behave` | `not_ignored` |
| `DISC_2A606B376DAFBA03FD30` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/16_logic_modules/tests.st` | `oscat_logic_crc_gen_behaves` | `not_ignored` |
| `DISC_6C033AEE8C131A36F9D2` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/16_logic_modules/tests.st` | `oscat_logic_decoders_and_muxes_behave` | `not_ignored` |
| `DISC_AC45E7C678C8BD5592A2` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/16_logic_modules/tests.st` | `oscat_logic_matrix_helpers_behave` | `not_ignored` |
| `DISC_4C0ACCF0A19D916189F2` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/16_logic_modules/tests.st` | `oscat_logic_pin_code_helpers_behave` | `not_ignored` |
| `DISC_D2169F17693956DF4D5A` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/17_latches_flip_flop_and_shift_register/tests.st` | `oscat_logic_edge_counters_and_toggle_behave` | `not_ignored` |
| `DISC_B6E3C2D7E93B305B85B3` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/17_latches_flip_flop_and_shift_register/tests.st` | `oscat_logic_edge_flip_flops_behave` | `not_ignored` |
| `DISC_970E98891C41ABD2087D` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/17_latches_flip_flop_and_shift_register/tests.st` | `oscat_logic_jk_rs_and_selector_behave` | `not_ignored` |
| `DISC_A746DC1B6F38CC49778A` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/17_latches_flip_flop_and_shift_register/tests.st` | `oscat_logic_pulse_latches_and_store_behave` | `not_ignored` |
| `DISC_C1BC1A3233B71787B6F7` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/17_latches_flip_flop_and_shift_register/tests.st` | `oscat_logic_shift_registers_behave` | `not_ignored` |
| `DISC_64BF8B51BB1FAEB45CA8` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/18_signal_generators/tests.st` | `oscat_signal_generator_misc_helpers_compile_and_stabilize` | `not_ignored` |
| `DISC_0497BD35AC68EFE3B48F` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/18_signal_generators/tests.st` | `oscat_signal_generator_ramp_helpers_behave` | `not_ignored` |
| `DISC_764F9CB2D8B8839E7425` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/18_signal_generators/tests.st` | `oscat_signal_generator_waveform_edge_cases_behave` | `not_ignored` |
| `DISC_71091884B8D112B64C7D` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/19_signal_processing/tests.st` | `oscat_logic_delay_behaves` | `not_ignored` |
| `DISC_86861533494437CC0173` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/19_signal_processing/tests.st` | `oscat_signal_processing_analog_scaling_helpers_behave` | `not_ignored` |
| `DISC_44AB2807D5FA842D216A` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/19_signal_processing/tests.st` | `oscat_signal_processing_delay4_and_fade_behave` | `not_ignored` |
| `DISC_CFF70381AB48A19BEA9A` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/19_signal_processing/tests.st` | `oscat_signal_processing_filter_and_mux_helpers_behave` | `not_ignored` |
| `DISC_67EB600577856213618A` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/19_signal_processing/tests.st` | `oscat_signal_processing_offset_and_scaling_helpers_behave` | `not_ignored` |
| `DISC_462C6D1EB20DEA0D44E7` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/19_signal_processing/tests.st` | `oscat_signal_processing_sample_hold_and_stair_helpers_behave` | `not_ignored` |
| `DISC_C67E97B926BE5BC2E9E4` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/19_signal_processing/tests.st` | `oscat_signal_processing_scale_and_range_helpers_behave` | `not_ignored` |
| `DISC_2E610FBCA211B385F714` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/19_signal_processing/tests.st` | `oscat_signal_processing_trend_helpers_behave` | `not_ignored` |
| `DISC_79B22F1AC5F1BBCA4C85` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/20_sensors/tests.st` | `oscat_sensors_multi_in_modes_behave` | `not_ignored` |
| `DISC_D0B6B8E4636AB143DF47` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/20_sensors/tests.st` | `oscat_sensors_resistance_temperature_roundtrips_behave` | `not_ignored` |
| `DISC_294B9140A122C7243283` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/20_sensors/tests.st` | `oscat_sensors_sensor_int_behaves` | `not_ignored` |
| `DISC_B7F83CAF70321ACCC0D2` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/21_measuring_modules/tests.st` | `oscat_measuring_cycle_time_helpers_behave` | `not_ignored` |
| `DISC_AC5F090497468CEE3D9D` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/21_measuring_modules/tests.st` | `oscat_measuring_modules_ontime_behaves` | `not_ignored` |
| `DISC_96F168A91A49A35095E9` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/21_measuring_modules/tests.st` | `oscat_measuring_runtime_and_meter_helpers_behave` | `not_ignored` |
| `DISC_0351A3A7E8B935E780AB` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/21_measuring_modules/tests.st` | `oscat_measuring_window_and_calibration_helpers_behave` | `not_ignored` |
| `DISC_4C8CEB60F564FD88EBBA` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/21_measuring_modules/tests.st` | `oscat_plc_clock_helpers_behave` | `not_ignored` |
| `DISC_CFE61D70751B8D37FE99` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/22_calculations/tests.st` | `oscat_astro_fb_converts_units` | `not_ignored` |
| `DISC_6A407E0BF6B245A5D16F` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/22_calculations/tests.st` | `oscat_direction_helpers_behave` | `not_ignored` |
| `DISC_05D659785C7CA109DEEA` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/22_calculations/tests.st` | `oscat_energy_fb_converts_units` | `not_ignored` |
| `DISC_8897AD671C7BC2995A40` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/22_calculations/tests.st` | `oscat_geographic_conversion_behaves` | `not_ignored` |
| `DISC_A6664B410D54188E316B` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/22_calculations/tests.st` | `oscat_length_fb_converts_units` | `not_ignored` |
| `DISC_2BCFFCA3640380A03E11` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/22_calculations/tests.st` | `oscat_pressure_fb_converts_units` | `not_ignored` |
| `DISC_880F7B4175F855F96016` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/22_calculations/tests.st` | `oscat_speed_and_frequency_conversions_behave` | `not_ignored` |
| `DISC_E8F89DDAC96E16FD88BF` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/22_calculations/tests.st` | `oscat_speed_fb_converts_units` | `not_ignored` |
| `DISC_686EA3F6BF062EB98A8D` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/22_calculations/tests.st` | `oscat_temperature_conversions_behave` | `not_ignored` |
| `DISC_9BB0859534122126F6A7` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/22_calculations/tests.st` | `oscat_temperature_fb_converts_units` | `not_ignored` |
| `DISC_7562CCA90C067C1A1165` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/23_control_modules/tests.st` | `oscat_control_modules_basic_helpers_behave` | `not_ignored` |
| `DISC_4253FC33DB2F04694596` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/23_control_modules/tests.st` | `oscat_control_modules_building_blocks_behave` | `not_ignored` |
| `DISC_E662D72DC93B7E5C40E4` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/23_control_modules/tests.st` | `oscat_control_modules_control_set_helpers_behave` | `not_ignored` |
| `DISC_2DB381DD130DB9207916` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/23_control_modules/tests.st` | `oscat_control_modules_ctrl_pid_manual_clamp_behaves` | `not_ignored` |
| `DISC_AFF04308B1A2DAB9574B` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/23_control_modules/tests.st` | `oscat_control_modules_environmental_functions_behave` | `not_ignored` |
| `DISC_71EF5D74FF9FF64DA86E` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/23_control_modules/tests.st` | `oscat_control_modules_hyst_2_retains_state_inside_window` | `not_ignored` |
| `DISC_93EED8D0E4FDA9FDC9CA` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/23_control_modules/tests.st` | `oscat_control_modules_hysteresis_helpers_behave` | `not_ignored` |
| `DISC_2CC17FE0A3EADC55C3C2` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/24_device_driver/tests.st` | `oscat_device_driver_interlocks_and_manuals_behave` | `not_ignored` |
| `DISC_DA6DD179555A6FDBCBB7` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/24_device_driver/tests.st` | `oscat_device_driver_parameter_helpers_behave` | `not_ignored` |
| `DISC_4415417E75A8EBAA68C1` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/24_device_driver/tests.st` | `oscat_device_driver_switching_helpers_behave` | `not_ignored` |
| `DISC_4FE0DAF7834A9CB19056` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/24_device_driver/tests.st` | `oscat_device_driver_tuning_helpers_behave` | `not_ignored` |
| `DISC_B9983D5F56E76C0A0C36` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/25_buffer_management/tests.st` | `oscat_buffer_management_copy_helpers_behave` | `not_ignored` |
| `DISC_05DE80652BB0774E97C7` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/25_buffer_management/tests.st` | `oscat_buffer_management_search_helpers_behave` | `not_ignored` |
| `DISC_C3BF234C81C7231FAE89` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/26_list_processing/tests.st` | `oscat_list_processing_iteration_and_retrieval_helpers_behave` | `not_ignored` |
| `DISC_65D1FFE60324D751D5FF` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/core/src/26_list_processing/tests.st` | `oscat_list_processing_mutation_helpers_behave` | `not_ignored` |
| `DISC_14C879F39AC65611F817` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/negative_public_surface/src/tests.st` | `oscat_negative_public_surface` | `not_ignored` |
| `DISC_0B0E907B03A08281CE61` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/oop_core/src/tests.st` | `oscat_oop_calendar_matches_classic_calendar_and_sun_helpers` | `not_ignored` |
| `DISC_1EDE82931FE02A991D24` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/oop_core/src/tests.st` | `oscat_oop_context_loads_constants_and_direction_helpers_match_classic` | `not_ignored` |
| `DISC_BCA9F19FE3AA0BA740A9` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/oop_core/src/tests.st` | `oscat_oop_fifo_and_stack_match_classic_memory_modules` | `not_ignored` |
| `DISC_3E5D3BE099DEEAE5DE97` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/oop_core/src/tests.st` | `oscat_oop_filter_pid_hysteresis_and_pulse_match_classic_scan_behaviour` | `not_ignored` |
| `DISC_32C509013C20697F95C6` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/oop_core/src/tests.st` | `oscat_oop_instances_do_not_share_scan_state` | `not_ignored` |
| `DISC_1F13B40B91FFE0E1F06F` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/oop_core/src/tests.st` | `oscat_oop_reset_validation_and_multiscan_behaviour_are_locked` | `not_ignored` |
| `DISC_0D6ABAAF097F898476FB` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/oop_core/src/tests.st` | `oscat_oop_unit_converter_matches_classic_records_and_scalars` | `not_ignored` |
| `DISC_CD36CAEC06119D5211CA` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/oop_core/src/tests.st` | `oscat_oop_v1_control_and_filter_components_match_classic` | `not_ignored` |
| `DISC_A5B211E2B981AEF3D922` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/oop_core/src/tests.st` | `oscat_oop_v1_generators_memory_and_logic_match_classic` | `not_ignored` |
| `DISC_E116D042CE82EB211139` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/oscat/oop_core/src/tests.st` | `oscat_oop_v1_measuring_calendar_building_and_driver_components_match_classic` | `not_ignored` |
| `DISC_729B7BD80DED32027115` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion/src/tests.st` | `plcopen_motion_coordinated_motion_command_acceptance_is_deterministic` | `not_ignored` |
| `DISC_E3E3F34D9FE55F20095D` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion/src/tests.st` | `plcopen_motion_coordinated_motion_dynamics_and_command_info_fbs` | `not_ignored` |
| `DISC_2A2A2310A2CC0687C2CE` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion/src/tests.st` | `plcopen_motion_coordinated_motion_fake_group_backend_models_membership_and_readback` | `not_ignored` |
| `DISC_735797CA38CF6D4B5AEF` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion/src/tests.st` | `plcopen_motion_coordinated_motion_group_admin_state_and_power_fbs` | `not_ignored` |
| `DISC_4FF8622E56E0FBAD8281` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion/src/tests.st` | `plcopen_motion_coordinated_motion_group_home_stop_halt_wait_and_override_fbs` | `not_ignored` |
| `DISC_C394138BF07961C5E372` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion/src/tests.st` | `plcopen_motion_coordinated_motion_group_membership_admin_fbs` | `not_ignored` |
| `DISC_0833A7ADFC8EB52164ED` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion/src/tests.st` | `plcopen_motion_coordinated_motion_group_parameter_fbs` | `not_ignored` |
| `DISC_8A89F1AF26B18E2E5330` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion/src/tests.st` | `plcopen_motion_coordinated_motion_group_read_motion_state_fbs` | `not_ignored` |
| `DISC_2B25F571548B9C0CD445` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion/src/tests.st` | `plcopen_motion_coordinated_motion_group_readback_position_velocity_acceleration` | `not_ignored` |
| `DISC_042EB497CED14A708CBB` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion/src/tests.st` | `plcopen_motion_coordinated_motion_group_ref_fields_resolve` | `not_ignored` |
| `DISC_9A3738DD6DC56822F730` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion/src/tests.st` | `plcopen_motion_coordinated_motion_group_set_position_and_transform_position` | `not_ignored` |
| `DISC_1ABADAD85474DE193E5C` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion/src/tests.st` | `plcopen_motion_coordinated_motion_group_status_and_error_readback_fbs` | `not_ignored` |
| `DISC_601C807D90F2B9C4C67E` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion/src/tests.st` | `plcopen_motion_coordinated_motion_group_swlimit_fbs` | `not_ignored` |
| `DISC_E70286A29781BDBC3773` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion/src/tests.st` | `plcopen_motion_coordinated_motion_linear_and_direct_motion_fbs` | `not_ignored` |
| `DISC_CD6E6F736BF9521A30E3` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion/src/tests.st` | `plcopen_motion_coordinated_motion_public_types_and_enums_resolve` | `not_ignored` |
| `DISC_37AC1EEFFA206A9811BB` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion/src/tests.st` | `plcopen_motion_coordinated_motion_seed_probe_smoke` | `not_ignored` |
| `DISC_15B0E02CE3BA5DB02758` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion/src/tests.st` | `plcopen_motion_coordinated_motion_transform_admin_and_readback_fbs` | `not_ignored` |
| `DISC_562D4862D3EA69C38343` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion/src/tests.st` | `plcopen_motion_coordinated_motion_transform_round_trip_identity` | `not_ignored` |
| `DISC_28F2434350CB7D85496E` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion/src/tests.st` | `plcopen_motion_coordinated_motion_unsupported_transition_and_legacy_blending_are_rejected` | `not_ignored` |
| `DISC_277B1E3ECB7578728A7B` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion_negative_deferred_public_surface/src/tests.st` | `plcopen_motion_coordinated_motion_deferred_public_surface_is_absent` | `not_ignored` |
| `DISC_EA984C0D9AC3BA4471BA` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/homing/src/tests.st` | `plcopen_motion_homing_direct_absolute_finish_and_generic_home_behaviors` | `not_ignored` |
| `DISC_97AC4842EAC707C6D0A9` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/homing/src/tests.st` | `plcopen_motion_homing_distance_coded_and_limit_errors` | `not_ignored` |
| `DISC_738D769BA80FB78B8554` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/homing/src/tests.st` | `plcopen_motion_homing_multi_step_sequence_finishes_in_work_area` | `not_ignored` |
| `DISC_741FD4E8A9E4647C8426` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/homing/src/tests.st` | `plcopen_motion_homing_public_types_resolve` | `not_ignored` |
| `DISC_0F588D2A8D5AF3B14484` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/homing/src/tests.st` | `plcopen_motion_homing_reference_pulse_behavior` | `not_ignored` |
| `DISC_2267F21B4BE8B5D0311A` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/homing/src/tests.st` | `plcopen_motion_homing_simulated_signals_can_be_seeded` | `not_ignored` |
| `DISC_AE5AE315B1D77340EE5F` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/homing/src/tests.st` | `plcopen_motion_homing_state_probe_helper_smoke` | `not_ignored` |
| `DISC_92FAAFB6123E16125BB8` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/homing/src/tests.st` | `plcopen_motion_homing_step_absolute_and_limit_switch_behaviors` | `not_ignored` |
| `DISC_19A3FE8AFB4002ED6160` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/homing/src/tests.st` | `plcopen_motion_homing_step_block_behavior` | `not_ignored` |
| `DISC_75F849491A6D18C0AE6D` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/homing/src/tests.st` | `plcopen_motion_homing_step_block_detection_time_behavior` | `not_ignored` |
| `DISC_346BB29CCAA752BB63BD` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/homing_negative_deferred_public_surface/src/tests.st` | `plcopen_motion_homing_deferred_public_surface_is_absent` | `not_ignored` |
| `DISC_EC3387E325EFB353311E` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/oop_single_axis/src/tests.st` | `plcopen_motion_oop_axis_assigns_to_interface` | `not_ignored` |
| `DISC_7BE54508C5CB5640161D` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/oop_single_axis/src/tests.st` | `plcopen_motion_oop_axis_interface_and_power_command` | `not_ignored` |
| `DISC_55000F2744C3ACCBFDF8` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/oop_single_axis/src/tests.st` | `plcopen_motion_oop_bind_returns_success` | `not_ignored` |
| `DISC_2DED137F519F2FBD9F2B` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/oop_single_axis/src/tests.st` | `plcopen_motion_oop_command_concrete_properties_dispatch` | `not_ignored` |
| `DISC_8F7E69346A517A2A8F9D` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/oop_single_axis/src/tests.st` | `plcopen_motion_oop_command_interface_properties_dispatch` | `not_ignored` |
| `DISC_FD22EA18970154477FE4` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/oop_single_axis/src/tests.st` | `plcopen_motion_oop_concrete_power_call_does_not_error` | `not_ignored` |
| `DISC_8BE91E1B537B63E8388A` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/oop_single_axis/src/tests.st` | `plcopen_motion_oop_concrete_power_returns_command` | `not_ignored` |
| `DISC_6E2A5FC578987C5B1630` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/oop_single_axis/src/tests.st` | `plcopen_motion_oop_home_setposition_and_reset_use_classic_state` | `not_ignored` |
| `DISC_541CF962933661F9A980` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/oop_single_axis/src/tests.st` | `plcopen_motion_oop_interface_power_returns_command` | `not_ignored` |
| `DISC_2EC0582BB283D36077F9` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/oop_single_axis/src/tests.st` | `plcopen_motion_oop_move_absolute_completes_after_release` | `not_ignored` |
| `DISC_256A0F2FC5E3ADFBEEF6` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/oop_single_axis/src/tests.st` | `plcopen_motion_oop_readback_properties_refresh_from_classic_axis` | `not_ignored` |
| `DISC_C92A8904F7EDBA7A36DF` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/oop_single_axis/src/tests.st` | `plcopen_motion_oop_refresh_after_bind_returns_success` | `not_ignored` |
| `DISC_1DB21BB00B8CD33534B6` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/oop_single_axis/src/tests.st` | `plcopen_motion_oop_unsupported_methods_return_not_supported` | `not_ignored` |
| `DISC_6D1B6DAFAD31E58A8E56` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/oop_single_axis/src/tests.st` | `plcopen_motion_oop_velocity_and_parameter_methods` | `not_ignored` |
| `DISC_DE7941EA23FB8E9BFE1E` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_active_fault_drives_errorstop_and_clears_queue` | `not_ignored` |
| `DISC_3BB5A966082B336822FB` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_actual_value_readbacks_follow_seeded_values` | `not_ignored` |
| `DISC_D72F60AAC882E0BC8FD8` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_administrative_fbs_preserve_state` | `not_ignored` |
| `DISC_0E3779AB9BF08FFFF3D0` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_administrative_readback_fbs_resolve` | `not_ignored` |
| `DISC_19E87247417B85193F74` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_axis_ref_fields_resolve` | `not_ignored` |
| `DISC_EE4638ED07D6E87C55CD` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_axis_status_literals_resolve` | `not_ignored` |
| `DISC_A6B1E44DB2C9E0375D16` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_bool_parameter_roundtrip` | `not_ignored` |
| `DISC_1A8EDE0EA215A7A6E8AE` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_buffer_mode_literals_resolve` | `not_ignored` |
| `DISC_5DF00898127CB8AB178F` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_buffer_mode_support_rules_by_fb_family` | `not_ignored` |
| `DISC_6F92A39D9A99760BACBF` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_buffered_activation_and_fifo_queue` | `not_ignored` |
| `DISC_9198B29A72F5020674E8` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_buffered_commands_report_commandaborted_after_active_fault` | `not_ignored` |
| `DISC_D2D27C18BFA8A70AC7E4` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_buffered_home_halt_and_additive_take_ownership_after_active_motion` | `not_ignored` |
| `DISC_4CCCDEB5FA8584377F51` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_buffered_queued_updates_are_ignored_until_new_execute` | `not_ignored` |
| `DISC_C4454E17A90BBBE1C607` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_continuous_commandaborted_resets_in_end_velocity` | `not_ignored` |
| `DISC_F13AC6997A7FD60EED83` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_continuous_update_changes_target_and_end_velocity` | `not_ignored` |
| `DISC_701897B8679138A4D7E5` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_core_command_fbs_resolve` | `not_ignored` |
| `DISC_5F524D693DA68977C204` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_direction_literals_resolve` | `not_ignored` |
| `DISC_E8318B8E5430E79C7F9D` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_dynamic_max_parameter_rejections` | `not_ignored` |
| `DISC_C68BFCBDBFD459D1C49E` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_enable_style_valid_error_exclusivity` | `not_ignored` |
| `DISC_4D61628552F9B9D9FA31` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_end_to_end_conformance_scenario` | `not_ignored` |
| `DISC_C4CB455123D877A5A038` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_error_constants_resolve` | `not_ignored` |
| `DISC_D1C0F9925AAEBA3FF48C` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_execute_edge_and_continuous_update_semantics` | `not_ignored` |
| `DISC_B3E4695231E3FBFE69DC` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_execution_mode_literals_resolve` | `not_ignored` |
| `DISC_A948EFE3FEB92F48BCD6` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_grouped_axis_motion_rejection_and_readonly_allowance` | `not_ignored` |
| `DISC_2D91146D7B8C1DFE6786` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_home_backend_fault_enters_errorstop` | `not_ignored` |
| `DISC_FF9F0E89FCE6915EF65F` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_home_from_nonstandstill_entry_state` | `not_ignored` |
| `DISC_BA399D90E1A53BEC70E5` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_home_halt_stop_and_continuous_end_velocity_behaviors` | `not_ignored` |
| `DISC_C48C8C54CA91253D82E5` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_mc_power_status_tracks_stage_not_enable` | `not_ignored` |
| `DISC_C08DC15CE5C5CF7C173D` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_missing_inputs_reuse_previous_invocation_values` | `not_ignored` |
| `DISC_74B6CD51B9D679A9EC81` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_motion_fbs_resolve` | `not_ignored` |
| `DISC_A5E1F0D51EB269BCB991` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_move_absolute_final_position_semantics` | `not_ignored` |
| `DISC_26506956BA6A5C2DD450` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_move_relative_continuous_update_uses_command_start_reference` | `not_ignored` |
| `DISC_7BBC6F59F62478AC8BA5` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_move_velocity_direction_and_state` | `not_ignored` |
| `DISC_52A80BCD49F3C1847B01` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_move_velocity_participates_in_abort_and_buffer_queue` | `not_ignored` |
| `DISC_CCC9A9488C1BB3CD78B2` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_numeric_parameter_dynamics_roundtrip` | `not_ignored` |
| `DISC_DE8C7AADD749CFBC1C51` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_numeric_parameter_position_and_limit_roundtrip` | `not_ignored` |
| `DISC_5114A7582A357DA76CD7` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_parameter_fbs_resolve` | `not_ignored` |
| `DISC_7C6E5EAD412F880FA7A8` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_parameter_number_constants_resolve` | `not_ignored` |
| `DISC_D77425946527D9920334` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_parameter_plane_rejections_and_mcDelayed` | `not_ignored` |
| `DISC_8F5D2471DE1C136226B1` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_queued_parameter_writes_follow_motion_queue_order` | `not_ignored` |
| `DISC_217BC237A6362811819A` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_read_axis_info_and_axis_error` | `not_ignored` |
| `DISC_0B84404FD879231032CC` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_read_motion_state_source_and_flags` | `not_ignored` |
| `DISC_9A22078FFE3F42C6CF75` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_read_status_and_reset_state_paths` | `not_ignored` |
| `DISC_D36125015BFB257EC03F` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_relative_and_additive_distinction` | `not_ignored` |
| `DISC_BE9612BC1CA820BEA06F` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_set_override_validates_factors_and_vel_zero_behavior` | `not_ignored` |
| `DISC_CCEA4C9F475B0C110A96` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_set_position_updates_actual_and_commanded_values` | `not_ignored` |
| `DISC_6746AE13B165243A8AF1` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_sign_rules_for_motion_inputs` | `not_ignored` |
| `DISC_A8A74B3AAC5976147216` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_software_limits_clamp_position_target_commands` | `not_ignored` |
| `DISC_AB6C7653376755D50F68` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_source_literals_resolve` | `not_ignored` |
| `DISC_B4F8882DBD25B293A821` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | `plcopen_motion_single_axis_zero_dynamics_use_configured_axis_maxima` | `not_ignored` |
| `DISC_86494BA1EAA94FE694AA` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_deferred_public_surface/src/tests.st` | `plcopen_motion_single_axis_deferred_public_fbs_are_absent` | `not_ignored` |
| `DISC_7A2CB667ED84A152C014` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_group_label/src/tests.st` | `plcopen_motion_single_axis_negative_group_label` | `not_ignored` |
| `DISC_8A532A8654EA2567E64D` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_power_enable_split/src/tests.st` | `plcopen_motion_single_axis_negative_power_enable_split` | `not_ignored` |
| `DISC_D48517ABAE500DF29D18` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_public_surface/src/tests.st` | `plcopen_motion_single_axis_negative_public_surface` | `not_ignored` |
| `DISC_458AD738C2D721187347` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_stop_active/src/tests.st` | `plcopen_motion_single_axis_negative_stop_active` | `not_ignored` |
| `DISC_9EF2D37CA6E7DE6E97D2` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_transition_vel_next/src/tests.st` | `plcopen_motion_single_axis_negative_transition_vel_next` | `not_ignored` |
| `DISC_68F811AD03D9E9630BBF` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_transition_vel_zero/src/tests.st` | `plcopen_motion_single_axis_negative_transition_vel_zero` | `not_ignored` |
| `DISC_DE4E11FFC250E64DE296` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization/src/tests.st` | `plcopen_motion_synchronization_aborting_command_flushes_active_and_queued_sync_commands` | `not_ignored` |
| `DISC_96753207BB74B624DDA7` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization/src/tests.st` | `plcopen_motion_synchronization_buffered_command_only_becomes_active_on_promotion` | `not_ignored` |
| `DISC_967EBB6EA37094D54462` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization/src/tests.st` | `plcopen_motion_synchronization_cam_activation_timing_is_deterministic` | `not_ignored` |
| `DISC_5E179124379CB0BB9824` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization/src/tests.st` | `plcopen_motion_synchronization_cam_and_gear_end_to_end_scenario` | `not_ignored` |
| `DISC_B68FEE8275296F574A1C` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization/src/tests.st` | `plcopen_motion_synchronization_cam_table_select_prepares_camtableid_and_rejects_delayed` | `not_ignored` |
| `DISC_45D13128696AF0240E7B` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization/src/tests.st` | `plcopen_motion_synchronization_camin_absolute_relative_and_actual_source_are_distinct` | `not_ignored` |
| `DISC_1888F8680BEBCE9C3BEA` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization/src/tests.st` | `plcopen_motion_synchronization_camin_missing_selected_cam_errors` | `not_ignored` |
| `DISC_49FBC524F169E8592A99` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization/src/tests.st` | `plcopen_motion_synchronization_camout_leaves_synchronized_motion_without_standstill` | `not_ignored` |
| `DISC_C1A2F619F2BC77FF156A` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization/src/tests.st` | `plcopen_motion_synchronization_gearin_tracks_master_ratio_and_ingear` | `not_ignored` |
| `DISC_B1C0989DA04CD6A68950` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization/src/tests.st` | `plcopen_motion_synchronization_gearinpos_startsync_then_insync` | `not_ignored` |
| `DISC_9737FCD6F54DA15B46FA` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization/src/tests.st` | `plcopen_motion_synchronization_gearout_requires_synchronized_motion` | `not_ignored` |
| `DISC_66ECA62ECD88DC65D735` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization/src/tests.st` | `plcopen_motion_synchronization_master_slave_phase_relationships_are_deterministic` | `not_ignored` |
| `DISC_0CCEB06343FFCE8E9522` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization/src/tests.st` | `plcopen_motion_synchronization_public_surface_resolves` | `not_ignored` |
| `DISC_A3B01B7E0984FA55A1DA` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization/src/tests.st` | `plcopen_motion_synchronization_sync_types_resolve` | `not_ignored` |
| `DISC_FB8A8871D2132A36C266` | `structured_text_test` | `crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization_negative_deferred_public_surface/src/tests.st` | `plcopen_motion_synchronization_deferred_public_surface_is_absent` | `not_ignored` |
| `DISC_DEBFC2798C267C9DB3C9` | `vscode_test` | `editors/vscode/src/test/suite/ads-status-summary.test.ts` | `handles missing and empty status reports` | `not_ignored` |
| `DISC_08764DB36247B68D502F` | `vscode_test` | `editors/vscode/src/test/suite/ads-status-summary.test.ts` | `summarizes device and degraded counts for runtime pane and ADS panel` | `not_ignored` |
| `DISC_A720969C879F1F3AC722` | `vscode_test` | `editors/vscode/src/test/suite/blockly-engine.test.ts` | `generates ST for Blockly while/until blocks` | `not_ignored` |
| `DISC_51E1E3E54C0DF2B2A921` | `vscode_test` | `editors/vscode/src/test/suite/blockly-engine.test.ts` | `generates complete connected statement chains` | `not_ignored` |
| `DISC_BB4FD7B1D3D0AB1D8459` | `vscode_test` | `editors/vscode/src/test/suite/blockly-engine.test.ts` | `resolves Blockly variable ids and infers untyped numeric variables` | `not_ignored` |
| `DISC_A548A7827B2CB72B11D9` | `vscode_test` | `editors/vscode/src/test/suite/blockly-engine.test.ts` | `supports IF0/DO0 input slots from Blockly control blocks` | `not_ignored` |
| `DISC_BEE91505AD1496C564EF` | `vscode_test` | `editors/vscode/src/test/suite/check-program.integration.test.ts` | `Compile diagnostics use the truST Problems source` | `not_ignored` |
| `DISC_9609ACF9F27D90EDB8A8` | `vscode_test` | `editors/vscode/src/test/suite/check-program.integration.test.ts` | `Compile reports an actionable missing runtime binary` | `not_ignored` |
| `DISC_5E22A3DC9F179CDE7F49` | `vscode_test` | `editors/vscode/src/test/suite/check-program.integration.test.ts` | `Compile reports an actionable runtime report version mismatch` | `not_ignored` |
| `DISC_BE5DA848938B7233D91D` | `vscode_test` | `editors/vscode/src/test/suite/connector-status-contract.test.ts` | `maps every canonical state and health without changing wire meaning` | `not_ignored` |
| `DISC_50B6D52320D460D3E7FB` | `vscode_test` | `editors/vscode/src/test/suite/connector-status-contract.test.ts` | `rejects unknown state and health instead of rendering healthy` | `not_ignored` |
| `DISC_34FD40C88CF9835E09CE` | `vscode_test` | `editors/vscode/src/test/suite/debug-io.integration.test.ts` | `integration: VM debug session returns non-empty stackTrace at stopOnEntry` | `not_ignored` |
| `DISC_C96EFCC4AFD7E327ABFB` | `vscode_test` | `editors/vscode/src/test/suite/debug-io.integration.test.ts` | `integration: auto default configuration creation` | `not_ignored` |
| `DISC_84316ED7702672ED0DB7` | `vscode_test` | `editors/vscode/src/test/suite/debug-io.integration.test.ts` | `integration: debug command surface is registered` | `not_ignored` |
| `DISC_A67B070B1504B924865D` | `vscode_test` | `editors/vscode/src/test/suite/debug-io.integration.test.ts` | `integration: io and expression commands are callable and reject without session` | `not_ignored` |
| `DISC_50DDA07ADA21B206FD3E` | `vscode_test` | `editors/vscode/src/test/suite/debug-io.integration.test.ts` | `integration: reload command returns failure without active structured-text session` | `not_ignored` |
| `DISC_BA40C1B0B9331E63E9B7` | `vscode_test` | `editors/vscode/src/test/suite/debug-io.integration.test.ts` | `integration: settings update persists values` | `not_ignored` |
| `DISC_82104467EBE44E6F96C1` | `vscode_test` | `editors/vscode/src/test/suite/debug-io.integration.test.ts` | `integration: stop command returns false without active structured-text session` | `not_ignored` |
| `DISC_F02ADA8D968F0515D0D6` | `vscode_test` | `editors/vscode/src/test/suite/debug-io.integration.test.ts` | `unit: Live Values I/O force/release use attach-safe custom requests` | `not_ignored` |
| `DISC_07B903BF63EBEFF86C86` | `vscode_test` | `editors/vscode/src/test/suite/debug-io.integration.test.ts` | `unit: Live Values I/O retries transient runtime busy responses` | `not_ignored` |
| `DISC_52777BD00CFC7445489F` | `vscode_test` | `editors/vscode/src/test/suite/debug-io.integration.test.ts` | `unit: Live Values transport errors use recovery text, not raw socket copy` | `not_ignored` |
| `DISC_81CBEF24931493853109` | `vscode_test` | `editors/vscode/src/test/suite/debug-io.integration.test.ts` | `unit: interactive vs auto folder selection` | `not_ignored` |
| `DISC_DA2C3C0458915D0A3A15` | `vscode_test` | `editors/vscode/src/test/suite/diagnostics.test.ts` | `augments diagnostics with IEC reference and spec link` | `not_ignored` |
| `DISC_CFA232CFABAB2B4812CE` | `vscode_test` | `editors/vscode/src/test/suite/diagnostics.test.ts` | `skips IEC augmentation when disabled` | `not_ignored` |
| `DISC_D196D28B04B92F83543F` | `vscode_test` | `editors/vscode/src/test/suite/hmi.integration.test.ts` | `LM HMI Phase 6 generate_candidates is deterministic and writes candidates evidence` | `not_ignored` |
| `DISC_C498AA34650BCD935DA0` | `vscode_test` | `editors/vscode/src/test/suite/hmi.integration.test.ts` | `LM HMI Phase 6 plan_intent writes deterministic _intent.toml` | `not_ignored` |
| `DISC_4A531E1A0C6B3A54A230` | `vscode_test` | `editors/vscode/src/test/suite/hmi.integration.test.ts` | `LM HMI Phase 6 run_journey executes API/event flow and explain_widget reports provenance` | `not_ignored` |
| `DISC_B1490B01B9B5D9120234` | `vscode_test` | `editors/vscode/src/test/suite/hmi.integration.test.ts` | `LM HMI Phase 6 trace_capture writes scenario traces and preview_snapshot writes viewport artifacts` | `not_ignored` |
| `DISC_EFEC7F1B777372D9AD8B` | `vscode_test` | `editors/vscode/src/test/suite/hmi.integration.test.ts` | `LM HMI Phase 6 validate emits _lock.json evidence and prunes retention` | `not_ignored` |
| `DISC_E19BD300A859D338064A` | `vscode_test` | `editors/vscode/src/test/suite/hmi.integration.test.ts` | `LM HMI get_bindings routes workspace executeCommand and validates inputs` | `not_ignored` |
| `DISC_58F952A0164BB246DEDC` | `vscode_test` | `editors/vscode/src/test/suite/hmi.integration.test.ts` | `LM HMI tools honor cancellation tokens` | `not_ignored` |
| `DISC_7A9A6B3596E1C7072160` | `vscode_test` | `editors/vscode/src/test/suite/hmi.integration.test.ts` | `LM HMI tools provide layout snapshot and dry-run patch conflicts` | `not_ignored` |
| `DISC_8790D7660B5F5125760D` | `vscode_test` | `editors/vscode/src/test/suite/hmi.integration.test.ts` | `descriptor refreshes open panel on hmi toml and svg changes` | `not_ignored` |
| `DISC_A7D53ECACA8482C27712` | `vscode_test` | `editors/vscode/src/test/suite/hmi.integration.test.ts` | `layout persistence accepts valid payload and rejects invalid page IDs` | `not_ignored` |
| `DISC_AD8E5A6723E12743A97D` | `vscode_test` | `editors/vscode/src/test/suite/hmi.integration.test.ts` | `panel keeps section metadata for dashboard layouts` | `not_ignored` |
| `DISC_D7251D6F1D9ACF47F0CE` | `vscode_test` | `editors/vscode/src/test/suite/hmi.integration.test.ts` | `panel open + schema/value refresh pipeline` | `not_ignored` |
| `DISC_63DD97766A6537F906D1` | `vscode_test` | `editors/vscode/src/test/suite/hmi.integration.test.ts` | `panel process page loads local svg asset and keeps bindings metadata` | `not_ignored` |
| `DISC_93CF8DECB91E8D0DC40E` | `vscode_test` | `editors/vscode/src/test/suite/hmi.integration.test.ts` | `widget navigation resolves declaration location` | `not_ignored` |
| `DISC_565EB6C92141562C5923` | `vscode_test` | `editors/vscode/src/test/suite/ladder-editor-ops.test.ts` | `adds repeated parallel contact legs on existing branch` | `not_ignored` |
| `DISC_B679A4B99594A36B62D9` | `vscode_test` | `editors/vscode/src/test/suite/ladder-editor-ops.test.ts` | `auto-routes all program networks` | `not_ignored` |
| `DISC_150C5F31955DE2E77E2D` | `vscode_test` | `editors/vscode/src/test/suite/ladder-editor-ops.test.ts` | `auto-routes network wires with deterministic edge geometry` | `not_ignored` |
| `DISC_7594C874D263FE0C8B13` | `vscode_test` | `editors/vscode/src/test/suite/ladder-editor-ops.test.ts` | `creates a parallel contact branch from a selected series contact` | `not_ignored` |
| `DISC_6AD11417D2DDFE9F68B7` | `vscode_test` | `editors/vscode/src/test/suite/ladder-editor-ops.test.ts` | `creates parallel branch on auto-routed simple topology` | `not_ignored` |
| `DISC_8B6687D7A16AB2CEA777` | `vscode_test` | `editors/vscode/src/test/suite/ladder-editor-ops.test.ts` | `keeps explicit local bool declarations that are not implicit defaults` | `not_ignored` |
| `DISC_F82919B6B36E1EFA3C49` | `vscode_test` | `editors/vscode/src/test/suite/ladder-editor-ops.test.ts` | `pastes a rung and reorders layout/order fields` | `not_ignored` |
| `DISC_5BDE49066C20612FF1BD` | `vscode_test` | `editors/vscode/src/test/suite/ladder-editor-ops.test.ts` | `pastes an element into a network with new id and offset` | `not_ignored` |
| `DISC_AD8158105B4853B3FA85` | `vscode_test` | `editors/vscode/src/test/suite/ladder-editor-ops.test.ts` | `pushes lower rungs down when branch depth increases` | `not_ignored` |
| `DISC_897D7CC05D9D1CAA8D5F` | `vscode_test` | `editors/vscode/src/test/suite/ladder-editor-ops.test.ts` | `reconciles contact variable declarations without keeping typing artifacts` | `not_ignored` |
| `DISC_F2F240E78921E4010CB3` | `vscode_test` | `editors/vscode/src/test/suite/ladder-editor-ops.test.ts` | `rejects parallel shortcut when there is no horizontal space` | `not_ignored` |
| `DISC_712196B4F0DE566242C8` | `vscode_test` | `editors/vscode/src/test/suite/ladder-editor-ops.test.ts` | `replaces ladder symbols across variables and node fields` | `not_ignored` |
| `DISC_65E03CFD6240A504624F` | `vscode_test` | `editors/vscode/src/test/suite/ladder-editor-ops.test.ts` | `routes branch edges through split/merge node x positions` | `not_ignored` |
| `DISC_07FC7111DE22CBCC25DC` | `vscode_test` | `editors/vscode/src/test/suite/ladder-engine.test.ts` | `drives outputs correctly for parallel branch + NC rung` | `not_ignored` |
| `DISC_537C8C1BA0F4691432C5` | `vscode_test` | `editors/vscode/src/test/suite/ladder-engine.test.ts` | `emits diagnostics for unresolved symbols and non-assignable coil targets` | `not_ignored` |
| `DISC_AF0F41C194540E7A7AFE` | `vscode_test` | `editors/vscode/src/test/suite/ladder-engine.test.ts` | `executes all coil modes (NORMAL/SET/RESET/NEGATED)` | `not_ignored` |
| `DISC_9B8E3C672377BC5A42F1` | `vscode_test` | `editors/vscode/src/test/suite/ladder-engine.test.ts` | `executes compare and math blocks for all configured operations` | `not_ignored` |
| `DISC_17F50377B8A048BCD8C3` | `vscode_test` | `editors/vscode/src/test/suite/ladder-engine.test.ts` | `implements TON and TOF timer behaviors` | `not_ignored` |
| `DISC_4FE33FA3D455F1A5C1DE` | `vscode_test` | `editors/vscode/src/test/suite/ladder-engine.test.ts` | `implements TP pulse timer and counter FB behavior` | `not_ignored` |
| `DISC_42EA19A189BB3D741986` | `vscode_test` | `editors/vscode/src/test/suite/ladder-engine.test.ts` | `keeps buffered write commit semantics across networks` | `not_ignored` |
| `DISC_84CB52EBFBF0F798EBA3` | `vscode_test` | `editors/vscode/src/test/suite/ladder-engine.test.ts` | `rejects invalid topology with actionable diagnostics` | `not_ignored` |
| `DISC_5B9660200BF4D381F91A` | `vscode_test` | `editors/vscode/src/test/suite/ladder-engine.test.ts` | `rejects unsupported coil symbol types` | `not_ignored` |
| `DISC_74E35A77C5958C9561B8` | `vscode_test` | `editors/vscode/src/test/suite/ladder-engine.test.ts` | `resolves local/global symbols with local-first precedence` | `not_ignored` |
| `DISC_15A0E67FA29C8111BFE1` | `vscode_test` | `editors/vscode/src/test/suite/ladder-engine.test.ts` | `supports numeric symbol operands and outputs for compare/math nodes` | `not_ignored` |
| `DISC_FC396F7F2CBA56D40FFC` | `vscode_test` | `editors/vscode/src/test/suite/ladder-engine.test.ts` | `supports parallel branch semantics via topology edges` | `not_ignored` |
| `DISC_F01D0837A17A31A084A0` | `vscode_test` | `editors/vscode/src/test/suite/ladder-engine.test.ts` | `supports symbolic input force/write against declared variables` | `not_ignored` |
| `DISC_571790EA1A64FABC5505` | `vscode_test` | `editors/vscode/src/test/suite/ladder-engine.test.ts` | `validates deterministic series NO/NC contact semantics` | `not_ignored` |
| `DISC_33AEB77AC1C4D59D4762` | `vscode_test` | `editors/vscode/src/test/suite/ladder-runtime-io-panel.test.ts` | `confirms symbolic memory force immediately without stIoState roundtrip` | `not_ignored` |
| `DISC_65286B83D8871F8B4B71` | `vscode_test` | `editors/vscode/src/test/suite/ladder-runtime-io-panel.test.ts` | `keeps symbolic debug addresses case-preserved while canonicalizing direct I/O` | `not_ignored` |
| `DISC_1677D830A336C8F5C258` | `vscode_test` | `editors/vscode/src/test/suite/ladder-runtime-io-panel.test.ts` | `maps local symbols to FB-qualified write targets` | `not_ignored` |
| `DISC_ABB7C7B58E6F3BEEA562` | `vscode_test` | `editors/vscode/src/test/suite/ladder-runtime-io-panel.test.ts` | `marks write operations pending until matching runtime I/O confirmation arrives` | `not_ignored` |
| `DISC_10F6FB6E0E8E65F46A93` | `vscode_test` | `editors/vscode/src/test/suite/ladder-runtime-io-panel.test.ts` | `syncs in-memory ladder program to disk before runtime start` | `not_ignored` |
| `DISC_73D70852591F6D41A57F` | `vscode_test` | `editors/vscode/src/test/suite/ladder-runtime-io-panel.test.ts` | `updates existing symbolic row by write target without duplicates` | `not_ignored` |
| `DISC_B047232E5B34B1F4C89C` | `vscode_test` | `editors/vscode/src/test/suite/ladder-schema.test.ts` | `accepts schema v2 fixtures` | `not_ignored` |
| `DISC_215F95264ECBE3E9B8C7` | `vscode_test` | `editors/vscode/src/test/suite/ladder-schema.test.ts` | `rejects invalid enum symbols with actionable error` | `not_ignored` |
| `DISC_081158F35E1F7880AD87` | `vscode_test` | `editors/vscode/src/test/suite/ladder-schema.test.ts` | `rejects legacy schema fixtures with actionable error` | `not_ignored` |
| `DISC_B48545B27F159FE8F46E` | `vscode_test` | `editors/vscode/src/test/suite/ladder-schema.test.ts` | `requires declared symbols for all ladder example node references` | `not_ignored` |
| `DISC_B5D49B0EB6C87CA0759D` | `vscode_test` | `editors/vscode/src/test/suite/libraries-model.test.ts` | `adds dependencies without rewriting existing project manifest shape` | `not_ignored` |
| `DISC_124A8D8170F4C3475FD9` | `vscode_test` | `editors/vscode/src/test/suite/libraries-model.test.ts` | `classifies git pins and normalizes paths` | `not_ignored` |
| `DISC_5511874A19C80F6D603F` | `vscode_test` | `editors/vscode/src/test/suite/libraries-model.test.ts` | `formats git dependency with exactly one pin selector` | `not_ignored` |
| `DISC_CD970D9CADC3B5432E6D` | `vscode_test` | `editors/vscode/src/test/suite/libraries-model.test.ts` | `parses path and git dependencies` | `not_ignored` |
| `DISC_C39EEE880A581D5DB06A` | `vscode_test` | `editors/vscode/src/test/suite/libraries-model.test.ts` | `reads package version and groups library symbols` | `not_ignored` |
| `DISC_232AEE406DE0621AEAA3` | `vscode_test` | `editors/vscode/src/test/suite/libraries-model.test.ts` | `updates and removes dependency entries in place` | `not_ignored` |
| `DISC_A3D916B00BC4D04BC5A0` | `vscode_test` | `editors/vscode/src/test/suite/library-code-actions.test.ts` | `does not offer OSCAT when the project already depends on it` | `not_ignored` |
| `DISC_AD4D72A19225B2F6FC96` | `vscode_test` | `editors/vscode/src/test/suite/library-code-actions.test.ts` | `offers OSCAT for known OSCAT symbols when the dependency is missing` | `not_ignored` |
| `DISC_685679B1DD97897EAF8A` | `vscode_test` | `editors/vscode/src/test/suite/library-code-actions.test.ts` | `offers PLCopen Motion for MC symbols and ignores unknown symbols` | `not_ignored` |
| `DISC_5F1947728716956A6907` | `vscode_test` | `editors/vscode/src/test/suite/lm-tools-contract.test.ts` | `HMI page listing excludes scene view payload TOML` | `not_ignored` |
| `DISC_F122614B980565AA0801` | `vscode_test` | `editors/vscode/src/test/suite/lm-tools-contract.test.ts` | `manifest declarations match activation events and registered tool names` | `not_ignored` |
| `DISC_8AD54E0E8B8724B3CF3F` | `vscode_test` | `editors/vscode/src/test/suite/lm-tools-contract.test.ts` | `synthetic registration drift is detected by the contract check` | `not_ignored` |
| `DISC_5DE612D3737058A4343D` | `vscode_test` | `editors/vscode/src/test/suite/lsp.integration.test.ts` | `code actions surface inline variable` | `not_ignored` |
| `DISC_4D5C12CE0D1149E9360B` | `vscode_test` | `editors/vscode/src/test/suite/lsp.integration.test.ts` | `code actions surface interface stub generation` | `not_ignored` |
| `DISC_AC7DE855EF1DECA51261` | `vscode_test` | `editors/vscode/src/test/suite/lsp.integration.test.ts` | `code actions surface undefined variable quick fix` | `not_ignored` |
| `DISC_197943C28763CE20D5DA` | `vscode_test` | `editors/vscode/src/test/suite/lsp.integration.test.ts` | `completion returns top-level keywords` | `not_ignored` |
| `DISC_2ACBB5889DA47BBE7C49` | `vscode_test` | `editors/vscode/src/test/suite/lsp.integration.test.ts` | `dependent diagnostics refresh across open files after edits` | `not_ignored` |
| `DISC_AE0D5D0F278C655AB826` | `vscode_test` | `editors/vscode/src/test/suite/lsp.integration.test.ts` | `executeCommand relocates namespaces across files` | `not_ignored` |
| `DISC_7D478082563897055AFE` | `vscode_test` | `editors/vscode/src/test/suite/lsp.integration.test.ts` | `formatting applies canonical layout` | `not_ignored` |
| `DISC_847A44F6019B1C0F755F` | `vscode_test` | `editors/vscode/src/test/suite/lsp.integration.test.ts` | `method calls surface VAR_INPUT completions and signature help` | `not_ignored` |
| `DISC_90BAEA4AAE83469926B4` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `ADD_PICKER_GROUPS is the canonical S-09 group order` | `not_ignored` |
| `DISC_33D8B26A40A6BF79BA14` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `ADS client links label the external counterpart as an ADS server` | `not_ignored` |
| `DISC_E211B66071E114C122CD` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `ADS server live client count survives topology to endpoint node data` | `not_ignored` |
| `DISC_1E1DEF1B911118F30BCE` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `ADS tag import enables the runtime ADS subsystem` | `not_ignored` |
| `DISC_2EAF4AB744C42E1A7A45` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `Network Canvas owns comms in-canvas — no Communication-panel command, import, or copy` | `not_ignored` |
| `DISC_CD7E2EE558BA0DABA256` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `OPC UA client links label the external counterpart as an OPC UA server` | `not_ignored` |
| `DISC_0D809E233BF48774356B` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `OPC UA client test failures render user-facing recovery text, not raw backend tokens` | `not_ignored` |
| `DISC_226CFA302E7FA001B2D3` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `OPC UA server links label the external counterpart as an OPC UA client` | `not_ignored` |
| `DISC_C1F8AA75F655057CEADF` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `a configured-but-unreachable fleet peer synthesizes an UNKNOWN node (not 'stopped'), never green` | `not_ignored` |
| `DISC_A6B6F73C6E5ADDED9782` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `a live local simulator topology hides raw ST resource names even when mode is missing` | `not_ignored` |
| `DISC_405A44F5D3232C0EBBA5` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `a live local simulator topology replaces the stopped project overlay instead of twinning` | `not_ignored` |
| `DISC_12717D8B241409AF09E2` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `a managed runtime already shown via fleet.topology is NOT doubled, and the surviving node stays OWNED (managed)` | `not_ignored` |
| `DISC_A66A27154365367DB516` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `a new project with no configured runtime shows the local simulator node, not an empty screen` | `not_ignored` |
| `DISC_EC00DFC397E9AC057FAC` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `a running local simulator renders as a connected runtime node` | `not_ignored` |
| `DISC_C112E4BBB5956E461FC0` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `a runtime start failure renders an error node + retry banner, not a failure screen` | `not_ignored` |
| `DISC_0DCEC002FD93F473483F` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `a stopped local control socket renders neutral stopped state, not an alarm fault` | `not_ignored` |
| `DISC_7BC1DF86B7EFCF27106D` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `advanced picker copy is user-facing and not backend review prose` | `not_ignored` |
| `DISC_0E8D300F6DFE48A05692` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `auth-failed synthetic runtime nodes keep control endpoint for inspector actions` | `not_ignored` |
| `DISC_5B8BA330CD45E51DAB5E` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `buildCanvasGraph does not duplicate a runtime as an external system` | `not_ignored` |
| `DISC_1FD0BC2C949021DCF273` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `buildCanvasGraph maps a real fleet to host/runtime/endpoint nodes + links` | `not_ignored` |
| `DISC_A8FAE8189430088CF4E9` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `buildCanvasGraph never emits an edge to a node that does not exist` | `not_ignored` |
| `DISC_EF8E811C426BB7C83243` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `buildCanvasGraph shows added fleet peers even on the stopped local-simulator view` | `not_ignored` |
| `DISC_E528A4C2F3198956CA71` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `canvas protocol names spell ADS direction instead of relying on the role band` | `not_ignored` |
| `DISC_AA87181C9C979EBC1620` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `classifyRuntimeStartFailure maps real error strings to actionable kinds` | `not_ignored` |
| `DISC_FA804B633AA0D28D50D0` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `conditional schema fields render and validate only for the selected backend` | `not_ignored` |
| `DISC_D61A79F526EF6A0C1B27` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `configured ADS overlay on a live simulator says restart required, not stopped` | `not_ignored` |
| `DISC_2A0B29F34861721CB3BA` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `connector status presentation uses first-user vocabulary` | `not_ignored` |
| `DISC_BE2C7D87B3B90930889A` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `connector status surface flows into endpoint graph metadata` | `not_ignored` |
| `DISC_C8A4FF4AFC74089E95CA` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `device goes connected only after real I/O values are reported` | `not_ignored` |
| `DISC_2FDB6B44C3ADF33D8F12` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `does not render runtime discovery as a protocol card` | `not_ignored` |
| `DISC_43F5EE94505FE33A1D90` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `draft/pending mesh peers drop a dashed bus wire; connected peers stay solid` | `not_ignored` |
| `DISC_4EC5E972D8211E2A93A1` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `draft/pending wire links render dashed, connected links stay solid (honest: not yet a live link)` | `not_ignored` |
| `DISC_EFD3D72BF8E066863354` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `drops topology-only evidence fields before re-applying ADS/OPC UA server config` | `not_ignored` |
| `DISC_23F2A8721296F667FE0C` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `editing a rejected add form hides stale apply faults without hiding real faults` | `not_ignored` |
| `DISC_DD3598DBD8291916D08E` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `external protocol nodes render display names, not raw driver ids` | `not_ignored` |
| `DISC_0DE2CE24974724ED73D7` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `fleet host headlines are user-facing, not raw lab machine names` | `not_ignored` |
| `DISC_B9C36FFCC93EDBF29640` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `fleet search dims external counterparts and wires without hiding warnings` | `not_ignored` |
| `DISC_8A4998901CB34C8FC5C8` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `fleet search never hides degraded endpoints from the runtime rollup` | `not_ignored` |
| `DISC_17AB49919A3EAEDECF4A` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `groups protocols by user intent in the S-09 order` | `not_ignored` |
| `DISC_AF254B63B201F1A10FB4` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `live managed topology preserves the stopped project runtime instead of morphing the canvas` | `not_ignored` |
| `DISC_068E48931FB2B7412656` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `local I/O endpoint titles leave the I/O role to the node band` | `not_ignored` |
| `DISC_E8833149FE742FE82266` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `managed local runtimes are injected under the existing This computer host` | `not_ignored` |
| `DISC_A209ED1B25924C1DCFE9` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `mergeFleetTopologies aggregates multiple runtimes: a host appears once with unioned runtimes` | `not_ignored` |
| `DISC_DB99796951016467844E` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `mergeFleetTopologies keeps configured endpoints on the same live runtime` | `not_ignored` |
| `DISC_BF68B6908A5FA1711EF0` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `omits empty groups and keeps advanced choices separate` | `not_ignored` |
| `DISC_F479F7DFFB964F434556` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `package contributes the Network Canvas command` | `not_ignored` |
| `DISC_2DE32D7921A10E279840` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `protocol filtering reports hidden degraded/faulted endpoints instead of silently losing them` | `not_ignored` |
| `DISC_8C1803B0FB55F1394443` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `routes unknown protocols to a trailing advanced Other choices group and never drops anything` | `not_ignored` |
| `DISC_950A2C69B282CE278663` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `runtime goes green only from runtime lifecycle evidence` | `not_ignored` |
| `DISC_7170BF00B718605FC49B` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `runtime rolls health up from raw endpoint evidence; host stays reachability` | `not_ignored` |
| `DISC_5CA934F017401A2C8D01` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `selected run target is projected onto the graph node and rendered node data` | `not_ignored` |
| `DISC_6849941EF510D0146E25` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `server and client pairs have distinct badges and direction copy` | `not_ignored` |
| `DISC_835787AD3E3209CDB992` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `server endpoint summaries answer where the server is and what it exposes` | `not_ignored` |
| `DISC_E7AF3FFC18B3C256B411` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `single-peer mesh fabric suppresses redundant bus label` | `not_ignored` |
| `DISC_A029791189004BCC1B61` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `stage progression alone never fabricates a running runtime or connected device` | `not_ignored` |
| `DISC_1E9A10F11AB488DC37C2` | `vscode_test` | `editors/vscode/src/test/suite/network-canvas.test.ts` | `unreachable configured peers keep their configured label, not 'This computer'` | `not_ignored` |
| `DISC_D773E74F5475E15798E3` | `vscode_test` | `editors/vscode/src/test/suite/new-project.test.ts` | `cancel at each prompt stage leaves filesystem unchanged` | `not_ignored` |
| `DISC_E6FC558146D4266DE3E2` | `vscode_test` | `editors/vscode/src/test/suite/new-project.test.ts` | `creates scaffold in an empty target directory` | `not_ignored` |
| `DISC_BCB84D739D52ED271B80` | `vscode_test` | `editors/vscode/src/test/suite/new-project.test.ts` | `existing target requires explicit confirmation behavior` | `not_ignored` |
| `DISC_78213ECD5BFD1CD1ACE3` | `vscode_test` | `editors/vscode/src/test/suite/new-project.test.ts` | `generated ST parses cleanly and TOML is usable by build` | `not_ignored` |
| `DISC_C93219F2DFCEFB0A61E5` | `vscode_test` | `editors/vscode/src/test/suite/new-project.test.ts` | `works in single-root and multi-root workspace setups` | `not_ignored` |
| `DISC_948DF4BCD12D709C5BAE` | `vscode_test` | `editors/vscode/src/test/suite/opcua-client-model.test.ts` | `a leaf with no raw node_id cannot be saved (guards pre-B1 sanitized-only ids)` | `not_ignored` |
| `DISC_BF89FF7E213BC674E7D5` | `vscode_test` | `editors/vscode/src/test/suite/opcua-client-model.test.ts` | `browse error details are user-facing recovery text, not raw status tokens` | `not_ignored` |
| `DISC_18AD35CEE4E4E1805B8A` | `vscode_test` | `editors/vscode/src/test/suite/opcua-client-model.test.ts` | `browse errors map to exactly one recovery action` | `not_ignored` |
| `DISC_104380B160C7A9B5174A` | `vscode_test` | `editors/vscode/src/test/suite/opcua-client-model.test.ts` | `buildOpcuaConnection assembles connection + points from target` | `not_ignored` |
| `DISC_368BF6D3E7B39FA811AA` | `vscode_test` | `editors/vscode/src/test/suite/opcua-client-model.test.ts` | `buildOpcuaConnection returns undefined with no usable points or endpoint` | `not_ignored` |
| `DISC_2D1E7A7BD56B5D79609E` | `vscode_test` | `editors/vscode/src/test/suite/opcua-client-model.test.ts` | `connection name slugs from endpoint when label is the raw protocol id` | `not_ignored` |
| `DISC_0AA29F5181D7C30D0F43` | `vscode_test` | `editors/vscode/src/test/suite/opcua-client-model.test.ts` | `display type prefers resolved data_type, falls back to raw type` | `not_ignored` |
| `DISC_C21CEFA45C9E3177C896` | `vscode_test` | `editors/vscode/src/test/suite/opcua-client-model.test.ts` | `leaves whose sanitized id collides stay distinct by nodeKey (React-key safety)` | `not_ignored` |
| `DISC_88968353809106D82006` | `vscode_test` | `editors/vscode/src/test/suite/opcua-client-model.test.ts` | `nodeKey prefers the raw node_id, then the React id, then the path` | `not_ignored` |
| `DISC_E915ED28BCAE9AFEB821` | `vscode_test` | `editors/vscode/src/test/suite/opcua-client-model.test.ts` | `point round-trips the raw node_id and apply-ready type` | `not_ignored` |
| `DISC_C44D1E0473FB74F920F7` | `vscode_test` | `editors/vscode/src/test/suite/opcua-client-model.test.ts` | `selectedLeaves returns only chosen leaves in tree order` | `not_ignored` |
| `DISC_758AC32EFEB16FBE7FF3` | `vscode_test` | `editors/vscode/src/test/suite/opcua-client-model.test.ts` | `two leaves sharing a path but different node_id are NOT conflated (B1 integrity)` | `not_ignored` |
| `DISC_B5A878128C809027FC48` | `vscode_test` | `editors/vscode/src/test/suite/opcua-client-model.test.ts` | `username auth carries credentials; anonymous does not` | `not_ignored` |
| `DISC_B5122EB8EE5BC09D712E` | `vscode_test` | `editors/vscode/src/test/suite/opcua-client-model.test.ts` | `var name is deterministic and folds non-identifier chars` | `not_ignored` |
| `DISC_327BCBBDB66B598A4E81` | `vscode_test` | `editors/vscode/src/test/suite/opcua-client-model.test.ts` | `write access requires server-writable AND user opt-in` | `not_ignored` |
| `DISC_0D25BB0929FCA73D80AA` | `vscode_test` | `editors/vscode/src/test/suite/plcopen-export.test.ts` | `cancel paths do not perform export` | `not_ignored` |
| `DISC_902BDB2DA35878F40438` | `vscode_test` | `editors/vscode/src/test/suite/plcopen-export.test.ts` | `existing output requires explicit overwrite` | `not_ignored` |
| `DISC_56A15B7BF3A48D9BB1D2` | `vscode_test` | `editors/vscode/src/test/suite/plcopen-export.test.ts` | `exports a project to PLCopen XML` | `not_ignored` |
| `DISC_C816BF3C4EC8C82A7894` | `vscode_test` | `editors/vscode/src/test/suite/plcopen-export.test.ts` | `missing project path is rejected` | `not_ignored` |
| `DISC_ED5CD0200D9AD0D02DE9` | `vscode_test` | `editors/vscode/src/test/suite/plcopen-import.test.ts` | `cancel paths do not perform import` | `not_ignored` |
| `DISC_49F93437ADD789A5A45F` | `vscode_test` | `editors/vscode/src/test/suite/plcopen-import.test.ts` | `existing non-empty target requires explicit overwrite` | `not_ignored` |
| `DISC_1EB6F01FF5B022528BA3` | `vscode_test` | `editors/vscode/src/test/suite/plcopen-import.test.ts` | `imports OpenPLC XML into a target project folder` | `not_ignored` |
| `DISC_09EB117F4915367BBF75` | `vscode_test` | `editors/vscode/src/test/suite/plcopen-import.test.ts` | `malformed XML reports actionable import error message` | `not_ignored` |
| `DISC_9968CDC54A0750B59765` | `vscode_test` | `editors/vscode/src/test/suite/plcopen-import.test.ts` | `missing input file is rejected` | `not_ignored` |
| `DISC_43BE24C51027D14ABF35` | `vscode_test` | `editors/vscode/src/test/suite/plcopen-import.test.ts` | `missing runtime binary reports actionable import launch error` | `not_ignored` |
| `DISC_6113A96C9747FD39E067` | `vscode_test` | `editors/vscode/src/test/suite/plcopen-ld-interop.test.ts` | `does not silently coerce invalid node enum attributes` | `not_ignored` |
| `DISC_C9125C10BBEB0DD0AF43` | `vscode_test` | `editors/vscode/src/test/suite/plcopen-ld-interop.test.ts` | `emits diagnostics for malformed node payloads` | `not_ignored` |
| `DISC_B83DB16F80E857413A39` | `vscode_test` | `editors/vscode/src/test/suite/plcopen-ld-interop.test.ts` | `exports schema v2 ladder and imports it back as schema v2` | `not_ignored` |
| `DISC_1EB4B04C7A23D47634E6` | `vscode_test` | `editors/vscode/src/test/suite/plcopen-ld-interop.test.ts` | `reports unsupported constructs during import` | `not_ignored` |
| `DISC_89803FF2606BDC1BFDFD` | `vscode_test` | `editors/vscode/src/test/suite/plcopen-runtime-errors.test.ts` | `export command failures remain generic` | `not_ignored` |
| `DISC_DE08AEC6D44823A85385` | `vscode_test` | `editors/vscode/src/test/suite/plcopen-runtime-errors.test.ts` | `import command failures distinguish malformed XML` | `not_ignored` |
| `DISC_10DC5C090BEEB06DDBB4` | `vscode_test` | `editors/vscode/src/test/suite/plcopen-runtime-errors.test.ts` | `import launch errors distinguish missing trust-runtime binary` | `not_ignored` |
| `DISC_01CE4CCE9E2B5911F30C` | `vscode_test` | `editors/vscode/src/test/suite/runtime-control-client.test.ts` | `classifies auth failures from control responses` | `not_ignored` |
| `DISC_A959A1A55BB69D0926E6` | `vscode_test` | `editors/vscode/src/test/suite/runtime-control-client.test.ts` | `distinguishes missing auth token responses` | `not_ignored` |
| `DISC_A557FDEC97F29A06F2F1` | `vscode_test` | `editors/vscode/src/test/suite/runtime-control-client.test.ts` | `probes endpoint reachability without sending a request` | `not_ignored` |
| `DISC_B439377B304423CE4345` | `vscode_test` | `editors/vscode/src/test/suite/runtime-control-client.test.ts` | `sends one JSON-line request and resolves successful responses` | `not_ignored` |
| `DISC_3E1B8A9AE3FE2AD4EC34` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `Connect failures show state-specific next actions, not auth for every failure` | `not_ignored` |
| `DISC_8B4AD9AE67B6DC72227F` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `Devices & Connections wires remote auth recovery through the SecretStorage command` | `not_ignored` |
| `DISC_21EF7498B8A7DC8A6855` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `ERR-04 control-endpoint override is test-mode only` | `not_ignored` |
| `DISC_9B5B37DA24FD67A5DFD4` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `HONESTY: a connected remote NEVER renders Stop` | `not_ignored` |
| `DISC_2F368998753B230E257E` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `HONESTY: a remote runtime NEVER renders Start or Stop (we don't own its process)` | `not_ignored` |
| `DISC_9E878E20D0C98A56EBED` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `Live Values normalizes runtime debug values before webview rendering` | `not_ignored` |
| `DISC_E2A1AA98AB818924FFF3` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `S-14: node inspector caps visible secondary actions at two with an overflow disclosure` | `not_ignored` |
| `DISC_A528632397A0B458E427` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `Start and Update disable with reasons when compile/config validity cannot succeed` | `not_ignored` |
| `DISC_20C3697C7228FCD45743` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `Start compiles first and does not launch after a failed Compile` | `not_ignored` |
| `DISC_7F373CD035DE6D8E3CF8` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `Update running simulation cannot hang forever on a stuck stReload request` | `not_ignored` |
| `DISC_4708AAFD6668D62FBB0F` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `attach sessions keep raw adapter logs out of canvas and Live Values workflows` | `not_ignored` |
| `DISC_C0A229CB0B1B32D38729` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `background I/O refresh failures do not persist as sidebar start failures` | `not_ignored` |
| `DISC_15BB68D9CC310C933A4B` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `compile/update gates use one shared user-facing reason model` | `not_ignored` |
| `DISC_6112977138BECAD7539A` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `dropdown is SELECT-ONLY: simulator first, then remotes — NO Add/Connect sentinel; invalid selection falls back to simulator` | `not_ignored` |
| `DISC_4BCDD2A839BE69819446` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `exactly one truST activity container + one view, and the view is a WebviewView` | `not_ignored` |
| `DISC_00AB9CCB602C92B09A3C` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `lifecycle success is HONEST: Start only at 'running', Stop only at 'stopped'` | `not_ignored` |
| `DISC_71FD9029CACB9C62677F` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `local simulator: stopped → Start, running → Stop, starting → disabled` | `not_ignored` |
| `DISC_D12F957E05EBF9F92CD0` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `managed Stop disconnects Live Values even when fleet stop omits the endpoint` | `not_ignored` |
| `DISC_84A22E5E025C0C8764A8` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `managed local runtime auth token is parsed only from runtime.control` | `not_ignored` |
| `DISC_17CD6FBA86AA28D7FF7D` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `managed local runtime auth token parser accepts top-level dotted runtime.control form` | `not_ignored` |
| `DISC_582285AA211A54728D6B` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `managed local runtime node: Start when stopped, Stop when running — never Connect (we own it)` | `not_ignored` |
| `DISC_9754F0A553AD2B4D9D7C` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `managed local runtime: projected into the dropdown; Start when stopped, Stop when running (we own it)` | `not_ignored` |
| `DISC_98A47B085D94930D3D0F` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `managed runtime logs are formatted for humans instead of raw JSON` | `not_ignored` |
| `DISC_E10F3FE5740E8A93BDEB` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `no ST editor-title Run/Stop controls` | `not_ignored` |
| `DISC_4B127A3A0DE94D4007B7` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `no status-bar / palette Start/Stop commands (one run surface)` | `not_ignored` |
| `DISC_3D764AC5F702B96CB615` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `normalizeManagedState + label` | `not_ignored` |
| `DISC_90A704FF49272CC1C299` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `primary Start can be disabled with a visible reason without hiding the affordance` | `not_ignored` |
| `DISC_C472214EBC02F1CB243C` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `remote Connect verifies control auth before opening an attach session` | `not_ignored` |
| `DISC_B262093056AF77A9B30D` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `remote attach refuses debug-disabled runtimes before reporting connected` | `not_ignored` |
| `DISC_33B2F243D7CECFF83529` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `remote auth failures make Set auth token the primary recovery without changing lifecycle ownership` | `not_ignored` |
| `DISC_62C419221630F8B20B48` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `remote labels keep the port so same-host runtimes are distinguishable` | `not_ignored` |
| `DISC_18A38E8A2E551A3401C8` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `remote: not attached → Connect, attached → Disconnect; Connect disabled without an endpoint` | `not_ignored` |
| `DISC_BE79CBCC4D898414F703` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `remote: not connected → Connect, connected → Disconnect` | `not_ignored` |
| `DISC_6281E2D00BF7DE380FD9` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `runtime node offers Set as run target + Settings; Logs only when a log backend exists` | `not_ignored` |
| `DISC_C09CC9CF86770C42BC1F` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `sidebar renders start failure messages even after simulator stays stopped` | `not_ignored` |
| `DISC_C5978692F8E72668034A` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `simulator Start treats a failed I/O probe as a failed launch` | `not_ignored` |
| `DISC_B9BDD049441ED1656A8F` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `simulator launch keeps raw adapter logs out of the first-run surface` | `not_ignored` |
| `DISC_4BFD139FD933E55F0770` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `simulator: stopped → Start, running → Stop, starting → disabled (no action)` | `not_ignored` |
| `DISC_7607EA2FDD5A7FFB1DF4` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `stopRuntime is idempotent (a disappeared session after Stop is success, not a warning)` | `not_ignored` |
| `DISC_B3400D347E75E547B9BD` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `stopping a runtime emits a fresh lifecycle refresh after the session is gone` | `not_ignored` |
| `DISC_4889180B9EDE622A411E` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `the status bar does not pretend a simulator target exists before a project exists` | `not_ignored` |
| `DISC_EBA6B8F5F5110461E18F` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `the status bar follows the selected target, not a separate simulator-only state` | `not_ignored` |
| `DISC_97F575C6CEA369A2DFDA` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `the status bar is passive: it only reveals the sidebar, never starts/stops` | `not_ignored` |
| `DISC_E2AE888B980D278EDA64` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `the truST panel is a WebviewView with examples-first onboarding and a compact action surface` | `not_ignored` |
| `DISC_4750CDD946B8A8E13A8B` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `toManagedRuntimes merges fleet list + per-name status` | `not_ignored` |
| `DISC_6F10F530E9B17D5183C6` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `unreachable runtime messages are human-facing and do not expose local socket paths` | `not_ignored` |
| `DISC_6416BA115672F37BC791` | `vscode_test` | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | `unreachable selected remote: Connect is DISABLED with a reason (never a button that just fails)` | `not_ignored` |
| `DISC_415CF41B12F676A26898` | `vscode_test` | `editors/vscode/src/test/suite/runtime-default-settings.integration.test.ts` | `activation does not seed runtime control endpoint into workspace folder settings` | `not_ignored` |
| `DISC_E558B19FFD3199EECF8F` | `vscode_test` | `editors/vscode/src/test/suite/runtime-default-settings.integration.test.ts` | `product Settings keys feed runtime config with trust-lsp fallback` | `not_ignored` |
| `DISC_F0E1633BAC0B9027B950` | `vscode_test` | `editors/vscode/src/test/suite/runtime-shared-utils.test.ts` | `buildRuntimeSourceOptions applies defaults when include globs missing` | `not_ignored` |
| `DISC_3DFD058F7CD871B79558` | `vscode_test` | `editors/vscode/src/test/suite/runtime-shared-utils.test.ts` | `buildRuntimeSourceOptions preserves explicit include globs` | `not_ignored` |
| `DISC_7B73DC47E3D57930DAA5` | `vscode_test` | `editors/vscode/src/test/suite/runtime-shared-utils.test.ts` | `isLocalControlEndpoint only accepts local addresses` | `not_ignored` |
| `DISC_8DB58FC532A558D6739E` | `vscode_test` | `editors/vscode/src/test/suite/runtime-shared-utils.test.ts` | `normalizeStringArray trims and filters non-string values` | `not_ignored` |
| `DISC_66CF23B75E3B24CAA307` | `vscode_test` | `editors/vscode/src/test/suite/runtime-shared-utils.test.ts` | `parseControlEndpoint accepts valid tcp endpoints` | `not_ignored` |
| `DISC_4AB87BE9907382DF9D61` | `vscode_test` | `editors/vscode/src/test/suite/runtime-shared-utils.test.ts` | `parseControlEndpoint handles unix endpoints on non-windows` | `not_ignored` |
| `DISC_7F1FB8705D686A0A8893` | `vscode_test` | `editors/vscode/src/test/suite/runtime-shared-utils.test.ts` | `parseControlEndpoint rejects invalid tcp endpoints` | `not_ignored` |
| `DISC_06B995D67BD0FDC9AF19` | `vscode_test` | `editors/vscode/src/test/suite/runtime-target.test.ts` | `classifies credential forwarding trust from endpoint shape` | `not_ignored` |
| `DISC_22C2B94C410B260808D8` | `vscode_test` | `editors/vscode/src/test/suite/runtime-target.test.ts` | `classifies simulate mode without probing endpoint` | `not_ignored` |
| `DISC_A12A594F3BD3543407B2` | `vscode_test` | `editors/vscode/src/test/suite/runtime-target.test.ts` | `reports auth failed when status request is rejected by auth` | `not_ignored` |
| `DISC_22FD1C7165A04CD78540` | `vscode_test` | `editors/vscode/src/test/suite/runtime-target.test.ts` | `reports missing auth token as a distinct auth failure` | `not_ignored` |
| `DISC_624A00C0CADCF7305203` | `vscode_test` | `editors/vscode/src/test/suite/runtime-target.test.ts` | `reports missing endpoint when configured endpoint is disabled` | `not_ignored` |
| `DISC_6D658370E5C9661B296D` | `vscode_test` | `editors/vscode/src/test/suite/runtime-target.test.ts` | `reports missing endpoint when online runtime has no active endpoint` | `not_ignored` |
| `DISC_9F03AA8253AE96AB07FD` | `vscode_test` | `editors/vscode/src/test/suite/runtime-target.test.ts` | `reports online reachable when probe and status succeed` | `not_ignored` |
| `DISC_2C464F5636BBC7EC9B30` | `vscode_test` | `editors/vscode/src/test/suite/runtime-target.test.ts` | `reports online unreachable when endpoint probe fails` | `not_ignored` |
| `DISC_2EB9BDFEC18EE1C25897` | `vscode_test` | `editors/vscode/src/test/suite/runtime-target.test.ts` | `reports online unreachable when status request fails after probe` | `not_ignored` |
| `DISC_4128181B64B9C220A49D` | `vscode_test` | `editors/vscode/src/test/suite/runtime-target.test.ts` | `runtime pane command is the existing runtime panel command` | `not_ignored` |
| `DISC_635BCE99FF77E25857FC` | `vscode_test` | `editors/vscode/src/test/suite/sfc-engine.test.ts` | `accepts valid parallel split/join topology` | `not_ignored` |
| `DISC_26A2975F509DF0307EA1` | `vscode_test` | `editors/vscode/src/test/suite/sfc-engine.test.ts` | `rejects join with missing/invalid continuation` | `not_ignored` |
| `DISC_FD0B15A8DDEE7430C749` | `vscode_test` | `editors/vscode/src/test/suite/sfc-engine.test.ts` | `rejects split with fewer than two branches` | `not_ignored` |
| `DISC_9C9760812C6BE468A20F` | `vscode_test` | `editors/vscode/src/test/suite/snippets.test.ts` | `expanded snippet bodies are syntactically valid ST` | `not_ignored` |
| `DISC_E83EC71DE72AA46334D6` | `vscode_test` | `editors/vscode/src/test/suite/snippets.test.ts` | `snippet JSON file is valid and includes required patterns` | `not_ignored` |
| `DISC_9232364780202EFA6D0C` | `vscode_test` | `editors/vscode/src/test/suite/snippets.test.ts` | `snippet contribution is registered with identifier-friendly aliases` | `not_ignored` |
| `DISC_032CC3FF7DCF6A6CD851` | `vscode_test` | `editors/vscode/src/test/suite/st-tests.integration.test.ts` | `discovers TEST_PROGRAM and TEST_FUNCTION_BLOCK declarations` | `not_ignored` |
| `DISC_506BE28C636C4086382B` | `vscode_test` | `editors/vscode/src/test/suite/st-tests.integration.test.ts` | `refresh clears stale results when a file no longer contains tests` | `not_ignored` |
| `DISC_5F96B77677D827FFC1A1` | `vscode_test` | `editors/vscode/src/test/suite/st-tests.integration.test.ts` | `refresh preserves results for tests that still exist` | `not_ignored` |
| `DISC_0B8331D7F8CF7AE6D1F9` | `vscode_test` | `editors/vscode/src/test/suite/st-tests.integration.test.ts` | `run all and run single commands execute expected tests` | `not_ignored` |
| `DISC_91FE5C81E187CC3EAF8B` | `vscode_test` | `editors/vscode/src/test/suite/st-tests.integration.test.ts` | `state updates track pass/fail results for UI decorations` | `not_ignored` |
| `DISC_8189DEDD635FB39B41F6` | `vscode_test` | `editors/vscode/src/test/suite/statechart-editor.lifecycle.test.ts` | `disposes running execution session when panel is closed` | `not_ignored` |
| `DISC_C77D180F1CEB748DC536` | `vscode_test` | `editors/vscode/src/test/suite/statechart-engine.test.ts` | `awaits hardware actions before transition completes` | `not_ignored` |
| `DISC_06C9F375ACF5C7378008` | `vscode_test` | `editors/vscode/src/test/suite/statechart-engine.test.ts` | `fails closed when guard I/O read errors in hardware mode` | `not_ignored` |
| `DISC_8904E723E9006EA42832` | `vscode_test` | `editors/vscode/src/test/suite/statechart-engine.test.ts` | `fails closed when guard expression is invalid in hardware mode` | `not_ignored` |
| `DISC_2C8EBA4B2DA06E88C4BD` | `vscode_test` | `editors/vscode/src/test/suite/statechart-runtime-client.test.ts` | `cleans request listeners when a request times out` | `not_ignored` |
| `DISC_F8D684DB3369FC5BF8BF` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `'Set up runtime…' wizard is capability-gated (Install/Docker gated in v1)` | `not_ignored` |
| `DISC_6C4C44310C21353333E4` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `'Set up runtime…' wizard uses the shared product inspector chrome` | `not_ignored` |
| `DISC_9C41C55D51543339DADB` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `ADS Add tags can import into a stopped project` | `not_ignored` |
| `DISC_D4B128A8F5AE94B3D615` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `ADS route recovery stays in the Browse pane and exposes Create route` | `not_ignored` |
| `DISC_0D218CEECC04A20235B3` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `ADS server allowed clients render through the humanized summary, not raw JSON pins` | `not_ignored` |
| `DISC_C187A8410C3F5EFC17E4` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Blockly generated-code actions use shared button chrome and no emoji glyphs` | `not_ignored` |
| `DISC_05A15BB8A9F6D626C468` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Blockly status counts visible blocks, not serialized top-level stacks` | `not_ignored` |
| `DISC_49C6A4C491EB5639A2C3` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Blockly toolbox labels use normal foreground tokens, not accent-button text` | `not_ignored` |
| `DISC_A3100AAEA529338DC1E9` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Blockly uses the shared truST theme instead of raw toy hues` | `not_ignored` |
| `DISC_8FFCF49A756D4A8C9752` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Compile is a fixed sidebar action plus palette escape hatch, not a Project bucket item` | `not_ignored` |
| `DISC_AF67A50F5171C862EA76` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Compile state uses icon + token role, and clean compile settles to neutral` | `not_ignored` |
| `DISC_75AEF53ED40BBF403CBD` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Connect existing runtime stores tokens securely and uses shared chrome` | `not_ignored` |
| `DISC_DC1AC628D925574CFFB5` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Connect on a runtime node ALSO sets the active Target` | `not_ignored` |
| `DISC_FAAB3AB5F11AE3745019` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Deploy is visible in the action row but disabled with a reason until supported` | `not_ignored` |
| `DISC_7B420060C563E5ACCB06` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Devices & Connections add pane follows the accepted S-09 picker taxonomy` | `not_ignored` |
| `DISC_D9A12A633886DE28361A` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Devices & Connections add pane uses the shared product chrome baseline` | `not_ignored` |
| `DISC_699C5D8166BF08EB0D87` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Devices & Connections filter panel uses plain status wording` | `not_ignored` |
| `DISC_816D7AA553F893BDFAB9` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Devices & Connections header reports active form field errors` | `not_ignored` |
| `DISC_1628F297093D44D8CA22` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Devices & Connections never opens as a blank webview while loading` | `not_ignored` |
| `DISC_54CB59758EEF8CE86156` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Devices & Connections node summaries use the shared product chrome baseline` | `not_ignored` |
| `DISC_7038F6DC111C3FA453AC` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Devices & Connections protocol identity colors use shared theme roles` | `not_ignored` |
| `DISC_2835E08A4221B4EED569` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Devices & Connections refits when endpoint children appear after managed Start` | `not_ignored` |
| `DISC_B7F41C54512F321ACFF0` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Discover Adopt preserves the runtime label and focuses the adopted node` | `not_ignored` |
| `DISC_512040D9DDB0C307ECF2` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Discover copy stays first-user-facing and avoids rejected network jargon` | `not_ignored` |
| `DISC_CD3E7F85EDD85AE5BC1B` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Discover exposes Modbus host and subnet targets separately` | `not_ignored` |
| `DISC_E528EDE2EB97182EE8A0` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Discover hardware scans are disabled with a reason until an origin can run them` | `not_ignored` |
| `DISC_DDDA00FA9228FE8E819E` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Discover result cards show runtime endpoints and candidate confidence` | `not_ignored` |
| `DISC_2A6EB35FFEBA47D17F66` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `EtherCAT channel browse saves through EtherCAT config, not ADS import` | `not_ignored` |
| `DISC_D95CA9A72034867BCC7F` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Force/Unforce work on remote attach too — the old 'not available' gate is removed` | `not_ignored` |
| `DISC_CD49E390CD8B9DBFB668` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `HMI preview formats live values like the rest of truST` | `not_ignored` |
| `DISC_FAEBCF4E161F2836E59F` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `HMI preview schedules descriptor refreshes from edit save and watcher events` | `not_ignored` |
| `DISC_B24E564555819588ED76` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `HMI preview uses shared truST product theme roles` | `not_ignored` |
| `DISC_5A9D0E72D6E039AE01AF` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Libraries failures stay visible with a recovery action but clear after success` | `not_ignored` |
| `DISC_44D2C87B1AE80FA118EB` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Libraries is reachable from the sidebar and the command palette escape hatch` | `not_ignored` |
| `DISC_BA148192A834C74C012F` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Libraries uses the shared truST theme instead of a private token layer` | `not_ignored` |
| `DISC_FF57A735C35B7DAFBB4E` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Live Values action buttons do not wrap safety verbs` | `not_ignored` |
| `DISC_F16B514F9D98C0B9FCA4` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Live Values can display word-like values as decimal hex or binary` | `not_ignored` |
| `DISC_B58468A80460889AC646` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Live Values clears connected UI immediately when a debug session terminates` | `not_ignored` |
| `DISC_8419ECFA9D379406FE91` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Live Values does not expose runtime lifecycle controls` | `not_ignored` |
| `DISC_C6E201B79D4B399A27B2` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Live Values does not force a stale split beside Devices & Connections` | `not_ignored` |
| `DISC_C5131170B8D2496CC357` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Live Values does not show stale compile diagnostics before a real result` | `not_ignored` |
| `DISC_600330EEB9F72B915087` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Live Values explains disabled program-driven writes` | `not_ignored` |
| `DISC_0C91D4D7B483CDA2CD7A` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Live Values exposes a forced-values inventory filter` | `not_ignored` |
| `DISC_A225F839B0F21FE0E476` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Live Values keeps BOOL rows compact and contextual` | `not_ignored` |
| `DISC_873EA88AC1EA9E8EEE1F` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Live Values keeps operation feedback visible in the sticky header` | `not_ignored` |
| `DISC_F8C79FF91451AE0B9A76` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Live Values lifecycle pill is lifecycle-only and does not fake remote running` | `not_ignored` |
| `DISC_0991AE26DCECBDD19614` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Live Values long signal names cannot collapse the table columns` | `not_ignored` |
| `DISC_9A440025BE9F60234946` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Live Values makes the active target and table columns visible` | `not_ignored` |
| `DISC_13E2B15D33D03714A72A` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Live Values mirrors runtime lifecycle without re-polling every I/O event` | `not_ignored` |
| `DISC_60469C891E956F61EBFC` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Live Values renders visible data-type labels instead of hidden value inference` | `not_ignored` |
| `DISC_2E5202BC6AC2BE6F5745` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Live Values uses explicit safety verbs for row actions` | `not_ignored` |
| `DISC_730148047813BBBA087C` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Live Values uses the selected runtime label instead of exposing raw endpoints` | `not_ignored` |
| `DISC_2D59D052F336ACEFF49F` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Live Values uses the shared truST product theme tokens` | `not_ignored` |
| `DISC_0672565AFED8ADC87AB6` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `NO user-facing command title contains the jargon 'Network Canvas'` | `not_ignored` |
| `DISC_34375A6CE4D12EE2ABDF` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `ONE selected-run-target store, written by the dropdown AND the graph` | `not_ignored` |
| `DISC_9CCD07B708E408EF8025` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `OPC UA browse auth warnings have an inline credential recovery action` | `not_ignored` |
| `DISC_C20CD52176684ACDB115` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `React Flow canvas controls use the shared Devices & Connections treatment` | `not_ignored` |
| `DISC_ABEAAC628F3332D9D7DE` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Release all forces exists end-to-end (button + message + host loop)` | `not_ignored` |
| `DISC_E23C36D7435DB7F2B5D3` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `SFC toolbar add actions reframe the canvas so the result is visible` | `not_ignored` |
| `DISC_FE37775F3189EFFA61DC` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `SFC transition routing avoids stacking non-linear labels through the center line` | `not_ignored` |
| `DISC_9F3F926E547FBBC9A355` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Set as run target selects WITHOUT connecting` | `not_ignored` |
| `DISC_9A79D3FA4A71E0CD5D1C` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Statechart import and add actions reframe the canvas inside the shared editor shell` | `not_ignored` |
| `DISC_B9C27DC43DA1F9D0D90F` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Structured Text Stop waits for termination before callers capture the UI` | `not_ignored` |
| `DISC_8A2D5F99C8F40E58808E` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Structured Text debugger exposes a named truST simulator configuration` | `not_ignored` |
| `DISC_D49FE0224AAC6E84626F` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `Update running simulation is sim-only, gated on a real source change, wired to the update command` | `not_ignored` |
| `DISC_A2A1A86D274913AED88A` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `VS Code extension test runner honors CARGO_TARGET_DIR` | `not_ignored` |
| `DISC_D9A2C781E64D0F605103` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `action row has a real narrow-width collapse rule` | `not_ignored` |
| `DISC_8C163CB626382DE1C514` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `add-device Test success does not render raw lifecycle tokens` | `not_ignored` |
| `DISC_94D825A21C3A00EE219E` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `add-device form does not reset user edits on schema refresh` | `not_ignored` |
| `DISC_4B5520D0EAD036C12924` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `advanced refactor commands use self-explanatory Structured Text wording` | `not_ignored` |
| `DISC_AA27235D935E99C46EEF` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `attached runtimes are labelled Connected, not Stopped or Running` | `not_ignored` |
| `DISC_1218EDE8B08DB51CDB21` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `browse add action disables honestly when there is nothing valid to add` | `not_ignored` |
| `DISC_EFE59CAFDB19DDE5B6A7` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `browse tree shows plain access labels, not protocol shorthand` | `not_ignored` |
| `DISC_D32AF44E2F688CC1BCF4` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `canvas grid backgrounds use the shared truST product grid role` | `not_ignored` |
| `DISC_A535CDFD8E267DCBE21C` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `curated libraries are packaged and catalog remains gated` | `not_ignored` |
| `DISC_7E4E79CE02AFFEBC1F8F` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `curated library updates compare the vendored project copy` | `not_ignored` |
| `DISC_A405ED7CCDAECFA5FBB9` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `dead execution panels with embedded runtime controls are removed` | `not_ignored` |
| `DISC_2A1330A8730F61208513` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `debug journey fixture has a native truST Simulator launch configuration` | `not_ignored` |
| `DISC_2EC86AB9B1842161F6A7` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `debug reload LM tool reports command failure honestly` | `not_ignored` |
| `DISC_B062FB2ABFF3C2880279` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `development binary resolver honors CARGO_TARGET_DIR` | `not_ignored` |
| `DISC_8012662EF660269EC7B7` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `empty runtime guidance points to + Add, not hidden Edit mode` | `not_ignored` |
| `DISC_677A0CEBB67E415C009F` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `endpoint disable is available from the inspector and writes through offline comm apply` | `not_ignored` |
| `DISC_989102A31323208EA82A` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `endpoint edit drafts are not reset by identical topology refreshes` | `not_ignored` |
| `DISC_D9F3923D3BEE2A889908` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `endpoint edit pane uses task-name breadcrumbs instead of role badges` | `not_ignored` |
| `DISC_6F1CD5D3B0B95E366AFB` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `endpoint removal is a deliberate two-step action` | `not_ignored` |
| `DISC_B15B22A3E04434943E0E` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `every bundled example has a native truST Simulator debug configuration` | `not_ignored` |
| `DISC_ECD5150419B33360CBB4` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `every bundled example hides the native debug status selector` | `not_ignored` |
| `DISC_C7A8E28864614D7C216E` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `every example folder is a runnable scaffold (the 4 files), bundled in media/` | `not_ignored` |
| `DISC_6EBBB53BA6BB406DE861` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `every example instantiates its program in a configuration` | `not_ignored` |
| `DISC_CACC83FBC8843807678C` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `every example runtime.toml has the sections the runtime parser requires` | `not_ignored` |
| `DISC_C1F6A5CA75A68E6FB440` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `example copy keeps native prompts and exposes only an acceptance-runner prompt override` | `not_ignored` |
| `DISC_83D0D709424CBE3BE810` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `example gallery entries carry hardware badges` | `not_ignored` |
| `DISC_37AE3A54B5BA095F2E87` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `example gallery has scalable search plus hardware and tag filters` | `not_ignored` |
| `DISC_D9CD3462B2F8FEF93512` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `example gallery separates hardware requirements from category labels` | `not_ignored` |
| `DISC_B2998A24536C6D0B231A` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `forced values are always visibly marked` | `not_ignored` |
| `DISC_26182A905C6D80B36D86` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `hardware badges map to the user-facing requirement labels` | `not_ignored` |
| `DISC_C5C0E5DDF023301B75C5` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `hidden commands are still registered where the surface contract keeps them` | `not_ignored` |
| `DISC_8094953FE41DF049FF7A` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `host runtime setup slot uses the self-explanatory setup wording` | `not_ignored` |
| `DISC_2101306264BBCFCDE99A` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `invalid visual model cards can escape to the text editor` | `not_ignored` |
| `DISC_92FDC4AD270E7E98AEC1` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `journey batch strips raw helper PNG output before validation` | `not_ignored` |
| `DISC_18984960E4BB47CD32DF` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `ladder contacts and coils show symbols with addresses using neutral edit strokes` | `not_ignored` |
| `DISC_22B456C39DA59CE867AA` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `legacy plaintext token setting is not contributed to native Settings` | `not_ignored` |
| `DISC_AE93A961291079A49612` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `library row actions use user-facing verbs and versioned updates` | `not_ignored` |
| `DISC_DC22B1FA9EA08AEBC83D` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `library symbol browser supports search, pagination, detail, and insertion` | `not_ignored` |
| `DISC_E53966F187411498430E` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `library symbol counts use real singular/plural copy` | `not_ignored` |
| `DISC_689F5DBBBF50BEDD7DDF` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `managed local runtimes are projected into the sidebar Target from the fleet lifecycle` | `not_ignored` |
| `DISC_43D7E06FF9E223A39019` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `managed runtime tokens are imported into SecretStorage before attach` | `not_ignored` |
| `DISC_B0BBF55ACFD8D4584F7B` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `native Settings contribution uses product-language setting keys and titles` | `not_ignored` |
| `DISC_EC6F220B77136927D8EB` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `native Testing view explains empty Structured Text test workspaces` | `not_ignored` |
| `DISC_4BB02C90C4EE44B1C690` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `network-canvas notifications do not expose backend protocol ids or awkward plurals` | `not_ignored` |
| `DISC_BD7DC3C02B602DC946DC` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `new project scaffolding writes the same native debug configuration` | `not_ignored` |
| `DISC_F96A9E4CAC0597A1C257` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `no palette-visible command embeds a 'Structured Text:' category prefix (one truST category)` | `not_ignored` |
| `DISC_B09E66A7603AD641BC97` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `no secrets in example fixtures (no tokens/passwords/keys)` | `not_ignored` |
| `DISC_FD3D4101D2258C0B8A52` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `no user-facing 'Network Canvas' anywhere it renders or reaches the user (bundle + runtime strings)` | `not_ignored` |
| `DISC_517AEA6DB7A3DABFE2A0` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `node inspector maps raw health ids to user-facing labels` | `not_ignored` |
| `DISC_3B6E97361EE4CE45A600` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `non-simulator force is explicitly armed before pinning a value` | `not_ignored` |
| `DISC_44D2538AAF201C6ABBD7` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `package contribution labels and descriptions use current product names` | `not_ignored` |
| `DISC_9E9BE0E24424346433D7` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `pickAuthToken: SecretStorage value wins; empty falls back to the legacy setting` | `not_ignored` |
| `DISC_1F6DC0B37EC99D2621B1` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `primary buttons use VS Code button tokens, not the focus/accent token as fill` | `not_ignored` |
| `DISC_124D39018265FC8C72A1` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `product webviews share the same truST theme source` | `not_ignored` |
| `DISC_77D94BB2145BDAB61F63` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `project switching is not hidden in the sidebar project name` | `not_ignored` |
| `DISC_544B7227A0C6ADF8EF4D` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `project-open sidebar renders the project name as an identity row` | `not_ignored` |
| `DISC_C3E8477FA51D63AFAF6D` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `protocol add/edit forms use the shared product chrome baseline` | `not_ignored` |
| `DISC_65E00F21757091DED808` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `refresh does not post through a disposed canvas panel` | `not_ignored` |
| `DISC_9D1B82946A31A0568E7B` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `release VSIX bundles trust-runtime beside trust-lsp and trust-debug` | `not_ignored` |
| `DISC_0A69A8BA0C0B065B5B5B` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `remote browse uses one configured client connection for ADS and OPC UA` | `not_ignored` |
| `DISC_96920E91D7B452C0363C` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `retired 3D twin surface is absent from the VS Code product surface` | `not_ignored` |
| `DISC_0D2FF2D7B7045244EE02` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `retired Communication and ADS panel commands are removed, not hidden escapes` | `not_ignored` |
| `DISC_0178F83758787AA886F8` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `row write force and release wait for the next runtime scan before refreshing rows` | `not_ignored` |
| `DISC_51F94583AFF2825BD4C9` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `runtime setup task panes keep the Devices & Connections breadcrumb` | `not_ignored` |
| `DISC_7696768494AAED23644C` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `schema json_array fields render as list editors, not raw one-line JSON` | `not_ignored` |
| `DISC_C84D96C8B18867D8C1D2` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `selected run target persists across VS Code restart with a workspace-scoped fallback` | `not_ignored` |
| `DISC_11BBA7D6EDF97286D62C` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `server endpoint summaries hide advanced transport limits by default` | `not_ignored` |
| `DISC_726ECCA435FACD53FBD5` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `server-expose examples drive the exposed global from ST, not a static initializer` | `not_ignored` |
| `DISC_65398089C491D6C7C77C` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `shared truST theme has an explicit high-contrast token contract` | `not_ignored` |
| `DISC_27A040B54E74EAC76068` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `sidebar four-button state table is explicit and has one primary source of truth` | `not_ignored` |
| `DISC_B9F5C7DD88E5D780B519` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `starter descriptions are compact enough for gallery cards` | `not_ignored` |
| `DISC_E476A7F8409929D5BA63` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `starting a new canvas drawer clears stale apply errors` | `not_ignored` |
| `DISC_CCD44C25F2A5A6440C16` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `stopped/no-session state is beginner-facing and clears stale values` | `not_ignored` |
| `DISC_26FF043877487F9DA5D0` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `successful add-device Save lands on the saved node without clearing the result` | `not_ignored` |
| `DISC_357ACDDC7DC09038D940` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `summarizeCheck: passed vs failed wording` | `not_ignored` |
| `DISC_D8305F273D4AEF2589BE` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `the Communication panel is no longer user-facing` | `not_ignored` |
| `DISC_5CC127E996CB1A2C5F95` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `the PLCopen Motion starter is portable outside the repository` | `not_ignored` |
| `DISC_F1D9C5F10D9EB5A93FF4` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `the canvas panel is user-facing 'Devices & Connections', never 'Network Canvas'` | `not_ignored` |
| `DISC_968BD401D87061A5B98C` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `the graph is user-facing 'Devices & Connections', never 'Network Canvas'` | `not_ignored` |
| `DISC_6C8FEAE756AA8F3CEE0F` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `the leaked core commands are hidden from the command palette (when:false)` | `not_ignored` |
| `DISC_C93DC351370AA439EC39` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `the manifest parses and ships the curated starters` | `not_ignored` |
| `DISC_412E98E79F2C9FC69F86` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `the values surface is named 'Live Values' (not 'Structured Text Runtime')` | `not_ignored` |
| `DISC_5EE4B5D37FF0D0DF4829` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `token read paths use the SecretStorage-backed store, not the raw plaintext setting` | `not_ignored` |
| `DISC_12F56A99CEF1A0DB8F1B` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `truST sidebar title exposes only Settings as a visible icon; New diagram stays in overflow` | `not_ignored` |
| `DISC_3D8C0CA2CDE5BCAC0219` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `viewer Live Values permissions disable Write/Force before a backend rejection` | `not_ignored` |
| `DISC_5454DCB596DED437F8EC` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `visual editor parse errors use user-facing recovery language` | `not_ignored` |
| `DISC_BCC9D903DBF6BF0A2C0C` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `visual editor right panes share Tools Edit View IA and one zoom placement` | `not_ignored` |
| `DISC_0D5863DB6B096AD16181` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `visual editor right panes use the shared product chrome, not private sidebars` | `not_ignored` |
| `DISC_500F30ABF3541F712088` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `visual editors do not render the legacy embedded runtime/I/O panel` | `not_ignored` |
| `DISC_7B3A5FA6D3614DD17271` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `visual editors reserve dashed strokes for product draft semantics` | `not_ignored` |
| `DISC_C3C68094BCF6582D3738` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `visual-editor chrome does not add private hardcoded colours` | `not_ignored` |
| `DISC_1FF72C914C287951D91F` | `vscode_test` | `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | `write / force / release are preserved (NOT read-only)` | `not_ignored` |
| `DISC_9F302838F3CF3EEE47EE` | `vscode_test` | `editors/vscode/src/test/suite/visual-companion.test.ts` | `all visual examples generate companions and runtime wrappers` | `not_ignored` |
| `DISC_19C217D346649BE4C47D` | `vscode_test` | `editors/vscode/src/test/suite/visual-companion.test.ts` | `declares global and local ladder symbols in companion function block` | `not_ignored` |
| `DISC_4E5329C3A9F790B6471F` | `vscode_test` | `editors/vscode/src/test/suite/visual-companion.test.ts` | `generates blockly companion as function block` | `not_ignored` |
| `DISC_20A999057132166C1A75` | `vscode_test` | `editors/vscode/src/test/suite/visual-companion.test.ts` | `generates ladder companion as function block` | `not_ignored` |
| `DISC_BC3D240B3614CB5281E6` | `vscode_test` | `editors/vscode/src/test/suite/visual-companion.test.ts` | `generates runtime entry wrapper with configuration and program binding` | `not_ignored` |
| `DISC_F543197BFE4210C953F7` | `vscode_test` | `editors/vscode/src/test/suite/visual-companion.test.ts` | `generates sfc companion as function block` | `not_ignored` |
| `DISC_2B6C1E4E9766D2EBD0D5` | `vscode_test` | `editors/vscode/src/test/suite/visual-companion.test.ts` | `generates statechart companion with event inputs and actions` | `not_ignored` |
| `DISC_5CFFFDD334E4916B71A1` | `vscode_test` | `editors/vscode/src/test/suite/visual-companion.test.ts` | `lowers branch topology so outputs change state for dfg parallel path` | `not_ignored` |
| `DISC_4CA2BF9F7DE91978131A` | `vscode_test` | `editors/vscode/src/test/suite/visual-companion.test.ts` | `maps ladder globals with addresses into runtime entry wrapper` | `not_ignored` |
| `DISC_103E90CD1063982834A7` | `vscode_test` | `editors/vscode/src/test/suite/visual-companion.test.ts` | `maps raw direct ladder operands into runtime entry wrapper globals` | `not_ignored` |
| `DISC_9174B324BB02C1D5F490` | `vscode_test` | `editors/vscode/src/test/suite/visual-companion.test.ts` | `maps visual source files to sibling .st companions` | `not_ignored` |
| `DISC_D5DAB339E3830DD3EBC8` | `vscode_test` | `editors/vscode/src/test/suite/visual-companion.test.ts` | `rejects undeclared ladder symbols in companion function block` | `not_ignored` |
| `DISC_B85DED686EF3F9E9F515` | `vscode_test` | `editors/vscode/src/test/suite/visual-right-pane-resize.test.ts` | `builds stable storage keys` | `not_ignored` |
| `DISC_82CF328D80AD2A929BBC` | `vscode_test` | `editors/vscode/src/test/suite/visual-right-pane-resize.test.ts` | `clamps widths to configured bounds` | `not_ignored` |
| `DISC_F646072A82984A6F257F` | `vscode_test` | `editors/vscode/src/test/suite/visual-right-pane-resize.test.ts` | `falls back to default width for invalid persisted values` | `not_ignored` |
| `DISC_2F081E0A74BD342A92E8` | `vscode_test` | `editors/vscode/src/test/suite/visual-right-pane-resize.test.ts` | `falls back to local storage when VS Code state is missing` | `not_ignored` |
| `DISC_96EEA4612336ED1EE917` | `vscode_test` | `editors/vscode/src/test/suite/visual-right-pane-resize.test.ts` | `prefers VS Code webview state over local storage` | `not_ignored` |
| `DISC_685F7123513B04BDBFA8` | `vscode_test` | `editors/vscode/src/test/suite/visual-runtime-controller.test.ts` | `captures start failures in runtime state` | `not_ignored` |
| `DISC_B4F300EA5F20BEE609A6` | `vscode_test` | `editors/vscode/src/test/suite/visual-runtime-controller.test.ts` | `routes runtime panel and settings actions` | `not_ignored` |
| `DISC_9E7C80F4B686C2EB5FC1` | `vscode_test` | `editors/vscode/src/test/suite/visual-runtime-controller.test.ts` | `runtime message schema guard accepts shared payloads` | `not_ignored` |
| `DISC_ABEF46C2CD647AA55AF7` | `vscode_test` | `editors/vscode/src/test/suite/visual-runtime-controller.test.ts` | `tracks mode/start/stop transitions` | `not_ignored` |
| `DISC_D29DC78389C9E7C836E9` | `vscode_test` | `editors/vscode/src/test/suite/visual-runtime-panel-bridge.test.ts` | `derives runtime status payload from shared runtime ui state` | `not_ignored` |
| `DISC_07486C548E10932C2E39` | `vscode_test` | `editors/vscode/src/test/suite/visual-runtime-panel-bridge.test.ts` | `maps runtime mode consistently` | `not_ignored` |
| `DISC_EE7C152DD89753ED4963` | `vscode_test` | `editors/vscode/src/test/suite/visual-runtime-panel-bridge.test.ts` | `validates shared runtime panel message schema` | `not_ignored` |
| `DISC_A301AFA625E6785DCC4F` | `vscode_test` | `editors/vscode/src/test/suite/visual-webview-vscode-api.test.ts` | `acquires VS Code API only once per webview runtime` | `not_ignored` |
| `DISC_AA4B8BE45C466AA24CB8` | `vscode_test` | `editors/vscode/src/test/suite/visual-webview-vscode-api.test.ts` | `returns a stable no-op API when VS Code API is unavailable` | `not_ignored` |

## Limitations

- The raw non-catalog-mapped list is the exact subtraction of reviewed generated_test discovery IDs from current scanner facts.
- Unresolved debt is zero only when the committed denominator gives every raw non-catalog-mapped fact an exact reviewed-nonmapping disposition.
- Case-table and mutation-runner artifacts never classify scanner facts.
- Ignored and conditionally ignored scanner facts remain visible and bind the Phase 3 ignored-test registry.
- A reviewed nonmapping retires mapping debt without deleting the native test or claiming that its behavior is adequate.
- No area, class, invariant, oracle, or expected behavior is inferred from a name or path.
- Scanner exclusions remain those documented by the generated existing-test catalog.
- Reviewed nonmapping does not fail this report-only command or change CI enforcement; an unreviewed fact fails generation.
- Platform is historical provenance requiring evidence review; at-rest validation cannot rederive a prior host.
