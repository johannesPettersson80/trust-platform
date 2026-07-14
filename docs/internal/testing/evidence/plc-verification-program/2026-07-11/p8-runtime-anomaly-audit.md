# Phase 8 Runtime Anomaly Audit

Generator: `runtime-anomaly-audit v1`
Source revision: `c935e82b209b8dabab17f17e398b4dc5fc5ab5b6`
Generated: `2026-07-14T14:35:59+02:00`
Platform: `linux-x86_64`
Generated JSON SHA-256: `a5669fb97aca4c6598406651c24e13401165d0342ad1a5493458bf045a3cb75c`
Input SHA-256: `sha256:a2e859e727e24f78e8b676128c81d927dcc85de1cdb13d8a4d4ed854754f4424`

This is a report-only audit of the reviewed runtime-anomaly taxonomy,
explicit existing-test associations, open test gaps, and planned suite tiers.
It executes no fault and creates no proof or invariant coverage.

## Summary

- Taxonomy classes: 19
- Explicit mapping records: 38
- Live Rust scanner facts: 3091
- Effectively runnable direct mappings: 28
- Ignored or conditional mappings: 1
- Gap classes: 9

## Classes

| Class | State | Primary suite | Conditional suites | Runnable mappings | Other mappings |
| --- | --- | --- | --- | --- | --- |
| `panic` | `mapped_runnable` | `pr` | `nightly`, `release` | `ANOM_MAP_PANIC_001`, `ANOM_MAP_PANIC_002`, `ANOM_MAP_PANIC_003`, `ANOM_MAP_PANIC_004` | none |
| `timeout` | `mapped_runnable` | `pr` | `nightly` | `ANOM_MAP_TIMEOUT_001`, `ANOM_MAP_TIMEOUT_002` | `ANOM_MAP_TIMEOUT_003` |
| `deadline` | `mapped_runnable` | `pr` | `nightly`, `release` | `ANOM_MAP_DEADLINE_001`, `ANOM_MAP_DEADLINE_002`, `ANOM_MAP_DEADLINE_003` | none |
| `watchdog` | `mapped_runnable` | `pr` | `nightly`, `release` | `ANOM_MAP_WATCHDOG_001`, `ANOM_MAP_WATCHDOG_002`, `ANOM_MAP_WATCHDOG_003` | none |
| `slow_device` | `mapped_runnable` | `nightly` | `release`, `hardware_lab` | `ANOM_MAP_SLOW_DEVICE_001`, `ANOM_MAP_SLOW_DEVICE_002` | `ANOM_MAP_SLOW_DEVICE_003` |
| `disconnect` | `mapped_runnable` | `nightly` | `release`, `hardware_lab` | `ANOM_MAP_DISCONNECT_001`, `ANOM_MAP_DISCONNECT_002` | `ANOM_MAP_DISCONNECT_003` |
| `queue_full` | `mapped_non_runnable_or_partial` | `nightly` | `release` | none | `ANOM_MAP_QUEUE_FULL_001`, `ANOM_MAP_QUEUE_FULL_002` |
| `stale_data` | `mapped_runnable` | `pr` | `nightly`, `hardware_lab` | `ANOM_MAP_STALE_DATA_001`, `ANOM_MAP_STALE_DATA_002`, `ANOM_MAP_STALE_DATA_003` | none |
| `corrupt_retain` | `mapped_runnable` | `pr` | `release` | `ANOM_MAP_CORRUPT_RETAIN_001`, `ANOM_MAP_CORRUPT_RETAIN_002` | none |
| `malformed_bytecode` | `mapped_runnable` | `pr` | `nightly` | `ANOM_MAP_MALFORMED_BYTECODE_001`, `ANOM_MAP_MALFORMED_BYTECODE_002`, `ANOM_MAP_MALFORMED_BYTECODE_003` | none |
| `bad_config` | `mapped_runnable` | `pr` | `release` | `ANOM_MAP_BAD_CONFIG_001`, `ANOM_MAP_BAD_CONFIG_002`, `ANOM_MAP_BAD_CONFIG_003`, `ANOM_MAP_BAD_CONFIG_004` | none |
| `bad_signal` | `unmapped` | `release` | `nightly` | none | none |
| `partial_web_request` | `mapped_non_runnable_or_partial` | `nightly` | `release` | none | `ANOM_MAP_PARTIAL_WEB_REQUEST_001` |
| `disk_error` | `mapped_non_runnable_or_partial` | `release` | `nightly` | none | `ANOM_MAP_DISK_ERROR_001`, `ANOM_MAP_DISK_ERROR_002` |
| `clock_step` | `mapped_non_runnable_or_partial` | `nightly` | `release` | none | `ANOM_MAP_CLOCK_STEP_001`, `ANOM_MAP_CLOCK_STEP_002` |
| `monotonic_wall_clock_divergence` | `unmapped` | `nightly` | `release` | none | none |
| `suspend_resume` | `unmapped` | `nightly` | `release` | none | none |
| `timer_duration_overflow` | `unmapped` | `pr` | `release` | none | none |
| `allocation_failure_oom` | `unmapped` | `nightly` | `release` | none | none |

