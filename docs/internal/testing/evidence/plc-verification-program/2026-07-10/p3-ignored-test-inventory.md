# Ignored-Test Inventory

Generator: `ignored-test-inventory v1`
Source revision: `9b746400755d6186acbcef217d78749a388cf7a4`
Generated: `2026-07-17T12:19:00+02:00`
Platform: `linux-x86_64`
Generated JSON SHA-256: `269850f9d341161da6c48100defe9c8ec35d0cbadef40b20d67f111aee14f0f5`
Input SHA-256: `sha256:cd18fa167ecf8c2be35b3cab8bb48cf37c1833d6acfcd6585b8f9fc7b3e5b44d`

This report is a mechanical inventory. It does not classify an ignored test,
establish expected behavior, or count as product proof.

## Summary

- Records: 29
- Statically ignored: 27
- Conditional ignore observations: 2
- Diagnostics: 0
- Errors: 0
- Warnings: 0

| Source kind | Records |
| --- | ---: |
| `playwright_test` | 1 |
| `rust_integration_test` | 15 |
| `rust_unit_test` | 12 |
| `vscode_test` | 1 |

## Surface Coverage

| Surface | Scanned files | Records | Ignored | Conditional | Coverage |
| --- | ---: | ---: | ---: | ---: | --- |
| `conformance` | 21 | 0 | 0 | 0 | `limitation` |
| `node` | 48 | 1 | 0 | 1 | `mechanical` |
| `playwright` | 7 | 1 | 1 | 0 | `mechanical` |
| `rust` | 583 | 27 | 26 | 1 | `mechanical` |
| `shell` | 29 | 0 | 0 | 0 | `limitation` |

Surface notes:

- `conformance`: Runtime skipped results are not source ignore declarations.
- `node`: Modeled VS Code facts and fail-closed excluded Node sentinel files are included in the scanned-file count.
- `playwright`: Only same-line literal test.skip calls in tracked capture specs are inventory facts.
- `rust`: Modeled crate Rust facts and fail-closed xtask/fuzz sentinel files are included in the scanned-file count.
- `shell`: Shell source has no repository-wide static ignored-test identity convention.

## Inventory

