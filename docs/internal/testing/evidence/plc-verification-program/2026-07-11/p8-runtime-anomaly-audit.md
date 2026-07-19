# Phase 8 Runtime Anomaly Audit

Generator: `runtime-anomaly-audit v3`
Source revision: `3803c39a829cf4d771c8b621f48edff5d2500600`
Generated: `2026-07-19T20:10:00+02:00`
Platform: `linux-x86_64`
Generated JSON SHA-256: `4e05549615452a9a4a9b69268f63b0a08356c887586abbb1acac5c6dbad77b3d`
Input SHA-256: `sha256:4ba51de5af4ff01002a0e3fe54c3cc1a9e7f63f4e9d0aae6974c538126e2f995`

This is a report-only audit of the reviewed runtime-anomaly taxonomy,
explicit existing-test associations, open test gaps, and planned suite tiers.
It executes no fault and creates no proof or invariant coverage.

## Summary

- Taxonomy classes: 19
- Explicit mapping records: 135
- Live Rust scanner facts: 3256
- Denominator mapped facts: 135
- Denominator reviewed-nonmapping facts: 3121
- Denominator review SHA-256: `sha256:07df7d77f57109189fc6f86501a087bb323972bcc5557b192d0218b52a65ba88`
- Effectively runnable direct mappings: 125
- Ignored or conditional mappings: 1
- Gap classes: 0

## Classes