## Explicit Associations

| Mapping | Class | Test | Kind | Ignore state | Runnable | Mechanism |
| --- | --- | --- | --- | --- | --- | --- |
| `ANOM_MAP_BAD_CONFIG_001` | `bad_config` | `DISC_78139822FEDCB8E1A379` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_BAD_CONFIG_002` | `bad_config` | `DISC_C9AB817C3804CDAD2BAF` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_BAD_CONFIG_003` | `bad_config` | `DISC_7CCA5C983F3BC05339EC` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_BAD_CONFIG_004` | `bad_config` | `DISC_86A13C88CFA096648AFB` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_CLOCK_STEP_001` | `clock_step` | `DISC_1F4A6A7E004592367309` | `partial` | `not_ignored` | `false` | `ordinary_input` |
| `ANOM_MAP_CLOCK_STEP_002` | `clock_step` | `DISC_2CC9B67E44CB8284B149` | `context_only` | `not_ignored` | `false` | `test_harness` |
| `ANOM_MAP_CORRUPT_RETAIN_001` | `corrupt_retain` | `DISC_6536BD14A5FAEA351CCF` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_CORRUPT_RETAIN_002` | `corrupt_retain` | `DISC_D57F0AA703E76450F141` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_DEADLINE_001` | `deadline` | `DISC_55D5957DD6826E1A6C0F` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_DEADLINE_002` | `deadline` | `DISC_E35FF38A598A52B9A748` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_DEADLINE_003` | `deadline` | `DISC_4C3F5D7171073A654EA8` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_DISCONNECT_001` | `disconnect` | `DISC_D0022CD29DB5061F146B` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_DISCONNECT_002` | `disconnect` | `DISC_0964F1D88F24D71AD970` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_DISCONNECT_003` | `disconnect` | `DISC_C3A91E2860C30B08BED4` | `partial` | `not_ignored` | `false` | `test_harness` |
| `ANOM_MAP_DISK_ERROR_001` | `disk_error` | `DISC_CAF0750D558B8114BEC6` | `partial` | `not_ignored` | `false` | `test_harness` |
| `ANOM_MAP_DISK_ERROR_002` | `disk_error` | `DISC_B25F48AA676D50A29BB2` | `partial` | `not_ignored` | `false` | `test_harness` |
| `ANOM_MAP_MALFORMED_BYTECODE_001` | `malformed_bytecode` | `DISC_F90D4502D7B68E02847C` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_MALFORMED_BYTECODE_002` | `malformed_bytecode` | `DISC_FB4371C17A9F9FB83CA9` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_MALFORMED_BYTECODE_003` | `malformed_bytecode` | `DISC_51482BB47DB5575280CE` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_PANIC_001` | `panic` | `DISC_30C382889325B64C5854` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_PANIC_002` | `panic` | `DISC_E6963439801BFF301755` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_PANIC_003` | `panic` | `DISC_96A8793BF2C3E6F579B4` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_PANIC_004` | `panic` | `DISC_EEF14FDB800A7590C8F3` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_PARTIAL_WEB_REQUEST_001` | `partial_web_request` | `DISC_20DF277B08348ED8A798` | `context_only` | `ignored` | `false` | `external_harness` |
| `ANOM_MAP_QUEUE_FULL_001` | `queue_full` | `DISC_398610728B7609607B57` | `partial` | `not_ignored` | `false` | `external_harness` |
| `ANOM_MAP_QUEUE_FULL_002` | `queue_full` | `DISC_36444246A468C534110A` | `partial` | `not_ignored` | `false` | `test_harness` |
| `ANOM_MAP_SLOW_DEVICE_001` | `slow_device` | `DISC_7159C3BA77CC33C8F48C` | `direct` | `not_ignored` | `true` | `external_harness` |
| `ANOM_MAP_SLOW_DEVICE_002` | `slow_device` | `DISC_6C10B5D7ACB21ECE8E9D` | `direct` | `not_ignored` | `true` | `external_harness` |
| `ANOM_MAP_SLOW_DEVICE_003` | `slow_device` | `DISC_D2A5AD5E1D2E5D50D9E4` | `context_only` | `not_ignored` | `false` | `test_harness` |
| `ANOM_MAP_STALE_DATA_001` | `stale_data` | `DISC_B3EBD5115E6DBD66F946` | `direct` | `not_ignored` | `true` | `external_harness` |
| `ANOM_MAP_STALE_DATA_002` | `stale_data` | `DISC_ADB41A6325C30D12D0AD` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_STALE_DATA_003` | `stale_data` | `DISC_5E088D53395CC4F852E5` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_TIMEOUT_001` | `timeout` | `DISC_0BD76C389DCA0387316F` | `direct` | `not_ignored` | `true` | `external_harness` |
| `ANOM_MAP_TIMEOUT_002` | `timeout` | `DISC_AD782C3AD1D808D4C4EE` | `direct` | `not_ignored` | `true` | `external_harness` |
| `ANOM_MAP_TIMEOUT_003` | `timeout` | `DISC_17A3C97ED5EF6F1D342D` | `partial` | `not_ignored` | `false` | `test_harness` |
| `ANOM_MAP_WATCHDOG_001` | `watchdog` | `DISC_C101BBAEC06311987C74` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_WATCHDOG_002` | `watchdog` | `DISC_3EFD5D5E736CB0C47DD6` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_WATCHDOG_003` | `watchdog` | `DISC_8BAF8461F95A94A773A6` | `direct` | `not_ignored` | `true` | `test_harness` |