| Discovery ID | State | Mechanism | Source | Path | Name | Reason |
| --- | --- | --- | --- | --- | --- | --- |
| `DISC_E45EDC8D2860AAECF144` | `ignored` | `playwright_literal_skip` | `playwright_test` | `scripts/captures/vscode/runtime-panel-command.spec.mjs:5` | `capture code-server runtime panel command palette` | literal test.skip declaration |
| `DISC_FE262D7963202D5AFB9D` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/device_in_the_loop.rs:177` | `ads_lab_twincat_doctor_records_status_json` | requires configured lab TwinCAT/ADS target; see docs/internal/testing/runtime-device-in-the-loop.md |
| `DISC_26934332D29338103AFD` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/device_in_the_loop.rs:16` | `ethercat_lab_hardware_discovery_records_topology` | requires configured lab EtherCAT hardware; see docs/internal/testing/runtime-device-in-the-loop.md |
| `DISC_A007AD886E7ABB17EF37` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/device_in_the_loop.rs:77` | `ethercat_lab_pdu_storage_stress_records_artifact` | requires explicit lab EtherCAT storage stress opt-in; see docs/internal/testing/runtime-device-in-the-loop.md |
| `DISC_EC41FC62FBEF09EFDFE8` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/device_in_the_loop.rs:273` | `modbus_lab_target_confirms_protocol_probe` | requires configured lab Modbus target; see docs/internal/testing/runtime-device-in-the-loop.md |
| `DISC_75F421CF89F3935418D6` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/device_in_the_loop.rs:366` | `mqtt_lab_broker_records_auth_tls_reconnect_and_disconnect` | requires configured lab MQTT broker; see docs/internal/testing/runtime-device-in-the-loop.md |
| `DISC_B9ACE9B650FE5BFE0DC6` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/ethercat_driver.rs:245` | `ethercat_missing_adapter_post_allocation_failure_is_terminal_until_rebuild` | red test for runtime-safety EtherCAT bounded post-allocation retry policy |
| `DISC_083918C0A1695BEDB92E` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/ethercat_driver.rs:170` | `ethercat_missing_adapter_records_pdu_storage_retry_baseline` | runtime-safety EtherCAT PduStorage baseline; explicitly run for storage evidence |
| `DISC_FE38938F6FB54A1B26E7` | `conditional` | `rust_cfg_attr` | `rust_integration_test` | `crates/trust-runtime/tests/io_multidriver_live.rs:410` | `runtime_composes_modbus_and_mqtt_drivers_live` | live MQTT broker handshake is network-timing flaky on non-Linux CI runners; \ run with `--ignored` locally to exercise. Linux runners are the source of truth \ for the driver-composition contract. |
| `DISC_7F7F305082D2D125BF56` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/openot_capstone.rs:98` | `openot_capstone_consumer_process` | spawned by openot_capstone_fenced_cross_process |
| `DISC_2EEB083ACDA59C7173DC` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/openot_capstone.rs:89` | `openot_capstone_producer_process` | spawned by openot_capstone_fenced_cross_process |
| `DISC_076A01316BE505A43D6C` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/openot_capstone.rs:59` | `openot_capstone_unfenced_contrast` | diagnostic unfenced experiment; set OPENOT_CAPSTONE_RUN_UNFENCED=1 |
| `DISC_B0E363553D57310C9D63` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/oscat_oop_examples.rs:583` | `oscat_oop_example_st_unit_tests_pass` | expensive OSCAT gate runs all 98 paired example projects through trust-runtime CLI |
| `DISC_BDEAFF431FC2B9A44DFD` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/phase10_performance.rs:105` | `phase10_ads_opcua_publish_clone_partial_baseline` | Phase 10 benchmark; run explicitly with --ignored --nocapture |
| `DISC_8A83ABE26CDE3A14E938` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/phase10_performance.rs:44` | `phase10_debug_snapshot_overhead_baseline` | Phase 10 benchmark; run explicitly with --ignored --nocapture |
| `DISC_9DA8F1E2D659F4E0F5E7` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/phase10_performance.rs:76` | `phase10_retain_fsync_impact_baseline` | Phase 10 benchmark; run explicitly with --ignored --nocapture |
| `DISC_6F5F5107DDE61D3C1E43` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-lsp/src/perf.rs:456` | `perf_completion_budget` | ignore |
| `DISC_A5BFD4D2FEC4A5FE7C73` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-lsp/src/perf.rs:759` | `perf_diagnostics_budget` | ignore |
| `DISC_F7D08DE71C821B2157CF` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-lsp/src/perf.rs:668` | `perf_document_highlight_scaling_budget` | ignore |
| `DISC_08CEAF52EF3509019BFF` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-lsp/src/perf.rs:813` | `perf_edit_loop_budget` | ignore |
| `DISC_B65B577CE97DDD0DC665` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-lsp/src/perf.rs:416` | `perf_hover_budget` | ignore |
| `DISC_D5AB69F6A0EBB17DA479` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-lsp/src/perf.rs:535` | `perf_large_workspace_index_budget` | ignore |
| `DISC_0D7E1103BA7F27B6CD0E` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-lsp/src/perf.rs:500` | `perf_rename_budget` | ignore |
| `DISC_E0C98D6962CBF3D5E78F` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-lsp/src/perf.rs:614` | `perf_semantic_tokens_scaling_budget` | ignore |
| `DISC_F4D12F76BE0C9EC8F0EF` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-lsp/src/perf.rs:718` | `perf_workspace_navigation_scaling_budget` | ignore |
| `DISC_CE3D5672C7ABFFB9A685` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-lsp/src/perf.rs:572` | `perf_workspace_symbol_budget` | ignore |
| `DISC_5BBB12B235856C1C45CC` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-runtime/src/openot_authoring/tests.rs:212` | `compile_session_surfaces_openot_validation_failure_instead_of_building_uninstrumented_bytecode` | red test for runtime-safety Phase 11 SEAM-TEST-015 |
| `DISC_1466DAFE96EF828CB3AF` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-runtime/src/openot_authoring/tests.rs:132` | `hir_and_runtime_authoring_report_explicit_sourceid_collisions_consistently` | red test for runtime-safety Phase 11 SEAM-TEST-014 |
| `DISC_78213ECD5BFD1CD1ACE3` | `conditional` | `vscode_runtime_skip` | `vscode_test` | `editors/vscode/src/test/suite/new-project.test.ts:339` | `generated ST parses cleanly and TOML is usable by build` | runtime this.skip() cannot be represented as a declared ignore attribute |

## Limitations

- Discovery is static and recognizes only the ignore mechanisms named by this report.
- Runtime this.skip() is accepted only when contained in exactly one literal VS Code test callback.
- Rust files under xtask, root fuzz, and crate-local fuzz are bound by a fail-closed ignore-marker sentinel until identity support is added.
- Modeled Node identities are limited to VS Code Mocha and tracked Playwright capture specs; other tracked test/spec files use a fail-closed skip sentinel.
- Shell source has no repository-wide static ignored-test identity convention.
- Conformance runtime skipped results are outcomes, not source ignore declarations.
- Ignore classes, owners, areas, unblock conditions, expected behavior, and proof are hand-owned metadata.