| Class | State | Primary suite | Conditional suites | Runnable mappings | Other mappings |
| --- | --- | --- | --- | --- | --- |
| `panic` | `mapped_runnable` | `pr` | `nightly`, `release` | `ANOM_MAP_PANIC_001`, `ANOM_MAP_PANIC_002`, `ANOM_MAP_PANIC_003`, `ANOM_MAP_PANIC_004`, `ANOM_MAP_PANIC_REVIEW_38786723`, `ANOM_MAP_PANIC_REVIEW_40F97415`, `ANOM_MAP_PANIC_REVIEW_9E4578A3`, `ANOM_MAP_PANIC_REVIEW_FA45370D` | none |
| `timeout` | `mapped_runnable` | `pr` | `nightly` | `ANOM_MAP_TIMEOUT_001`, `ANOM_MAP_TIMEOUT_002`, `ANOM_MAP_TIMEOUT_REVIEW_0FF9C29E`, `ANOM_MAP_TIMEOUT_REVIEW_4C97001E`, `ANOM_MAP_TIMEOUT_REVIEW_EB64DFF4` | `ANOM_MAP_TIMEOUT_003` |
| `deadline` | `mapped_runnable` | `pr` | `nightly`, `release` | `ANOM_MAP_DEADLINE_001`, `ANOM_MAP_DEADLINE_002`, `ANOM_MAP_DEADLINE_003`, `ANOM_MAP_DEADLINE_REVIEW_03FF4321`, `ANOM_MAP_DEADLINE_REVIEW_2588D30B`, `ANOM_MAP_DEADLINE_REVIEW_3605DE35`, `ANOM_MAP_DEADLINE_REVIEW_A5CEFA4B`, `ANOM_MAP_DEADLINE_REVIEW_E918297B`, `ANOM_MAP_DEADLINE_REVIEW_F36273CB`, `ANOM_MAP_DEADLINE_REVIEW_F78A6E2D`, `ANOM_MAP_DEADLINE_REVIEW_F9E7F4D4`, `ANOM_MAP_DEADLINE_REVIEW_FFEEE9C4` | none |
| `watchdog` | `mapped_runnable` | `pr` | `nightly`, `release` | `ANOM_MAP_WATCHDOG_001`, `ANOM_MAP_WATCHDOG_002`, `ANOM_MAP_WATCHDOG_003`, `ANOM_MAP_WATCHDOG_004`, `ANOM_MAP_WATCHDOG_REVIEW_08B99942`, `ANOM_MAP_WATCHDOG_REVIEW_9102EFEE`, `ANOM_MAP_WATCHDOG_REVIEW_913C5C99`, `ANOM_MAP_WATCHDOG_REVIEW_C5ABDD0A`, `ANOM_MAP_WATCHDOG_REVIEW_D66DF67B`, `ANOM_MAP_WATCHDOG_REVIEW_DAB2FA44` | none |
| `slow_device` | `mapped_runnable` | `nightly` | `release`, `hardware_lab` | `ANOM_MAP_SLOW_DEVICE_001`, `ANOM_MAP_SLOW_DEVICE_002`, `ANOM_MAP_SLOW_DEVICE_REVIEW_9CFB4226`, `ANOM_MAP_SLOW_DEVICE_REVIEW_DCF6125A` | `ANOM_MAP_SLOW_DEVICE_003` |
| `disconnect` | `mapped_runnable` | `nightly` | `release`, `hardware_lab` | `ANOM_MAP_DISCONNECT_001`, `ANOM_MAP_DISCONNECT_002`, `ANOM_MAP_DISCONNECT_REVIEW_292EA5BC`, `ANOM_MAP_DISCONNECT_REVIEW_3F7971AD`, `ANOM_MAP_DISCONNECT_REVIEW_60892A74`, `ANOM_MAP_DISCONNECT_REVIEW_682836D2`, `ANOM_MAP_DISCONNECT_REVIEW_6C2E3DC5`, `ANOM_MAP_DISCONNECT_REVIEW_72DDC249`, `ANOM_MAP_DISCONNECT_REVIEW_AB7C9A1D`, `ANOM_MAP_DISCONNECT_REVIEW_AB9EB582` | `ANOM_MAP_DISCONNECT_003`, `ANOM_MAP_DISCONNECT_REVIEW_935418D6` |
| `queue_full` | `mapped_runnable` | `nightly` | `release` | `ANOM_MAP_QUEUE_FULL_OPCUA_001` | `ANOM_MAP_QUEUE_FULL_001`, `ANOM_MAP_QUEUE_FULL_002` |
| `stale_data` | `mapped_runnable` | `pr` | `nightly`, `hardware_lab` | `ANOM_MAP_STALE_DATA_001`, `ANOM_MAP_STALE_DATA_002`, `ANOM_MAP_STALE_DATA_003`, `ANOM_MAP_STALE_DATA_REVIEW_088DF038`, `ANOM_MAP_STALE_DATA_REVIEW_168A2FE0`, `ANOM_MAP_STALE_DATA_REVIEW_35AC385C`, `ANOM_MAP_STALE_DATA_REVIEW_6AABCC29`, `ANOM_MAP_STALE_DATA_REVIEW_A536482F`, `ANOM_MAP_STALE_DATA_REVIEW_C006B362`, `ANOM_MAP_STALE_DATA_REVIEW_E8BE3DB4` | none |
| `corrupt_retain` | `mapped_runnable` | `pr` | `release` | `ANOM_MAP_CORRUPT_RETAIN_001`, `ANOM_MAP_CORRUPT_RETAIN_002` | none |
| `malformed_bytecode` | `mapped_runnable` | `pr` | `nightly` | `ANOM_MAP_MALFORMED_BYTECODE_001`, `ANOM_MAP_MALFORMED_BYTECODE_002`, `ANOM_MAP_MALFORMED_BYTECODE_003`, `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_068CB733`, `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_23536278`, `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_51976E8D`, `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_57B29BDD`, `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_5F835C5A`, `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_60C11C2F`, `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_65242C35`, `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_68143A94`, `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_7DAB1239`, `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_8CDFEC91`, `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_8E289F44`, `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_B9539D13`, `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_BDA8C505`, `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_C090BECF`, `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_C1EEFB47`, `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_C334ADB8`, `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_C624B037`, `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_CA5C342D`, `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_CC5BF7BC`, `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_D7BC4E2B`, `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_DF0C8754`, `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_DF53A63C`, `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_E22B3380`, `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_F1486738`, `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_FC4FC922` | none |
| `bad_config` | `mapped_runnable` | `pr` | `release` | `ANOM_MAP_BAD_CONFIG_001`, `ANOM_MAP_BAD_CONFIG_002`, `ANOM_MAP_BAD_CONFIG_003`, `ANOM_MAP_BAD_CONFIG_004`, `ANOM_MAP_BAD_CONFIG_REVIEW_02F1F150`, `ANOM_MAP_BAD_CONFIG_REVIEW_17824DB6`, `ANOM_MAP_BAD_CONFIG_REVIEW_33175423`, `ANOM_MAP_BAD_CONFIG_REVIEW_43D27E1F`, `ANOM_MAP_BAD_CONFIG_REVIEW_8EAC462B`, `ANOM_MAP_BAD_CONFIG_REVIEW_B3AAE6BE`, `ANOM_MAP_BAD_CONFIG_REVIEW_EB5E79BA` | none |
| `bad_signal` | `mapped_runnable` | `release` | `nightly` | `ANOM_MAP_BAD_SIGNAL_UNREVIEWED_001` | none |
| `partial_web_request` | `mapped_runnable` | `nightly` | `release` | `ANOM_MAP_PARTIAL_WEB_REQUEST_001`, `ANOM_MAP_PARTIAL_WEB_REQUEST_SATURATION_001` | none |
| `disk_error` | `mapped_runnable` | `release` | `nightly` | `ANOM_MAP_DISK_ERROR_FILE_MATRIX_001`, `ANOM_MAP_DISK_ERROR_PARENT_SYNC_001`, `ANOM_MAP_DISK_ERROR_READ_001`, `ANOM_MAP_DISK_ERROR_REAL_PATH_001`, `ANOM_MAP_DISK_ERROR_REVIEW_0CA72C3E` | `ANOM_MAP_DISK_ERROR_001`, `ANOM_MAP_DISK_ERROR_002` |
| `clock_step` | `mapped_runnable` | `nightly` | `release` | `ANOM_MAP_CLOCK_STEP_SCHEDULER_003` | `ANOM_MAP_CLOCK_STEP_001`, `ANOM_MAP_CLOCK_STEP_002` |
| `monotonic_wall_clock_divergence` | `mapped_runnable` | `nightly` | `release` | `ANOM_MAP_MONOTONIC_WALL_DIVERGENCE_001` | none |
| `suspend_resume` | `mapped_runnable` | `nightly` | `release` | `ANOM_MAP_SUSPEND_RESUME_COALESCE_001` | none |
| `timer_duration_overflow` | `mapped_runnable` | `pr` | `release` | `ANOM_MAP_TIMER_DURATION_OVERFLOW_SIM_CLOCK_001`, `ANOM_MAP_TIMER_DURATION_OVERFLOW_TOF_001`, `ANOM_MAP_TIMER_DURATION_OVERFLOW_TON_001`, `ANOM_MAP_TIMER_DURATION_OVERFLOW_TP_001` | none |
| `allocation_failure_oom` | `mapped_runnable` | `nightly` | `release` | `ANOM_MAP_ALLOCATION_FAILURE_BOUNDS_001`, `ANOM_MAP_ALLOCATION_FAILURE_OOM_REVIEW_01A4060E`, `ANOM_MAP_ALLOCATION_FAILURE_OOM_REVIEW_5815D217`, `ANOM_MAP_ALLOCATION_FAILURE_OOM_REVIEW_5BFE0DC6`, `ANOM_MAP_ALLOCATION_FAILURE_OOM_REVIEW_67A48271`, `ANOM_MAP_ALLOCATION_FAILURE_OOM_REVIEW_8051C670`, `ANOM_MAP_ALLOCATION_FAILURE_OOM_REVIEW_A409C1E1`, `ANOM_MAP_ALLOCATION_FAILURE_OOM_REVIEW_B2E06B11`, `ANOM_MAP_ALLOCATION_FAILURE_OOM_REVIEW_B5D2ECF1` | none |