## Test Gaps

| Class | State | Reason | Planned suite | Associations |
| --- | --- | --- | --- | --- |
| `queue_full` | `mapped_non_runnable_or_partial` | `no_effectively_runnable_direct_mapping` | `nightly` | `ANOM_MAP_QUEUE_FULL_001`, `ANOM_MAP_QUEUE_FULL_002` |
| `bad_signal` | `unmapped` | `no_explicit_mapping` | `release` | none |
| `partial_web_request` | `mapped_non_runnable_or_partial` | `no_effectively_runnable_direct_mapping` | `nightly` | `ANOM_MAP_PARTIAL_WEB_REQUEST_001` |
| `disk_error` | `mapped_non_runnable_or_partial` | `no_effectively_runnable_direct_mapping` | `release` | `ANOM_MAP_DISK_ERROR_001`, `ANOM_MAP_DISK_ERROR_002` |
| `clock_step` | `mapped_non_runnable_or_partial` | `no_effectively_runnable_direct_mapping` | `nightly` | `ANOM_MAP_CLOCK_STEP_001`, `ANOM_MAP_CLOCK_STEP_002` |
| `monotonic_wall_clock_divergence` | `unmapped` | `no_explicit_mapping` | `nightly` | none |
| `suspend_resume` | `unmapped` | `no_explicit_mapping` | `nightly` | none |
| `timer_duration_overflow` | `unmapped` | `no_explicit_mapping` | `pr` | none |
| `allocation_failure_oom` | `unmapped` | `no_explicit_mapping` | `nightly` | none |

## Spec-Gap Review

- Scan-cycle allocation policy: `written_contract_present` via `SPEC_RUNTIME_ENGINE_001` (`docs/specs/11-runtime-engine.md`).
- Restart time base: `resolved_source` via `SPEC_IEC_STANDARD_FBS_CANDIDATE_001` (`docs/specs/08-standard-function-blocks.md`), superseding `SPEC_GAP_IEC_TIMER_RESTART_TIMEBASE_001`.

## Planned Tier Counts

- `pr`: 9
- `nightly`: 8
- `release`: 2
- `hardware_lab`: 0

## Boundaries

- `report_creates_proof`: `false`
- `report_creates_invariant_coverage`: `false`
- `report_closes_spec_gaps`: `false`
- `semantic_oracles_assessed`: `false`
- `faults_executed`: `false`
- `fault_interfaces_implemented`: `false`
- `production_fault_hooks_added`: `false`
- `p8_002_exhaustive_mapping_row_remains_open`: `true`
- `p8_005_fault_toggle_row_remains_open`: `true`
- `p8_006_production_hook_guard_remains_open`: `true`
- `runtime_or_product_behavior_changed`: `false`
- `ci_enforcement_changed`: `false`

## Limitations

- Mappings are hand-reviewed associations joined only by live discovery_id; names, paths, comments, and lexical candidates never create an association.
- A runnable direct association means an existing non-ignored test asserts part of the named anomaly stimulus; it is not invariant coverage or behavioral proof.
- Partial, context-only, ignored, and conditional associations remain test-gap rows and cannot satisfy the class.
- The Rust scanner denominator is provenance context, not a claim that every semantically relevant repository test was reviewed for Phase 8.
- VERIF-P8-002 remains open until a reviewed runtime-safety test denominator has an explicit mapped or reviewed-nonmapping disposition for every fact.
- Suite tiers are planned routing metadata. This report does not wire commands, change suite enforcement, or claim that a tier ran.
- The allocation-policy review reuses an active written contract; allocation-failure and OOM testing remains visible debt outside that claimed scan path.
- The restart-timebase review uses one closed schema-v1 state: existing_open_gap requires an actionable gap, while resolved_source binds an active reviewed source and any later closed gap must name that same resolution source; neither state creates test coverage, proof, or gap closure.
- No fault interface or production hook is added. VERIF-P8-005 and VERIF-P8-006 remain open until a governed harness and enforceable design-review boundary exist.
- The implementation board is checked live but excluded from the digest because board and evidence closure follow report generation.