## Explicit Associations

| Mapping | Class | Test | Kind | Ignore state | Runnable | Mechanism |
| --- | --- | --- | --- | --- | --- | --- |
| `ANOM_MAP_ALLOCATION_FAILURE_BOUNDS_001` | `allocation_failure_oom` | `DISC_4BABD0A9C6328EC16A6A` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_ALLOCATION_FAILURE_OOM_REVIEW_01A4060E` | `allocation_failure_oom` | `DISC_6EDF6218DDBF01A4060E` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_ALLOCATION_FAILURE_OOM_REVIEW_5815D217` | `allocation_failure_oom` | `DISC_FE665C5DDCC65815D217` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_ALLOCATION_FAILURE_OOM_REVIEW_5BFE0DC6` | `allocation_failure_oom` | `DISC_B9ACE9B650FE5BFE0DC6` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_ALLOCATION_FAILURE_OOM_REVIEW_67A48271` | `allocation_failure_oom` | `DISC_356CDEB0445067A48271` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_ALLOCATION_FAILURE_OOM_REVIEW_8051C670` | `allocation_failure_oom` | `DISC_CF07A66B6BF18051C670` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_ALLOCATION_FAILURE_OOM_REVIEW_A409C1E1` | `allocation_failure_oom` | `DISC_4D65BEF6CD15A409C1E1` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_ALLOCATION_FAILURE_OOM_REVIEW_B2E06B11` | `allocation_failure_oom` | `DISC_98E387D5D8C3B2E06B11` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_ALLOCATION_FAILURE_OOM_REVIEW_B5D2ECF1` | `allocation_failure_oom` | `DISC_7B56C0BED850B5D2ECF1` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_BAD_CONFIG_001` | `bad_config` | `DISC_78139822FEDCB8E1A379` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_BAD_CONFIG_002` | `bad_config` | `DISC_C9AB817C3804CDAD2BAF` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_BAD_CONFIG_003` | `bad_config` | `DISC_7CCA5C983F3BC05339EC` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_BAD_CONFIG_004` | `bad_config` | `DISC_86A13C88CFA096648AFB` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_BAD_CONFIG_REVIEW_02F1F150` | `bad_config` | `DISC_22D364962DA502F1F150` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_BAD_CONFIG_REVIEW_17824DB6` | `bad_config` | `DISC_129EF130404117824DB6` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_BAD_CONFIG_REVIEW_33175423` | `bad_config` | `DISC_2CE088F1926B33175423` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_BAD_CONFIG_REVIEW_43D27E1F` | `bad_config` | `DISC_D6DAC722D68243D27E1F` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_BAD_CONFIG_REVIEW_8EAC462B` | `bad_config` | `DISC_6F3F154A1A548EAC462B` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_BAD_CONFIG_REVIEW_B3AAE6BE` | `bad_config` | `DISC_FB97F87B230CB3AAE6BE` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_BAD_CONFIG_REVIEW_EB5E79BA` | `bad_config` | `DISC_F6E17543C6E5EB5E79BA` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_BAD_SIGNAL_UNREVIEWED_001` | `bad_signal` | `DISC_57D298DD438F5E6F5975` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_CLOCK_STEP_001` | `clock_step` | `DISC_1F4A6A7E004592367309` | `partial` | `not_ignored` | `false` | `ordinary_input` |
| `ANOM_MAP_CLOCK_STEP_002` | `clock_step` | `DISC_2CC9B67E44CB8284B149` | `context_only` | `not_ignored` | `false` | `test_harness` |
| `ANOM_MAP_CLOCK_STEP_SCHEDULER_003` | `clock_step` | `DISC_F6508FDFBB3BD1D4F440` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_CORRUPT_RETAIN_001` | `corrupt_retain` | `DISC_6536BD14A5FAEA351CCF` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_CORRUPT_RETAIN_002` | `corrupt_retain` | `DISC_D57F0AA703E76450F141` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_DEADLINE_001` | `deadline` | `DISC_55D5957DD6826E1A6C0F` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_DEADLINE_002` | `deadline` | `DISC_E35FF38A598A52B9A748` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_DEADLINE_003` | `deadline` | `DISC_4C3F5D7171073A654EA8` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_DEADLINE_REVIEW_03FF4321` | `deadline` | `DISC_8EF2B55E3EBA03FF4321` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_DEADLINE_REVIEW_2588D30B` | `deadline` | `DISC_34CD5052A10F2588D30B` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_DEADLINE_REVIEW_3605DE35` | `deadline` | `DISC_769ABE73CC183605DE35` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_DEADLINE_REVIEW_A5CEFA4B` | `deadline` | `DISC_1D15CD11714BA5CEFA4B` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_DEADLINE_REVIEW_E918297B` | `deadline` | `DISC_7E40E58CEF1BE918297B` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_DEADLINE_REVIEW_F36273CB` | `deadline` | `DISC_FB2DC1C843F8F36273CB` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_DEADLINE_REVIEW_F78A6E2D` | `deadline` | `DISC_D74A24E1A5D2F78A6E2D` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_DEADLINE_REVIEW_F9E7F4D4` | `deadline` | `DISC_4F5DDFB0E5B6F9E7F4D4` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_DEADLINE_REVIEW_FFEEE9C4` | `deadline` | `DISC_4E8E4144D9AEFFEEE9C4` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_DISCONNECT_001` | `disconnect` | `DISC_D0022CD29DB5061F146B` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_DISCONNECT_002` | `disconnect` | `DISC_0964F1D88F24D71AD970` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_DISCONNECT_003` | `disconnect` | `DISC_C3A91E2860C30B08BED4` | `partial` | `not_ignored` | `false` | `test_harness` |
| `ANOM_MAP_DISCONNECT_REVIEW_292EA5BC` | `disconnect` | `DISC_8A42BABA8B61292EA5BC` | `direct` | `not_ignored` | `true` | `external_harness` |
| `ANOM_MAP_DISCONNECT_REVIEW_3F7971AD` | `disconnect` | `DISC_2CD8F1025F7B3F7971AD` | `direct` | `not_ignored` | `true` | `external_harness` |
| `ANOM_MAP_DISCONNECT_REVIEW_60892A74` | `disconnect` | `DISC_1F69D92EC8C260892A74` | `direct` | `not_ignored` | `true` | `external_harness` |
| `ANOM_MAP_DISCONNECT_REVIEW_682836D2` | `disconnect` | `DISC_A89C5FA2356D682836D2` | `direct` | `not_ignored` | `true` | `external_harness` |
| `ANOM_MAP_DISCONNECT_REVIEW_6C2E3DC5` | `disconnect` | `DISC_ABF55C65457D6C2E3DC5` | `direct` | `not_ignored` | `true` | `external_harness` |
| `ANOM_MAP_DISCONNECT_REVIEW_72DDC249` | `disconnect` | `DISC_FE959E7B21A272DDC249` | `direct` | `not_ignored` | `true` | `external_harness` |
| `ANOM_MAP_DISCONNECT_REVIEW_935418D6` | `disconnect` | `DISC_75F421CF89F3935418D6` | `direct` | `ignored` | `false` | `external_harness` |
| `ANOM_MAP_DISCONNECT_REVIEW_AB7C9A1D` | `disconnect` | `DISC_2A6A13834385AB7C9A1D` | `direct` | `not_ignored` | `true` | `external_harness` |
| `ANOM_MAP_DISCONNECT_REVIEW_AB9EB582` | `disconnect` | `DISC_0C0EB84CEED3AB9EB582` | `direct` | `not_ignored` | `true` | `external_harness` |
| `ANOM_MAP_DISK_ERROR_001` | `disk_error` | `DISC_CAF0750D558B8114BEC6` | `partial` | `not_ignored` | `false` | `test_harness` |
| `ANOM_MAP_DISK_ERROR_002` | `disk_error` | `DISC_B25F48AA676D50A29BB2` | `partial` | `not_ignored` | `false` | `test_harness` |
| `ANOM_MAP_DISK_ERROR_FILE_MATRIX_001` | `disk_error` | `DISC_A8FA0CAF53A7D6D0134E` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_DISK_ERROR_PARENT_SYNC_001` | `disk_error` | `DISC_55C765213824F6A01A03` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_DISK_ERROR_READ_001` | `disk_error` | `DISC_402CC51B509F2C268C12` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_DISK_ERROR_REAL_PATH_001` | `disk_error` | `DISC_32E18E31451D4BF469C6` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_DISK_ERROR_REVIEW_0CA72C3E` | `disk_error` | `DISC_65D98FB688680CA72C3E` | `direct` | `not_ignored` | `true` | `external_harness` |
| `ANOM_MAP_MALFORMED_BYTECODE_001` | `malformed_bytecode` | `DISC_F90D4502D7B68E02847C` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_MALFORMED_BYTECODE_002` | `malformed_bytecode` | `DISC_FB4371C17A9F9FB83CA9` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_MALFORMED_BYTECODE_003` | `malformed_bytecode` | `DISC_51482BB47DB5575280CE` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_068CB733` | `malformed_bytecode` | `DISC_36947D824681068CB733` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_23536278` | `malformed_bytecode` | `DISC_3308821824B623536278` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_51976E8D` | `malformed_bytecode` | `DISC_547557A88A1751976E8D` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_57B29BDD` | `malformed_bytecode` | `DISC_E8D36E045CD657B29BDD` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_5F835C5A` | `malformed_bytecode` | `DISC_A324C9FF622A5F835C5A` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_60C11C2F` | `malformed_bytecode` | `DISC_3A6CAB92BC8F60C11C2F` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_65242C35` | `malformed_bytecode` | `DISC_C74FB437C18E65242C35` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_68143A94` | `malformed_bytecode` | `DISC_66836DF45D5768143A94` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_7DAB1239` | `malformed_bytecode` | `DISC_F1F0C6C285477DAB1239` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_8CDFEC91` | `malformed_bytecode` | `DISC_06C78E4FD9548CDFEC91` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_8E289F44` | `malformed_bytecode` | `DISC_C160AB9422688E289F44` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_B9539D13` | `malformed_bytecode` | `DISC_1685CC6D1F65B9539D13` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_BDA8C505` | `malformed_bytecode` | `DISC_6D15F1D43CA3BDA8C505` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_C090BECF` | `malformed_bytecode` | `DISC_A6FCBE2677A8C090BECF` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_C1EEFB47` | `malformed_bytecode` | `DISC_70C232CD199EC1EEFB47` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_C334ADB8` | `malformed_bytecode` | `DISC_E8BDC2A27F6EC334ADB8` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_C624B037` | `malformed_bytecode` | `DISC_1ACB18866C2FC624B037` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_CA5C342D` | `malformed_bytecode` | `DISC_0D216C5A37ABCA5C342D` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_CC5BF7BC` | `malformed_bytecode` | `DISC_FDE1B5B9C014CC5BF7BC` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_D7BC4E2B` | `malformed_bytecode` | `DISC_EC610E3917E2D7BC4E2B` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_DF0C8754` | `malformed_bytecode` | `DISC_80833EFC8B17DF0C8754` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_DF53A63C` | `malformed_bytecode` | `DISC_1B5448C8DB96DF53A63C` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_E22B3380` | `malformed_bytecode` | `DISC_B4A1DBB63C27E22B3380` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_F1486738` | `malformed_bytecode` | `DISC_F850C3D482F1F1486738` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_MALFORMED_BYTECODE_REVIEW_FC4FC922` | `malformed_bytecode` | `DISC_2017F195D5EEFC4FC922` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_MONOTONIC_WALL_DIVERGENCE_001` | `monotonic_wall_clock_divergence` | `DISC_AC89BA6FAA7BC42ACCBC` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_PANIC_001` | `panic` | `DISC_30C382889325B64C5854` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_PANIC_002` | `panic` | `DISC_E6963439801BFF301755` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_PANIC_003` | `panic` | `DISC_96A8793BF2C3E6F579B4` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_PANIC_004` | `panic` | `DISC_EEF14FDB800A7590C8F3` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_PANIC_REVIEW_38786723` | `panic` | `DISC_92FCECA7163F38786723` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_PANIC_REVIEW_40F97415` | `panic` | `DISC_D4793CC01E1740F97415` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_PANIC_REVIEW_9E4578A3` | `panic` | `DISC_C946B9FD799F9E4578A3` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_PANIC_REVIEW_FA45370D` | `panic` | `DISC_BF01EBAA4B0DFA45370D` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_PARTIAL_WEB_REQUEST_001` | `partial_web_request` | `DISC_A77CDB943FD0784CC65A` | `direct` | `not_ignored` | `true` | `external_harness` |
| `ANOM_MAP_PARTIAL_WEB_REQUEST_SATURATION_001` | `partial_web_request` | `DISC_7B7DA852E4D9CE86AF04` | `direct` | `not_ignored` | `true` | `external_harness` |
| `ANOM_MAP_QUEUE_FULL_001` | `queue_full` | `DISC_398610728B7609607B57` | `partial` | `not_ignored` | `false` | `external_harness` |
| `ANOM_MAP_QUEUE_FULL_002` | `queue_full` | `DISC_36444246A468C534110A` | `partial` | `not_ignored` | `false` | `test_harness` |
| `ANOM_MAP_QUEUE_FULL_OPCUA_001` | `queue_full` | `DISC_D074D829A8C6B93DAC92` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_SLOW_DEVICE_001` | `slow_device` | `DISC_7159C3BA77CC33C8F48C` | `direct` | `not_ignored` | `true` | `external_harness` |
| `ANOM_MAP_SLOW_DEVICE_002` | `slow_device` | `DISC_6C10B5D7ACB21ECE8E9D` | `direct` | `not_ignored` | `true` | `external_harness` |
| `ANOM_MAP_SLOW_DEVICE_003` | `slow_device` | `DISC_D2A5AD5E1D2E5D50D9E4` | `context_only` | `not_ignored` | `false` | `test_harness` |
| `ANOM_MAP_SLOW_DEVICE_REVIEW_9CFB4226` | `slow_device` | `DISC_22FFF435A5C79CFB4226` | `direct` | `not_ignored` | `true` | `external_harness` |
| `ANOM_MAP_SLOW_DEVICE_REVIEW_DCF6125A` | `slow_device` | `DISC_50F0CBE7F09ADCF6125A` | `direct` | `not_ignored` | `true` | `external_harness` |
| `ANOM_MAP_STALE_DATA_001` | `stale_data` | `DISC_B3EBD5115E6DBD66F946` | `direct` | `not_ignored` | `true` | `external_harness` |
| `ANOM_MAP_STALE_DATA_002` | `stale_data` | `DISC_ADB41A6325C30D12D0AD` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_STALE_DATA_003` | `stale_data` | `DISC_5E088D53395CC4F852E5` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_STALE_DATA_REVIEW_088DF038` | `stale_data` | `DISC_F3555CD7ED8E088DF038` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_STALE_DATA_REVIEW_168A2FE0` | `stale_data` | `DISC_0CB6E85705D0168A2FE0` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_STALE_DATA_REVIEW_35AC385C` | `stale_data` | `DISC_5F738C6D18E335AC385C` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_STALE_DATA_REVIEW_6AABCC29` | `stale_data` | `DISC_391F410D2DA36AABCC29` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_STALE_DATA_REVIEW_A536482F` | `stale_data` | `DISC_7ABC26CF210FA536482F` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_STALE_DATA_REVIEW_C006B362` | `stale_data` | `DISC_7057CE85A02EC006B362` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_STALE_DATA_REVIEW_E8BE3DB4` | `stale_data` | `DISC_5D87E3264428E8BE3DB4` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_SUSPEND_RESUME_COALESCE_001` | `suspend_resume` | `DISC_891F1D234DBDE29429A4` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_TIMEOUT_001` | `timeout` | `DISC_0BD76C389DCA0387316F` | `direct` | `not_ignored` | `true` | `external_harness` |
| `ANOM_MAP_TIMEOUT_002` | `timeout` | `DISC_AD782C3AD1D808D4C4EE` | `direct` | `not_ignored` | `true` | `external_harness` |
| `ANOM_MAP_TIMEOUT_003` | `timeout` | `DISC_17A3C97ED5EF6F1D342D` | `partial` | `not_ignored` | `false` | `test_harness` |
| `ANOM_MAP_TIMEOUT_REVIEW_0FF9C29E` | `timeout` | `DISC_5A83D1A380180FF9C29E` | `direct` | `not_ignored` | `true` | `external_harness` |
| `ANOM_MAP_TIMEOUT_REVIEW_4C97001E` | `timeout` | `DISC_3FD9DCA298624C97001E` | `direct` | `not_ignored` | `true` | `external_harness` |
| `ANOM_MAP_TIMEOUT_REVIEW_EB64DFF4` | `timeout` | `DISC_D0A4FCC61D908C1E0CA3` | `direct` | `not_ignored` | `true` | `external_harness` |
| `ANOM_MAP_TIMER_DURATION_OVERFLOW_SIM_CLOCK_001` | `timer_duration_overflow` | `DISC_33E5D40504EBC4C32030` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_TIMER_DURATION_OVERFLOW_TOF_001` | `timer_duration_overflow` | `DISC_DA4DF0247AD9823BF96F` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_TIMER_DURATION_OVERFLOW_TON_001` | `timer_duration_overflow` | `DISC_93D5E86E7CF67A1A5AAE` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_TIMER_DURATION_OVERFLOW_TP_001` | `timer_duration_overflow` | `DISC_FF77A462527037C6EEA8` | `direct` | `not_ignored` | `true` | `ordinary_input` |
| `ANOM_MAP_WATCHDOG_001` | `watchdog` | `DISC_C101BBAEC06311987C74` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_WATCHDOG_002` | `watchdog` | `DISC_3EFD5D5E736CB0C47DD6` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_WATCHDOG_003` | `watchdog` | `DISC_8BAF8461F95A94A773A6` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_WATCHDOG_004` | `watchdog` | `DISC_753F27F0D2AE25935D15` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_WATCHDOG_REVIEW_08B99942` | `watchdog` | `DISC_8B901DEF0D4408B99942` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_WATCHDOG_REVIEW_9102EFEE` | `watchdog` | `DISC_F6F3665F8B379102EFEE` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_WATCHDOG_REVIEW_913C5C99` | `watchdog` | `DISC_4E0EFE2F6CE2913C5C99` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_WATCHDOG_REVIEW_C5ABDD0A` | `watchdog` | `DISC_22429344F278C5ABDD0A` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_WATCHDOG_REVIEW_D66DF67B` | `watchdog` | `DISC_5408211286BED66DF67B` | `direct` | `not_ignored` | `true` | `test_harness` |
| `ANOM_MAP_WATCHDOG_REVIEW_DAB2FA44` | `watchdog` | `DISC_D8D92D6082D7DAB2FA44` | `direct` | `not_ignored` | `true` | `test_harness` |

## Test Gaps

| Class | State | Reason | Planned suite | Associations |
| --- | --- | --- | --- | --- |

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
- `fault_interfaces_implemented`: `true`
- `production_fault_hooks_added`: `false`
- `p8_002_exhaustive_review_complete`: `true`
- `p8_005_fault_toggle_row_remains_open`: `false`
- `p8_006_production_hook_guard_remains_open`: `false`
- `runtime_or_product_behavior_changed`: `false`
- `ci_enforcement_changed`: `false`

## Limitations

- Mappings are hand-reviewed associations joined only by live discovery_id; names, paths, comments, and lexical candidates never create an association.
- A runnable direct association means an existing non-ignored test asserts part of the named anomaly stimulus; it is not invariant coverage or behavioral proof.
- Partial, context-only, ignored, and conditional associations remain test-gap rows and cannot satisfy the class.
- The committed denominator ledger binds every live Rust fact by discovery ID, source kind, path, and name to either an existing explicit association or an explicit reviewed-nonmapping rationale.
- Exhaustive denominator review does not turn nonmapping facts into anomaly coverage, proof, or an assertion that their ordinary behavior is adequate.
- Suite tiers are planned routing metadata. This report does not wire commands, change suite enforcement, or claim that a tier ran.
- The allocation-policy review reuses an active written contract; allocation-failure and OOM testing remains visible debt outside that claimed scan path.
- The restart-timebase review uses one closed schema-v1 state: existing_open_gap requires an actionable gap, while resolved_source binds an active reviewed source and any later closed gap must name that same resolution source; neither state creates test coverage, proof, or gap closure.
- Fault stimuli are admitted only through exact scanner-bound test_harness or external_harness mappings; ordinary_input records are not fault toggles and no general production toggle is introduced.
- The metadata gate rejects production Cargo features and public runtime symbols with fault-hook vocabulary. Production hooks remain prohibited pending an explicit reviewed design and contract update.
- The implementation board is checked live but excluded from the digest because board and evidence closure follow report generation.
